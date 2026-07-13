//! Turn a module's `[priors]` config (or the zero-config default) into a
//! concrete, resolver-ready plan.
//!
//! The plan is pure config-derivation: it does not touch the transport or read
//! any notes. It resolves the zero-config defaults (§3), classifies each
//! `match_on` key's mode from the field's `wikilink` flag, parses the `rank_by`
//! grammar, and picks the `show` columns. Validation of the config has already
//! happened in `Config::validate` — this module assumes a valid config and
//! falls back sensibly on anything it does not recognise.

use crate::config::{FieldConfig, FieldType, MatchOn, ModuleConfig, ShowField};

/// Default row limit when `limit` is absent (§3).
pub const DEFAULT_LIMIT: usize = 5;

/// Zero-config `show` column cap (§3).
const ZERO_CONFIG_SHOW_CAP: usize = 4;

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

/// Aggregation for a numeric summary field (§6). Only the numeric aggregations
/// are modelled in L1; `show` fields are numeric on coffee's path.
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

/// A resolved `show` column.
#[derive(Debug, Clone)]
pub struct ShowColumn {
    pub field: String,
    pub agg: Agg,
    /// Whether the field is numeric (drives right-alignment + summary inclusion).
    pub numeric: bool,
}

/// How the matched rows are ordered (§5).
#[derive(Debug, Clone)]
pub enum RankBy {
    /// Sort by a field, descending or ascending. Rows split into qualifying
    /// (have the field) and texture (missing it).
    Field { field: String, descending: bool },
    /// Newest capture first. Every row qualifies; no texture split.
    Recent,
    /// Preserve scan order, unranked. Every row qualifies.
    None,
}

/// A fully-resolved priors plan.
#[derive(Debug, Clone)]
pub struct PriorsPlan {
    /// Ordered match keys, most → least specific (drives the cascade).
    pub match_keys: Vec<MatchKey>,
    pub rank_by: RankBy,
    pub show: Vec<ShowColumn>,
    pub limit: usize,
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
            zero_config_show(module)
        } else {
            cfg.show
                .iter()
                .map(|s| resolve_show_column(module, s))
                .collect()
        };

        let limit = cfg.limit.unwrap_or(DEFAULT_LIMIT);

        PriorsPlan {
            match_keys,
            rank_by,
            show,
            limit,
        }
    }

    fn zero_config(module: &ModuleConfig) -> PriorsPlan {
        // Match on the module's first wikilink/select field, if one exists.
        let match_keys = first_match_field(module)
            .map(|k| vec![k])
            .unwrap_or_default();

        PriorsPlan {
            match_keys,
            rank_by: RankBy::Recent,
            show: zero_config_show(module),
            limit: DEFAULT_LIMIT,
        }
    }
}

/// The zero-config match field: the module's first `wikilink`/select field.
fn first_match_field(module: &ModuleConfig) -> Option<MatchKey> {
    module.fields.iter().find_map(|f| {
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
}

/// Zero-config `show`: numeric + select fields in config order, capped at 4.
fn zero_config_show(module: &ModuleConfig) -> Vec<ShowColumn> {
    module
        .fields
        .iter()
        .filter(|f| {
            matches!(
                f.field_type,
                FieldType::Number | FieldType::StaticSelect | FieldType::DynamicSelect
            )
        })
        .take(ZERO_CONFIG_SHOW_CAP)
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
