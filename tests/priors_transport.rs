//! Transport tests (`src/priors/{jsonlogic,search}.rs`):
//!
//! 1. The JsonLogic builder is injection-safe: no user-supplied value reaches
//!    the query as a structural string literal — values ride as JSON data.
//! 2. The FS-scan corpus fetch parses on-disk frontmatter into the same
//!    `Capture` shape the API path would (search-path / FS-fallback parity).

use std::collections::HashMap;
use std::path::PathBuf;

use pour::config::Config;
use pour::priors::jsonlogic::build_predicate;
use pour::priors::plan::{MatchKey, MatchMode, PriorsPlan};
use pour::priors::search::{build_tier_predicate, fetch_corpus};
use pour::transport::Transport;
use pour::transport::fs::FsWriter;

// ── JsonLogic builder: injection safety ───────────────────────────────────────

#[test]
fn builder_never_interpolates_user_value_into_query_structure() {
    // A hostile value packed with JSON/DQL metacharacters. If the builder ever
    // interpolated it into the query STRUCTURE, these characters would appear as
    // object keys or operators. They must appear ONLY as a JSON string leaf.
    let hostile = r#"Onyx"] } , {"delete":["everything"#;
    let key = MatchKey {
        field: "roaster".to_string(),
        mode: MatchMode::Equality,
    };
    let predicate = build_predicate(&[(&key, hostile)]);

    // Serialize and re-parse: the hostile value must be a single string node.
    let json = serde_json::to_value(&predicate).unwrap();
    let clause = &json["and"][0]["=="];
    // clause[0] is the frontmatter accessor, clause[1] is the value.
    assert_eq!(
        clause[0],
        serde_json::json!({ "var": "frontmatter.roaster" })
    );
    // The value node is EXACTLY the hostile string — not parsed as structure.
    assert_eq!(clause[1], serde_json::Value::String(hostile.to_string()));

    // And crucially, the metacharacters did not create extra structural keys.
    // The `and` array has exactly one clause; the clause has exactly one op.
    assert_eq!(json["and"].as_array().unwrap().len(), 1);
    assert!(json["and"][0].get("delete").is_none());
}

#[test]
fn builder_uses_frontmatter_accessor_shape() {
    // Architect decision #2: confirmed `{"var": "frontmatter.<key>"}` from the
    // Obsidian OpenAPI examples. Pin it so a regression is caught.
    let key = MatchKey {
        field: "method".to_string(),
        mode: MatchMode::Equality,
    };
    let predicate = build_predicate(&[(&key, "V60")]);
    let accessor = &predicate["and"][0]["=="][0];
    assert_eq!(accessor["var"], "frontmatter.method");
}

#[test]
fn wikilink_key_matches_both_bare_and_wrapped_forms() {
    let key = MatchKey {
        field: "roaster".to_string(),
        mode: MatchMode::Wikilink,
    };
    let predicate = build_predicate(&[(&key, "Onyx")]);
    // An `or` over bare "Onyx" and "[[Onyx]]".
    let or = predicate["and"][0]["or"].as_array().unwrap();
    assert_eq!(or.len(), 2);
    assert_eq!(or[0]["=="][1], "Onyx");
    assert_eq!(or[1]["=="][1], "[[Onyx]]");
}

#[test]
fn tier_predicate_none_when_a_key_lacks_value() {
    let module = coffee_module();
    let plan = PriorsPlan::build(&module);
    // Only roaster provided; the tail key `method` (tier_len 3) has no value.
    let mut mv = HashMap::new();
    mv.insert("bean".to_string(), "New".to_string());
    mv.insert("roaster".to_string(), "Onyx".to_string());
    // Full tier (bean+roaster+method) is not queryable → None.
    assert!(build_tier_predicate(&plan, 3, &mv).is_none());
    // But the two-key tier (bean+roaster) is queryable.
    assert!(build_tier_predicate(&plan, 2, &mv).is_some());
}

