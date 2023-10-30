use std::path::Path;

use plotters::{
    coord::Shift,
    prelude::{DrawingArea, IntoDrawingArea, SVGBackend},
    style::WHITE,
};

use crate::error::OpmResult;

pub fn prepare_drawing_area(file_path: &Path) -> DrawingArea<SVGBackend<'_>, Shift> {
    let root = SVGBackend::new(file_path, (800, 600)).into_drawing_area();
    root.fill(&WHITE).unwrap();
    root
}

pub trait Plottable {
    fn to_plot(&self, file_path: &Path) -> OpmResult<()>;
}
