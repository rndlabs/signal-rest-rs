use std::path::Path;
use std::{path::PathBuf, time::UNIX_EPOCH};

use std::time::Duration;
use anyhow::Context;
use chrono::Local;
use futures::{pin_mut, StreamExt};
use notify_rust::Notification;
use presage::prelude::content::Reaction;
use presage::prelude::proto::data_message::Quote;
use presage::prelude::proto::sync_message::Sent;
use presage::{Store, Thread};
use presage::prelude::{Content, SyncMessage};
use presage::{Registered, Manager, prelude::{ContentBody, DataMessage, Uuid}};
use presage_store_sled::{SledStore, MigrationConflictStrategy};
use tempfile::Builder;
use tokio::fs;
use tokio::{sync::mpsc, task, time::sleep};
use tracing::{error, info, warn};

pub type Queue = mpsc::UnboundedSender<(String, String)>;
pub type QueueReceiver = mpsc::UnboundedReceiver<(String, String)>;

pub struct SignalServiceWrapper {
    queue: QueueReceiver,
    db_path: PathBuf,
    passphrase: Option<String>,
    // Put other persistent data here
}

impl SignalServiceWrapper {
    pub fn new (db_path: PathBuf, passphrase: Option<String>, queue: QueueReceiver) -> Self {
        // Initialize members here
        Self { queue, db_path, passphrase }
    }

    pub async fn run(mut self) {
        while let Some(req) = self.queue.recv().await {
            self.process(req).await;
        }
    }

    async fn process(&mut self, req: (String, String)) {
        // Spawn and run a new task here that will process the request.
        // Use a new runtime for this task, and make sure it all runs in the same thread
        // despite any await

        let config_store = SledStore::open_with_passphrase(
            self.db_path.clone(),
            self.passphrase.clone(),
            MigrationConflictStrategy::Raise,
        ).expect("failed to open config database");
        let mut manager = Manager::load_registered(config_store).await.unwrap();

        let destination = Uuid::parse_str(req.0.as_str()).unwrap();

        let timestamp = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as u64;

        let message = ContentBody::DataMessage(DataMessage {
            body: Some(req.1),
            timestamp: Some(timestamp),
            ..Default::default()
        });

        let local = tokio::task::LocalSet::new();
        local.run_until(async move {
            let mut receiving_manager = manager.clone();
            task::spawn_local(async move {
                if let Err(e) = Self::receive(&mut receiving_manager, false).await {
                    error!("error while receiving stuff: {e}");
                }
            });

            sleep(Duration::from_secs(4)).await;

            manager.send_message(destination, message, timestamp).await.unwrap();
        }).await;

    }

    async fn receive<C: Store>(
        manager: &mut Manager<C, Registered>,
        notifications: bool,
    ) -> anyhow::Result<()> {
        let attachments_tmp_dir = Builder::new().prefix("presage-attachments").tempdir()?;
        info!(
            "attachments will be stored in {}",
            attachments_tmp_dir.path().display()
        );
    
        let messages = manager
            .receive_messages()
            .await
            .context("failed to initialize messages stream")?;
        pin_mut!(messages);
    
        while let Some(content) = messages.next().await {
            Self::process_incoming_message(manager, attachments_tmp_dir.path(), notifications, &content)
                .await;
        }
    
        Ok(())
    }

    // Note to developers, this is a good example of a function you can use as a source of inspiration
    // to process incoming messages.
    async fn process_incoming_message<C: Store>(
        manager: &mut Manager<C, Registered>,
        attachments_tmp_dir: &Path,
        notifications: bool,
        content: &Content,
    ) {
        Self::print_message(manager, notifications, content);

        let sender = content.metadata.sender.uuid;
        if let ContentBody::DataMessage(DataMessage { attachments, .. }) = &content.body {
            for attachment_pointer in attachments {
                let Ok(attachment_data) = manager.get_attachment(attachment_pointer).await else {
                    warn!("failed to fetch attachment");
                    continue;
                };

                let extensions = mime_guess::get_mime_extensions_str(
                    attachment_pointer
                        .content_type
                        .as_deref()
                        .unwrap_or("application/octet-stream"),
                );
                let extension = extensions.and_then(|e| e.first()).unwrap_or(&"bin");
                let filename = attachment_pointer
                    .file_name
                    .clone()
                    .unwrap_or_else(|| Local::now().format("%Y-%m-%d-%H-%M-%s").to_string());
                let file_path = attachments_tmp_dir.join(format!("presage-{filename}.{extension}",));
                match fs::write(&file_path, &attachment_data).await {
                    Ok(_) => info!("saved attachment from {sender} to {}", file_path.display()),
                    Err(error) => error!(
                        "failed to write attachment from {sender} to {}: {error}",
                        file_path.display()
                    ),
                }
            }
        }
    }

