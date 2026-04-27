use crate::types::*;
use parking_lot::RwLock;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Clone)]
struct SshEntry {
    host: String,
    user: Option<String>,
    hostname: Option<String>,
}

#[derive(Clone)]
pub struct SshHostsSource {
    hosts: Arc<RwLock<Vec<SshEntry>>>,
}

impl Default for SshHostsSource {
    fn default() -> Self {
        Self::new()
    }
}

impl SshHostsSource {
    pub fn new() -> Self {
        Self {
            hosts: Arc::new(RwLock::new(Vec::new())),
        }
    }

    fn parse_ssh_config(path: &Path) -> Vec<SshEntry> {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return vec![],
        };

        let mut entries = Vec::new();
        let mut current_host: Option<String> = None;
        let mut current_user: Option<String> = None;
        let mut current_hostname: Option<String> = None;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = line.splitn(2, char::is_whitespace).collect();
            if parts.len() < 2 {
                continue;
            }
            let key = parts[0].to_lowercase();
            let value = parts[1].trim();

            if key == "host" {
                // Save previous entry
                if let Some(host) = current_host.take() {
                    if host != "*" && !host.contains('*') && !host.contains('?') {
                        entries.push(SshEntry {
                            host,
                            user: current_user.take(),
                            hostname: current_hostname.take(),
                        });
                    }
                }
                current_host = Some(value.to_string());
                current_user = None;
                current_hostname = None;
            } else if key == "user" {
                current_user = Some(value.to_string());
            } else if key == "hostname" {
                current_hostname = Some(value.to_string());
            }
        }

        // Save last entry
        if let Some(host) = current_host {
            if host != "*" && !host.contains('*') && !host.contains('?') {
                entries.push(SshEntry {
                    host,
                    user: current_user,
                    hostname: current_hostname,
                });
            }
        }

        entries
    }
}

#[async_trait::async_trait]
impl super::SearchSource for SshHostsSource {
    fn name(&self) -> &'static str {
        "ssh_hosts"
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<LauncherItem> {
        let hosts = self.hosts.read();
        let scored =
            super::fuzzy_match2(query, &hosts, |h| &h.host, |h| h.hostname.as_deref(), limit);

        scored
            .into_iter()
            .map(|(score, h)| {
                let subtitle = match (&h.user, &h.hostname) {
                    (Some(u), Some(hn)) => format!("{u}@{hn}"),
                    (None, Some(hn)) => hn.clone(),
                    (Some(u), None) => format!("{u}@{}", h.host),
                    (None, None) => h.host.clone(),
                };
                LauncherItem {
                    id: format!("ssh:{}", h.host),
                    title: h.host.clone(),
                    subtitle: Some(subtitle),
                    icon: Some("terminal".to_string()),
                    kind: LauncherItemKind::SshHost {
                        host: h.host.clone(),
                        user: h.user.clone(),
                    },
                    score: (score as f64) / 1000.0 * 0.6,
                    no_view: false,
                    arguments: vec![],
                                    pinned: false,
                    }
            })
            .collect()
    }

    async fn refresh(&self) {
        let home = std::env::var("HOME").unwrap_or_default();
        let ssh_config = Path::new(&home).join(".ssh/config");
        let hosts = Self::parse_ssh_config(&ssh_config);
        tracing::info!("Indexed {} SSH hosts", hosts.len());
        *self.hosts.write() = hosts;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_parse_ssh_config() {
        let dir = tempfile::TempDir::new().unwrap();
        let config_path = dir.path().join("config");
        let mut f = std::fs::File::create(&config_path).unwrap();
        writeln!(
            f,
            "Host production\n  HostName prod.example.com\n  User deploy\n\nHost staging\n  HostName staging.example.com\n\nHost *\n  ServerAliveInterval 60"
        )
        .unwrap();

        let entries = SshHostsSource::parse_ssh_config(&config_path);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].host, "production");
        assert_eq!(entries[0].user.as_deref(), Some("deploy"));
        assert_eq!(entries[1].host, "staging");
        assert!(entries[1].user.is_none());
    }
}
