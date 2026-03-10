//! Shell integration hook for capturing terminal commands.
//!
//! Generates zsh/bash scripts that fire-and-forget command events to the
//! klyntbot ingestion API after each command completes.

use std::io::Write;

/// Generate the zsh shell hook script.
pub fn generate_zsh_hook(api_url: &str, token: &str) -> String {
    format!(
        r#"
# klyntbot shell integration — auto-generated
# Remove this block to uninstall

_klyntbot_preexec() {{
    export _KLYNTBOT_CMD_START=$(($(date +%s)))
    export _KLYNTBOT_CMD="$1"
}}

_klyntbot_precmd() {{
    local exit_code=$?
    if [[ -n "$_KLYNTBOT_CMD" ]]; then
        local end=$(($(date +%s)))
        local duration_secs=$((end - _KLYNTBOT_CMD_START))
        (curl -sf -X POST "{api_url}/api/v1/ingest" \
            -H "Authorization: Bearer {token}" \
            -H "Content-Type: application/json" \
            -d "$(printf '{{"source":"terminal","actor":"user","resource_type":"command","resource_name":"%s","action":"run","content_preview":"%s","metadata":{{"cwd":"%s","exit_code":%d,"duration_secs":%d,"shell":"zsh"}},"duration_secs":%d}}' \
            "$(echo "$_KLYNTBOT_CMD" | head -c 200 | sed 's/"/\\"/g; s/\\/\\\\/g')" \
            "$(echo "$_KLYNTBOT_CMD" | head -c 500 | sed 's/"/\\"/g; s/\\/\\\\/g')" \
            "$(echo "$PWD" | sed 's/"/\\"/g')" \
            "$exit_code" "$duration_secs" "$duration_secs")" &) 2>/dev/null
        unset _KLYNTBOT_CMD
    fi
}}

autoload -Uz add-zsh-hook
add-zsh-hook preexec _klyntbot_preexec
add-zsh-hook precmd _klyntbot_precmd
# end klyntbot shell integration
"#,
        api_url = api_url,
        token = token
    )
}

/// Generate the bash shell hook script.
pub fn generate_bash_hook(api_url: &str, token: &str) -> String {
    format!(
        r#"
# klyntbot shell integration — auto-generated
# Remove this block to uninstall

_klyntbot_preexec() {{
    _KLYNTBOT_CMD_START=$(($(date +%s)))
    _KLYNTBOT_CMD="$(HISTTIMEFORMAT= history 1 | sed 's/^ *[0-9]* *//')"
}}

_klyntbot_precmd() {{
    local exit_code=$?
    if [[ -n "$_KLYNTBOT_CMD" ]]; then
        local end=$(($(date +%s)))
        local duration_secs=$((end - _KLYNTBOT_CMD_START))
        (curl -sf -X POST "{api_url}/api/v1/ingest" \
            -H "Authorization: Bearer {token}" \
            -H "Content-Type: application/json" \
            -d "$(printf '{{"source":"terminal","actor":"user","resource_type":"command","resource_name":"%s","action":"run","content_preview":"%s","metadata":{{"cwd":"%s","exit_code":%d,"duration_secs":%d,"shell":"bash"}},"duration_secs":%d}}' \
            "$(echo "$_KLYNTBOT_CMD" | head -c 200 | sed 's/"/\\"/g; s/\\/\\\\/g')" \
            "$(echo "$_KLYNTBOT_CMD" | head -c 500 | sed 's/"/\\"/g; s/\\/\\\\/g')" \
            "$(echo "$PWD" | sed 's/"/\\"/g')" \
            "$exit_code" "$duration_secs" "$duration_secs")" &) 2>/dev/null
        unset _KLYNTBOT_CMD
    fi
}}

trap '_klyntbot_preexec' DEBUG
PROMPT_COMMAND="_klyntbot_precmd${{PROMPT_COMMAND:+;$PROMPT_COMMAND}}"
# end klyntbot shell integration
"#,
        api_url = api_url,
        token = token
    )
}

pub const MARKER_START: &str = "# klyntbot shell integration";
const MARKER_END: &str = "# end klyntbot shell integration";

/// Return the rc file path for the given shell, or an error for unsupported shells.
pub fn rc_file_for_shell(shell: &str) -> common::Result<String> {
    let home = std::env::var("HOME").map_err(|_| {
        common::KlyntbotError::Config(common::ConfigError::Invalid("HOME not set".into()))
    })?;
    match shell {
        "zsh" => Ok(format!("{home}/.zshrc")),
        "bash" => Ok(format!("{home}/.bashrc")),
        _ => Err(common::KlyntbotError::Config(common::ConfigError::Invalid(
            format!("Unsupported shell: {shell}"),
        ))),
    }
}

