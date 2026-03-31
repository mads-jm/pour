---
tags:
  - reference
  - rust
  - serialization
aliases:
  - toml-serde
  - toml
  - serde
date created: Tuesday, March 31st 2026, 12:14:44 am
date modified: Tuesday, March 31st 2026, 12:51:42 am
---

# TOML & Serde - Config Parsing Reference

> __Sources:__ <https://docs.rs/toml/latest/toml/>, <https://docs.rs/serde_yaml/latest/serde_yaml/>
> __Crates:__ `toml`, `serde`, `serde_json`, `serde_yaml`

## TOML Config Parsing

### Deserialize into Structs

```rust
use serde::Deserialize;

#[derive(Deserialize)]
struct Config {
    vault: VaultConfig,
    modules: std::collections::HashMap<String, ModuleConfig>,
}

#[derive(Deserialize)]
struct VaultConfig {
    base_path: String,
    api_port: u16,
}

#[derive(Deserialize)]
struct ModuleConfig {
    mode: String,          // "append" or "create"
    path: String,          // strftime-templated path
    append_under_header: Option<String>,
    fields: Vec<FieldConfig>,
}

#[derive(Deserialize)]
struct FieldConfig {
    name: String,
    #[serde(rename = "type")]
    field_type: String,    // "textarea", "dynamic_select", "number", etc.
    prompt: String,
    source: Option<String>,
}

// Parse
let config: Config = toml::from_str(&std::fs::read_to_string(path)?)?;
```

### Key Functions

| Function | Purpose |
|----------|---------|
| `toml::from_str::<T>(s)` | Deserialize TOML string into `T` |
| `toml::to_string(&val)` | Serialize to TOML string |
| `toml::to_string_pretty(&val)` | Serialize to pretty TOML |
| `toml::Value` | Dynamic TOML value type |

## Serde YAML (Frontmatter Generation)

```rust
use serde::Serialize;
use serde_yaml;

#[derive(Serialize)]
struct CoffeeFrontmatter {
    bean: String,
    yield_g: u32,
    date: String,
    tags: Vec<String>,
}

let fm = CoffeeFrontmatter {
    bean: "Ethiopian Yirgacheffe".into(),
    yield_g: 36,
    date: "2026-03-30".into(),
    tags: vec!["coffee".into(), "v60".into()],
};

let yaml = serde_yaml::to_string(&fm)?;
// Output:
// bean: Ethiopian Yirgacheffe
// yield_g: 36
// date: '2026-03-30'
// tags:
// - coffee
// - v60

// Wrap as frontmatter
let frontmatter = format!("---\n{}---\n", yaml);
```

### Key Functions

| Function | Purpose |
|----------|---------|
| `serde_yaml::to_string(&val)` | Serialize to YAML string |
| `serde_yaml::to_writer(w, &val)` | Serialize to writer |
| `serde_yaml::from_str::<T>(s)` | Deserialize YAML string |
| `serde_yaml::Value` | Dynamic YAML value type |

## Serde JSON (API communication)

```rust
use serde_json;

// Parse API response
let files: serde_json::Value = serde_json::from_str(&response_body)?;

// Build request body
let body = serde_json::to_string(&data)?;
```