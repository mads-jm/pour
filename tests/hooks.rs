//! Tests for `post_write_shell` (`src/hooks.rs`).
//!
//! No `POUR_HOME` guard: hooks touch no Pour state — they run a command in a
//! tempdir. Nothing here reaches `paths::pour_home`.
//!
//! The token-*rejection* tests live in `tests/config.rs`, because rejection is
//! config-validation behavior; what is pinned here is the scanner those rules
//! are built on, plus execution semantics.

use chrono::{Local, TimeZone};
use pour::hooks::{ALLOWED_TOKENS, HookContext, render, unknown_tokens};
// `run` is only exercised by the unix-gated execution tests below.
#[cfg(unix)]
use pour::hooks::run;

/// Fixed local timestamp: 2026-07-16 14:32:55.
fn fixed_now() -> chrono::DateTime<Local> {
    Local.with_ymd_and_hms(2026, 7, 16, 14, 32, 55).unwrap()
}

fn ctx_for(base: &str, rel: &str, title: &str) -> HookContext {
    HookContext::new(base, rel, title, fixed_now())
}

// ── token scanning ───────────────────────────────────────────────────────────

#[test]
fn every_allowed_token_scans_clean() {
    let command = ALLOWED_TOKENS
        .iter()
        .map(|t| format!("{{{{{t}}}}}"))
        .collect::<Vec<_>>()
        .join(" ");

    assert!(unknown_tokens(&command).is_empty());
}

#[test]
fn a_command_with_no_tokens_scans_clean() {
    assert!(unknown_tokens("git push").is_empty());
}

#[test]
fn field_tokens_are_reported_as_unknown() {
    // The security-critical case: user text must never reach a shell string.
    assert_eq!(unknown_tokens("git commit -m '{{title}}'"), vec!["title"]);
}

#[test]
fn unknown_tokens_are_reported_once_each() {
    assert_eq!(
        unknown_tokens("{{title}} {{title}} {{body}}"),
        vec!["title", "body"]
    );
}

#[test]
fn padded_token_spellings_are_unknown() {
    // `{{ slug }}` is not what `render` substitutes, so tolerating it here
    // would let it survive into the executed command as literal text.
    assert_eq!(unknown_tokens("echo {{ slug }}"), vec![" slug "]);
}

#[test]
fn an_unterminated_brace_pair_does_not_hang_or_panic() {
    assert!(unknown_tokens("echo {{slug").is_empty());
    assert_eq!(unknown_tokens("echo {{title}} {{oops"), vec!["title"]);
}

#[test]
fn empty_token_is_unknown() {
    assert_eq!(unknown_tokens("echo {{}}"), vec![""]);
}

// ── token values ─────────────────────────────────────────────────────────────

#[test]
fn context_derives_abs_path_from_base_and_rel() {
    let ctx = ctx_for("/srv/inbox", "notes/a.md", "");
    assert_eq!(ctx.base_path, "/srv/inbox");
    assert_eq!(ctx.rel_path, "notes/a.md");
    assert!(
        ctx.abs_path.ends_with("a.md") && ctx.abs_path.starts_with("/srv/inbox"),
        "got: {}",
        ctx.abs_path
    );
}

#[test]
fn context_slug_tokens_track_the_title() {
    let titled = ctx_for("/srv", "a.md", "Peace vs Effort");
    assert_eq!(titled.slug, "-peace-vs-effort");
    assert_eq!(titled.slug_or_time, "peace-vs-effort");

    let untitled = ctx_for("/srv", "a.md", "");
    assert_eq!(untitled.slug, "");
    assert_eq!(
        untitled.slug_or_time, "20260716-143255",
        "a commit message must never be empty"
    );
}

#[test]
fn render_substitutes_every_allowed_token() {
    let ctx = ctx_for("/srv/inbox", "notes/a.md", "My Title");
    let out = render(
        "cd {{base_path}} && add {{rel_path}} {{abs_path}} {{slug}} {{slug_or_time}}",
        &ctx,
    );

    assert_eq!(
        out,
        "cd /srv/inbox && add notes/a.md /srv/inbox/notes/a.md -my-title my-title"
    );
}

