#![warn(missing_docs)]
//! Trait for adding the possibility to generate a (x/y) plot of an element.
use crate::error::OpmResult;
use crate::error::OpossumError;
use approx::RelativeEq;
use colorous::Gradient;
use delaunator::{triangulate, Point};
use image::RgbImage;
use itertools::{iproduct, izip};
use nalgebra::{
    ComplexField, DMatrix, DVector, DVectorSlice, Matrix1xX, Matrix3xX, MatrixSliceXx1, MatrixXx1,
    MatrixXx2, MatrixXx3,
};
use num::ToPrimitive;
use plotters::{
    backend::DrawingBackend,
    backend::PixelFormat,
    chart::{ChartBuilder, ChartContext, LabelAreaPosition},
    coord::{cartesian::Cartesian2d, ranged3d::Cartesian3d, types::RangedCoordf64, Shift},
    element::{Circle, Polygon, Rectangle},
    prelude::{BitMapBackend, DrawingArea, IntoDrawingArea, SVGBackend},
    series::LineSeries,
    style::WHITE,
    style::{IntoFont, RGBAColor, ShapeStyle},
};
use std::{collections::HashMap, env::current_dir, f64::consts::PI, path::Path};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

///Enum to define the type of plot that should be created
pub enum PlotType {
    ///Scatter plot in two dimensions for pairwise data
    Scatter2D(PlotParameters),
    // ///Scatter plot in three dimensions for 3D data
    // Scatter3D,
    ///Line plot in two dimensions for pairwise data
    Line2D(PlotParameters),
    // ///Line plot in three dimensions for 3D data
    // Line3D,
    // ///Line plot for multiple lines, e.g. rays, in two dimensions with pairwise data
    // MultiLine2D,
    // ///Line plot for multiple lines, e.g. rays, in three dimensions with 3D data
    // MultiLine3D,
    ///2D color plot of gridded data with color representing the amplitude over an x-y grid
    ColorMesh(PlotParameters),

    // ///2D color plot of ungridded data with color representing the amplitude over an x-y grid
    // ColorScatter(PlotParameters),
    /// 2D, triangulated color plot of ungridded data with color representing the amplitude over an x-y grid
    ColorTriangulated(PlotParameters),
    /// 3D surface plot of ungridded data
    TriangulatedSurface(PlotParameters),
}
impl PlotType {
    const fn get_plot_params(&self) -> &PlotParameters {
        match self {
            Self::ColorMesh(p)
            | Self::Scatter2D(p)
            | Self::Line2D(p)
            | Self::ColorTriangulated(p)
            | Self::TriangulatedSurface(p) => p,
        }
    }
    fn create_plot<B: DrawingBackend>(&self, backend: &DrawingArea<B, Shift>, plot: &Plot) {
        match self {
            Self::ColorMesh(_) => Self::plot_color_mesh(plot, backend),
            Self::TriangulatedSurface(_) => Self::plot_triangulated_surface(plot, backend),
            Self::ColorTriangulated(_) => Self::plot_color_triangulated(plot, backend),
            Self::Scatter2D(_) => Self::plot_2d_scatter(plot, backend),
            Self::Line2D(_) => Self::plot_2d_line(plot, backend),
        };
    }

    /// This method creates a plot
    /// # Attributes
    /// - `plt_data`: plot data. See [`PlotData`]
    /// # Returns
    /// This mehotd returns an [`OpmResult<Option<RgbImage>>`]. It is None if a new file (such as svg, png, bmp or jpg) is created. It is Some(RgbImage) if the image is written to a buffer
    /// # Errors
    /// This method throws an error if
    /// - some plot parameters contradict each other
    /// - the file path can not be extracted
    /// - the plotting backend can not be extracted
    /// - the plot can not be created inside the `create_plot()` method
    /// - the image buffer is too small
    pub fn plot(&self, plt_data: &PlotData) -> OpmResult<Option<RgbImage>> {
        let params = self.get_plot_params();
        params.check_validity()?;
        let path = params.get_fpath()?;
        let plot = Plot::new(plt_data, params);

        match params.get_backend()? {
            PltBackEnd::BMP => {
                let backend = BitMapBackend::new(&path, plot.img_size).into_drawing_area();
                self.create_plot(&backend, &plot);
                Ok(None)
            }
            PltBackEnd::SVG => {
                let backend = SVGBackend::new(&path, plot.img_size).into_drawing_area();
                self.create_plot(&backend, &plot);
                Ok(None)
            }
            PltBackEnd::Buf => {
                let mut image_buffer = vec![
                    0;
                    (plot.img_size.0 * plot.img_size.1) as usize
                        * plotters::backend::RGBPixel::PIXEL_SIZE
                ];
                {
                    let backend = BitMapBackend::with_buffer(&mut image_buffer, plot.img_size)
                        .into_drawing_area();
                    self.create_plot(&backend, &plot);
                }
                let img = RgbImage::from_raw(plot.img_size.0, plot.img_size.1, image_buffer)
                    .ok_or_else(|| OpossumError::Other("image buffer size too small".into()))?;
                Ok(Some(img))
            }
        }
    }

    fn draw_line<T: DrawingBackend>(
        chart: &mut ChartContext<'_, T, Cartesian2d<RangedCoordf64, RangedCoordf64>>,
        x: &MatrixSliceXx1<'_, f64>,
        y: &MatrixSliceXx1<'_, f64>,
        line_color: &RGBAColor,
    ) {
        chart
            .draw_series(LineSeries::new(
                izip!(x, y).map(|xy| (*xy.0, *xy.1)),
                line_color,
            ))
            .unwrap();
    }

    fn draw_points<T: DrawingBackend>(
        chart: &mut ChartContext<'_, T, Cartesian2d<RangedCoordf64, RangedCoordf64>>,
        x: &MatrixSliceXx1<'_, f64>,
        y: &MatrixSliceXx1<'_, f64>,
        marker_color: &RGBAColor,
    ) {
        chart
            .draw_series(izip!(x, y).map(|x| {
                Circle::new(
                    (*x.0, *x.1),
                    5,
                    Into::<ShapeStyle>::into(marker_color).filled(),
                )
            }))
            .unwrap();
    }

    fn draw_color_triangles<T: DrawingBackend>(
        chart: &mut ChartContext<'_, T, Cartesian2d<RangedCoordf64, RangedCoordf64>>,
        triangle_index: &MatrixXx3<usize>,
        x: &DVector<f64>,
        y: &DVector<f64>,
        c: &DVector<f64>,
        cmap: &Gradient,
        cbounds: (f64, f64),
    ) {
        let z_min = cbounds.0;
        let z_max: f64 = cbounds.1 - z_min; //z.max();

        let series = izip!(triangle_index.row_iter(), c).map(|(idx, c)| {
            Polygon::new(
                vec![
                    (x[idx[0]], y[idx[0]]),
                    (x[idx[1]], y[idx[1]]),
                    (x[idx[2]], y[idx[2]]),
                ],
                {
                    let cor = cmap.eval_continuous((c - z_min) / z_max);
                    let color = RGBAColor(cor.r, cor.g, cor.b, 1.);
                    Into::<ShapeStyle>::into(color).filled()
                },
            )
        });

        chart.draw_series(series).unwrap();
    }

    fn draw_triangle_surf<T: DrawingBackend>(
        chart: &mut ChartContext<
            '_,
            T,
            Cartesian3d<RangedCoordf64, RangedCoordf64, RangedCoordf64>,
        >,
        triangle_index: &MatrixXx3<usize>,
        x: &DVector<f64>,
        y: &DVector<f64>,
        z: &DVector<f64>,
    ) {
        let series = triangle_index.row_iter().map(|idx| {
            Polygon::new(
                vec![
                    (x[idx[0]], y[idx[0]], z[idx[0]]),
                    (x[idx[1]], y[idx[1]], z[idx[1]]),
                    (x[idx[2]], y[idx[2]], z[idx[2]]),
                ],
                Into::<ShapeStyle>::into(RGBAColor(0, 0, 255, 0.2)).filled(),
            )
        });
        chart.draw_series(series).unwrap();
    }

