use std::process;

use pour::app::App;
use pour::config::Config;
use pour::data::cache::Cache;
use pour::data::field_presets::FieldPresets;
use pour::data::history::History;
use pour::data::presets::Presets;

#[tokio::main]
async fn main() {
    // Parse CLI args: `pour` = dashboard, `pour <module>` = fast path
    let args: Vec<String> = std::env::args().collect();

    // Handle `pour init` before config loading
    if args.get(1).map(|s| s.as_str()) == Some("init") {
        let force = args.iter().any(|a| a == "--force");
        let template = args
            .iter()
            .position(|a| a == "--template")
            .and_then(|i| args.get(i + 1))
            .map(std::path::PathBuf::from);

        if let Some(ref t) = template
            && !t.exists()
        {
            eprintln!("pour init: template not found: {}", t.display());
            process::exit(1);
        }

        match pour::init::run(pour::init::InitOptions { force, template }) {
            Ok(_) => process::exit(0),
            Err(e) => {
                eprintln!("pour init: {e}");
                process::exit(1);
            }
        }
    }

    // Handle `pour serve [--port <N>]` before config loading (mirrors `pour init` pattern)
    if args.get(1).map(|s| s.as_str()) == Some("serve") {
        // Strict flag parsing — reject unknown flags and malformed --port values.
        // Accepted: --port <N>  (where N is 1–65535)
        let mut port_raw: Option<&str> = None;
        let mut i = 2usize;
        while i < args.len() {
            match args[i].as_str() {
                "--port" => match args.get(i + 1) {
                    Some(v) if !v.starts_with('-') => {
                        port_raw = Some(v.as_str());
                        i += 2;
                    }
                    _ => {
                        eprintln!("pour serve: --port requires a value (e.g. --port 8421)");
                        process::exit(1);
                    }
                },
                flag if flag.starts_with('-') => {
                    eprintln!(
                        "pour serve: unknown flag '{flag}'\n\
                         accepted flags: --port <port>"
                    );
                    process::exit(1);
                }
                _ => {
                    i += 1;
                }
            }
        }

        // Default port 8421 — chosen to avoid common dev ports (3000/8080/8000/5000)
        // while being memorable as a Pour-specific port.
        let port: u16 = match port_raw {
            None => 8421,
            Some(s) => {
                let n: u32 = s.parse().unwrap_or_else(|_| {
                    eprintln!("pour serve: --port value must be a number (1–65535), got '{s}'");
                    process::exit(1);
                });
                if n == 0 {
                    eprintln!(
                        "pour serve: port 0 is not allowed (OS-assigned ports are surprising);\
                        \n             use an explicit port, e.g. --port 8421"
                    );
                    process::exit(1);
                }
                if n > 65535 {
                    eprintln!("pour serve: --port value must be 1–65535, got {n}");
                    process::exit(1);
                }
                n as u16
            }
        };

        let config = match pour::config::Config::load() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("pour serve: {e}");
                process::exit(1);
            }
        };

        let transport = pour::transport::Transport::connect(&config).await;

        // Resolve or generate the mobile auth token.
        // Precedence: POUR_MOBILE_TOKEN env var > secrets.toml > generate new.
        let token = pour::server::startup::resolve_token();

        // Print the styled startup banner (QR, URL, transport, footer hint).
        pour::server::startup::print_banner(&pour::server::startup::StartupContext {
            port,
            token: token.clone(),
            transport_mode: transport.mode(),
        });

        if let Err(e) = pour::server::run(config, transport, port, token).await {
            eprintln!("pour serve: {e}");
            process::exit(1);
        }
        process::exit(0);
    }

    let fast_path_module = args.get(1).cloned();

    // Load config — exit with user-friendly error on failure
    let config = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("pour: {e}");
            process::exit(1);
        }
    };

    // Connect transport (auto-fallback from API to filesystem)
    let transport = pour::transport::Transport::connect(&config).await;

    // One-shot argv capture: `pour <module> <field> [value]` writes and exits
    // without ever entering the TUI (spec §5). `pour <module>` alone parses to
    // `None` and falls through to the fast path below, unchanged.
    match pour::oneshot::parse(&args, &config) {
        Err(e) => {
            eprintln!("pour: {e}");
            process::exit(1);
        }
        Ok(Some(shot)) => match pour::oneshot::run(&config, &transport, &shot).await {
            Ok(line) => {
                println!("{line}");
                process::exit(0);
            }
            Err(e) => {
                eprintln!("pour: {e}");
                process::exit(1);
            }
        },
        Ok(None) => {}
    }

    // Load capture history for dashboard stats
    let history = History::load();

    // Load saved presets
    let presets = Presets::load();
    let field_presets = FieldPresets::load();

    // Build app state
    let mut app = App::new(config, transport, history, presets, field_presets);

    // Check for path issues at startup; shown as a dismissable overlay on the dashboard
    app.startup_warnings = app
        .config
        .check_paths(std::path::Path::new(app.config.vault.effective_base_path()));

    // Load cache for dynamic selects
    let mut cache = Cache::load();

    // Fast path: validate module name and jump directly to form
    if let Some(ref module_name) = fast_path_module {
        if !app.config.modules.contains_key(module_name) {
            eprintln!("pour: unknown module '{module_name}'");
            eprintln!("available modules: {}", app.module_keys.join(", "));
            process::exit(1);
        }

        // Set selected_module index to match the fast-path module
        if let Some(idx) = app.module_keys.iter().position(|k| k == module_name) {
            app.selected_module = idx;
        }

        app.form_state = app.init_form(module_name);
        app.screen = pour::app::Screen::Form;

        // Fetch dynamic select options for this module
        pour::tui::fetch_dynamic_options(&mut app, module_name, &mut cache).await;
        pour::tui::fetch_current_values(&mut app, module_name).await;
    }

    // Install panic hook that restores terminal before printing panic.
    // Known limitation: autocreate messages queued in App.deferred_stderr during
    // raw mode are not drained on panic (ownership of `app` prevents safe access
    // from the hook closure without a shared Mutex). Panic path prioritizes
    // terminal restoration over diagnostics surfacing.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        ratatui::restore();
        original_hook(info);
    }));

    // Initialize terminal
    let mut terminal = ratatui::init();

    // Main event loop
    let result = pour::tui::run_loop(&mut terminal, &mut app, &mut cache).await;

    // Restore terminal
    ratatui::restore();

    // Drain messages that were deferred during TUI raw mode (e.g. autocreate diagnostics)
    for msg in app.deferred_stderr.drain(..) {
        eprintln!("{msg}");
    }

    // Report any error from the main loop
    if let Err(e) = result {
        eprintln!("pour: {e}");
        process::exit(1);
    }
}
