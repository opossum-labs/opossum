//! Which of the editor's two panes each tab sits in, and which tab each pane shows.
//!
//! Plain data with no signals in it, so the rules below can be tested without a running app.
//! `GraphEditor` holds one [`TabLayout`] in a signal and renders from it.
use dioxus::html::geometry::euclid::default::Point2D;
use uuid::Uuid;

/// The value the tab bar identifies the 3D view by.
///
/// Every other tab is named by the uuid of the graph it shows; this one is not a graph, so it needs
/// a name that no uuid can collide with.
const SCENE_TAB_VALUE: &str = "scene-3d";

/// The narrowest either pane may be dragged, in pixels - enough for a tab and the pane's button.
const MIN_PANE_WIDTH: f64 = 200.0;

/// How far, in pixels, a pressed tab has to travel before it counts as dragged rather than clicked.
const TAB_DRAG_THRESHOLD: f64 = 4.0;

/// One entry of the editor's tab bar.
///
/// A view type, deliberately not a key of `GraphsWorkspaceState`. The store is a map of graphs and
/// stays that way: its `active_tab` answers "which graph is being edited", which is what its several
/// dozen readers - the properties panel, the selection, undo targets - actually want, and what must
/// *not* change just because someone looks at the 3D view for a moment.
///
/// What this type adds is the other question, "which tab is in front", and having it as one value
/// is what lets every panel derive its visibility from a single comparison.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TabKey {
    /// The graph with this uuid.
    Graph(Uuid),
    /// The 3D view of the model.
    Scene,
}

impl TabKey {
    /// The string the tab bar identifies this tab by.
    pub fn value(self) -> String {
        match self {
            Self::Graph(id) => id.as_simple().to_string(),
            Self::Scene => SCENE_TAB_VALUE.to_owned(),
        }
    }

    /// Read back what [`Self::value`] wrote.
    ///
    /// # Returns
    ///
    /// The tab, or `None` for a value from neither of the two shapes.
    pub fn parse(value: &str) -> Option<Self> {
        if value == SCENE_TAB_VALUE {
            Some(Self::Scene)
        } else {
            Uuid::parse_str(value).ok().map(Self::Graph)
        }
    }
}

/// One of the editor's two panes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    /// The left pane - the only one while the editor is not split.
    Left,
    /// The right pane, which exists only while it holds a tab.
    Right,
}

impl Side {
    /// The pane across from this one.
    pub const fn other(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }

    /// The column of the editor's grid this pane occupies. The column between the two holds the
    /// handle that separates them.
    pub const fn grid_column(self) -> usize {
        match self {
            Self::Left => 1,
            Self::Right => 3,
        }
    }

    /// What moving a tab out of this pane is called, in the tab's context menu and on the button
    /// that does the same.
    ///
    /// # Arguments
    ///
    /// * `split` - whether the editor is split; if not, the move is what splits it.
    pub const fn move_label(self, split: bool) -> &'static str {
        match (split, self) {
            (false, _) => "Split right",
            (true, Self::Left) => "Move to right pane",
            (true, Self::Right) => "Move to left pane",
        }
    }
}

/// Where a tab can be dropped.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DropTarget {
    /// Into the pane on this side of a split editor.
    Pane(Side),
    /// Into a new pane on this side, splitting an editor that is not split yet.
    Split(Side),
}

impl DropTarget {
    /// Where the drop zone for this target sits in the editor's grid, as an inline style: the whole
    /// column of a pane, tab bar included, or one half of the unsplit editor's content.
    pub fn zone_style(self) -> String {
        match self {
            Self::Pane(side) => {
                format!("grid-column: {}; grid-row: 1 / span 2;", side.grid_column())
            }
            Self::Split(Side::Left) => {
                "grid-column: 1; grid-row: 2; justify-self: start; width: 50%;".to_owned()
            }
            Self::Split(Side::Right) => {
                "grid-column: 1; grid-row: 2; justify-self: end; width: 50%;".to_owned()
            }
        }
    }
}

/// A tab held down with the mouse, which becomes a drag once it has travelled far enough.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TabDrag {
    /// The tab held.
    pub tab: TabKey,
    /// Where it was pressed, in client coordinates.
    start: Point2D<f64>,
    /// Where the pointer is now, in client coordinates.
    pub pointer: Point2D<f64>,
    /// Whether it has travelled far enough to be a drag, rather than the start of a click.
    started: bool,
}

