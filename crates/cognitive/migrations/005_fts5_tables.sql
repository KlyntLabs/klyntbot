-- Full-text index for semantic facts
CREATE VIRTUAL TABLE IF NOT EXISTS semantic_facts_fts USING fts5(
    id UNINDEXED,
    domain,
    subject,
    predicate,
    object,
    memory_type,
    content='semantic_facts',
    content_rowid='rowid',
    tokenize='porter unicode61'
);

-- Keep FTS in sync with semantic_facts
CREATE TRIGGER IF NOT EXISTS semantic_facts_ai AFTER INSERT ON semantic_facts BEGIN
    INSERT INTO semantic_facts_fts(rowid, id, domain, subject, predicate, object, memory_type)
    VALUES (new.rowid, new.id, new.domain, new.subject, new.predicate, new.object, new.memory_type);
END;

CREATE TRIGGER IF NOT EXISTS semantic_facts_ad AFTER DELETE ON semantic_facts BEGIN
    INSERT INTO semantic_facts_fts(semantic_facts_fts, rowid, id, domain, subject, predicate, object, memory_type)
    VALUES ('delete', old.rowid, old.id, old.domain, old.subject, old.predicate, old.object, old.memory_type);
END;

CREATE TRIGGER IF NOT EXISTS semantic_facts_au AFTER UPDATE ON semantic_facts BEGIN
    INSERT INTO semantic_facts_fts(semantic_facts_fts, rowid, id, domain, subject, predicate, object, memory_type)
    VALUES ('delete', old.rowid, old.id, old.domain, old.subject, old.predicate, old.object, old.memory_type);
    INSERT INTO semantic_facts_fts(rowid, id, domain, subject, predicate, object, memory_type)
    VALUES (new.rowid, new.id, new.domain, new.subject, new.predicate, new.object, new.memory_type);
END;

-- Full-text index for episodic memories
CREATE VIRTUAL TABLE IF NOT EXISTS episodic_memories_fts USING fts5(
    id UNINDEXED,
    domain,
    content,
    summary,
    content='episodic_memories',
    content_rowid='rowid',
    tokenize='porter unicode61'
);

CREATE TRIGGER IF NOT EXISTS episodic_memories_ai AFTER INSERT ON episodic_memories BEGIN
    INSERT INTO episodic_memories_fts(rowid, id, domain, content, summary)
    VALUES (new.rowid, new.id, new.domain, new.content, new.summary);
END;

CREATE TRIGGER IF NOT EXISTS episodic_memories_ad AFTER DELETE ON episodic_memories BEGIN
    INSERT INTO episodic_memories_fts(episodic_memories_fts, rowid, id, domain, content, summary)
    VALUES ('delete', old.rowid, old.id, old.domain, old.content, old.summary);
END;

CREATE TRIGGER IF NOT EXISTS episodic_memories_au AFTER UPDATE ON episodic_memories BEGIN
    INSERT INTO episodic_memories_fts(episodic_memories_fts, rowid, id, domain, content, summary)
    VALUES ('delete', old.rowid, old.id, old.domain, old.content, old.summary);
    INSERT INTO episodic_memories_fts(rowid, id, domain, content, summary)
    VALUES (new.rowid, new.id, new.domain, new.content, new.summary);
END;

-- Full-text index for procedural rules
CREATE VIRTUAL TABLE IF NOT EXISTS procedural_rules_fts USING fts5(
    id UNINDEXED,
    domain,
    rule_text,
    content='procedural_rules',
    content_rowid='rowid',
    tokenize='porter unicode61'
);

CREATE TRIGGER IF NOT EXISTS procedural_rules_ai AFTER INSERT ON procedural_rules BEGIN
    INSERT INTO procedural_rules_fts(rowid, id, domain, rule_text)
    VALUES (new.rowid, new.id, new.domain, new.rule_text);
END;

CREATE TRIGGER IF NOT EXISTS procedural_rules_ad AFTER DELETE ON procedural_rules BEGIN
    INSERT INTO procedural_rules_fts(procedural_rules_fts, rowid, id, domain, rule_text)
    VALUES ('delete', old.rowid, old.id, old.domain, old.rule_text);
END;

CREATE TRIGGER IF NOT EXISTS procedural_rules_au AFTER UPDATE ON procedural_rules BEGIN
    INSERT INTO procedural_rules_fts(procedural_rules_fts, rowid, id, domain, rule_text)
    VALUES ('delete', old.rowid, old.id, old.domain, old.rule_text);
    INSERT INTO procedural_rules_fts(rowid, id, domain, rule_text)
    VALUES (new.rowid, new.id, new.domain, new.rule_text);
END;
