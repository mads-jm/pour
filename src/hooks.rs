//! Post-write shell hooks (`post_write_shell`).
//!
//! An optional per-module command run after a successful write. Its reason to
//! exist is that a file on disk is not always the end of the capture: an agent
//! inbox kept in a git repo is only *delivered* once the toss is committed and
//! pushed.
//!
//! # This is arbitrary command execution from config
//!
//! Said plainly, because it is. The mitigations, in order of how much weight
//! they carry:
//!
//! 1. **Safe tokens only.** Only [`ALLOWED_TOKENS`] interpolate, and every one
//!    of them is Pour-generated: two are config-authored roots, one is a path
//!    Pour rendered and validated, and the slugs are `[a-z0-9-]` by
//!    construction. **None can carry raw user text.** That is what makes it
//!    sound to substitute into a shell string without quoting — the safety is
//!    in what the tokens *can be*, not in escaping what they are.
//! 2. **Unknown tokens are rejected at config-load time**, not stripped. A
//!    stripped `{{title}}` would look supported while silently rewriting the
//!    command.
//! 3. **Serve is opt-in, defaulting off.** A LAN-submitted capture does not
//!    fire the hook unless the module sets `post_write_shell_on_serve = true`.
//!    (The wire DTO cannot carry config keys at all — see
//!    `src/server/dto/requests.rs` — so this is a second gate, not the only one.)
//! 4. **Best-effort.** The note is on disk before the hook runs. A failing hook
//!    warns; it never loses the capture.
//!
//! **If a hook ever needs to carry a value that is not on this list — anything
//! derived from user text — do not add it here.** That is the trigger to switch
//! to argv-style execution, where a value is an argument rather than a fragment
//! of a command line. Adding a user-text token to `ALLOWED_TOKENS` while
//! keeping shell-string interpolation is a shell-injection hole.

use crate::output::template::slug_tokens;
use chrono::{DateTime, Local};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

/// The only tokens `post_write_shell` may interpolate.
///
/// Every entry must be Pour-generated and incapable of carrying raw user text.
/// Read the module docs before touching this list.
pub const ALLOWED_TOKENS: &[&str] = &["base_path", "rel_path", "abs_path", "slug", "slug_or_time"];

/// How long to wait for a hook before giving up on it.
///
/// A hook is normally sub-second (`git commit`) to a couple of seconds
/// (`git push`). The bound exists for the pathological case — a command that
/// blocks forever on input it will never get — because the TUI awaits this and
/// would otherwise be wedged with no way out. On timeout Pour stops *waiting*;
/// it cannot reap the child, which keeps running detached.
const HOOK_TIMEOUT: Duration = Duration::from_secs(30);

