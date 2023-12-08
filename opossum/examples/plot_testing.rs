use std::{f64::consts::PI, time::Instant};

use approx::RelativeEq;
use itertools::izip;
use nalgebra::{MatrixSliceXx1, MatrixXx1, MatrixXx2, MatrixXx3, Matrix1xX, DMatrix, MatrixSlice1xX};
use opossum::error::OpmResult;
use plotters::{backend::DrawingBackend, style::{RGBAColor, ShapeStyle, WHITE, TextStyle, IntoFont}, drawing::DrawingArea, coord::{Shift, cartesian::Cartesian2d, types::RangedCoordf64, ranged3d::Cartesian3d}, chart::{ChartContext, ChartBuilder}, series::LineSeries, element::Circle};

fn linspace(start: f64, end: f64, num: usize) -> Matrix1xX<f64>{
    let mut linspace = Matrix1xX::<f64>::from_element(num, start);
    let bin_size = (end -start)/(num-1) as f64;
    for (i, val) in linspace.iter_mut().enumerate(){
        *val += bin_size * i as f64
    }
    linspace
}
fn meshgrid(x: &Matrix1xX<f64>, y: &Matrix1xX<f64>) -> (DMatrix<f64>,DMatrix<f64>){
    let x_len = x.len();
    let y_len = y.len();

    let mut x_mat = DMatrix::<f64>::zeros(y_len, x_len);
    let mut y_mat = DMatrix::<f64>::zeros(y_len, x_len);


    for x_id in 0..x_len{
        for y_id in 0..y_len{
            x_mat[(y_id, x_id)] = x[x_id];
            y_mat[(y_id, x_id)] = y[y_id];
        }
    };

    (x_mat, y_mat)

}
enum PlotData {
    ///Pairwise 2D data (e.g. x, y data) for scatter2D, Line2D. Data Structure as Matrix with N rows and two columns (x,y)
    Dim2(MatrixXx2<f64>),
    ///Triplet 3D data (e.g. x, y, z data) for scatter3D, Line3D or colormesh. Data Structure as Matrix with N rows and three columns (x,y,z)
    Dim3(MatrixXx3<f64>),
    ///Vector of pairwise 2D data (e.g. x, y data) for MultiLine2D. Data Structure as Vector filled with Matrices with N rows and two columns (x,y)
    MultiDim2(Vec<MatrixXx2<f64>>),
    ///Vector of triplet 3D data (e.g. x, y, z data) for MultiLine3D. Data Structure as Vector filled with Matrices with N rows and three columns (x,y,z)
    MultiDim3(Vec<MatrixXx3<f64>>),
}


fn define_axis_bounds(    
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

fn plot_2d_line<B: DrawingBackend>(
    plt_data:       &PlotData, 
    marker_color:   RGBAColor, 
    expand_bounds:  Vec<[bool;2]>,
    xlabel: &str, 
    ylabel: &str,
    root: &DrawingArea<B, Shift>
) -> OpmResult<()>{

    if let PlotData::Dim2(dat) = plt_data{
        let (x_min, x_max) = define_axis_bounds(&dat.column(0), true, true);
        let (y_min, y_max) = define_axis_bounds(&dat.column(1), true, true);

        let mut chart = create_2d_plot_chart(
            &root,
            [x_min, x_max, y_min, y_max],
            xlabel,
            ylabel
            )?;draw_line(&mut chart, &dat.column(0), &dat.column(1), &marker_color);
        }

        root.present().unwrap();

        Ok(())
    }

    fn plot_2d_scatter<B: DrawingBackend>(
        plt_data:       &PlotData, 
        marker_color:   RGBAColor, 
        expand_bounds:  Vec<[bool;2]>,
        xlabel: &str, 
        ylabel: &str,
        root: &DrawingArea<B, Shift>
    ) -> OpmResult<()>{

        if let PlotData::Dim2(dat) = plt_data{
            let (x_min, x_max) = define_axis_bounds(&dat.column(0), true, true);
            let (y_min, y_max) = define_axis_bounds(&dat.column(1), true, true);

            let mut chart = create_2d_plot_chart(
                &root,
                [x_min, x_max, y_min, y_max],
                xlabel,
                ylabel
                )?;

            draw_points(&mut chart, &dat.column(0), &dat.column(1), &marker_color);
        }

        root.present().unwrap();
        Ok(())
    }

    fn draw_line<'a, T: DrawingBackend>(
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

fn main(){

    let x = linspace(-50., 50., 5000);
    let y = linspace(-50., 50., 5000);

    let start = Instant::now();
    let (xx,yy) = meshgrid(&x, &y);
    let duration = start.elapsed();
    println!("{:?}", duration);
}

        