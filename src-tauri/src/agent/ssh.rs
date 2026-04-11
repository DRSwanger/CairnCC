//! SSH command builder for remote Claude Code execution.
//!
//! Uses the system `ssh` binary — no new crate dependencies.
//! All remote commands are shell-escaped to prevent injection.

use crate::models::RemoteHost;
use crate::process_ext::HideConsole;
use tokio::process::Command;

/// Shell-escape a string using single quotes (POSIX-safe).
/// Any embedded single quote is replaced with `'\''` (end quote, escaped quote, start quote).
pub fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Shell-escape a path, preserving leading `~` for shell tilde expansion on the remote host.
/// `~/projects/my app` → `~/'projects/my app'` (tilde outside quotes, rest escaped).
/// Bare `~` is passed through unquoted so the shell expands it to $HOME.
fn shell_escape_path(s: &str) -> String {
    if s == "~" {
        "~".to_string()
    } else if let Some(rest) = s.strip_prefix("~/") {
        format!("~/{}", shell_escape(rest))
    } else {
        shell_escape(s)
    }
}

/// Expand `~` to `$HOME` for local filesystem paths (e.g. SSH key paths).
/// Unlike shell_escape_path, this does actual expansion since `Command::arg()` doesn't go through a shell.
pub fn expand_local_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = crate::storage::home_dir() {
            let mut p = std::path::PathBuf::from(&home);
            p.push(rest);
            return p.to_string_lossy().into_owned();
        }
    }
    path.to_string()
}

/// Build an SSH `Command` that runs `remote_shell_command` on the remote host.
pub fn build_ssh_command(remote: &RemoteHost, remote_shell_command: &str) -> Command {
    let mut cmd = Command::new("ssh");
    cmd.hide_console();
    cmd.arg("-o").arg("BatchMode=yes");
    cmd.arg("-o").arg("ServerAliveInterval=30");
    cmd.arg("-o").arg("StrictHostKeyChecking=accept-new");

    if remote.port != 22 {
        cmd.arg("-p").arg(remote.port.to_string());
    }
    if let Some(ref key) = remote.key_path {
        // Expand ~/... for local key path (Command::arg doesn't go through shell)
        cmd.arg("-i").arg(expand_local_tilde(key));
    }

    let target = format!("{}@{}", remote.user, remote.host);
    cmd.arg(&target);
    cmd.arg(remote_shell_command);

    log::debug!(
        "[ssh] build_ssh_command: target={}, port={}, key={:?}, cmd_len={}",
        target,
        remote.port,
        remote.key_path,
        remote_shell_command.len()
    );

    cmd
}

