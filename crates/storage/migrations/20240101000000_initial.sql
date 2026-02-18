-- Initial schema for klyntbot PostgreSQL storage.
-- Replaces all JSONL flat-file stores.

-- Enable pgvector for embedding similarity search.
CREATE EXTENSION IF NOT EXISTS vector;

-- ============================================================
-- Projects
-- ============================================================
CREATE TABLE projects (
    id          VARCHAR(8) PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT,
    color       TEXT NOT NULL DEFAULT 'orange',
    tags        TEXT[] NOT NULL DEFAULT '{}',
    status      TEXT NOT NULL DEFAULT 'active',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ============================================================
-- Todos
-- ============================================================
CREATE TABLE todos (
    id                   VARCHAR(8) PRIMARY KEY,
    title                TEXT NOT NULL,
    description          TEXT,
    priority             SMALLINT,
    due_date             TIMESTAMPTZ,
    tags                 TEXT[] NOT NULL DEFAULT '{}',
    status               TEXT NOT NULL DEFAULT 'todo',
    focused_at           TIMESTAMPTZ,
    focus_deadline       TIMESTAMPTZ,
    focus_expired_count  INT NOT NULL DEFAULT 0,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at         TIMESTAMPTZ,
    parent_id            VARCHAR(8) REFERENCES todos(id) ON DELETE SET NULL,
    project_id           VARCHAR(8) REFERENCES projects(id) ON DELETE SET NULL,
    total_tracked_secs   BIGINT NOT NULL DEFAULT 0,
    estimated_minutes    INT,
    calendar_event_uid   TEXT,
    last_reminded_at     TIMESTAMPTZ,
    recurrence_rule      TEXT,
    recurrence_parent_id TEXT,
    is_template          BOOLEAN NOT NULL DEFAULT FALSE,
    next_instance_date   TIMESTAMPTZ
);

CREATE INDEX idx_todos_status ON todos(status);
CREATE INDEX idx_todos_project_id ON todos(project_id);
CREATE INDEX idx_todos_parent_id ON todos(parent_id);
CREATE INDEX idx_todos_due_date ON todos(due_date);
CREATE INDEX idx_todos_tags ON todos USING GIN (tags);
CREATE INDEX idx_todos_focused_at ON todos(focused_at) WHERE focused_at IS NOT NULL;
CREATE INDEX idx_todos_is_template ON todos(is_template) WHERE is_template = TRUE;

-- ============================================================
-- Todo Attachments (join table)
-- ============================================================
CREATE TABLE todo_attachments (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    todo_id         VARCHAR(8) NOT NULL REFERENCES todos(id) ON DELETE CASCADE,
    attachment_type TEXT NOT NULL,
    value           TEXT NOT NULL,
    title           TEXT,
    tags            TEXT[] NOT NULL DEFAULT '{}',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_todo_attachments_todo_id ON todo_attachments(todo_id);

-- ============================================================
-- Todo Time Entries (join table)
-- ============================================================
CREATE TABLE todo_time_entries (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    todo_id       VARCHAR(8) NOT NULL REFERENCES todos(id) ON DELETE CASCADE,
    source        TEXT NOT NULL DEFAULT 'focus',
    started_at    TIMESTAMPTZ NOT NULL,
    ended_at      TIMESTAMPTZ,
    duration_secs BIGINT,
    note          TEXT
);

CREATE INDEX idx_todo_time_entries_todo_id ON todo_time_entries(todo_id);

-- ============================================================
-- Todo Dependencies (edge table)
-- ============================================================
CREATE TABLE todo_dependencies (
    task_id    VARCHAR(8) NOT NULL REFERENCES todos(id) ON DELETE CASCADE,
    blocker_id VARCHAR(8) NOT NULL REFERENCES todos(id) ON DELETE CASCADE,
    PRIMARY KEY (task_id, blocker_id),
    CHECK (task_id != blocker_id)
);

CREATE INDEX idx_todo_dependencies_blocker_id ON todo_dependencies(blocker_id);

-- ============================================================
-- Sessions
-- ============================================================
CREATE TABLE sessions (
    key        TEXT PRIMARY KEY,
    metadata   JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ============================================================
-- Session Messages
-- ============================================================
CREATE TABLE session_messages (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_key TEXT NOT NULL REFERENCES sessions(key) ON DELETE CASCADE,
    role        TEXT NOT NULL,
    content     TEXT NOT NULL,
    timestamp   TIMESTAMPTZ NOT NULL DEFAULT now(),
    request_id  TEXT
);

CREATE INDEX idx_session_messages_key_ts ON session_messages(session_key, timestamp);

-- ============================================================
-- Goals
-- ============================================================
CREATE TABLE goals (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title       TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    status      TEXT NOT NULL DEFAULT 'Active',
    priority    SMALLINT NOT NULL DEFAULT 3,
    target_date TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    metrics     JSONB NOT NULL DEFAULT '[]',
    metadata    JSONB NOT NULL DEFAULT '{}'
);

-- ============================================================
-- Goal ↔ Project Links (many-to-many)
-- ============================================================
CREATE TABLE goal_project_links (
    goal_id    UUID NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
    project_id VARCHAR(8) NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    PRIMARY KEY (goal_id, project_id)
);

-- ============================================================
-- Plans
-- ============================================================
CREATE TABLE plans (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_key        TEXT NOT NULL,
    goal_id            UUID REFERENCES goals(id) ON DELETE SET NULL,
    title              TEXT NOT NULL,
    description        TEXT NOT NULL DEFAULT '',
    status             TEXT NOT NULL DEFAULT 'Draft',
    current_step_index INT NOT NULL DEFAULT 0,
    iteration_limit    INT NOT NULL DEFAULT 50,
    backtrack_history  JSONB NOT NULL DEFAULT '[]',
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at       TIMESTAMPTZ
);

CREATE INDEX idx_plans_session_status ON plans(session_key, status);

-- ============================================================
-- Plan Steps
-- ============================================================
CREATE TABLE plan_steps (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    plan_id        UUID NOT NULL REFERENCES plans(id) ON DELETE CASCADE,
    step_index     INT NOT NULL,
    description    TEXT NOT NULL,
    reasoning      TEXT NOT NULL DEFAULT '',
    expected_tools TEXT[] NOT NULL DEFAULT '{}',
    status         TEXT NOT NULL DEFAULT 'Pending',
    attempt_count  SMALLINT NOT NULL DEFAULT 0,
    max_attempts   SMALLINT NOT NULL DEFAULT 3,
    result         TEXT,
    started_at     TIMESTAMPTZ,
    completed_at   TIMESTAMPTZ
);

CREATE INDEX idx_plan_steps_plan_id ON plan_steps(plan_id);

-- ============================================================
-- Todo Embeddings (pgvector)
-- ============================================================
CREATE TABLE todo_embeddings (
    todo_id    VARCHAR(8) PRIMARY KEY REFERENCES todos(id) ON DELETE CASCADE,
    embedding  vector(384) NOT NULL,
    model      TEXT NOT NULL DEFAULT 'paraphrase-multilingual-MiniLM-L12-v2',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- IVFFlat index for approximate nearest neighbor search.
-- Requires at least some rows to exist; safe to create on empty table.
CREATE INDEX idx_todo_embeddings_ann ON todo_embeddings
    USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);

-- ============================================================
-- Conversation Embeddings (pgvector)
-- ============================================================
CREATE TABLE conversation_embeddings (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_key     TEXT NOT NULL,
    embedding       vector(384) NOT NULL,
    role            TEXT NOT NULL,
    content_preview TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_conv_embeddings_ann ON conversation_embeddings
    USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100);

-- ============================================================
-- Learning Outcomes
-- ============================================================
CREATE TABLE learning_outcomes (
    id                     TEXT PRIMARY KEY,
    session_key            TEXT NOT NULL,
    tool_name              TEXT NOT NULL,
    success                BOOLEAN NOT NULL,
    error_category         TEXT,
    duration_ms            BIGINT NOT NULL,
    confidence_score       REAL,
    confidence_dimensions  JSONB,
    execution_mode         JSONB NOT NULL DEFAULT '"chat"',
    created_at             TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_learning_outcomes_created_at ON learning_outcomes(created_at);

-- ============================================================
-- Strategy Records
-- ============================================================
CREATE TABLE strategy_records (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    timestamp          TIMESTAMPTZ NOT NULL DEFAULT now(),
    request_id         TEXT NOT NULL,
    predicted_strategy TEXT NOT NULL,
    actual_strategy    TEXT NOT NULL,
    escalation_count   INT NOT NULL DEFAULT 0,
    iterations_used    INT NOT NULL DEFAULT 0,
    max_iterations     INT NOT NULL DEFAULT 0,
    success            BOOLEAN NOT NULL,
    user_satisfaction  REAL,
    response_time_ms   BIGINT NOT NULL DEFAULT 0
);

-- ============================================================
-- Enrichment Feedback
-- ============================================================
CREATE TABLE enrichment_feedback (
    id              SERIAL PRIMARY KEY,
    task_id         TEXT NOT NULL,
    field           TEXT NOT NULL,
    suggested_value TEXT NOT NULL,
    actual_value    TEXT,
    accepted        BOOLEAN NOT NULL,
    confidence      DOUBLE PRECISION NOT NULL,
    timestamp       TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ============================================================
-- Usage Records (cost tracking)
-- ============================================================
CREATE TABLE usage_records (
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    timestamp          TIMESTAMPTZ NOT NULL DEFAULT now(),
    request_id         TEXT NOT NULL,
    model              TEXT NOT NULL,
    provider           TEXT NOT NULL,
    prompt_tokens      INT NOT NULL DEFAULT 0,
    completion_tokens  INT NOT NULL DEFAULT 0,
    cache_read_tokens  INT NOT NULL DEFAULT 0,
    cache_write_tokens INT NOT NULL DEFAULT 0,
    estimated_cost_usd DOUBLE PRECISION NOT NULL DEFAULT 0,
    channel            TEXT NOT NULL DEFAULT '',
    strategy           TEXT NOT NULL DEFAULT ''
);

CREATE INDEX idx_usage_records_timestamp ON usage_records(timestamp);

-- ============================================================
-- Cron Jobs
-- ============================================================
CREATE TABLE cron_jobs (
    id               TEXT PRIMARY KEY,
    name             TEXT NOT NULL,
    enabled          BOOLEAN NOT NULL DEFAULT TRUE,
    schedule         JSONB NOT NULL,
    payload          JSONB NOT NULL DEFAULT '{}',
    next_run_at_ms   BIGINT,
    last_run_at_ms   BIGINT,
    last_status      TEXT,
    last_error       TEXT,
    created_at_ms    BIGINT NOT NULL DEFAULT 0,
    updated_at_ms    BIGINT NOT NULL DEFAULT 0,
    delete_after_run BOOLEAN NOT NULL DEFAULT FALSE
);

-- ============================================================
-- Calendar Sync State
-- ============================================================
CREATE TABLE calendar_sync_state (
    provider_id  TEXT PRIMARY KEY,
    sync_token   TEXT,
    last_sync_at TIMESTAMPTZ
);
