/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

/// Given a list of keywords for a suggestion, returns a phrase that best
/// completes the user's query. This function uses two heuristics to pick the
/// best match:
///
/// 1. Find the first keyword in the list that has at least one more word than
///    the query, then trim the keyword up to the end of that word.
/// 2. If there isn't a keyword with more words, pick the keyword that forms the
///    longest suffix of the query. This might be the query itself.
pub fn full_keyword(query: &str, keywords: &[impl AsRef<str>]) -> String {
    let query_words_len = query.split_whitespace().count();
    let min_phrase_words_len = if query.ends_with(char::is_whitespace) {
        // If the query ends with a space, find a keyword with at least one more
        // word, so that the completed phrase can show a word after the space.
        query_words_len + 1
    } else {
        query_words_len
    };
    keywords
        .iter()
        .map(AsRef::as_ref)
        .filter(|phrase| phrase.starts_with(query))
        .map(|phrase| phrase.split_whitespace().collect::<Vec<_>>())
        .find(|phrase_words| phrase_words.len() > min_phrase_words_len)
        .map(|phrase_words| phrase_words[..min_phrase_words_len].join(" "))
        .unwrap_or_else(|| {
            keywords
                .iter()
                .map(AsRef::as_ref)
                .filter(|phrase| phrase.starts_with(query) && query.len() < phrase.len())
                .max_by_key(|phrase| phrase.trim().len())
                .unwrap_or(query)
                .to_owned()
        })
}

/// Helper for traversing over chunk sequences in a string.
///
/// Use [ChunkedString::iter] to get a [ChunkedStringIter], which can be used to iterate over a
/// sequence of (chunk, iter) pairs.  `chunk` is 1 or more words from the beginning of the string
/// and `iter` is another [ChunkedStringIter] to continue iterating over the rest of the string.
///
/// The intended use-case is a depth-first search to tokenize a string, where tokens can be
/// multiple words.  See `weather.rs` for a real-world use case.
pub struct ChunkedString {
    /// Source string that we're iterating over
    source: String,
    /// Start/end positions for words in the string
    word_boundaries: Vec<(usize, usize)>,
    max_size: usize,
}

impl ChunkedString {
    pub fn new(source: String, max_size: usize) -> Self {
        let mut word_boundaries = vec![];
        let mut word_start = None;
        for (pos, c) in source.chars().enumerate() {
            match (word_start, c.is_whitespace()) {
                (None, false) => word_start = Some(pos),
                (Some(start_pos), true) => {
                    word_boundaries.push((start_pos, pos));
                    word_start = None;
                }
                _ => (),
            }
        }
        if let Some(start_pos) = word_start {
            word_boundaries.push((start_pos, source.len()));
        }
        Self {
            source,
            word_boundaries,
            max_size,
        }
    }

    pub fn iter(&self) -> ChunkedStringIter<'_> {
        ChunkedStringIter {
            chunked_string: &self,
            start_pos: 0,
            current_pos: 0,
        }
    }
}

#[derive(Clone)]
pub struct ChunkedStringIter<'a> {
    chunked_string: &'a ChunkedString,
    // Initial and current position, the next chunk will have the range `start_pos..current_pos`
    start_pos: usize,
    current_pos: usize,
}

impl<'a> ChunkedStringIter<'a> {
    pub fn at_end(&self) -> bool {
        self.current_chunk().is_none()
    }

    fn current_chunk(&self) -> Option<&'a str> {
        if self.current_pos - self.start_pos >= self.chunked_string.max_size {
            // Past the max chunk size, return None
            None
        } else if let Some(end) = self.chunked_string.word_boundaries.get(self.current_pos) {
            // Normal case, get a chunk using the words in `self.start_pos..self.current_pos`

            // Never out of bounds because `start_pos <= current_pos`
            let start = self.chunked_string.word_boundaries[self.start_pos];
            // Never out of bounds because all word_boundaries are guaranteed to be inside the string
            Some(&self.chunked_string.source[start.0..end.1])
        } else {
            // Past the last word, return None
            None
        }
    }
}

