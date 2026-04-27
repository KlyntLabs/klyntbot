//! Hard CI gate: no raw `#[tauri::command]` may appear in `crates/desktop/src/commands/`
//! or `crates/desktop/src/oauth/` unless wrapped by `#[klynt_command]` or
//! `#[klynt_raw_command]`. Ensures the convention can't degrade silently.

#[test]
fn no_raw_tauri_command_outside_macros() {
    let dirs = [
        "crates/desktop/src/commands/",
        "crates/desktop/src/oauth/",
    ];

    for dir in &dirs {
        let output = std::process::Command::new("rg")
            .args(["-l", "#\\[tauri::command", dir])
            .output()
            .expect("rg available — install ripgrep if missing");
        let files: Vec<_> = String::from_utf8(output.stdout).unwrap().lines().map(String::from).collect();

        for file in &files {
            let content = std::fs::read_to_string(file).unwrap();
            for (i, line) in content.lines().enumerate() {
                if line.contains("#[tauri::command") {
                    let context = content
                        .lines()
                        .skip(i.saturating_sub(3))
                        .take(7)
                        .collect::<Vec<_>>()
                        .join("\n");
                    assert!(
                        context.contains("klynt_command") || context.contains("klynt_raw_command"),
                        "Raw #[tauri::command] in {file} at line {} — must be wrapped by #[klynt_command] or #[klynt_raw_command]",
                        i + 1
                    );
                }
            }
        }
    }
}
