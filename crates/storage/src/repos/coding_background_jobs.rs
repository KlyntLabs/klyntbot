//! Repo for `coding_background_jobs` table.
//!
//! Spec: `docs/superpowers/specs/2026-05-08-coding-background-bash-design.md` §4.1.

use jiff::Timestamp;
use sqlx::SqlitePool;

use crate::error::StorageError;

#[derive(Debug, Clone, PartialEq)]
pub struct BashJobRow {
    pub id: String,
    pub session_id: String,
    pub agent_id: String,
    pub description: String,
    pub command: String,
    pub command_key: String,
    pub cwd: String,
    pub timeout_ms: i64,
    pub silent_completion: bool,
    pub status: String,
    pub exit_code: Option<i32>,
    pub failure_kind: Option<String>,
    pub failure_detail: Option<String>,
    pub failure_extracted: Option<String>,
    pub started_at: Timestamp,
    pub finished_at: Option<Timestamp>,
    pub total_bytes_emitted: i64,
    pub bisect_count: i64,
    pub log_path: String,
    pub final_path: Option<String>,
    pub last_polled_at: Option<Timestamp>,
    pub last_seen_offset: i64,
}

#[derive(Debug, Clone)]
pub struct BashJobRepo {
    pool: SqlitePool,
}

impl BashJobRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, row: &BashJobRow) -> Result<(), StorageError> {
        sqlx::query(
            r#"
            INSERT INTO coding_background_jobs (
                id, session_id, agent_id, description, command, command_key, cwd,
                timeout_ms, silent_completion, status, exit_code,
                failure_kind, failure_detail, failure_extracted,
                started_at, finished_at, total_bytes_emitted, bisect_count,
                log_path, final_path, last_polled_at, last_seen_offset
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&row.id)
        .bind(&row.session_id)
        .bind(&row.agent_id)
        .bind(&row.description)
        .bind(&row.command)
        .bind(&row.command_key)
        .bind(&row.cwd)
        .bind(row.timeout_ms)
        .bind(row.silent_completion as i64)
        .bind(&row.status)
        .bind(row.exit_code)
        .bind(&row.failure_kind)
        .bind(&row.failure_detail)
        .bind(&row.failure_extracted)
        .bind(row.started_at.to_string())
        .bind(row.finished_at.map(|t| t.to_string()))
        .bind(row.total_bytes_emitted)
        .bind(row.bisect_count)
        .bind(&row.log_path)
        .bind(&row.final_path)
        .bind(row.last_polled_at.map(|t| t.to_string()))
        .bind(row.last_seen_offset)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_status(
        &self,
        id: &str,
        status: &str,
        exit_code: Option<i32>,
        failure_kind: Option<&str>,
        failure_detail: Option<&str>,
        failure_extracted: Option<&str>,
        finished_at: Option<Timestamp>,
        final_path: Option<&str>,
        total_bytes_emitted: i64,
        bisect_count: i64,
    ) -> Result<(), StorageError> {
        sqlx::query(
            r#"
            UPDATE coding_background_jobs
            SET status = ?, exit_code = ?, failure_kind = ?, failure_detail = ?,
                failure_extracted = ?, finished_at = ?, final_path = ?,
                total_bytes_emitted = ?, bisect_count = ?
            WHERE id = ?
            "#,
        )
        .bind(status)
        .bind(exit_code)
        .bind(failure_kind)
        .bind(failure_detail)
        .bind(failure_extracted)
        .bind(finished_at.map(|t| t.to_string()))
        .bind(final_path)
        .bind(total_bytes_emitted)
        .bind(bisect_count)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_poll_cursor(
        &self,
        id: &str,
        last_polled_at: Timestamp,
        last_seen_offset: i64,
    ) -> Result<(), StorageError> {
        sqlx::query(
            r#"
            UPDATE coding_background_jobs
            SET last_polled_at = ?, last_seen_offset = MAX(last_seen_offset, ?)
            WHERE id = ?
            "#,
        )
        .bind(last_polled_at.to_string())
        .bind(last_seen_offset)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get(&self, id: &str) -> Result<Option<BashJobRow>, StorageError> {
        let row = sqlx::query(
            r#"SELECT id, session_id, agent_id, description, command, command_key, cwd,
                      timeout_ms, silent_completion, status, exit_code,
                      failure_kind, failure_detail, failure_extracted,
                      started_at, finished_at, total_bytes_emitted, bisect_count,
                      log_path, final_path, last_polled_at, last_seen_offset
               FROM coding_background_jobs WHERE id = ?"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        row.map(|r| Self::map_row(&r)).transpose()
    }

    /// Most recent terminal-state job in this session with the same command_key,
    /// excluding `exclude_id`. Returns None if no prior run exists. Excludes Lost
    /// status — Lost runs lack reliable final output to diff against.
    pub async fn find_prior_by_command_key(
        &self,
        session_id: &str,
        command_key: &str,
        exclude_id: &str,
    ) -> Result<Option<BashJobRow>, StorageError> {
        let row_opt = sqlx::query(
            r#"
            SELECT id, session_id, agent_id, description, command, command_key, cwd,
                   timeout_ms, silent_completion, status, exit_code,
                   failure_kind, failure_detail, failure_extracted,
                   started_at, finished_at, total_bytes_emitted, bisect_count,
                   log_path, final_path, last_polled_at, last_seen_offset
            FROM coding_background_jobs
            WHERE session_id = ?1
              AND command_key = ?2
              AND id != ?3
              AND status IN ('Completed','Failed','Cancelled')
            ORDER BY started_at DESC
            LIMIT 1
            "#,
        )
        .bind(session_id)
        .bind(command_key)
        .bind(exclude_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(StorageError::from)?;

        row_opt.as_ref().map(Self::map_row).transpose()
    }

    pub async fn list_for_session(
        &self,
        session_id: &str,
        agent_chain: &[String],
        active_only: bool,
    ) -> Result<Vec<BashJobRow>, StorageError> {
        if agent_chain.is_empty() {
            return Ok(vec![]);
        }
        let placeholders = std::iter::repeat_n("?", agent_chain.len())
            .collect::<Vec<_>>()
            .join(",");
        let active_clause = if active_only {
            "AND status IN ('Starting','Running')"
        } else {
            ""
        };
        let sql = format!(
            r#"SELECT id, session_id, agent_id, description, command, command_key, cwd,
                      timeout_ms, silent_completion, status, exit_code,
                      failure_kind, failure_detail, failure_extracted,
                      started_at, finished_at, total_bytes_emitted, bisect_count,
                      log_path, final_path, last_polled_at, last_seen_offset
               FROM coding_background_jobs
               WHERE session_id = ? AND agent_id IN ({placeholders}) {active_clause}
               ORDER BY started_at DESC LIMIT 20"#
        );
        let mut q = sqlx::query(&sql).bind(session_id);
        for ag in agent_chain {
            q = q.bind(ag);
        }
        let rows = q.fetch_all(&self.pool).await?;
        rows.iter().map(Self::map_row).collect()
    }

    pub async fn list_all_for_session(
        &self,
        session_id: &str,
        active_only: bool,
    ) -> Result<Vec<BashJobRow>, StorageError> {
        let mut qb = sqlx::QueryBuilder::new(
            "SELECT id, session_id, agent_id, description, command, command_key, cwd, \
             timeout_ms, silent_completion, status, exit_code, \
             failure_kind, failure_detail, failure_extracted, \
             started_at, finished_at, total_bytes_emitted, bisect_count, \
             log_path, final_path, last_polled_at, last_seen_offset \
             FROM coding_background_jobs WHERE session_id = ",
        );
        qb.push_bind(session_id);
        if active_only {
            qb.push(" AND status IN ('Starting','Running')");
        }
        qb.push(" ORDER BY started_at DESC LIMIT 20");
        let rows = qb.build().fetch_all(&self.pool).await?;
        rows.iter().map(Self::map_row).collect()
    }

    pub async fn count_active_for_chain(
        &self,
        session_id: &str,
        agent_chain: &[String],
    ) -> Result<i64, StorageError> {
        if agent_chain.is_empty() {
            return Ok(0);
        }
        let placeholders = std::iter::repeat_n("?", agent_chain.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT COUNT(*) FROM coding_background_jobs
             WHERE session_id = ? AND agent_id IN ({placeholders})
             AND status IN ('Starting','Running')"
        );
        let mut q = sqlx::query_scalar::<_, i64>(&sql).bind(session_id);
        for ag in agent_chain {
            q = q.bind(ag);
        }
        let count = q.fetch_one(&self.pool).await?;
        Ok(count)
    }

    pub async fn list_orphans(&self) -> Result<Vec<BashJobRow>, StorageError> {
        let rows = sqlx::query(
            r#"SELECT id, session_id, agent_id, description, command, command_key, cwd,
                      timeout_ms, silent_completion, status, exit_code,
                      failure_kind, failure_detail, failure_extracted,
                      started_at, finished_at, total_bytes_emitted, bisect_count,
                      log_path, final_path, last_polled_at, last_seen_offset
               FROM coding_background_jobs
               WHERE status IN ('Starting','Running')
               LIMIT 100"#,
        )
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(Self::map_row).collect()
    }

    pub async fn delete(&self, id: &str) -> Result<(), StorageError> {
        sqlx::query("DELETE FROM coding_background_jobs WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    fn map_row(row: &sqlx::sqlite::SqliteRow) -> Result<BashJobRow, StorageError> {
        use sqlx::Row;
        let started_at: String = row.try_get("started_at")?;
        let started_at = started_at
            .parse::<Timestamp>()
            .map_err(|e| StorageError::Serialization(format!("started_at: {e}")))?;
        let finished_at: Option<String> = row.try_get("finished_at")?;
        let finished_at = finished_at
            .map(|s| s.parse::<Timestamp>())
            .transpose()
            .map_err(|e| StorageError::Serialization(format!("finished_at: {e}")))?;
        let last_polled_at: Option<String> = row.try_get("last_polled_at")?;
        let last_polled_at = last_polled_at
            .map(|s| s.parse::<Timestamp>())
            .transpose()
            .map_err(|e| StorageError::Serialization(format!("last_polled_at: {e}")))?;
        Ok(BashJobRow {
            id: row.try_get("id")?,
            session_id: row.try_get("session_id")?,
            agent_id: row.try_get("agent_id")?,
            description: row.try_get("description")?,
            command: row.try_get("command")?,
            command_key: row.try_get("command_key")?,
            cwd: row.try_get("cwd")?,
            timeout_ms: row.try_get("timeout_ms")?,
            silent_completion: row.try_get::<i64, _>("silent_completion")? != 0,
            status: row.try_get("status")?,
            exit_code: row.try_get("exit_code")?,
            failure_kind: row.try_get("failure_kind")?,
            failure_detail: row.try_get("failure_detail")?,
            failure_extracted: row.try_get("failure_extracted")?,
            started_at,
            finished_at,
            total_bytes_emitted: row.try_get("total_bytes_emitted")?,
            bisect_count: row.try_get("bisect_count")?,
            log_path: row.try_get("log_path")?,
            final_path: row.try_get("final_path")?,
            last_polled_at,
            last_seen_offset: row.try_get("last_seen_offset")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    const SCHEMA: &str = r#"
        CREATE TABLE coding_background_jobs (
            id                    TEXT PRIMARY KEY,
            session_id            TEXT NOT NULL,
            agent_id              TEXT NOT NULL,
            description           TEXT NOT NULL,
            command               TEXT NOT NULL,
            command_key           TEXT NOT NULL,
            cwd                   TEXT NOT NULL,
            timeout_ms            INTEGER NOT NULL,
            silent_completion     INTEGER NOT NULL DEFAULT 0,
            status                TEXT NOT NULL,
            exit_code             INTEGER,
            failure_kind          TEXT,
            failure_detail        TEXT,
            failure_extracted     TEXT,
            started_at            TEXT NOT NULL,
            finished_at           TEXT,
            total_bytes_emitted   INTEGER NOT NULL DEFAULT 0,
            bisect_count          INTEGER NOT NULL DEFAULT 0,
            log_path              TEXT NOT NULL,
            final_path            TEXT,
            last_polled_at        TEXT,
            last_seen_offset      INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX idx_cbj_session_status ON coding_background_jobs(session_id, status);
    "#;

    async fn setup() -> SqlitePool {
        let pool = SqlitePoolOptions::new().connect(":memory:").await.unwrap();
        sqlx::query(SCHEMA).execute(&pool).await.unwrap();
        pool
    }

    fn fixture_row(id: &str, session: &str, agent: &str) -> BashJobRow {
        BashJobRow {
            id: id.into(),
            session_id: session.into(),
            agent_id: agent.into(),
            description: "test".into(),
            command: "echo hi".into(),
            command_key: "echo_hi".into(),
            cwd: "/tmp".into(),
            timeout_ms: 600_000,
            silent_completion: false,
            status: "Running".into(),
            exit_code: None,
            failure_kind: None,
            failure_detail: None,
            failure_extracted: None,
            started_at: jiff::Timestamp::now(),
            finished_at: None,
            total_bytes_emitted: 0,
            bisect_count: 0,
            log_path: format!("/tmp/{id}.log"),
            final_path: None,
            last_polled_at: None,
            last_seen_offset: 0,
        }
    }

    #[tokio::test]
    async fn insert_and_get() {
        let pool = setup().await;
        let repo = BashJobRepo::new(pool);
        let row = fixture_row("bash-test000001", "s1", "root");
        repo.insert(&row).await.unwrap();
        let got = repo.get("bash-test000001").await.unwrap().unwrap();
        assert_eq!(got.id, row.id);
        assert_eq!(got.status, "Running");
    }

    #[tokio::test]
    async fn list_for_session_filters_by_chain() {
        let pool = setup().await;
        let repo = BashJobRepo::new(pool);
        repo.insert(&fixture_row("bash-aaaaaaaa01", "s1", "root"))
            .await
            .unwrap();
        repo.insert(&fixture_row("bash-bbbbbbbb01", "s1", "subagent-1"))
            .await
            .unwrap();
        repo.insert(&fixture_row("bash-cccccccc01", "s1", "outsider"))
            .await
            .unwrap();
        let chain = vec!["root".to_string(), "subagent-1".to_string()];
        let list = repo.list_for_session("s1", &chain, true).await.unwrap();
        assert_eq!(list.len(), 2);
        assert!(list
            .iter()
            .all(|r| r.agent_id == "root" || r.agent_id == "subagent-1"));
    }

    #[tokio::test]
    async fn count_active_for_chain_respects_status() {
        let pool = setup().await;
        let repo = BashJobRepo::new(pool);
        repo.insert(&fixture_row("bash-aaaaaaaa01", "s1", "root"))
            .await
            .unwrap();
        let mut completed = fixture_row("bash-bbbbbbbb01", "s1", "root");
        completed.status = "Completed".into();
        repo.insert(&completed).await.unwrap();
        let chain = vec!["root".to_string()];
        let n = repo.count_active_for_chain("s1", &chain).await.unwrap();
        assert_eq!(n, 1);
    }

    #[tokio::test]
    async fn update_status_persists_failure_extracted() {
        let pool = setup().await;
        let repo = BashJobRepo::new(pool);
        repo.insert(&fixture_row("bash-aaaaaaaa01", "s1", "root"))
            .await
            .unwrap();
        repo.update_status(
            "bash-aaaaaaaa01",
            "Failed",
            Some(101),
            Some("TestFailure"),
            Some("3 failed"),
            Some(r#"{"test_name":"foo","n_failed":3}"#),
            Some(jiff::Timestamp::now()),
            Some("/tmp/bash-aaaaaaaa01.final"),
            12_345,
            0,
        )
        .await
        .unwrap();
        let got = repo.get("bash-aaaaaaaa01").await.unwrap().unwrap();
        assert_eq!(got.status, "Failed");
        assert_eq!(got.failure_kind.unwrap(), "TestFailure");
        assert!(got.failure_extracted.unwrap().contains("test_name"));
    }

    #[tokio::test]
    async fn list_orphans_returns_active_only() {
        let pool = setup().await;
        let repo = BashJobRepo::new(pool);
        repo.insert(&fixture_row("bash-aaaaaaaa01", "s1", "root"))
            .await
            .unwrap();
        let mut completed = fixture_row("bash-bbbbbbbb01", "s1", "root");
        completed.status = "Completed".into();
        repo.insert(&completed).await.unwrap();
        let orphans = repo.list_orphans().await.unwrap();
        assert_eq!(orphans.len(), 1);
        assert_eq!(orphans[0].id, "bash-aaaaaaaa01");
    }

    #[tokio::test]
    async fn find_prior_returns_most_recent_terminal() {
        let pool = setup().await;
        let repo = BashJobRepo::new(pool);
        let t1 = jiff::Timestamp::from_millisecond(1_700_000_000_000).unwrap();
        let t2 = jiff::Timestamp::from_millisecond(1_700_000_010_000).unwrap();
        let mut r1 = fixture_row("bash-a", "s1", "root");
        r1.command_key = "k1".into();
        r1.status = "Completed".into();
        r1.started_at = t1;
        r1.finished_at = Some(t1);
        repo.insert(&r1).await.unwrap();

        let mut r2 = fixture_row("bash-b", "s1", "root");
        r2.command_key = "k1".into();
        r2.status = "Failed".into();
        r2.started_at = t2;
        r2.finished_at = Some(t2);
        repo.insert(&r2).await.unwrap();

        let prior = repo
            .find_prior_by_command_key("s1", "k1", "bash-x")
            .await
            .unwrap();
        assert_eq!(prior.unwrap().id, "bash-b");
    }

    #[tokio::test]
    async fn find_prior_excludes_self_id() {
        let pool = setup().await;
        let repo = BashJobRepo::new(pool);
        let mut r1 = fixture_row("bash-a", "s1", "root");
        r1.command_key = "k1".into();
        r1.status = "Completed".into();
        repo.insert(&r1).await.unwrap();

        assert!(repo
            .find_prior_by_command_key("s1", "k1", "bash-a")
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn find_prior_excludes_lost_status() {
        let pool = setup().await;
        let repo = BashJobRepo::new(pool);
        let mut r1 = fixture_row("bash-a", "s1", "root");
        r1.command_key = "k1".into();
        r1.status = "Lost".into();
        repo.insert(&r1).await.unwrap();

        assert!(repo
            .find_prior_by_command_key("s1", "k1", "bash-x")
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn find_prior_returns_none_when_no_prior() {
        let pool = setup().await;
        let repo = BashJobRepo::new(pool);
        assert!(repo
            .find_prior_by_command_key("s1", "k1", "bash-x")
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn find_prior_scoped_by_session() {
        let pool = setup().await;
        let repo = BashJobRepo::new(pool);
        let mut r1 = fixture_row("bash-a", "s1", "root");
        r1.command_key = "k1".into();
        r1.status = "Completed".into();
        repo.insert(&r1).await.unwrap();

        assert!(repo
            .find_prior_by_command_key("s2", "k1", "bash-x")
            .await
            .unwrap()
            .is_none());
    }
}
