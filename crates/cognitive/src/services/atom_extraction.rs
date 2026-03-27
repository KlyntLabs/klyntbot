//! Background service that extracts knowledge atoms from note content.
//!
//! Subscribes to [`DomainEventBus`] for `NoteEditingFinished` events (fired when
//! the user blurs, closes, or idles on a note). Debounces rapid events, deduplicates
//! against existing atoms, and creates new `active` atoms via an LLM extraction prompt.
//! Cross-note reinforcement is detected and boosted rather than duplicated.

use std::collections::HashMap;
use std::sync::Arc;

use sha2::Digest;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use bus::{DomainEvent, DomainEventBus};
use config::schema::AtomExtractionConfig;
use providers::{ChatParams, DynProvider, Message, UserContent};

use crate::repos::{AtomExtractionCache, KnowledgeAtomRepo, NewKnowledgeAtom};

/// Minimum word count for a section to be worth extracting.
const MIN_SECTION_WORDS: usize = 30;

/// Target word count per chunk when there are no headings.
const CHUNK_TARGET_WORDS: usize = 200;

/// Debounce window: ignore events for the same note within this duration.
const DEBOUNCE_SECS: u64 = 5;

/// Default personal importance for suggested atoms.
const SUGGESTED_IMPORTANCE: f64 = 0.7;

/// Salience boost applied when a concept is reinforced from another note.
const REINFORCEMENT_BOOST: f64 = 0.1;

/// LLM extraction result for a single atom.
#[derive(Debug, serde::Deserialize)]
struct ExtractedAtom {
    subject: String,
    #[serde(rename = "atomType")]
    atom_type: String,
    domain: String,
    #[serde(rename = "sourceContext")]
    source_context: Option<String>,
}

pub struct AtomExtractionService;

impl AtomExtractionService {
    /// Spawn the background extraction loop. Returns immediately.
    pub fn start(
        pool: sqlx::SqlitePool,
        provider: DynProvider,
        bus: Arc<DomainEventBus>,
        config: AtomExtractionConfig,
        cancel: CancellationToken,
    ) {
        if !config.enabled {
            info!("atom extraction service disabled by config");
            return;
        }

        let mut rx = bus.subscribe();
        let max_tokens = config.max_tokens;

        tokio::spawn(async move {
            let cache = AtomExtractionCache::new(pool.clone());
            let atom_repo = KnowledgeAtomRepo::new(pool);
            let mut debounce_map: HashMap<String, tokio::time::Instant> = HashMap::new();

            info!("atom extraction service started");

            loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        info!("atom extraction service shutting down");
                        break;
                    }
                    result = rx.recv() => {
                        match result {
                            Ok(DomainEvent::NoteEditingFinished { note_id, content }) => {
                                // Debounce: skip if we processed this note recently
                                let now = tokio::time::Instant::now();
                                if let Some(last) = debounce_map.get(&note_id) {
                                    if now.duration_since(*last).as_secs() < DEBOUNCE_SECS {
                                        debug!(note_id, "debounced atom extraction");
                                        continue;
                                    }
                                }
                                debounce_map.insert(note_id.clone(), now);

                                // Prevent unbounded growth of the debounce map
                                if debounce_map.len() > 500 {
                                    let cutoff = now - tokio::time::Duration::from_secs(60);
                                    debounce_map.retain(|_, ts| *ts > cutoff);
                                }

                                if let Err(e) = process_note(
                                    &note_id,
                                    &content,
                                    &cache,
                                    &atom_repo,
                                    &provider,
                                    &bus,
                                    max_tokens,
                                ).await {
                                    warn!(note_id, error = %e, "atom extraction failed");
                                }
                            }
                            Ok(_) => {} // ignore other events
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                warn!("atom extraction lagged by {n} events");
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        }
                    }
                }
            }
        });
    }
}

