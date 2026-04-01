use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::config::Config;

pub struct InitOptions {
    pub force: bool,
}

const DEFAULT_CONFIG_TEMPLATE: &str = r####"# Pour — config.toml
# https://github.com/mads-jm/pour
#
# This file defines your Obsidian vault connection and capture modules.
# Each [modules.<name>] block becomes a `pour <name>` command.
#
# Edit this file to add new modules or customize existing ones.

# ─── Vault Connection ───────────────────────────────────────────────
[vault]
# Absolute path to your Obsidian vault root directory
base_path = "{{vault_path}}"

# Obsidian Local REST API (optional — Pour falls back to direct filesystem writes)
# api_port = 27124
# api_key = "your-api-key"  # or set POUR_API_KEY env var

# ─── Modules ────────────────────────────────────────────────────────
# Each module defines a capture form. Two modes:
#   "append" — adds content under a heading in an existing note
#   "create" — generates a new note file per entry
#
# Path supports strftime tokens: %Y, %m, %d, %H, %M, %S
# Field types: text, textarea, number, static_select, dynamic_select

# ── Journal / Thought Capture ───────────────────────────────────────
# Usage: `pour me`
# Appends timestamped thoughts to your daily note.
[modules.me]
mode = "append"
path = "Journal/%Y/%Y-%m-%d.md"
append_under_header = "## Log"
append_template = "> [!note] {{time}}\n> {{body}}"
display_name = "Journal"

[[modules.me.fields]]
name = "body"
field_type = "textarea"
prompt = "What's on your mind?"
required = true
target = "body"

# ── Coffee Brew Log ─────────────────────────────────────────────────
# Usage: `pour coffee`
# Creates a new note for each brew with YAML frontmatter.
[modules.coffee]
mode = "create"
path = "Coffee/%Y/%Y-%m-%d-%H%M%S.md"
display_name = "Coffee"

[[modules.coffee.fields]]
name = "brew_method"
field_type = "static_select"
prompt = "Brew method"
required = true
options = ["V60", "AeroPress", "Espresso", "French Press"]
target = "frontmatter"

[[modules.coffee.fields]]
name = "bean"
field_type = "dynamic_select"
prompt = "Bean"
source = "Coffee/Beans"

[[modules.coffee.fields]]
name = "rating"
field_type = "number"
prompt = "Rating (1-5)"
default = "3"

[[modules.coffee.fields]]
name = "notes"
field_type = "textarea"
prompt = "Tasting notes"
target = "body"
"####;

/// Escape a string for use inside a TOML basic string (double-quoted).
/// Handles all characters that TOML requires escaping: \, ", \n, \t, \r,
/// and control characters (U+0000–U+001F, U+007F).
fn escape_toml_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 || c as u32 == 0x7F => {
                // Remaining control characters as Unicode escapes
                out.push_str(&format!("\\u{:04X}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

/// Generate a config TOML string for the given vault path.
/// All TOML basic-string special characters in the path are escaped.
pub fn generate_config(vault_path: &str) -> String {
    let escaped = escape_toml_string(vault_path);
    DEFAULT_CONFIG_TEMPLATE.replace("{{vault_path}}", &escaped)
}

/// Run the init flow: detect/prompt vault, write config, validate.
/// Returns the path of the written config file.
pub fn run(options: InitOptions) -> Result<PathBuf> {
    let target = resolve_init_target();

    if target.exists() && !options.force {
        println!(
            "pour init: config already exists at {}\n       pass --force to overwrite",
            target.display()
        );
        return Ok(target);
    }

    let vault_path = detect_or_prompt_vault()?;

    let config_content = generate_config(&vault_path);

    // Validate before touching disk — a broken config on disk blocks both
    // `pour` (parse error) and `pour init` (exists guard).
    Config::from_toml(&config_content)
        .map_err(|e| anyhow::anyhow!("generated config failed validation: {e}"))?;

    // Create parent directories if needed
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create config directory: {}", parent.display()))?;
    }

    std::fs::write(&target, &config_content)
        .with_context(|| format!("failed to write config to {}", target.display()))?;

    println!("pour: config written to {}", target.display());
    println!();
    println!("Next steps:");
    println!("  1. Open {} and set your vault path if needed", target.display());
    println!("  2. Run `pour me` to capture a journal entry");
    println!("  3. Run `pour coffee` to log a brew");
    println!("  4. Add your own modules to the config");

    Ok(target)
}

/// Return the expected config file path without checking existence.
/// Respects `POUR_CONFIG` env var, otherwise uses the platform config dir.
fn resolve_init_target() -> PathBuf {
    crate::config::Config::default_config_path()
}

/// Scan common locations and either auto-detect, present a numbered list,
/// or fall back to a freetext prompt. Uses plain stdin/stdout — no TUI.
fn detect_or_prompt_vault() -> Result<String> {
    let candidates = detect_obsidian_vaults();

    match candidates.len() {
        0 => {
            println!("No Obsidian vault detected automatically.");
            prompt_vault_path()
        }
        1 => {
            let path = candidates[0].to_string_lossy().into_owned();
            print!("Found vault at {}. Use this? [Y/n]: ", path);
            io::stdout().flush()?;

            let mut line = String::new();
            io::stdin().lock().read_line(&mut line)?;
            let answer = line.trim().to_lowercase();

            if answer.is_empty() || answer == "y" || answer == "yes" {
                Ok(path)
            } else {
                prompt_vault_path()
            }
        }
        _ => {
            println!("Multiple Obsidian vaults found:");
            for (i, p) in candidates.iter().enumerate() {
                println!("  [{}] {}", i + 1, p.display());
            }
            println!("  [{}] Enter a path manually", candidates.len() + 1);
            print!("Choice [1]: ");
            io::stdout().flush()?;

            let mut line = String::new();
            io::stdin().lock().read_line(&mut line)?;
            let trimmed = line.trim();

            if trimmed.is_empty() {
                return Ok(candidates[0].to_string_lossy().into_owned());
            }

            match trimmed.parse::<usize>() {
                Ok(n) if n >= 1 && n <= candidates.len() => {
                    Ok(candidates[n - 1].to_string_lossy().into_owned())
                }
                Ok(n) if n == candidates.len() + 1 => prompt_vault_path(),
                _ => {
                    eprintln!("Invalid choice, prompting for path.");
                    prompt_vault_path()
                }
            }
        }
    }
}

/// Prompt the user to type a vault path manually.
fn prompt_vault_path() -> Result<String> {
    print!("Vault path: ");
    io::stdout().flush()?;

    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    let path_str = line.trim().to_string();

    if path_str.is_empty() {
        anyhow::bail!("vault path cannot be empty");
    }

    if !Path::new(&path_str).exists() {
        eprintln!(
            "Warning: path '{}' does not exist. Proceeding anyway.",
            path_str
        );
    }

    Ok(path_str)
}

/// Scan common locations one level deep for directories containing `.obsidian/`.
fn detect_obsidian_vaults() -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

    let mut search_roots = vec![
        home.join("Documents"),
        home.clone(),
    ];

    // Windows: also check OneDrive Documents
    #[cfg(target_os = "windows")]
    search_roots.push(home.join("OneDrive").join("Documents"));

    let mut vaults = Vec::new();

    for root in &search_roots {
        let entries = match std::fs::read_dir(root) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join(".obsidian").is_dir() && !vaults.contains(&path) {
                vaults.push(path);
            }
        }
    }

    vaults
}