/// The allowed token list, rendered for an error message.
pub fn allowed_tokens_display() -> String {
    ALLOWED_TOKENS
        .iter()
        .map(|t| format!("{{{{{t}}}}}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Every `{{token}}` in `command` that is not in [`ALLOWED_TOKENS`].
///
/// Matching is exact and unforgiving: `{{ slug }}` is reported as unknown
/// rather than accepted, because [`render`] only substitutes the exact form and
/// a "tolerated" spelling would survive into the executed command as literal
/// text. Duplicates are reported once.
pub fn unknown_tokens(command: &str) -> Vec<String> {
    let mut unknown: Vec<String> = Vec::new();
    let mut rest = command;

    while let Some(start) = rest.find("{{") {
        let after_open = &rest[start + 2..];
        let Some(end) = after_open.find("}}") else {
            break;
        };
        let token = &after_open[..end];
        if !ALLOWED_TOKENS.contains(&token) && !unknown.iter().any(|t| t == token) {
            unknown.push(token.to_string());
        }
        rest = &after_open[end + 2..];
    }

    unknown
}

/// The resolved token values for one capture's hook.
pub struct HookContext {
    /// The module's resolved root — the hook's working directory.
    pub base_path: String,
    /// The written file, relative to `base_path`.
    pub rel_path: String,
    /// The written file, absolute.
    pub abs_path: String,
    /// Filename slug: `-my-title`, or empty when untitled. Dash-prefixed, so
    /// that the same token reads correctly in a path template.
    pub slug: String,
    /// Bare slug (`my-title`) when titled, else a timestamp — for a commit
    /// message that should never be empty.
    pub slug_or_time: String,
}

impl HookContext {
    /// Build the token values for a capture written to `rel_path` under `base_path`.
    ///
    /// `title` is the capture's `title` field value (empty when the module has
    /// no such field), and is only ever read through
    /// [`slug_tokens`][crate::output::template::slug_tokens] — it never reaches
    /// the command as typed text.
    pub fn new(base_path: &str, rel_path: &str, title: &str, now: DateTime<Local>) -> Self {
        let (slug, slug_or_time) = slug_tokens(title, now);
        let abs_path = Path::new(base_path)
            .join(rel_path.replace('\\', "/"))
            .to_string_lossy()
            .into_owned();

        Self {
            base_path: base_path.to_string(),
            rel_path: rel_path.to_string(),
            abs_path,
            slug,
            slug_or_time,
        }
    }

    fn value_for(&self, token: &str) -> Option<&str> {
        match token {
            "base_path" => Some(&self.base_path),
            "rel_path" => Some(&self.rel_path),
            "abs_path" => Some(&self.abs_path),
            "slug" => Some(&self.slug),
            "slug_or_time" => Some(&self.slug_or_time),
            _ => None,
        }
    }
}

/// Interpolate the allowed tokens into `command`.
///
/// Unknown tokens are left untouched — they are rejected at config-load time,
/// so reaching this function with one means validation was bypassed, and
/// leaving `{{title}}` visible in a failing command is far kinder than silently
/// running a different command than the one written down.
pub fn render(command: &str, ctx: &HookContext) -> String {
    let mut out = command.to_string();
    for token in ALLOWED_TOKENS {
        if let Some(value) = ctx.value_for(token) {
            out = out.replace(&format!("{{{{{token}}}}}"), value);
        }
    }
    out
}

/// Run a module's `post_write_shell`, returning `Some(warning)` on failure.
///
/// `Ok`/`None` means the hook exited 0. Every failure mode — non-zero exit,
/// spawn failure, timeout — is a *warning*, never an error: the note is already
/// written and the hook is best-effort by contract.
///
/// The child gets a null stdin and piped stdout/stderr. Both matter: the TUI
/// owns a raw-mode terminal, so an inherited stream would scribble over the
/// frame, and an inherited stdin would let a credential prompt hang forever
/// behind a UI that cannot show it.
pub async fn run(command: &str, ctx: &HookContext) -> Option<String> {
    let rendered = render(command, ctx);
    let cwd = ctx.base_path.clone();

    // spawn_blocking, not tokio::process: `process` is not in our tokio feature
    // set, and adding a feature to wait on one short-lived child is a poor
    // trade against a blocking thread we are awaiting anyway.
    let joined = tokio::time::timeout(
        HOOK_TIMEOUT,
        tokio::task::spawn_blocking(move || exec(&rendered, &cwd)),
    )
    .await;

    match joined {
        Err(_) => Some(format!(
            "post_write_shell timed out after {}s (the command is still running)",
            HOOK_TIMEOUT.as_secs()
        )),
        Ok(Err(e)) => Some(format!("post_write_shell task failed: {e}")),
        Ok(Ok(Err(e))) => Some(format!("post_write_shell failed to start: {e}")),
        Ok(Ok(Ok(outcome))) => outcome,
    }
}

/// Execute the rendered command through the OS shell, from `cwd`.
///
/// Shell — not argv — because the point is to let a hook be a real one-liner
/// (`… && …`, `git -C … push`). That is only defensible because of what the
/// tokens can be; see the module docs.
fn exec(command: &str, cwd: &str) -> std::io::Result<Option<String>> {
    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(command);
        c
    } else {
        let mut c = Command::new("sh");
        c.arg("-c").arg(command);
        c
    };

    let output = cmd
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    if output.status.success() {
        return Ok(None);
    }

    let code = output
        .status
        .code()
        .map(|c| c.to_string())
        .unwrap_or_else(|| "signal".to_string());

    let detail = first_line(&output.stderr)
        .or_else(|| first_line(&output.stdout))
        .unwrap_or_else(|| "no output".to_string());

    Ok(Some(format!("post_write_shell exited {code}: {detail}")))
}

/// First non-empty line of captured output, truncated for a one-line warning.
fn first_line(bytes: &[u8]) -> Option<String> {
    const MAX: usize = 200;

    let text = String::from_utf8_lossy(bytes);
    let line = text.lines().map(str::trim).find(|l| !l.is_empty())?;

    if line.chars().count() > MAX {
        Some(format!("{}…", line.chars().take(MAX).collect::<String>()))
    } else {
        Some(line.to_string())
    }
}
