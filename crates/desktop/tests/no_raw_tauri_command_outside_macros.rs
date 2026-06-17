//! Hard CI gate: no raw `#[tauri::command]` may appear in `src/commands/`
//! or `src/oauth/` unless wrapped by `#[klynt_command]` or
//! `#[klynt_raw_command]`. Ensures the convention can't degrade silently.

use std::path::PathBuf;

#[test]
fn no_raw_tauri_command_outside_macros() {
    let crate_root: PathBuf = std::env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap());
    let dirs = [
        crate_root.join("src/commands"),
        crate_root.join("src/oauth"),
    ];

    let mut raw: Vec<(PathBuf, usize, String)> = Vec::new();

    for dir in &dirs {
        visit_rs_files(dir, &mut |path| {
            let content = std::fs::read_to_string(path)?;
            for (i, line) in content.lines().enumerate() {
                if line.contains("#[tauri::command") {
                    let context = content
                        .lines()
                        .skip(i.saturating_sub(3))
                        .take(7)
                        .collect::<Vec<_>>()
                        .join("\n");
                    if !context.contains("klynt_command") && !context.contains("klynt_raw_command")
                    {
                        raw.push((path.to_path_buf(), i + 1, context));
                    }
                }
            }
            Ok(())
        })
        .unwrap();
    }

    if !raw.is_empty() {
        let mut msg = String::from("Raw #[tauri::command] found outside klynt macros:\n");
        for (path, line, context) in raw {
            msg.push_str(&format!("\n{}:{}\n{context}\n", path.display(), line));
        }
        panic!("{msg}");
    }
}

fn visit_rs_files(
    dir: &std::path::Path,
    visitor: &mut dyn FnMut(&std::path::Path) -> std::io::Result<()>,
) -> std::io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            visit_rs_files(&path, visitor)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            visitor(&path)?;
        }
    }
    Ok(())
}