    fn print_message<C: Store>(
        manager: &Manager<C, Registered>,
        notifications: bool,
        content: &Content,
    ) {
        let Ok(thread) = Thread::try_from(content) else {
            warn!("failed to derive thread from content");
            return;
        };
    
        let format_data_message = |thread: &Thread, data_message: &DataMessage| match data_message {
            DataMessage {
                quote:
                    Some(Quote {
                        text: Some(quoted_text),
                        ..
                    }),
                body: Some(body),
                ..
            } => Some(format!("Answer to message \"{quoted_text}\": {body}")),
            DataMessage {
                reaction:
                    Some(Reaction {
                        target_sent_timestamp: Some(timestamp),
                        emoji: Some(emoji),
                        ..
                    }),
                ..
            } => {
                let Ok(Some(message)) = manager.message(thread, *timestamp) else {
                    warn!("no message in {thread} sent at {timestamp}");
                    return None;
                };
    
                let ContentBody::DataMessage(DataMessage { body: Some(body), .. }) = message.body else {
                    warn!("message reacted to has no body");
                    return None;
                };
    
                Some(format!("Reacted with {emoji} to message: \"{body}\""))
            }
            DataMessage {
                body: Some(body), ..
            } => Some(body.to_string()),
            _ => Some("Empty data message".to_string()),
        };
    
        let format_contact = |uuid| {
            manager
                .contact_by_id(uuid)
                .ok()
                .flatten()
                .filter(|c| !c.name.is_empty())
                .map(|c| format!("{}: {}", c.name, uuid))
                .unwrap_or_else(|| uuid.to_string())
        };
    
        let format_group = |key| {
            manager
                .group(key)
                .ok()
                .flatten()
                .map(|g| g.title)
                .unwrap_or_else(|| "<missing group>".to_string())
        };
    
        enum Msg<'a> {
            Received(&'a Thread, String),
            Sent(&'a Thread, String),
        }
    
        if let Some(msg) = match &content.body {
            ContentBody::NullMessage(_) => Some(Msg::Received(
                &thread,
                "Null message (for example deleted)".to_string(),
            )),
            ContentBody::DataMessage(data_message) => {
                format_data_message(&thread, data_message).map(|body| Msg::Received(&thread, body))
            }
            ContentBody::SynchronizeMessage(SyncMessage {
                sent:
                    Some(Sent {
                        message: Some(data_message),
                        ..
                    }),
                ..
            }) => format_data_message(&thread, data_message).map(|body| Msg::Sent(&thread, body)),
            ContentBody::CallMessage(_) => Some(Msg::Received(&thread, "is calling!".into())),
            ContentBody::TypingMessage(_) => Some(Msg::Received(&thread, "is typing...".into())),
            c => {
                warn!("unsupported message {c:?}");
                None
            }
        } {
            let ts = content.metadata.timestamp;
            let (prefix, body) = match msg {
                Msg::Received(Thread::Contact(sender), body) => {
                    let contact = format_contact(sender);
                    (format!("From {contact} @ {ts}: "), body)
                }
                Msg::Sent(Thread::Contact(recipient), body) => {
                    let contact = format_contact(recipient);
                    (format!("To {contact} @ {ts}"), body)
                }
                Msg::Received(Thread::Group(key), body) => {
                    let sender = format_contact(&content.metadata.sender.uuid);
                    let group = format_group(key);
                    (format!("From {sender} to group {group} @ {ts}: "), body)
                }
                Msg::Sent(Thread::Group(key), body) => {
                    let group = format_group(key);
                    (format!("To group {group} @ {ts}"), body)
                }
            };
    
            println!("{prefix} / {body}");
    
            if notifications {
                if let Err(e) = Notification::new()
                    .summary(&prefix)
                    .body(&body)
                    .icon("presage")
                    .show()
                {
                    error!("failed to display desktop notification: {e}");
                }
            }
        }
    }
}
