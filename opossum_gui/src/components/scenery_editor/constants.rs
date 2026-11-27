// constants for GraphEditor
pub const ZOOM_SENSITIVITY: f64 = 1.1;
pub const MAX_ZOOM: f64 = 2.5;
pub const MIN_ZOOM: f64 = 0.4;
/// Share of the canvas that Zoom to Fit fills with the graph; the rest is margin.
pub const VIEW_PADDING_FACTOR: f64 = 0.95;

// constants for EDGE drawing
/// Length of the straight piece with which a connection leaves and enters a port.
pub const EDGE_PORT_STUB: f64 = 15.0;
pub const EDGE_DISTANCE_FIELD_WIDTH: f64 = 80.0;
pub const EDGE_DISTANCE_FIELD_HEIGHT: f64 = 30.0;

// constants for Node layout
pub const NODE_WIDTH: f64 = 130.0; // The node width is fixed, but the height is dynamic depending on the number of ports
pub const HEADER_HEIGHT: f64 = 30.0;
pub const BORDER_WIDTH: f64 = 1.;

// constants for port layout
pub const PORT_VER_SPACING: f64 = 18.0;
pub const PORT_HEIGHT: f64 = 13.0;
pub const PORT_WIDTH: f64 = 13.0;
pub const PORT_MAP_DIST: f64 = 20.0;

/// Total horizontal overhang of a mapped port tag extending beyond the node border.
/// Calculated as: `PORT_MAP_DIST + 2.0 * PORT_WIDTH + BORDER_WIDTH + PORT_MAP_TAG_WIDTH / 2.0 (20.0)`.
pub const PORT_MAP_OVERHANG: f64 = 2.0f64.mul_add(PORT_WIDTH, PORT_MAP_DIST) + BORDER_WIDTH + 20.0;

// constants for GraphStore
pub const SUGIYAMA_VERTEX_SPACING: f64 = 250.0;
pub const SUGIYAMA_VERT_PATH_FACTOR: f64 = 0.5;
/// Horizontal gap between the nodes of an auto-layout row.
pub const LAYOUT_NODE_GAP: f64 = 50.0;
/// Vertical gap between the lines of an auto layout.
pub const LAYOUT_LINE_GAP: f64 = 100.0;

// constant for node positioning
pub const MIN_NODE_DISTANCE_RADIUS: f64 = 50.0; // If placing a new node, this is the minimum distance from an already existing node in order to avoid complete overlapping.
pub const NODE_PLACEMENT_MAX_ITERATIONS: usize = 100;
