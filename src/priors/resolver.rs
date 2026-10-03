//! The priors resolver: hard filter, similarity score, ordering, and the
//! opt-in summary column. Pure over an in-memory corpus of captures — no
//! transport, no terminal — so it is fully unit-testable against fixtures.
//!
//! The output is reference material (§2): each [`PriorColumn`] is one real
//! prior with its raw cell values. Whether a cell equals the form's current
//! value is decided at render time with [`same_as_current`], so the `·` marks
//! follow the live form without fetching the corpus again.

use std::cmp::Ordering;
use std::collections::HashMap;

use crate::data::frontmatter_read::{Frontmatter, FrontmatterValue};
use crate::data::wikilink::strip_wikilink;

use super::plan::{Agg, MatchKey, MatchMode, PriorsPlan, RankBy, ShowColumn};

/// A single prior capture: its parsed frontmatter plus a recency key.
///
/// `recency` is the note's mtime in milliseconds since the Unix epoch on both
/// transport paths (higher = newer). The resolver only compares it; the panel
/// footer turns it into an age with [`age_label`].
#[derive(Debug, Clone)]
pub struct Capture {
    pub frontmatter: Frontmatter,
    pub recency: i64,
    /// Vault-relative path, carried through for potential display / dedup.
    pub path: String,
}

impl Capture {
    /// Read a scalar frontmatter value as a string, if present and scalar.
    fn scalar(&self, field: &str) -> Option<&str> {
        self.frontmatter
            .get(field)
            .and_then(FrontmatterValue::as_scalar)
    }

    /// Read a frontmatter field as an `f64`, if it parses cleanly.
    fn number(&self, field: &str) -> Option<f64> {
        self.scalar(field)
            .and_then(|s| s.trim().parse::<f64>().ok())
    }
}

/// One prior, rendered as one panel column.
#[derive(Debug, Clone)]
pub struct PriorColumn {
    /// One cell per `show` field, in [`ResolvedPanel::fields`] order. `None`
    /// when the prior lacks the field (the cell renders blank).
    pub cells: Vec<Option<String>>,
    /// The prior's `rank_by` value for the column header, when `rank_by` names
    /// a field and the prior has it.
    pub rank_value: Option<String>,
    /// `true` when `rank_by` names a field this prior lacks. The column renders
    /// dimmed (§5).
    pub dimmed: bool,
    /// How many `match_on` fields agree with the form (§4.1).
    pub score: usize,
    /// Recency key (mtime ms), for the age footer.
    pub recency: i64,
    pub path: String,
}

/// The opt-in summary column (§6).
#[derive(Debug, Clone)]
pub struct SummaryColumn {
    /// Header label: the aggregation name when every number row shares one
    /// (`median`), else `summary`.
    pub label: String,
    /// One cell per `show` field, aligned with [`ResolvedPanel::fields`].
    /// `Some` only on number rows with at least one value among the displayed
    /// priors.
    pub cells: Vec<Option<String>>,
}

/// The resolved panel.
#[derive(Debug, Clone)]
pub struct ResolvedPanel {
    /// The `show` field names, in plan order. Every column's `cells` and the
    /// summary's `cells` line up with this list.
    pub fields: Vec<String>,
    /// The priors, best first, at most `limit`. Empty is the empty state (no
    /// prior survived the hard filter).
    pub columns: Vec<PriorColumn>,
    /// `true` when the form has at least one `match_on` value to compare and
    /// every displayed prior scored zero (`no close match`). With nothing
    /// filled there is nothing to miss, so the header stays `similar`.
    pub no_close_match: bool,
    /// `"<field> <dir>"` when `rank_by` names a field, else `None`.
    pub rank_label: Option<String>,
    /// `Some` when `summary = true` and there is at least one column.
    pub summary: Option<SummaryColumn>,
}