    fn draw_2d_colormesh<T: DrawingBackend>(
        chart: &mut ChartContext<'_, T, Cartesian2d<RangedCoordf64, RangedCoordf64>>,
        x_ax: &MatrixXx1<f64>,
        y_ax: &MatrixXx1<f64>,
        z_dat: &DMatrix<f64>,
        cmap: &Gradient,
        cbounds: (f64, f64),
    ) {
        let mut x_dist = (x_ax[1] - x_ax[0]) / 2.;
        if x_dist <= 2. * f64::EPSILON {
            x_dist = 0.5;
        }
        let mut y_dist = (y_ax[1] - y_ax[0]) / 2.;
        if y_dist <= 2. * f64::EPSILON {
            y_dist = 0.5;
        }

        let (z_shape_rows, z_shape_cols) = z_dat.shape();

        if z_shape_rows != y_ax.len() || z_shape_cols != x_ax.len() {
            println!("Shapes of x,y and z do not match!");
            return;
        }

        //there will probably be a more direct way to achieve the series without this conversion to a vec<f64> when we can use nalgebra >=v0.32.
        //currently, clone is not implemented for matrix_iter in v0.30 which we use due to ncollide2d. Therefore, we go this way
        let a: Vec<f64> = x_ax.data.clone().into();
        let b: Vec<f64> = y_ax.data.clone().into();
        let z_min = cbounds.0;

        let z_max: f64 = cbounds.1 - z_min; //z.max();
        let series = izip!(iproduct!(a, b), z_dat).map(|((x, y), z)| {
            Rectangle::new([(x - x_dist, y + y_dist), (x + x_dist, y - y_dist)], {
                let cor = cmap.eval_continuous((z - z_min) / z_max);
                let color = RGBAColor(cor.r, cor.g, cor.b, 1.);
                Into::<ShapeStyle>::into(color).filled()
            })
        });

        chart.draw_series(series).unwrap();
    }

    fn plot_2d_line<B: DrawingBackend>(plt: &Plot, root: &DrawingArea<B, Shift>) {
        if let Some(PlotData::Dim2(dat)) = &plt.data {
            let (x_min, x_max) = if plt.bounds.x.is_none() {
                plt.define_axis_bounds(&dat.column(0), true, true)
            } else {
                plt.bounds.x.unwrap()
            };
            let (y_min, y_max) = if plt.bounds.x.is_none() {
                plt.define_axis_bounds(&dat.column(0), true, true)
            } else {
                plt.bounds.x.unwrap()
            };

            let mut chart = Self::create_2d_plot_chart(
                root,
                (x_min, x_max),
                (y_min, y_max),
                &plt.label,
                true,
                true,
            );
            Self::draw_line(&mut chart, &dat.column(0), &dat.column(1), &plt.color);
        }

        root.present().unwrap();
    }

    fn plot_2d_scatter<B: DrawingBackend>(plt: &Plot, root: &DrawingArea<B, Shift>) {
        if let Some(PlotData::Dim2(dat)) = plt.get_data() {
            _ = root.fill(&WHITE);

            let (x_min, x_max) = if plt.bounds.x.is_none() {
                plt.define_axis_bounds(&dat.column(0), true, true)
            } else {
                plt.bounds.x.unwrap()
            };
            let (y_min, y_max) = if plt.bounds.x.is_none() {
                plt.define_axis_bounds(&dat.column(0), true, true)
            } else {
                plt.bounds.x.unwrap()
            };

            let mut chart = Self::create_2d_plot_chart(
                root,
                (x_min, x_max),
                (y_min, y_max),
                &plt.label,
                true,
                true,
            );

            Self::draw_points(&mut chart, &dat.column(0), &dat.column(1), &plt.color);
        }

        root.present().unwrap();
    }

    fn plot_color_triangulated<B: DrawingBackend>(plt: &Plot, root: &DrawingArea<B, Shift>) {
        if let Some(PlotData::ColorTriangulated(triangle_index, color, dat)) = plt.get_data() {
            _ = root.fill(&WHITE);

            let (main_root, cbar_root) = root.split_horizontally(830);

            let (x_min, x_max) = if plt.bounds.x.is_none() {
                plt.define_axis_bounds(&DVectorSlice::from(dat.column(0)), false, false)
            } else {
                plt.bounds.x.unwrap()
            };
            let (y_min, y_max) = if plt.bounds.y.is_none() {
                plt.define_axis_bounds(&DVectorSlice::from(dat.column(1)), false, false)
            } else {
                plt.bounds.y.unwrap()
            };
            let (z_min, z_max) = if plt.bounds.z.is_none() {
                plt.define_axis_bounds(&DVectorSlice::from(dat.column(2)), false, false)
            } else {
                plt.bounds.z.unwrap()
            };

            //colorbar. first because otherwise the xlabel of the main plot is cropped
            let mut chart = Self::create_2d_plot_chart(
                &cbar_root,
                (0., 1.),
                (z_min, z_max),
                &[
                    LabelDescription::new("", plt.label[0].label_pos),
                    plt.cbar.label.clone(),
                ],
                true,
                false,
            );

            let c_dat = linspace(z_min, z_max, 100.).unwrap().transpose();
            let d_mat = DMatrix::<f64>::from_columns(&[c_dat.clone(), c_dat]);
            let xxx = DVector::<f64>::from_vec(vec![0., 1.]);
            Self::draw_2d_colormesh(
                &mut chart,
                &xxx,
                &linspace(z_min, z_max, 100.).unwrap().transpose(),
                &d_mat,
                &plt.cbar.cmap,
                (z_min, z_max),
            );

            //main plot
            let mut chart = Self::create_2d_plot_chart(
                &main_root,
                (x_min, x_max),
                (y_min, y_max),
                &plt.label,
                true,
                true,
            );
            Self::draw_color_triangles(
                &mut chart,
                triangle_index,
                &DVector::from(dat.column(0)),
                &DVector::from(dat.column(1)),
                color,
                &plt.cbar.cmap,
                (z_min, z_max),
            );
        }
    }

    fn plot_triangulated_surface<B: DrawingBackend>(plt: &Plot, root: &DrawingArea<B, Shift>) {
        if let Some(PlotData::TriangulatedSurface(triangle_index, dat)) = plt.get_data() {
            _ = root.fill(&WHITE);

            let (x_min, x_max) = if plt.bounds.x.is_none() {
                plt.define_axis_bounds(&DVectorSlice::from(dat.column(0)), false, false)
            } else {
                plt.bounds.x.unwrap()
            };
            let (y_min, y_max) = if plt.bounds.y.is_none() {
                plt.define_axis_bounds(&DVectorSlice::from(dat.column(1)), false, false)
            } else {
                plt.bounds.y.unwrap()
            };
            let (z_min, z_max) = if plt.bounds.z.is_none() {
                plt.define_axis_bounds(&DVectorSlice::from(dat.column(2)), false, false)
            } else {
                plt.bounds.z.unwrap()
            };

            //main plot
            //currently there is no support for axes labels in 3d plots
            let mut chart =
                Self::create_3d_plot_chart(root, (x_min, x_max), (z_min, z_max), (y_min, y_max));

            Self::draw_triangle_surf(
                &mut chart,
                triangle_index,
                &DVector::from(dat.column(0)),
                &DVector::from(dat.column(2)),
                &DVector::from(dat.column(1)),
            );
        }
    }
    fn plot_color_mesh<B: DrawingBackend>(plt: &Plot, root: &DrawingArea<B, Shift>) {
        if let Some(PlotData::ColorMesh(x, y, dat)) = plt.get_data() {
            _ = root.fill(&WHITE);
            //split root for main plot and colorbar
            let (main_root, cbar_root) = root.split_horizontally(830);

            let shape = dat.shape();
            let flattened_size = shape.0 * shape.1;
            let dat_flat = MatrixXx1::<f64>::from_iterator(flattened_size, dat.iter().copied());

            let (x_min, x_max) = if plt.bounds.x.is_none() {
                plt.define_axis_bounds(&DVectorSlice::from(x), false, false)
            } else {
                plt.bounds.x.unwrap()
            };
            let (y_min, y_max) = if plt.bounds.y.is_none() {
                plt.define_axis_bounds(&DVectorSlice::from(y), false, false)
            } else {
                plt.bounds.y.unwrap()
            };
            let (z_min, z_max) = if plt.bounds.z.is_none() {
                plt.define_axis_bounds(&DVectorSlice::from(&dat_flat), false, false)
            } else {
                plt.bounds.z.unwrap()
            };

            //colorbar. first because otherwise the xlabel of the main plot is cropped
            let mut chart = Self::create_2d_plot_chart(
                &cbar_root,
                (0., 1.),
                (z_min, z_max),
                &[
                    LabelDescription::new("", plt.label[0].label_pos),
                    plt.cbar.label.clone(),
                ],
                true,
                false,
            );

            let c_dat = linspace(z_min, z_max, 100.).unwrap().transpose();
            let d_mat = DMatrix::<f64>::from_columns(&[c_dat.clone(), c_dat]);
            let xxx = DVector::<f64>::from_vec(vec![0., 1.]);
            Self::draw_2d_colormesh(
                &mut chart,
                &xxx,
                &linspace(z_min, z_max, 100.).unwrap().transpose(),
                &d_mat,
                &plt.cbar.cmap,
                (z_min, z_max),
            );

            //main plot
            let mut chart = Self::create_2d_plot_chart(
                &main_root,
                (x_min, x_max),
                (y_min, y_max),
                &plt.label,
                true,
                true,
            );

            Self::draw_2d_colormesh(&mut chart, x, y, dat, &plt.cbar.cmap, (z_min, z_max));
        }

        root.present().unwrap();
    }