impl<'a> Iterator for ChunkedStringIter<'a> {
    type Item = (&'a str, Self);

    fn next(&mut self) -> Option<Self::Item> {
        self.current_chunk().map(move |chunk| {
            self.current_pos += 1;
            (
                chunk,
                ChunkedStringIter {
                    chunked_string: self.chunked_string,
                    start_pos: self.current_pos,
                    current_pos: self.current_pos,
                },
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keywords_with_more_words() {
        assert_eq!(
            full_keyword(
                "moz",
                &[
                    "moz",
                    "mozi",
                    "mozil",
                    "mozill",
                    "mozilla",
                    "mozilla firefox"
                ]
            ),
            "mozilla".to_owned(),
        );
        assert_eq!(
            full_keyword(
                "mozilla",
                &[
                    "moz",
                    "mozi",
                    "mozil",
                    "mozill",
                    "mozilla",
                    "mozilla firefox"
                ]
            ),
            "mozilla".to_owned(),
        );
    }

    #[test]
    fn keywords_with_longer_phrase() {
        assert_eq!(
            full_keyword("moz", &["moz", "mozi", "mozil", "mozill", "mozilla"]),
            "mozilla".to_owned()
        );
        assert_eq!(
            full_keyword(
                "mozilla f",
                &["moz", "mozi", "mozil", "mozill", "mozilla firefox"]
            ),
            "mozilla firefox".to_owned()
        );
    }

    #[test]
    fn query_ends_with_space() {
        assert_eq!(
            full_keyword(
                "mozilla ",
                &["moz", "mozi", "mozil", "mozill", "mozilla firefox"]
            ),
            "mozilla firefox".to_owned()
        );
    }

    /// Test a ChunkedString by iterating through all nested chunks and creating a Vec of strings
    /// by joining them together.  This makes it easier to test.
    ///
    /// Each string will have the form "[chunk1]:[chunk2]:...[chunkn]".
    fn check_chunk_string(chunked: ChunkedString, mut correct_strings: Vec<&'static str>) {
        let mut all_chunks = vec![];
        fn recurse<'a>(
            all_chunks: &mut Vec<String>,
            current_chunk: &mut Vec<&'a str>,
            iter: ChunkedStringIter<'a>,
        ) {
            if iter.at_end() {
                if !current_chunk.is_empty() {
                    all_chunks.push(current_chunk.join(":"));
                }
            } else {
                for (chunk, child_iter) in iter {
                    current_chunk.push(chunk);
                    recurse(all_chunks, current_chunk, child_iter);
                    current_chunk.pop();
                }
            }
        }
        recurse(&mut all_chunks, &mut vec![], chunked.iter());
        all_chunks.sort();
        correct_strings.sort();
        assert_eq!(all_chunks, correct_strings);
    }

    #[test]
    fn test_chunked_string() -> anyhow::Result<()> {
        check_chunk_string(
            ChunkedString::new("a b c".into(), 3),
            vec!["a b c", "a b:c", "a:b c", "a:b:c"],
        );
        Ok(())
    }

    #[test]
    fn test_chunked_string_extra_whitespace() -> anyhow::Result<()> {
        // Extra whitespace on the end is ignored
        check_chunk_string(
            ChunkedString::new("  a b c  ".into(), 3),
            vec!["a b c", "a b:c", "a:b c", "a:b:c"],
        );
        // Extra whitespace in the middle appears in the chunks, but doesn't affect the word
        // splitting
        check_chunk_string(
            ChunkedString::new("a  b  c".into(), 3),
            vec!["a  b  c", "a  b:c", "a:b  c", "a:b:c"],
        );
        Ok(())
    }

    #[test]
    fn test_chunked_string_max_size() -> anyhow::Result<()> {
        check_chunk_string(
            ChunkedString::new("a b c".into(), 2),
            vec![
                // No "a b c", since that would be would be size=3
                "a b:c", "a:b c", "a:b:c",
            ],
        );
        check_chunk_string(
            ChunkedString::new("a b c".into(), 1),
            vec![
                // This is the only result, since each chunk can only have 1 word
                "a:b:c",
            ],
        );
        // Corner case: when max_size=0, then it should not generate any items
        check_chunk_string(ChunkedString::new("a b c".into(), 0), vec![]);
        Ok(())
    }
}
