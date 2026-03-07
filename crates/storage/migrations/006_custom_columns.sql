-- Custom column definitions (per-project)
CREATE TABLE IF NOT EXISTS custom_columns (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    column_type TEXT NOT NULL CHECK(column_type IN (
        'text','number','date','dropdown','multi_select','checkbox',
        'tags','link','rating','progress','duration','currency'
    )),
    options_json TEXT,
    position INTEGER NOT NULL DEFAULT 0,
    width INTEGER DEFAULT 150,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Custom column values (per-task, per-column)
CREATE TABLE IF NOT EXISTS custom_column_values (
    task_id TEXT NOT NULL REFERENCES actions(id) ON DELETE CASCADE,
    column_id TEXT NOT NULL REFERENCES custom_columns(id) ON DELETE CASCADE,
    value_json TEXT NOT NULL,
    PRIMARY KEY (task_id, column_id)
);
