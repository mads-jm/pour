set shell := ["bash", "-cu"]

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

# Run with default config (pass module as arg: just dev coffee)
dev *ARGS:
    POUR_CONFIG=resources/default_config.toml cargo run -- {{ARGS}}

# Run with specific config file (just run resources/mads_config.toml coffee)
run CONFIG *ARGS:
    POUR_CONFIG={{CONFIG}} cargo run -- {{ARGS}}
