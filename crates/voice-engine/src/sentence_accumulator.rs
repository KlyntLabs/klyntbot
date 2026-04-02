/// Buffers LLM streaming text chunks and yields complete sentences at punctuation boundaries.
///
/// Sits between the LLM streaming output and the TTS engine. The LLM delivers text token-by-token;
/// the TTS engine needs complete sentences. `SentenceAccumulator` bridges this gap by detecting
/// sentence boundaries (`.`, `!`, `?`, `。`, `！`, `？`) and yielding sentences only when they
/// meet the minimum length threshold (to avoid splitting on abbreviations like "Dr.").
pub struct SentenceAccumulator {
    buffer: String,
    min_sentence_len: usize,
}

/// Returns true if the character is a sentence-ending punctuation mark (ASCII or CJK).
fn is_sentence_end(c: char) -> bool {
    matches!(c, '.' | '!' | '?' | '。' | '！' | '？')
}

impl SentenceAccumulator {
    /// Creates a new accumulator.
    ///
    /// `min_sentence_len` is the minimum byte length (inclusive) up to and including the
    /// punctuation mark for a sentence boundary to be recognised. This prevents splitting
    /// on abbreviations such as "Dr." when `min_sentence_len` is, e.g., 10. Note: for
    /// multi-byte UTF-8 characters (CJK), this effectively uses a lower character threshold.
    pub fn new(min_sentence_len: usize) -> Self {
        Self {
            buffer: String::new(),
            min_sentence_len,
        }
    }

    /// Appends a text chunk from LLM streaming into the internal buffer.
    pub fn push(&mut self, chunk: &str) {
        self.buffer.push_str(chunk);
    }

    /// Returns and removes a complete sentence from the front of the buffer if one is available.
    ///
    /// A complete sentence is defined as text ending with a sentence-ending punctuation character
    /// (`. ! ? 。 ！ ？`) that is either:
    /// - immediately followed by whitespace or another CJK/non-ASCII character, or
    /// - at the end of the buffer
    ///
    /// For ASCII punctuation (`. ! ?`), the character immediately after must be whitespace or
    /// end-of-buffer (to avoid splitting on decimals like "3.14" or mid-word punctuation).
    /// For CJK punctuation (`。 ！ ？`), any following character is acceptable because CJK
    /// prose does not use spaces between sentences.
    ///
    /// Additionally, the byte length of the text up to (and including) the punctuation character
    /// must be >= `min_sentence_len`.
    ///
    /// After extraction the remaining buffer text is left-trimmed of whitespace. The returned
    /// sentence is also left-trimmed so that leading whitespace from the previous extraction is
    /// not included.
    ///
    /// Returns `None` if no complete sentence is available yet.
    pub fn take_sentence(&mut self) -> Option<String> {
        // Iterate over character indices to find a valid sentence boundary.
        let mut chars = self.buffer.char_indices().peekable();

        while let Some((byte_pos, ch)) = chars.next() {
            if !is_sentence_end(ch) {
                continue;
            }

            // byte_pos is the start of `ch`; end of `ch` in bytes:
            let end_byte = byte_pos + ch.len_utf8();

            // Check length threshold — measured in bytes up to and including the punctuation.
            if end_byte < self.min_sentence_len {
                continue;
            }

            let is_cjk_punct = matches!(ch, '。' | '！' | '？');

            // Check what follows.
            let boundary = match chars.peek() {
                None => {
                    // Punctuation is at end of buffer — always a valid boundary.
                    true
                }
                Some(&(_, next_ch)) if next_ch.is_whitespace() => {
                    // Any punctuation followed by whitespace is a valid boundary.
                    true
                }
                Some(&(_, _)) if is_cjk_punct => {
                    // CJK punctuation can be followed by any character (no space needed).
                    true
                }
                _ => {
                    // ASCII punctuation followed by a non-whitespace character (e.g. "3.14") —
                    // not a sentence boundary.
                    false
                }
            };

            if boundary {
                let sentence = self.buffer[..end_byte].trim_start().to_owned();
                let remainder = self.buffer[end_byte..].trim_start().to_owned();
                self.buffer = remainder;
                return Some(sentence);
            }
        }

        None
    }

    /// Returns any remaining text in the buffer (used at end-of-stream).
    ///
    /// Returns `None` if the buffer is empty or contains only whitespace.
    /// After calling this the buffer is cleared.
    pub fn flush(&mut self) -> Option<String> {
        let trimmed = self.buffer.trim();
        if trimmed.is_empty() {
            self.buffer.clear();
            return None;
        }
        let result = trimmed.to_owned();
        self.buffer.clear();
        Some(result)
    }

