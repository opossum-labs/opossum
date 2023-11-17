#![warn(missing_docs)]
//! Trait for adding the possibility to generate a (x/y) plot of an element.
use crate::error::OpmResult;
use crate::error::OpossumError;
use image::RgbImage;
use plotters::backend::PixelFormat;
use plotters::{
    backend::DrawingBackend,
    coord::Shift,
    prelude::{BitMapBackend, DrawingArea, IntoDrawingArea, SVGBackend},
    style::WHITE,
};
use std::path::Path;

/// Trait for adding the possibility to generate a (x/y) plot of an element.
pub trait Plottable {
    /// This function must be implemented by the particular element in order to generate a plot.
    ///
    /// At this time the drawing area / backend is already intialized.
    ///
    /// # Errors
    ///
    /// This function will return an error if the drawing code of the implementing function fails.
    fn chart<B: DrawingBackend>(&self, root: &DrawingArea<B, Shift>) -> OpmResult<()>;
    /// Generate a plot of the element as SVG file with the given path.
    /// # Attributes
    /// `f_path`: path to the file destination
    ///
    /// # Errors
    /// This function will return an error if the plot area cannot be filled with a background colour..
    fn to_svg_plot(&self, f_path: &Path) -> OpmResult<()> {
        let root = SVGBackend::new(f_path, (800, 600)).into_drawing_area();
        root.fill(&WHITE)
            .map_err(|e| format!("filling plot background failed: {e}"))?;
        self.chart(&root)
    }
    /// Generate a plot of the given element as an image buffer.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    ///  - the plot area cannot be filled.
    ///  - the image buffer cannot be allocated or has the wrong size.
    fn to_img_buf_plot(&self) -> OpmResult<RgbImage> {
        let image_width = 800_u32;
        let image_height = 600_u32;
        let mut image_buffer = vec![
            0;
            (image_width * image_height) as usize
                * plotters::backend::RGBPixel::PIXEL_SIZE
        ];
        {
            let root = BitMapBackend::with_buffer(&mut image_buffer, (image_width, image_height))
                .into_drawing_area();
            root.fill(&WHITE)
                .map_err(|e| format!("filling plot background failed: {e}"))?;
            self.chart(&root)?;
        }
        let img = RgbImage::from_raw(image_width, image_height, image_buffer)
            .ok_or_else(|| OpossumError::Other("image buffer size too small".into()))?;
        Ok(img)
    }
}