    fn create_3d_plot_chart<T: DrawingBackend>(
        root: &DrawingArea<T, Shift>,
        x_bounds: (f64, f64),
        y_bounds: (f64, f64),
        z_bounds: (f64, f64),
        // xlabel: &String,
        // ylabel: &String,
        // zlabel: &String,
        // xlabelpos: &LabelPos,
        // ylabelpos: &LabelPos,
        // zlabelpos: &LabelPos,
    ) -> ChartContext<'_, T, Cartesian3d<RangedCoordf64, RangedCoordf64, RangedCoordf64>> {
        root.fill(&WHITE).unwrap();

        let mut chart = ChartBuilder::on(root)
            .margin(20)
            .set_all_label_area_size(100)
            .build_cartesian_3d(
                x_bounds.0..x_bounds.1,
                y_bounds.0..y_bounds.1,
                z_bounds.0..z_bounds.1,
            )
            .unwrap();

        chart.with_projection(
            |mut pb: plotters::coord::ranged3d::ProjectionMatrixBuilder| {
                pb.pitch = 20. / 180. * PI;
                pb.yaw = 20. / 180. * PI;
                pb.scale = 0.7;
                pb.into_matrix()
            },
        );

        chart.configure_axes().draw().unwrap();

        chart
    }

    fn create_2d_plot_chart<'a, T: DrawingBackend>(
        root: &'a DrawingArea<T, Shift>,
        x_bounds: (f64, f64),
        y_bounds: (f64, f64),
        label_desc: &[LabelDescription; 2],
        // x_label: &String,
        // y_label: &String,
        // x_labelpos: LabelPos,
        // y_labelpos: LabelPos,
        y_ax: bool,
        x_ax: bool,
    ) -> ChartContext<'a, T, Cartesian2d<RangedCoordf64, RangedCoordf64>> {
        root.fill(&WHITE).unwrap();

        let mut chart_builder = ChartBuilder::on(root);
        chart_builder.margin(10).margin_top(40);

        if y_ax {
            //absolutely ugly "automation" of margin. not done nicely and not accurate, works only for sans serif with 30 pt
            let digits_max =
                y_bounds.1.abs().log10().floor() + 2. + f64::from(y_bounds.1.is_sign_negative());
            let digits_min =
                y_bounds.0.abs().log10().floor() + 2. + f64::from(y_bounds.0.is_sign_negative());
            let digits = if digits_max >= digits_min {
                digits_max.to_i32()
            } else {
                digits_min.to_i32()
            }
            .unwrap();

            let pixel_margin = digits * 13 + 20;
            chart_builder.set_label_area_size(label_desc[1].label_pos.into(), 21 + pixel_margin);

            if LabelPos::Right == label_desc[1].label_pos && (pixel_margin < 72) {
                chart_builder.margin_right(82 - pixel_margin);
            } else if LabelPos::Left == label_desc[1].label_pos && (pixel_margin < 72) {
                chart_builder.margin_left(82 - pixel_margin);
            }
        }
        chart_builder.set_label_area_size(label_desc[0].label_pos.into(), 65);

        let mut chart = chart_builder
            .build_cartesian_2d(x_bounds.0..x_bounds.1, y_bounds.0..y_bounds.1)
            .unwrap();

        let mut mesh = chart.configure_mesh();

        if y_ax {
            mesh.y_desc(&label_desc[1].label);
        } else {
            mesh.disable_y_axis();
        }

        if x_ax {
            mesh.x_desc(&label_desc[0].label);
        } else {
            mesh.disable_x_axis();
        }

        mesh.label_style(("sans-serif", 30).into_font())
            .draw()
            .unwrap();

        chart
    }
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

#[derive(Debug, Clone)]
///Enum to define the type of plot that should be created
pub enum PlotData {
    ///Pairwise 2D data (e.g. x, y data) for scatter2D, Line2D. Data Structure as Matrix with N rows and two columns (x,y)
    Dim2(MatrixXx2<f64>),
    ///Triplet 3D data (e.g. x, y, z data) for scatter3D, Line3D or colorscatter. Data Structure as Matrix with N rows and three columns (x,y,z)
    Dim3(MatrixXx3<f64>),
    // ///Vector of pairwise 2D data (e.g. x, y data) for MultiLine2D. Data Structure as Vector filled with Matrices with N rows and two columns (x,y)
    // MultiDim2(Vec<MatrixXx2<f64>>),
    // ///Vector of triplet 3D data (e.g. x, y, z data) for MultiLine3D. Data Structure as Vector filled with Matrices with N rows and three columns (x,y,z)
    // MultiDim3(Vec<MatrixXx3<f64>>),
    /// Data to create a 2d colormesh plot. Vector with N entries for x, Vector with M entries for y and a Matrix with NxM entries for the colordata
    ColorMesh(DVector<f64>, DVector<f64>, DMatrix<f64>), // ColorScatter(DVector<f64>, DVector<f64>, DMatrix<f64>)
    /// Data to create a 2d triangulated color plot.
    /// Matrix with 3 columns and N rows that is filled with the indices that correspond to the data points that ave been triangulated
    /// Vector with N rows which holds the average color value of the triangle
    /// Matrix with 3 columns and N rows that hold the x,y,z data
    ColorTriangulated(MatrixXx3<usize>, DVector<f64>, MatrixXx3<f64>),
    /// Data to create a 3d triangulated surface plot.
    /// Matrix with 3 columns and N rows that is filled with the indices that correspond to the data points that ave been triangulated
    /// Matrix with 3 columns and N rows that hold the x,y,z data
    TriangulatedSurface(MatrixXx3<usize>, MatrixXx3<f64>),
}

impl PlotData {
    /// This function tries to find the valid data range of the provided data. It returns a (f64,f64,f64) tuple with the first value being the full range of the data, the second value the  minimum value of the data and the last one the maximum value of the data.
    /// If min and max are approximately equal, the range ist set to the maximum value and the minimum value is set to 0
    /// if the maximum is zero AND approximately equal to the minimum, then it is set to 1 and the minimum to zero to avoid awkward ax scalings
    #[must_use]
    pub fn get_min_max_range(&self, ax_vals: &DVectorSlice<'_, f64>) -> (f64, f64, f64) {
        let mut max_val = ax_vals.max();
        let mut min_val = ax_vals.min();
        let mut ax_range = max_val - min_val;

        //check if minimum and maximum values are approximately equal. if so, take the max value as range
        if max_val.relative_eq(&min_val, f64::EPSILON, f64::EPSILON) {
            ax_range = max_val;
            min_val = 0.;
        };

        //check if for some reason maximum is 0, then set it to 1, so that the axis spans at least some distance
        if ax_range < f64::EPSILON {
            max_val = 1.;
            min_val = 0.;
            ax_range = 1.;
        };

        (ax_range, min_val, max_val)
    }
}

