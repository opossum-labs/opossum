// constants for GraphEditor
pub const ZOOM_SENSITIVITY: f64 = 1.1;
pub const MAX_ZOOM: f64 = 2.5;
pub const MIN_ZOOM: f64 = 0.1;

// constants for EDGE drawing
pub const EDGE_BEZIER_OFFSET: f64 = 50.0;
pub const EDGE_DISTANCE_FIELD_WIDTH: f64 = 60.0;
pub const EDGE_DISTANCE_FIELD_HEIGHT: f64 = 25.0;

// constants for Node layout
pub const NODE_WIDTH: f64 = 130.0; // The node width is fixed, but the height is dynamic depending on the number of ports
pub const HEADER_HEIGHT: f64 = 30.0;

// constants for port layout
pub const PORT_VER_SPACING: f64 = 16.0;
pub const PORT_HEIGHT: f64 = 12.0;
pub const PORT_WIDTH: f64 = 12.0;

// constants for GraphStore
pub const SUGIYAMA_VERTEX_SPACING: usize = 250;
pub const SUGIYAMA_VERT_PATH_FACTOR: f64 = 0.7;