impl TabDrag {
    /// A tab just pressed at `at`, not yet dragged.
    pub const fn new(tab: TabKey, at: Point2D<f64>) -> Self {
        Self {
            tab,
            start: at,
            pointer: at,
            started: false,
        }
    }

    /// Follow the pointer to `at`. Once the tab has travelled past a few pixels it is being dragged,
    /// and it stays dragged even if the pointer returns to where it started.
    pub fn move_to(&mut self, at: Point2D<f64>) {
        self.pointer = at;
        self.started = self.started || (at - self.start).length() > TAB_DRAG_THRESHOLD;
    }

    /// Whether the tab is being dragged, rather than only pressed.
    pub const fn is_started(&self) -> bool {
        self.started
    }
}

/// How the open tabs are distributed over the editor's panes, and what each pane shows.
///
/// Every method takes the tabs that are currently open, in tab-bar order, rather than keeping its
/// own copy: the graphs are owned by the workspace store and the 3D view by `SCENE_VIEW_OPEN`, and a
/// second list here would only be one more thing to drift out of step with them.
#[derive(Clone, PartialEq, Debug)]
pub struct TabLayout {
    /// The pane of every tab placed so far, in the order they were placed. A tab not listed has not
    /// been seen yet; see [`Self::sync`].
    sides: Vec<(TabKey, Side)>,
    /// The tab last brought to the front of the left pane.
    front_left: Option<TabKey>,
    /// The tab last brought to the front of the right pane.
    front_right: Option<TabKey>,
    /// The left pane's share of the width while split. A share rather than a width in pixels, so
    /// both panes keep their proportions when the window is resized. Kept while the editor is not
    /// split, so splitting it again brings back the widths it had.
    ratio: f64,
}

impl Default for TabLayout {
    fn default() -> Self {
        Self {
            sides: Vec::new(),
            front_left: None,
            front_right: None,
            ratio: 0.5,
        }
    }
}

impl TabLayout {
    /// The pane `tab` sits in. A tab not placed yet counts as left, where everything starts.
    pub fn side_of(&self, tab: TabKey) -> Side {
        self.sides
            .iter()
            .find(|(t, _)| *t == tab)
            .map_or(Side::Left, |(_, side)| *side)
    }

    /// The open tabs of one pane.
    ///
    /// # Arguments
    ///
    /// * `side` - the pane asked for.
    /// * `open` - every open tab, in tab-bar order.
    ///
    /// # Returns
    ///
    /// The tabs of `open` that sit in `side`, in the same order.
    pub fn tabs_on(&self, side: Side, open: &[TabKey]) -> Vec<TabKey> {
        open.iter()
            .copied()
            .filter(|tab| self.side_of(*tab) == side)
            .collect()
    }

    /// Whether the editor is split, i.e. both panes hold an open tab.
    ///
    /// # Arguments
    ///
    /// * `open` - every open tab.
    pub fn is_split(&self, open: &[TabKey]) -> bool {
        [Side::Left, Side::Right]
            .iter()
            .all(|side| open.iter().any(|tab| self.side_of(*tab) == *side))
    }

    /// The tab one pane shows.
    ///
    /// # Arguments
    ///
    /// * `side` - the pane asked for.
    /// * `open` - every open tab.
    /// * `active` - the graph being edited (`GraphsWorkspaceState::active_tab`).
    ///
    /// # Returns
    ///
    /// The tab last brought to the front there, if it is still open in that pane; failing that the
    /// active graph, if it sits in that pane; failing that the pane's first tab. `None` only for a
    /// pane without any open tab.
    pub fn shown(&self, side: Side, open: &[TabKey], active: Uuid) -> Option<TabKey> {
        let here = |tab: &TabKey| open.contains(tab) && self.side_of(*tab) == side;
        self.front(side)
            .filter(here)
            .or_else(|| Some(TabKey::Graph(active)).filter(here))
            .or_else(|| open.iter().copied().find(here))
    }

    /// Bring the layout in line with the tabs that are open now.
    ///
    /// # Arguments
    ///
    /// * `open` - every open tab.
    /// * `opener` - the pane a newly opened tab joins: the one the user was working in when it
    ///   opened, so a group opens beside the graph it was opened from, and the 3D view beside the
    ///   graph being edited.
    ///
    /// # Returns
    ///
    /// The tabs placed by this call, i.e. the ones that were not open before.
    ///
    /// Closed tabs are forgotten, so a tab opened again later is placed afresh. If that leaves one
    /// pane empty, the editor is no longer split and everything goes back to the left pane.
    pub fn sync(&mut self, open: &[TabKey], opener: Side) -> Vec<TabKey> {
        self.sides.retain(|(tab, _)| open.contains(tab));
        for front in [&mut self.front_left, &mut self.front_right] {
            if front.is_some_and(|tab| !open.contains(&tab)) {
                *front = None;
            }
        }
        let placed: Vec<TabKey> = open
            .iter()
            .copied()
            .filter(|tab| !self.sides.iter().any(|(t, _)| t == tab))
            .collect();
        self.sides.extend(placed.iter().map(|tab| (*tab, opener)));
        self.unsplit_if_one_sided(open);
        placed
    }

