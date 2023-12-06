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

    /// Generate a plot of the element as SVG file with the given path.
    /// # Attributes
    /// `f_path`: path to the file destination
    ///
    /// # Errors
    /// This function will return an error if
    ///  - the given path is not writable or does not exist.
    ///  - the plot area cannot be filled with a background colour.
    fn to_svg_plot(&self, f_path: &Path) -> OpmResult<()> {
        let root = SVGBackend::new(f_path, (800, 800)).into_drawing_area();
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
    
    fn to_plot(&self, f_path: &Path, plot_type: PlotType) -> OpmResult<()>{
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
        
        Ok(())
    }

    fn plot_2d_scatter(
        &self, 
        x:              &Array1<f64>, 
        y:              &Array1<f64>, 
        marker_color:   RGBAColor, 
        fname:          &str,
        expand_bounds:  Vec<[bool;2]>,
        xlabel: &str, 
        ylabel: &str
    ) -> Result<()>{

        let root = SVGBackend::new(fname, (800, 600)).into_drawing_area();

        let bounds = self.define_axes_bounds(vec![x.clone(),y.clone()], expand_bounds)?;

        let mut chart = self.create_2d_plot_chart(
            &root,
            bounds,
            xlabel,
            ylabel
            )?;
        
        self.draw_points(&mut chart, x, y, &marker_color);
        

        root.present().unwrap();

        Ok(())

    }

    // fn define_axes_bounds(
    //     &self, 
    //     axes_values: Vec<Array1<f64>>, 
    //     expand_bounds: Vec<[bool; 2]>
    // ) -> Result<Vec<(f64, f64)>>{

    //     if axes_values.len() != expand_bounds.len(){
    //         Err(OpossumError::Plots("Number of axes does not match the number of axes bounds!".into()))
    //     }
    //     else{

    //         let mut bound_vec = Vec::<(f64,f64)>::new();

    //         for (axis, expand) in axes_values.iter().zip(expand_bounds.iter()){
    //             bound_vec.push(self.define_axis_bounds(axis, expand[0], expand[1]));
    //         };

    //         Ok(bound_vec)
    //     }

    // }


    


    // fn draw_points<'a, T: DrawingBackend>(
    //     &self, 
    //     chart:          &mut ChartContext<'a, T, Cartesian2d<RangedCoordf64, RangedCoordf64>>,
    //     x:              &Array1<f64>,
    //     y:              &Array1<f64>,
    //     marker_color:   &RGBAColor
    // ){
    //     chart.draw_series(
    //         izip!(x, y).map(|x| Circle::new((*x.0, *x.1), 5, marker_color),),
    //     ).unwrap();
    // }

    // fn plot_3d_multi_line(
    //     &self, 
    //     x:              &Vec<Array1<f64>>, 
    //     y:              &Vec<Array1<f64>>, 
    //     z:              &Vec<Array1<f64>>, 
    //     line_color:     RGBAColor, 
    //     fname:          &str,
    //     expand_bounds:  Vec<[bool;2]>
    // ) -> Result<()>{

    //     let root = SVGBackend::new(fname, (800, 600)).into_drawing_area();

    //     let x_arr_flat = Array1::from_vec(x.into_iter().flat_map(|row| row.to_vec()).collect());
    //     let y_arr_flat = Array1::from_vec(y.into_iter().flat_map(|row| row.to_vec()).collect());
    //     let z_arr_flat = Array1::from_vec(y.into_iter().flat_map(|row| row.to_vec()).collect());

    //     let bounds = self.define_axes_bounds(vec![x_arr_flat.clone(),y_arr_flat.clone(),z_arr_flat.clone()], expand_bounds)?;

    //     let mut chart = self.create_3d_plot_chart(
    //         &root,
    //         bounds
    //         )?;
        
    //     for (x, y, z) in izip!(x,y,z){
    //         self.draw_3d_line(&mut chart, x, y, z, &line_color);
    //     }

    //     root.present().unwrap();

    //     Ok(())

    // }

    // fn plot_3d_line(
    //     &self, 
    //     x:              &Array1<f64>, 
    //     y:              &Array1<f64>, 
    //     z:              &Array1<f64>, 
    //     line_color:     RGBAColor, 
    //     fname:          &str,
    //     expand_bounds:  Vec<[bool;2]>
    // ) -> Result<()>{

    //     let root = SVGBackend::new(fname, (800, 600)).into_drawing_area();

    //     let bounds = self.define_axes_bounds(vec![x.clone(),y.clone(),z.clone()], expand_bounds)?;

    //     let mut chart = self.create_3d_plot_chart(
    //         &root,
    //         bounds
    //         )?;
        
    //     self.draw_3d_line(&mut chart, x, y, z, &line_color);

    //     root.present().unwrap();

    //     Ok(())

    // }
    // fn create_3d_plot_chart<'a, T: DrawingBackend>(
    //     &self, 
    //     root: &'a DrawingArea<T, plotters::coord::Shift>,
    //     bounds: Vec<(f64, f64)>, 
    // ) -> Result<ChartContext<'a, T, Cartesian3d<RangedCoordf64, RangedCoordf64, RangedCoordf64>>> {

    //     root.fill(&WHITE).unwrap();

    //     if bounds.len() != 3{
    //         Err(OpossumError::Plots("number of defines axes bounds is not 3!".into()))
    //     }
    //     else{
    //         let mut chart = ChartBuilder::on(root)
    //             .margin(20)
    //             .set_all_label_area_size(40)
    //             .build_cartesian_3d(
    //                 bounds[0].0..bounds[0].1, 
    //                 bounds[1].0..bounds[1].1, 
    //                 bounds[2].0..bounds[2].1)
    //             .unwrap();

    //         chart.with_projection(|mut pb: plotters::coord::ranged3d::ProjectionMatrixBuilder| {
    //                 pb.pitch = 45./180.*PI;
    //                 pb.yaw = 45./180.*PI;
    //                 pb.scale = 0.7;
    //                 pb.into_matrix()
    //             });

    //         chart
    //             .configure_axes()
    //             .draw()
    //             .unwrap();        

    //         Ok(chart)
    //     }
    // }

    // fn create_2d_plot_chart<'a, T: DrawingBackend>(
    //     &self, 
    //     root: &'a DrawingArea<T, plotters::coord::Shift>,
    //     bounds: Vec<(f64, f64)>, 
    //     xlabel: &str, 
    //     ylabel: &str
    // ) -> Result<ChartContext<'a, T, Cartesian2d<RangedCoordf64, RangedCoordf64>>> {

    //     root.fill(&WHITE).unwrap();

    //     if bounds.len() != 2{
    //         Err(OpossumError::Plots("number of defines axes bounds is not 2!".into()))
    //     }
    //     else{
    //         let mut chart = ChartBuilder::on(root)
    //             .margin(5)
    //             .x_label_area_size(40)
    //             .y_label_area_size(40)
    //             .build_cartesian_2d(
    //                 bounds[0].0..bounds[0].1, 
    //                 bounds[1].0..bounds[1].1)
    //             .unwrap();

    //         chart
    //             .configure_mesh()
    //             .x_desc(xlabel)
    //             .y_desc(ylabel)
    //             .draw()
    //             .unwrap();        

    //         Ok(chart)
    //     }
    // }

    // fn plot_2d_multi_line(        
    //     &self, 
    //     x:              &Vec<Array1<f64>>, 
    //     y:              &Vec<Array1<f64>>, 
    //     line_color:     RGBAColor, 
    //     xlabel:         &str, 
    //     ylabel:         &str,
    //     fname:          &str,
    //     expand_lims:    Vec<[bool;2]>
    // )-> Result<()>{

    //     let root = SVGBackend::new(fname, (800, 600)).into_drawing_area();
        
    //     let x_arr_flat = Array1::from_vec(x.into_iter().flat_map(|row| row.to_vec()).collect());
    //     let y_arr_flat = Array1::from_vec(y.into_iter().flat_map(|row| row.to_vec()).collect());

    //     let bounds = self.define_axes_bounds(vec![x_arr_flat.clone(), y_arr_flat.clone()], expand_lims)?;
        
    //     let mut chart = self.create_2d_plot_chart(
    //         &root,
    //         bounds,
    //         xlabel,
    //         ylabel)?;
        
    //     for (x, y) in x.iter().zip(y.iter()){
    //         self.draw_line(&mut chart, x, y, &line_color);
    //     }
        
    //     root.present().unwrap();

    //     Ok(())
    // }

    // fn draw_3d_line<'a, T: DrawingBackend>(
    //     &self, 
    //     chart:      &mut ChartContext<'a, T, Cartesian3d<RangedCoordf64, RangedCoordf64, RangedCoordf64>>,
    //     x:          &Array1<f64>,
    //     y:          &Array1<f64>,
    //     z:          &Array1<f64>,
    //     line_color: &RGBAColor
    // )
    // {
    //     chart
    //         .draw_series(LineSeries::new(
    //                     izip!(x, y, z)
    //                    .map(|xyz| (*xyz.0, *xyz.1, *xyz.2)), 
    //             line_color)
    //         ).unwrap();
    // }

    // fn draw_line<'a, T: DrawingBackend>(
    //     &self, 
    //     chart:      &mut ChartContext<'a, T, Cartesian2d<RangedCoordf64, RangedCoordf64>>,
    //     x:          &Array1<f64>,
    //     y:          &Array1<f64>,
    //     line_color: &RGBAColor
    // ){
    //     chart
    //         .draw_series(LineSeries::new(
    //             izip!(x, y)
    //                  .map(|xy| (*xy.0, *xy.1)), 
    //            line_color)
    //         ).unwrap();
    // }

    // fn plot_2d_line(
    //     &self, 
    //     x:              &Array1<f64>, 
    //     y:              &Array1<f64>, 
    //     line_color:     RGBAColor, 
    //     xlabel:         &str, 
    //     ylabel:         &str,
    //     fname:          &str,
    //     expand_lims:    Vec<[bool;2]>
    // ) -> Result<()>{

    //     let root = SVGBackend::new(fname, (800, 600)).into_drawing_area();

    //     let bounds = self.define_axes_bounds(vec![x.clone(), y.clone()], expand_lims)?;

    //     let mut chart = self.create_2d_plot_chart(
    //         &root,
    //         bounds,
    //         xlabel,
    //         ylabel)?;
        
    //     self.draw_line(&mut chart, x, y, &line_color);

    //     root.present().unwrap();

    //     Ok(())
    // }

    // fn define_axis_bounds(&self, x: &Array1<f64>, expand_min: bool, expand_max: bool) -> (f64, f64){

    //     let (x_min, x_max) = (x.min_skipnan(), x.max_skipnan());

    //     let mut x_range = x_max-x_min;
    //     if x_range < f64::EPSILON{x_range = *x_max};
    //     if x_range < f64::EPSILON{x_range = 1.};

    //     let add_range_fac = 0.1 ;

    //     let expand_min_fac = expand_min as i32 as f64;
    //     let expand_max_fac = expand_max as i32 as f64;

    //     (*x_min-x_range*add_range_fac*expand_min_fac, *x_max+x_range*add_range_fac*expand_max_fac)

    // }

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
