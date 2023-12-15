#![warn(missing_docs)]
//! Trait for adding the possibility to generate a (x/y) plot of an element.
use crate::error::OpmResult;
use crate::error::OpossumError;
use image::RgbImage;
use approx::RelativeEq;
use itertools::izip;
use nalgebra::MatrixSliceXx1;
use nalgebra::MatrixXx1;
use nalgebra::MatrixXx2;
use nalgebra::MatrixXx3;
use plotters::backend::PixelFormat;
use plotters::chart::ChartBuilder;
use plotters::chart::ChartContext;
use plotters::coord::cartesian::Cartesian2d;
use plotters::coord::ranged3d::Cartesian3d;
use plotters::coord::types::RangedCoordf64;
use plotters::element::Circle;
use plotters::series::LineSeries;
use plotters::series::SurfaceSeries;
use plotters::style::IntoFont;
use plotters::style::RGBAColor;
use plotters::style::ShapeStyle;
use plotters::style::TextStyle;
use plotters::{
    backend::DrawingBackend,
    coord::Shift,
    prelude::{BitMapBackend, DrawingArea, IntoDrawingArea, SVGBackend},
    style::WHITE,
};
use std::f64::consts::PI;
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

// impl PlotType{    
//     fn plot(
//         &self, 
//         plt_data: &PlotData, 
//         plot_params: PlotParameters,
//     ) -> OpmResult<()> {
        
//         let mut plot = Plot::new(plt_data, plot_params);
        
//         match self{
//             PlotType::ColorMesh => {
//                 let path = plot.fpath.clone();
//                 let backend = BitMapBackend::new(&path, plot.img_size).into_drawing_area();
//                 _ = self.plot_color_mesh(&mut plot, &backend);
//                 Ok(())
//             },
//             PlotType::Scatter2D =>{
//                 Err(OpossumError::Other("plot() not yet defined for plottype::Scatter2D!".into()))
//             },
//             PlotType::Scatter3D =>{
//                 Err(OpossumError::Other("plot() not yet defined for plottype::Scatter2D!".into()))
//             },
//             PlotType::Line2D =>{
//                 Err(OpossumError::Other("plot() not yet defined for plottype::Scatter2D!".into()))
//             },
//             PlotType::Line3D =>{
//                 Err(OpossumError::Other("plot() not yet defined for plottype::Scatter2D!".into()))
//             },
//             PlotType::MultiLine2D =>{
//                 Err(OpossumError::Other("plot() not yet defined for plottype::Scatter2D!".into()))
//             },
//             PlotType::MultiLine3D =>{
//                 Err(OpossumError::Other("plot() not yet defined for plottype::Scatter2D!".into()))
//             },
            
//             _ => Err(OpossumError::Other("Plottype notdefined yet!".into()))
//         }
//     }
// }


///Enum to define the type of plot that should be created
pub enum PlotData {
    ///Pairwise 2D data (e.g. x, y data) for scatter2D, Line2D. Data Structure as Matrix with N rows and two columns (x,y)
    Dim2(MatrixXx2<f64>),
    ///Triplet 3D data (e.g. x, y, z data) for scatter3D, Line3D or colormesh. Data Structure as Matrix with N rows and three columns (x,y,z)
    Dim3(MatrixXx3<f64>),
    ///Vector of pairwise 2D data (e.g. x, y data) for MultiLine2D. Data Structure as Vector filled with Matrices with N rows and two columns (x,y)
    MultiDim2(Vec<MatrixXx2<f64>>),
    ///Vector of triplet 3D data (e.g. x, y, z data) for MultiLine3D. Data Structure as Vector filled with Matrices with N rows and three columns (x,y,z)
    MultiDim3(Vec<MatrixXx3<f64>>),
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
    // fn chart<B: DrawingBackend>(&self, root: &DrawingArea<B, Shift>) -> OpmResult<()>;

