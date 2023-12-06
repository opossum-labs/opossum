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

///Enum to define the type of plot that should be created
pub enum PlotType {
    ///Scatter plot in two dimensions for pairwise data
    Scatter2D,
    ///Scatter plot in three dimensions for 3D data
    Scatter3D,
    ///Line plot in two dimentions for pairwise data
    Line2D,
    ///Line plot in three dimensions for 3D data
    Line3D,
    ///Line plot for multiple lines, e.g. rays, in two dimentions with pairwise data
    MultiLine2D,
    ///Line plot for multiple lines, e.g. rays, in three dimentions with 3D data
    MultiLine3D,
    ///2D color plot of gridded data with color representing the amplitude over an x-y grid
    ColorMesh,
}

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

    fn scatter_plot_2D<B: DrawingBackend>(&self, root: &DrawingArea<B, Shift>) -> OpmResult<()> {
        Ok(())
    }

    fn get_plot_data(&self, )
    /// Generate a plot of the element as SVG file with the given path.
    /// # Attributes
    /// `f_path`: path to the file destination
    ///
    /// # Errors
    /// This function will return an error if
    ///  - the given path is not writable or does not exist.
    ///  - the plot area cannot be filled with a background colour.
    fn to_svg_plot(&self, f_path: &Path, plot_type: PlotType) -> OpmResult<()> {
        let root = SVGBackend::new(f_path, (800, 800)).into_drawing_area();
        root.fill(&WHITE)
            .map_err(|e| format!("filling plot background failed: {e}"))?;
        match plot_type{
            PlotType::Scatter2D => self.scatter_plot_2D(&root),
            // PlotType::Scatter3D =>,
            // PlotType::Line2D =>,
            // PlotType::Line3D =>,
            // PlotType::MultiLine2D =>,
            // PlotType::MultiLine3D =>,
            // PlotType::ColorMesh =>,
            _ => Err(OpossumError::Other("Plot Type not defined, yet!".into()))
        };
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
        let image_height = 800_u32;
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
#[cfg(test)]
mod test {
    use super::*;
    use crate::rays::Rays;
    use tempfile::NamedTempFile;
    #[test]
    fn to_svg_plot() {
        let rays = Rays::default();
        let path = NamedTempFile::new().unwrap();
        assert!(rays.to_svg_plot(path.path()).is_ok());
    }
    #[test]
    fn to_img_buf_plot() {
        let rays = Rays::default();
        assert!(rays.to_img_buf_plot().is_ok());
    }
}