#[test]
fn render_leaves_unknown_tokens_visible() {
    // Unreachable in practice (validation rejects them first). If it ever
    // happens, a visibly-broken command beats a silently different one.
    let ctx = ctx_for("/srv", "a.md", "T");
    assert_eq!(render("echo {{title}}", &ctx), "echo {{title}}");
}

// ── execution ────────────────────────────────────────────────────────────────
//
// `#[cfg(unix)]`: these assert on real shell behavior, and the command strings
// are `sh` dialect. `hooks::exec` runs `cmd /C` on Windows, whose dialect
// differs enough (`test -d`, `printf`, `read` do not exist) that the same
// strings would test nothing there. The Windows branch of `exec` is covered by
// review, not by these tests — the release matrix runs Windows, so a test using
// sh syntax would fail there rather than skip.

#[cfg(unix)]
#[tokio::test]
async fn a_successful_hook_reports_no_warning() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx_for(dir.path().to_str().unwrap(), "a.md", "");

    assert_eq!(run("exit 0", &ctx).await, None);
}

#[cfg(unix)]
#[tokio::test]
async fn a_failing_hook_warns_and_carries_the_exit_code() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx_for(dir.path().to_str().unwrap(), "a.md", "");

    let warning = run("exit 3", &ctx).await.expect("non-zero exit must warn");

    assert!(warning.contains("exited 3"), "got: {warning}");
}

#[cfg(unix)]
#[tokio::test]
async fn a_failing_hook_reports_its_stderr() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx_for(dir.path().to_str().unwrap(), "a.md", "");

    let warning = run("echo nothing-to-commit >&2; exit 1", &ctx)
        .await
        .expect("non-zero exit must warn");

    assert!(warning.contains("nothing-to-commit"), "got: {warning}");
}

#[cfg(unix)]
#[tokio::test]
async fn a_hook_runs_with_base_path_as_its_working_directory() {
    // This is what lets a hook say `git add <rel_path>` with no `git -C`.
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx_for(dir.path().to_str().unwrap(), "a.md", "");

    assert_eq!(run("test -d .", &ctx).await, None);
    run("touch marker", &ctx).await;

    assert!(
        dir.path().join("marker").exists(),
        "hook should have run inside base_path"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn hook_tokens_reach_the_command() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx_for(dir.path().to_str().unwrap(), "notes/toss.md", "Hello World");

    // Writes the interpolated values out so we can read back what the shell saw.
    let warning = run(
        "printf '%s|%s' '{{rel_path}}' '{{slug_or_time}}' > out",
        &ctx,
    )
    .await;
    assert_eq!(warning, None);

    let seen = std::fs::read_to_string(dir.path().join("out")).unwrap();
    assert_eq!(seen, "notes/toss.md|hello-world");
}

#[cfg(unix)]
#[tokio::test]
async fn a_hook_that_cannot_start_warns_instead_of_erroring() {
    // Non-existent cwd — the note is already written, so this must degrade to a
    // warning, never a lost capture.
    let ctx = ctx_for("/nonexistent/root/for/pour/test", "a.md", "");

    let warning = run("echo hi", &ctx).await.expect("spawn failure must warn");

    assert!(
        warning.contains("post_write_shell"),
        "warning should name the hook, got: {warning}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_hook_cannot_read_stdin() {
    // stdin is /dev/null, so a command that prompts fails fast instead of
    // hanging forever behind a raw-mode TUI that cannot show the prompt.
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx_for(dir.path().to_str().unwrap(), "a.md", "");

    // `read` returns non-zero on EOF, which is exactly the fast failure we want.
    let warning = run("read line", &ctx).await;

    assert!(
        warning.is_some(),
        "reading stdin should not block or succeed"
    );
}
