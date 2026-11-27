//! The live document behind a lock that counts every change made through it.

use std::ops::{Deref, DerefMut};

use opossum_core::opm_document::OpmDocument;
use parking_lot::{Mutex, MutexGuard};

/// The document together with how often it was changed.
#[derive(Debug, Clone, Default)]
struct Revised {
    document: OpmDocument,
    revision: u64,
}

/// The live document, behind a lock whose guard raises a revision number whenever it hands out
/// mutable access.
///
/// Every change of the document needs that access, whoever makes it - a request, an undo, a file
/// loaded over it, or the optimizer writing its results from a background thread - so anything
/// derived from the document can tell by the revision whether it still matches (see
/// [`LastPositioning`](crate::positioning::LastPositioning)). Mutable access that changes nothing
/// raises it all the same; that only costs deriving something once more.
#[derive(Debug, Default)]
pub struct LiveDocument(Mutex<Revised>);

impl LiveDocument {
    /// Locks the document.
    ///
    /// # Returns
    ///
    /// The guard, which reads the document freely and raises its revision on mutable access.
    pub fn lock(&self) -> DocumentGuard<'_> {
        DocumentGuard(self.0.lock())
    }
}

impl Clone for LiveDocument {
    /// Copies the document with its revision, so that what was derived from it still matches.
    fn clone(&self) -> Self {
        Self(Mutex::new(self.0.lock().clone()))
    }
}

/// Access to the locked live document, see [`LiveDocument`].
pub struct DocumentGuard<'a>(MutexGuard<'a, Revised>);

impl DocumentGuard<'_> {
    /// Returns how often the document was changed so far.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.0.revision
    }
}

impl Deref for DocumentGuard<'_> {
    type Target = OpmDocument;

    fn deref(&self) -> &OpmDocument {
        &self.0.document
    }
}

impl DerefMut for DocumentGuard<'_> {
    /// Hands out the document to change and counts it as changed.
    fn deref_mut(&mut self) -> &mut OpmDocument {
        self.0.revision += 1;
        &mut self.0.document
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// Reading the document leaves its revision alone.
    #[test]
    fn reading_does_not_count_as_a_change() {
        let live = LiveDocument::default();
        let document = live.lock();
        let _ = document.scenery();
        assert_eq!(document.revision(), 0);
    }

    /// Every mutable access counts as a change, a replaced document as well.
    #[test]
    fn mutable_access_counts_as_a_change() {
        let live = LiveDocument::default();
        let mut document = live.lock();
        let _ = document.scenery_mut();
        *document = OpmDocument::default();
        assert_eq!(document.revision(), 2);
    }

    /// A copy keeps the revision of the document it copies.
    #[test]
    fn a_copy_keeps_the_revision() {
        let live = LiveDocument::default();
        let _ = live.lock().scenery_mut();
        assert_eq!(live.clone().lock().revision(), 1);
    }
}