// ── FS-scan corpus parity ─────────────────────────────────────────────────────

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
show = ["dose_g"]

[[modules.coffee.fields]]
name = "bean"
field_type = "text"
prompt = "Bean"

[[modules.coffee.fields]]
name = "roaster"
field_type = "text"
prompt = "Roaster"
wikilink = true

[[modules.coffee.fields]]
name = "method"
field_type = "static_select"
prompt = "Method"
options = ["V60"]

[[modules.coffee.fields]]
name = "dose_g"
field_type = "number"
prompt = "Dose"

[[modules.coffee.fields]]
name = "rating"
field_type = "number"
prompt = "Rating"
"#;
    Config::from_toml(toml)
        .unwrap()
        .modules
        .get("coffee")
        .unwrap()
        .clone()
}

#[tokio::test]
async fn fs_scan_reads_corpus_frontmatter() {
    let dir = tempfile::tempdir().unwrap();
    let coffee_dir = dir.path().join("Coffee");
    std::fs::create_dir_all(&coffee_dir).unwrap();

    std::fs::write(
        coffee_dir.join("brew1.md"),
        "---\nroaster: \"[[Onyx]]\"\nmethod: V60\ndose_g: 15\nrating: 5\n---\nnotes\n",
    )
    .unwrap();
    std::fs::write(
        coffee_dir.join("brew2.md"),
        "---\nroaster: \"[[Onyx]]\"\nmethod: V60\ndose_g: 16\nrating: 4\n---\nnotes\n",
    )
    .unwrap();

    let transport = Transport::Fs(FsWriter::new(PathBuf::from(dir.path())));
    let corpus = fetch_corpus(&transport, "Coffee").await;

    assert_eq!(corpus.len(), 2);
    // Frontmatter parsed into the shared Capture shape.
    let doses: Vec<&str> = corpus
        .iter()
        .filter_map(|c| c.frontmatter.get("dose_g").and_then(|v| v.as_scalar()))
        .collect();
    assert!(doses.contains(&"15"));
    assert!(doses.contains(&"16"));
}

#[tokio::test]
async fn fs_scan_feeds_resolver_end_to_end() {
    use pour::priors::resolve;

    let dir = tempfile::tempdir().unwrap();
    let coffee_dir = dir.path().join("Coffee");
    std::fs::create_dir_all(&coffee_dir).unwrap();

    // Stored roaster is a wikilink; the form value is the bare target.
    std::fs::write(
        coffee_dir.join("brew1.md"),
        "---\nbean: Old\nroaster: \"[[Onyx]]\"\nmethod: V60\ndose_g: 15\nrating: 5\n---\n",
    )
    .unwrap();
    std::fs::write(
        coffee_dir.join("brew2.md"),
        "---\nbean: Old\nroaster: \"[[Onyx]]\"\nmethod: V60\ndose_g: 17\nrating: 4\n---\n",
    )
    .unwrap();

    let transport = Transport::Fs(FsWriter::new(PathBuf::from(dir.path())));
    let corpus = fetch_corpus(&transport, "Coffee").await;

    let module = coffee_module();
    let plan = PriorsPlan::build(&module);

    // New bag → cascade widens to roaster+method; wikilink match compares
    // the bare "Onyx" against the stored "[[Onyx]]".
    let mut mv = HashMap::new();
    mv.insert("bean".to_string(), "New".to_string());
    mv.insert("roaster".to_string(), "Onyx".to_string());
    mv.insert("method".to_string(), "V60".to_string());

    let panel = resolve(&plan, &corpus, &mv).expect("roaster+method tier matches");
    assert_eq!(panel.tier_fields, vec!["roaster", "method"]);
    assert_eq!(panel.rows.len(), 2);
    // Median dose over qualifying {15, 17} = 16.
    let summary = panel.summary.unwrap();
    assert_eq!(summary.cells[0].1, "16");
}