/// Trait for adding the possibility to generate a (x/y) plot of an element.
pub trait Plottable {
    /// This method must be implemented in order to retrieve the plot data.
    /// As the plot data may differ, the implementation must be done for each kind of plot type [`PlotType`]
    /// # Attributes
    /// - `plt_type`: plot type to be used. See [`PlotType`]
    /// # Returns
    /// This method returns an [`OpmResult<Option<PlotData>>`]. Whether Some(PlotData) or None is returned depends on the individual implementation
    /// # Errors
    /// Whether an error is thrown depends on the individual implementation of the method
    fn get_plot_data(&self, plt_type: &PlotType) -> OpmResult<Option<PlotData>>;

    /// This method must be implemented in order to create a plot.
    /// As the plot data may differ, the implementation must be done for each kind of plot
    /// # Attributes
    /// - `f_path`: path to the file
    /// - `img_size`: the size of the image in pixels
    /// - `backend`: used backend to create the plot. See [`PltBackEnd`]
    /// # Errors
    /// Whether an error is thrown depends on the individual implementation of the method
    fn to_plot(
        &self,
        f_path: &Path,
        img_size: (u32, u32),
        backend: PltBackEnd,
    ) -> OpmResult<Option<RgbImage>>;

    /// This method triangulates [`PlotData`] of the variant Dim3,
    /// # Attributes
    /// - `plt_dat`: plot data
    /// # Returns
    /// This method returns [`Option<PlotData>`]. It is None if the [`PlotData`] Variant is not Dim3
    fn triangulate_plot_data(&self, plt_dat: &PlotData, plt_type: &PlotType) -> Option<PlotData> {
        if let PlotData::Dim3(dat) = plt_dat {
            let points: Vec<Point> = dat
                .row_iter()
                .map(|c| Point { x: c[0], y: c[1] })
                .collect::<Vec<Point>>();
            let tri_index_mat = Matrix3xX::from_vec(triangulate(&points).triangles).transpose();

            if let PlotType::ColorTriangulated(_) = plt_type {
                let mut triangle_centroid_z = DVector::<f64>::zeros(tri_index_mat.column(0).len());

                for (c, t) in izip!(tri_index_mat.row_iter(), triangle_centroid_z.iter_mut()) {
                    *t = (dat[(c[0], 2)] + dat[(c[1], 2)] + dat[(c[2], 2)]) / 3.;
                }
                Some(PlotData::ColorTriangulated(
                    tri_index_mat,
                    triangle_centroid_z,
                    dat.clone(),
                ))
            } else {
                Some(PlotData::TriangulatedSurface(tri_index_mat, dat.clone()))
            }
        } else {
            None
        }
    }

    /// This method bins 2d scatter data to a regularly gridded mesh
    /// # Attributes
    /// - `plt_dat`: plot data
    /// # Returns
    /// This method returns [`Option<PlotData>`]. It is None if the [`PlotData`] Variant is not Dim3
    fn bin_2d_scatter_data(&self, plt_dat: &PlotData) -> Option<PlotData> {
        if let PlotData::Dim3(dat) = plt_dat {
            let (x_range, x_min, x_max) = plt_dat.get_min_max_range(&dat.column(0));
            let (y_range, y_min, y_max) = plt_dat.get_min_max_range(&dat.column(1));

            let num_entries = dat.column(0).len();
            let mut num = f64::sqrt((num_entries / 2).to_f64().unwrap()).floor();

            if (num % 2.).relative_eq(&0., f64::EPSILON, f64::EPSILON) {
                num += 1.;
            }

            let xbin = x_range / (num - 1.0);
            let ybin = y_range / (num - 1.0);

            let x = linspace(x_min - xbin / 2., x_max + xbin / 2., num).unwrap();
            let y = linspace(y_min - ybin / 2., y_max + ybin / 2., num).unwrap();

            let xbin = x[1] - x[0];
            let ybin = y[1] - y[0];
            let x_min = x.min();
            let y_min = y.min();

            let mut zz = DMatrix::<f64>::zeros(x.len(), y.len());
            // xx.clone() * 0.;
            let mut zz_counter = DMatrix::<f64>::zeros(x.len(), y.len()); //xx.clone() * 0.;

            for row in dat.row_iter() {
                let x_index = ((row[(0, 0)] - x_min + xbin / 2.) / xbin).to_usize();
                let y_index = ((row[(0, 1)] - y_min + ybin / 2.) / ybin).to_usize();
                if x_index.is_some() && y_index.is_some() {
                    let x_index = x_index.unwrap();
                    let y_index = y_index.unwrap();
                    zz[(y_index, x_index)] += row[(0, 2)];
                    zz_counter[(y_index, x_index)] += 1.;
                }
            }
            for (z, z_count) in izip!(zz.iter_mut(), zz_counter.iter()) {
                if *z_count > 0.5 {
                    *z /= *z_count;
                }
            }

            Some(PlotData::ColorMesh(x.transpose(), y.transpose(), zz))
        } else {
            None
        }
    }

    // fn plot_2d_line<B: DrawingBackend>(
    //     &self,
    //     plt_data:       &PlotData,
    //     marker_color:   RGBAColor,
    //     expand_bounds:  Vec<[bool;2]>,
    //     xlabel: &str,
    //     ylabel: &str,
    //     root: &DrawingArea<B, Shift>
    // ) -> OpmResult<()>{

    //     if let PlotData::Dim2(dat) = plt_data{
    //         let (x_min, x_max) = self.define_axis_bounds(&dat.column(0), true, true);
    //         let (y_min, y_max) = self.define_axis_bounds(&dat.column(1), true, true);

    //         let mut chart = self.create_2d_plot_chart(
    //             &root,
    //             [x_min, x_max, y_min, y_max],
    //             xlabel,
    //             ylabel
    //             )?;

    //         self.draw_line(&mut chart, &dat.column(0), &dat.column(1), &marker_color);
    //     }

    //     root.present().unwrap();

    //     Ok(())
    // }

    // fn plot_2d_scatter<B: DrawingBackend>(
    //     &self,
    //     plt_data:       &PlotData,
    //     marker_color:   RGBAColor,
    //     expand_bounds:  Vec<[bool;2]>,
    //     xlabel: &str,
    //     ylabel: &str,
    //     root: &DrawingArea<B, Shift>
    // ) -> OpmResult<()>{

    //     if let PlotData::Dim2(dat) = plt_data{
    //         let (x_min, x_max) = self.define_axis_bounds(&dat.column(0), true, true);
    //         let (y_min, y_max) = self.define_axis_bounds(&dat.column(1), true, true);

    //         let mut chart = self.create_2d_plot_chart(
    //             &root,
    //             [x_min, x_max, y_min, y_max],
    //             xlabel,
    //             ylabel
    //             )?;

    //         self.draw_points(&mut chart, &dat.column(0), &dat.column(1), &marker_color);
    //     }

    //     root.present().unwrap();
    //     Ok(())
    // }

    // fn draw_line<'a, T: DrawingBackend>(
    //     &self,
    //     chart:      &mut ChartContext<'a, T, Cartesian2d<RangedCoordf64, RangedCoordf64>>,
    //     x:          &MatrixSliceXx1<f64>,
    //     y:          &MatrixSliceXx1<f64>,
    //     line_color: &RGBAColor
    // ){
    //     chart
    //         .draw_series(LineSeries::new(
    //             izip!(x, y)
    //                  .map(|xy| (*xy.0, *xy.1)),
    //            line_color)
    //         ).unwrap();
    // }

    // fn draw_points<'a, T: DrawingBackend>(
    //     &self,
    //     chart:          &mut ChartContext<'a, T, Cartesian2d<RangedCoordf64, RangedCoordf64>>,
    //     x:              &MatrixSliceXx1<f64>,
    //     y:              &MatrixSliceXx1<f64>,
    //     marker_color:   &RGBAColor
    // ){
    //     chart.draw_series(
    //         izip!(x, y).map(|x| Circle::new((*x.0, *x.1), 5, Into::<ShapeStyle>::into(marker_color).filled())),
    //     ).unwrap();
    // }

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