impl ResolvedPanel {
    /// Whether this is the empty state (no prior survived the hard filter).
    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    /// The header text: `similar` or `no close match`, plus the `rank_by`
    /// label when one is set. Never says "best" (§5).
    pub fn header(&self) -> String {
        match &self.rank_label {
            Some(label) => format!("{} · {label}", self.header_base()),
            None => self.header_base().to_string(),
        }
    }

    /// The header without the `rank_by` label: `similar` or `no close match`.
    /// The panel falls back to this when the full header doesn't fit.
    pub fn header_base(&self) -> &'static str {
        if self.no_close_match {
            "no close match"
        } else {
            "similar"
        }
    }

    /// The cell a column holds for `field`, if `field` is a `show` field and
    /// the prior has a value for it.
    pub fn cell<'a>(&self, column: &'a PriorColumn, field: &str) -> Option<&'a str> {
        let idx = self.fields.iter().position(|f| f == field)?;
        column.cells.get(idx)?.as_deref()
    }

    /// The summary cell for `field`, if the summary column is on and `field`
    /// is a number row with values.
    pub fn summary_cell(&self, field: &str) -> Option<&str> {
        let idx = self.fields.iter().position(|f| f == field)?;
        self.summary.as_ref()?.cells.get(idx)?.as_deref()
    }
}

/// Resolve the panel: hard-filter `corpus`, score, order, take `limit`.
///
/// `form_values` is the form's current value for each field (keyed by field
/// name). Empty values never filter and never score.
pub fn resolve(
    plan: &PriorsPlan,
    corpus: &[Capture],
    form_values: &HashMap<String, String>,
) -> ResolvedPanel {
    let fields: Vec<String> = plan.show.iter().map(|c| c.field.clone()).collect();

    // 1. Hard filter (§4.1): a filled gate must match exactly. A prior that
    //    lacks the gate key is dropped too: nothing shows it describes this
    //    form's shape.
    let filled_gates = filled(&plan.gates, form_values);
    let survivors = corpus.iter().filter(|cap| {
        filled_gates
            .iter()
            .all(|(key, target)| key_agrees(cap, key, target))
    });

    // 2. Score: one point per agreeing `match_on` field the form has filled.
    //    The agreement flags, in `match_on` order, break equal counts.
    let filled_keys = filled(&plan.match_keys, form_values);
    let mut scored: Vec<Scored> = survivors
        .enumerate()
        .map(|(scan_order, cap)| {
            let agrees: Vec<bool> = plan
                .match_keys
                .iter()
                .map(|key| {
                    filled_keys
                        .iter()
                        .find(|(k, _)| k.field == key.field)
                        .is_some_and(|(k, target)| key_agrees(cap, k, target))
                })
                .collect();
            Scored {
                cap,
                score: agrees.iter().filter(|a| **a).count(),
                agrees,
                scan_order,
            }
        })
        .collect();

    // 3. Order (§5): score, then earlier-field agreement, then `rank_by`, then
    //    recency. `sort_by` is stable, so `rank_by = "none"` keeps scan order.
    scored.sort_by(|a, b| compare(a, b, &plan.rank_by));
    scored.truncate(plan.limit);

    let rank_field = match &plan.rank_by {
        RankBy::Field { field, .. } => Some(field.as_str()),
        RankBy::Recent | RankBy::None => None,
    };

    let columns: Vec<PriorColumn> = scored
        .iter()
        .map(|s| PriorColumn {
            cells: plan
                .show
                .iter()
                .map(|col| render_cell(s.cap, col))
                .collect(),
            rank_value: rank_field.and_then(|f| s.cap.scalar(f)).map(display_value),
            dimmed: rank_field.is_some_and(|f| s.cap.number(f).is_none()),
            score: s.score,
            recency: s.cap.recency,
            path: s.cap.path.clone(),
        })
        .collect();

    let summary = if plan.summary && !scored.is_empty() {
        let caps: Vec<&Capture> = scored.iter().map(|s| s.cap).collect();
        Some(summary_column(&plan.show, &caps))
    } else {
        None
    };

    ResolvedPanel {
        fields,
        no_close_match: !filled_keys.is_empty()
            && !columns.is_empty()
            && columns.iter().all(|c| c.score == 0),
        columns,
        rank_label: rank_label(&plan.rank_by),
        summary,
    }
}