/// Build the shell command string to run Claude CLI on the remote host.
///
/// - `cwd`: Already-snapshotted remote_cwd from RunMeta (audit #4).
/// - `claude_args`: CLI arguments (e.g. `["--output-format", "stream-json", ...]`).
/// - `api_key`: Anthropic official API key (`x-api-key` header).
/// - `auth_token`: Third-party platform token (`Authorization: Bearer` header).
/// - `base_url`: Custom API endpoint URL.
#[allow(clippy::too_many_arguments)]
pub fn build_remote_claude_command(
    remote: &RemoteHost,
    cwd: &str,
    claude_args: &[String],
    api_key: Option<&str>,
    auth_token: Option<&str>,
    base_url: Option<&str>,
    models: Option<&[String]>,
    extra_env: Option<&std::collections::HashMap<String, String>>,
) -> String {
    let claude_bin = remote.remote_claude_path.as_deref().unwrap_or("claude");

    let mut parts = Vec::new();

    // cd to remote working directory (preserves ~/... expansion)
    parts.push(format!("cd {}", shell_escape_path(cwd)));

    // Build the claude command with optional env var prefixes
    let mut claude_parts = Vec::new();
    if let Some(key) = api_key {
        claude_parts.push(format!("ANTHROPIC_API_KEY={}", shell_escape(key)));
        // Clear AUTH_TOKEN to avoid remote shell env vars interfering
        claude_parts.push("ANTHROPIC_AUTH_TOKEN=".to_string());
    }
    if let Some(token) = auth_token {
        claude_parts.push(format!("ANTHROPIC_AUTH_TOKEN={}", shell_escape(token)));
        // Clear API_KEY to avoid conflict
        claude_parts.push("ANTHROPIC_API_KEY=".to_string());
    }
    if let Some(url) = base_url {
        claude_parts.push(format!("ANTHROPIC_BASE_URL={}", shell_escape(url)));
    }
    // Inject model tier env vars for third-party platforms
    if let Some(m) = models {
        for (k, v) in crate::commands::session::resolve_model_tiers(m) {
            claude_parts.push(format!("{}={}", k, shell_escape(&v)));
        }
    }
    // Inject extra env vars (only allow safe key names: [A-Z0-9_])
    if let Some(extra) = extra_env {
        for (k, v) in extra {
            if k.chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
            {
                claude_parts.push(format!("{}={}", k, shell_escape(v)));
            } else {
                log::warn!("[ssh] skipping extra_env key with invalid chars: {}", k);
            }
        }
    }
    // Enable file checkpointing in SDK/non-interactive mode
    claude_parts.push("CLAUDE_CODE_ENABLE_SDK_FILE_CHECKPOINTING=1".to_string());
    // Escape claude binary path (preserves ~/... expansion)
    claude_parts.push(shell_escape_path(claude_bin));
    for arg in claude_args {
        claude_parts.push(shell_escape(arg));
    }

    parts.push(claude_parts.join(" "));

    let full_cmd = parts.join(" && ");
    log::debug!(
        "[ssh] build_remote_claude_command: cwd={}, bin={}, args={}, has_key={}, has_token={}, has_url={}, has_model={}, extra_env_count={}",
        cwd,
        claude_bin,
        claude_args.len(),
        api_key.is_some(),
        auth_token.is_some(),
        base_url.is_some(),
        models.is_some(),
        extra_env.map_or(0, |e| e.len())
    );

    full_cmd
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::RemoteHost;

    fn make_remote(claude_path: Option<&str>) -> RemoteHost {
        RemoteHost {
            name: "test".into(),
            host: "192.168.1.10".into(),
            user: "dallas".into(),
            port: 22,
            key_path: None,
            remote_cwd: None,
            remote_claude_path: claude_path.map(str::to_string),
            forward_api_key: false,
        }
    }

    // ── shell_escape ──

    #[test]
    fn escape_plain_string() {
        assert_eq!(shell_escape("hello"), "'hello'");
    }

    #[test]
    fn escape_string_with_spaces() {
        assert_eq!(shell_escape("hello world"), "'hello world'");
    }

    #[test]
    fn escape_string_with_single_quote() {
        // "it's" → 'it'\''s'
        assert_eq!(shell_escape("it's"), "'it'\\''s'");
    }

    #[test]
    fn escape_string_with_special_chars() {
        assert_eq!(shell_escape("$HOME;rm -rf /"), "'$HOME;rm -rf /'");
    }

    #[test]
    fn escape_empty_string() {
        assert_eq!(shell_escape(""), "''");
    }

    #[test]
    fn escape_backtick_and_dollar() {
        // Backticks and $ are neutralised by single-quote wrapping
        assert_eq!(shell_escape("`id`"), "'`id`'");
        assert_eq!(shell_escape("$(id)"), "'$(id)'");
    }

    // ── shell_escape_path ──

    #[test]
    fn escape_path_with_tilde_prefix() {
        // ~/my projects → ~/'my projects'
        assert_eq!(shell_escape_path("~/my projects"), "~/'my projects'");
    }

    #[test]
    fn escape_absolute_path() {
        assert_eq!(shell_escape_path("/home/dallas/code"), "'/home/dallas/code'");
    }

    #[test]
    fn escape_path_tilde_bare() {
        // bare "~" must stay unquoted so the shell expands it to $HOME
        assert_eq!(shell_escape_path("~"), "~");
    }

    #[test]
    fn escape_path_tilde_only() {
        // "~/" strips "~/" → rest is "" → shell_escape("") → "''" → "~/" + "''" = "~/''"
        assert_eq!(shell_escape_path("~/"), "~/''"  );
    }

    // ── build_remote_claude_command ──

    #[test]
    fn cmd_uses_default_claude_bin_when_path_not_set() {
        let remote = make_remote(None);
        let cmd = build_remote_claude_command(&remote, "/home/dallas", &[], None, None, None, None, None);
        assert!(cmd.contains(" 'claude'") || cmd.ends_with(" 'claude'"), "expected 'claude' in: {cmd}");
    }

    #[test]
    fn cmd_uses_custom_claude_path() {
        let remote = make_remote(Some("/opt/bin/claude"));
        let cmd = build_remote_claude_command(&remote, "/home/dallas", &[], None, None, None, None, None);
        assert!(cmd.contains("'/opt/bin/claude'"), "expected escaped custom path in: {cmd}");
    }

    #[test]
    fn cmd_uses_tilde_path_unquoted() {
        let remote = make_remote(Some("~/.local/bin/claude"));
        let cmd = build_remote_claude_command(&remote, "/home/dallas", &[], None, None, None, None, None);
        assert!(cmd.contains("~/'"), "tilde should be outside quotes in: {cmd}");
    }

    #[test]
    fn cmd_starts_with_cd() {
        let remote = make_remote(None);
        let cmd = build_remote_claude_command(&remote, "/home/dallas/myproject", &[], None, None, None, None, None);
        assert!(cmd.starts_with("cd '/home/dallas/myproject'"), "cmd={cmd}");
    }

    #[test]
    fn cmd_includes_api_key_and_clears_auth_token() {
        let remote = make_remote(None);
        let cmd = build_remote_claude_command(&remote, "/tmp", &[], Some("sk-ant-test"), None, None, None, None);
        assert!(cmd.contains("ANTHROPIC_API_KEY='sk-ant-test'"), "cmd={cmd}");
        assert!(cmd.contains("ANTHROPIC_AUTH_TOKEN="), "cmd={cmd}");
        assert!(!cmd.contains("ANTHROPIC_AUTH_TOKEN='"), "auth token should be cleared, not set: {cmd}");
    }

    #[test]
    fn cmd_includes_auth_token_and_clears_api_key() {
        let remote = make_remote(None);
        let cmd = build_remote_claude_command(&remote, "/tmp", &[], None, Some("Bearer xyz"), None, None, None);
        assert!(cmd.contains("ANTHROPIC_AUTH_TOKEN='Bearer xyz'"), "cmd={cmd}");
        assert!(cmd.contains("ANTHROPIC_API_KEY="), "cmd={cmd}");
        assert!(!cmd.contains("ANTHROPIC_API_KEY='"), "api key should be cleared: {cmd}");
    }

    #[test]
    fn cmd_api_key_with_special_chars_is_escaped() {
        let remote = make_remote(None);
        // Key contains a single-quote and shell metacharacters.
        // The shell_escape implementation wraps in single quotes and breaks on each quote:
        //   sk-ant-test'key;rm -rf /  →  'sk-ant-test'\''key;rm -rf /'
        // The semicolon appears inside single quotes so it cannot be executed as a command.
        let tricky = "sk-ant-test'key;rm -rf /";
        let cmd = build_remote_claude_command(&remote, "/tmp", &[], Some(tricky), None, None, None, None);
        assert!(cmd.contains("ANTHROPIC_API_KEY='sk-ant-test'\\''key;rm -rf /'"), "cmd={cmd}");
    }

    #[test]
    fn cmd_includes_base_url() {
        let remote = make_remote(None);
        let cmd = build_remote_claude_command(&remote, "/tmp", &[], None, None, Some("https://api.example.com"), None, None);
        assert!(cmd.contains("ANTHROPIC_BASE_URL='https://api.example.com'"), "cmd={cmd}");
    }

    #[test]
    fn cmd_joined_with_and_and() {
        let remote = make_remote(None);
        let cmd = build_remote_claude_command(&remote, "/tmp", &[], None, None, None, None, None);
        assert!(cmd.contains(" && "), "parts must be joined with &&: {cmd}");
    }

    #[test]
    fn cmd_includes_checkpointing_env() {
        let remote = make_remote(None);
        let cmd = build_remote_claude_command(&remote, "/tmp", &[], None, None, None, None, None);
        assert!(cmd.contains("CLAUDE_CODE_ENABLE_SDK_FILE_CHECKPOINTING=1"), "cmd={cmd}");
    }

    #[test]
    fn cmd_extra_env_valid_key_included() {
        let remote = make_remote(None);
        let mut extra = std::collections::HashMap::new();
        extra.insert("API_TIMEOUT_MS".to_string(), "5000".to_string());
        let cmd = build_remote_claude_command(&remote, "/tmp", &[], None, None, None, None, Some(&extra));
        assert!(cmd.contains("API_TIMEOUT_MS='5000'"), "cmd={cmd}");
    }

    #[test]
    fn cmd_extra_env_invalid_key_rejected() {
        let remote = make_remote(None);
        let mut extra = std::collections::HashMap::new();
        extra.insert("bad-key".to_string(), "value".to_string());
        extra.insert("also bad key".to_string(), "v".to_string());
        let cmd = build_remote_claude_command(&remote, "/tmp", &[], None, None, None, None, Some(&extra));
        assert!(!cmd.contains("bad-key"), "invalid key must be rejected: {cmd}");
        assert!(!cmd.contains("also bad key"), "cmd={cmd}");
    }

    #[test]
    fn cmd_claude_args_are_escaped() {
        let remote = make_remote(None);
        let args = vec!["--resume".to_string(), "session id with spaces".to_string()];
        let cmd = build_remote_claude_command(&remote, "/tmp", &args, None, None, None, None, None);
        assert!(cmd.contains("'--resume'"), "cmd={cmd}");
        assert!(cmd.contains("'session id with spaces'"), "cmd={cmd}");
    }
}
