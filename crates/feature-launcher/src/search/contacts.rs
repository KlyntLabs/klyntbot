use crate::types::*;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ContactEntry {
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
}

#[derive(Clone)]
pub struct ContactsSource {
    contacts: Arc<RwLock<Vec<ContactEntry>>>,
    /// Once set, we never attempt to load contacts again (denied or failed).
    gave_up: Arc<AtomicBool>,
    cache_path: Option<std::path::PathBuf>,
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
            gave_up: Arc::new(AtomicBool::new(false)),
            cache_path: None,
        }
    }

    pub fn with_cache_dir(cache_dir: std::path::PathBuf) -> Self {
        let cache_path = cache_dir.join("contacts-cache.json");
        Self {
            contacts: Arc::new(RwLock::new(Vec::new())),
            gave_up: Arc::new(AtomicBool::new(false)),
            cache_path: Some(cache_path),
        }
    }

    /// Try loading contacts from disk cache. Returns None if cache is stale (>10 min).
    fn try_load_cache(&self) -> Option<Vec<ContactEntry>> {
        let path = self.cache_path.as_ref()?;
        let metadata = std::fs::metadata(path).ok()?;
        let age = metadata.modified().ok()?.elapsed().ok()?;
        if age > std::time::Duration::from_secs(600) {
            return None;
        }
        let data = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&data).ok()
    }

    fn write_cache(&self, contacts: &[ContactEntry]) {
        if let Some(path) = &self.cache_path {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string(contacts) {
                let _ = std::fs::write(path, json);
            }
        }
    }

    /// Swift script that checks CNContactStore authorization and fetches contacts
    /// via the Contacts framework — never opens the Contacts.app GUI.
    ///
    /// Authorization status values:
    ///   0 = notDetermined, 1 = restricted, 2 = denied, 3 = authorized
    ///
    /// If not authorized, prints "DENIED" to stderr and exits 1.
    /// If authorized, prints JSON array to stdout.
    #[cfg(target_os = "macos")]
    const SWIFT_FETCH: &'static str = r#"
import Contacts
import Foundation

let status = CNContactStore.authorizationStatus(for: .contacts)
guard status == .authorized else {
    fputs("DENIED:\(status.rawValue)", stderr)
    exit(1)
}
let store = CNContactStore()
let keys: [CNKeyDescriptor] = [
    CNContactGivenNameKey as CNKeyDescriptor,
    CNContactFamilyNameKey as CNKeyDescriptor,
    CNContactEmailAddressesKey as CNKeyDescriptor,
    CNContactPhoneNumbersKey as CNKeyDescriptor,
]
let req = CNContactFetchRequest(keysToFetch: keys)
req.sortOrder = .givenName
var results: [[String: Any]] = []
var count = 0
try? store.enumerateContacts(with: req) { contact, stop in
    let name = [contact.givenName, contact.familyName]
        .filter { !$0.isEmpty }.joined(separator: " ")
    guard !name.isEmpty else { return }
    let email: Any = (contact.emailAddresses.first?.value as String?) ?? NSNull()
    let phone: Any = contact.phoneNumbers.first?.value.stringValue ?? NSNull()
    results.append(["name": name, "email": email, "phone": phone])
    count += 1
    if count >= 500 { stop.pointee = true }
}
let data = try! JSONSerialization.data(withJSONObject: results)
print(String(data: data, encoding: .utf8)!)
"#;

    #[cfg(target_os = "macos")]
    async fn load_contacts(&self) -> Vec<ContactEntry> {
        if self.gave_up.load(Ordering::Relaxed) {
            return vec![];
        }

        let output = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            tokio::process::Command::new("swift")
                .args(["-e", Self::SWIFT_FETCH])
                .output(),
        )
        .await;

        let output = match output {
            Ok(Ok(o)) if o.status.success() => o,
            Ok(Ok(o)) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                if stderr.contains("DENIED") {
                    tracing::warn!(
                        "Contacts access not authorized ({}). \
                         Grant in System Settings > Privacy > Contacts. \
                         Disabling contacts source for this session.",
                        stderr.trim()
                    );
                } else {
                    tracing::warn!("Contacts fetch failed: {}", stderr.trim());
                }
                self.gave_up.store(true, Ordering::Relaxed);
                return vec![];
            }
            Ok(Err(e)) => {
                tracing::warn!("Failed to run swift for contacts: {e}");
                self.gave_up.store(true, Ordering::Relaxed);
                return vec![];
            }
            Err(_) => {
                tracing::warn!("Contacts fetch timed out");
                self.gave_up.store(true, Ordering::Relaxed);
                return vec![];
            }
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
        // Try disk cache first (avoids Swift subprocess)
        if let Some(cached) = self.try_load_cache() {
            let count = cached.len();
            *self.contacts.write() = cached;
            tracing::debug!("Loaded {count} contacts from disk cache");
            return;
        }

        let contacts = self.load_contacts().await;
        if !contacts.is_empty() {
            self.write_cache(&contacts);
        }
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