/// Process a single note: hash-check, split, extract, deduplicate, persist.
async fn process_note(
    note_id: &str,
    content: &str,
    cache: &AtomExtractionCache,
    atom_repo: &KnowledgeAtomRepo,
    provider: &DynProvider,
    bus: &Arc<DomainEventBus>,
    max_tokens: u32,
) -> common::Result<()> {
    // 1. Content hash check
    let content_hash = hex_sha256(content);
    if cache
        .is_cached(note_id, &content_hash)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?
    {
        debug!(note_id, "content unchanged, skipping extraction");
        return Ok(());
    }

    // 2. Split into sections
    let sections = split_into_sections(content);
    if sections.is_empty() {
        debug!(note_id, "no extractable sections found");
        cache
            .set(note_id, &content_hash)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        return Ok(());
    }

    info!(
        note_id,
        section_count = sections.len(),
        "extracting atoms from note"
    );

    let mut total_created = 0usize;
    let mut total_reinforced = 0usize;
    let mut topic_cache: HashMap<String, String> = HashMap::new(); // domain → topic_id
    let mut affected_topic_ids: std::collections::HashSet<String> =
        std::collections::HashSet::new();

    // 3. Extract from each section
    for section in &sections {
        let mut extracted = match call_extraction_llm(provider, section, max_tokens).await {
            Ok(atoms) => atoms,
            Err(e) => {
                warn!(note_id, error = %e, "LLM extraction failed for section");
                continue;
            }
        };

        // Validate atom_type + normalize domain
        let valid_types = [
            "concept",
            "fact",
            "procedure",
            "reference",
            "pattern",
            "insight",
            "relation",
        ];
        for atom in &mut extracted {
            if !valid_types.contains(&atom.atom_type.as_str()) {
                atom.atom_type = "concept".to_string();
            }
            atom.domain = normalize_domain(&atom.domain);
        }

        if extracted.is_empty() {
            continue;
        }

        // 4. Group extracted atoms by domain for batch dedup
        let mut by_domain: HashMap<String, Vec<&ExtractedAtom>> = HashMap::new();
        for atom in &extracted {
            by_domain.entry(atom.domain.clone()).or_default().push(atom);
        }

        let mut existing_subjects = std::collections::HashSet::new();
        for (domain, atoms_in_domain) in &by_domain {
            let domain_subjects: Vec<String> =
                atoms_in_domain.iter().map(|a| a.subject.clone()).collect();
            match atom_repo
                .find_existing_subjects(domain, &domain_subjects)
                .await
            {
                Ok(found) => existing_subjects.extend(found),
                Err(e) => warn!(note_id, error = %e, "failed to check existing subjects"),
            }
        }

        // 5. Process each extracted atom
        for atom in &extracted {
            if existing_subjects.contains(&atom.subject) {
                // Already exists in same domain — skip (same-note dedup)
                debug!(
                    subject = atom.subject,
                    "atom already exists in domain, skipping"
                );
                continue;
            }

            // 6. Check cross-note reinforcement
            match atom_repo
                .find_by_subject_across_notes(&atom.subject, note_id)
                .await
            {
                Ok(existing) if !existing.is_empty() => {
                    // Reinforce the first matching atom
                    let target = &existing[0];
                    match atom_repo
                        .boost_salience(&target.id, REINFORCEMENT_BOOST, note_id)
                        .await
                    {
                        Ok(new_salience) => {
                            info!(
                                atom_id = target.id,
                                subject = atom.subject,
                                new_salience,
                                "reinforced existing atom from cross-note reference"
                            );
                            bus.publish(DomainEvent::AtomReinforced {
                                atom_id: target.id.clone(),
                                referencing_note_id: note_id.to_string(),
                                new_salience,
                            });
                            total_reinforced += 1;
                        }
                        Err(e) => {
                            warn!(atom_id = target.id, error = %e, "failed to boost salience");
                        }
                    }
                    continue;
                }
                Ok(_) => {} // no cross-note match — create new
                Err(e) => {
                    warn!(subject = atom.subject, error = %e, "cross-note lookup failed");
                    // Proceed to create anyway
                }
            }

            // 7. Assign to a topic (cached per domain to avoid repeated DB lookups)
            let topic_id = if let Some(cached) = topic_cache.get(&atom.domain) {
                Some(cached.clone())
            } else {
                match atom_repo
                    .get_or_create_topic(&atom.domain, &atom.domain)
                    .await
                {
                    Ok(topic) => {
                        topic_cache.insert(atom.domain.clone(), topic.id.clone());
                        Some(topic.id)
                    }
                    Err(e) => {
                        warn!(domain = atom.domain, error = %e, "failed to get/create topic");
                        None
                    }
                }
            };

            // 8. Create new active atom
            let new_atom = NewKnowledgeAtom {
                subject: atom.subject.clone(),
                atom_type: atom.atom_type.clone(),
                domain: atom.domain.clone(),
                source_note_id: Some(note_id.to_string()),
                source_context: atom.source_context.clone(),
                personal_importance: SUGGESTED_IMPORTANCE,
                status: "active".to_string(),
                topic_id,
                ..Default::default()
            };

            match atom_repo.create(&new_atom).await {
                Ok(row) => {
                    info!(
                        atom_id = row.id,
                        subject = row.subject,
                        domain = row.domain,
                        "created active atom"
                    );
                    bus.publish(DomainEvent::KnowledgeAtomCreated {
                        atom_id: row.id,
                        atom_type: row.atom_type,
                        domain: row.domain,
                        source_note_id: Some(note_id.to_string()),
                        personal_importance: SUGGESTED_IMPORTANCE,
                    });
                    if let Some(tid) = &row.topic_id {
                        affected_topic_ids.insert(tid.clone());
                    }
                    total_created += 1;
                }
                Err(e) => {
                    warn!(subject = atom.subject, error = %e, "failed to create atom");
                }
            }
        }
    }

    // 9. Update only affected topic aggregates
    for tid in &affected_topic_ids {
        if let Err(e) = atom_repo.update_topic_aggregates(tid).await {
            warn!(topic_id = tid, error = %e, "failed to update topic aggregates");
        }
    }

    // 10. Update cache after successful processing
    cache
        .set(note_id, &content_hash)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

    info!(
        note_id,
        created = total_created,
        reinforced = total_reinforced,
        "atom extraction complete"
    );

    Ok(())
}

