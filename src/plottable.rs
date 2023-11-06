use std::path::Path;

use crate::error::OpmResult;
use image::RgbImage;
use plotters::backend::PixelFormat;
use plotters::{
    backend::DrawingBackend,
    coord::Shift,
    prelude::{BitMapBackend, DrawingArea, IntoDrawingArea, SVGBackend},
    style::WHITE,
};

pub trait Plottable {
    fn chart<B: DrawingBackend>(&self, root: &DrawingArea<B, Shift>) -> OpmResult<()>;

    fn to_svg_plot(&self, p: &Path) -> OpmResult<()> {
        let root = SVGBackend::new(p, (800, 600)).into_drawing_area();
        root.fill(&WHITE).unwrap();
        self.chart(&root)
    }
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
            root.fill(&WHITE).unwrap();
            self.chart(&root)?
        }
        let img = RgbImage::from_raw(image_width, image_height, image_buffer).unwrap();
        Ok(img)
    }
}