/// Install the shell hook by appending to the user's rc file.
pub fn install(shell: &str, api_url: &str, token: &str) -> common::Result<String> {
    let rc_file = rc_file_for_shell(shell)?;
    let hook_script = match shell {
        "zsh" => generate_zsh_hook(api_url, token),
        "bash" => generate_bash_hook(api_url, token),
        _ => unreachable!("rc_file_for_shell already validated"),
    };

    let contents = std::fs::read_to_string(&rc_file).unwrap_or_default();
    if contents.contains(MARKER_START) {
        return Ok(format!("Already installed in {rc_file}"));
    }

    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&rc_file)
        .map_err(|e| {
            common::KlyntbotError::Config(common::ConfigError::Invalid(format!(
                "Cannot write {rc_file}: {e}"
            )))
        })?;

    writeln!(file, "{hook_script}").map_err(|e| {
        common::KlyntbotError::Config(common::ConfigError::Invalid(format!("Write failed: {e}")))
    })?;

    Ok(format!(
        "Installed to {rc_file}. Restart your terminal or run: source {rc_file}"
    ))
}

/// Uninstall the shell hook by removing the block from the rc file.
pub fn uninstall(shell: &str) -> common::Result<String> {
    let rc_file = rc_file_for_shell(shell)?;

    let contents = std::fs::read_to_string(&rc_file).map_err(|e| {
        common::KlyntbotError::Config(common::ConfigError::Invalid(format!(
            "Cannot read {rc_file}: {e}"
        )))
    })?;

    let cleaned = remove_block(&contents);
    std::fs::write(&rc_file, cleaned).map_err(|e| {
        common::KlyntbotError::Config(common::ConfigError::Invalid(format!(
            "Cannot write {rc_file}: {e}"
        )))
    })?;

    Ok(format!("Removed from {rc_file}"))
}

/// Remove the klyntbot block between start/end markers.
fn remove_block(contents: &str) -> String {
    let mut result = String::new();
    let mut in_block = false;

    for line in contents.lines() {
        if line.trim().starts_with(MARKER_START) {
            in_block = true;
            continue;
        }
        if line.trim().starts_with(MARKER_END) {
            in_block = false;
            continue;
        }
        if !in_block {
            result.push_str(line);
            result.push('\n');
        }
    }

    // Trim trailing whitespace added by the loop
    while result.ends_with("\n\n") {
        result.pop();
    }
    result
}

/// Detect the current shell from $SHELL env var.
pub fn detect_shell() -> String {
    std::env::var("SHELL")
        .ok()
        .and_then(|s| s.rsplit('/').next().map(String::from))
        .unwrap_or_else(|| "zsh".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_zsh_hook_contains_markers() {
        let script = generate_zsh_hook("http://127.0.0.1:3456", "test-token");
        assert!(script.contains(MARKER_START));
        assert!(script.contains(MARKER_END));
        assert!(script.contains("test-token"));
        assert!(script.contains("127.0.0.1:3456"));
        assert!(script.contains("add-zsh-hook preexec"));
    }

    #[test]
    fn test_generate_bash_hook_contains_markers() {
        let script = generate_bash_hook("http://127.0.0.1:3456", "test-token");
        assert!(script.contains(MARKER_START));
        assert!(script.contains(MARKER_END));
        assert!(script.contains("trap '_klyntbot_preexec' DEBUG"));
        assert!(script.contains("PROMPT_COMMAND"));
    }

    #[test]
    fn test_remove_block() {
        let contents = "before\n# klyntbot shell integration\nhook stuff\n# end klyntbot shell integration\nafter\n";
        let cleaned = remove_block(contents);
        assert!(cleaned.contains("before"));
        assert!(cleaned.contains("after"));
        assert!(!cleaned.contains("hook stuff"));
    }

    #[test]
    fn test_remove_block_no_markers() {
        let contents = "line1\nline2\n";
        let cleaned = remove_block(contents);
        assert!(cleaned.contains("line1"));
        assert!(cleaned.contains("line2"));
    }

    #[test]
    fn test_detect_shell_default() {
        // May vary by environment, just ensure no panic
        let shell = detect_shell();
        assert!(!shell.is_empty());
    }
}