/// Compute hex-encoded SHA-256 of the content.
fn hex_sha256(content: &str) -> String {
    let digest = sha2::Sha256::digest(content.as_bytes());
    format!("{digest:x}")
}

/// Split note content into sections suitable for LLM extraction.
///
/// If the note contains `## ` headings, split on those. Otherwise split into
/// roughly 200-word chunks. Sections shorter than 30 words are dropped.
fn split_into_sections(content: &str) -> Vec<String> {
    let lines: Vec<&str> = content.lines().collect();

    // Try heading-based splitting first
    let mut sections = Vec::new();
    let mut current = String::new();
    let mut has_headings = false;

    for line in &lines {
        if line.starts_with("## ") {
            has_headings = true;
            let trimmed = current.trim().to_string();
            if word_count(&trimmed) >= MIN_SECTION_WORDS {
                sections.push(trimmed);
            }
            current = format!("{line}\n");
        } else {
            current.push_str(line);
            current.push('\n');
        }
    }

    // Push the last section
    let trimmed = current.trim().to_string();
    if word_count(&trimmed) >= MIN_SECTION_WORDS {
        sections.push(trimmed);
    }

    if has_headings && !sections.is_empty() {
        return sections;
    }

    // No headings — split into word-based chunks
    sections.clear();
    let words: Vec<&str> = content.split_whitespace().collect();
    let mut start = 0;

    while start < words.len() {
        let end = (start + CHUNK_TARGET_WORDS).min(words.len());
        let chunk = words[start..end].join(" ");
        if word_count(&chunk) >= MIN_SECTION_WORDS {
            sections.push(chunk);
        }
        start = end;
    }

    sections
}

/// Count whitespace-separated words.
fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

/// Normalize an LLM-produced domain string into a canonical `parent/child` format.
///
/// Handles inconsistent separators, known synonyms, and bare single-segment domains.
/// This is the single source of truth for domain taxonomy — all atoms pass through here.
pub fn normalize_domain(raw: &str) -> String {
    // 1. Lowercase, normalize separators (`:` and `_` → `/`), trim
    let cleaned = raw.trim().to_lowercase().replace([':', '_'], "/");

    // 2. Remove redundant slashes, split into segments
    let segments: Vec<&str> = cleaned.split('/').filter(|s| !s.is_empty()).collect();
    if segments.is_empty() {
        return "general/uncategorized".to_string();
    }

    // 3. Check full path against alias map first (e.g. "deep-learning" → "machine-learning/deep-learning")
    let full = segments.join("/");
    if let Some(mapped) = DOMAIN_ALIASES.iter().find(|(from, _)| *from == full) {
        return mapped.1.to_string();
    }

    // 4. If already two+ segments, check if parent needs remapping
    if segments.len() >= 2 {
        let parent = segments[0];
        let child = segments[1..].join("/");

        // Remap parent if it's an alias (e.g. "programming-languages/rust" → "software-engineering/rust")
        let canonical_parent = PARENT_ALIASES
            .iter()
            .find(|(from, _)| *from == parent)
            .map(|(_, to)| *to)
            .unwrap_or(parent);

        return format!("{canonical_parent}/{child}");
    }

    // 5. Single segment — check if it's a known child that should be nested
    let single = segments[0];
    if let Some(mapped) = CHILD_ALIASES.iter().find(|(from, _)| *from == single) {
        return mapped.1.to_string();
    }

    // 6. Single segment, unknown — make it `{segment}/general`
    format!("{single}/general")
}

