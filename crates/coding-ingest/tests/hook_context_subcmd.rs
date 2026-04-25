use std::process::Command;

#[test]
fn context_session_start_returns_zero_with_offline_desktop() {
    let bin = env!("CARGO_BIN_EXE_klyntbot-hook");
    let tmp = tempfile::tempdir().unwrap();
    let out = Command::new(bin)
        .args(["context", "--session-start", "--repo", "repo:demo"])
        .env("KLYNTBOT_HOME", tmp.path())
        .output()
        .expect("run");
    assert!(out.status.success(), "stdout={}", String::from_utf8_lossy(&out.stdout));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("klyntbot recall unavailable") || stdout.contains("Project memory"),
        "stdout={stdout}"
    );
}

#[test]
fn context_user_prompt_returns_zero_with_offline_desktop() {
    let bin = env!("CARGO_BIN_EXE_klyntbot-hook");
    let tmp = tempfile::tempdir().unwrap();
    let out = Command::new(bin)
        .args(["context", "--user-prompt-submit", "how do I fix auth", "--repo", "repo:demo"])
        .env("KLYNTBOT_HOME", tmp.path())
        .output()
        .expect("run");
    assert!(out.status.success(), "stdout={}", String::from_utf8_lossy(&out.stdout));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("klyntbot recall unavailable") || stdout.contains("likely relevant"),
        "stdout={stdout}"
    );
}
