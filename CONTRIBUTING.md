# Contributing to Pour

Pour is a terminal-native TUI capture tool written in Rust that writes structured data into Obsidian vaults. This guide covers how to contribute code, documentation, and configuration changes.

## Branching Strategy

Pour uses a three-tier branch model:

```
main          Stable releases only. Tagged with semver (v0.2.0, v0.3.0).
 ^            PRs from dev after release validation.
 |
dev           Integration branch. All feature work merges here first.
 ^            PRs from nightly/feature branches after review.
 |
nightly       Active development. Daily work, experiments, WIP.
              Feature branches fork from here.
```

### Rules

- **`main`** is always release-ready. Only `dev -> main` PRs are merged here, after all CI passes and the changelog is updated. Never force-push to `main`.
- **`dev`** is the integration target. Feature branches and `nightly` merge here via PR. Must pass `cargo test`, `cargo clippy`, and `cargo fmt -- --check` before merge.
- **`nightly`** is the daily working branch. Commits here can be rough. Rebase onto `dev` before opening a PR.
- **Feature branches** fork from `nightly` (or `dev` for isolated fixes). Name them descriptively: `fix/strftime-injection`, `feat/history-pruning`, `docs/keybinding-reference`.

### Release Flow

```
1. nightly -> dev       PR with squash or rebase. CI must pass.
2. dev -> main          PR with merge commit. Update CHANGELOG.md and Cargo.toml version.
3. Tag on main          git tag v0.X.0 && git push --tags
4. Rebase nightly       git checkout nightly && git rebase dev
```

## Commit Conventions

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>: <short description>

