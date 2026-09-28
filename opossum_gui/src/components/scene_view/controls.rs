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
    /// How opaque the drawn rays are. Faint by default, because a dense bundle reads as a solid block
    /// at full opacity; `0.0` draws no rays at all. The optical axis is not affected - it is always
    /// drawn solid, being the one line every component is placed on.
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

/// A key the 3D view acts on while its viewport holds focus, paired with the action it triggers.
///
/// The view's keys are deliberately local to it - they fire only while the viewport is focused, so
/// they must not join the global
/// [`SHORTCUTS`](crate::components::short_cuts::SHORTCUTS) table, which feeds a document-wide listener
/// that would steal the same letters from every other panel. This type is the single place that pairs
/// a key with its action; the view's key handler reads it, and the 3D View menu will show the key
/// beside the matching entry. Each variant is one of the actions the menu already offers - two that
/// move the camera ([`SceneViewRequest`]) and four that flip a switch in [`SceneViewControls`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneShortcut {
    /// Frame all components - the menu's **Fit view**.
    Fit,
    /// Return the camera to its starting pose - the menu's **Reset camera**.
    Reset,
    /// Show or hide the optical table - the menu's **Table**.
    Table,
    /// Show or hide the orientation gizmo - the menu's **Axes**.
    Axes,
    /// Show or hide the optical axis - the menu's **Beam axis**.
    Beam,
    /// Show or hide the traced rays - the menu's **Rays**.
    Rays,
}

impl SceneShortcut {
    /// The shortcut a printable key selects, if any.
    ///
    /// `Rays` is on `L` (for *light*) rather than its own initial, because `R` is kept for `Reset` -
    /// the action reached for more often.
    ///
    /// # Arguments
    ///
    /// * `key` - The character the key produced, as the browser reports it (e.g. `"f"`). Matched
    ///   case-insensitively, so a held Shift or Caps Lock does not change the outcome.
    ///
    /// # Returns
    ///
    /// The matching shortcut, or `None` for a key that is not bound.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        match key.to_ascii_lowercase().as_str() {
            "f" => Some(Self::Fit),
            "r" => Some(Self::Reset),
            "t" => Some(Self::Table),
            "a" => Some(Self::Axes),
            "b" => Some(Self::Beam),
            "l" => Some(Self::Rays),
            _ => None,
        }
    }

    /// The key label shown beside this shortcut's entry in the 3D View menu, e.g. `"F"`.
    ///
    /// The inverse of [`from_key`](Self::from_key) for the exact character it reports, so the menu
    /// hint and the key the handler acts on can never drift apart.
    ///
    /// # Returns
    ///
    /// The single upper-case letter that triggers this shortcut.
    #[must_use]
    pub const fn key_hint(self) -> &'static str {
        match self {
            Self::Fit => "F",
            Self::Reset => "R",
            Self::Table => "T",
            Self::Axes => "A",
            Self::Beam => "B",
            Self::Rays => "L",
        }
    }
}