    /// Make `tab` the one its pane shows.
    pub fn bring_to_front(&mut self, tab: TabKey) {
        *self.front_mut(self.side_of(tab)) = Some(tab);
    }

    /// Where `tab` can be dropped, given how the editor is laid out now.
    ///
    /// # Arguments
    ///
    /// * `tab` - the tab being dragged.
    /// * `open` - every open tab.
    ///
    /// # Returns
    ///
    /// For a split editor, the pane `tab` is not in. For an editor that is not split, a new pane
    /// on either side - unless `tab` is the only tab, which would leave nothing behind to split from.
    pub fn drop_targets(&self, tab: TabKey, open: &[TabKey]) -> Vec<DropTarget> {
        if self.is_split(open) {
            vec![DropTarget::Pane(self.side_of(tab).other())]
        } else if open.len() > 1 {
            vec![
                DropTarget::Split(Side::Left),
                DropTarget::Split(Side::Right),
            ]
        } else {
            Vec::new()
        }
    }

    /// Put `tab` where it was dropped, and show it there.
    ///
    /// # Arguments
    ///
    /// * `tab` - the tab dropped.
    /// * `target` - where it was dropped.
    /// * `open` - every open tab.
    ///
    /// Dropping a tab into the pane it already sits in changes nothing. Splitting to the left keeps
    /// `tab` where it is and moves every other tab into the new right pane, so that `tab` ends up
    /// alone on the left, as asked.
    pub fn drop(&mut self, tab: TabKey, target: DropTarget, open: &[TabKey]) {
        match target {
            DropTarget::Pane(side) if side == self.side_of(tab) => {}
            DropTarget::Pane(side) | DropTarget::Split(side @ Side::Right) => {
                self.move_tab(tab, side, open);
            }
            DropTarget::Split(Side::Left) => {
                for entry in &mut self.sides {
                    entry.1 = if entry.0 == tab {
                        Side::Left
                    } else {
                        Side::Right
                    };
                }
                self.front_right = self.front_left.filter(|front| *front != tab);
                self.unsplit_if_one_sided(open);
                self.bring_to_front(tab);
            }
        }
    }

    /// Move `tab` to the pane `side` and show it there.
    ///
    /// # Arguments
    ///
    /// * `tab` - the tab to move.
    /// * `side` - the pane it goes to.
    /// * `open` - every open tab.
    ///
    /// Moving the last tab out of a pane closes that pane; the tab then ends up in front of the one
    /// remaining pane.
    fn move_tab(&mut self, tab: TabKey, side: Side, open: &[TabKey]) {
        match self.sides.iter_mut().find(|(t, _)| *t == tab) {
            Some(entry) => entry.1 = side,
            None => self.sides.push((tab, side)),
        }
        self.unsplit_if_one_sided(open);
        self.bring_to_front(tab);
    }

    /// The left pane's share of the width while split.
    pub const fn ratio(&self) -> f64 {
        self.ratio
    }

    /// Resize the panes so the left one is `requested` pixels wide, as far as both stay usable.
    ///
    /// # Arguments
    ///
    /// * `requested` - the width the left pane is asked to have, in pixels.
    /// * `available` - the width both panes share, in pixels.
    ///
    /// Each pane keeps at least `MIN_PANE_WIDTH`; asked for less, the left pane stops there rather
    /// than collapsing. Too narrow for both minimums, or given a width that is not a number, the
    /// panes are split evenly.
    pub fn set_left_width(&mut self, requested: f64, available: f64) {
        self.ratio = if requested.is_finite()
            && available.is_finite()
            && available >= 2.0 * MIN_PANE_WIDTH
        {
            requested.clamp(MIN_PANE_WIDTH, available - MIN_PANE_WIDTH) / available
        } else {
            0.5
        };
    }

