//! The 3D view of the current model.

mod adapter;
mod scene_view_component;

pub use adapter::{aux_table_object, objects_of, reveal_action_for_pick};
pub use scene_view_component::SceneView;
