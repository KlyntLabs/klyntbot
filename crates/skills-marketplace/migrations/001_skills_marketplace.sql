CREATE TABLE IF NOT EXISTS installed_skills (
  name                   TEXT PRIMARY KEY,
  source_type            TEXT NOT NULL CHECK(source_type IN ('github','skills_sh','local','bundled')),
  source_ref             TEXT NOT NULL,
  installed_version      TEXT NOT NULL,
  installed_sha          TEXT NOT NULL,
  enabled                INTEGER NOT NULL DEFAULT 1,
  is_adapted             INTEGER NOT NULL DEFAULT 0,
  bootstrapped_databases TEXT,
  installed_at           TEXT NOT NULL,
  updated_at             TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_installed_skills_source
  ON installed_skills(source_type, source_ref);

CREATE TABLE IF NOT EXISTS adapted_skills (
  cache_key              TEXT PRIMARY KEY,
  adapted_skill_md       TEXT NOT NULL,
  generated_templates    TEXT NOT NULL,
  rationale              TEXT NOT NULL,
  adapter_model          TEXT NOT NULL,
  created_at             TEXT NOT NULL
);