    fn to_svg_plot(&self, f_path: &Path, img_size: (u32, u32)) -> OpmResult<()>{
        let root = SVGBackend::new(f_path, img_size).into_drawing_area();
        root.fill(&WHITE)
            .map_err(|e| format!("filling plot background failed: {e}"))?;
        self.create_plot(&root)
    }

    fn create_plot<B: DrawingBackend>(&self, root: &DrawingArea<B, Shift>) -> OpmResult<()>;
    
    fn define_axis_bounds(
        &self, 
        x: &MatrixSliceXx1<f64>, 
        expand_min: bool, 
        expand_max: bool
    ) -> (f64, f64){

        //filter out every infinite value and every NaN
        let x_filtered = MatrixXx1::from(x.iter().cloned().filter(|x| !x.is_nan() & x.is_finite()).collect::<Vec<f64>>());

        //this only happens if all entries in this matrix are either infinite or NAN
        let (x_range, x_min, x_max) =  if x_filtered.len() == 0{
            (1., 0., 1.)
        }
        else{
            //get maximum and minimum of the axis
            let max_val = x_filtered.max();
            let min_val = x_filtered.min();
            let mut ax_range = max_val - min_val;

            //check if mininum and maximum value are approximately equal. if so, take max value as range
            if max_val.relative_eq(&min_val, f64::EPSILON, f64::EPSILON)
            {
                ax_range = max_val;
            };

            //check if for some reason maximum is 0, then set to 1, so that the axis spans at least some distance
            if ax_range < f64::EPSILON
            {
                ax_range = 1.;
            };

            (ax_range, min_val, max_val)

        };

        //add spacing to the edges if defined
        let add_range_fac = 0.1 ;
        let expand_min_fac = expand_min as i32 as f64;
        let expand_max_fac = expand_max as i32 as f64;

        (x_min-x_range*add_range_fac*expand_min_fac, x_max+x_range*add_range_fac*expand_max_fac)

    }

    // fn define_axes_bounds(
    //     &self, 
    //     plt_data:       &PlotData, 
    //     num_axes:       usize,
    //     expand_bounds:  Vec<[bool; 2]>
    // ) -> Option<Vec<(f64, f64)>>{

    //     if num_axes != expand_bounds.len(){
    //         Err(OpossumError::Plots("Number of axes does not match the number of axes bounds!".into()))
    //     }
    //     else{

    //         let mut bound_vec = Vec::<(f64,f64)>::new();

    //         let dat = match plt_data{
    //             PlotData::Dim2(dat) => dat,
    //             PlotData::Dim3(dat) => dat,
    //             PlotData::MultiDim2(dat) => dat,
    //             PlotData::MultiDim3(dat) => dat,
    //             _ =>  Err(OpossumError::Plots("Unkown PlotData variant!".into()))
    //         };

    //         for (axis, expand) in dat.column_iter().zip(expand_bounds.iter()){
    //             bound_vec.push(self.define_axis_bounds(axis, expand[0], expand[1]));
    //         };

    //         Some(bound_vec)
    //     }

    // }

    fn plot_2d_line<B: DrawingBackend>(
        &self, 
        plt_data:       &PlotData, 
        marker_color:   RGBAColor, 
        expand_bounds:  Vec<[bool;2]>,
        xlabel: &str, 
        ylabel: &str,
        root: &DrawingArea<B, Shift>
    ) -> OpmResult<()>{

        if let PlotData::Dim2(dat) = plt_data{
            let (x_min, x_max) = self.define_axis_bounds(&dat.column(0), true, true);
            let (y_min, y_max) = self.define_axis_bounds(&dat.column(1), true, true);

            let mut chart = self.create_2d_plot_chart(
                &root,
                [x_min, x_max, y_min, y_max],
                xlabel,
                ylabel
                )?;

            self.draw_line(&mut chart, &dat.column(0), &dat.column(1), &marker_color);
        }

        root.present().unwrap();

        Ok(())
    }

