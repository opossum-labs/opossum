//! What the 3D view shows, and what it has been asked to do.
//!
//! Both of these are set from outside the view - from the menu bar, which sits on the other side of
//! the component tree - and read inside it; see [`SCENE_VIEW_CONTROLS`](crate::SCENE_VIEW_CONTROLS)
//! and [`SCENE_VIEW_REQUEST`](crate::SCENE_VIEW_REQUEST).

/// Which parts of the 3D view are shown, and how the light in it is drawn.
///
/// State, not actions: every field here is something that stays the way it was set, and every one of
/// them can be answered by looking at the view. The three things that only *happen* are
/// [`SceneViewRequest`]s instead.
#[derive(Clone, Copy, Debug, PartialEq)]
// Four independent switches, each with its own entry in the menu. They are state, not arguments:
// there is no combination of them that is not a legitimate view, so there is nothing to fold into an
// enum.
#[allow(clippy::struct_excessive_bools)]
pub struct SceneViewControls {
    /// Whether the optical table under the setup is shown. Mirrors `ViewerOptions::ground`.
    pub table: bool,
    /// Whether the corner orientation gizmo is shown. Mirrors `ViewerOptions::orientation_gizmo`.
    pub axes: bool,
    /// Whether the optical axis is shown. Mirrors the `visible` flag of the axis object.
    pub beam_axis: bool,
    /// Whether the light of the model's sources is shown. Off by default: unlike the axis it costs a
    /// real trace on every model change. While off, the rays object is not in the view's object list
    /// at all, so nothing is fetched for it.
    pub rays: bool,
    /// The most rays drawn per source; the backend caps it. A new number is a new trace, which is why
    /// the control that sets it commits on Enter rather than per keystroke.
    pub max_rays: usize,
    /// How opaque the drawn light - axis and rays alike - is. Faint by default, because a dense
    /// bundle reads as a solid block at full opacity; `0.0` draws no light at all.
    pub opacity: f32,
}

impl Default for SceneViewControls {
    fn default() -> Self {
        Self {
            table: true,
            axes: true,
            beam_axis: true,
            rays: false,
            max_rays: 500,
            opacity: 0.2,
        }
    }
}

/// Something the 3D view is asked to do once, rather than a state it is asked to be in.
///
/// These three need what only the view itself holds - its model resource and its viewer handle - so
/// asking for one is all a caller outside the view can do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneViewRequest {
    /// Ask the backend for the model again, at once.
    Refresh,
    /// Frame all components.
    FitView,
    /// Return the camera to its starting pose.
    ResetCamera,
}
