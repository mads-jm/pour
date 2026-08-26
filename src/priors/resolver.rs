//! The priors resolver: cascade match, qualifying-first ranking, numeric
//! summary. Pure over an in-memory corpus of captures — no transport, no
//! terminal — so it is fully unit-testable against fixtures.

use crate::data::frontmatter_read::{Frontmatter, FrontmatterValue};
use crate::data::wikilink::strip_wikilink;

use super::plan::{Agg, MatchKey, MatchMode, PriorsPlan, RankBy, ShowColumn};

/// A single prior capture: its parsed frontmatter plus a recency key.
///
/// `recency` is an opaque, monotonically-increasing ordering key (higher =
/// newer). On the API path it is derived from `stat.mtime`; on the FS path from
/// the file's modified time. The resolver only compares it, never interprets it.
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

/// A displayed row in the resolved panel.
#[derive(Debug, Clone)]
pub struct PanelRow {
    /// Rendered cell values, one per `show` column (in plan order).
    pub cells: Vec<String>,
    /// `true` when this row is texture (missing the `rank_by` field) and should
    /// render dimmed.
    pub is_texture: bool,
    pub path: String,
}

/// The per-field numeric summary line (§6), computed over qualifying rows.
#[derive(Debug, Clone)]
pub struct Summary {
    /// One `(field, formatted_value)` per summarizable (numeric) `show` column,
    /// in plan order.
    pub cells: Vec<(String, String)>,
    /// How many rows contributed to the summary.
    pub source_count: usize,
}

/// The resolved panel for a matched tier.
#[derive(Debug, Clone)]
pub struct ResolvedPanel {
    /// The tier label naming which match keys survived the cascade (e.g.
    /// `"roaster · method"`), or `None` when the module has no match keys.
    pub tier_fields: Vec<String>,
    /// The rank qualifier label, shown only when every displayed row qualifies
    /// (§5.4). `None` when texture is mixed in or nothing qualifies.
    pub rank_qualifier: Option<String>,
    pub rows: Vec<PanelRow>,
    /// `None` when no `show` column is summarizable.
    pub summary: Option<Summary>,
    /// Total captures in the corpus (for the `N of M` match-count line).
    pub corpus_size: usize,
    /// Column headers (the `show` field names), in plan order.
    pub columns: Vec<String>,
}

/// Resolve the panel: run the cascade over `corpus`, rank, and summarize.
///
/// `match_values` supplies the current form value for each `match_on` field
/// (keyed by field name). A key with no value (empty / absent) is treated as
/// unmatchable and effectively drops that tier's specificity — matching the
/// new-bag story where the most-specific key has no value yet.
///
/// Returns `None` when no tier matches (empty state) — the panel shows a
/// one-line empty message rather than an empty box.
pub fn resolve(
    plan: &PriorsPlan,
    corpus: &[Capture],
    match_values: &std::collections::HashMap<String, String>,
) -> Option<ResolvedPanel> {
    let columns: Vec<String> = plan.show.iter().map(|c| c.field.clone()).collect();

    // Cascade: try the full conjunction, then drop the tail key on zero rows.
    let (matched, tier_keys) = cascade(&plan.match_keys, corpus, match_values)?;

    // Rank + split.
    let (rows_ordered, all_qualify) = rank(&matched, &plan.rank_by, plan.limit);

    let panel_rows: Vec<PanelRow> = rows_ordered
        .iter()
        .map(|(cap, is_texture)| PanelRow {
            cells: plan.show.iter().map(|col| render_cell(cap, col)).collect(),
            is_texture: *is_texture,
            path: cap.path.clone(),
        })
        .collect();

    // Header qualifier: only when every displayed row qualifies (§5.4).
    let rank_qualifier = if all_qualify {
        rank_qualifier_label(&plan.rank_by)
    } else {
        None
    };

    // Summary over qualifying rows only; fall back to the displayed set when
    // there are no qualifying rows (§5).
    let summary_source: Vec<&Capture> = {
        let qualifying: Vec<&Capture> = rows_ordered
            .iter()
            .filter(|(_, texture)| !texture)
            .map(|(cap, _)| *cap)
            .collect();
        if qualifying.is_empty() {
            rows_ordered.iter().map(|(cap, _)| *cap).collect()
        } else {
            qualifying
        }
    };
    let summary = compute_summary(&plan.show, &summary_source);

    Some(ResolvedPanel {
        tier_fields: tier_keys,
        rank_qualifier,
        rows: panel_rows,
        summary,
        corpus_size: corpus.len(),
        columns,
    })
}

/// Run the widen-by-drop-tail cascade. Returns the matched captures and the
/// surviving tier's field names, or `None` if the list empties with no match.
fn cascade<'a>(
    match_keys: &[MatchKey],
    corpus: &'a [Capture],
    match_values: &std::collections::HashMap<String, String>,
) -> Option<(Vec<&'a Capture>, Vec<String>)> {
    // No match keys → recent-N of the same module (whole corpus qualifies).
    if match_keys.is_empty() {
        if corpus.is_empty() {
            return None;
        }
        return Some((corpus.iter().collect(), Vec::new()));
    }

    // `match_on` is ordered most-specific → least-specific. Widen by dropping
    // the *most-specific* (front) key: try the full conjunction, then the
    // suffix `[1..]`, `[2..]`, … until a tier matches or the list empties. This
    // is the new-bag cascade — the front key (`bean`) has no history precisely
    // when the panel is most wanted, so it is the first to drop (§4.1, story).
    for start in 0..match_keys.len() {
        let keys = &match_keys[start..];

        // A tier is only meaningful if every active key has a value to match on.
        let all_have_values = keys.iter().all(|k| {
            match_values
                .get(&k.field)
                .is_some_and(|v| !v.trim().is_empty())
        });

        if all_have_values {
            let matched: Vec<&Capture> = corpus
                .iter()
                .filter(|cap| keys.iter().all(|k| key_matches(cap, k, match_values)))
                .collect();

            if !matched.is_empty() {
                let tier_fields = keys.iter().map(|k| k.field.clone()).collect();
                return Some((matched, tier_fields));
            }
        }
    }

    None
}