    //// Generate a plot of the element as SVG file with the given path.
    //// # Attributes
    //// `f_path`: path to the file destination
    ////
    //// # Errors
    //// This function will return an error if
    ////  - the given path is not writable or does not exist.
    ////  - the plot area cannot be filled with a background colour.
    // fn to_svg_plot(&self, f_path: &Path) -> OpmResult<()> {
    //     let root = SVGBackend::new(f_path, (800, 800)).into_drawing_area();
    //     root.fill(&WHITE)
    //         .map_err(|e| format!("filling plot background failed: {e}"))?;
    //     self.chart(&root)
    // }
    //// Generate a plot of the given element as an image buffer.
    ////
    //// # Errors
    ////
    //// This function will return an error if
    ////  - the plot area cannot be filled.
    ////  - the image buffer cannot be allocated or has the wrong size.
    // fn to_img_buf_plot(&self, img_size: (u32, u32)) -> OpmResult<RgbImage> {
    //     let (image_width, image_height) = img_size;
    //     let mut image_buffer = vec![
    //         0;
    //         (image_width * image_height) as usize
    //             * plotters::backend::RGBPixel::PIXEL_SIZE
    //     ];
    //     {
    //         let root = BitMapBackend::with_buffer(&mut image_buffer, (image_width, image_height))
    //             .into_drawing_area();
    //         root.fill(&WHITE)
    //             .map_err(|e| format!("filling plot background failed: {e}"))?;
    //         self.create_plot(&root)?;
    //     }
    //     let img = RgbImage::from_raw(image_width, image_height, image_buffer)
    //         .ok_or_else(|| OpossumError::Other("image buffer size too small".into()))?;
    //     Ok(img)
    // }
}

///Enum to describe which type of plotting backend should be used
#[derive(Clone, Debug, Default)]
pub enum PltBackEnd {
    /// BitmapBackend. Used to create .png, .bmp, .jpg
    #[default]
    BMP,
    /// SVGBackend. Used to create .svg
    SVG,
    /// Buffered Backend. Used to buffer the image data into an image buffer.
    Buf,
}

///Struct to hold the color gradient information of a [`ColorBar`]
#[derive(Debug, Clone, Copy)]
pub struct CGradient {
    gradient: Gradient,
}
impl CGradient {
    /// Returns the gradient of this [`CGradient`].
    #[must_use]
    pub const fn get_gradient(&self) -> Gradient {
        self.gradient
    }
}
impl Default for CGradient {
    #[must_use]
    fn default() -> Self {
        Self {
            gradient: colorous::TURBO,
        }
    }
}

///Enum to hold the information to position an axis label
#[derive(Debug, Eq, PartialEq, Clone, Copy, Default)]
pub enum LabelPos {
    ///Label on the top. Only for x axis
    Top,
    ///Label on the bottom. Only for x axis
    Bottom,
    ///Label on the left. Only for y axis
    #[default]
    Left,
    ///Label on the right. Only for y axis
    Right,
}

impl From<LabelPos> for LabelAreaPosition {
    fn from(val: LabelPos) -> Self {
        match val {
            LabelPos::Top => Self::Top,
            LabelPos::Bottom => Self::Bottom,
            LabelPos::Left => Self::Left,
            LabelPos::Right => Self::Right,
        }
    }
}

///Struct to hold the information to describe and set up an axis label
#[derive(Clone)]
pub struct LabelDescription {
    label: String,
    label_pos: LabelPos,
}
impl LabelDescription {
    /// Creates a new [`LabelDescription`].
    /// # Attributes
    /// - `label`: text to be shown as label
    /// - `label_pos`: position of the label. See [`LabelPos`]
    /// # Returns
    /// This method returns a new `LabelDescription` struct
    #[must_use]
    pub fn new(label: &str, label_pos: LabelPos) -> Self {
        Self {
            label: label.to_owned(),
            label_pos,
        }
    }

    /// Returns a [`LabelDescription`] with a default value for the y axis, which is "y" as the label and is positioned to the left of the plot.
    #[must_use]
    pub fn y_default(&self) -> Self {
        Self::new("y", LabelPos::Left)
    }

    /// Sets the label position of this [`LabelDescription`].
    /// # Attributes
    /// - `pos`: position of the label. See [`LabelPos`]
    pub fn set_label_pos(&mut self, pos: LabelPos) {
        self.label_pos = pos;
    }

    /// Sets the label of this [`LabelDescription`].
    /// # Attributes
    /// - `txt`: text to be shown as label
    pub fn set_label(&mut self, txt: &str) {
        self.label = txt.to_owned();
    }
}

///Struct to hold the information to set up a colorbar
#[derive(Clone)]
pub struct ColorBar {
    cmap: Gradient,
    label: LabelDescription,
}
impl ColorBar {
    /// Creates a new [`ColorBar`].
    /// /// # Attributes
    /// - `cmap`: [`Gradient`] to be used in the colorbar
    /// - `label`: text to be shown as label
    /// - `label_pos`: position of the label. See [`LabelPos`]
    /// # Returns
    /// This method returns a new colorbar struct
    #[must_use]
    pub fn new(cmap: Gradient, label: &str, label_pos: LabelPos) -> Self {
        Self {
            cmap,
            label: LabelDescription::new(label, label_pos),
        }
    }

    /// Sets the label of this [`ColorBar`].
    /// # Attributes
    /// - `pos`: position of the label. See [`LabelPos`]
    pub fn set_pos(&mut self, pos: LabelPos) {
        self.label.label_pos = pos;
    }

    /// Sets the label of this [`ColorBar`].
    /// # Attributes
    /// - `txt`: text to be shown as label
    pub fn set_label(&mut self, txt: &str) {
        self.label.label = txt.to_owned();
    }
}

impl Default for ColorBar {
    #[must_use]
    fn default() -> Self {
        Self {
            cmap: colorous::INFERNO,
            label: LabelDescription::new("", LabelPos::Right),
        }
    }
}

/// Struct to hold the plot boundaries of the plot in the x, y, z axes.
/// The values may also be None. Then, reasonable boundaries are chosen automatically
#[derive(Clone)]
pub struct PlotBounds {
    x: Option<(f64, f64)>,
    y: Option<(f64, f64)>,
    z: Option<(f64, f64)>,
}

/// Holds all necessary plot parameters in a Hashmap that contains a String-key and an [`PlotArgs`] argument.
#[derive(Debug, Clone)]
pub struct PlotParameters {
    params: HashMap<String, PlotArgs>,
}

impl Default for PlotParameters {
    ///This method creates a new [`PlotParameters`] struct that is filled by default values
    /// Default values are:
    /// - `PlotArgs::Backend`: `PltBackEnd::BMP`
    /// - `PlotArgs::XLabel`: `x`
    /// - `PlotArgs::XLabelPos`: `LabelPos::Bottom`
    /// - `PlotArgs::YLabel`: `y`
    /// - `PlotArgs::YLabelPos`: `LabelPos::Left`
    /// - `PlotArgs::CBarLabel`: `z value`
    /// - `PlotArgs::CBarLabelPos`: `LabelPos::Right`
    /// - `PlotArgs::XLim`: `None`
    /// - `PlotArgs::YLim`: `None`
    /// - `PlotArgs::ZLim`: `None`
    /// - `PlotArgs::CMap`: `colorous::TURBO`
    /// - `PlotArgs::Color`: `RGBAColor(255, 0, 0, 1.)`
    /// - `PlotArgs::FDir`: `current directory`
    /// - `PlotArgs::FName`: `opossum_default_plot_{i}.png`. Here, i is chosen such that no file is overwritten, but a new file is generated
    /// - `PlotArgs::FigSize`: `(1000, 850)`
    /// # Returns
    /// This method returns a new [`PlotParameters`] struct
    /// # Panics
    /// This method panics if the current working directory is invalid. See `std::env:current_dir()`
    #[must_use]
    fn default() -> Self {
        let current_dir = current_dir().unwrap().to_str().unwrap().to_owned() + "\\";
        let mut i = 0;
        loop {
            let fpath = current_dir.clone() + format!("opossum_default_plot_{i}.png").as_str();
            let path = Path::new(&fpath);
            if !path.exists() {
                break;
            }
            i += 1;
        }
        let mut plt_params = Self {
            params: HashMap::new(),
        };
        plt_params
            .set(&PlotArgs::Backend(PltBackEnd::BMP))
            .set(&PlotArgs::XLabel("x".into()))
            .set(&PlotArgs::XLabelPos(LabelPos::Bottom))
            .set(&PlotArgs::YLabel("y".into()))
            .set(&PlotArgs::YLabelPos(LabelPos::Left))
            .set(&PlotArgs::CBarLabel("z value".into()))
            .set(&PlotArgs::CBarLabelPos(LabelPos::Right))
            .set(&PlotArgs::XLim(None))
            .set(&PlotArgs::YLim(None))
            .set(&PlotArgs::ZLim(None))
            .set(&PlotArgs::CMap(CGradient::default()))
            .set(&PlotArgs::Color(RGBAColor(255, 0, 0, 1.)))
            .set(&PlotArgs::FDir(current_dir))
            .set(&PlotArgs::FName(format!("opossum_default_plot_{i}.png")))
            .set(&PlotArgs::FigSize((1000, 850)));

        plt_params
    }
}

