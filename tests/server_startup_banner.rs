// Regression tests for the `print_banner` styled box in `src/server/startup.rs`.
//
// Every line of the banner box must be exactly BANNER_WIDTH (70) visible
// characters wide. ANSI escape codes are stripped before counting so that
// colour/bold sequences don't skew the measurement.

use pour::server::startup::{BANNER_WIDTH, build_banner_box};

/// Strip ANSI escape sequences (ESC + `[` + params + letter) from `s`.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Consume '[' and everything up to and including the terminating letter.
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['
                for ch in chars.by_ref() {
                    if ch.is_ascii_alphabetic() {
                        break; // terminating letter consumed
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Every banner line for a typical LAN URL must be exactly BANNER_WIDTH visible chars.
#[test]
fn banner_box_lines_are_correct_width() {
    let url = "http://192.168.1.42:8421/?token=abcdef1234567890";
    let lines = build_banner_box(url, "API", 8421);

    for (i, line) in lines.iter().enumerate() {
        let visible = strip_ansi(line);
        let width = visible.chars().count();
        assert_eq!(
            width, BANNER_WIDTH,
            "banner line {i} has {width} visible chars, expected {BANNER_WIDTH}:\n  raw:  {line:?}\n  vis:  {visible:?}"
        );
    }
}

/// A URL that exactly fills the URL field (64 chars) must not overflow.
#[test]
fn banner_box_url_at_max_width() {
    // 64-char URL: exactly fills the field, no truncation.
    let url = "http://".to_string() + &"x".repeat(64 - 7); // total 64 chars
    let lines = build_banner_box(&url, "FileSystem", 8421);

    for (i, line) in lines.iter().enumerate() {
        let visible = strip_ansi(line);
        let width = visible.chars().count();
        assert_eq!(
            width, BANNER_WIDTH,
            "banner line {i} (max-width URL): {width} != {BANNER_WIDTH}\n  vis: {visible:?}"
        );
    }
}

/// A URL longer than 64 chars must be truncated and the line still fits.
#[test]
fn banner_box_long_url_is_truncated() {
    let url = "http://192.168.100.200:8421/?token=".to_string() + &"a".repeat(80);
    let lines = build_banner_box(&url, "API", 8421);

    for (i, line) in lines.iter().enumerate() {
        let visible = strip_ansi(line);
        let width = visible.chars().count();
        assert_eq!(
            width, BANNER_WIDTH,
            "banner line {i} (long URL): {width} != {BANNER_WIDTH}\n  vis: {visible:?}"
        );
    }

    // Confirm the ellipsis appears in the URL line (index 4).
    let url_line = strip_ansi(&lines[4]);
    assert!(
        url_line.contains('\u{2026}'),
        "expected ellipsis in truncated URL line: {url_line:?}"
    );
}

/// FileSystem transport label (10 chars) must fit in the 20-char transport field.
#[test]
fn banner_box_filesystem_transport_fits() {
    let url = "http://10.0.0.1:8421/?token=abc";
    let lines = build_banner_box(url, "FileSystem", 9999);

    for (i, line) in lines.iter().enumerate() {
        let visible = strip_ansi(line);
        let width = visible.chars().count();
        assert_eq!(
            width, BANNER_WIDTH,
            "banner line {i} (FileSystem): {width} != {BANNER_WIDTH}\n  vis: {visible:?}"
        );
    }
}