/// Whether a capture matches a single key against the current form value.
fn key_matches(
    cap: &Capture,
    key: &MatchKey,
    match_values: &std::collections::HashMap<String, String>,
) -> bool {
    let target = match match_values.get(&key.field) {
        Some(v) if !v.trim().is_empty() => v.trim(),
        _ => return false,
    };

    let stored = match cap.scalar(&key.field) {
        Some(s) => s,
        None => return false,
    };

    match key.mode {
        MatchMode::Equality => stored == target,
        MatchMode::Wikilink => {
            // Strip both sides so `[[Onyx]]` (stored) matches `Onyx` (target),
            // and an aliased/fragmented stored link matches the bare target.
            strip_wikilink(stored) == strip_wikilink(target)
        }
    }
}

/// Rank matched captures into (capture, is_texture) rows, capped at `limit`.
///
/// Returns the ordered rows and whether *every displayed row* qualifies (drives
/// the header qualifier per §5.4).
fn rank<'a>(
    matched: &[&'a Capture],
    rank_by: &RankBy,
    limit: usize,
) -> (Vec<(&'a Capture, bool)>, bool) {
    match rank_by {
        RankBy::Recent => {
            let mut rows: Vec<&Capture> = matched.to_vec();
            rows.sort_by_key(|c| std::cmp::Reverse(c.recency));
            rows.truncate(limit);
            (rows.into_iter().map(|c| (c, false)).collect(), true)
        }
        RankBy::None => {
            let rows: Vec<&Capture> = matched.iter().copied().take(limit).collect();
            (rows.into_iter().map(|c| (c, false)).collect(), true)
        }
        RankBy::Field { field, descending } => {
            // Qualifying rows have a parseable numeric value for the field.
            let mut qualifying: Vec<&Capture> = matched
                .iter()
                .copied()
                .filter(|c| c.number(field).is_some())
                .collect();
            let mut texture: Vec<&Capture> = matched
                .iter()
                .copied()
                .filter(|c| c.number(field).is_none())
                .collect();

            qualifying.sort_by(|a, b| {
                let av = a.number(field).unwrap_or(f64::NAN);
                let bv = b.number(field).unwrap_or(f64::NAN);
                let ord = av.partial_cmp(&bv).unwrap_or(std::cmp::Ordering::Equal);
                if *descending { ord.reverse() } else { ord }
            });
            // Texture fills to limit by recency.
            texture.sort_by_key(|c| std::cmp::Reverse(c.recency));

            let mut rows: Vec<(&Capture, bool)> = qualifying.iter().map(|c| (*c, false)).collect();
            let remaining = limit.saturating_sub(rows.len());
            for c in texture.into_iter().take(remaining) {
                rows.push((c, true));
            }
            rows.truncate(limit);

            let all_qualify = rows.iter().all(|(_, texture)| !texture);
            (rows, all_qualify)
        }
    }
}

/// The header rank-qualifier label for a `rank_by`, shown only when every
/// displayed row qualifies. `recent`/`none` produce no "best"-style qualifier.
fn rank_qualifier_label(rank_by: &RankBy) -> Option<String> {
    match rank_by {
        RankBy::Field { field, descending } => {
            let dir = if *descending { "desc" } else { "asc" };
            Some(format!("{field} {dir}"))
        }
        RankBy::Recent | RankBy::None => None,
    }
}

/// Render a single cell for a show column.
fn render_cell(cap: &Capture, col: &ShowColumn) -> String {
    match cap.frontmatter.get(&col.field) {
        Some(FrontmatterValue::Scalar(s)) => strip_wikilink(s),
        Some(FrontmatterValue::List(items)) => items.join(", "),
        None => String::new(),
    }
}

/// Compute the numeric summary line over the source rows (§6). Returns `None`
/// when no `show` column is numeric (the caller falls back to a match-count).
fn compute_summary(show: &[ShowColumn], source: &[&Capture]) -> Option<Summary> {
    let mut cells = Vec::new();

    for col in show {
        if !col.numeric {
            continue;
        }
        let mut values: Vec<f64> = source.iter().filter_map(|c| c.number(&col.field)).collect();
        if values.is_empty() {
            continue;
        }
        // `latest` needs recency ordering; gather (recency, value) for it.
        let formatted = match col.agg {
            Agg::Latest => {
                let latest = source
                    .iter()
                    .filter_map(|c| c.number(&col.field).map(|v| (c.recency, v)))
                    .max_by_key(|(r, _)| *r)
                    .map(|(_, v)| v);
                latest.map(format_number)
            }
            _ => Some(format_number(aggregate(&mut values, col.agg))),
        };
        if let Some(f) = formatted {
            cells.push((col.field.clone(), f));
        }
    }

    if cells.is_empty() {
        None
    } else {
        Some(Summary {
            cells,
            source_count: source.len(),
        })
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
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
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
