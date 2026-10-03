//! Resolver tests (`src/priors/resolver.rs`) and plan derivation
//! (`src/priors/plan.rs`): the `show_when` hard filter, similarity scoring,
//! column ordering and its tie-breaks, the opt-in summary column, zero-config
//! defaults, the render-time helpers (`same_as_current`, `age_label`), and
//! the form-value helpers the TUI wraps around the resolver
//! (`visible_form_values`, `trigger_values`).

use std::collections::BTreeMap;
use std::collections::HashMap;

use pour::config::Config;
use pour::data::frontmatter_read::{Frontmatter, FrontmatterValue};
use pour::priors::plan::{DEFAULT_LIMIT, PriorsPlan};
use pour::priors::resolver::{Capture, age_label, resolve, same_as_current};
use pour::priors::{trigger_values, visible_form_values};

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

fn paths(panel: &pour::priors::ResolvedPanel) -> Vec<&str> {
    panel.columns.iter().map(|c| c.path.as_str()).collect()
}

/// A coffee-shaped module: `brew_method` gates `brewer` through `show_when`,
/// and the `[priors]` block is the §3 example. `extra_priors` lands inside the
/// `[priors]` block.
fn coffee_module_with(extra_priors: &str) -> pour::config::ModuleConfig {
    let toml = format!(
        r#"
[vault]
base_path = "/tmp"

[modules.coffee]
mode = "create"
path = "Coffee/{{date}}.md"

[modules.coffee.priors]
match_on = ["bean", "brewer", "grinder", "intent"]
rank_by = "rating desc"
{extra_priors}

[[modules.coffee.fields]]
name = "brew_method"
field_type = "static_select"
prompt = "Brew method"
options = ["Pour Over", "Espresso"]

[[modules.coffee.fields]]
name = "intent"
field_type = "static_select"
prompt = "Intent"
options = ["Bright", "Balanced"]

[[modules.coffee.fields]]
name = "brewer"
field_type = "dynamic_select"
prompt = "Brewer"
source = "Brewers"
wikilink = true
show_when = {{ field = "brew_method", equals = "Pour Over" }}

[[modules.coffee.fields]]
name = "bean"
field_type = "dynamic_select"
prompt = "Bean"
source = "Beans"
wikilink = true

[[modules.coffee.fields]]
name = "grinder"
field_type = "dynamic_select"
prompt = "Grinder"
source = "Grinders"
wikilink = true

[[modules.coffee.fields]]
name = "dose_g"
field_type = "number"
prompt = "Dose"

[[modules.coffee.fields]]
name = "rating"
field_type = "number"
prompt = "Rating"
"#
    );
    let config = Config::from_toml(&toml).unwrap();
    config.modules.get("coffee").unwrap().clone()
}

fn coffee_module() -> pour::config::ModuleConfig {
    coffee_module_with("")
}

// ── Similarity scoring ──

#[test]
fn new_bag_ranks_brewer_and_grinder_agreement_first() {
    let plan = PriorsPlan::build(&coffee_module());
    let corpus = vec![
        // Brewer + grinder agree (score 2), older.
        cap(
            1,
            &[
                ("brew_method", "Pour Over"),
                ("bean", "[[Old Bag]]"),
                ("brewer", "[[V60]]"),
                ("grinder", "[[K-Ultra]]"),
                ("intent", "Bright"),
            ],
        ),
        // Grinder only (score 1).
        cap(
            5,
            &[
                ("brew_method", "Pour Over"),
                ("bean", "[[Other]]"),
                ("brewer", "[[Switch]]"),
                ("grinder", "[[K-Ultra]]"),
            ],
        ),
        // Brewer only (score 1), newest of all.
        cap(
            9,
            &[
                ("brew_method", "Pour Over"),
                ("bean", "[[Other]]"),
                ("brewer", "[[V60]]"),
                ("grinder", "[[Comandante]]"),
            ],
        ),
        // Brewer + grinder agree (score 2), newer than the first.
        cap(
            2,
            &[
                ("brew_method", "Pour Over"),
                ("bean", "[[Old Bag]]"),
                ("brewer", "[[V60]]"),
                ("grinder", "[[K-Ultra]]"),
            ],
        ),
    ];

    // A bean with no history: nothing agrees on `bean`, and no widening step
    // is needed for brewer + grinder matches to rise.
    let mv = values(&[
        ("brew_method", "Pour Over"),
        ("bean", "New Bag"),
        ("brewer", "V60"),
        ("grinder", "K-Ultra"),
    ]);
    let panel = resolve(&plan, &corpus, &mv);

    assert_eq!(panel.columns.len(), 3, "limit defaults to 3");
    // Score 2 first (newest of the two, both unrated), then the brewer-only
    // prior beats the grinder-only one because brewer comes earlier in
    // match_on.
    assert_eq!(
        paths(&panel),
        vec!["Coffee/2.md", "Coffee/1.md", "Coffee/9.md"]
    );
    assert_eq!(panel.columns[0].score, 2);
    assert_eq!(panel.columns[2].score, 1);
    assert!(!panel.no_close_match);
    assert_eq!(panel.header(), "similar · rating desc");
}

