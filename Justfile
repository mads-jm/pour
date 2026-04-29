set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

# Show available recipes
default:
    @just --list

# Init with default template
pour:
    cargo run -- init --force

# Init with mads vault config
mads:
    cargo run -- init --force --template resources/mads_config.toml

# Init with personal config
me:
    cargo run -- init --force --template resources/user_config.toml

# Run with default config (just dev coffee)
[windows]
dev *ARGS:
    $env:POUR_CONFIG = "resources/default_config.toml"; cargo run -- {{ARGS}}

[unix]
dev *ARGS:
    POUR_CONFIG=resources/default_config.toml cargo run -- {{ARGS}}

# Run with specific config file (just run resources/mads_config.toml coffee)
[windows]
run CONFIG *ARGS:
    $env:POUR_CONFIG = "{{CONFIG}}"; cargo run -- {{ARGS}}

[unix]
run CONFIG *ARGS:
    POUR_CONFIG={{CONFIG}} cargo run -- {{ARGS}}

# ─── Install + live-config symlink ──────────────────────────────────────────
# `just install` drops a fresh `pour` on $PATH so it works from any directory.
# `just link` symlinks ~/.pour/config.toml to a tracked file in resources/, so
# edits show up in `git diff` immediately — no `pour init` round-trip.
#
# Typical setup (one-time):
#   just install
#   just link                          # -> resources/mads_config.toml
#   just link resources/default_config.toml   # or pick a different source
#
# Re-run `just install` after code changes to refresh the binary on $PATH.
#
# CAUTION: while linked, `pour init` (and `just pour`/`just mads`/`just me`)
# writes THROUGH the symlink and clobbers the source file. `just unlink`
# first if you want to reset.

# Build and install pour to ~/.cargo/bin/pour (overwrites existing)
install:
    cargo install --path . --force

# Symlink ~/.pour/config.toml -> CONFIG (default: resources/mads_config.toml)
[windows]
link CONFIG="resources/mads_config.toml":
    if (-not (Test-Path "$env:USERPROFILE\.pour")) { New-Item -ItemType Directory -Path "$env:USERPROFILE\.pour" | Out-Null }
    if (Test-Path "$env:USERPROFILE\.pour\config.toml") { Remove-Item -Force "$env:USERPROFILE\.pour\config.toml" }
    New-Item -ItemType SymbolicLink -Path "$env:USERPROFILE\.pour\config.toml" -Target (Resolve-Path '{{CONFIG}}').Path | Out-Null
    Write-Host "Linked ~/.pour/config.toml -> {{CONFIG}}"

[unix]
link CONFIG="resources/mads_config.toml":
    mkdir -p "${POUR_HOME:-$HOME/.pour}"
    ln -sf "$(pwd)/{{CONFIG}}" "${POUR_HOME:-$HOME/.pour}/config.toml"
    echo "Linked ~/.pour/config.toml -> {{CONFIG}}"

# Remove ~/.pour/config.toml if (and only if) it is a symlink
[windows]
unlink:
    $p = "$env:USERPROFILE\.pour\config.toml"; if ((Get-Item $p -ErrorAction SilentlyContinue).LinkType -eq 'SymbolicLink') { Remove-Item -Force $p; Write-Host "Removed symlink $p" } else { Write-Host "No symlink at $p (skipped)" }

[unix]
unlink:
    if [ -L "${POUR_HOME:-$HOME/.pour}/config.toml" ]; then rm -f "${POUR_HOME:-$HOME/.pour}/config.toml" && echo "Removed symlink"; else echo "Not a symlink (skipped)"; fi
