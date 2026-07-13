//! Config-schema tests for the `[modules.<key>.priors]` block: deserialization
//! of the full grammar plus L1 validation (unknown refs, L2-only mode
//! rejection, rank_by grammar, agg vocabulary).

use pour::config::{Config, MatchOn, ShowField};

const BASE_FIELDS: &str = r#"
[[modules.coffee.fields]]
name = "bean"
field_type = "dynamic_select"
prompt = "Bean"
source = "Beans"

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

fn config_with_priors(priors_block: &str) -> Result<Config, pour::config::ConfigError> {
    let toml = format!(
        r#"
[vault]
base_path = "/tmp/vault"

[modules.coffee]
mode = "create"
path = "Coffee/{{date}}.md"

{priors_block}
{BASE_FIELDS}
"#
    );
    Config::from_toml(&toml)
}

#[test]
fn full_priors_block_deserializes() {
    let config = config_with_priors(
        r#"
[modules.coffee.priors]
match_on = ["bean", "roaster", "method"]
rank_by = "rating desc"
show = ["dose_g", { field = "rating", agg = "mean" }]
limit = 3
"#,
    )
    .expect("valid priors block");

    let priors = config
        .modules
        .get("coffee")
        .unwrap()
        .priors
        .as_ref()
        .unwrap();

    assert_eq!(priors.match_on.len(), 3);
    assert!(matches!(&priors.match_on[0], MatchOn::Field(f) if f == "bean"));
    assert_eq!(priors.rank_by.as_deref(), Some("rating desc"));
    assert_eq!(priors.limit, Some(3));
    // The object-form show entry preserves the agg override.
    assert!(matches!(
        &priors.show[1],
        ShowField::Object { field, agg } if field == "rating" && agg.as_deref() == Some("mean")
    ));
}

#[test]
fn object_form_match_on_deserializes() {
    let config = config_with_priors(
        r#"
[modules.coffee.priors]
match_on = [{ field = "roaster", mode = "wikilink" }]
"#,
    )
    .expect("object-form match_on parses");
    let priors = config
        .modules
        .get("coffee")
        .unwrap()
        .priors
        .as_ref()
        .unwrap();
    assert!(matches!(
        &priors.match_on[0],
        MatchOn::Object { field, mode, .. } if field == "roaster" && mode.as_deref() == Some("wikilink")
    ));
}

#[test]
fn empty_priors_block_is_valid() {
    // All keys optional.
    config_with_priors("[modules.coffee.priors]").expect("empty block valid");
}

#[test]
fn unknown_match_on_field_rejected() {
    let err = config_with_priors(
        r#"
[modules.coffee.priors]
match_on = ["nonexistent"]
"#,
    )
    .unwrap_err()
    .to_string();
    assert!(
        err.contains("match_on references unknown field"),
        "got: {err}"
    );
}

#[test]
fn l2_overlap_mode_rejected_in_l1() {
    let err = config_with_priors(
        r#"
[modules.coffee.priors]
match_on = [{ field = "roaster", mode = "overlap" }]
"#,
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("not supported in L1"), "got: {err}");
}

#[test]
fn l2_window_mode_rejected_in_l1() {
    let err = config_with_priors(
        r#"
[modules.coffee.priors]
match_on = [{ field = "rating", mode = "window", days = 90 }]
"#,
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("not supported in L1"), "got: {err}");
}

#[test]
fn rank_by_max_form_rejected_in_l1() {
    let err = config_with_priors(
        r#"
[modules.coffee.priors]
rank_by = "rating max"
"#,
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("not supported in L1"), "got: {err}");
}

#[test]
fn rank_by_unknown_field_rejected() {
    let err = config_with_priors(
        r#"
[modules.coffee.priors]
rank_by = "ghost desc"
"#,
    )
    .unwrap_err()
    .to_string();
    assert!(
        err.contains("rank_by references unknown field"),
        "got: {err}"
    );
}

#[test]
fn invalid_agg_rejected() {
    let err = config_with_priors(
        r#"
[modules.coffee.priors]
show = [{ field = "dose_g", agg = "bogus" }]
"#,
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("agg 'bogus'"), "got: {err}");
}

#[test]
fn zero_limit_rejected() {
    let err = config_with_priors(
        r#"
[modules.coffee.priors]
limit = 0
"#,
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("limit must be greater than 0"), "got: {err}");
}

#[test]
fn recent_and_none_rank_by_accepted() {
    config_with_priors(
        r#"
[modules.coffee.priors]
rank_by = "recent"
"#,
    )
    .expect("recent valid");
    config_with_priors(
        r#"
[modules.coffee.priors]
rank_by = "none"
"#,
    )
    .expect("none valid");
}
