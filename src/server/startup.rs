//! Startup helpers shared by `pour serve` (CLI) and the TUI serve handoff.
//!
//! Both paths call `resolve_token` to obtain or generate the mobile auth token,
//! then `print_banner` to display the QR code and connection info in the terminal.

use crate::transport::TransportMode;

/// Data required to print the serve banner.
pub struct StartupContext {
    pub port: u16,
    pub token: String,
    pub transport_mode: TransportMode,
}

/// Resolve or generate the mobile auth token.
///
/// Precedence: `POUR_MOBILE_TOKEN` env var → `secrets.toml` → generate new UUID.
/// A newly generated token is persisted to `secrets.toml`; errors are printed as
/// warnings but do not abort startup.
pub fn resolve_token() -> String {
    match crate::config::Config::read_mobile_token() {
        Some(t) => t,
        None => {
            let token = uuid::Uuid::new_v4().simple().to_string();
            if let Err(e) = crate::config::Config::write_mobile_token(&token) {
                eprintln!("pour serve: warning: could not persist mobile token: {e}");
            } else {
                eprintln!("pour serve: new mobile token generated and saved to secrets.toml");
            }
            token
        }
    }
}

/// Total visible character width of the banner box, including the two corner/border glyphs.
///
/// All horizontal rules are exactly `BANNER_WIDTH` chars; all content lines are also
/// exactly `BANNER_WIDTH` chars (│ + 68 inner chars + │). Unit-tested in
/// `tests/server_startup_banner.rs`.
pub const BANNER_WIDTH: usize = 70;

/// Inner content width for each banner line (BANNER_WIDTH minus two border chars).
const BANNER_INNER: usize = BANNER_WIDTH - 2;

/// Truncate `url` to at most `max_chars` visible characters, appending `…` if truncated.
///
/// Only ASCII characters are expected in URLs produced by this crate, so
/// `chars().count()` == byte length in practice. We use `chars()` anyway for
/// correctness — and critically, we do NOT use byte-length formatting (`{:<width$}`)
/// on strings that may have been truncated with a multi-byte ellipsis character.
fn truncate_url(url: &str, max_chars: usize) -> String {
    if url.chars().count() <= max_chars {
        url.to_string()
    } else {
        let truncated: String = url.chars().take(max_chars - 1).collect();
        format!("{truncated}\u{2026}") // U+2026 HORIZONTAL ELLIPSIS
    }
}

/// Build the styled banner box lines for a given URL, transport label, and port.
///
/// Returns one `String` per line. ANSI escape codes are stripped before computing
/// widths internally, but the returned strings contain the raw ANSI escapes for
/// terminal display. The unit test in `tests/server_startup_banner.rs` strips ANSI
/// codes itself before asserting widths.
///
/// This function is `pub` only so the banner-width regression test can call it
/// directly without spawning a full server.
pub fn build_banner_box(url: &str, transport_label: &str, port: u16) -> Vec<String> {
    let h_rule: String = "\u{2500}".repeat(BANNER_INNER);

    let url_field_width: usize = BANNER_INNER - 4; // 64
    let url_display = truncate_url(url, url_field_width);
    let url_pad = url_field_width.saturating_sub(url_display.chars().count());
    let url_line = format!(
        "\u{2502}    \x1b[33m{url_display}\x1b[0m{pad}\u{2502}",
        pad = " ".repeat(url_pad),
    );

    let transport_line = format!(
        "\u{2502}  Transport: {transport:<20}  Listening: 0.0.0.0:{port:<5}         \u{2502}",
        transport = transport_label,
        port = port,
    );

    vec![
        format!("\u{256d}{h_rule}\u{256e}"),
        format!(
            "\u{2502}  \x1b[1mPOUR SERVE\x1b[0m{pad}\u{2502}",
            pad = " ".repeat(BANNER_INNER - 12),
        ),
        format!("\u{2502}{pad}\u{2502}", pad = " ".repeat(BANNER_INNER)),
        format!(
            "\u{2502}  Scan the QR code or open:{pad}\u{2502}",
            pad = " ".repeat(BANNER_INNER - 27),
        ),
        url_line,
        format!("\u{2502}{pad}\u{2502}", pad = " ".repeat(BANNER_INNER)),
        transport_line,
        format!("\u{2502}{pad}\u{2502}", pad = " ".repeat(BANNER_INNER)),
        format!(
            "\u{2502}  \x1b[2mPress Ctrl+C to stop and return to the dashboard.\x1b[0m{pad}\u{2502}",
            pad = " ".repeat(BANNER_INNER - 51),
        ),
        format!("\u{2570}{h_rule}\u{256f}"),
    ]
}

/// Print the styled startup banner: QR code, URL, transport mode, footer hint.
///
/// On LAN-IP detection failure the QR is skipped and a warning box is printed
/// instead (matching the pre-extraction behaviour from `main.rs`).
///
/// Output goes to stdout/stderr exactly as before. Call this after
/// `ratatui::restore()` so output reaches the cooked terminal in the TUI path.
///
/// Every line of the styled box is exactly `BANNER_WIDTH` visible characters wide.
/// See `tests/server_startup_banner.rs` for regression coverage.
pub fn print_banner(ctx: &StartupContext) {
    let port = ctx.port;
    let token = &ctx.token;
    let transport_label = match ctx.transport_mode {
        TransportMode::Api => "API",
        TransportMode::FileSystem => "FileSystem",
    };

    match local_ip_address::local_ip() {
        Ok(ip) => {
            let lan_ip = ip.to_string();
            let url = format!("http://{lan_ip}:{port}/?token={token}");

            // QR code rendered with Dense1x2 (renders cleanly in most terminals).
            use qrcode::QrCode;
            use qrcode::render::unicode;
            match QrCode::new(url.as_bytes()) {
                Ok(code) => {
                    let image = code
                        .render::<unicode::Dense1x2>()
                        .dark_color(unicode::Dense1x2::Dark)
                        .light_color(unicode::Dense1x2::Light)
                        .build();
                    println!("\n{image}");
                }
                Err(e) => {
                    eprintln!("pour serve: warning: could not render QR code: {e}");
                }
            }

            // Print the styled banner box — exactly BANNER_WIDTH chars per line.
            for line in build_banner_box(&url, transport_label, port) {
                println!("{line}");
            }
            println!();
        }
        Err(e) => {
            // LAN IP detection failed — QR would encode 127.0.0.1, useless on a phone.
            eprintln!();
            eprintln!("┌─────────────────────────────────────────────────────────────────┐");
            eprintln!("│  WARNING: LAN IP could not be detected ({e})");
            eprintln!("│");
            eprintln!("│  The QR code has been skipped — it would encode 127.0.0.1,");
            eprintln!("│  which is unreachable from a phone on the same network.");
            eprintln!("│");
            eprintln!("│  Local testing URL (same machine only):");
            eprintln!("│    http://127.0.0.1:{port}/?token={token}");
            eprintln!("│");
            eprintln!("│  To fix: specify your LAN IP explicitly once --host is available.");
            eprintln!("│  (TODO: add --host flag to bind a known IP)");
            eprintln!("└─────────────────────────────────────────────────────────────────┘");
            eprintln!();
            println!("Transport: {transport_label}  |  Listening on 0.0.0.0:{port}");
            println!();
            println!("\x1b[2mPress Ctrl+C to stop and return to the dashboard.\x1b[0m");
            println!();
        }
    }
}