    /// The editor grid's `grid-template-columns` for this layout.
    ///
    /// # Arguments
    ///
    /// * `open` - every open tab.
    ///
    /// # Returns
    ///
    /// One column while the editor is not split; while it is, the two panes in their share of the
    /// width with a column for the handle between them.
    pub fn grid_columns(&self, open: &[TabKey]) -> String {
        if self.is_split(open) {
            format!(
                "minmax(0, {}fr) auto minmax(0, {}fr)",
                self.ratio,
                1.0 - self.ratio
            )
        } else {
            "minmax(0, 1fr)".to_owned()
        }
    }

    /// Put every tab back into the left pane if one of the two panes holds no open tab - an empty
    /// pane is closed, and a lone right pane becomes the left one.
    fn unsplit_if_one_sided(&mut self, open: &[TabKey]) {
        if !self.is_split(open) {
            for entry in &mut self.sides {
                entry.1 = Side::Left;
            }
            if self.front_left.is_none() {
                self.front_left = self.front_right;
            }
            self.front_right = None;
        }
    }

    /// The tab last brought to the front of `side`.
    const fn front(&self, side: Side) -> Option<TabKey> {
        match side {
            Side::Left => self.front_left,
            Side::Right => self.front_right,
        }
    }

    /// The slot remembering the front tab of `side`.
    const fn front_mut(&mut self, side: Side) -> &mut Option<TabKey> {
        match side {
            Side::Left => &mut self.front_left,
            Side::Right => &mut self.front_right,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph(n: u128) -> TabKey {
        TabKey::Graph(Uuid::from_u128(n))
    }

    /// A layout with `left` and `right` placed in their panes, as if the user had split it.
    fn split(left: &[TabKey], right: &[TabKey]) -> (TabLayout, Vec<TabKey>) {
        let open: Vec<TabKey> = left.iter().chain(right).copied().collect();
        let mut layout = TabLayout::default();
        layout.sync(&open, Side::Left);
        for tab in right {
            layout.move_tab(*tab, Side::Right, &open);
        }
        (layout, open)
    }

    #[test]
    fn tab_values_round_trip() {
        for tab in [graph(7), TabKey::Scene] {
            assert_eq!(TabKey::parse(&tab.value()), Some(tab));
        }
        assert_eq!(TabKey::parse("not a tab"), None);
    }

    #[test]
    fn a_new_tab_joins_the_pane_it_was_opened_from() {
        let (mut layout, mut open) = split(&[graph(1)], &[graph(2)]);
        open.push(graph(3));
        let placed = layout.sync(&open, Side::Right);
        assert_eq!(placed, vec![graph(3)]);
        assert_eq!(layout.side_of(graph(3)), Side::Right);
        assert_eq!(layout.tabs_on(Side::Right, &open), vec![graph(2), graph(3)]);
    }

    #[test]
    fn a_tab_seen_before_is_not_placed_again() {
        let (mut layout, open) = split(&[graph(1)], &[graph(2)]);
        assert!(layout.sync(&open, Side::Left).is_empty());
        assert_eq!(layout.side_of(graph(2)), Side::Right);
    }

    #[test]
    fn the_scene_opens_beside_the_active_graph_and_is_forgotten_on_close() {
        let (mut layout, mut open) = split(&[graph(1)], &[graph(2)]);
        open.push(TabKey::Scene);
        layout.sync(&open, layout.side_of(graph(2)));
        assert_eq!(layout.side_of(TabKey::Scene), Side::Right);

        open.pop();
        layout.sync(&open, Side::Left);
        open.push(TabKey::Scene);
        layout.sync(&open, layout.side_of(graph(1)));
        assert_eq!(layout.side_of(TabKey::Scene), Side::Left);
    }

    #[test]
    fn moving_a_tab_splits_the_editor_and_moving_it_back_unsplits_it() {
        let mut layout = TabLayout::default();
        let open = vec![graph(1), TabKey::Scene];
        layout.sync(&open, Side::Left);
        assert!(!layout.is_split(&open));

        layout.move_tab(TabKey::Scene, Side::Right, &open);
        assert!(layout.is_split(&open));
        assert_eq!(
            layout.shown(Side::Right, &open, graph_id(1)),
            Some(TabKey::Scene)
        );
        assert_eq!(layout.shown(Side::Left, &open, graph_id(1)), Some(graph(1)));

        layout.move_tab(TabKey::Scene, Side::Left, &open);
        assert!(!layout.is_split(&open));
        assert_eq!(
            layout.shown(Side::Left, &open, graph_id(1)),
            Some(TabKey::Scene)
        );
    }

    #[test]
    fn moving_the_last_tab_out_of_the_left_pane_leaves_one_pane() {
        let (mut layout, open) = split(&[graph(1)], &[graph(2), TabKey::Scene]);
        layout.move_tab(graph(1), Side::Right, &open);
        assert!(!layout.is_split(&open));
        assert_eq!(layout.tabs_on(Side::Left, &open), open);
        assert_eq!(layout.shown(Side::Left, &open, graph_id(1)), Some(graph(1)));
    }

    #[test]
    fn closing_the_last_tab_of_the_right_pane_unsplits() {
        let (mut layout, mut open) = split(&[graph(1)], &[TabKey::Scene]);
        open.retain(|tab| *tab != TabKey::Scene);
        layout.sync(&open, Side::Left);
        assert!(!layout.is_split(&open));
        assert_eq!(layout.shown(Side::Left, &open, graph_id(1)), Some(graph(1)));
    }

    #[test]
    fn a_pane_shows_its_front_tab_then_the_active_graph_then_its_first_tab() {
        let (mut layout, open) = split(&[graph(1), graph(2), graph(3)], &[TabKey::Scene]);
        // Nothing brought forward on the left yet: the active graph, if it is there.
        assert_eq!(layout.shown(Side::Left, &open, graph_id(2)), Some(graph(2)));
        // The active graph is elsewhere: the pane's first tab.
        assert_eq!(layout.shown(Side::Left, &open, graph_id(9)), Some(graph(1)));
        // A tab brought forward wins over the active graph.
        layout.bring_to_front(graph(3));
        assert_eq!(layout.shown(Side::Left, &open, graph_id(2)), Some(graph(3)));
        // A front tab that was closed is skipped.
        let without_3: Vec<TabKey> = open.iter().copied().filter(|t| *t != graph(3)).collect();
        layout.sync(&without_3, Side::Left);
        assert_eq!(
            layout.shown(Side::Left, &without_3, graph_id(2)),
            Some(graph(2))
        );
    }

    #[test]
    fn each_pane_remembers_its_own_front_tab() {
        let (mut layout, open) = split(&[graph(1), graph(2)], &[graph(3), TabKey::Scene]);
        layout.bring_to_front(graph(2));
        layout.bring_to_front(TabKey::Scene);
        assert_eq!(layout.shown(Side::Left, &open, graph_id(3)), Some(graph(2)));
        assert_eq!(
            layout.shown(Side::Right, &open, graph_id(3)),
            Some(TabKey::Scene)
        );
    }

    #[test]
    fn a_pane_without_tabs_shows_nothing() {
        let layout = TabLayout::default();
        assert_eq!(layout.shown(Side::Right, &[graph(1)], graph_id(1)), None);
    }

    #[test]
    fn the_panes_start_out_equally_wide() {
        assert!((TabLayout::default().ratio() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn the_left_pane_follows_the_requested_width() {
        let mut layout = TabLayout::default();
        layout.set_left_width(300.0, 1000.0);
        assert!((layout.ratio() - 0.3).abs() < 1e-12);
    }

    #[test]
    fn neither_pane_gets_narrower_than_the_minimum() {
        let mut layout = TabLayout::default();
        layout.set_left_width(50.0, 1000.0);
        assert!((layout.ratio() - MIN_PANE_WIDTH / 1000.0).abs() < 1e-12);
        layout.set_left_width(990.0, 1000.0);
        assert!((layout.ratio() - (1000.0 - MIN_PANE_WIDTH) / 1000.0).abs() < 1e-12);
    }

    #[test]
    fn too_narrow_for_both_minimums_splits_evenly() {
        let mut layout = TabLayout::default();
        layout.set_left_width(300.0, 1000.0);
        layout.set_left_width(100.0, MIN_PANE_WIDTH + MIN_PANE_WIDTH - 1.0);
        assert!((layout.ratio() - 0.5).abs() < f64::EPSILON);
        layout.set_left_width(f64::NAN, 1000.0);
        assert!((layout.ratio() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn the_grid_has_a_handle_column_only_while_split() {
        let (mut layout, open) = split(&[graph(1)], &[TabKey::Scene]);
        layout.set_left_width(250.0, 1000.0);
        assert_eq!(
            layout.grid_columns(&open),
            "minmax(0, 0.25fr) auto minmax(0, 0.75fr)"
        );
        assert_eq!(layout.grid_columns(&[graph(1)]), "minmax(0, 1fr)");
    }

    #[test]
    fn a_split_editor_offers_the_other_pane_as_drop_target() {
        let (layout, open) = split(&[graph(1)], &[graph(2), TabKey::Scene]);
        assert_eq!(
            layout.drop_targets(graph(1), &open),
            vec![DropTarget::Pane(Side::Right)]
        );
        assert_eq!(
            layout.drop_targets(TabKey::Scene, &open),
            vec![DropTarget::Pane(Side::Left)]
        );
    }

    #[test]
    fn an_unsplit_editor_offers_a_new_pane_on_either_side_unless_the_tab_is_alone() {
        let mut layout = TabLayout::default();
        let open = vec![graph(1), TabKey::Scene];
        layout.sync(&open, Side::Left);
        assert_eq!(
            layout.drop_targets(TabKey::Scene, &open),
            vec![
                DropTarget::Split(Side::Left),
                DropTarget::Split(Side::Right)
            ]
        );
        assert!(layout.drop_targets(graph(1), &[graph(1)]).is_empty());
    }

    #[test]
    fn splitting_right_moves_the_tab_into_a_new_right_pane() {
        let mut layout = TabLayout::default();
        let open = vec![graph(1), graph(2), TabKey::Scene];
        layout.sync(&open, Side::Left);
        layout.drop(TabKey::Scene, DropTarget::Split(Side::Right), &open);
        assert_eq!(layout.tabs_on(Side::Left, &open), vec![graph(1), graph(2)]);
        assert_eq!(layout.tabs_on(Side::Right, &open), vec![TabKey::Scene]);
        assert_eq!(
            layout.shown(Side::Right, &open, graph_id(1)),
            Some(TabKey::Scene)
        );
    }

    #[test]
    fn splitting_left_leaves_the_tab_alone_on_the_left() {
        let mut layout = TabLayout::default();
        let open = vec![graph(1), graph(2), TabKey::Scene];
        layout.sync(&open, Side::Left);
        layout.bring_to_front(graph(2));
        layout.drop(TabKey::Scene, DropTarget::Split(Side::Left), &open);
        assert_eq!(layout.tabs_on(Side::Left, &open), vec![TabKey::Scene]);
        assert_eq!(layout.tabs_on(Side::Right, &open), vec![graph(1), graph(2)]);
        assert_eq!(
            layout.shown(Side::Left, &open, graph_id(1)),
            Some(TabKey::Scene)
        );
        // The other tabs carry on showing what the pane showed before.
        assert_eq!(
            layout.shown(Side::Right, &open, graph_id(1)),
            Some(graph(2))
        );
    }

    #[test]
    fn dropping_into_the_other_pane_moves_the_tab_there() {
        let (mut layout, open) = split(&[graph(1), graph(2)], &[TabKey::Scene]);
        layout.drop(graph(2), DropTarget::Pane(Side::Right), &open);
        assert_eq!(
            layout.tabs_on(Side::Right, &open),
            vec![graph(2), TabKey::Scene]
        );
        assert_eq!(
            layout.shown(Side::Right, &open, graph_id(1)),
            Some(graph(2))
        );
    }

    #[test]
    fn dropping_into_its_own_pane_changes_nothing() {
        let (mut layout, open) = split(&[graph(1), graph(2)], &[TabKey::Scene]);
        let before = layout.clone();
        layout.drop(graph(2), DropTarget::Pane(Side::Left), &open);
        assert_eq!(layout, before);
    }

    #[test]
    fn dragging_the_last_tab_out_of_a_pane_unsplits() {
        let (mut layout, open) = split(&[graph(1)], &[TabKey::Scene]);
        layout.drop(TabKey::Scene, DropTarget::Pane(Side::Left), &open);
        assert!(!layout.is_split(&open));
        assert_eq!(
            layout.shown(Side::Left, &open, graph_id(1)),
            Some(TabKey::Scene)
        );
    }

    #[test]
    fn a_pressed_tab_becomes_a_drag_only_past_the_threshold_and_stays_one() {
        let mut drag = TabDrag::new(TabKey::Scene, Point2D::new(100.0, 10.0));
        drag.move_to(Point2D::new(102.0, 12.0));
        assert!(!drag.is_started());
        drag.move_to(Point2D::new(110.0, 10.0));
        assert!(drag.is_started());
        drag.move_to(Point2D::new(100.0, 10.0));
        assert!(drag.is_started());
        assert_eq!(drag.pointer, Point2D::new(100.0, 10.0));
    }

    fn graph_id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }
}
