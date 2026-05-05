use crate::AppCore;
use common::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, specta::Type)]
pub struct SandboxTestResult {
    pub ok: bool,
    pub details: String,
}

#[derive(Debug, Serialize, Deserialize, specta::Type)]
pub struct DiagnosticItem {
    pub name: String,
    pub status: String, // "green" | "yellow" | "red"
    pub detail: String,
}

#[derive(Debug, Serialize, Deserialize, specta::Type)]
pub struct DiagnosticChecklist {
    pub items: Vec<DiagnosticItem>,
}

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_test_sandbox(&self) -> Result<SandboxTestResult> {
        Ok(SandboxTestResult {
            ok: true,
            details: "sandbox stub (Phase 1)".into(),
        })
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn coding_doctor(&self) -> Result<DiagnosticChecklist> {
        let mut items = Vec::new();
        let home = self.klyntbot_home().await;

        // hooks.toml
        let hooks_path: PathBuf = home.join("hooks.toml");
        let hooks_exists = tokio::fs::try_exists(&hooks_path).await.unwrap_or(false);
        items.push(if !hooks_exists {
            diag("hooks.toml", "green", "absent (default)")
        } else {
            match tokio::task::spawn_blocking({
                let hooks_path = hooks_path.clone();
                move || klynt_hooks::HookEngine::load_from_path(&hooks_path)
            })
            .await
            {
                Ok(Ok(_)) => diag("hooks.toml", "green", "parsed"),
                Ok(Err(e)) => diag("hooks.toml", "red", &e.to_string()),
                Err(_) => diag("hooks.toml", "red", "load panicked"),
            }
        });

        // starlark rules
        let rules_dir = home.join("rules");
        items.push(
            match tokio::task::spawn_blocking({
                let rules_dir = rules_dir.clone();
                move || klynt_execpolicy::Policy::load_from_dir(&rules_dir)
            })
            .await
            {
                Ok(Ok(_)) => diag(
                    "starlark rules",
                    "green",
                    &format!("dir: {}", rules_dir.display()),
                ),
                Ok(Err(e)) => diag("starlark rules", "red", &e.to_string()),
                Err(_) => diag("starlark rules", "red", "load panicked"),
            },
        );

        // sandbox
        items.push(match self.coding_test_sandbox().await {
            Ok(r) if r.ok => diag("sandbox", "green", &r.details),
            Ok(r) => diag("sandbox", "red", &r.details),
            Err(e) => diag("sandbox", "red", &e.to_string()),
        });

        // skills
        items.push(match self.coding_skills_list().await {
            Ok(list) if !list.is_empty() => diag(
                "skill loader",
                "green",
                &format!("{} skills indexed", list.len()),
            ),
            Ok(_) => diag("skill loader", "yellow", "0 skills indexed"),
            Err(e) => diag("skill loader", "red", &e.to_string()),
        });

        // coding-memory recall (best-effort)
        items.push(diag("coding-memory", "yellow", "stub (Phase 1)"));

        Ok(DiagnosticChecklist { items })
    }
}

fn diag(name: &str, status: &str, detail: &str) -> DiagnosticItem {
    DiagnosticItem {
        name: name.into(),
        status: status.into(),
        detail: detail.into(),
    }
}
