//! Resolver tests (`src/priors/resolver.rs`): cascade widening, qualifying /
//! texture split, all-texture degeneration, and summary-source semantics.

use std::collections::BTreeMap;
use std::collections::HashMap;

use pour::config::Config;
use pour::data::frontmatter_read::{Frontmatter, FrontmatterValue};
use pour::priors::plan::PriorsPlan;
use pour::priors::resolver::{Capture, resolve};

/// Build a capture from `(field, value)` scalar pairs plus a recency key.
fn cap(recency: i64, pairs: &[(&str, &str)]) -> Capture {
    let mut fm: Frontmatter = BTreeMap::new();
    for (k, v) in pairs {
        fm.insert(k.to_string(), FrontmatterValue::Scalar(v.to_string()));
    }
    Capture {
        frontmatter: fm,
        recency,
        path: format!("Coffee/{recency}.md"),
    }
}

fn values(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/// A coffee module with `match_on = [bean, roaster, method]`, `rank_by =
/// rating desc`, `show = [dose_g, time_s]`.
fn coffee_module() -> pour::config::ModuleConfig {
    let toml = r#"
[vault]
base_path = "/tmp"

[modules.coffee]
mode = "create"
path = "Coffee/{date}.md"

[modules.coffee.priors]
match_on = ["bean", "roaster", "method"]
rank_by = "rating desc"
show = ["dose_g", "time_s"]
limit = 5

[[modules.coffee.fields]]
name = "bean"
field_type = "dynamic_select"
prompt = "Bean"
source = "Beans"

[[modules.coffee.fields]]
name = "roaster"
field_type = "text"
prompt = "Roaster"

[[modules.coffee.fields]]
name = "method"
field_type = "static_select"
prompt = "Method"
options = ["V60", "Aeropress"]

[[modules.coffee.fields]]
name = "dose_g"
field_type = "number"
prompt = "Dose"

[[modules.coffee.fields]]
name = "time_s"
field_type = "number"
prompt = "Time"

[[modules.coffee.fields]]
name = "rating"
field_type = "number"
prompt = "Rating"
"#;
    let config = Config::from_toml(toml).unwrap();
    config.modules.get("coffee").unwrap().clone()
}

#[test]
fn cascade_widens_by_dropping_tail_key() {
    let plan = PriorsPlan::build(&coffee_module());

    // No prior for this exact bean, but two priors for the roaster+method pair.
    let corpus = vec![
        cap(
            10,
            &[
                ("bean", "Old Bag"),
                ("roaster", "Onyx"),
                ("method", "V60"),
                ("dose_g", "15"),
                ("time_s", "180"),
                ("rating", "5"),
            ],
        ),
        cap(
            20,
            &[
                ("bean", "Another"),
                ("roaster", "Onyx"),
                ("method", "V60"),
                ("dose_g", "16"),
                ("time_s", "170"),
                ("rating", "4"),
            ],
        ),
    ];

    // The user picked a NEW bag → bean has no history; cascade must widen.
    let mv = values(&[("bean", "New Bag"), ("roaster", "Onyx"), ("method", "V60")]);
    let panel = resolve(&plan, &corpus, &mv).expect("a tier should match");

    // Widened past `bean` → tier is roaster+method.
    assert_eq!(panel.tier_fields, vec!["roaster", "method"]);
    assert_eq!(panel.rows.len(), 2);
}

#[test]
fn full_conjunction_matches_when_bean_has_history() {
    let plan = PriorsPlan::build(&coffee_module());
    let corpus = vec![cap(
        10,
        &[
            ("bean", "Known"),
            ("roaster", "Onyx"),
            ("method", "V60"),
            ("dose_g", "15"),
            ("time_s", "180"),
            ("rating", "5"),
        ],
    )];
    let mv = values(&[("bean", "Known"), ("roaster", "Onyx"), ("method", "V60")]);
    let panel = resolve(&plan, &corpus, &mv).expect("exact tier matches");
    assert_eq!(panel.tier_fields, vec!["bean", "roaster", "method"]);
}

#[test]
fn empty_state_when_nothing_matches() {
    let plan = PriorsPlan::build(&coffee_module());
    let corpus = vec![cap(
        10,
        &[
            ("bean", "X"),
            ("roaster", "Different"),
            ("method", "Aeropress"),
        ],
    )];
    // Only `method` V60 requested but no V60 exists → all tiers empty.
    let mv = values(&[("bean", "New"), ("roaster", "Onyx"), ("method", "V60")]);
    assert!(resolve(&plan, &corpus, &mv).is_none());
}

#[test]
fn qualifying_first_then_dim_texture() {
    let plan = PriorsPlan::build(&coffee_module());
    // Two rated (qualifying) + one unrated (texture) for roaster+method.
    let corpus = vec![
        cap(
            1,
            &[
                ("roaster", "Onyx"),
                ("method", "V60"),
                ("dose_g", "15"),
                ("rating", "4"),
            ],
        ),
        cap(
            2,
            &[
                ("roaster", "Onyx"),
                ("method", "V60"),
                ("dose_g", "16"),
                ("rating", "5"),
            ],
        ),
        cap(
            3,
            &[("roaster", "Onyx"), ("method", "V60"), ("dose_g", "14")],
        ), // no rating → texture
    ];
    // Provide empty bean so cascade drops to roaster+method.
    let mv = values(&[("bean", ""), ("roaster", "Onyx"), ("method", "V60")]);
    let panel = resolve(&plan, &corpus, &mv).unwrap();

    assert_eq!(panel.rows.len(), 3);
    // Rating desc: 5 first, then 4, both qualifying; texture (unrated) last.
    assert!(!panel.rows[0].is_texture);
    assert!(!panel.rows[1].is_texture);
    assert!(panel.rows[2].is_texture);

    // Header qualifier dropped because texture is mixed in (§5.4).
    assert!(panel.rank_qualifier.is_none());
}

#[test]
fn header_qualifier_shown_when_all_qualify() {
    let plan = PriorsPlan::build(&coffee_module());
    let corpus = vec![
        cap(
            1,
            &[
                ("roaster", "Onyx"),
                ("method", "V60"),
                ("dose_g", "15"),
                ("rating", "4"),
            ],
        ),
        cap(
            2,
            &[
                ("roaster", "Onyx"),
                ("method", "V60"),
                ("dose_g", "16"),
                ("rating", "5"),
            ],
        ),
    ];
    let mv = values(&[("bean", ""), ("roaster", "Onyx"), ("method", "V60")]);
    let panel = resolve(&plan, &corpus, &mv).unwrap();
    assert!(panel.rows.iter().all(|r| !r.is_texture));
    assert_eq!(panel.rank_qualifier.as_deref(), Some("rating desc"));
}

#[test]
fn all_texture_tier_degenerates_to_recency() {
    let plan = PriorsPlan::build(&coffee_module());
    // No brew in this tier is rated → all texture; recency order applies.
    let corpus = vec![
        cap(
            1,
            &[("roaster", "Onyx"), ("method", "V60"), ("dose_g", "15")],
        ),
        cap(
            3,
            &[("roaster", "Onyx"), ("method", "V60"), ("dose_g", "16")],
        ),
        cap(
            2,
            &[("roaster", "Onyx"), ("method", "V60"), ("dose_g", "14")],
        ),
    ];
    let mv = values(&[("bean", ""), ("roaster", "Onyx"), ("method", "V60")]);
    let panel = resolve(&plan, &corpus, &mv).unwrap();

    // All rows are texture → ranked by recency (newest first: 3, 2, 1).
    assert!(panel.rows.iter().all(|r| r.is_texture));
    assert_eq!(panel.rows[0].path, "Coffee/3.md");
    assert_eq!(panel.rows[1].path, "Coffee/2.md");
    assert_eq!(panel.rows[2].path, "Coffee/1.md");
    assert!(panel.rank_qualifier.is_none());
}

#[test]
fn summary_computed_over_qualifying_rows_only() {
    let plan = PriorsPlan::build(&coffee_module());
    // Two qualifying doses (15, 17 → median 16) and one texture dose (99) that
    // must NOT pollute the summary.
    let corpus = vec![
        cap(
            1,
            &[
                ("roaster", "Onyx"),
                ("method", "V60"),
                ("dose_g", "15"),
                ("rating", "4"),
            ],
        ),
        cap(
            2,
            &[
                ("roaster", "Onyx"),
                ("method", "V60"),
                ("dose_g", "17"),
                ("rating", "5"),
            ],
        ),
        cap(
            3,
            &[("roaster", "Onyx"), ("method", "V60"), ("dose_g", "99")],
        ), // texture
    ];
    let mv = values(&[("bean", ""), ("roaster", "Onyx"), ("method", "V60")]);
    let panel = resolve(&plan, &corpus, &mv).unwrap();

    let summary = panel.summary.expect("numeric summary present");
    // Median of qualifying doses {15, 17} = 16, NOT influenced by texture 99.
    let dose = summary
        .cells
        .iter()
        .find(|(f, _)| f == "dose_g")
        .map(|(_, v)| v.as_str());
    assert_eq!(dose, Some("16"));
    assert_eq!(summary.source_count, 2);
}

#[test]
fn zero_config_default_matches_first_select_field() {
    // No [priors] block → zero-config: match on first wikilink/select field.
    let toml = r#"
[vault]
base_path = "/tmp"

[modules.me]
mode = "create"
path = "Journal/{date}.md"

[[modules.me.fields]]
name = "mood"
field_type = "static_select"
prompt = "Mood"
options = ["good", "bad"]

[[modules.me.fields]]
name = "energy"
field_type = "number"
prompt = "Energy"
"#;
    let config = Config::from_toml(toml).unwrap();
    let module = config.modules.get("me").unwrap();
    let plan = PriorsPlan::build(module);

    assert_eq!(plan.match_keys.len(), 1);
    assert_eq!(plan.match_keys[0].field, "mood");

    let corpus = vec![
        cap(1, &[("mood", "good"), ("energy", "5")]),
        cap(2, &[("mood", "good"), ("energy", "7")]),
        cap(3, &[("mood", "bad"), ("energy", "2")]),
    ];
    let mv = values(&[("mood", "good")]);
    let panel = resolve(&plan, &corpus, &mv).unwrap();
    // Only the two `good` captures, recency-ordered (rank_by = recent default).
    assert_eq!(panel.rows.len(), 2);
    assert!(panel.rows.iter().all(|r| !r.is_texture));
}