    fn plot_2d_scatter<B: DrawingBackend>(
        &self, 
        plt_data:       &PlotData, 
        marker_color:   RGBAColor, 
        expand_bounds:  Vec<[bool;2]>,
        xlabel: &str, 
        ylabel: &str,
        root: &DrawingArea<B, Shift>
    ) -> OpmResult<()>{

        if let PlotData::Dim2(dat) = plt_data{
            let (x_min, x_max) = self.define_axis_bounds(&dat.column(0), true, true);
            let (y_min, y_max) = self.define_axis_bounds(&dat.column(1), true, true);

            let mut chart = self.create_2d_plot_chart(
                &root,
                [x_min, x_max, y_min, y_max],
                xlabel,
                ylabel
                )?;

            self.draw_points(&mut chart, &dat.column(0), &dat.column(1), &marker_color);
        }

        root.present().unwrap();
        Ok(())
    }

    fn draw_line<'a, T: DrawingBackend>(
        &self, 
        chart:      &mut ChartContext<'a, T, Cartesian2d<RangedCoordf64, RangedCoordf64>>,
        x:          &MatrixSliceXx1<f64>,
        y:          &MatrixSliceXx1<f64>,
        line_color: &RGBAColor
    ){
        chart
            .draw_series(LineSeries::new(
                izip!(x, y)
                     .map(|xy| (*xy.0, *xy.1)), 
               line_color)
            ).unwrap();
    }

    fn draw_points<'a, T: DrawingBackend>(
        &self, 
        chart:          &mut ChartContext<'a, T, Cartesian2d<RangedCoordf64, RangedCoordf64>>,
        x:              &MatrixSliceXx1<f64>,
        y:              &MatrixSliceXx1<f64>,
        marker_color:   &RGBAColor
    ){
        chart.draw_series(
            izip!(x, y).map(|x| Circle::new((*x.0, *x.1), 5, Into::<ShapeStyle>::into(marker_color).filled())),
        ).unwrap();
    }

    // fn draw_2d_colormesh<'a, T: DrawingBackend>(
    //     &self, 
    //     chart:          &mut ChartContext<'a, T, Cartesian3d<RangedCoordf64, RangedCoordf64, RangedCoordf64>>,
    //     x:              &MatrixSliceXx1<f64>,
    //     y:              &MatrixSliceXx1<f64>,
    //     z:              &MatrixSliceXx1<f64>,
    //     marker_color:   &RGBAColor
    // ){
    //     let test = (-15..=15).map(|x| x as f64 / 5.0);
    //     chart.draw_series(
    //         SurfaceSeries::xoz(
    //             x.iter(),
    //             y.iter(),
    //             x.iter()
    //         )
            
    //         izip!(x, y).map(|x| Circle::new((*x.0, *x.1), 5, Into::<ShapeStyle>::into(marker_color).filled())),
    //     ).unwrap();
    // }

    // fn plot_color_mesh<B: DrawingBackend>(
    //     &self, 
    //     plt_data:       &PlotData, 
    //     marker_color:   RGBAColor, 
    //     expand_bounds:  Vec<[bool;3]>,
    //     xlabel: &str, 
    //     ylabel: &str,
    //     zlabel: &str,
    //     root: &DrawingArea<B, Shift>
    // ) -> OpmResult<()>{

    //     if let PlotData::Dim3(dat) = plt_data{
    //         let (x_min, x_max) = self.define_axis_bounds(&dat.column(0), true, true);
    //         let (y_min, y_max) = self.define_axis_bounds(&dat.column(1), true, true);
    //         let (z_min, z_max) = self.define_axis_bounds(&dat.column(2), true, true);

    //         let mut chart = self.create_3d_plot_chart(
    //             &root,
    //             [x_min, x_max, y_min, y_max, z_min, z_max],
    //             0.,
    //             0.,
    //             xlabel,
    //             ylabel,
    //             zlabel
    //             )?;

    //         self.draw_points(&mut chart, &dat.column(0), &dat.column(1), &marker_color);
    //     }

    //     root.present().unwrap();
    //     Ok(())
    // }

    fn create_3d_plot_chart<'a, T: DrawingBackend>(
        &self, 
        root: &'a DrawingArea<T, plotters::coord::Shift>,
        bounds: [f64; 6], 
        pitch: f64,
        yaw: f64,
        xlabel: &str,
        ylabel: &str,
        zlabel: &str
    ) -> OpmResult<ChartContext<'a, T, Cartesian3d<RangedCoordf64, RangedCoordf64, RangedCoordf64>>> {

        root.fill(&WHITE).unwrap();

        let mut chart = ChartBuilder::on(root)
            .margin(20)
            .set_all_label_area_size(100)
            .build_cartesian_3d(
                bounds[0]..bounds[1], 
                bounds[2]..bounds[3], 
                bounds[4]..bounds[5])
            .unwrap();

        chart.with_projection(|mut pb: plotters::coord::ranged3d::ProjectionMatrixBuilder| {
                pb.pitch = pitch*PI;
                pb.yaw = yaw*PI;
                pb.scale = 0.7;
                pb.into_matrix()
            });

        chart
            .configure_axes()
            .draw()
            .unwrap();        

        Ok(chart)
    }

    fn create_2d_plot_chart<'a, T: DrawingBackend>(
    &self, 
    root: &'a DrawingArea<T, Shift>,
    bounds: [f64; 4], 
    xlabel: &str, 
    ylabel: &str
    ) -> OpmResult<ChartContext<'a, T, Cartesian2d<RangedCoordf64, RangedCoordf64>>> {

        root.fill(&WHITE).unwrap();

        let mut chart = ChartBuilder::on(root)
            .margin(5)
            .x_label_area_size(100)
            .y_label_area_size(100)
            .build_cartesian_2d(
                bounds[0]..bounds[1], 
                bounds[2]..bounds[3])
            .unwrap();

        chart
            .configure_mesh()
            .x_desc(xlabel)
            .y_desc(ylabel)
            .label_style(TextStyle::from(("sans-serif", 30).into_font()))
            .draw()
            .unwrap();        

        Ok(chart)
    }

    /// Generate a plot of the element as SVG file with the given path.
    /// # Attributes
    /// `f_path`: path to the file destination
    ///
    /// # Errors
    /// This function will return an error if
    ///  - the given path is not writable or does not exist.
    ///  - the plot area cannot be filled with a background colour.
    // fn to_svg_plot(&self, f_path: &Path) -> OpmResult<()> {
    //     let root = SVGBackend::new(f_path, (800, 800)).into_drawing_area();
    //     root.fill(&WHITE)
    //         .map_err(|e| format!("filling plot background failed: {e}"))?;
    //     self.chart(&root)
    // }
    /// Generate a plot of the given element as an image buffer.
    ///
    /// # Errors
    ///
    /// This function will return an error if
    ///  - the plot area cannot be filled.
    ///  - the image buffer cannot be allocated or has the wrong size.
    fn to_img_buf_plot(&self, img_size: (u32, u32)) -> OpmResult<RgbImage> {
        let (image_width, image_height) = img_size;
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
            self.create_plot(&root)?;
        }
        let img = RgbImage::from_raw(image_width, image_height, image_buffer)
            .ok_or_else(|| OpossumError::Other("image buffer size too small".into()))?;
        Ok(img)
    }

}
// #[cfg(test)]
// mod test {
//     use super::*;
//     use crate::rays::Rays;
//     use tempfile::NamedTempFile;
//     #[test]
//     fn to_svg_plot() {
//         let rays = Rays::default();
//         let path = NamedTempFile::new().unwrap();
//         assert!(rays.to_svg_plot(path.path()).is_ok());
//     }
//     #[test]
//     fn to_img_buf_plot() {
//         let rays = Rays::default();
//         assert!(rays.to_img_buf_plot().is_ok());
//     }
// }