<optional body>
```

### Types

| Type       | When to use                                      |
|------------|--------------------------------------------------|
| `feat`     | New user-facing feature                          |
| `fix`      | Bug fix                                          |
| `refactor` | Code change that neither fixes a bug nor adds a feature |
| `docs`     | Documentation only                               |
| `test`     | Adding or updating tests                         |
| `chore`    | Build, CI, dependency updates                    |
| `style`    | Formatting, whitespace (no logic change)         |

### Guidelines

- Keep the subject line under 72 characters.
- Use imperative mood: "fix strftime injection" not "fixed" or "fixes".
- Reference issues where applicable: `fix: escape % in field values (#42)`.
- One logical change per commit. If a fix requires a refactor, split them.

## Code Standards

### Rust

- **Edition**: 2024 (set in `Cargo.toml`).
- **Formatting**: Run `cargo fmt` before committing. CI rejects unformatted code.
- **Linting**: Run `cargo clippy --all-targets --all-features -- -D warnings` and resolve every warning. CI runs exactly that on Linux and Windows. Treat clippy as authoritative.
- **File size**: CI fails any `src/**/*.rs` file over 800 lines unless it carries a `// LINTOK: oversized: <reason>` comment. Check locally with `bash scripts/check-file-size.sh`.
- **Error handling**: Use `anyhow::Result` for fallible operations. Never `unwrap()` or `expect()` in paths reachable from user input. Panics crash the TUI and corrupt the terminal.
- **Visibility**: Keep structs and functions `pub(crate)` unless they need to be public API. Don't expose internal transport or output details.
- **Unsafe**: Not used. Don't introduce it.

### Testing

Tests live in dedicated files under `tests/` mirroring the `src/` structure:

```
src/config.rs       ->  tests/config.rs
src/output/foo.rs   ->  tests/output/foo.rs
```

Do **not** use inline `#[cfg(test)]` blocks in `src/`.

- Use `tempfile` (dev-dependency) for any test that touches the filesystem.
- Use `POUR_CONFIG` env var to point tests at temporary config files.
- Set `POUR_HOME` to a temp dir in any test that touches state (presets, cache, history, secrets). Integration tests panic if they reach `pour_home()` with `POUR_HOME` unset, so a forgotten override can't write to your real `~/.pour/`. Tests that set env vars hold the file's `ENV_LOCK` mutex while they do.
- A test that only works on Unix (symlinks, `sh` hooks) needs its imports and helpers gated too, not just the `#[test]` functions. CI runs clippy with `-D warnings` on Windows, where an ungated helper is dead code and fails the build. When every test in a file is Unix-only, put `#![cfg(unix)]` at the top of the file, as `tests/config_symlink.rs` does.
- Test the boundary, not the internals: validate outputs for given inputs rather than asserting on intermediate state.

### Config-Driven Design

Pour has no hardcoded knowledge of specific modules. All modules, fields, paths, and templates are defined in the user's `config.toml`. When adding features:

- Don't add module-specific logic (no `if module == "coffee"` branches).
- New field types or config keys must be added to `src/config.rs` validation *and* documented in `pour - docs/02 references/field-types.md`.
- Config changes must be backwards-compatible within a minor version. New keys should have sensible defaults.

## Documentation

### Structure

Documentation lives in `pour - docs/`, an Obsidian vault with numbered folders:

```
00 index/         Index files linking into each section
01 concepts/      Explanatory notes (why things work the way they do)
02 references/    Field types, library APIs, config schema
03 guides/        How-to guides (config-to-vault, phone capture, adding field types, design language)
04 architecture/  System overview, ADRs
05 notes/         Working notes and research
06 reports/       Release reports and assessments (frozen)
07 stories/       Narrative and manifesto
08 specs/         Design specifications
09 milestones/    Release milestone summaries
10 PRs/           Plaintext copies of GitHub PRs
99 meta/          Obsidian templates
```

### Rules

1. **Update docs with every behavioral change.** If your PR changes config schema, field behavior, keybindings, or architecture, update the affected docs in the same PR. Don't defer to a follow-up.

2. **Frontmatter is mandatory.** Every markdown file in the vault must have YAML frontmatter with at least `tags` and `date created`:
   ```yaml
   ---
   tags:
     - reference
     - field-types
   date created: Thursday, April 2nd 2026
   ---
   ```

3. **Use wikilinks for internal references.** Link to other vault docs with `[[doc-name]]` syntax, not relative file paths. This keeps the Obsidian graph connected.

4. **Index files must stay current.** When adding a new doc, add a link in the corresponding `00 index/` file.

5. **Reports are frozen.** Files in `06 reports/` are historical records. Never modify them.

6. **Design spec deviations.** If the implementation intentionally diverges from `pour-design-spec.md`, annotate the deviation inline with `*[Deviation: description]*` rather than rewriting the spec.

7. **ADR format.** Architecture Decision Records in `04 architecture/adr/` follow the template: Status, Context, Decision, Consequences.

### Reference Docs to Keep in Sync

These docs are the most likely to drift and should be checked on every release:

| Document | What to check |
|----------|---------------|
| `02 references/field-types.md` | Config keys, field types, validation rules |
| `02 references/keyboard-shortcuts.md` | Keybindings match `src/tui/` |
| `03 guides/Guide-Config-to-Vault.md` | Config examples still load; error messages match `src/config.rs` |
| `04 architecture/System-Architecture-Overview.md` | Module map matches `src/` |
| `08 specs/pour-design-spec.md` | Deviations annotated inline |
| `09 milestones/` (`v0.2.0-Foundation.md`, `v1.0.0-Release.md`) | Known limitations, freeze gates |
| `README.md` | Config examples, tech stack, quick start |
| `CLAUDE.md` | Build commands, architecture summary, tech stack |

### HTML Site

The `docs/` directory (distinct from `pour - docs/`) hosts the GitHub Pages site at `pour.madigan.app`. When the Obsidian vault is updated, the HTML site should be regenerated to stay in sync. Stale HTML artifacts should be removed.

## Pull Request Process

1. **Branch from the right place.** Features from `nightly`, hotfixes from `dev`.
2. **One concern per PR.** Don't bundle unrelated changes.
3. **PR description must include:**
   - What changed and why
   - How to test it (manual steps or test commands)
   - Which docs were updated
4. **CI must pass.** The required check is `ci / all`, which needs `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings` and `cargo test --all-features --locked` (both on Linux and Windows), `cargo doc --no-deps --all-features` with warnings as errors, and the file-size budget. A macOS compile job also runs and reports a failure without blocking merge.
5. **Self-review before requesting review.** Read your own diff. Check for `unwrap()`, `eprintln!` in TUI paths, missing doc updates.

## Setting Up for Development

```bash
# Clone
git clone https://github.com/mads-jm/pour.git
cd pour

# Build
cargo build

# First run -- generates starter config
cargo run -- init

# Run dashboard
cargo run

# Run a module directly
cargo run -- coffee

# Test
cargo test

# Lint + format check (what CI runs)
cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --all -- --check
```

On Linux, building needs the ALSA headers and `pkg-config` (`libasound2-dev` on Debian and Ubuntu), because `cpal` links `libasound`. Windows needs nothing extra.

### Justfile Workflows

Install [`just`](https://github.com/casey/just) (`cargo install just`) and run `just` to list recipes. Two flows are supported.

**Init from a tracked template.** Copies a resources file into `~/.pour/`:

```bash
just pour    # init from resources/default_config.toml (sanitized contributor template)
just mads    # init from resources/mads_config.toml (a real-world working example)
```

**Live edits via symlink.** Skips the `pour init` round-trip, so edits to the source file show up in `git diff` directly:

```bash
just install   # cargo install --path . --force, puts `pour` on $PATH
just link      # ~/.pour/config.toml -> resources/mads_config.toml (default)
just unlink    # remove the symlink (no-op if not a symlink)
```

`just link <path>` accepts any tracked config as the target: `just link resources/default_config.toml` for the contributor template, or your own `resources/<name>_config.toml`.

`resources/mads_config.toml` is the maintainer's personal config, kept in version control as a real-world example. `resources/default_config.toml` is the sanitized template that ships with `pour init`.

**Caution:** while a symlink is in place, `pour init` (and `just pour` / `just mads`) writes *through* the link and overwrites the source file. Run `just unlink` first if you want to reset.

**Windows:** creating symlinks needs [Developer Mode](https://learn.microsoft.com/en-us/windows/apps/get-started/developer-mode-features-and-debugging) or an elevated terminal.

### Environment Variables

| Variable | Purpose |
|----------|---------|
| `POUR_HOME` | Override the state directory (default `~/.pour/`): config, secrets, presets, cache, history |
| `POUR_CONFIG` | Override config file path (useful for testing). `secrets.toml` moves with it |
| `POUR_API_KEY` | Obsidian REST API bearer token. Overrides `secrets.toml` and `config.toml` |
| `POUR_MOBILE_TOKEN` | Auth token for `pour serve`. Overrides `mobile_token` in `secrets.toml` |
| `POUR_LOG` | Log filter for `pour serve` (default `info`), e.g. `POUR_LOG=debug` |
| `POUR_VERSION` | Installer only: pin the release `install.sh` or `install.ps1` downloads |
| `POUR_INSTALL_DIR`, `POUR_BIN_DIR` | `install.sh` only: where the release unpacks (default `~/.local/share/pour`) and where the `pour` link goes (default `~/.local/bin`) |

## License

By contributing to Pour, you agree that your contributions will be licensed under the MIT License.