#[test]
fn every_prior_scoring_zero_still_shows_the_top_limit_as_no_close_match() {
    let plan = PriorsPlan::build(&coffee_module());
    let corpus = vec![
        cap(1, &[("bean", "[[A]]"), ("brewer", "[[Switch]]")]),
        cap(2, &[("bean", "[[B]]"), ("brewer", "[[Switch]]")]),
        cap(3, &[("bean", "[[C]]"), ("brewer", "[[Switch]]")]),
        cap(4, &[("bean", "[[D]]"), ("brewer", "[[Switch]]")]),
    ];
    let mv = values(&[("bean", "New"), ("brewer", "V60")]);
    let panel = resolve(&plan, &corpus, &mv);

    // No minimum score: the closest priors still show, newest first.
    assert_eq!(
        paths(&panel),
        vec!["Coffee/4.md", "Coffee/3.md", "Coffee/2.md"]
    );
    assert!(panel.columns.iter().all(|c| c.score == 0));
    assert!(panel.no_close_match);
    assert_eq!(panel.header(), "no close match · rating desc");
}

#[test]
fn empty_form_fields_never_score() {
    let plan = PriorsPlan::build(&coffee_module());
    let corpus = vec![cap(1, &[("bean", "[[A]]"), ("intent", "")])];
    // Empty `intent` on the form must not "agree" with an empty stored intent.
    let mv = values(&[("bean", ""), ("intent", "")]);
    let panel = resolve(&plan, &corpus, &mv);
    assert_eq!(panel.columns[0].score, 0);
}

// ── Hard filter (show_when gates) ──

#[test]
fn hard_filter_drops_priors_with_another_brew_method() {
    let plan = PriorsPlan::build(&coffee_module());
    let corpus = vec![
        // Agrees on bean and grinder but not the gate: still dropped.
        cap(
            9,
            &[
                ("brew_method", "Espresso"),
                ("bean", "[[Onyx]]"),
                ("grinder", "[[K-Ultra]]"),
            ],
        ),
        // No gate value at all: dropped, nothing shows it fits this form.
        cap(8, &[("bean", "[[Onyx]]"), ("grinder", "[[K-Ultra]]")]),
        cap(1, &[("brew_method", "Pour Over"), ("bean", "[[Other]]")]),
    ];
    let mv = values(&[
        ("brew_method", "Pour Over"),
        ("bean", "Onyx"),
        ("grinder", "K-Ultra"),
    ]);
    let panel = resolve(&plan, &corpus, &mv);
    assert_eq!(paths(&panel), vec!["Coffee/1.md"]);
}

#[test]
fn an_empty_gate_filters_nothing() {
    let plan = PriorsPlan::build(&coffee_module());
    let corpus = vec![
        cap(1, &[("brew_method", "Pour Over")]),
        cap(2, &[("brew_method", "Espresso")]),
        cap(3, &[]),
    ];
    let panel = resolve(&plan, &corpus, &values(&[("brew_method", "")]));
    assert_eq!(panel.columns.len(), 3);
}

#[test]
fn no_survivor_is_the_empty_state() {
    let plan = PriorsPlan::build(&coffee_module());
    let corpus = vec![cap(1, &[("brew_method", "Espresso")])];
    let panel = resolve(&plan, &corpus, &values(&[("brew_method", "Pour Over")]));
    assert!(panel.is_empty());
    assert!(!panel.no_close_match, "an empty panel is not a weak match");
}