/// Whether a prior's cell shows the same value as the form's current value,
/// so the panel renders it as `·` (§8.1).
///
/// Both sides are trimmed and wikilink-stripped. Two values that both parse
/// as numbers compare numerically, so a stored `18.0` equals a typed `18`. An
/// empty form value never matches.
pub fn same_as_current(cell: &str, current: &str) -> bool {
    let cell = strip_wikilink(cell.trim());
    let current = strip_wikilink(current.trim());
    if current.is_empty() {
        return false;
    }
    match (cell.parse::<f64>(), current.parse::<f64>()) {
        (Ok(a), Ok(b)) => a == b,
        _ => cell == current,
    }
}

/// A prior's age for the column footer, from its recency key and `now` (both
/// ms since the Unix epoch): `5m`, `3h`, `3d`, `1w`, `2mo`, `1y`. A recency in
/// the future reads `0m`.
pub fn age_label(recency_ms: i64, now_ms: i64) -> String {
    let minutes = (now_ms.saturating_sub(recency_ms) / 60_000).max(0);
    let hours = minutes / 60;
    let days = hours / 24;
    if minutes < 60 {
        format!("{minutes}m")
    } else if hours < 24 {
        format!("{hours}h")
    } else if days < 7 {
        format!("{days}d")
    } else if days < 30 {
        format!("{}w", days / 7)
    } else if days < 365 {
        format!("{}mo", days / 30)
    } else {
        format!("{}y", days / 365)
    }
}

/// A surviving prior with its score, ready to sort.
struct Scored<'a> {
    cap: &'a Capture,
    score: usize,
    /// Per `match_on` key, in plan order: does this prior agree?
    agrees: Vec<bool>,
    /// Position in the corpus scan, for `rank_by = "none"`.
    scan_order: usize,
}

/// The keys the form has a non-empty value for, paired with that value.
fn filled<'k, 'v>(
    keys: &'k [MatchKey],
    form_values: &'v HashMap<String, String>,
) -> Vec<(&'k MatchKey, &'v str)> {
    keys.iter()
        .filter_map(|k| {
            let v = form_values.get(&k.field)?.trim();
            (!v.is_empty()).then_some((k, v))
        })
        .collect()
}

/// Whether a prior agrees with the form's (non-empty) value on one key.
fn key_agrees(cap: &Capture, key: &MatchKey, target: &str) -> bool {
    let Some(stored) = cap.scalar(&key.field) else {
        return false;
    };
    match key.mode {
        MatchMode::Equality => stored.trim() == target,
        // Strip both sides so `[[Onyx]]` (stored) matches `Onyx` (form), and an
        // aliased/fragmented stored link matches the bare target.
        MatchMode::Wikilink => strip_wikilink(stored) == strip_wikilink(target),
    }
}

/// Column order (§5): higher score first; on equal score, the prior agreeing on
/// the earlier `match_on` field; then `rank_by`; then newest first.
fn compare(a: &Scored, b: &Scored, rank_by: &RankBy) -> Ordering {
    b.score
        .cmp(&a.score)
        // `true > false`, so comparing b to a puts earlier agreement first.
        .then_with(|| b.agrees.cmp(&a.agrees))
        .then_with(|| match rank_by {
            RankBy::Field { field, descending } => {
                match (a.cap.number(field), b.cap.number(field)) {
                    (Some(av), Some(bv)) => {
                        let ord = av.partial_cmp(&bv).unwrap_or(Ordering::Equal);
                        if *descending { ord.reverse() } else { ord }
                    }
                    // A prior missing the field sorts after one that has it.
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => Ordering::Equal,
                }
            }
            RankBy::Recent | RankBy::None => Ordering::Equal,
        })
        .then_with(|| match rank_by {
            RankBy::None => a.scan_order.cmp(&b.scan_order),
            _ => b.cap.recency.cmp(&a.cap.recency),
        })
}

