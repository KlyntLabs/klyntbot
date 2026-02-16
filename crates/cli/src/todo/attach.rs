//! Task attachment commands (attach and detach).

use anyhow::Result;
use chrono::Utc;
use common::utils::terminal::*;
use tools::todo_store::TodoStore;
use tools::todo_types::{Attachment, AttachmentType, Todo};

/// Handle the `todo attach` command.
///
/// Adds an attachment to a task. Exactly one of --file, --url, or --note must be provided.
pub async fn handle_attach(
    id: String,
    file: Option<String>,
    url: Option<String>,
    note: Option<String>,
    title: Option<String>,
) -> Result<()> {
    // Validate exactly one attachment type
    let provided = [file.is_some(), url.is_some(), note.is_some()]
        .iter()
        .filter(|&&v| v)
        .count();

    if provided == 0 {
        println!(
            "{} Must provide one of --file, --url, or --note",
            status_error()
        );
        return Ok(());
    }
    if provided > 1 {
        println!(
            "{} Only one of --file, --url, or --note may be provided",
            status_error()
        );
        return Ok(());
    }

    let (attachment_type, value) = if let Some(f) = file {
        (AttachmentType::File, f)
    } else if let Some(u) = url {
        (AttachmentType::Url, u)
    } else {
        // note must be Some() because we validated exactly one of file/url/note is provided
        (
            AttachmentType::Note,
            note.ok_or_else(|| anyhow::anyhow!("Note value is required"))?,
        )
    };

    let config = config::load().await?;
    let store_path = config.todo_store_path();
    let mut store = TodoStore::new(store_path);

    let type_label = match attachment_type {
        AttachmentType::File => "file",
        AttachmentType::Url => "url",
        AttachmentType::Note => "note",
    };

    let attachment = Attachment {
        id: Todo::generate_id(),
        attachment_type,
        title,
        value: value.clone(),
        tags: Vec::new(),
        created_at: Utc::now(),
    };
    let attachment_id = attachment.id.clone();

    if store.add_attachment(&id, attachment).await? {
        println!(
            "{} Attached {} to task {}: {} (ID: {})",
            status_success(),
            colorize(type_label, TOOL),
            colorize(&id, TOOL),
            value,
            colorize(&attachment_id, DIM),
        );
    } else {
        println!("{} Task not found: {}", status_error(), id);
    }

    Ok(())
}

/// Handle the `todo detach` command.
///
/// Removes an attachment from a task by attachment ID.
pub async fn handle_detach(id: &str, attachment_id: &str) -> Result<()> {
    let config = config::load().await?;
    let store_path = config.todo_store_path();
    let mut store = TodoStore::new(store_path);

    if store.remove_attachment(id, attachment_id).await? {
        println!(
            "{} Removed attachment {} from task {}",
            status_success(),
            colorize(attachment_id, TOOL),
            colorize(id, TOOL),
        );
    } else {
        println!(
            "{} Task or attachment not found: task={}, attachment={}",
            status_error(),
            id,
            attachment_id
        );
    }

    Ok(())
}
