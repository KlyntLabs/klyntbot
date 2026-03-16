use crate::types::*;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ContactEntry {
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
}

#[derive(Clone)]
pub struct ContactsSource {
    contacts: Arc<RwLock<Vec<ContactEntry>>>,
    permission_warned: Arc<AtomicBool>,
}

impl Default for ContactsSource {
    fn default() -> Self {
        Self::new()
    }
}

impl ContactsSource {
    pub fn new() -> Self {
        Self {
            contacts: Arc::new(RwLock::new(Vec::new())),
            permission_warned: Arc::new(AtomicBool::new(false)),
        }
    }

    #[cfg(target_os = "macos")]
    const JXA_FETCH_ALL: &'static str = r#"
        var app = Application("Contacts");
        var people = app.people();
        var results = [];
        var limit = Math.min(people.length, 500);
        for (var i = 0; i < limit; i++) {
            var p = people[i];
            var emails = p.emails();
            var phones = p.phones();
            results.push({
                name: p.name(),
                email: emails.length > 0 ? emails[0].value() : null,
                phone: phones.length > 0 ? phones[0].value() : null,
            });
        }
        JSON.stringify(results);
    "#;

    #[cfg(target_os = "macos")]
    async fn load_contacts(&self) -> Vec<ContactEntry> {
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            tokio::process::Command::new("osascript")
                .args(["-l", "JavaScript", "-e", Self::JXA_FETCH_ALL])
                .output(),
        )
        .await;

        let output = match output {
            Ok(Ok(o)) if o.status.success() => o,
            Ok(Ok(o)) => {
                if !self.permission_warned.swap(true, Ordering::Relaxed) {
                    let stderr = String::from_utf8_lossy(&o.stderr);
                    if stderr.contains("not allowed") || stderr.contains("denied") {
                        tracing::warn!(
                            "Contacts access denied. Grant access in System Settings > Privacy > Contacts."
                        );
                    }
                }
                return vec![];
            }
            _ => return vec![],
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let contacts: Vec<serde_json::Value> =
            serde_json::from_str(stdout.trim()).unwrap_or_default();

        contacts
            .into_iter()
            .filter_map(|c| {
                let name = c.get("name")?.as_str()?.to_string();
                let email = c
                    .get("email")
                    .and_then(|e| e.as_str())
                    .map(|s| s.to_string());
                let phone = c
                    .get("phone")
                    .and_then(|p| p.as_str())
                    .map(|s| s.to_string());
                Some(ContactEntry { name, email, phone })
            })
            .collect()
    }

    #[cfg(not(target_os = "macos"))]
    async fn load_contacts(&self) -> Vec<ContactEntry> {
        vec![]
    }
}

#[async_trait::async_trait]
impl super::SearchSource for ContactsSource {
    fn name(&self) -> &'static str {
        "contacts"
    }

    fn prefix(&self) -> Option<char> {
        Some('@')
    }

    async fn refresh(&self) {
        let contacts = self.load_contacts().await;
        *self.contacts.write() = contacts;
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        let contacts = self.contacts.read();
        let scored = super::fuzzy_match(query, &contacts, |c| &c.name, limit);

        scored
            .into_iter()
            .map(|(score, c)| {
                let subtitle = c
                    .email
                    .as_deref()
                    .or(c.phone.as_deref())
                    .unwrap_or_default()
                    .to_string();
                LauncherItem {
                    id: format!("contact:{}", c.name),
                    title: c.name.clone(),
                    subtitle: Some(subtitle),
                    icon: Some("user".to_string()),
                    kind: LauncherItemKind::Contact {
                        name: c.name.clone(),
                        email: c.email.clone(),
                        phone: c.phone.clone(),
                    },
                    score: (score as f64) / 1000.0 * 0.6,
                }
            })
            .collect()
    }
}