impl PlotParameters {
    ///This method creates a new empty [`PlotParameters`] struct
    /// # Returns
    /// This method returns a new [`PlotParameters`] struct
    #[must_use]
    pub fn empty() -> Self {
        Self {
            params: HashMap::new(),
        }
    }

    ///This method creates a new [`PlotParameters`] struct and inserts the passed [`PlotArgs`]
    /// # Attributes
    /// - `plt_args`: Vector of Plot Arguments
    /// # Returns
    /// This method returns a new [`PlotParameters`] struct
    #[must_use]
    pub fn new(plt_args: Vec<PlotArgs>) -> Self {
        let mut p_i_params = Self {
            params: HashMap::new(),
        };
        for plt_arg in plt_args {
            p_i_params.insert(&plt_arg);
        }

        for plt_arg in PlotArgs::iter() {
            if !p_i_params.check_if_set(&plt_arg) {
                p_i_params.insert(&plt_arg);
            }
        }
        p_i_params
    }

    ///This method gets the x label which is stored in the [`PlotParameters`]
    /// # Returns
    /// This method returns an [`OpmResult<String>`] containing the label of the x axis
    /// # Errors
    /// This method throws an error if the argument is not found
    pub fn get_x_label(&self) -> OpmResult<String> {
        if let Some(PlotArgs::XLabel(x_label)) = self.params.get("xlabel") {
            Ok(x_label.clone())
        } else {
            Err(OpossumError::Other("xlabel argument not found!".into()))
        }
    }

    ///This method gets the y label which is stored in the [`PlotParameters`]
    /// # Returns
    /// This method returns an [`OpmResult<String>`] containing the label of the y axis
    /// # Errors
    /// This method throws an error if the argument is not found
    pub fn get_y_label(&self) -> OpmResult<String> {
        if let Some(PlotArgs::YLabel(y_label)) = self.params.get("ylabel") {
            Ok(y_label.clone())
        } else {
            Err(OpossumError::Other("ylabel argument not found!".into()))
        }
    }

    ///This method gets the position of the x label which is stored in the [`PlotParameters`]
    /// # Returns
    /// This method returns an [`OpmResult<LabelPos>`] containing the [`LabelPos`] of the x axis
    /// # Errors
    /// This method throws an error if the argument is not found
    pub fn get_y_label_pos(&self) -> OpmResult<LabelPos> {
        if let Some(PlotArgs::YLabelPos(y_label_pos)) = self.params.get("ylabelpos") {
            Ok(*y_label_pos)
        } else {
            Err(OpossumError::Other("ylabelpos argument not found!".into()))
        }
    }

    ///This method gets the position of the y label which is stored in the [`PlotParameters`]
    /// # Returns
    /// This method returns an [`OpmResult<LabelPos>`] containing the [`LabelPos`] of the y axis
    /// # Errors
    /// This method throws an error if the argument is not found
    pub fn get_x_label_pos(&self) -> OpmResult<LabelPos> {
        if let Some(PlotArgs::XLabelPos(x_label_pos)) = self.params.get("xlabelpos") {
            Ok(*x_label_pos)
        } else {
            Err(OpossumError::Other("xlabelpos argument not found!".into()))
        }
    }

    ///This method gets the [`RGBAColor`] which is stored in the [`PlotParameters`]
    /// # Returns
    /// This method returns an [`OpmResult<RGBAColor>`] containing the color information
    /// # Errors
    /// This method throws an error if the argument is not found
    pub fn get_color(&self) -> OpmResult<RGBAColor> {
        if let Some(PlotArgs::Color(c)) = self.params.get("color") {
            Ok(*c)
        } else {
            Err(OpossumError::Other("color argument not found!".into()))
        }
    }

    ///This method gets the [`PltBackEnd`] which is stored in the [`PlotParameters`]
    /// # Returns
    /// This method returns an [`OpmResult<PltBackEnd>`] containing the variant of the backend struct
    /// # Errors
    /// This method throws an error if the argument is not found
    pub fn get_backend(&self) -> OpmResult<PltBackEnd> {
        if let Some(PlotArgs::Backend(backend)) = self.params.get("backend") {
            Ok(backend.clone())
        } else {
            Err(OpossumError::Other("backend argument not found!".into()))
        }
    }

    ///This method gets the [`CGradient`] which is stored in the [`PlotParameters`]
    /// # Returns
    /// This method returns an [`OpmResult<CGradient>`] containing the colorbar gradient information
    /// # Errors
    /// This method throws an error if the argument is not found
    pub fn get_cmap(&self) -> OpmResult<CGradient> {
        if let Some(PlotArgs::CMap(cmap)) = self.params.get("cmap") {
            Ok(*cmap)
        } else {
            Err(OpossumError::Other("cmap argument not found!".into()))
        }
    }

    ///This method gets the label of the colorbar label which is stored in the [`PlotParameters`]
    /// # Returns
    /// This method returns an [`OpmResult<String>`] containing the colorbar label
    /// # Errors
    /// This method throws an error if the argument is not found
    pub fn get_cbar_label(&self) -> OpmResult<String> {
        if let Some(PlotArgs::CBarLabel(cbar_label)) = self.params.get("cbarlabel") {
            Ok(cbar_label.clone())
        } else {
            Err(OpossumError::Other("cbarlabel argument not found!".into()))
        }
    }

    ///This method gets the position of the colorbar which is stored in the [`PlotParameters`]
    /// # Returns
    /// This method returns an [`OpmResult<LabelPos>`] containing the [`LabelPos`]
    /// # Errors
    /// This method throws an error if the argument is not found
    pub fn get_cbar_label_pos(&self) -> OpmResult<LabelPos> {
        if let Some(PlotArgs::CBarLabelPos(cbar_label_pos)) = self.params.get("cbarlabelpos") {
            Ok(*cbar_label_pos)
        } else {
            Err(OpossumError::Other(
                "cbarlabelpos argument not found!".into(),
            ))
        }
    }

    ///This method gets the file name which is stored in the [`PlotParameters`]
    /// # Returns
    /// This method returns an [`OpmResult<String>`] containing the file name
    /// # Errors
    /// This method throws an error if the argument is not found
    pub fn get_fname(&self) -> OpmResult<String> {
        if let Some(PlotArgs::FName(fname)) = self.params.get("fname") {
            Ok(fname.clone())
        } else {
            Err(OpossumError::Other("fpath argument not found!".into()))
        }
    }

    ///This method gets the file path which is stored in the [`PlotParameters`]
    /// # Returns
    /// This method returns an [`OpmResult<String>`] containing the file path
    /// # Errors
    /// This method throws an error if the argument is not found
    pub fn get_fpath(&self) -> OpmResult<String> {
        let fdir = self.get_fdir()?;
        let fname = self.get_fname()?;

        Ok(fdir + "/" + fname.as_str())
    }

    ///This method gets the file directory which is stored in the [`PlotParameters`]
    /// # Returns
    /// This method returns an [`OpmResult<String>`] containing the file directory
    /// # Errors
    /// This method throws an error if the argument is not found
    pub fn get_fdir(&self) -> OpmResult<String> {
        if let Some(PlotArgs::FDir(fdir)) = self.params.get("fdir") {
            Ok(fdir.clone())
        } else {
            Err(OpossumError::Other("fdir argument not found!".into()))
        }
    }

    ///This method gets the x limit which is stored in the [`PlotParameters`]
    /// # Returns
    /// This method returns an [`OpmResult<Option<(f64, f64)>>`] with the min and max of the x values as f64
    /// # Errors
    /// This method throws an error if the argument is not found
    pub fn get_xlim(&self) -> OpmResult<Option<(f64, f64)>> {
        if let Some(PlotArgs::XLim(xlim)) = self.params.get("xlim") {
            Ok(*xlim)
        } else {
            Err(OpossumError::Other("xlim argument not found!".into()))
        }
    }

