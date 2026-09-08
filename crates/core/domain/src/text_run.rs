//! Evidence-based text completion, independent of operating system and Adapter.
//! A run is supplied by its owning renderer; proximity or character count is not
//! evidence that two callbacks belong to the same logical text.
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const MAX_TEXT_RUN_UNITS: usize = 16 * 1024;
const MAX_PENDING_RUNS: usize = 64;
const MAX_RECENT_RUNS: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TextRunKey {
    pub surface: u64,
    pub run: u64,
    pub epoch: u64,
}

impl TextRunKey {
    fn valid(self) -> bool {
        self.surface != 0 && self.run != 0 && self.epoch != 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextUse {
    /// The caller can replace this entire string in the current drawing call.
    Draw,
    /// The caller owns a restorable logical text object.
    Retained,
    /// Complete text is known, but there is no corresponding replacement site.
    Observe,
}

pub enum TextRunEvent<'a> {
    Complete {
        text: &'a [u16],
        usage: TextUse,
    },
    Fragment {
        key: TextRunKey,
        ordinal: u32,
        text: &'a [u16],
    },
    Finish {
        key: TextRunKey,
        parts: u32,
    },
    Cancel {
        key: TextRunKey,
    },
    /// Explicit evidence from a glyph rasterizer, never inferred from text length.
    GlyphRaster,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ResolvedText {
    pub text: String,
    pub usage: TextUse,
}

impl ResolvedText {
    #[must_use]
    pub fn can_replace(&self) -> bool {
        self.usage != TextUse::Observe
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum TextRunOutcome {
    Resolved(ResolvedText),
    Pending,
    Ignored,
    Rejected,
}

#[derive(Default)]
struct PendingRun {
    next: u32,
    units: Vec<u16>,
}

#[derive(Default)]
pub struct TextRunResolver {
    pending: BTreeMap<TextRunKey, PendingRun>,
    recent: BTreeSet<TextRunKey>,
    recent_order: VecDeque<TextRunKey>,
}

impl TextRunResolver {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn accept(&mut self, event: TextRunEvent<'_>) -> TextRunOutcome {
        match event {
            TextRunEvent::Complete { text, usage } => resolve(text, usage),
            TextRunEvent::GlyphRaster => TextRunOutcome::Ignored,
            TextRunEvent::Cancel { key } => {
                self.close(key);
                TextRunOutcome::Ignored
            }
            TextRunEvent::Finish { key, parts } => {
                let run = self.pending.remove(&key);
                self.close(key);
                match run {
                    Some(run) if key.valid() && parts > 0 && run.next == parts => {
                        // Reconstruction provides observation only. Past rendering
                        // callbacks are not an atomic replacement site.
                        resolve(&run.units, TextUse::Observe)
                    }
                    _ => TextRunOutcome::Rejected,
                }
            }
            TextRunEvent::Fragment { key, ordinal, text } => {
                if !key.valid() || text.is_empty() || text.len() > MAX_TEXT_RUN_UNITS {
                    self.close(key);
                    return TextRunOutcome::Rejected;
                }
                if self.recent.contains(&key) {
                    return TextRunOutcome::Ignored;
                }
                if !self.pending.contains_key(&key) {
                    if ordinal != 0 || self.pending.len() >= MAX_PENDING_RUNS {
                        self.close(key);
                        return TextRunOutcome::Rejected;
                    }
                    self.pending.insert(key, PendingRun::default());
                }
                let run = self.pending.get_mut(&key).expect("inserted run");
                if run.next != ordinal || run.units.len() + text.len() > MAX_TEXT_RUN_UNITS {
                    self.close(key);
                    return TextRunOutcome::Rejected;
                }
                run.next += 1; // A nonempty fragment consumes at least one bounded unit.
                run.units.extend_from_slice(text);
                TextRunOutcome::Pending
            }
        }
    }

    fn close(&mut self, key: TextRunKey) {
        self.pending.remove(&key);
        if self.recent.insert(key) {
            self.recent_order.push_back(key);
        }
        while self.recent_order.len() > MAX_RECENT_RUNS {
            if let Some(oldest) = self.recent_order.pop_front() {
                self.recent.remove(&oldest);
            }
        }
    }
}

fn resolve(units: &[u16], usage: TextUse) -> TextRunOutcome {
    if units.is_empty() || units.len() > MAX_TEXT_RUN_UNITS {
        return TextRunOutcome::Rejected;
    }
    match String::from_utf16(units) {
        Ok(text) if !text.contains('\0') => TextRunOutcome::Resolved(ResolvedText { text, usage }),
        _ => TextRunOutcome::Rejected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(surface: u64, run: u64) -> TextRunKey {
        TextRunKey {
            surface,
            run,
            epoch: 1,
        }
    }
    fn append(resolver: &mut TextRunResolver, key: TextRunKey, ordinal: u32, text: &str) {
        assert_eq!(
            resolver.accept(TextRunEvent::Fragment {
                key,
                ordinal,
                text: &text.encode_utf16().collect::<Vec<_>>()
            }),
            TextRunOutcome::Pending
        );
    }
    fn finish(resolver: &mut TextRunResolver, key: TextRunKey, parts: u32, expected: &str) {
        assert_eq!(
            resolver.accept(TextRunEvent::Finish { key, parts }),
            TextRunOutcome::Resolved(ResolvedText {
                text: expected.into(),
                usage: TextUse::Observe
            })
        );
    }
    #[test]
    fn interleaved_surfaces_and_controls_never_merge() {
        let mut resolver = TextRunResolver::default();
        append(&mut resolver, key(1, 1), 0, "开始");
        append(&mut resolver, key(2, 1), 0, "Cancel");
        append(&mut resolver, key(1, 2), 0, "是");
        append(&mut resolver, key(1, 1), 1, "游戏");
        finish(&mut resolver, key(1, 1), 2, "开始游戏");
        finish(&mut resolver, key(2, 1), 1, "Cancel");
        finish(&mut resolver, key(1, 2), 1, "是");
    }
    #[test]
    fn fragments_keep_spaces_repeated_characters_and_split_surrogates() {
        let mut resolver = TextRunResolver::default();
        append(&mut resolver, key(1, 1), 0, "A A ");
        assert_eq!(
            resolver.accept(TextRunEvent::Fragment {
                key: key(1, 1),
                ordinal: 1,
                text: &[0xd83d]
            }),
            TextRunOutcome::Pending
        );
        assert_eq!(
            resolver.accept(TextRunEvent::Fragment {
                key: key(1, 1),
                ordinal: 2,
                text: &[0xde00]
            }),
            TextRunOutcome::Pending
        );
        finish(&mut resolver, key(1, 1), 3, "A A 😀");
    }
    #[test]
    fn dropped_reordered_cancelled_or_unfinished_runs_do_not_publish() {
        let mut resolver = TextRunResolver::default();
        append(&mut resolver, key(1, 1), 0, "A");
        assert_eq!(
            resolver.accept(TextRunEvent::Fragment {
                key: key(1, 1),
                ordinal: 2,
                text: &[66]
            }),
            TextRunOutcome::Rejected
        );
        assert_eq!(
            resolver.accept(TextRunEvent::Finish {
                key: key(1, 1),
                parts: 3
            }),
            TextRunOutcome::Rejected
        );
        append(&mut resolver, key(1, 2), 0, "B");
        resolver.accept(TextRunEvent::Cancel { key: key(1, 2) });
        assert_eq!(
            resolver.accept(TextRunEvent::Finish {
                key: key(1, 2),
                parts: 1
            }),
            TextRunOutcome::Rejected
        );
        append(&mut resolver, key(2, 1), 0, "C");
        resolver.reset();
        assert_eq!(
            resolver.accept(TextRunEvent::Finish {
                key: key(2, 1),
                parts: 1
            }),
            TextRunOutcome::Rejected
        );
    }
    #[test]
    fn raster_evidence_is_distinct_from_a_valid_single_character_label() {
        let mut resolver = TextRunResolver::default();
        assert_eq!(
            resolver.accept(TextRunEvent::GlyphRaster),
            TextRunOutcome::Ignored
        );
        let TextRunOutcome::Resolved(text) = resolver.accept(TextRunEvent::Complete {
            text: &[0x662f],
            usage: TextUse::Draw,
        }) else {
            panic!("valid single label");
        };
        assert_eq!(text.text, "是");
        assert!(text.can_replace());
        let TextRunOutcome::Resolved(text) = resolver.accept(TextRunEvent::Complete {
            text: &[0x662f],
            usage: TextUse::Observe,
        }) else {
            panic!("measurement");
        };
        assert!(!text.can_replace());
    }
    #[test]
    fn epochs_and_completed_replays_remain_separate() {
        let mut resolver = TextRunResolver::default();
        let old = key(1, 1);
        let new = TextRunKey { epoch: 2, ..old };
        append(&mut resolver, old, 0, "Old");
        append(&mut resolver, new, 0, "New");
        finish(&mut resolver, new, 1, "New");
        finish(&mut resolver, old, 1, "Old");
        assert_eq!(
            resolver.accept(TextRunEvent::Fragment {
                key: old,
                ordinal: 0,
                text: &[65]
            }),
            TextRunOutcome::Ignored
        );
    }
    #[test]
    fn bounded_storage_never_publishes_truncated_text() {
        let mut resolver = TextRunResolver::default();
        for run in 1..=MAX_PENDING_RUNS as u64 {
            append(&mut resolver, key(1, run), 0, "A");
        }
        assert_eq!(
            resolver.accept(TextRunEvent::Fragment {
                key: key(2, 1),
                ordinal: 0,
                text: &[65]
            }),
            TextRunOutcome::Rejected
        );
        assert_eq!(
            resolver.accept(TextRunEvent::Fragment {
                key: key(1, 1),
                ordinal: 1,
                text: &vec![65; MAX_TEXT_RUN_UNITS]
            }),
            TextRunOutcome::Rejected
        );
        assert_eq!(
            resolver.accept(TextRunEvent::Finish {
                key: key(1, 1),
                parts: 2
            }),
            TextRunOutcome::Rejected
        );
    }
}