#[test]
fn gates_come_from_show_when_with_any_field_name() {
    // A non-coffee module: `movement` gates `bar` through `show_when`. Nothing
    // in src/priors/ knows either name.
    let toml = r#"
[vault]
base_path = "/tmp"

[modules.lift]
mode = "create"
path = "Lifts/{date}.md"

[[modules.lift.fields]]
name = "movement"
field_type = "static_select"
prompt = "Movement"
options = ["Squat", "Deadlift"]

[[modules.lift.fields]]
name = "bar"
field_type = "static_select"
prompt = "Bar"
options = ["Straight", "Safety"]
show_when = { field = "movement", equals = "Squat" }

[[modules.lift.fields]]
name = "weight_kg"
field_type = "number"
prompt = "Weight"
"#;
    let config = Config::from_toml(toml).unwrap();
    let plan = PriorsPlan::build(config.modules.get("lift").unwrap());

    let gates: Vec<&str> = plan.gates.iter().map(|g| g.field.as_str()).collect();
    assert_eq!(gates, vec!["movement"]);

    let corpus = vec![
        cap(1, &[("movement", "Squat"), ("weight_kg", "100")]),
        cap(2, &[("movement", "Deadlift"), ("weight_kg", "140")]),
        cap(3, &[("movement", "Squat"), ("weight_kg", "105")]),
    ];
    let panel = resolve(&plan, &corpus, &values(&[("movement", "Squat")]));
    assert_eq!(paths(&panel), vec!["Coffee/3.md", "Coffee/1.md"]);
}

#[test]
fn trigger_fields_include_a_gate_outside_match_on() {
    // match_on leaves out brew_method, but changing it must still re-resolve.
    let plan = PriorsPlan::build(&coffee_module());
    assert_eq!(
        plan.trigger_fields(),
        vec!["bean", "brewer", "grinder", "intent", "brew_method"]
    );
}

#[test]
fn trigger_values_change_on_a_gate_outside_match_on_but_not_on_dose() {
    let module = coffee_module();
    let before = values(&[("brew_method", "Pour Over"), ("dose_g", "18")]);

    let mut gate_changed = before.clone();
    gate_changed.insert("brew_method".to_string(), "Espresso".to_string());
    assert_ne!(
        trigger_values(&module, &before),
        trigger_values(&module, &gate_changed),
        "a gate change re-resolves even though brew_method is not in match_on"
    );

    let mut dose_changed = before.clone();
    dose_changed.insert("dose_g".to_string(), "16".to_string());
    assert_eq!(
        trigger_values(&module, &before),
        trigger_values(&module, &dose_changed),
        "dose_g only moves the `·` marks, it never re-resolves"
    );
}

// ── Hidden fields (show_when) ──

#[test]
fn visible_form_values_drop_fields_show_when_hides() {
    let module = coffee_module();
    let mv = values(&[
        ("brew_method", "Espresso"),
        ("brewer", "V60"),
        ("bean", "Onyx"),
    ]);
    let visible = visible_form_values(&module, &mv);
    assert_eq!(
        visible.get("brew_method").map(String::as_str),
        Some("Espresso")
    );
    assert_eq!(visible.get("bean").map(String::as_str), Some("Onyx"));
    assert!(
        !visible.contains_key("brewer"),
        "brewer is hidden under Espresso: {visible:?}"
    );

    // Under Pour Over the same value is visible and kept.
    let mv = values(&[("brew_method", "Pour Over"), ("brewer", "V60")]);
    let visible = visible_form_values(&module, &mv);
    assert_eq!(visible.get("brewer").map(String::as_str), Some("V60"));
}

#[test]
fn a_stale_hidden_match_on_value_does_not_read_no_close_match() {
    // Pick Pour Over, pick brewer V60, switch to Espresso: `brewer` is hidden
    // but keeps its value. Espresso priors have no brewer, so if the stale
    // value were compared every prior would score zero against a filled
    // match_on field, and the header would say `no close match` though the
    // user has filled nothing visible that could match.
    let module = coffee_module();
    let plan = PriorsPlan::build(&module);
    let corpus = vec![
        cap(2, &[("brew_method", "Espresso"), ("rating", "4")]),
        cap(1, &[("brew_method", "Espresso"), ("rating", "5")]),
    ];
    let mv = values(&[("brew_method", "Espresso"), ("brewer", "V60")]);

    let stale = resolve(&plan, &corpus, &mv);
    assert!(
        stale.no_close_match,
        "precondition: the raw form values do trip `no close match`"
    );

    let panel = resolve(&plan, &corpus, &visible_form_values(&module, &mv));
    assert_eq!(panel.columns.len(), 2);
    assert!(!panel.no_close_match);
    assert_eq!(panel.header(), "similar · rating desc");
}

// ── Tie-breaks (§4.1, §5) ──