/// Full-path aliases: exact match of the entire domain string.
const DOMAIN_ALIASES: &[(&str, &str)] = &[
    // ML ecosystem
    ("deep-learning", "machine-learning/deep-learning"),
    ("nlp", "machine-learning/nlp"),
    ("natural-language-processing", "machine-learning/nlp"),
    (
        "reinforcement-learning",
        "machine-learning/reinforcement-learning",
    ),
    ("machine-learning-systems", "machine-learning/systems"),
    ("ml-systems", "machine-learning/systems"),
    ("ml-ops", "machine-learning/ops"),
    ("computer-vision", "machine-learning/computer-vision"),
    ("generative-ai", "machine-learning/generative-ai"),
    ("llm", "machine-learning/llm"),
    ("llms", "machine-learning/llm"),
    ("neural-networks", "machine-learning/neural-networks"),
    ("transformers", "machine-learning/transformers"),
    // Math ecosystem
    ("linear-algebra", "mathematics/linear-algebra"),
    ("statistics", "mathematics/statistics"),
    ("probability", "mathematics/probability"),
    ("calculus", "mathematics/calculus"),
    ("optimization", "mathematics/optimization"),
    ("statistics/machine-learning", "machine-learning/statistics"),
    // SE ecosystem
    ("database-systems", "software-engineering/databases"),
    ("databases", "software-engineering/databases"),
    (
        "distributed-systems",
        "software-engineering/distributed-systems",
    ),
    ("system-design", "software-engineering/system-design"),
    ("devops", "software-engineering/devops"),
    ("testing", "software-engineering/testing"),
    ("web-development", "software-engineering/web"),
    ("backend", "software-engineering/backend"),
    ("frontend", "software-engineering/frontend"),
    ("api-design", "software-engineering/api-design"),
    // CS
    ("algorithms", "computer-science/algorithms"),
    ("data-structures", "computer-science/data-structures"),
    ("operating-systems", "computer-science/operating-systems"),
    ("networking", "computer-science/networking"),
    ("compilers", "computer-science/compilers"),
    ("cryptography", "computer-science/cryptography"),
    // Misc
    ("system", "software-engineering/general"),
    ("general", "general/uncategorized"),
];

/// Parent segment aliases: when the first segment matches, remap it.
const PARENT_ALIASES: &[(&str, &str)] = &[
    ("programming-languages", "software-engineering"),
    ("programming", "software-engineering"),
    ("math", "mathematics"),
    ("maths", "mathematics"),
    ("stats", "mathematics"),
    ("ml", "machine-learning"),
    ("ai", "machine-learning"),
    ("artificial-intelligence", "machine-learning"),
    ("cs", "computer-science"),
    ("se", "software-engineering"),
    ("infra", "software-engineering"),
    ("infrastructure", "software-engineering"),
    ("lang", "language"),
    ("languages", "language"),
];

/// Single-segment child aliases: bare terms that should nest under a parent.
const CHILD_ALIASES: &[(&str, &str)] = &[
    ("rust", "software-engineering/rust"),
    ("python", "software-engineering/python"),
    ("typescript", "software-engineering/typescript"),
    ("javascript", "software-engineering/javascript"),
    ("go", "software-engineering/go"),
    ("java", "software-engineering/java"),
    ("sql", "software-engineering/databases"),
    ("postgres", "software-engineering/databases"),
    ("redis", "software-engineering/databases"),
    ("docker", "software-engineering/devops"),
    ("kubernetes", "software-engineering/devops"),
    ("pytorch", "machine-learning/deep-learning"),
    ("tensorflow", "machine-learning/deep-learning"),
];

