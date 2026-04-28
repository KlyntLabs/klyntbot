//! Tool-use dispatch for Codex.

use super::{base, decode, payload};
use crate::event::{AgentEventV1, EventKind, FileOp};
use common::Result;
use std::path::PathBuf;

pub(super) fn parse_tool_use(raw: &[u8]) -> Result<Option<AgentEventV1>> {
    let b: payload::ToolUseBody = decode(raw)?;
    let kind = match b.tool_name.as_str() {
        "bash" | "shell" => classify_bash(&b),
        "read" => file_edit(&b, FileOp::Read),
        "write" => file_edit(&b, FileOp::Create),
        "edit" => file_edit(&b, FileOp::Modify),
        _ => tool_call(&b),
    };
    Ok(Some(base(b.common, kind)))
}

fn classify_bash(b: &payload::ToolUseBody) -> EventKind {
    let cmd = b
        .tool_input
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let trimmed = cmd.trim();
    if let Some(fw) = detect_framework(trimmed) {
        let stdout = b
            .tool_response
            .get("stdout")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let (passed, failed) = parse_results(fw, stdout);
        return EventKind::TestRun {
            command: trimmed.to_string(),
            framework: Some(fw.into()),
            passed,
            failed,
            duration_ms: b.duration_ms,
        };
    }
    tool_call(b)
}

fn tool_call(b: &payload::ToolUseBody) -> EventKind {
    let args = serde_json::to_string(&b.tool_input).unwrap_or_default();
    let args_preview = truncate(&args, 512);
    let result = serde_json::to_string(&b.tool_response).unwrap_or_default();
    let result_preview = truncate(&result, 512);
    let ok = b
        .tool_response
        .get("exit_code")
        .and_then(|v| v.as_i64())
        .map(|c| c == 0)
        .unwrap_or(true);
    EventKind::ToolCall {
        tool: b.tool_name.clone(),
        args_preview,
        ok,
        duration_ms: b.duration_ms,
        result_preview,
    }
}

fn file_edit(b: &payload::ToolUseBody, op: FileOp) -> EventKind {
    let path = b
        .tool_input
        .get("file_path")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .unwrap_or_default();
    let bytes = b
        .tool_response
        .get("bytes")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    EventKind::FileEdit {
        path,
        op,
        bytes,
        diff_preview: None,
    }
}

fn detect_framework(cmd: &str) -> Option<&'static str> {
    let first = cmd.split_whitespace().next()?;
    let looks_like = |n: &str| first == n || cmd.starts_with(&format!("{n} "));
    if looks_like("pytest") {
        return Some("pytest");
    }
    if looks_like("cargo") && cmd.contains("test") {
        return Some("cargo");
    }
    if (looks_like("npm") || looks_like("pnpm") || looks_like("yarn") || looks_like("bun"))
        && cmd.contains("test")
    {
        return Some("node");
    }
    if looks_like("go") && cmd.contains("test") {
        return Some("go");
    }
    if looks_like("jest") {
        return Some("jest");
    }
    if looks_like("vitest") {
        return Some("vitest");
    }
    None
}

fn parse_results(framework: &str, stdout: &str) -> (u32, u32) {
    match framework {
        "cargo" => {
            let passed = capture_u32(stdout, r"(\d+)\s+passed").unwrap_or(0);
            let failed = capture_u32(stdout, r"(\d+)\s+failed").unwrap_or(0);
            (passed, failed)
        }
        "pytest" => {
            let passed = capture_u32(stdout, r"(\d+)\s+passed").unwrap_or(0);
            let failed = capture_u32(stdout, r"(\d+)\s+failed").unwrap_or(0);
            (passed, failed)
        }
        _ => {
            let passed = capture_u32(stdout, r"(\d+)\s+pass").unwrap_or(0);
            let failed = capture_u32(stdout, r"(\d+)\s+fail").unwrap_or(0);
            (passed, failed)
        }
    }
}

fn capture_u32(text: &str, pat: &str) -> Option<u32> {
    let marker = pat
        .split(')')
        .nth(1)?
        .trim_start_matches("\\s+")
        .trim_start_matches('+');
    let idx = text.find(marker)?;
    let prefix = &text[..idx];
    let digits: String = prefix
        .chars()
        .rev()
        .skip_while(|c| c.is_whitespace())
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    digits.parse().ok()
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}
