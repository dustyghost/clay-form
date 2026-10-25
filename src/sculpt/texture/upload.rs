//! Ephemeral dirty history. Revisions identify a branch of image edits, including undo.
use std::{
    collections::VecDeque,
    ops::Range,
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Clone, Debug, Default)]
pub(crate) struct TextureUpload {
    revision: u64,
    changes: VecDeque<(u64, u64, Range<usize>)>,
}
// Upload bookkeeping is not document content and must not affect history equality.
impl PartialEq for TextureUpload {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl TextureUpload {
    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }
    pub(crate) fn record(&mut self, pixels: Range<usize>) {
        if pixels.is_empty() {
            return;
        }
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let next = NEXT.fetch_add(1, Ordering::Relaxed);
        self.changes.push_back((self.revision, next, pixels));
        self.revision = next;
        if self.changes.len() > 64 {
            self.changes.pop_front();
        }
    }
    /// None requests a full refresh (evicted history, replacement, or an undo branch).
    pub(crate) fn changes_since(&self, revision: u64) -> Option<Range<usize>> {
        if revision == self.revision {
            return Some(0..0);
        }
        let start = self
            .changes
            .iter()
            .position(|(from, _, _)| *from == revision)?;
        let mut range = usize::MAX..0;
        for (_, _, dirty) in self.changes.iter().skip(start) {
            range.start = range.start.min(dirty.start);
            range.end = range.end.max(dirty.end);
        }
        Some(range)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upload_history_tracks_dabs_and_rejects_undo_branches() {
        let mut image = TextureUpload::default();
        image.record(10..20);
        let revision = image.revision();
        let mut branch = image.clone();
        image.record(30..35);
        image.record(15..40);
        assert_eq!(image.changes_since(revision), Some(15..40));
        branch.record(50..51);
        assert_eq!(image.changes_since(branch.revision()), None);
        for i in 0..65 {
            image.record(i..i + 1);
        }
        assert_eq!(image.changes_since(revision), None);
    }
}