/// Call the LLM provider with the extraction prompt and parse JSON results.
async fn call_extraction_llm(
    provider: &DynProvider,
    section_text: &str,
    max_tokens: u32,
) -> common::Result<Vec<ExtractedAtom>> {
    let system_prompt = "You are a knowledge extraction assistant. Analyze this text and identify 3-8 key knowledge atoms worth tracking.\n\n\
        For each item, return:\n\
        - \"subject\": short label (2-5 words)\n\
        - \"atomType\": one of \"concept\", \"fact\", \"procedure\", \"reference\", \"pattern\", \"insight\", \"relation\"\n\
        - \"domain\": hierarchical category using EXACTLY the format \"parent/child\" (two levels max, lowercase, hyphenated). \
        Use broad parent categories and specific children. Examples:\n\
          - \"machine-learning/deep-learning\", \"machine-learning/nlp\", \"machine-learning/reinforcement-learning\"\n\
          - \"software-engineering/rust\", \"software-engineering/databases\", \"software-engineering/distributed-systems\"\n\
          - \"mathematics/linear-algebra\", \"mathematics/statistics\", \"mathematics/probability\"\n\
          - \"computer-science/algorithms\", \"computer-science/data-structures\"\n\
          - \"finance/investing\", \"finance/budgeting\"\n\
          - \"language/japanese\", \"language/english\"\n\
        Always use a parent even for broad topics (e.g. \"machine-learning/general\" not just \"machine-learning\"). \
        Group related subfields under the same parent — e.g. deep learning, NLP, and reinforcement learning all belong under \"machine-learning\".\n\
        - \"sourceContext\": the relevant sentence or phrase from the text (verbatim)\n\n\
        Return JSON array. Include genuinely learnable concepts, notable facts, procedures, patterns, and insights — skip obvious/trivial content.";

    let user_prompt = format!("Text:\n{section_text}");

    let messages = vec![
        Message::System {
            content: system_prompt.to_string(),
        },
        Message::User {
            content: UserContent::Text(user_prompt),
        },
    ];

    let params = ChatParams::new(provider.default_model())
        .with_max_tokens(max_tokens)
        .with_temperature(0.2);

    let response = provider.chat(&messages, None, &params).await?;

    let content = response.content.unwrap_or_default();
    parse_extraction_response(&content)
}

