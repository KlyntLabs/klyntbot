use approval::{
    ApprovalCapabilities, ApprovalChannel, ApprovalClass, ApprovalDecision, ApprovalRequest,
};
use dashmap::DashMap;
use std::collections::HashSet;
use std::sync::Arc;
use tauri::Emitter;
use tokio::sync::oneshot;
use uuid::Uuid;

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ApprovalRequestEvent {
    id: String,
    tool_name: String,
    action: Option<String>,
    class: ApprovalClass,
}

pub struct DesktopApprovalChannel {
    app: tauri::AppHandle,
    pending: Arc<DashMap<String, oneshot::Sender<ApprovalDecision>>>,
}

impl DesktopApprovalChannel {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self {
            app,
            pending: Arc::new(DashMap::new()),
        }
    }

    pub fn respond(&self, approval_id: &str, decision: ApprovalDecision) -> bool {
        if let Some((_, sender)) = self.pending.remove(approval_id) {
            sender.send(decision).is_ok()
        } else {
            false
        }
    }
}

#[async_trait::async_trait]
impl ApprovalChannel for DesktopApprovalChannel {
    async fn request(&self, req: ApprovalRequest) -> ApprovalDecision {
        let id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();

        self.pending.insert(id.clone(), tx);

        let event = ApprovalRequestEvent {
            id: id.clone(),
            tool_name: req.tool_name.clone(),
            action: req.action.clone(),
            class: req.class,
        };
        let _ = self.app.emit("approval:request", event);

        match tokio::time::timeout(std::time::Duration::from_secs(600), rx).await {
            Ok(Ok(decision)) => decision,
            Ok(Err(_)) => {
                self.pending.remove(&id);
                ApprovalDecision::Decline {
                    reason: "Channel closed".into(),
                }
            }
            Err(_) => {
                self.pending.remove(&id);
                ApprovalDecision::Decline {
                    reason: "Approval timed out".into(),
                }
            }
        }
    }

    fn capabilities(&self) -> ApprovalCapabilities {
        ApprovalCapabilities {
            supports_inline: true,
            supports_classes: HashSet::from([
                ApprovalClass::Safe,
                ApprovalClass::Sensitive,
                ApprovalClass::Destructive,
                ApprovalClass::Admin,
            ]),
        }
    }
}
