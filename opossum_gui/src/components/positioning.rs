//! What the positioning of the model found, kept up to date for the whole GUI.

use std::collections::HashSet;

use dioxus::prelude::*;
use opossum_core::types::api_types::PositioningSummary;
use uuid::Uuid;

use crate::{
    CONNECTIONS_PLACING_BY_DISTANCE, OPOSSUM_UI_LOGS, SCENE_REVISION, api, api::eval_action_run,
    utils::debounce,
};

/// Fetch what the positioning of the model found whenever the model changes, and hand it to the
/// globals that show it.
///
/// The backend positions the model once per change, whether a 3D view is open or not, so the
/// editor knows the placement from the start: which distances into references place something
/// (see [`CONNECTIONS_PLACING_BY_DISTANCE`]), and why the positioning stops, logged once each time
/// the reason changes. A burst of edits is fetched once, after it settles, as the 3D view does.
///
/// Called once, by the layout that is mounted for as long as the app runs.
pub fn use_positioning() {
    let mut fetches = use_signal(|| 0_usize);
    let mut stop_reason = use_signal(|| None::<String>);
    let summary = use_resource(move || async move {
        SCENE_REVISION();
        if *fetches.peek() > 0 {
            debounce().await;
        }
        api::get_positioning_summary().await
    });
    use_effect(move || {
        if let Some(fetched) = &*summary.read() {
            eval_action_run(
                fetched.clone(),
                Some(move |fetched: PositioningSummary| {
                    *fetches.write() += 1;
                    // Written only when it differs, so the edges re-render only then.
                    let placing = fetched.connections_placing_by_distance.map(|connections| {
                        connections
                            .iter()
                            .map(|c| (c.src_uuid(), c.src_port().to_owned()))
                            .collect::<HashSet<(Uuid, String)>>()
                    });
                    if *CONNECTIONS_PLACING_BY_DISTANCE.peek() != placing {
                        *CONNECTIONS_PLACING_BY_DISTANCE.write() = placing;
                    }
                    if *stop_reason.peek() != fetched.stop_reason {
                        if let Some(reason) = &fetched.stop_reason {
                            OPOSSUM_UI_LOGS
                                .write()
                                .add_log(&format!("The model cannot be placed: {reason}"));
                        }
                        stop_reason.set(fetched.stop_reason);
                    }
                }),
            );
        }
    });
}
