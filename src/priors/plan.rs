//! Turn a module's `[priors]` config (or the zero-config default) into a
//! concrete, resolver-ready plan.
//!
//! The plan is pure config-derivation: it does not touch the transport or read
//! any notes. It resolves the zero-config defaults (§3), classifies each
//! `match_on` key's mode from the field's `wikilink` flag, derives the hard
//! filter gates from the module's `show_when` rules (§4.1), parses the
//! `rank_by` grammar, and picks the `show` fields. Validation of the config has already
//! happened in `Config::validate` — this module assumes a valid config and
//! falls back sensibly on anything it does not recognise.

use crate::config::{FieldConfig, FieldType, MatchOn, ModuleConfig, ShowField};

/// Default number of priors (one column each) when `limit` is absent (§3).
pub const DEFAULT_LIMIT: usize = 3;

/// How a single `match_on` key is compared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchMode {
    /// Exact frontmatter value equality.
    Equality,
    /// Strip `[[ ]]`/alias/fragment on both sides, then compare targets.
    Wikilink,
}

/// One resolved match key: a frontmatter field plus how to compare it.
#[derive(Debug, Clone)]
pub struct MatchKey {
    pub field: String,
    pub mode: MatchMode,
}

/// Aggregation for a number row in the opt-in summary column (§6). Only the
/// numeric aggregations are modelled; select/tag `mode` summaries are L2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agg {
    Median,
    Mean,
    Max,
    Min,
    Latest,
}

impl Agg {
    fn parse(s: Option<&str>) -> Agg {
        match s {
            Some("mean") => Agg::Mean,
            Some("max") => Agg::Max,
            Some("min") => Agg::Min,
            Some("latest") => Agg::Latest,
            // Default (and "median") → median.
            _ => Agg::Median,
        }
    }
}

/// A resolved `show` entry: a field that gets a cell in each prior's column.
#[derive(Debug, Clone)]
pub struct ShowColumn {
    pub field: String,
    pub agg: Agg,
    /// Whether the field is a `number` field (only these get a summary cell).
    pub numeric: bool,
}

/// How priors with equal similarity are ordered (§5).
#[derive(Debug, Clone)]
pub enum RankBy {
    /// Break ties by a numeric field, descending or ascending. Priors missing
    /// the field sort after the ones that have it and render dimmed.
    Field { field: String, descending: bool },
    /// Break ties by recency alone, newest first.
    Recent,
    /// Preserve scan order between equally similar priors.
    None,
}

/// A fully-resolved priors plan.
#[derive(Debug, Clone)]
pub struct PriorsPlan {
    /// Ordered match keys, most → least important. Each agreeing key adds one
    /// to a prior's score; earlier keys break ties (§4.1).
    pub match_keys: Vec<MatchKey>,
    /// Hard-filter gates: every field another field's `show_when` names. A
    /// prior must match the form's value for each gate the form has filled.
    pub gates: Vec<MatchKey>,
    pub rank_by: RankBy,
    pub show: Vec<ShowColumn>,
    pub limit: usize,
    /// Whether the opt-in summary column is on (§6).
    pub summary: bool,
}

impl PriorsPlan {
    /// Build the plan for a module — from its `[priors]` block if present, else
    /// from the zero-config default (§3).
    pub fn build(module: &ModuleConfig) -> PriorsPlan {
        match &module.priors {
            Some(cfg) => Self::from_config(module, cfg),
            None => Self::zero_config(module),
        }
    }

    fn from_config(module: &ModuleConfig, cfg: &crate::config::PriorsConfig) -> PriorsPlan {
        let match_keys = cfg
            .match_on
            .iter()
            .map(|m| resolve_match_key(module, m))
            .collect();

        let rank_by = parse_rank_by(cfg.rank_by.as_deref());

        let show = if cfg.show.is_empty() {
            default_show(module)
        } else {
            cfg.show
                .iter()
                .map(|s| resolve_show_column(module, s))
                .collect()
        };

        PriorsPlan {
            match_keys,
            gates: gate_keys(module),
            rank_by,
            show,
            limit: cfg.limit.unwrap_or(DEFAULT_LIMIT),
            summary: cfg.summary.unwrap_or(false),
        }
    }

    fn zero_config(module: &ModuleConfig) -> PriorsPlan {
        PriorsPlan {
            match_keys: zero_config_match_keys(module),
            gates: gate_keys(module),
            rank_by: RankBy::Recent,
            show: default_show(module),
            limit: DEFAULT_LIMIT,
            summary: false,
        }
    }

