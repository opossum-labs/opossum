// constants for GraphEditor
pub const ZOOM_SENSITIVITY: f64 = 1.1;
pub const MAX_ZOOM: f64 = 2.5;
pub const MIN_ZOOM: f64 = 0.4;

// constants for EDGE drawing
pub const EDGE_BEZIER_OFFSET: f64 = 50.0;
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

// constants for GraphStore
pub const SUGIYAMA_VERTEX_SPACING: f64 = 250.0;
pub const SUGIYAMA_VERT_PATH_FACTOR: f64 = 0.7;

// constant for node positioning
pub const MIN_NODE_DISTANCE_RADIUS: f64 = 50.0; // If placing a new node, this is the minimum distance from an already existing node in order to avoid complete overlapping.
pub const NODE_PLACEMENT_MAX_ITERATIONS: usize = 100;