    ///This method gets the y limit which is stored in the [`PlotParameters`]
    /// # Returns
    /// This method returns an [`OpmResult<Option<(f64, f64)>>`] with the min and max of the y values as f64
    /// # Errors
    /// This method throws an error if the argument is not found
    pub fn get_ylim(&self) -> OpmResult<Option<(f64, f64)>> {
        if let Some(PlotArgs::YLim(ylim)) = self.params.get("ylim") {
            Ok(*ylim)
        } else {
            Err(OpossumError::Other("ylim argument not found!".into()))
        }
    }

    ///This method gets the z limit which is stored in the [`PlotParameters`]
    /// # Returns
    /// This method returns an [`OpmResult<Option<(f64, f64)>>`] with the min and max of the z values as f64
    /// # Errors
    /// This method throws an error if the argument is not found
    pub fn get_zlim(&self) -> OpmResult<Option<(f64, f64)>> {
        if let Some(PlotArgs::ZLim(zlim)) = self.params.get("zlim") {
            Ok(*zlim)
        } else {
            Err(OpossumError::Other("zlim argument not found!".into()))
        }
    }

    ///This method gets the figure size which is stored in the [`PlotParameters`]
    /// # Returns
    /// This method returns an [`OpmResult<(u32, u32)>`] with the width and height in number of pixels as u32
    /// # Errors
    /// This method throws an error if the argument is not found
    pub fn get_figsize(&self) -> OpmResult<(u32, u32)> {
        if let Some(PlotArgs::FigSize(figsize)) = self.params.get("figsize") {
            Ok(*figsize)
        } else {
            Err(OpossumError::Other("figsize argument not found!".into()))
        }
    }

    fn check_if_set(&self, plt_arg: &PlotArgs) -> bool {
        let mut found = false;
        for param_val in self.params.values() {
            if std::mem::discriminant(param_val) == std::mem::discriminant(plt_arg) {
                found = true;
                break;
            }
        }
        found
    }

    fn get_plt_arg_key(plt_arg: &PlotArgs) -> String {
        match plt_arg {
            PlotArgs::XLabel(_) => "xlabel".to_owned(),
            PlotArgs::YLabel(_) => "ylabel".to_owned(),
            PlotArgs::XLabelPos(_) => "xlabelpos".to_owned(),
            PlotArgs::YLabelPos(_) => "ylabelpos".to_owned(),
            PlotArgs::Color(_) => "color".to_owned(),
            PlotArgs::CMap(_) => "cmap".to_owned(),
            PlotArgs::XLim(_) => "xlim".to_owned(),
            PlotArgs::YLim(_) => "ylim".to_owned(),
            PlotArgs::ZLim(_) => "zlim".to_owned(),
            PlotArgs::FigSize(_) => "figsize".to_owned(),
            PlotArgs::CBarLabelPos(_) => "cbarlabelpos".to_owned(),
            PlotArgs::CBarLabel(_) => "cbarlabel".to_owned(),
            PlotArgs::FDir(_) => "fdir".to_owned(),
            PlotArgs::FName(_) => "fname".to_owned(),
            PlotArgs::Backend(_) => "backend".to_owned(),
        }
    }

    ///This method sets a plot argument ([`PlotArgs`]) to [`PlotParameters`]
    /// # Attributes
    /// - `plt_arg`: plot argument [`PlotArgs`]
    /// # Returns
    /// This method returns a mutable reference to the changed [`PlotParameters`]
    pub fn set(&mut self, plt_arg: &PlotArgs) -> &mut Self {
        let key = Self::get_plt_arg_key(plt_arg);
        if self.check_if_set(plt_arg) {
            self.params.remove_entry(&key);
        }
        self.insert(plt_arg);
        self
    }

    /// This method checks if
    /// - the path to the save-directory is valid
    /// - the backend matches with the set file extension
    ///
    /// # Errors
    /// - if the file directory does not exist
    /// - if the wrong backend or wrong file extension was chosen
    pub fn check_validity(&self) -> OpmResult<()> {
        let fdir = self.get_fdir()?;
        let dir_path = Path::new(&fdir);

        let (valid_backend, err_msg) = self.check_backend_file_ext_compatibility()?;
        let mut err_path = String::new();
        let valid_path = if dir_path.exists() {
            true
        } else {
            err_path.push_str(format!("File-directory path \"{fdir}\" is not valid!\n\n").as_str());
            false
        };

        if valid_path && valid_backend {
            Ok(())
        } else {
            err_path.push_str(err_msg.to_string().as_str());
            Err(OpossumError::Other(err_path))
        }
    }

    /// This method checks if compatibility between the chosen [`PltBackEnd`] and the file extension
    /// # Attributes
    /// - `path_fname`: name of the file
    /// - `backend`: backend to plot with. See [`PltBackEnd`]
    /// # Returns
    /// Returns a tuple consisting of a boolean and a potential error message
    /// The boolean is true if the backend and fname are compatible. False if not
    fn check_backend_file_ext_compatibility(&self) -> OpmResult<(bool, &str)> {
        let backend = self.get_backend()?;
        let path_fname = self.get_fname()?;

        match backend {
            PltBackEnd::BMP => {
                if std::path::Path::new(&path_fname)
                    .extension()
                    .map_or(false, |ext| ext.eq_ignore_ascii_case("png"))
                    || std::path::Path::new(&path_fname)
                        .extension()
                        .map_or(false, |ext| ext.eq_ignore_ascii_case("bmp"))
                    || std::path::Path::new(&path_fname)
                        .extension()
                        .map_or(false, |ext| ext.eq_ignore_ascii_case("jpg"))
                {
                    Ok((true, ""))
                } else {
                    Ok((false, "Incompatible file extension for DrawingBackend: BitmapBackend! Choose \".jpg\", \".bmp\" or \".png\" for this type of backend!"))
                }
            }
            PltBackEnd::SVG => {
                if std::path::Path::new(&path_fname)
                    .extension()
                    .map_or(false, |ext| ext.eq_ignore_ascii_case("svg"))
                {
                    Ok((true, ""))
                } else {
                    Ok((false, "Incompatible file extension for DrawingBackend: SVGBackend! Choose \".svg\"for this type of backend!"))
                }
            }
            PltBackEnd::Buf => Ok((true, "")),
        }
    }

    fn insert(&mut self, plt_arg: &PlotArgs) {
        match plt_arg {
            PlotArgs::XLabel(_) => self.params.insert("xlabel".to_owned(), plt_arg.clone()),
            PlotArgs::YLabel(_) => self.params.insert("ylabel".to_owned(), plt_arg.clone()),
            PlotArgs::XLabelPos(_) => self.params.insert("xlabelpos".to_owned(), plt_arg.clone()),
            PlotArgs::YLabelPos(_) => self.params.insert("ylabelpos".to_owned(), plt_arg.clone()),
            PlotArgs::Color(_) => self.params.insert("color".to_owned(), plt_arg.clone()),
            PlotArgs::CMap(_) => self.params.insert("cmap".to_owned(), plt_arg.clone()),
            PlotArgs::XLim(_) => self.params.insert("xlim".to_owned(), plt_arg.clone()),
            PlotArgs::YLim(_) => self.params.insert("ylim".to_owned(), plt_arg.clone()),
            PlotArgs::ZLim(_) => self.params.insert("zlim".to_owned(), plt_arg.clone()),
            PlotArgs::FigSize(_) => self.params.insert("figsize".to_owned(), plt_arg.clone()),
            PlotArgs::CBarLabelPos(_) => self
                .params
                .insert("cbarlabelpos".to_owned(), plt_arg.clone()),
            PlotArgs::CBarLabel(_) => self.params.insert("cbarlabel".to_owned(), plt_arg.clone()),
            PlotArgs::FDir(_) => self.params.insert("fdir".to_owned(), plt_arg.clone()),
            PlotArgs::FName(_) => self.params.insert("fname".to_owned(), plt_arg.clone()),
            PlotArgs::Backend(_) => self.params.insert("backend".to_owned(), plt_arg.clone()),
        };
    }
}

