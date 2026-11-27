//! The last positioning run of the document, kept so that everything asking where the setup
//! stands is answered from one run per change of the model.

use std::sync::Arc;

use opossum_core::{error::OpmResult, opm_document::PositioningRun};

use crate::live_document::DocumentGuard;

/// The positioning run of the document, with the revision of the document it was made at (see
/// [`LiveDocument`](crate::live_document::LiveDocument)).
///
/// The run is reused only while the document is still at that revision: any change since, a
/// request or not, has raised it.
#[derive(Debug, Clone, Default)]
pub struct LastPositioning(Option<(u64, Arc<OpmResult<PositioningRun>>)>);

impl LastPositioning {
    /// Returns the positioning run of `document`: the one kept if the document has not changed
    /// since it was made, else a new one, which is kept from now on.
    ///
    /// # Arguments
    ///
    /// - `document`: the locked live document
    ///
    /// # Returns
    ///
    /// The run, or why the model cannot be placed at all (see
    /// [`OpmDocument::run_positioning`](opossum_core::opm_document::OpmDocument::run_positioning)).
    pub fn run_for(&mut self, document: &DocumentGuard<'_>) -> Arc<OpmResult<PositioningRun>> {
        let revision = document.revision();
        if let Some((kept_at, run)) = &self.0
            && *kept_at == revision
        {
            return Arc::clone(run);
        }
        let run = Arc::new(document.run_positioning());
        self.0 = Some((revision, Arc::clone(&run)));
        run
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::document::test::state_with_a_source_in_front_of_a_reference;
    use opossum_core::millimeter;

    /// An unchanged document is answered from the run kept, without positioning it again, also
    /// when it was locked again in between.
    #[test]
    fn an_unchanged_document_reuses_the_run() {
        let (app_state, _, _) = state_with_a_source_in_front_of_a_reference();
        let mut last = LastPositioning::default();
        let first = last.run_for(&app_state.document.lock());
        let second = last.run_for(&app_state.document.lock());
        assert!(Arc::ptr_eq(&first, &second));
    }

    /// A document changed without any request - as the optimizer writes its results - is
    /// positioned anew, and the new run follows the change.
    #[test]
    fn a_changed_document_is_positioned_anew() {
        let (app_state, _, _) = state_with_a_source_in_front_of_a_reference();
        let mut last = LastPositioning::default();
        let before = last.run_for(&app_state.document.lock());
        {
            let mut document = app_state.document.lock();
            let source = document.scenery().find_source_ports().unwrap()[0];
            document
                .scenery_mut()
                .update_connection_distance(source, "output_1", millimeter!(150.0))
                .unwrap();
        }

        let after = last.run_for(&app_state.document.lock());

        assert!(!Arc::ptr_eq(&before, &after));
        let farthest_z = |run: &Arc<OpmResult<PositioningRun>>| {
            run.as_ref()
                .as_ref()
                .unwrap()
                .placements()
                .into_iter()
                .filter_map(|(_, placement)| placement)
                .map(|placement| placement.translation().z.value)
                .fold(f64::NEG_INFINITY, f64::max)
        };
        assert!(farthest_z(&after) > farthest_z(&before));
    }
}