#[test]
fn equal_count_goes_to_the_earlier_match_on_field() {
    let plan = PriorsPlan::build(&coffee_module());
    let corpus = vec![
        // Agrees on intent (last key) only: newer and better rated.
        cap(9, &[("intent", "Bright"), ("rating", "5")]),
        // Agrees on bean (first key) only.
        cap(1, &[("bean", "[[Onyx]]"), ("rating", "1")]),
    ];
    let mv = values(&[("bean", "Onyx"), ("intent", "Bright")]);
    let panel = resolve(&plan, &corpus, &mv);
    assert_eq!(paths(&panel), vec!["Coffee/1.md", "Coffee/9.md"]);
}

#[test]
fn equal_score_is_broken_by_rank_by() {
    let plan = PriorsPlan::build(&coffee_module());
    let corpus = vec![
        cap(9, &[("bean", "[[Onyx]]"), ("rating", "3")]),
        cap(1, &[("bean", "[[Onyx]]"), ("rating", "4.5")]),
    ];
    let panel = resolve(&plan, &corpus, &values(&[("bean", "Onyx")]));
    assert_eq!(paths(&panel), vec!["Coffee/1.md", "Coffee/9.md"]);
    assert_eq!(panel.columns[0].rank_value.as_deref(), Some("4.5"));
}

#[test]
fn a_prior_missing_rank_by_sorts_after_and_is_dimmed() {
    let plan = PriorsPlan::build(&coffee_module());
    let corpus = vec![
        cap(9, &[("bean", "[[Onyx]]")]), // unrated, newest
        cap(1, &[("bean", "[[Onyx]]"), ("rating", "2")]),
    ];
    let panel = resolve(&plan, &corpus, &values(&[("bean", "Onyx")]));
    assert_eq!(paths(&panel), vec!["Coffee/1.md", "Coffee/9.md"]);
    assert!(!panel.columns[0].dimmed);
    assert!(panel.columns[1].dimmed);
    assert_eq!(panel.columns[1].rank_value, None);
}

#[test]
fn equal_rank_by_is_broken_by_recency() {
    let plan = PriorsPlan::build(&coffee_module());
    let corpus = vec![
        cap(1, &[("bean", "[[Onyx]]"), ("rating", "4")]),
        cap(5, &[("bean", "[[Onyx]]"), ("rating", "4")]),
        cap(3, &[("bean", "[[Onyx]]"), ("rating", "4")]),
    ];
    let panel = resolve(&plan, &corpus, &values(&[("bean", "Onyx")]));
    assert_eq!(
        paths(&panel),
        vec!["Coffee/5.md", "Coffee/3.md", "Coffee/1.md"]
    );
}

#[test]
fn rank_by_recent_has_no_header_label_and_dims_nothing() {
    // No [priors] block → rank_by = recent.
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
"#;
    let config = Config::from_toml(toml).unwrap();
    let plan = PriorsPlan::build(config.modules.get("me").unwrap());
    let corpus = vec![cap(1, &[]), cap(2, &[("mood", "good")])];
    let panel = resolve(&plan, &corpus, &values(&[("mood", "good")]));
    assert_eq!(panel.header(), "similar");
    assert!(panel.columns.iter().all(|c| !c.dimmed));
}

// ── Cells ──

#[test]
fn cells_hold_raw_values_and_none_for_missing_fields() {
    let plan = PriorsPlan::build(&coffee_module());
    let corpus = vec![cap(
        1,
        &[("bean", "[[Onyx|Onyx Geometry]]"), ("dose_g", "18")],
    )];
    let panel = resolve(&plan, &corpus, &values(&[]));
    let col = &panel.columns[0];
    assert_eq!(panel.cell(col, "bean"), Some("Onyx"), "wikilink stripped");
    assert_eq!(panel.cell(col, "dose_g"), Some("18"));
    assert_eq!(panel.cell(col, "grinder"), None, "missing → blank");
    assert_eq!(panel.cell(col, "no_such_field"), None);
}

// ── Summary column (§6) ──

#[test]
fn summary_is_off_by_default() {
    let plan = PriorsPlan::build(&coffee_module());
    let corpus = vec![cap(1, &[("dose_g", "18")])];
    let panel = resolve(&plan, &corpus, &values(&[]));
    assert!(panel.summary.is_none());
}

