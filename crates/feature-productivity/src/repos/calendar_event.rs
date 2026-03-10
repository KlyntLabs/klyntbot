use sqlx::SqlitePool;

use crate::types::CalendarEvent;

#[derive(Debug, Clone)]
pub struct CalendarEventRepo {
    pool: SqlitePool,
}

impl CalendarEventRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Upsert a calendar event by external_uid (idempotent sync).
    pub async fn upsert(&self, event: &CalendarEvent) -> common::Result<()> {
        sqlx::query(
            r#"INSERT INTO calendar_events
                (id, calendar_id, title, description, started_at, ended_at,
                 location, attendees_count, is_recurring, recurrence_id,
                 source, external_uid, session_id, color, synced_at, created_at, updated_at)
            VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)
            ON CONFLICT(external_uid) DO UPDATE SET
                title = excluded.title,
                description = excluded.description,
                started_at = excluded.started_at,
                ended_at = excluded.ended_at,
                location = excluded.location,
                attendees_count = excluded.attendees_count,
                is_recurring = excluded.is_recurring,
                recurrence_id = excluded.recurrence_id,
                session_id = excluded.session_id,
                color = excluded.color,
                synced_at = excluded.synced_at,
                updated_at = excluded.updated_at"#,
        )
        .bind(&event.id)
        .bind(&event.calendar_id)
        .bind(&event.title)
        .bind(&event.description)
        .bind(&event.started_at)
        .bind(&event.ended_at)
        .bind(&event.location)
        .bind(event.attendees_count)
        .bind(event.is_recurring)
        .bind(&event.recurrence_id)
        .bind(&event.source)
        .bind(&event.external_uid)
        .bind(&event.session_id)
        .bind(&event.color)
        .bind(&event.synced_at)
        .bind(&event.created_at)
        .bind(&event.updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    /// List calendar events within a time range.
    pub async fn list_range(
        &self,
        from: &str,
        to: &str,
    ) -> common::Result<Vec<CalendarEvent>> {
        sqlx::query_as::<_, CalendarEvent>(
            "SELECT * FROM calendar_events WHERE started_at >= ?1 AND started_at < ?2 ORDER BY started_at",
        )
        .bind(from)
        .bind(to)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))
    }

    /// List calendar events for a specific date (YYYY-MM-DD prefix match).
    pub async fn list_for_date(&self, date: &str) -> common::Result<Vec<CalendarEvent>> {
        let from = format!("{date}T00:00:00Z");
        let to = format!("{date}T23:59:59Z");
        self.list_range(&from, &to).await
    }

    /// Delete by external UID (for sync removals).
    pub async fn delete_by_external_uid(&self, external_uid: &str) -> common::Result<()> {
        sqlx::query("DELETE FROM calendar_events WHERE external_uid = ?1")
            .bind(external_uid)
            .execute(&self.pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Get calendar event linked to a specific session.
    pub async fn get_by_session_id(
        &self,
        session_id: &str,
    ) -> common::Result<Option<CalendarEvent>> {
        sqlx::query_as::<_, CalendarEvent>(
            "SELECT * FROM calendar_events WHERE session_id = ?1",
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))
    }
}