    /// Returns `true` if the buffer is empty or contains only whitespace.
    pub fn is_empty(&self) -> bool {
        self.buffer.trim().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test 1: basic multi-chunk sentence assembly
    #[test]
    fn single_sentence_from_chunks() {
        let mut acc = SentenceAccumulator::new(1);
        acc.push("Hello ");
        acc.push("world.");
        let result = acc.take_sentence();
        assert_eq!(result, Some("Hello world.".to_owned()));
        assert!(acc.is_empty());
    }

    // Test 2: two sentences from a single push — yields in order
    #[test]
    fn yields_at_period_followed_by_space() {
        let mut acc = SentenceAccumulator::new(1);
        acc.push("Hello world. How are you?");
        let first = acc.take_sentence();
        assert_eq!(first, Some("Hello world.".to_owned()));
        let second = acc.take_sentence();
        assert_eq!(second, Some("How are you?".to_owned()));
        assert!(acc.is_empty());
    }

    // Test 3: min_sentence_len prevents splitting on short text
    #[test]
    fn respects_min_length() {
        let mut acc = SentenceAccumulator::new(20);
        // "Hi." is only 3 bytes — below threshold.
        acc.push("Hi. ");
        assert_eq!(acc.take_sentence(), None);

        // Now push more text until we have >= 20 bytes before a period.
        acc.push("This is a longer sentence.");
        // "Hi. This is a longer sentence." — the '.' after "sentence" is at byte 30 (>= 20).
        // But the '.' after "Hi" is only at byte 3 — below threshold.
        // After the first take_sentence call returned None and we pushed more text the buffer is
        // "Hi. This is a longer sentence."
        // The first boundary candidate is at byte 3 (< 20, skipped).
        // The second is at byte 30 (>= 20) — should fire.
        let result = acc.take_sentence();
        assert_eq!(result, Some("Hi. This is a longer sentence.".to_owned()));
    }

    // Test 4: CJK sentence endings
    #[test]
    fn chinese_sentence_endings() {
        let mut acc = SentenceAccumulator::new(1);
        acc.push("你好世界。这是测试。");
        let first = acc.take_sentence();
        assert_eq!(first, Some("你好世界。".to_owned()));
        let second = acc.take_sentence();
        assert_eq!(second, Some("这是测试。".to_owned()));
        assert!(acc.is_empty());
    }

    // Test 5: incomplete text — take_sentence returns None, flush returns remainder
    #[test]
    fn flush_returns_remainder() {
        let mut acc = SentenceAccumulator::new(1);
        acc.push("incomplete thought");
        assert_eq!(acc.take_sentence(), None);
        let flushed = acc.flush();
        assert_eq!(flushed, Some("incomplete thought".to_owned()));
        assert!(acc.is_empty());
    }

    // Test 6: flush on empty/whitespace-only buffer returns None
    #[test]
    fn flush_empty_returns_none() {
        let mut acc = SentenceAccumulator::new(1);
        assert_eq!(acc.flush(), None);

        // Also check whitespace-only
        acc.push("   ");
        assert_eq!(acc.flush(), None);
    }

    // Test 7: exclamation and question marks — three sentences
    #[test]
    fn exclamation_and_question_marks() {
        let mut acc = SentenceAccumulator::new(1);
        acc.push("Wow! Really? Yes.");
        let s1 = acc.take_sentence();
        assert_eq!(s1, Some("Wow!".to_owned()));
        let s2 = acc.take_sentence();
        assert_eq!(s2, Some("Really?".to_owned()));
        let s3 = acc.take_sentence();
        assert_eq!(s3, Some("Yes.".to_owned()));
        assert!(acc.is_empty());
    }

    // Test 8: incremental chunks — take_sentence called after each push
    #[test]
    fn incremental_chunks() {
        let mut acc = SentenceAccumulator::new(1);

        acc.push("First");
        assert_eq!(acc.take_sentence(), None);

        acc.push(" sentence.");
        assert_eq!(acc.take_sentence(), Some("First sentence.".to_owned()));

        acc.push(" Second");
        assert_eq!(acc.take_sentence(), None);

        acc.push(" sentence!");
        assert_eq!(acc.take_sentence(), Some("Second sentence!".to_owned()));

        assert!(acc.is_empty());
    }

    // Test 9: "Dr." is below min_sentence_len=10, does not split
    #[test]
    fn abbreviations_below_min_length_not_split() {
        let mut acc = SentenceAccumulator::new(10);
        // "Dr." is at byte position 3 — below threshold of 10.
        acc.push("Dr. Smith arrived. ");
        // "Dr." → 3 bytes < 10, skipped.
        // "Dr. Smith arrived." → 19 bytes >= 10, fires.
        let result = acc.take_sentence();
        assert_eq!(result, Some("Dr. Smith arrived.".to_owned()));
        assert!(acc.is_empty());
    }
}