    /// The form fields whose change re-resolves the panel: every `match_on`
    /// field plus every gate, without duplicates, in that order.
    pub fn trigger_fields(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for k in self.match_keys.iter().chain(&self.gates) {
            if !out.contains(&k.field.as_str()) {
                out.push(&k.field);
            }
        }
        out
    }
}

/// Zero-config `match_on`: every `wikilink`/select field, in config order.
fn zero_config_match_keys(module: &ModuleConfig) -> Vec<MatchKey> {
    module
        .fields
        .iter()
        .filter_map(|f| {
            let is_select = matches!(
                f.field_type,
                FieldType::StaticSelect | FieldType::DynamicSelect
            );
            let is_wikilink = f.wikilink.unwrap_or(false);
            if is_wikilink || is_select {
                Some(MatchKey {
                    field: f.name.clone(),
                    mode: if is_wikilink {
                        MatchMode::Wikilink
                    } else {
                        MatchMode::Equality
                    },
                })
            } else {
                None
            }
        })
        .collect()
}

/// The hard-filter gates (§4.1): every field that another field's `show_when`
/// names, in config order, compared like a `match_on` key on that field.
/// Derived from config alone, so no module's field names are hardcoded here.
fn gate_keys(module: &ModuleConfig) -> Vec<MatchKey> {
    module
        .fields
        .iter()
        .filter(|g| {
            module
                .fields
                .iter()
                .any(|f| f.show_when.as_ref().is_some_and(|sw| sw.field == g.name))
        })
        .map(|g| MatchKey {
            field: g.name.clone(),
            mode: if g.wikilink.unwrap_or(false) {
                MatchMode::Wikilink
            } else {
                MatchMode::Equality
            },
        })
        .collect()
}

/// Default `show` (no block, or a block without `show`): every field in config
/// order except `textarea` and `composite_array`. Textarea values live in the
/// body, and composite rows render blank because presets cover them (§8.1).
fn default_show(module: &ModuleConfig) -> Vec<ShowColumn> {
    module
        .fields
        .iter()
        .filter(|f| {
            !matches!(
                f.field_type,
                FieldType::Textarea | FieldType::CompositeArray
            )
        })
        .map(|f| ShowColumn {
            field: f.name.clone(),
            agg: Agg::Median,
            numeric: f.field_type == FieldType::Number,
        })
        .collect()
}

/// Resolve a configured `match_on` entry into a `MatchKey`. The mode is derived
/// from the referenced field's `wikilink` flag unless the object form set one.
fn resolve_match_key(module: &ModuleConfig, m: &MatchOn) -> MatchKey {
    let field = m.field().to_string();
    let mode = match m.mode() {
        Some("wikilink") => MatchMode::Wikilink,
        Some("equality") => MatchMode::Equality,
        // Bare string (or unrecognised): infer from the field's wikilink flag.
        _ => {
            if field_is_wikilink(module, &field) {
                MatchMode::Wikilink
            } else {
                MatchMode::Equality
            }
        }
    };
    MatchKey { field, mode }
}

fn resolve_show_column(module: &ModuleConfig, s: &ShowField) -> ShowColumn {
    let field = s.field().to_string();
    ShowColumn {
        agg: Agg::parse(s.agg()),
        numeric: field_type(module, &field) == Some(FieldType::Number),
        field,
    }
}

fn parse_rank_by(rank_by: Option<&str>) -> RankBy {
    match rank_by.map(str::trim) {
        None | Some("recent") => RankBy::Recent,
        Some("none") => RankBy::None,
        Some(spec) => {
            let parts: Vec<&str> = spec.split_whitespace().collect();
            match parts.as_slice() {
                [field, "desc"] => RankBy::Field {
                    field: field.to_string(),
                    descending: true,
                },
                [field, "asc"] => RankBy::Field {
                    field: field.to_string(),
                    descending: false,
                },
                // Anything else was rejected by validation; be safe and treat
                // it as recency rather than panicking.
                _ => RankBy::Recent,
            }
        }
    }
}

fn find_field<'a>(module: &'a ModuleConfig, name: &str) -> Option<&'a FieldConfig> {
    module.fields.iter().find(|f| f.name == name)
}

fn field_is_wikilink(module: &ModuleConfig, name: &str) -> bool {
    find_field(module, name)
        .and_then(|f| f.wikilink)
        .unwrap_or(false)
}

fn field_type(module: &ModuleConfig, name: &str) -> Option<FieldType> {
    find_field(module, name).map(|f| f.field_type.clone())
}