/// Struct that holds all necessary attributes to create a plot, such as [`PlotData`], [`PlotBounds`] etc
#[derive(Clone)]
pub struct Plot {
    label: [LabelDescription; 2],
    cbar: ColorBar,
    color: RGBAColor,
    data: Option<PlotData>,
    bounds: PlotBounds,
    img_size: (u32, u32),
}

impl Plot {
    /// creates a new [`Plot`]
    /// # Attributes
    /// - reference to a [`PlotData`]
    /// - reference to [`PlotParameters`]
    /// # Returns
    /// This function returns a [`Plot`] struct
    /// # Panics
    /// This method panics if the [`Plot`] can not be created from [`PlotParameters`]
    #[must_use]
    pub fn new(plt_data: &PlotData, plt_params: &PlotParameters) -> Self {
        let mut plot = Self::try_from(plt_params).unwrap();
        plot.set_data(plt_data.clone());

        plot
    }

    /// Sets the [`PlotData`] of this [`Plot`]
    pub fn set_data(&mut self, data: PlotData) {
        self.data = Some(data);
    }

    /// Returns a reference to the [`PlotData`] of this [`Plot`]
    #[must_use]
    pub const fn get_data(&self) -> Option<&PlotData> {
        self.data.as_ref()
    }

    /// Defines the axes bounds of this [`Plot`].
    ///
    /// # Errors
    /// - if the [`PlotData`] variant is not defined
    /// - if the [`PlotData`] is None
    pub fn define_axes_bounds(&mut self) -> OpmResult<()> {
        if let Some(dat) = &self.data {
            match dat {
                PlotData::ColorMesh(x, y, _) => {
                    if self.bounds.x.is_none() {
                        self.bounds.x = Some(self.define_axis_bounds(
                            &DVectorSlice::from(&x.transpose()),
                            false,
                            false,
                        ));
                    }
                    if self.bounds.y.is_none() {
                        self.bounds.y = Some(self.define_axis_bounds(
                            &DVectorSlice::from(&y.transpose()),
                            false,
                            false,
                        ));
                    }
                    self.bounds.z = None;
                    Ok(())
                }
                _ => Err(OpossumError::Other("Not defined yet!".into())),
            }
        } else {
            Err(OpossumError::Other("No plot data defined!".into()))
        }
    }

    fn define_axis_bounds(
        &self,
        x: &DVectorSlice<'_, f64>,
        expand_min: bool,
        expand_max: bool,
    ) -> (f64, f64) {
        //filter out every infinite value and every NaN
        let x_filtered = MatrixXx1::from(
            x.iter()
                .copied()
                .filter(|x| !x.is_nan() & x.is_finite())
                .collect::<Vec<f64>>(),
        );

        //this only happens if all entries in this matrix are either infinite or NAN
        let (x_range, x_min, x_max) = if x_filtered.is_empty() {
            (1., 0., 1.)
        } else {
            //get the maximum and minimum of the axis
            self.data
                .as_ref()
                .expect("No PlotData available!")
                .get_min_max_range(&DVectorSlice::from(&x_filtered))
            // self.data.unwrap().get_min_max_range(&DVectorSlice::from(&x_filtered));
        };

        //add spacing to the edges if defined
        let add_range_fac = 0.1;
        let expand_min_fac = f64::from(i32::from(expand_min));
        let expand_max_fac = f64::from(i32::from(expand_max));

        let range_start = (x_range * add_range_fac).mul_add(-expand_min_fac, x_min);
        let range_end = (x_range * add_range_fac).mul_add(expand_max_fac, x_max);

        (range_start, range_end)
    }
}

impl TryFrom<&PlotParameters> for Plot {
    type Error = OpossumError;
    fn try_from(p_i_params: &PlotParameters) -> OpmResult<Self> {
        let cmap_gradient = p_i_params.get_cmap()?;
        let cbar_label_str = p_i_params.get_cbar_label()?;
        let cbar_label_pos = p_i_params.get_cbar_label_pos()?;
        let color = p_i_params.get_color()?;
        let fig_size = p_i_params.get_figsize()?;
        let x_lim = p_i_params.get_xlim()?;
        let y_lim = p_i_params.get_ylim()?;
        let z_lim = p_i_params.get_zlim()?;
        let x_label_str = p_i_params.get_x_label()?;
        let y_label_str = p_i_params.get_y_label()?;
        let x_label_pos = p_i_params.get_x_label_pos()?;
        let y_label_pos = p_i_params.get_y_label_pos()?;

        let x_label = LabelDescription::new(&x_label_str, x_label_pos);
        let y_label = LabelDescription::new(&y_label_str, y_label_pos);
        let cbarlabel = LabelDescription::new(&cbar_label_str, cbar_label_pos);

        let cbar = ColorBar {
            cmap: cmap_gradient.get_gradient(),
            label: cbarlabel,
        };

        let plt_info = Self {
            label: [x_label, y_label],
            cbar,
            color,
            data: None,
            bounds: PlotBounds {
                x: x_lim,
                y: y_lim,
                z: z_lim,
            },
            img_size: fig_size,
        };

        Ok(plt_info)
    }
}

///Enum to hold all Arguments that are necessary to describe a plot
#[derive(EnumIter, Debug, Clone)]
pub enum PlotArgs {
    ///Label of the x axis. Holds a String
    XLabel(String),
    ///Label of the y axis. Holds a String
    YLabel(String),
    ///Position of the x label. Holds a [`LabelPos`] enum
    XLabelPos(LabelPos),
    ///Position of the y label. Holds a [`LabelPos`] enum
    YLabelPos(LabelPos),
    ///Color of the Data Points. Holds an [`RGBAColor`] as defined in plotters
    Color(RGBAColor),
    ///Colormap of the Data Points. Holds a [`CGradient`] struct
    CMap(CGradient),
    ///Label of the colorbar. Holds a String
    CBarLabel(String),
    ///Position of the colorbar label. Holds a [`LabelPos`] enum
    CBarLabelPos(LabelPos),
    ///Boundaries of the x axis. If not defined, the inserted plot data will be used to get a reasonable boundary. Holds an [`Option<(f64, f64)>`]
    XLim(Option<(f64, f64)>),
    ///Boundaries of the y axis. If not defined, the inserted plot data will be used to get a reasonable boundary. Holds an [`Option<(f64, f64)>`]
    YLim(Option<(f64, f64)>),
    ///Boundaries of the z axis. If not defined, the inserted plot data will be used to get a reasonable boundary. Holds an [`Option<(f64, f64)>`]
    ZLim(Option<(f64, f64)>),
    ///Figure size in pixels. Holds an `(usize, usize)` tuple
    FigSize((u32, u32)),
    ///Path to the save directory of the image. Only necessary if the data is not written into a buffer. Holds a String
    FDir(String),
    ///Name of the file to be written. Holds a String
    FName(String),
    ///Plotting backend that should be used. Holds a [`PltBackEnd`] enum
    Backend(PltBackEnd),
}
fn _meshgrid(x: &Matrix1xX<f64>, y: &Matrix1xX<f64>) -> (DMatrix<f64>, DMatrix<f64>) {
    let x_len = x.len();
    let y_len = y.len();

    let mut x_mat = DMatrix::<f64>::zeros(y_len, x_len);
    let mut y_mat = DMatrix::<f64>::zeros(y_len, x_len);

    for x_id in 0..x_len {
        for y_id in 0..y_len {
            x_mat[(y_id, x_id)] = x[x_id];
            y_mat[(y_id, x_id)] = y[y_id];
        }
    }

    (x_mat, y_mat)
}

fn linspace(start: f64, end: f64, num: f64) -> OpmResult<Matrix1xX<f64>> {
    let num_usize = num.to_usize();
    if num_usize.is_some() {
        let mut linspace = Matrix1xX::<f64>::from_element(num_usize.unwrap(), start);
        let bin_size = (end - start)
            / (num_usize
                .unwrap()
                .to_f64()
                .expect("Cast from usize to f64 may truncate the value!")
                - 1.);
        for (i, val) in (0_u32..).zip(linspace.iter_mut()) {
            *val += bin_size * f64::from(i);
        }
        Ok(linspace)
    } else {
        Err(OpossumError::Other(
            "Cannot cast num value to usize!".into(),
        ))
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
