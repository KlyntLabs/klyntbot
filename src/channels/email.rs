//! Email channel using IMAP polling for inbound and SMTP for outbound.

use async_trait::async_trait;
use futures_util::StreamExt;
use html2text::from_read;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message as LettreMessage, SmtpTransport, Transport};
use mail_parser::MessageParser;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

use crate::bus::{InboundMessage, MessageBus, OutboundMessage};
use crate::channels::{check_allowlist, Channel};
use crate::config::EmailConfig;
use crate::error::{ChannelError, Result};

/// Email channel implementation
pub struct EmailChannel {
    config: EmailConfig,
    last_subject_by_chat: Arc<RwLock<HashMap<String, String>>>,
    last_message_id_by_chat: Arc<RwLock<HashMap<String, String>>>,
    processed_uids: Arc<RwLock<HashSet<String>>>,
    running: Arc<AtomicBool>,
}

impl EmailChannel {
    /// Create a new Email channel
    pub fn new(config: EmailConfig) -> Result<Self> {
        Ok(Self {
            config,
            last_subject_by_chat: Arc::new(RwLock::new(HashMap::new())),
            last_message_id_by_chat: Arc::new(RwLock::new(HashMap::new())),
            processed_uids: Arc::new(RwLock::new(HashSet::new())),
            running: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Validate configuration
    fn validate_config(&self) -> Result<()> {
        // Check consent first
        if !self.config.consent_granted {
            return Err(ChannelError::ConnectionFailed(
                "Email channel not configured: Email access requires explicit consent. \
                 Set channels.email.consentGranted=true in config after reviewing privacy implications. \
                 Email channel will read your mailbox and requires IMAP/SMTP credentials.".to_string(),
            )
            .into());
        }

        let mut missing = Vec::new();

        if self.config.imap_host.is_empty() {
            missing.push("imap_host");
        }
        if self.config.imap_username.is_empty() {
            missing.push("imap_username");
        }
        if self.config.imap_password.is_empty() {
            missing.push("imap_password");
        }
        if self.config.smtp_host.is_empty() {
            missing.push("smtp_host");
        }
        if self.config.smtp_username.is_empty() {
            missing.push("smtp_username");
        }
        if self.config.smtp_password.is_empty() {
            missing.push("smtp_password");
        }

        if !missing.is_empty() {
            return Err(ChannelError::ConnectionFailed(format!(
                "Email channel not configured. Missing fields: {}. \
                 Configure these using 'klyntbot channels login email' for setup instructions.",
                missing.join(", ")
            ))
            .into());
        }

        Ok(())
    }

    /// Poll IMAP for new messages
    async fn poll_imap(&self, bus: &MessageBus) -> Result<()> {
        use tokio::net::TcpStream;

        if self.config.imap_use_ssl {
            // TLS connection path
            use tokio_native_tls::TlsConnector;

            let tcp_stream =
                TcpStream::connect((&self.config.imap_host[..], self.config.imap_port))
                    .await
                    .map_err(|e| {
                        ChannelError::ConnectionFailed(format!("TCP connection failed: {}", e))
                    })?;

            let tls_connector = native_tls::TlsConnector::builder()
                .build()
                .map_err(|e| ChannelError::ConnectionFailed(format!("TLS setup failed: {}", e)))?;
            let tls_connector = TlsConnector::from(tls_connector);

            let tls_stream = tls_connector
                .connect(&self.config.imap_host, tcp_stream)
                .await
                .map_err(|e| {
                    ChannelError::ConnectionFailed(format!("TLS handshake failed: {}", e))
                })?;

            let client = async_imap::Client::new(tls_stream);

            let mut session = client
                .login(&self.config.imap_username, self.config.imap_password.expose())
                .await
                .map_err(|(e, _)| {
                    ChannelError::ConnectionFailed(format!("IMAP login failed: {}", e))
                })?;

            self.process_mailbox(&mut session, bus).await?;

            let _ = session.logout().await;
        } else {
            // Plain TCP connection path
            let tcp_stream =
                TcpStream::connect((&self.config.imap_host[..], self.config.imap_port))
                    .await
                    .map_err(|e| {
                        ChannelError::ConnectionFailed(format!("TCP connection failed: {}", e))
                    })?;

            let client = async_imap::Client::new(tcp_stream);

            let mut session = client
                .login(&self.config.imap_username, self.config.imap_password.expose())
                .await
                .map_err(|(e, _)| {
                    ChannelError::ConnectionFailed(format!("IMAP login failed: {}", e))
                })?;

            self.process_mailbox(&mut session, bus).await?;

            let _ = session.logout().await;
        }

        Ok(())
    }

    /// Process mailbox messages (shared logic for SSL and non-SSL connections)
    async fn process_mailbox<T>(
        &self,
        session: &mut async_imap::Session<T>,
        bus: &MessageBus,
    ) -> Result<()>
    where
        T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + std::fmt::Debug,
    {
        // Select mailbox
        session
            .select(&self.config.imap_mailbox)
            .await
            .map_err(|e| ChannelError::SendFailed(format!("Failed to select mailbox: {}", e)))?;

        // Search for unseen messages
        let unseen = session
            .search("UNSEEN")
            .await
            .map_err(|e| ChannelError::SendFailed(format!("Search failed: {}", e)))?;

        debug!("Found {} unseen email(s)", unseen.len());

        for seq_num in unseen.iter() {
            // Fetch message - returns a Stream
            let mut messages = session
                .fetch(format!("{}", seq_num), "(UID BODY.PEEK[])")
                .await
                .map_err(|e| ChannelError::SendFailed(format!("Fetch failed: {}", e)))?;

            // Get the first message from the stream
            let fetch_opt = messages.next().await.and_then(|r| r.ok());

            // Drop the stream to release the mutable borrow on session
            drop(messages);

            // Process the fetched message if we got one
            if let Some(fetch) = fetch_opt {
                let uid = fetch.uid.map(|u: u32| u.to_string()).unwrap_or_default();

                // Check if already processed
                {
                    let processed = self.processed_uids.read().await;
                    if processed.contains(&uid) {
                        continue;
                    }
                }

                // Parse message
                if let Some(body) = fetch.body() {
                    if let Err(e) = self.process_email_body(body, &uid, bus).await {
                        error!("Failed to process email: {}", e);
                    }

                    // Mark as processed
                    {
                        let mut processed = self.processed_uids.write().await;
                        processed.insert(uid.clone());
                        // Limit set size
                        if processed.len() > 10000 {
                            processed.clear();
                        }
                    }

                    // Mark as seen if configured
                    if self.config.mark_seen {
                        let _ = session
                            .store(format!("{}", seq_num), "+FLAGS (\\Seen)")
                            .await;
                    }
                }
            }
        }

        Ok(())
    }

    /// Process email body bytes
    async fn process_email_body(&self, body: &[u8], uid: &str, bus: &MessageBus) -> Result<()> {
        let parser = MessageParser::default();
        let message = parser.parse(body);

        if message.is_none() {
            return Ok(());
        }
        let message = message.unwrap();

        // Extract sender
        let from = message
            .from()
            .and_then(|f| f.first())
            .and_then(|addr| addr.address())
            .unwrap_or("");

        if from.is_empty() {
            return Ok(());
        }

        let sender = from.to_lowercase();

        // Check allowlist
        if !check_allowlist(&self.config.allow_from, &sender) {
            warn!("Access denied for sender {} on Email", sender);
            return Ok(());
        }

        // Extract subject
        let subject = message.subject().unwrap_or("").to_string();

        // Extract message ID
        let message_id = message
            .message_id()
            .map(|id| id.to_string())
            .unwrap_or_default();

        // Extract date
        let date = message.date().map(|d| d.to_rfc3339()).unwrap_or_default();

        // Extract body text
        let mut body_text = String::new();

        // Try text/plain first
        if let Some(text_body) = message.body_text(0) {
            body_text = text_body.to_string();
        } else if let Some(html_body) = message.body_html(0) {
            // Convert HTML to text
            body_text = from_read(html_body.as_bytes(), 80)
                .unwrap_or_else(|_| "(failed to parse HTML)".to_string());
        }

        if body_text.is_empty() {
            body_text = "(empty email body)".to_string();
        }

        // Truncate if needed
        let max_chars = self.config.max_body_chars as usize;
        if body_text.len() > max_chars {
            body_text.truncate(max_chars);
            body_text.push_str("\n[truncated]");
        }

        // Format content
        let content = format!(
            "Email received.\nFrom: {}\nSubject: {}\nDate: {}\n\n{}",
            sender, subject, date, body_text
        );

        debug!("Email from {}: {}", sender, subject);

        // Store subject and message ID for replies
        {
            let mut last_subject = self.last_subject_by_chat.write().await;
            last_subject.insert(sender.clone(), subject.clone());
        }
        {
            let mut last_msg_id = self.last_message_id_by_chat.write().await;
            last_msg_id.insert(sender.clone(), message_id.clone());
        }

        // Publish to bus
        let mut inbound = InboundMessage::new("email", sender.as_str(), sender.as_str(), &content);
        inbound.metadata.insert(
            "email".to_string(),
            serde_json::json!({
                "message_id": message_id,
                "subject": subject,
                "date": date,
                "sender_email": sender,
                "uid": uid,
            }),
        );

        bus.publish_inbound(inbound)
            .await
            .map_err(|e| ChannelError::SendFailed(format!("Failed to publish to bus: {}", e)))?;

        Ok(())
    }

    /// Send email via SMTP
    async fn send_email(&self, to: &str, content: &str, thread_msg_id: Option<&str>) -> Result<()> {
        // Get reply subject
        let base_subject = {
            let subjects = self.last_subject_by_chat.read().await;
            subjects
                .get(to)
                .cloned()
                .unwrap_or_else(|| "nanobot reply".to_string())
        };

        let subject = self.reply_subject(&base_subject);

        // Get from address
        let from_addr = if !self.config.from_address.is_empty() {
            &self.config.from_address
        } else if !self.config.smtp_username.is_empty() {
            &self.config.smtp_username
        } else {
            &self.config.imap_username
        };

        // Build message
        let mut email_builder =
            LettreMessage::builder()
                .from(from_addr.parse().map_err(|e| {
                    ChannelError::SendFailed(format!("Invalid from address: {}", e))
                })?)
                .to(to
                    .parse()
                    .map_err(|e| ChannelError::SendFailed(format!("Invalid to address: {}", e)))?)
                .subject(subject);

        // Add In-Reply-To and References headers for threading
        if let Some(msg_id) = thread_msg_id {
            email_builder = email_builder
                .in_reply_to(msg_id.to_string())
                .references(msg_id.to_string());
        }

        let email = email_builder
            .body(content.to_string())
            .map_err(|e| ChannelError::SendFailed(format!("Failed to build email: {}", e)))?;

        // Send via SMTP
        let creds = Credentials::new(
            self.config.smtp_username.clone(),
            self.config.smtp_password.expose().clone(),
        );

        let mailer = SmtpTransport::relay(&self.config.smtp_host)
            .map_err(|e| ChannelError::SendFailed(format!("SMTP relay failed: {}", e)))?
            .credentials(creds)
            .port(self.config.smtp_port)
            .build();

        tokio::task::spawn_blocking(move || mailer.send(&email))
            .await
            .map_err(|e| ChannelError::SendFailed(format!("SMTP send task failed: {}", e)))?
            .map_err(|e| ChannelError::SendFailed(format!("SMTP send failed: {}", e)))?;

        info!("Sent email to {}", to);

        Ok(())
    }

    /// Generate reply subject with configured prefix
    fn reply_subject(&self, base_subject: &str) -> String {
        let subject = if base_subject.is_empty() {
            "nanobot reply"
        } else {
            base_subject
        };

        let prefix = if self.config.subject_prefix.is_empty() {
            "Re: "
        } else {
            &self.config.subject_prefix
        };

        if subject.to_lowercase().starts_with("re:") {
            subject.to_string()
        } else {
            format!("{}{}", prefix, subject)
        }
    }
}

#[async_trait]
impl Channel for EmailChannel {
    fn name(&self) -> &str {
        "email"
    }

    async fn start(&self, bus: Arc<MessageBus>) -> Result<()> {
        self.validate_config()?;

        self.running.store(true, Ordering::SeqCst);

        info!("Starting Email channel (IMAP polling mode)...");

        let poll_interval = Duration::from_secs(self.config.poll_interval_seconds.max(5) as u64);

        while self.running.load(Ordering::SeqCst) {
            if let Err(e) = self.poll_imap(&bus).await {
                error!("Email polling error: {}", e);
            }

            sleep(poll_interval).await;
        }

        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn send(&self, msg: &OutboundMessage) -> Result<()> {
        if !self.config.auto_reply_enabled {
            info!("Skip automatic email reply: auto_reply_enabled is false");
            return Ok(());
        }

        let to = msg.chat_id.as_str().trim();
        if to.is_empty() {
            return Err(ChannelError::SendFailed("Empty recipient address".to_string()).into());
        }

        // Get message_id for threading
        let thread_msg_id = {
            let last_ids = self.last_message_id_by_chat.read().await;
            last_ids.get(to).cloned()
        };

        self.send_email(to, &msg.content, thread_msg_id.as_deref())
            .await
    }

    fn is_allowed(&self, sender_id: &str) -> bool {
        check_allowlist(&self.config.allow_from, sender_id)
    }
}