/// Parse the LLM response content as a JSON array of extracted atoms.
///
/// Handles both raw JSON arrays and markdown-fenced code blocks.
fn parse_extraction_response(content: &str) -> common::Result<Vec<ExtractedAtom>> {
    let trimmed = content.trim();

    // Try direct JSON parse first
    if let Ok(atoms) = serde_json::from_str::<Vec<ExtractedAtom>>(trimmed) {
        return Ok(atoms);
    }

    // Strip markdown fences and extract JSON array using common helpers
    let cleaned = common::helpers::strip_llm_fences(trimmed);
    let json_str = common::helpers::extract_json_array(cleaned);
    if let Ok(atoms) = serde_json::from_str::<Vec<ExtractedAtom>>(json_str) {
        return Ok(atoms);
    }

    warn!("failed to parse extraction response as JSON array");
    debug!(content, "unparseable extraction response");
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_sha256() {
        let hash = hex_sha256("hello world");
        assert_eq!(hash.len(), 64); // SHA-256 = 32 bytes = 64 hex chars
                                    // Stable hash
        assert_eq!(hex_sha256("hello world"), hex_sha256("hello world"));
        assert_ne!(hex_sha256("hello world"), hex_sha256("hello world!"));
    }

    #[test]
    fn test_split_by_headings() {
        let content = "## Introduction\n\
            This is a longer introduction section that contains enough words to pass the minimum \
            threshold for extraction. We need at least thirty words to make it worthwhile for the \
            LLM to process this section.\n\n\
            ## Details\n\
            Here are more details about the topic. This section also needs to be long enough to \
            pass the word count threshold. Let us add some more content here to ensure we meet \
            the minimum requirement for extraction.";

        let sections = split_into_sections(content);
        assert_eq!(sections.len(), 2);
        assert!(sections[0].starts_with("## Introduction"));
        assert!(sections[1].starts_with("## Details"));
    }

    #[test]
    fn test_split_no_headings_chunked() {
        // Generate > 200 words with no headings
        let words: Vec<&str> = (0..250).map(|_| "word").collect();
        let content = words.join(" ");

        let sections = split_into_sections(&content);
        assert_eq!(sections.len(), 2); // 200 + 50
    }

    #[test]
    fn test_split_short_content_skipped() {
        let content = "Just a few words here.";
        let sections = split_into_sections(content);
        assert!(sections.is_empty());
    }

    #[test]
    fn test_parse_extraction_response_valid_json() {
        let json = r#"[
            {"subject":"Rust ownership","atomType":"concept","domain":"software-engineering","sourceContext":"Rust uses ownership for memory safety"},
            {"subject":"Borrow checker","atomType":"concept","domain":"software-engineering","sourceContext":"The borrow checker enforces rules"}
        ]"#;

        let atoms = parse_extraction_response(json).unwrap();
        assert_eq!(atoms.len(), 2);
        assert_eq!(atoms[0].subject, "Rust ownership");
        assert_eq!(atoms[0].atom_type, "concept");
        assert_eq!(atoms[1].subject, "Borrow checker");
    }

    #[test]
    fn test_parse_extraction_response_fenced() {
        let content = "Here are the concepts:\n```json\n[\n{\"subject\":\"Test\",\"atomType\":\"fact\",\"domain\":\"test\",\"sourceContext\":\"testing\"}\n]\n```";
        let atoms = parse_extraction_response(content).unwrap();
        assert_eq!(atoms.len(), 1);
        assert_eq!(atoms[0].subject, "Test");
    }

    #[test]
    fn test_parse_extraction_response_garbage() {
        let content = "I couldn't extract anything useful.";
        let atoms = parse_extraction_response(content).unwrap();
        assert!(atoms.is_empty());
    }

    #[test]
    fn test_word_count() {
        assert_eq!(word_count(""), 0);
        assert_eq!(word_count("hello world"), 2);
        assert_eq!(word_count("  spaced   out  "), 2);
    }

    #[test]
    fn test_normalize_domain_aliases() {
        // Full-path aliases
        assert_eq!(
            normalize_domain("deep-learning"),
            "machine-learning/deep-learning"
        );
        assert_eq!(normalize_domain("nlp"), "machine-learning/nlp");
        assert_eq!(
            normalize_domain("reinforcement-learning"),
            "machine-learning/reinforcement-learning"
        );
        assert_eq!(
            normalize_domain("linear-algebra"),
            "mathematics/linear-algebra"
        );
        assert_eq!(normalize_domain("statistics"), "mathematics/statistics");
        assert_eq!(
            normalize_domain("database-systems"),
            "software-engineering/databases"
        );
        assert_eq!(
            normalize_domain("distributed-systems"),
            "software-engineering/distributed-systems"
        );
        assert_eq!(
            normalize_domain("machine-learning-systems"),
            "machine-learning/systems"
        );
        assert_eq!(
            normalize_domain("statistics/machine-learning"),
            "machine-learning/statistics"
        );
        assert_eq!(normalize_domain("system"), "software-engineering/general");
        assert_eq!(normalize_domain("general"), "general/uncategorized");
    }

    #[test]
    fn test_normalize_domain_parent_aliases() {
        // Parent remapping
        assert_eq!(
            normalize_domain("programming-languages/rust"),
            "software-engineering/rust"
        );
        assert_eq!(
            normalize_domain("ai/transformers"),
            "machine-learning/transformers"
        );
        assert_eq!(normalize_domain("math/calculus"), "mathematics/calculus");
        assert_eq!(normalize_domain("ml/nlp"), "machine-learning/nlp");
    }

    #[test]
    fn test_normalize_domain_child_aliases() {
        // Bare language/tool names
        assert_eq!(normalize_domain("rust"), "software-engineering/rust");
        assert_eq!(normalize_domain("python"), "software-engineering/python");
        assert_eq!(
            normalize_domain("pytorch"),
            "machine-learning/deep-learning"
        );
    }

    #[test]
    fn test_normalize_domain_separator_normalization() {
        // Colon separator (software-engineering:rust → software-engineering/rust)
        assert_eq!(
            normalize_domain("software-engineering:rust"),
            "software-engineering/rust"
        );
        assert_eq!(normalize_domain("language:ja"), "language/ja");
    }

    #[test]
    fn test_normalize_domain_already_canonical() {
        // Already in parent/child format — pass through
        assert_eq!(
            normalize_domain("machine-learning/deep-learning"),
            "machine-learning/deep-learning"
        );
        assert_eq!(
            normalize_domain("software-engineering/databases"),
            "software-engineering/databases"
        );
        assert_eq!(normalize_domain("finance/investing"), "finance/investing");
    }

    #[test]
    fn test_normalize_domain_unknown_single() {
        // Unknown single segment → {segment}/general
        assert_eq!(normalize_domain("biology"), "biology/general");
        assert_eq!(normalize_domain("philosophy"), "philosophy/general");
    }

    #[test]
    fn test_normalize_domain_edge_cases() {
        assert_eq!(normalize_domain(""), "general/uncategorized");
        assert_eq!(
            normalize_domain("  Deep-Learning  "),
            "machine-learning/deep-learning"
        );
        assert_eq!(normalize_domain("NLP"), "machine-learning/nlp");
    }
}