#[test]
fn summary_column_takes_the_median_of_displayed_priors_on_number_rows() {
    let plan = PriorsPlan::build(&coffee_module_with("summary = true"));
    let corpus = vec![
        cap(4, &[("bean", "[[A]]"), ("dose_g", "15")]),
        cap(3, &[("bean", "[[A]]"), ("dose_g", "16")]),
        cap(2, &[("bean", "[[A]]"), ("dose_g", "20")]),
        // Fourth prior falls outside limit 3 and must not count.
        cap(1, &[("bean", "[[A]]"), ("dose_g", "99")]),
    ];
    let panel = resolve(&plan, &corpus, &values(&[]));
    let summary = panel.summary.as_ref().expect("summary = true");
    assert_eq!(summary.label, "median");
    assert_eq!(panel.summary_cell("dose_g"), Some("16"));
    assert_eq!(panel.summary_cell("bean"), None, "selects stay blank (L2)");
    assert_eq!(panel.summary_cell("rating"), None, "no values → blank");
}

// ── Plan defaults ──

#[test]
fn zero_config_matches_every_select_and_wikilink_field_in_config_order() {
    let toml = r#"
[vault]
base_path = "/tmp"

[modules.me]
mode = "create"
path = "Journal/{date}.md"

[[modules.me.fields]]
name = "energy"
field_type = "number"
prompt = "Energy"

[[modules.me.fields]]
name = "mood"
field_type = "static_select"
prompt = "Mood"
options = ["good", "bad"]

[[modules.me.fields]]
name = "place"
field_type = "text"
prompt = "Place"
wikilink = true

[[modules.me.fields]]
name = "topic"
field_type = "dynamic_select"
prompt = "Topic"
source = "Topics"

[[modules.me.fields]]
name = "body"
field_type = "textarea"
prompt = "Body"

[[modules.me.fields]]
name = "sets"
field_type = "composite_array"
prompt = "Sets"

[[modules.me.fields.sub_fields]]
name = "reps"
field_type = "number"
prompt = "Reps"
"#;
    let config = Config::from_toml(toml).unwrap();
    let plan = PriorsPlan::build(config.modules.get("me").unwrap());

    let keys: Vec<&str> = plan.match_keys.iter().map(|k| k.field.as_str()).collect();
    assert_eq!(keys, vec!["mood", "place", "topic"]);
    let show: Vec<&str> = plan.show.iter().map(|c| c.field.as_str()).collect();
    assert_eq!(show, vec!["energy", "mood", "place", "topic"]);
    assert_eq!(plan.limit, 3);
    assert_eq!(DEFAULT_LIMIT, 3);
    assert!(!plan.summary);
}

#[test]
fn a_block_without_show_shows_every_field() {
    let plan = PriorsPlan::build(&coffee_module());
    let show: Vec<&str> = plan.show.iter().map(|c| c.field.as_str()).collect();
    assert_eq!(
        show,
        vec![
            "brew_method",
            "intent",
            "brewer",
            "bean",
            "grinder",
            "dose_g",
            "rating"
        ]
    );
}

// ── Render-time helpers ──

#[test]
fn same_as_current_compares_numbers_numerically_and_strips_wikilinks() {
    assert!(same_as_current("18", "18"));
    assert!(same_as_current("18.0", "18"));
    assert!(same_as_current("Onyx", "[[Onyx]]"));
    assert!(same_as_current(" V60 ", "V60"));
    assert!(!same_as_current("16", "18"));
    assert!(
        !same_as_current("18", ""),
        "an empty form value never matches"
    );
    assert!(!same_as_current("Pour over", "Pour Over"), "case-sensitive");
}

#[test]
fn age_label_buckets() {
    let minute = 60_000;
    let day = 24 * 60 * minute;
    let now = 1_000 * day;
    assert_eq!(age_label(now - 5 * minute, now), "5m");
    assert_eq!(age_label(now - 3 * 60 * minute, now), "3h");
    assert_eq!(age_label(now - 3 * day, now), "3d");
    assert_eq!(age_label(now - 8 * day, now), "1w");
    assert_eq!(age_label(now - 61 * day, now), "2mo");
    assert_eq!(age_label(now - 400 * day, now), "1y");
    assert_eq!(age_label(now + day, now), "0m", "future mtime clamps");
}

#[test]
fn nothing_to_compare_reads_similar_not_no_close_match() {
    // Form open with every match_on field empty: all priors score zero, but
    // nothing was compared, so the header must not claim a failed match.
    let plan = PriorsPlan::build(&coffee_module());
    let corpus = vec![cap(1, &[("bean", "[[A]]")]), cap(2, &[("bean", "[[B]]")])];
    let panel = resolve(&plan, &corpus, &values(&[("bean", ""), ("dose_g", "18")]));
    assert_eq!(panel.columns.len(), 2);
    assert!(!panel.no_close_match);
    assert_eq!(panel.header(), "similar · rating desc");
}