/// The header's `rank_by` label, when `rank_by` names a field.
fn rank_label(rank_by: &RankBy) -> Option<String> {
    match rank_by {
        RankBy::Field { field, descending } => {
            let dir = if *descending { "desc" } else { "asc" };
            Some(format!("{field} {dir}"))
        }
        RankBy::Recent | RankBy::None => None,
    }
}

/// A frontmatter scalar as the panel shows it: wikilink-stripped and trimmed.
fn display_value(s: &str) -> String {
    strip_wikilink(s.trim())
}

/// One prior's cell for a `show` field. `None` (blank) when the prior lacks
/// the field or holds an empty value.
fn render_cell(cap: &Capture, col: &ShowColumn) -> Option<String> {
    let text = match cap.frontmatter.get(&col.field)? {
        FrontmatterValue::Scalar(s) => display_value(s),
        FrontmatterValue::List(items) => items.join(", "),
    };
    (!text.is_empty()).then_some(text)
}

/// Build the summary column over the displayed priors (§6). Only number rows
/// get a cell; everything else stays blank (select `mode` summaries are L2).
fn summary_column(show: &[ShowColumn], source: &[&Capture]) -> SummaryColumn {
    let cells = show
        .iter()
        .map(|col| {
            if !col.numeric {
                return None;
            }
            let mut values: Vec<f64> = source.iter().filter_map(|c| c.number(&col.field)).collect();
            if values.is_empty() {
                return None;
            }
            let value = match col.agg {
                // `latest` needs recency ordering; gather (recency, value).
                Agg::Latest => source
                    .iter()
                    .filter_map(|c| c.number(&col.field).map(|v| (c.recency, v)))
                    .max_by_key(|(r, _)| *r)
                    .map(|(_, v)| v)?,
                agg => aggregate(&mut values, agg),
            };
            Some(format_number(value))
        })
        .collect();

    let mut aggs = show.iter().filter(|c| c.numeric).map(|c| c.agg);
    let label = match aggs.next() {
        Some(first) if aggs.all(|a| a == first) => agg_name(first).to_string(),
        _ => "summary".to_string(),
    };

    SummaryColumn { label, cells }
}

fn agg_name(agg: Agg) -> &'static str {
    match agg {
        Agg::Median => "median",
        Agg::Mean => "mean",
        Agg::Max => "max",
        Agg::Min => "min",
        Agg::Latest => "latest",
    }
}

/// Aggregate a numeric slice per the aggregation. `Latest` is handled by the
/// caller (needs recency); everything else is order-independent.
fn aggregate(values: &mut [f64], agg: Agg) -> f64 {
    match agg {
        Agg::Mean => values.iter().sum::<f64>() / values.len() as f64,
        Agg::Max => values.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        Agg::Min => values.iter().cloned().fold(f64::INFINITY, f64::min),
        // Median (and Latest, though Latest is handled upstream).
        Agg::Median | Agg::Latest => median(values),
    }
}

/// Median of a slice (sorts in place). Even-length → mean of the two middles.
fn median(values: &mut [f64]) -> f64 {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    let n = values.len();
    if n % 2 == 1 {
        values[n / 2]
    } else {
        (values[n / 2 - 1] + values[n / 2]) / 2.0
    }
}

/// Format an aggregated number for display: integers print without a decimal,
/// non-integers to at most 2 decimal places with trailing zeros trimmed.
fn format_number(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.2}");
        let s = s.trim_end_matches('0').trim_end_matches('.');
        s.to_string()
    }
}
