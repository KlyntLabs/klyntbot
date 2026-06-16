use std::collections::HashMap;

use desktop_shared::commands::{
    LinkSuggestionResponse, NoteSuggestionsResponse, ScoredNoteResponse,
};
use desktop_shared::errors::ApiError;

use super::converters::note_row_to_response;
use crate::state::AppCore;

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn note_suggestions(
        &self,
        note_id: &str,
    ) -> Result<NoteSuggestionsResponse, ApiError> {
        // Gather all scores into a unified map: note_id -> (total_weighted_score, reasons)
        let mut scores: HashMap<String, (f64, Vec<String>)> = HashMap::new();

        // 1. Semantic similarity (weight 0.4)
        if let Some(handler) = self.note_embedding_handler() {
            if let Ok(Some(note)) = self.note_repo.get_note(note_id).await {
                let text = format!("{} {}", note.title, note.body);
                if let Ok(query_vec) = handler.embed_query(&text).await {
                    if let Ok(similar) = handler.search_similar(&query_vec, 15, 0.3).await {
                        for (id, sim_score) in similar {
                            if id == note_id {
                                continue;
                            }
                            let entry = scores.entry(id).or_insert((0.0, Vec::new()));
                            entry.0 += sim_score * 0.4;
                            entry.1.push("semantically similar".to_string());
                        }
                    }
                }
            }
        }

        // 2. Structural holes (weight 0.25)
        if let Ok(holes) = self.note_repo.find_structural_holes(note_id).await {
            let max_count = holes.first().map(|(_, c)| *c as f64).unwrap_or(1.0);
            for (id, count) in holes {
                let normalized = count as f64 / max_count;
                let entry = scores.entry(id).or_insert((0.0, Vec::new()));
                entry.0 += normalized * 0.25;
                entry.1.push(format!("{count} shared neighbors"));
            }
        }

        // 3. Entity co-occurrence (weight 0.2)
        if let Ok(coocs) = self.note_repo.find_entity_cooccurrences(note_id).await {
            let max_count = coocs.first().map(|(_, c)| *c as f64).unwrap_or(1.0);
            for (id, count) in coocs {
                let normalized = count as f64 / max_count;
                let entry = scores.entry(id).or_insert((0.0, Vec::new()));
                entry.0 += normalized * 0.2;
                entry.1.push(format!("{count} shared entities"));
            }
        }

        // 4. Tag overlap (weight 0.15)
        if let Ok(overlaps) = self.note_repo.find_tag_overlaps(note_id).await {
            let max_count = overlaps.first().map(|(_, c)| *c as f64).unwrap_or(1.0);
            for (id, count) in overlaps {
                let normalized = count as f64 / max_count;
                let entry = scores.entry(id).or_insert((0.0, Vec::new()));
                entry.0 += normalized * 0.15;
                entry.1.push(format!("{count} shared tags"));
            }
        }

        // Sort by total score descending
        let mut ranked: Vec<(String, f64, Vec<String>)> = scores
            .into_iter()
            .map(|(id, (score, reasons))| (id, score, reasons))
            .collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Get existing outgoing link target IDs to separate related from link suggestions
        let existing_links: Vec<String> = self
            .note_repo
            .get_links_from(note_id)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|link| link.target_id)
            .collect();

        // Hydrate top results with full note data
        let mut related_notes = Vec::new();
        let mut link_suggestions = Vec::new();

        for (id, score, reasons) in ranked.iter().take(10) {
            if let Ok(Some(row)) = self.note_repo.get_note(id).await {
                let tags = self.note_repo.get_tags(id).await.unwrap_or_default();
                let response = note_row_to_response(&row, tags);
                let reason = reasons.join(", ");

                if related_notes.len() < 5 {
                    related_notes.push(ScoredNoteResponse {
                        note: response.clone(),
                        score: *score,
                        reason: reason.clone(),
                    });
                }

                // Link suggestions: notes that aren't already linked
                if !existing_links.contains(id) && link_suggestions.len() < 3 {
                    link_suggestions.push(LinkSuggestionResponse {
                        note: response,
                        score: *score,
                        reason,
                    });
                }
            }
        }

        // Suggested tags: find popular tags from related notes that current note doesn't have
        let current_tags: Vec<String> = self.note_repo.get_tags(note_id).await.unwrap_or_default();
        let mut tag_counts: HashMap<String, usize> = HashMap::new();
        for scored in &related_notes {
            for tag in &scored.note.tags {
                if !current_tags.contains(tag) {
                    *tag_counts.entry(tag.clone()).or_default() += 1;
                }
            }
        }
        let mut suggested_tags: Vec<(String, usize)> = tag_counts.into_iter().collect();
        suggested_tags.sort_by_key(|b| std::cmp::Reverse(b.1));
        let suggested_tags: Vec<String> =
            suggested_tags.into_iter().take(5).map(|(t, _)| t).collect();

        Ok(NoteSuggestionsResponse {
            related_notes,
            link_suggestions,
            suggested_tags,
        })
    }
}
