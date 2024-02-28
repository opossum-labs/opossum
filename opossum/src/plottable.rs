#![warn(missing_docs)]
//! Trait for adding the possibility to generate a (x/y) plot of an element.
use crate::error::OpmResult;
use crate::error::OpossumError;
use crate::utils::griddata::linspace;
use approx::RelativeEq;
use colorous::Gradient;
use delaunator::{triangulate, Point};
use image::RgbImage;
use itertools::{iproduct, izip};
use kahan::KahanSum;
use log::warn;
use nalgebra::{
    ComplexField, DMatrix, DVector, DVectorSlice, Matrix3xX, MatrixXx1, MatrixXx2, MatrixXx3,
};
use num::ToPrimitive;
use plotters::chart::MeshStyle;
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
use std::path::PathBuf;
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
    ///Line plot for multiple lines, e.g. rays, in two dimensions with pairwise data
    MultiLine2D(PlotParameters),
    ///Line plot for multiple lines, e.g. rays, in three dimensions with 3D data
    MultiLine3D(PlotParameters),
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
            | Self::MultiLine3D(p)
            | Self::MultiLine2D(p)
            | Self::TriangulatedSurface(p) => p,
        }
    }
    fn get_plot_params_mut(&mut self) -> &mut PlotParameters {
        match self {
            Self::ColorMesh(p)
            | Self::Scatter2D(p)
            | Self::Line2D(p)
            | Self::ColorTriangulated(p)
            | Self::MultiLine3D(p)
            | Self::MultiLine2D(p)
            | Self::TriangulatedSurface(p) => p,
        }
    }
    fn create_plot<B: DrawingBackend>(
        &self,
        backend: &DrawingArea<B, Shift>,
        plot: &mut Plot,
    ) -> OpmResult<()> {
        plot.define_axes_bounds()?;

        match self {
            Self::ColorMesh(_) => Self::plot_color_mesh(plot, backend),
            Self::TriangulatedSurface(_) => Self::plot_triangulated_surface(plot, backend),
            Self::ColorTriangulated(_) => Self::plot_color_triangulated(plot, backend),
            Self::Scatter2D(_) => Self::plot_2d_scatter(plot, backend),
            Self::Line2D(_) => Self::plot_2d_line(plot, backend),
            Self::MultiLine3D(_) => Self::plot_3d_multi_line(plot, backend),
            Self::MultiLine2D(_) => Self::plot_2d_multi_line(plot, backend),
        };

        Ok(())
    }

    ///This method sets a plot argument ([`PlotArgs`]) to [`PlotParameters`] which is stored in this [`PlotType`]
    /// # Attributes
    /// - `plt_arg`: plot argument [`PlotArgs`]
    /// # Errors
    /// This method errors if the `set()` function fails
    /// # Returns
    /// This method returns a mutable reference to the changed [`PlotType`]
    pub fn set_plot_param(&mut self, plt_arg: &PlotArgs) -> OpmResult<&mut Self> {
        let plt_params: &mut PlotParameters = self.get_plot_params_mut();
        plt_params.set(plt_arg)?;

        Ok(self)
    }

    /// This method creates a plot
    /// # Attributes
    /// - `plt_data`: plot data. See [`PlotData`]
    /// # Returns
    /// This method returns an [`OpmResult<Option<RgbImage>>`]. It is None if a new file (such as svg, png, bmp or jpg) is created. It is Some(RgbImage) if the image is written to a buffer
    /// # Errors
    /// This method throws an error if
    /// - some plot parameters contradict each other
    /// - the file path can not be extracted
    /// - the plotting backend can not be extracted
    /// - the plot can not be created inside the `create_plot()` method
    /// - the image buffer is too small
    pub fn plot(&self, plt_data: &PlotData) -> OpmResult<Option<RgbImage>> {
        let params = self.get_plot_params();
        params.check_backend_file_ext_compatibility()?;
        let path = params.get_fpath()?;
        let mut plot = Plot::new(plt_data, params);

        match params.get_backend()? {
            PltBackEnd::BMP => {
                let backend = BitMapBackend::new(&path, plot.img_size).into_drawing_area();
                let _ = self.create_plot(&backend, &mut plot);
                Ok(None)
            }
            PltBackEnd::SVG => {
                let backend = SVGBackend::new(&path, plot.img_size).into_drawing_area();
                let _ = self.create_plot(&backend, &mut plot);
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
                    let _ = self.create_plot(&backend, &mut plot);
                }
                let img = RgbImage::from_raw(plot.img_size.0, plot.img_size.1, image_buffer)
                    .ok_or_else(|| OpossumError::Other("image buffer size too small".into()))?;
                Ok(Some(img))
            }
        }
    }

    fn draw_line_2d<T: DrawingBackend>(
        chart: &mut ChartContext<'_, T, Cartesian2d<RangedCoordf64, RangedCoordf64>>,
        x: &DVectorSlice<'_, f64>,
        y: &DVectorSlice<'_, f64>,
        line_color: &RGBAColor,
    ) {
        chart
            .draw_series(LineSeries::new(
                izip!(x, y).map(|xy| (*xy.0, *xy.1)),
                line_color,
            ))
            .unwrap();
    }

    fn draw_line_3d<T: DrawingBackend>(
        chart: &mut ChartContext<
            '_,
            T,
            Cartesian3d<RangedCoordf64, RangedCoordf64, RangedCoordf64>,
        >,
        x: &DVectorSlice<'_, f64>,
        y: &DVectorSlice<'_, f64>,
        z: &DVectorSlice<'_, f64>,
        line_color: &RGBAColor,
    ) {
        chart
            .draw_series(LineSeries::new(
                izip!(x, y, z).map(|xyz| (*xyz.0, *xyz.1, *xyz.2)),
                line_color,
            ))
            .unwrap();
    }

    fn draw_points<T: DrawingBackend>(
        chart: &mut ChartContext<'_, T, Cartesian2d<RangedCoordf64, RangedCoordf64>>,
        x: &DVectorSlice<'_, f64>,
        y: &DVectorSlice<'_, f64>,
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
        x: &DVectorSlice<'_, f64>,
        y: &DVectorSlice<'_, f64>,
        c: &DVectorSlice<'_, f64>,
        cmap: &Gradient,
        cbounds: AxLims,
    ) {
        let z_min = cbounds.min;
        let z_max: f64 = cbounds.max - z_min; //z.max();

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
        x: &DVectorSlice<'_, f64>,
        y: &DVectorSlice<'_, f64>,
        z: &DVectorSlice<'_, f64>,
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

    fn check_equistancy_of_mesh(ax_vals: &MatrixXx1<f64>) -> bool {
        let len_ax = ax_vals.len();
        let mut equi = true;
        if len_ax > 2 {
            let mut distance = KahanSum::new_with_value(ax_vals[1]);
            distance += -ax_vals[0];
            for idx in 2..len_ax {
                let mut diff = KahanSum::new_with_value(ax_vals[idx]);
                diff += -ax_vals[idx - 1];
                diff += -distance.sum();
                if (diff.sum() / distance.sum()).abs() > 100. * f64::EPSILON {
                    equi = false;
                    break;
                }
            }
        }
        equi
    }

    fn get_ax_val_distance_if_equidistant(ax_vals: &MatrixXx1<f64>) -> f64 {
        let mut dist = (ax_vals[1] - ax_vals[0]) / 2.;
        if Self::check_equistancy_of_mesh(ax_vals) {
            if dist <= 2. * f64::EPSILON {
                dist = 0.5;
            }
        } else {
            warn!(
                "Warning! The points on this axis are not equistant!\n This may distort the plot!"
            );
        };
        dist
    }

    fn draw_2d_colormesh<T: DrawingBackend>(
        chart: &mut ChartContext<'_, T, Cartesian2d<RangedCoordf64, RangedCoordf64>>,
        x_ax: &MatrixXx1<f64>,
        y_ax: &MatrixXx1<f64>,
        z_dat: &DMatrix<f64>,
        cmap: &Gradient,
        cbounds: AxLims,
    ) {
        let x_dist = Self::get_ax_val_distance_if_equidistant(x_ax);
        let y_dist = Self::get_ax_val_distance_if_equidistant(y_ax);

        let (z_shape_rows, z_shape_cols) = z_dat.shape();
        if z_shape_rows != y_ax.len() || z_shape_cols != x_ax.len() {
            warn!("Shapes of x,y and z do not match!");
            return;
        }

        //there will probably be a more direct way to achieve the series without this conversion to a vec<f64> when we can use nalgebra >=v0.32.
        //currently, clone is not implemented for matrix_iter in v0.30 which we use due to ncollide2d. Therefore, we go this way
        let a: Vec<f64> = x_ax.data.clone().into();
        let b: Vec<f64> = y_ax.data.clone().into();
        let z_min = cbounds.min;

        let z_max: f64 = cbounds.max - z_min; //z.max();
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
            let mut chart = Self::create_2d_plot_chart(
                root,
                plt.bounds.x.unwrap(),
                plt.bounds.y.unwrap(),
                &plt.label,
                true,
                true,
            );
            Self::draw_line_2d(&mut chart, &dat.column(0), &dat.column(1), &plt.color);
        }

        root.present().unwrap();
    }

    fn plot_2d_scatter<B: DrawingBackend>(plt: &Plot, root: &DrawingArea<B, Shift>) {
        if let Some(PlotData::Dim2(dat)) = plt.get_data() {
            _ = root.fill(&WHITE);

            let mut chart = Self::create_2d_plot_chart(
                root,
                plt.bounds.x.unwrap(),
                plt.bounds.y.unwrap(),
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

            //colorbar. first because otherwise the xlabel of the main plot is cropped
            let mut chart = Self::create_2d_plot_chart(
                &cbar_root,
                AxLims { min: 0., max: 1. },
                plt.bounds.z.unwrap(),
                &[
                    LabelDescription::new("", plt.label[0].label_pos),
                    plt.cbar.label.clone(),
                ],
                true,
                false,
            );

            let c_dat =
                linspace(plt.bounds.z.unwrap().min, plt.bounds.z.unwrap().max, 100.).unwrap();
            let d_mat = DMatrix::<f64>::from_columns(&[c_dat.clone(), c_dat]);
            let xxx = DVector::<f64>::from_vec(vec![0., 1.]);
            Self::draw_2d_colormesh(
                &mut chart,
                &xxx,
                &linspace(plt.bounds.z.unwrap().min, plt.bounds.z.unwrap().max, 100.).unwrap(),
                &d_mat,
                &plt.cbar.cmap,
                plt.bounds.z.unwrap(),
            );

            //main plot
            let mut chart = Self::create_2d_plot_chart(
                &main_root,
                plt.bounds.x.unwrap(),
                plt.bounds.y.unwrap(),
                &plt.label,
                true,
                true,
            );
            Self::draw_color_triangles(
                &mut chart,
                triangle_index,
                &dat.column(0),
                &dat.column(1),
                &color.column(0),
                &plt.cbar.cmap,
                plt.bounds.z.unwrap(),
            );
        }
    }

    fn plot_2d_multi_line<B: DrawingBackend>(plt: &Plot, root: &DrawingArea<B, Shift>) {
        if let Some(PlotData::MultiDim2(dat)) = plt.get_data() {
            _ = root.fill(&WHITE);
            //main plot
            //currently there is no support for axes labels in 3d plots
            let mut chart = Self::create_2d_plot_chart(
                root,
                plt.bounds.x.unwrap(),
                plt.bounds.y.unwrap(),
                &plt.label,
                true,
                true,
            );

            for line_dat in dat {
                Self::draw_line_2d(
                    &mut chart,
                    &line_dat.column(0),
                    &line_dat.column(1),
                    &RGBAColor(255, 0, 0, 0.3),
                );
            }
        }
    }

    fn plot_3d_multi_line<B: DrawingBackend>(plt: &Plot, root: &DrawingArea<B, Shift>) {
        if let Some(PlotData::MultiDim3(dat)) = plt.get_data() {
            _ = root.fill(&WHITE);
            //main plot
            //currently there is no support for axes labels in 3d plots
            let mut chart = Self::create_3d_plot_chart(root, plt);

            for line_dat in dat {
                Self::draw_line_3d(
                    &mut chart,
                    &line_dat.column(0),
                    &line_dat.column(1),
                    &line_dat.column(2),
                    &RGBAColor(255, 0, 0, 0.3),
                );
            }
        }
    }

    fn plot_triangulated_surface<B: DrawingBackend>(plt: &Plot, root: &DrawingArea<B, Shift>) {
        if let Some(PlotData::TriangulatedSurface(triangle_index, dat)) = plt.get_data() {
            _ = root.fill(&WHITE);

            //main plot
            //currently there is no support for axes labels in 3d plots
            let mut chart = Self::create_3d_plot_chart(root, plt);

            Self::draw_triangle_surf(
                &mut chart,
                triangle_index,
                &dat.column(0),
                &dat.column(2),
                &dat.column(1),
            );
        }
    }
    fn plot_color_mesh<B: DrawingBackend>(plt: &Plot, root: &DrawingArea<B, Shift>) {
        if let Some(PlotData::ColorMesh(x, y, dat)) = plt.get_data() {
            _ = root.fill(&WHITE);
            //split root for main plot and colorbar
            let (main_root, cbar_root) = root.split_horizontally(830);

            //colorbar. first because otherwise the xlabel of the main plot is cropped
            let mut chart = Self::create_2d_plot_chart(
                &cbar_root,
                AxLims { min: 0., max: 1. },
                plt.bounds.z.unwrap(),
                &[
                    LabelDescription::new("", plt.label[0].label_pos),
                    plt.cbar.label.clone(),
                ],
                true,
                false,
            );

            let c_dat =
                linspace(plt.bounds.z.unwrap().min, plt.bounds.z.unwrap().max, 100.).unwrap();
            let d_mat = DMatrix::<f64>::from_columns(&[c_dat.clone(), c_dat]);
            let xxx = DVector::<f64>::from_vec(vec![0., 1.]);
            Self::draw_2d_colormesh(
                &mut chart,
                &xxx,
                &linspace(plt.bounds.z.unwrap().min, plt.bounds.z.unwrap().max, 100.).unwrap(),
                &d_mat,
                &plt.cbar.cmap,
                plt.bounds.z.unwrap(),
            );

            //main plot
            let mut chart = Self::create_2d_plot_chart(
                &main_root,
                plt.bounds.x.unwrap(),
                plt.bounds.y.unwrap(),
                &plt.label,
                true,
                true,
            );

            Self::draw_2d_colormesh(&mut chart, x, y, dat, &plt.cbar.cmap, plt.bounds.z.unwrap());
        }

        root.present().unwrap();
    }

    fn create_3d_plot_chart<'a, T: DrawingBackend>(
        root: &'a DrawingArea<T, Shift>,
        plot: &Plot,
    ) -> ChartContext<'a, T, Cartesian3d<RangedCoordf64, RangedCoordf64, RangedCoordf64>> {
        root.fill(&WHITE).unwrap();

        //plotters axes are defined with z going upwards. therefore, I change this
        let x_bounds = plot.bounds.x.unwrap();
        let y_bounds = plot.bounds.y.unwrap();
        let z_bounds = plot.bounds.z.unwrap();

        let mut chart = ChartBuilder::on(root)
            .margin(20)
            .set_all_label_area_size(100)
            .build_cartesian_3d(
                x_bounds.min..x_bounds.max,
                y_bounds.min..y_bounds.max,
                z_bounds.min..z_bounds.max,
            )
            .unwrap();

        chart.with_projection(
            |mut pb: plotters::coord::ranged3d::ProjectionMatrixBuilder| {
                pb.pitch = 0. / 180. * PI;
                pb.yaw = -90. / 180. * PI;
                pb.scale = 0.7;
                pb.into_matrix()
            },
        );

        chart.configure_axes().draw().unwrap();

        chart
    }

    fn create_2d_plot_chart<'a, T: DrawingBackend>(
        root: &'a DrawingArea<T, Shift>,
        x_bounds: AxLims,
        y_bounds: AxLims,
        label_desc: &[LabelDescription; 2],
        y_ax: bool,
        x_ax: bool,
    ) -> ChartContext<'a, T, Cartesian2d<RangedCoordf64, RangedCoordf64>> {
        root.fill(&WHITE).unwrap();

        let mut chart_builder = ChartBuilder::on(root);
        chart_builder.margin(10).margin_top(40);

        if y_ax {
            let pixel_margin = Self::calc_pixel_margin(y_bounds);

            chart_builder.set_label_area_size(label_desc[1].label_pos.into(), 21 + pixel_margin);

            if LabelPos::Right == label_desc[1].label_pos && (pixel_margin < 72) {
                chart_builder.margin_right(82 - pixel_margin);
            } else if LabelPos::Left == label_desc[1].label_pos && (pixel_margin < 72) {
                chart_builder.margin_left(82 - pixel_margin);
            }
        }
        chart_builder.set_label_area_size(label_desc[0].label_pos.into(), 65);

        let mut chart = chart_builder
            .build_cartesian_2d(x_bounds.min..x_bounds.max, y_bounds.min..y_bounds.max)
            .unwrap();

        let mut mesh: MeshStyle<'_, '_, RangedCoordf64, RangedCoordf64, T> = chart.configure_mesh();

        Self::set_or_disable_axis_desc([x_ax, y_ax], label_desc, &mut mesh);

        mesh.label_style(("sans-serif", 30).into_font())
            .draw()
            .unwrap();

        chart
    }

    fn set_or_disable_axis_desc<T: DrawingBackend>(
        ax: [bool; 2],
        label_desc: &[LabelDescription; 2],
        mesh: &mut MeshStyle<'_, '_, RangedCoordf64, RangedCoordf64, T>,
    ) {
        if ax[1] {
            mesh.y_desc(&label_desc[1].label);
        } else {
            mesh.disable_y_axis();
        }

        if ax[0] {
            mesh.x_desc(&label_desc[0].label);
        } else {
            mesh.disable_x_axis();
        }
    }

    fn calc_pixel_margin(bounds: AxLims) -> i32 {
        //absolutely ugly "automation" of margin. not done nicely and not accurate, works only for sans serif with 30 pt
        let mut digits_max =
            bounds.max.abs().log10().abs().floor() + 2. + f64::from(bounds.max.is_sign_negative());
        if digits_max.is_infinite() {
            digits_max = 4.;
        }
        let mut digits_min =
            bounds.min.abs().log10().abs().floor() + 2. + f64::from(bounds.min.is_sign_negative());
        if digits_min.is_infinite() {
            digits_min = 4.;
        }
        let digits = if digits_max >= digits_min {
            digits_max.to_i32()
        } else {
            digits_min.to_i32()
        }
        .unwrap();

        digits * 13 + 20
    }
}

#[derive(Debug, Clone)]
///Enum to define the type of plot that should be created
pub enum PlotData {
    ///Pairwise 2D data (e.g. x, y data) for scatter2D, Line2D. Data Structure as Matrix with N rows and two columns (x,y)
    Dim2(MatrixXx2<f64>),
    ///Triplet 3D data (e.g. x, y, z data) for scatter3D, Line3D or colorscatter. Data Structure as Matrix with N rows and three columns (x,y,z)
    Dim3(MatrixXx3<f64>),
    ///Vector of pairwise 2D data (e.g. x, y data) for MultiLine2D. Data Structure as Vector filled with Matrices with N rows and two columns (x,y)
    MultiDim2(Vec<MatrixXx2<f64>>),
    ///Vector of triplet 3D data (e.g. x, y, z data) for MultiLine3D. Data Structure as Vector filled with Matrices with N rows and three columns (x,y,z)
    MultiDim3(Vec<MatrixXx3<f64>>),
    /// Data to create a 2d colormesh plot. Vector with N entries for x, Vector with M entries for y and a Matrix with NxM entries for the colordata
    ColorMesh(DVector<f64>, DVector<f64>, DMatrix<f64>), // ColorScatter(DVector<f64>, DVector<f64>, DMatrix<f64>)
    /// Data to create a 2d triangulated color plot.
    /// - Matrix with 3 columns and N rows that is filled with the indices that correspond to the data points that ave been triangulated
    /// - Vector with N rows which holds the average color value of the triangle
    /// - Matrix with 3 columns and N rows that hold the x,y,z data
    ColorTriangulated(MatrixXx3<usize>, DVector<f64>, MatrixXx3<f64>),
    /// Data to create a 3d triangulated surface plot.
    /// - Matrix with 3 columns and N rows that is filled with the indices that correspond to the data points that ave been triangulated
    /// - Matrix with 3 columns and N rows that hold the x,y,z data
    TriangulatedSurface(MatrixXx3<usize>, MatrixXx3<f64>),
}

impl PlotData {
    /// This method gets the actual maximum and minimum data values of an axis
    /// # Attributes
    /// - `ax_vals`: dynamically sized vector slice of the data vector on this axis
    /// # Returns
    /// This method returns the maximum and minimum data values on this axis in form of an [`AxLims`] struct
    #[must_use]
    pub fn get_min_max_data_values(&self, ax_vals: &DVectorSlice<'_, f64>) -> AxLims {
        let filtered_ax_vals = Self::filter_nan_infinite(ax_vals);
        AxLims {
            min: filtered_ax_vals.min(),
            max: filtered_ax_vals.max(),
        }
    }

    /// Gets the minimum and maximum values of all axes
    /// # Returns
    /// This method returns a vector of axes limits [`Vec<AxLims>`]
    #[must_use]
    pub fn get_axes_min_max_ranges(&self) -> Vec<AxLims> {
        match self {
            Self::Dim2(dat) => vec![
                self.get_min_max_data_values(&dat.column(0)),
                self.get_min_max_data_values(&dat.column(1)),
            ],
            Self::Dim3(dat)
            | Self::ColorTriangulated(_, _, dat)
            | Self::TriangulatedSurface(_, dat) => vec![
                self.get_min_max_data_values(&dat.column(0)),
                self.get_min_max_data_values(&dat.column(1)),
                self.get_min_max_data_values(&dat.column(2)),
            ],
            Self::ColorMesh(x, y, z) => {
                let z_flat = DVector::from_vec(z.into_iter().copied().collect::<Vec<f64>>());
                vec![
                    self.get_min_max_data_values(&DVectorSlice::from(x)),
                    self.get_min_max_data_values(&DVectorSlice::from(y)),
                    self.get_min_max_data_values(&z_flat.column(0)),
                ]
            }
            Self::MultiDim3(dat) => {
                let num_cols = dat[0].row(0).len();
                let mut min_max = MatrixXx3::zeros(dat.len() * 2);
                for (row, d) in dat.iter().enumerate() {
                    for col in 0..num_cols {
                        let axlim = self.get_min_max_data_values(&d.column(col));
                        min_max[(2 * row, col)] = axlim.min;
                        min_max[(2 * row + 1, col)] = axlim.max;
                    }
                }

                let mut ax_lim_vec = Vec::<AxLims>::new();
                for col in 0..num_cols {
                    ax_lim_vec.push(self.get_min_max_data_values(&min_max.column(col)));
                }
                ax_lim_vec
            }

            Self::MultiDim2(dat) => {
                let num_cols = dat[0].row(0).len();
                let mut min_max = MatrixXx2::zeros(dat.len() * 2);
                for (row, d) in dat.iter().enumerate() {
                    for col in 0..num_cols {
                        let axlim = self.get_min_max_data_values(&d.column(col));
                        min_max[(2 * row, col)] = axlim.min;
                        min_max[(2 * row + 1, col)] = axlim.max;
                    }
                }

                let mut ax_lim_vec = Vec::<AxLims>::new();
                for col in 0..num_cols {
                    ax_lim_vec.push(self.get_min_max_data_values(&min_max.column(col)));
                }
                ax_lim_vec
            }
        }
    }

    /// This method filters out all NaN and infinite values  
    /// # Attributes
    /// - `ax_vals`: dynamically sized vector slice of the data vector on this axis
    /// # Returns
    /// This method returns an array containing only the non-NaN and finite entries of the passed vector
    #[must_use]
    pub fn filter_nan_infinite(ax_vals: &DVectorSlice<'_, f64>) -> MatrixXx1<f64> {
        MatrixXx1::from(
            ax_vals
                .iter()
                .copied()
                .filter(|x| !x.is_nan() & x.is_finite())
                .collect::<Vec<f64>>(),
        )
    }

    /// Defines the plot-axes bounds of this [`PlotData`].
    /// # Attributes
    /// - `expand_flag`: true if the ax bounds should expand by +- 10%, such that the data is not on the edge of the plot. false for no expansion
    /// # Returns
    /// This function returns a Vector of optional [`AxLims`]
    /// # Panics
    /// This function panics if the `expand_lims` function fails. As this only happens for a non-normal number this cannnot happen here.
    #[must_use]
    fn define_data_based_axes_bounds(&self, expand_flag: bool) -> Vec<AxLims> {
        let mut ax_lims = self.get_axes_min_max_ranges();

        //check if the limits are useful for visualization
        for axlim in &mut ax_lims {
            axlim.make_plot_axlims_useful();
        }

        if expand_flag {
            for ax_lim in &mut ax_lims {
                ax_lim.expand_lims(0.1).unwrap();
            }
        };
        ax_lims
    }
}

/// Struct that holds the maximum and minimum values of an axis
#[derive(Clone, Debug, Copy, PartialEq)]
pub struct AxLims {
    /// minimum value of the axis
    pub min: f64,
    /// maximum value of the axis
    pub max: f64,
}

impl AxLims {
    ///Creates a new [`AxLims`] struct
    /// # Attributes
    /// -`min`: minimum value of the ax limit
    /// -`max`: maximum value of the ax limit
    ///
    /// # Errors
    /// This function errors if the chosen minimum or maximum valus is NaN or infinite
    pub fn new(min: f64, max: f64) -> OpmResult<Self> {
        let axlim = Self { min, max };
        if axlim.check_validity() {
            Ok(axlim)
        } else {
            Err(OpossumError::Other(
                "Invalid ax limit! Must be finite, not NaN, not equal and min must be smaller than max!".into(),
            ))
        }
    }

    fn check_validity(self) -> bool {
        self.max.is_finite()
            && !self.max.is_nan()
            && self.min.is_finite()
            && !self.min.is_nan()
            && (self.max - self.min).abs() > f64::EPSILON
            && self.max > self.min
    }

    /// Shifts the minimum and the maximum to lower and higher values respectively.
    /// The extend of the shift is expressed as a relative ratio of the full range
    /// # Attributes
    /// -`ratio`: relative extension of the range. must be positive, non-zero, not NAN and finite
    /// # Errors
    /// This function errors if the expansion ration is neither positive nor normal
    pub fn expand_lims(&mut self, expansion_ratio: f64) -> OpmResult<()> {
        if expansion_ratio.is_normal() && expansion_ratio.is_sign_positive() {
            let range = self.max - self.min;
            self.max += range * expansion_ratio;
            self.min -= range * expansion_ratio;
            Ok(())
        } else {
            Err(OpossumError::Other(
                "Expansion ratio must be normal and positive!".into(),
            ))
        }
    }

    /// This function checks if the prrovided axis limits are useful in terms of visualization.
    /// If min and max are approximately equal, the range ist set to the maximum value and the minimum value is set to 0
    /// if the maximum is zero AND approximately equal to the minimum, then it is set to 1 and the minimum to zero to avoid awkward ax scalings
    /// # Attributes
    /// - `axlims`: provided axes limits [`AxLims`]
    /// #  Returns
    /// This function returns an [`AxLims`]
    pub fn make_plot_axlims_useful(&mut self) {
        let mut ax_range = self.max - self.min;

        //check if minimum and maximum values are approximately equal. if so, take the max value as range
        if self.max.relative_eq(&self.min, f64::EPSILON, f64::EPSILON) {
            ax_range = self.max;
            self.min = 0.;
        };

        //check if for some reason maximum is 0, then set it to 1, so that the axis spans at least some distance
        if ax_range < f64::EPSILON {
            self.max = 1.;
            self.min = 0.;
        };
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

    /// This method handles the plot creation for a specific data type or node type
    /// # Attributes
    /// - `f_path`: path to the file
    /// - `img_size`: the size of the image in pixels: (width, height)
    /// - `backend`: used backend to create the plot. See [`PltBackEnd`]
    /// # Errors
    /// Whether an error is thrown depends on the individual implementation of the method
    fn to_plot(
        &self,
        f_path: &Path,
        img_size: (u32, u32),
        backend: PltBackEnd,
    ) -> OpmResult<Option<RgbImage>> {
        let mut plt_params = PlotParameters::default();
        match backend {
            PltBackEnd::Buf => plt_params.set(&PlotArgs::FigSize(img_size))?,
            _ => plt_params
                .set(&PlotArgs::FName(
                    f_path.file_name().unwrap().to_str().unwrap().to_owned(),
                ))?
                .set(&PlotArgs::FDir(f_path.parent().unwrap().into()))?
                .set(&PlotArgs::FigSize(img_size))?,
        };
        plt_params.set(&PlotArgs::Backend(backend))?;

        let _ = self.add_plot_specific_params(&mut plt_params);

        let plt_type = self.get_plot_type(&plt_params);

        let plt_data_opt = self.get_plot_data(&plt_type)?;

        plt_data_opt.map_or(Ok(None), |plt_dat| plt_type.plot(&plt_dat))
    }

    /// This method must be implemented in order to create a plot.
    /// As the plot data may differ, the implementation must be done for each kind of plot
    /// # Returns
    /// This method returns the [`PlotParameters`] of this [`Plot`]
    /// # Errors
    /// This method errors if setting a plot parameter fails
    fn add_plot_specific_params(&self, plt_params: &mut PlotParameters) -> OpmResult<()>;

    /// This method must be implemented in order to create a plot.
    /// As the plot type may differ, the implementation must be done for each kind of plot
    /// # Returns
    /// This method returns the [`PlotType`] of this [`Plot`]
    fn get_plot_type(&self, plt_params: &PlotParameters) -> PlotType;

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
            let ax_lims = plt_dat.get_axes_min_max_ranges();
            let mut x_ax_lims = ax_lims[0];
            let mut y_ax_lims = ax_lims[1];

            let num_entries = dat.column(0).len();
            let mut num = f64::sqrt((num_entries / 2).to_f64().unwrap()).floor();

            if (num % 2.).relative_eq(&0., f64::EPSILON, f64::EPSILON) {
                num += 1.;
            }

            let xbin = (x_ax_lims.max - x_ax_lims.min) / (num - 1.0);
            let ybin = (y_ax_lims.max - y_ax_lims.min) / (num - 1.0);

            let x = linspace(x_ax_lims.min - xbin / 2., x_ax_lims.max + xbin / 2., num).unwrap();
            let y = linspace(y_ax_lims.min - ybin / 2., y_ax_lims.max + ybin / 2., num).unwrap();

            let xbin = x[1] - x[0];
            let ybin = y[1] - y[0];
            x_ax_lims.min = x.min();
            y_ax_lims.min = y.min();

            let mut zz = DMatrix::<f64>::zeros(x.len(), y.len());
            // xx.clone() * 0.;
            let mut zz_counter = DMatrix::<f64>::zeros(x.len(), y.len()); //xx.clone() * 0.;

            for row in dat.row_iter() {
                let x_index = ((row[(0, 0)] - x_ax_lims.min + xbin / 2.) / xbin).to_usize();
                let y_index = ((row[(0, 1)] - y_ax_lims.min + ybin / 2.) / ybin).to_usize();
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

            Some(PlotData::ColorMesh(x, y, zz))
        } else {
            None
        }
    }

    /// This method bins or triangulates a set of [`PlotData`]
    /// # Attributes
    /// - `plt_type`: [`PlotType`] of the plot
    /// - `plt_data`: [`PlotData`] of the plot
    ///
    /// # Returns
    /// This method returns a Some([`PlotData`]) for the `ColorMesh`, `ColorTriangulated` and `TriangulatedSurface` [`PlotType`] variants. It Returns None for all other variants
    fn bin_or_triangulate_data(
        &self,
        plt_type: &PlotType,
        plt_data: &PlotData,
    ) -> Option<PlotData> {
        match plt_type {
            PlotType::ColorMesh(_) => self.bin_2d_scatter_data(plt_data),
            PlotType::TriangulatedSurface(_) | PlotType::ColorTriangulated(_) => {
                self.triangulate_plot_data(plt_data, plt_type)
            }
            _ => None,
        }
    }
}

///Enum to describe which type of plotting backend should be used
#[derive(Clone, Debug, Default, PartialEq, Eq)]
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
        self.label.set_label_pos(pos);
    }

    /// Sets the label of this [`ColorBar`].
    /// # Attributes
    /// - `txt`: text to be shown as label
    pub fn set_label(&mut self, txt: &str) {
        self.label.set_label(txt);
    }
}

impl Default for ColorBar {
    #[must_use]
    fn default() -> Self {
        Self {
            cmap: colorous::TURBO,
            label: LabelDescription::new("", LabelPos::Right),
        }
    }
}

/// Struct to hold the plot boundaries of the plot in the x, y, z axes.
/// The values may also be None. Then, reasonable boundaries are chosen automatically
#[derive(Clone)]
pub struct PlotBounds {
    x: Option<AxLims>,
    y: Option<AxLims>,
    z: Option<AxLims>,
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
        let mut plt_params = Self {
            params: HashMap::new(),
        };

        //iterate over all enum variants and provide a default value for the argument of that variant
        for plt_arg in PlotArgs::iter() {
            match plt_arg {
                PlotArgs::Backend(_) => {
                    plt_params.set(&PlotArgs::Backend(PltBackEnd::BMP)).unwrap()
                }
                PlotArgs::XLabel(_) => plt_params.set(&PlotArgs::XLabel("x".into())).unwrap(),
                PlotArgs::XLabelPos(_) => plt_params
                    .set(&PlotArgs::XLabelPos(LabelPos::Bottom))
                    .unwrap(),
                PlotArgs::YLabel(_) => plt_params.set(&PlotArgs::YLabel("y".into())).unwrap(),
                PlotArgs::YLabelPos(_) => plt_params
                    .set(&PlotArgs::YLabelPos(LabelPos::Left))
                    .unwrap(),
                PlotArgs::CBarLabel(_) => plt_params
                    .set(&PlotArgs::CBarLabel("z value".into()))
                    .unwrap(),
                PlotArgs::CBarLabelPos(_) => plt_params
                    .set(&PlotArgs::CBarLabelPos(LabelPos::Right))
                    .unwrap(),
                PlotArgs::XLim(_) => plt_params.set(&PlotArgs::XLim(None)).unwrap(),
                PlotArgs::YLim(_) => plt_params.set(&PlotArgs::YLim(None)).unwrap(),
                PlotArgs::ZLim(_) => plt_params.set(&PlotArgs::ZLim(None)).unwrap(),
                PlotArgs::CMap(_) => plt_params
                    .set(&PlotArgs::CMap(CGradient::default()))
                    .unwrap(),
                PlotArgs::Color(_) => plt_params
                    .set(&PlotArgs::Color(RGBAColor(255, 0, 0, 1.)))
                    .unwrap(),
                PlotArgs::FName(_) => {
                    let (_, fname) = Self::create_unused_filename_dir();

                    plt_params.set(&PlotArgs::FName(fname)).unwrap()
                }
                PlotArgs::FigSize(_) => plt_params.set(&PlotArgs::FigSize((1000, 850))).unwrap(),
                PlotArgs::FDir(_) => {
                    let (current_dir, _) = Self::create_unused_filename_dir();

                    plt_params.set(&PlotArgs::FDir(current_dir)).unwrap()
                }
            };
        }

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

    ///This method creates a default directory and default filename that is unused
    /// # Returns
    /// This method returns a (String, String) tuple with the first String being the directory and the second String being the filename
    fn create_unused_filename_dir() -> (PathBuf, String) {
        let cur_dir = current_dir().unwrap().to_str().unwrap().to_owned() + "\\";
        let mut i = 0;
        loop {
            let fpath = cur_dir.clone() + format!("opossum_default_plot_{i}.png").as_str();
            let path = Path::new(&fpath);
            if !path.exists() {
                break;
            }
            i += 1;
        }
        (
            current_dir().unwrap(),
            format!("opossum_default_plot_{i}.png"),
        )
    }

    ///This method creates a new [`PlotParameters`] struct and inserts the passed [`PlotArgs`]. The other [`PlotArgs`] are set to default
    /// # Attributes
    /// - `plt_args`: Vector of Plot Arguments
    /// # Returns
    /// This method returns a new [`PlotParameters`] struct
    #[must_use]
    pub fn new(plt_args: Vec<PlotArgs>) -> Self {
        let mut plt_params = Self::default();

        for plt_arg in plt_args {
            plt_params.insert(&plt_arg);
        }
        plt_params
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
    /// # Panics
    /// This method panics if the path cannot be casted to a str
    pub fn get_fpath(&self) -> OpmResult<String> {
        let fdir = self.get_fdir()?;
        let fname = self.get_fname()?;

        Ok(fdir.join(fname).to_str().unwrap().to_owned())
    }

    ///This method gets the file directory which is stored in the [`PlotParameters`]
    /// # Returns
    /// This method returns an [`OpmResult<String>`] containing the file directory
    /// # Errors
    /// This method throws an error if the argument is not found
    pub fn get_fdir(&self) -> OpmResult<PathBuf> {
        if let Some(PlotArgs::FDir(fdir)) = self.params.get("fdir") {
            Ok(fdir.clone())
        } else {
            Err(OpossumError::Other("fdir argument not found!".into()))
        }
    }

    ///This method gets the x limit which is stored in the [`PlotParameters`]
    /// # Returns
    /// This method returns an [`OpmResult<Option<AxLims>>`] with the min and max of the x values as f64
    /// # Errors
    /// This method throws an error if the argument is not found
    pub fn get_xlim(&self) -> OpmResult<Option<AxLims>> {
        if let Some(PlotArgs::XLim(xlim)) = self.params.get("xlim") {
            Ok(*xlim)
        } else {
            Err(OpossumError::Other("xlim argument not found!".into()))
        }
    }

    ///This method gets the y limit which is stored in the [`PlotParameters`]
    /// # Returns
    /// This method returns an [`OpmResult<Option<AxLims>>`] with the min and max of the y values as f64
    /// # Errors
    /// This method throws an error if the argument is not found
    pub fn get_ylim(&self) -> OpmResult<Option<AxLims>> {
        if let Some(PlotArgs::YLim(ylim)) = self.params.get("ylim") {
            Ok(*ylim)
        } else {
            Err(OpossumError::Other("ylim argument not found!".into()))
        }
    }

    ///This method gets the z limit which is stored in the [`PlotParameters`]
    /// # Returns
    /// This method returns an [`OpmResult<Option<AxLims>>`] with the min and max of the z values as f64
    /// # Errors
    /// This method throws an error if the argument is not found
    pub fn get_zlim(&self) -> OpmResult<Option<AxLims>> {
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

    fn check_ax_lim_validity(ax_lim_opt: &Option<AxLims>) -> bool {
        ax_lim_opt.as_ref().map_or(true, |lim| lim.check_validity())
    }

    fn check_plot_arg_validity(plt_arg: &PlotArgs) -> bool {
        match plt_arg {
            PlotArgs::XLabelPos(xlabel_pos) => {
                matches!(xlabel_pos, LabelPos::Bottom | LabelPos::Top)
            }
            PlotArgs::YLabelPos(label_pos) | PlotArgs::CBarLabelPos(label_pos) => {
                matches!(label_pos, LabelPos::Left | LabelPos::Right)
            }
            PlotArgs::XLim(lim_opt) | PlotArgs::YLim(lim_opt) | PlotArgs::ZLim(lim_opt) => {
                Self::check_ax_lim_validity(lim_opt)
            }
            PlotArgs::FigSize(figsize) => !(figsize.0 == 0 || figsize.1 == 0),
            PlotArgs::FDir(fdir) => Path::new(fdir).exists(),
            PlotArgs::FName(fname) => {
                Self::check_file_ext_validity(fname, vec!["jpg", "png", "bmp", "svg"])
            }
            // labels, color and gradient are irrelevant to check.
            //cross check of backend and full file path is done later, as a change would otherwise always result in an error.
            _ => true,
        }
    }

    fn check_file_ext_validity(fname: &str, valid_exts: Vec<&str>) -> bool {
        let mut valid = false;
        for valid_ext in valid_exts {
            if std::path::Path::new(fname)
                .extension()
                .map_or(false, |ext| ext.eq_ignore_ascii_case(valid_ext))
            {
                valid = true;
                break;
            }
        }
        valid
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
    /// # Errors
    /// This function errors if the plot argument is not valid
    /// # Returns
    /// This method returns a mutable reference to the changed [`PlotParameters`]
    pub fn set(&mut self, plt_arg: &PlotArgs) -> OpmResult<&mut Self> {
        let key = Self::get_plt_arg_key(plt_arg);
        if Self::check_plot_arg_validity(plt_arg) {
            if self.check_if_set(plt_arg) {
                self.params.remove_entry(&key);
            }
            self.insert(plt_arg);
            Ok(self)
        } else {
            Err(OpossumError::Other(format!(
                "Parameter of plot argument \"{plt_arg:?}\" is invalid and could not be set!"
            )))
        }
    }

    /// This method checks if compatibility between the chosen [`PltBackEnd`] and the file extension
    /// # Attributes
    /// - `path_fname`: name of the file
    /// - `backend`: backend to plot with. See [`PltBackEnd`]
    /// # Returns
    /// Returns a tuple consisting of a boolean and a potential error message
    /// The boolean is true if the backend and fname are compatible. False if not
    fn check_backend_file_ext_compatibility(&self) -> OpmResult<()> {
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
                    Ok(())
                } else {
                    Err(OpossumError::Other("Incompatible file extension for DrawingBackend: BitmapBackend! Choose \".jpg\", \".bmp\" or \".png\" for this type of backend!".into()))
                }
            }
            PltBackEnd::SVG => {
                if std::path::Path::new(&path_fname)
                    .extension()
                    .map_or(false, |ext| ext.eq_ignore_ascii_case("svg"))
                {
                    Ok(())
                } else {
                    Err(OpossumError::Other("Incompatible file extension for DrawingBackend: SVGBackend! Choose \".svg\"for this type of backend!".into()))
                }
            }
            PltBackEnd::Buf => Ok(()),
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

    /// Defines the axes bounds of this [`Plot`] if the limit is not already defined by the initial [`PlotParameters`].
    ///
    /// # Errors
    /// - if the [`PlotData`] variant is not defined
    /// - if the [`PlotData`] is None
    pub fn define_axes_bounds(&mut self) -> OpmResult<()> {
        if let Some(dat) = &self.data {
            let axes_limits = dat.define_data_based_axes_bounds(false);

            if self.bounds.x.is_none() {
                self.bounds.x = Some(axes_limits[0]);
            }
            if self.bounds.y.is_none() {
                self.bounds.y = Some(axes_limits[1]);
            }
            if axes_limits.len() == 3 && self.bounds.z.is_none() {
                self.bounds.z = Some(axes_limits[2]);
            }
            Ok(())
        } else {
            Err(OpossumError::Other("No plot data defined!".into()))
        }
    }
}

impl TryFrom<&PlotParameters> for Plot {
    type Error = OpossumError;
    fn try_from(plt_params: &PlotParameters) -> OpmResult<Self> {
        let cmap_gradient = plt_params.get_cmap()?;
        let cbar_label_str = plt_params.get_cbar_label()?;
        let cbar_label_pos = plt_params.get_cbar_label_pos()?;
        let color = plt_params.get_color()?;
        let fig_size = plt_params.get_figsize()?;
        let x_lim = plt_params.get_xlim()?;
        let y_lim = plt_params.get_ylim()?;
        let z_lim = plt_params.get_zlim()?;
        let x_label_str = plt_params.get_x_label()?;
        let y_label_str = plt_params.get_y_label()?;
        let x_label_pos = plt_params.get_x_label_pos()?;
        let y_label_pos = plt_params.get_y_label_pos()?;

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
    ///Boundaries of the x axis. If not defined, the inserted plot data will be used to get a reasonable boundary. Holds an [`Option<AxLims>`]
    XLim(Option<AxLims>),
    ///Boundaries of the y axis. If not defined, the inserted plot data will be used to get a reasonable boundary. Holds an [`Option<AxLims>`]
    YLim(Option<AxLims>),
    ///Boundaries of the z axis. If not defined, the inserted plot data will be used to get a reasonable boundary. Holds an [`Option<AxLims>`]
    ZLim(Option<AxLims>),
    ///Figure size in pixels. Holds an `(usize, usize)` tuple
    FigSize((u32, u32)),
    ///Path to the save directory of the image. Only necessary if the data is not written into a buffer. Holds a String
    FDir(PathBuf),
    ///Name of the file to be written. Holds a String
    FName(String),
    ///Plotting backend that should be used. Holds a [`PltBackEnd`] enum
    Backend(PltBackEnd),
}

#[cfg(test)]
mod test {
    use tempfile::NamedTempFile;

    use super::*;
    #[test]
    fn empty_plot_params() {
        let plt_params = PlotParameters::empty();

        assert_eq!(plt_params.get_backend().is_err(), true);
        assert_eq!(plt_params.get_x_label().is_err(), true);
        assert_eq!(plt_params.get_x_label_pos().is_err(), true);
        assert_eq!(plt_params.get_y_label().is_err(), true);
        assert_eq!(plt_params.get_y_label_pos().is_err(), true);
        assert_eq!(plt_params.get_cbar_label().is_err(), true);
        assert_eq!(plt_params.get_cbar_label_pos().is_err(), true);
        assert_eq!(plt_params.get_xlim().is_err(), true);
        assert_eq!(plt_params.get_ylim().is_err(), true);
        assert_eq!(plt_params.get_zlim().is_err(), true);
        assert_eq!(plt_params.get_color().is_err(), true);
        assert_eq!(plt_params.get_fdir().is_err(), true);
        assert_eq!(plt_params.get_fname().is_err(), true);
        assert_eq!(plt_params.get_cmap().is_err(), true);
        assert_eq!(plt_params.get_figsize().is_err(), true);
    }
    #[test]
    fn default_plot_params() {
        let plt_params = PlotParameters::default();
        assert_eq!(plt_params.get_backend().unwrap(), PltBackEnd::BMP);
        assert_eq!(plt_params.get_x_label().unwrap(), "x".to_owned());
        assert_eq!(plt_params.get_x_label_pos().unwrap(), LabelPos::Bottom);
        assert_eq!(plt_params.get_y_label().unwrap(), "y".to_owned());
        assert_eq!(plt_params.get_y_label_pos().unwrap(), LabelPos::Left);
        assert_eq!(plt_params.get_cbar_label().unwrap(), "z value".to_owned());
        assert_eq!(plt_params.get_cbar_label_pos().unwrap(), LabelPos::Right);
        assert_eq!(plt_params.get_xlim().unwrap(), None);
        assert_eq!(plt_params.get_ylim().unwrap(), None);
        assert_eq!(plt_params.get_zlim().unwrap(), None);
        assert_eq!(plt_params.get_color().unwrap(), RGBAColor(255, 0, 0, 1.));
        assert_eq!(
            format!("{:?}", plt_params.get_cmap().unwrap().get_gradient()),
            "Gradient(Turbo)".to_owned()
        );
        assert_eq!(plt_params.get_fdir().unwrap(), current_dir().unwrap());
        assert_eq!(
            plt_params.get_fname().unwrap(),
            format!("opossum_default_plot_0.png")
        );
        assert_eq!(plt_params.get_figsize().unwrap(), (1000, 850));
    }
    #[test]
    fn new_plot_params() {
        let plt_args = vec![
            PlotArgs::XLabel("new x test".into()),
            PlotArgs::XLabelPos(LabelPos::Top),
        ];

        let plt_params = PlotParameters::new(plt_args);

        assert_eq!(plt_params.get_x_label().unwrap(), "new x test".to_owned());
        assert_eq!(plt_params.get_x_label_pos().unwrap(), LabelPos::Top);
    }
    #[test]
    fn plot_params_set_invalid() {
        let mut plt_params = PlotParameters::default();
        assert!(plt_params
            .set(&&PlotArgs::FName("test.invalidfileext".to_owned()))
            .is_err());
    }
    #[test]
    fn plot_params_backend() {
        let mut plt_params = PlotParameters::default();
        plt_params.set(&PlotArgs::Backend(PltBackEnd::Buf)).unwrap();
        assert_eq!(plt_params.get_backend().unwrap(), PltBackEnd::Buf);
    }
    #[test]
    fn plot_params_xlabel() {
        let mut plt_params = PlotParameters::default();
        plt_params.set(&PlotArgs::XLabel("x test".into())).unwrap();
        assert_eq!(plt_params.get_x_label().unwrap(), "x test".to_owned());
    }
    #[test]
    fn plot_params_xlabelpos() {
        let mut plt_params = PlotParameters::default();
        plt_params.set(&PlotArgs::XLabelPos(LabelPos::Top)).unwrap();
        assert_eq!(plt_params.get_x_label_pos().unwrap(), LabelPos::Top);
    }
    #[test]
    fn plot_params_ylabel() {
        let mut plt_params = PlotParameters::default();
        plt_params.set(&PlotArgs::YLabel("y test".into())).unwrap();
        assert_eq!(plt_params.get_y_label().unwrap(), "y test".to_owned());
    }
    #[test]
    fn plot_params_ylabelpos() {
        let mut plt_params = PlotParameters::default();
        plt_params
            .set(&PlotArgs::YLabelPos(LabelPos::Right))
            .unwrap();
        assert_eq!(plt_params.get_y_label_pos().unwrap(), LabelPos::Right);
    }
    #[test]
    fn plot_params_cbarlabel() {
        let mut plt_params = PlotParameters::default();
        plt_params
            .set(&PlotArgs::CBarLabel("cbar test".into()))
            .unwrap();
        assert_eq!(plt_params.get_cbar_label().unwrap(), "cbar test".to_owned());
    }
    #[test]
    fn plot_params_cbarlabelpos() {
        let mut plt_params = PlotParameters::default();
        plt_params
            .set(&PlotArgs::CBarLabelPos(LabelPos::Left))
            .unwrap();
        assert_eq!(plt_params.get_cbar_label_pos().unwrap(), LabelPos::Left);
    }
    #[test]
    fn plot_params_cmap() {
        let mut plt_params = PlotParameters::default();
        plt_params
            .set(&PlotArgs::CMap(CGradient {
                gradient: colorous::TURBO,
            }))
            .unwrap();
        assert_eq!(
            format!("{:?}", plt_params.get_cmap().unwrap().get_gradient()),
            "Gradient(Turbo)".to_owned()
        );
    }
    #[test]
    fn plot_params_ax_lims() {
        let mut plt_params = PlotParameters::default();
        plt_params
            .set(&PlotArgs::XLim(Some(AxLims { min: 0., max: 1. })))
            .unwrap();
        assert_eq!(
            plt_params.get_xlim().unwrap().unwrap(),
            AxLims { min: 0., max: 1. }
        );
        plt_params.set(&PlotArgs::XLim(None)).unwrap();
        assert_eq!(plt_params.get_xlim().unwrap(), None);

        plt_params
            .set(&PlotArgs::YLim(Some(AxLims { min: 0., max: 1. })))
            .unwrap();
        assert_eq!(
            plt_params.get_ylim().unwrap().unwrap(),
            AxLims { min: 0., max: 1. }
        );
        plt_params.set(&PlotArgs::YLim(None)).unwrap();
        assert_eq!(plt_params.get_ylim().unwrap(), None);

        plt_params
            .set(&PlotArgs::ZLim(Some(AxLims { min: 0., max: 1. })))
            .unwrap();
        assert_eq!(
            plt_params.get_zlim().unwrap().unwrap(),
            AxLims { min: 0., max: 1. }
        );
        plt_params.set(&PlotArgs::ZLim(None)).unwrap();
        assert_eq!(plt_params.get_zlim().unwrap(), None);
    }

    #[test]
    fn plot_params_color() {
        let mut plt_params = PlotParameters::default();
        plt_params
            .set(&PlotArgs::Color(RGBAColor(255, 233, 211, 0.5)))
            .unwrap();
        assert_eq!(
            plt_params.get_color().unwrap(),
            RGBAColor(255, 233, 211, 0.5)
        );
    }
    #[test]
    fn plot_params_fdir() {
        let mut plt_params = PlotParameters::default();
        let current_dir = current_dir().unwrap();
        plt_params
            .set(&PlotArgs::FDir(current_dir.clone()))
            .unwrap();
        assert_eq!(plt_params.get_fdir().unwrap(), current_dir);
    }
    #[test]
    fn plot_params_fname() {
        let mut plt_params = PlotParameters::default();
        plt_params
            .set(&PlotArgs::FName("test_name.png".to_owned()))
            .unwrap();
        assert_eq!(plt_params.get_fname().unwrap(), "test_name.png".to_owned());
    }
    #[test]
    fn plot_params_fpathh() {
        let mut plt_params = PlotParameters::default();
        plt_params
            .set(&PlotArgs::FName("test_name.png".to_owned()))
            .unwrap();
        assert!(plt_params
            .get_fpath()
            .unwrap()
            .ends_with("opossum\\opossum\\test_name.png"));
    }
    #[test]
    fn get_plot_params() {
        let plt_params = PlotParameters::default();
        let mut plt_type = PlotType::ColorTriangulated(plt_params.clone());
        let _ = plt_type.get_plot_params();
        let _ = plt_type.get_plot_params_mut();

        let mut plt_type = PlotType::ColorMesh(plt_params.clone());
        let _ = plt_type.get_plot_params();
        let _ = plt_type.get_plot_params_mut();

        let mut plt_type = PlotType::Scatter2D(plt_params.clone());
        let _ = plt_type.get_plot_params();
        let _ = plt_type.get_plot_params_mut();

        let mut plt_type = PlotType::Line2D(plt_params);
        let _ = plt_type.get_plot_params();
        let _ = plt_type.get_plot_params_mut();
    }
    #[test]
    fn plt_type_set_get_plot_param() {
        let plt_params = PlotParameters::default();
        let mut plt_type = PlotType::ColorTriangulated(plt_params);

        let _ = plt_type.set_plot_param(&PlotArgs::Backend(PltBackEnd::Buf));

        assert_eq!(
            plt_type.get_plot_params().get_backend().unwrap(),
            PltBackEnd::Buf
        );
    }
    #[test]
    fn plot_from_plotparams() {
        let plt_params = PlotParameters::default();
        let plot = Plot::try_from(&plt_params).unwrap();

        assert_eq!(plot.bounds.x, plt_params.get_xlim().unwrap());
        assert_eq!(plot.bounds.y, plt_params.get_ylim().unwrap());
        assert_eq!(plot.bounds.z, plt_params.get_zlim().unwrap());
        assert_eq!(plot.label[0].label, plt_params.get_x_label().unwrap());
        assert_eq!(plot.label[1].label, plt_params.get_y_label().unwrap());
        assert_eq!(
            plot.label[0].label_pos,
            plt_params.get_x_label_pos().unwrap()
        );
        assert_eq!(
            plot.label[1].label_pos,
            plt_params.get_y_label_pos().unwrap()
        );
        assert!(plot.data.is_none());
        assert_eq!(
            format!("{:?}", plot.cbar.cmap),
            format!("{:?}", plt_params.get_cmap().unwrap().get_gradient())
        );
        assert_eq!(plot.cbar.label.label, plt_params.get_cbar_label().unwrap());
        assert_eq!(
            plot.cbar.label.label_pos,
            plt_params.get_cbar_label_pos().unwrap()
        );
        assert_eq!(plot.img_size, plt_params.get_figsize().unwrap());
    }
    #[test]
    fn check_ax_lim_validity_valid() {
        assert!(AxLims { min: 0., max: 1. }.check_validity());
        assert!(AxLims {
            min: -10.,
            max: 10.
        }
        .check_validity());
    }
    #[test]
    fn check_ax_lim_validity_nan() {
        assert!(!AxLims {
            min: f64::NAN,
            max: 1.
        }
        .check_validity());
        assert!(!AxLims {
            min: 0.,
            max: f64::NAN
        }
        .check_validity());
    }
    #[test]
    fn check_ax_lim_validity_equal() {
        assert!(!AxLims { min: 1., max: 1. }.check_validity());
        assert!(!AxLims { min: -1., max: -1. }.check_validity());
        assert!(!AxLims {
            min: 1e20,
            max: 1e20
        }
        .check_validity());
        assert!(!AxLims {
            min: -1e20,
            max: -1e20
        }
        .check_validity());
    }
    #[test]
    fn check_ax_lim_validity_max_smaller() {
        assert!(!AxLims { min: 1., max: 0. }.check_validity());
    }
    #[test]
    fn check_ax_lim_validity_infinite() {
        assert!(!AxLims {
            min: f64::INFINITY,
            max: 1.
        }
        .check_validity());
        assert!(!AxLims {
            min: 0.,
            max: f64::INFINITY
        }
        .check_validity());
        assert!(!AxLims {
            min: -f64::INFINITY,
            max: 1.
        }
        .check_validity());
        assert!(!AxLims {
            min: 0.,
            max: -f64::INFINITY
        }
        .check_validity());
    }

    #[test]
    fn check_ax_lim_opt_validity() {
        assert!(!PlotParameters::check_ax_lim_validity(&Some(AxLims {
            min: f64::INFINITY,
            max: 1.
        })));
        assert!(PlotParameters::check_ax_lim_validity(&Some(AxLims {
            min: 0.,
            max: 1.
        })));
        assert!(PlotParameters::check_ax_lim_validity(&None));
    }
    #[test]
    fn check_plot_arg_validity_xlabel_pos() {
        assert!(PlotParameters::check_plot_arg_validity(
            &PlotArgs::XLabelPos(LabelPos::Bottom)
        ));
        assert!(PlotParameters::check_plot_arg_validity(
            &PlotArgs::XLabelPos(LabelPos::Top)
        ));
        assert!(!PlotParameters::check_plot_arg_validity(
            &PlotArgs::XLabelPos(LabelPos::Left)
        ));
        assert!(!PlotParameters::check_plot_arg_validity(
            &PlotArgs::XLabelPos(LabelPos::Right)
        ));
    }
    #[test]
    fn check_plot_arg_validity_ylabel_pos() {
        assert!(!PlotParameters::check_plot_arg_validity(
            &PlotArgs::YLabelPos(LabelPos::Bottom)
        ));
        assert!(!PlotParameters::check_plot_arg_validity(
            &PlotArgs::YLabelPos(LabelPos::Top)
        ));
        assert!(PlotParameters::check_plot_arg_validity(
            &PlotArgs::YLabelPos(LabelPos::Left)
        ));
        assert!(PlotParameters::check_plot_arg_validity(
            &PlotArgs::YLabelPos(LabelPos::Right)
        ));
    }
    #[test]
    fn check_plot_arg_validity_lims() {
        //already covered in other test, as here only a function is called which is already tested
        assert!(PlotParameters::check_plot_arg_validity(&PlotArgs::XLim(
            None
        )));
    }
    #[test]
    fn check_plot_arg_validity_figsize() {
        //already covered in other test, as here only a function is called which is already tested
        assert!(!PlotParameters::check_plot_arg_validity(
            &PlotArgs::FigSize((0, 0))
        ));
        assert!(!PlotParameters::check_plot_arg_validity(
            &PlotArgs::FigSize((0, 1))
        ));
        assert!(!PlotParameters::check_plot_arg_validity(
            &PlotArgs::FigSize((1, 0))
        ));
        assert!(PlotParameters::check_plot_arg_validity(&PlotArgs::FigSize(
            (1, 1)
        )));
    }
    #[test]
    fn check_plot_arg_validity_fname() {
        assert!(!PlotParameters::check_plot_arg_validity(&PlotArgs::FName(
            "invalid.pdf".to_owned()
        )));
        assert!(!PlotParameters::check_plot_arg_validity(&PlotArgs::FName(
            "invalid.abc".to_owned()
        )));
        assert!(PlotParameters::check_plot_arg_validity(&PlotArgs::FName(
            "valid.jpg".to_owned()
        )));
        assert!(PlotParameters::check_plot_arg_validity(&PlotArgs::FName(
            "valid.png".to_owned()
        )));
        assert!(PlotParameters::check_plot_arg_validity(&PlotArgs::FName(
            "valid.bmp".to_owned()
        )));
        assert!(PlotParameters::check_plot_arg_validity(&PlotArgs::FName(
            "valid.svg".to_owned()
        )));
        assert!(PlotParameters::check_plot_arg_validity(&PlotArgs::FName(
            "valid.sVg".to_owned()
        )));
        assert!(PlotParameters::check_plot_arg_validity(&PlotArgs::FName(
            "valid.test.sVg".to_owned()
        )));
    }
    #[test]
    fn check_backend_fpath_compatibility_test() {
        let mut plt_params = PlotParameters::default();
        assert!(plt_params.check_backend_file_ext_compatibility().is_ok());

        let _ = plt_params.set(&PlotArgs::FName("test.bmp".to_owned()));
        assert!(plt_params.check_backend_file_ext_compatibility().is_ok());

        let _ = plt_params.set(&PlotArgs::FName("test.jpg".to_owned()));
        assert!(plt_params.check_backend_file_ext_compatibility().is_ok());

        let _ = plt_params.set(&PlotArgs::FName("test.svg".to_owned()));
        assert!(plt_params.check_backend_file_ext_compatibility().is_err());

        let _ = plt_params.set(&PlotArgs::Backend(PltBackEnd::SVG));
        assert!(plt_params.check_backend_file_ext_compatibility().is_ok());

        let _ = plt_params.set(&PlotArgs::FName("test.bmp".to_owned()));
        assert!(plt_params.check_backend_file_ext_compatibility().is_err());

        let _ = plt_params.set(&PlotArgs::Backend(PltBackEnd::Buf));
        assert!(plt_params.check_backend_file_ext_compatibility().is_ok());

        //Result is an error, but Buf is fine with everything
        let _ = plt_params.set(&PlotArgs::FName("test.abcdefghijkelemenop".to_owned()));
        assert!(plt_params.check_backend_file_ext_compatibility().is_ok());
    }
    #[test]
    fn linspace_test() {
        let x = linspace(1., 3., 3.).unwrap();
        assert_eq!(x.len(), 3);
        assert!((x[0] - 1.).abs() < f64::EPSILON);
        assert!((x[1] - 2.).abs() < f64::EPSILON);
        assert!((x[2] - 3.).abs() < f64::EPSILON);
        assert!(linspace(1., 3., -3.).is_err());
    }
    #[test]
    fn new_plot() {
        let plt_params = PlotParameters::default();
        let x = linspace(0., 2., 3.).unwrap();
        let y = linspace(3., 5., 3.).unwrap();
        let plt_dat_dim2 = PlotData::Dim2(MatrixXx2::from_columns(&[x, y]));

        let plot = Plot::new(&plt_dat_dim2, &plt_params);
        assert!(plot.get_data().is_some());

        if let PlotData::Dim2(xy) = plot.get_data().unwrap() {
            assert!((xy[(0, 0)] - 0.).abs() < f64::EPSILON);
            assert!((xy[(0, 1)] - 3.).abs() < f64::EPSILON);
        }
    }
    #[test]
    fn set_get_plot_data() {
        let plt_params = PlotParameters::default();
        let mut plot = Plot::try_from(&plt_params).unwrap();

        let x = linspace(0., 2., 3.).unwrap();
        let y = linspace(3., 5., 3.).unwrap();
        let plt_dat_dim2 = PlotData::Dim2(MatrixXx2::from_columns(&[x, y]));

        assert!(plot.get_data().is_none());
        plot.set_data(plt_dat_dim2);

        if let PlotData::Dim2(xy) = plot.get_data().unwrap() {
            assert!((xy[(0, 0)] - 0.).abs() < f64::EPSILON);
            assert!((xy[(0, 1)] - 3.).abs() < f64::EPSILON);
        }
    }
    #[test]
    fn define_data_based_axes_bounds_test() {
        let x = linspace(0., 1., 2.).unwrap().transpose();
        let dat_2d = MatrixXx2::from_columns(&[x.clone().transpose(), x.transpose()]);
        let plt_dat_dim2 = PlotData::Dim2(dat_2d);

        let axlims = plt_dat_dim2.define_data_based_axes_bounds(true);
        assert!((axlims[0].min + 0.1).abs() < f64::EPSILON);
        assert!((axlims[0].max - 1.1).abs() < f64::EPSILON);
        assert!((axlims[1].min + 0.1).abs() < f64::EPSILON);
        assert!((axlims[1].max - 1.1).abs() < f64::EPSILON);
    }
    #[test]
    fn define_plot_axes_bounds() {
        //define test data
        let x = linspace(-2., -1., 2.).unwrap();
        let y = linspace(2., 3., 2.).unwrap();
        let z = linspace(4., 5., 2.).unwrap();
        let z_mat = x.clone() * y.clone().transpose();
        let dummmy_triangles: Vec<usize> = vec![1, 2, 3, 4, 5, 6];
        let dat_2d = MatrixXx2::from_columns(&[x.clone(), y.clone()]);
        let dat_3d = MatrixXx3::from_columns(&[x.clone(), y.clone(), z.clone()]);

        //define PlotData
        let plt_dat_dim2 = PlotData::Dim2(dat_2d);
        let plt_dat_dim3 = PlotData::Dim3(dat_3d.clone());
        let plt_dat_colormesh = PlotData::ColorMesh(x.clone(), y.clone(), z_mat.clone());
        let plt_dat_colortriangulated = PlotData::ColorTriangulated(
            MatrixXx3::from_vec(dummmy_triangles.clone()),
            y.clone(),
            dat_3d.clone(),
        );
        let plt_dat_surf_triangle = PlotData::TriangulatedSurface(
            MatrixXx3::from_vec(dummmy_triangles.clone()),
            dat_3d.clone(),
        );

        let mut plot = Plot::try_from(&PlotParameters::default()).unwrap();
        assert!(plot.define_axes_bounds().is_err());

        let mut plot = Plot::new(&plt_dat_dim2, &PlotParameters::default());
        let _ = plot.define_axes_bounds();
        assert!((plot.bounds.x.unwrap().min + 2.).abs() < f64::EPSILON);
        assert!((plot.bounds.x.unwrap().max + 1.).abs() < f64::EPSILON);
        assert!((plot.bounds.y.unwrap().min - 2.).abs() < f64::EPSILON);
        assert!((plot.bounds.y.unwrap().max - 3.).abs() < f64::EPSILON);
        assert!(plot.bounds.z.is_none());

        let mut plot = Plot::new(&plt_dat_dim3, &PlotParameters::default());
        let _ = plot.define_axes_bounds();
        assert!((plot.bounds.x.unwrap().min + 2.).abs() < f64::EPSILON);
        assert!((plot.bounds.x.unwrap().max + 1.).abs() < f64::EPSILON);
        assert!((plot.bounds.y.unwrap().min - 2.).abs() < f64::EPSILON);
        assert!((plot.bounds.y.unwrap().max - 3.).abs() < f64::EPSILON);
        assert!((plot.bounds.z.unwrap().min - 4.).abs() < f64::EPSILON);
        assert!((plot.bounds.z.unwrap().max - 5.).abs() < f64::EPSILON);

        let mut plot = Plot::new(&plt_dat_colormesh, &PlotParameters::default());
        let _ = plot.define_axes_bounds();
        assert!((plot.bounds.x.unwrap().min + 2.).abs() < f64::EPSILON);
        assert!((plot.bounds.x.unwrap().max + 1.).abs() < f64::EPSILON);
        assert!((plot.bounds.y.unwrap().min - 2.).abs() < f64::EPSILON);
        assert!((plot.bounds.y.unwrap().max - 3.).abs() < f64::EPSILON);
        assert!((plot.bounds.z.unwrap().min + 6.).abs() < f64::EPSILON);
        assert!((plot.bounds.z.unwrap().max + 2.).abs() < f64::EPSILON);

        let mut plot = Plot::new(&plt_dat_colortriangulated, &PlotParameters::default());
        let _ = plot.define_axes_bounds();
        assert!((plot.bounds.x.unwrap().min + 2.).abs() < f64::EPSILON);
        assert!((plot.bounds.x.unwrap().max + 1.).abs() < f64::EPSILON);
        assert!((plot.bounds.y.unwrap().min - 2.).abs() < f64::EPSILON);
        assert!((plot.bounds.y.unwrap().max - 3.).abs() < f64::EPSILON);
        assert!((plot.bounds.z.unwrap().min - 4.).abs() < f64::EPSILON);
        assert!((plot.bounds.z.unwrap().max - 5.).abs() < f64::EPSILON);

        let mut plot = Plot::new(&plt_dat_surf_triangle, &PlotParameters::default());
        let _ = plot.define_axes_bounds();
        assert!((plot.bounds.x.unwrap().min + 2.).abs() < f64::EPSILON);
        assert!((plot.bounds.x.unwrap().max + 1.).abs() < f64::EPSILON);
        assert!((plot.bounds.y.unwrap().min - 2.).abs() < f64::EPSILON);
        assert!((plot.bounds.y.unwrap().max - 3.).abs() < f64::EPSILON);
        assert!((plot.bounds.z.unwrap().min - 4.).abs() < f64::EPSILON);
        assert!((plot.bounds.z.unwrap().max - 5.).abs() < f64::EPSILON);
    }
    #[test]
    fn colorbar_new() {
        let colorbar = ColorBar::new(colorous::TURBO, "fancy label", LabelPos::Right);
        assert_eq!(format!("{:?}", colorbar.cmap), "Gradient(Turbo)".to_owned());
        assert_eq!(colorbar.label.label, "fancy label".to_owned());
        assert_eq!(colorbar.label.label_pos, LabelPos::Right);
    }
    #[test]
    fn colorbar_default() {
        let colorbar = ColorBar::default();
        assert_eq!(format!("{:?}", colorbar.cmap), "Gradient(Turbo)".to_owned());
        assert_eq!(colorbar.label.label, "".to_owned());
        assert_eq!(colorbar.label.label_pos, LabelPos::Right);
    }
    #[test]
    fn colorbar_set_label() {
        let mut colorbar = ColorBar::default();
        colorbar.set_label("labeltest");
        assert_eq!(colorbar.label.label, "labeltest".to_owned());
    }
    #[test]
    fn colorbar_set_pos() {
        let mut colorbar = ColorBar::default();
        colorbar.set_pos(LabelPos::Top);
        assert_eq!(colorbar.label.label_pos, LabelPos::Top);
    }
    #[test]
    fn labeldescription_new() {
        let l_desc = LabelDescription::new("test", LabelPos::Bottom);
        assert_eq!(l_desc.label_pos, LabelPos::Bottom);
        assert_eq!(l_desc.label, "test".to_owned());
    }
    #[test]
    fn cgradient_default() {
        let c_grad = CGradient::default();
        assert_eq!(
            format!("{:?}", c_grad.gradient),
            "Gradient(Turbo)".to_owned()
        );
    }
    #[test]
    fn cgradient_get_gradient() {
        let c_grad = CGradient::default();
        assert_eq!(
            format!("{:?}", c_grad.get_gradient()),
            "Gradient(Turbo)".to_owned()
        );
    }
    #[test]
    fn axlim_new() {
        assert!(AxLims::new(-10., 10.).is_ok());
        assert!(AxLims::new(0., f64::NAN).is_err());

        assert!((AxLims::new(-10., 10.).unwrap().min + 10.).abs() < f64::EPSILON);
        assert!((AxLims::new(-10., 10.).unwrap().max - 10.).abs() < f64::EPSILON);
    }
    #[test]
    fn axlim_expand() {
        let mut axlim = AxLims::new(-10., 10.).unwrap();
        let _ = axlim.expand_lims(0.1);

        assert!((axlim.min + 12.).abs() < f64::EPSILON);
        assert!((axlim.max - 12.).abs() < f64::EPSILON);
        assert!(axlim.expand_lims(-1.).is_err());
        assert!(axlim.expand_lims(f64::NAN).is_err());
        assert!(axlim.expand_lims(f64::INFINITY).is_err());
        assert!(axlim.expand_lims(0.).is_err());
    }
    #[test]
    fn make_plot_axlims_useful_test() {
        let mut axlim = AxLims { min: 0., max: 10. };
        axlim.make_plot_axlims_useful();
        assert!((axlim.max - 10.).abs() < f64::EPSILON);

        let mut axlim = AxLims { min: 10., max: 10. };
        axlim.make_plot_axlims_useful();
        assert!((axlim.max - 10.).abs() < f64::EPSILON);
        assert!(axlim.min.abs() < f64::EPSILON);

        let mut axlim = AxLims { min: 0., max: 0. };
        axlim.make_plot_axlims_useful();
        assert!((axlim.max - 1.).abs() < f64::EPSILON);
        assert!(axlim.min.abs() < f64::EPSILON);
    }
    #[test]
    fn get_ax_val_distance_if_equidistant_test() {
        let x = linspace(0., 1., 101.).unwrap();
        let dist = PlotType::get_ax_val_distance_if_equidistant(&x);
        assert!((dist - 0.005).abs() < f64::EPSILON);

        let x = linspace(0., f64::EPSILON, 101.).unwrap();
        let dist = PlotType::get_ax_val_distance_if_equidistant(&x);
        assert!((dist - 0.5).abs() < f64::EPSILON);
    }
    #[test]
    fn check_equistancy_of_mesh_test() {
        let x = linspace(0., 1., 101.).unwrap();
        assert!(PlotType::check_equistancy_of_mesh(&x));

        let x = linspace(-118.63435185555608, 0.000000000000014210854715202004, 100.).unwrap();
        assert!(PlotType::check_equistancy_of_mesh(&x));

        let x = MatrixXx1::from_vec(vec![0., 1., 3.]);
        assert!(!PlotType::check_equistancy_of_mesh(&x));

        let x = MatrixXx1::from_vec(vec![0.]);
        assert!(PlotType::check_equistancy_of_mesh(&x));
    }
    #[test]
    fn calc_pixel_margin_test() {
        let axlims = AxLims::new(0., 1.).unwrap();
        assert!(PlotType::calc_pixel_margin(axlims) == 72);

        let axlims = AxLims::new(-100., 1.).unwrap();
        assert!(PlotType::calc_pixel_margin(axlims) == 85);

        let axlims = AxLims::new(-10., 1000.).unwrap();
        assert!(PlotType::calc_pixel_margin(axlims) == 85);
    }

    #[test]
    fn create_plots_png_test() {
        //define test data
        let x = linspace(-2., -1., 3.).unwrap();
        let y = linspace(2., 3., 3.).unwrap();
        let z = linspace(4., 5., 3.).unwrap();
        let z_mat = x.clone() * y.clone().transpose();

        let dat_2d = MatrixXx2::from_columns(&[x.clone(), y.clone()]);
        let dat_3d = MatrixXx3::from_columns(&[x.clone(), y.clone(), z.clone()]);

        let points: Vec<Point> = dat_3d
            .row_iter()
            .map(|c| Point { x: c[0], y: c[1] })
            .collect::<Vec<Point>>();

        let trianglulation = triangulate(&points);
        let triangles = trianglulation.triangles;
        let dummmy_triangles = Matrix3xX::from_vec(triangles).transpose();

        //define PlotData
        let plt_dat_dim2 = PlotData::Dim2(dat_2d);
        let plt_dat_colormesh = PlotData::ColorMesh(x.clone(), y.clone(), z_mat.clone());
        let plt_dat_colortriangulated =
            PlotData::ColorTriangulated(dummmy_triangles.clone(), y.clone(), dat_3d.clone());
        let plt_dat_surf_triangle = PlotData::TriangulatedSurface(dummmy_triangles, dat_3d.clone());

        let mut plt_params = PlotParameters::default();
        let path = NamedTempFile::new().unwrap();
        let _ = plt_params.set(&PlotArgs::FDir(path.path().parent().unwrap().into()));
        let _ = PlotType::Line2D(plt_params.clone()).plot(&plt_dat_dim2);
        let _ = PlotType::ColorMesh(plt_params.clone()).plot(&plt_dat_colormesh);
        let _ = PlotType::ColorTriangulated(plt_params.clone()).plot(&plt_dat_colortriangulated);
        let _ = PlotType::Scatter2D(plt_params.clone()).plot(&plt_dat_dim2);
        let _ = PlotType::TriangulatedSurface(plt_params.clone()).plot(&plt_dat_surf_triangle);
    }
    #[test]
    fn create_plots_svg_test() {
        //define test data
        let x = linspace(-2., -1., 3.).unwrap();
        let y = linspace(2., 3., 3.).unwrap();
        let z = linspace(4., 5., 3.).unwrap();
        let z_mat = x.clone() * y.clone().transpose();

        let dat_2d = MatrixXx2::from_columns(&[x.clone(), y.clone()]);
        let dat_3d = MatrixXx3::from_columns(&[x.clone(), y.clone(), z.clone()]);

        let points: Vec<Point> = dat_3d
            .row_iter()
            .map(|c| Point { x: c[0], y: c[1] })
            .collect::<Vec<Point>>();

        let trianglulation = triangulate(&points);
        let triangles = trianglulation.triangles;
        let dummmy_triangles = Matrix3xX::from_vec(triangles).transpose();

        //define PlotData
        let plt_dat_dim2 = PlotData::Dim2(dat_2d);
        let plt_dat_colormesh = PlotData::ColorMesh(x.clone(), y.clone(), z_mat.clone());
        let plt_dat_colortriangulated =
            PlotData::ColorTriangulated(dummmy_triangles.clone(), y.clone(), dat_3d.clone());
        let plt_dat_surf_triangle = PlotData::TriangulatedSurface(dummmy_triangles, dat_3d.clone());

        let mut plt_params = PlotParameters::default();
        let path = NamedTempFile::new().unwrap();
        let _ = plt_params
            .set(&PlotArgs::FDir(path.path().parent().unwrap().into()))
            .unwrap()
            .set(&PlotArgs::Backend(PltBackEnd::SVG))
            .unwrap()
            .set(&PlotArgs::FName("test.svg".into()));
        let _ = PlotType::Line2D(plt_params.clone()).plot(&plt_dat_dim2);
        let _ = PlotType::ColorMesh(plt_params.clone()).plot(&plt_dat_colormesh);
        let _ = PlotType::ColorTriangulated(plt_params.clone()).plot(&plt_dat_colortriangulated);
        let _ = PlotType::Scatter2D(plt_params.clone()).plot(&plt_dat_dim2);
        let _ = PlotType::TriangulatedSurface(plt_params.clone()).plot(&plt_dat_surf_triangle);
    }
    #[test]
    fn create_plots_buffer_test() {
        //define test data
        let x = linspace(-2., -1., 3.).unwrap();
        let y = linspace(2., 3., 3.).unwrap();
        let z = linspace(4., 5., 3.).unwrap();
        let z_mat = x.clone() * y.clone().transpose();

        let dat_2d = MatrixXx2::from_columns(&[x.clone(), y.clone()]);
        let dat_3d = MatrixXx3::from_columns(&[x.clone(), y.clone(), z.clone()]);

        let points: Vec<Point> = dat_3d
            .row_iter()
            .map(|c| Point { x: c[0], y: c[1] })
            .collect::<Vec<Point>>();

        let trianglulation = triangulate(&points);
        let triangles = trianglulation.triangles;
        let dummmy_triangles = Matrix3xX::from_vec(triangles).transpose();

        //define PlotData
        let plt_dat_dim2 = PlotData::Dim2(dat_2d);
        let plt_dat_colormesh = PlotData::ColorMesh(x.clone(), y.clone(), z_mat.clone());
        let plt_dat_colortriangulated =
            PlotData::ColorTriangulated(dummmy_triangles.clone(), y.clone(), dat_3d.clone());
        let plt_dat_surf_triangle = PlotData::TriangulatedSurface(dummmy_triangles, dat_3d.clone());

        let mut plt_params = PlotParameters::default();
        let path = NamedTempFile::new().unwrap();
        let _ = plt_params
            .set(&PlotArgs::FDir(path.path().parent().unwrap().into()))
            .unwrap()
            .set(&PlotArgs::Backend(PltBackEnd::Buf));
        let _ = PlotType::Line2D(plt_params.clone()).plot(&plt_dat_dim2);
        let _ = PlotType::ColorMesh(plt_params.clone()).plot(&plt_dat_colormesh);
        let _ = PlotType::ColorTriangulated(plt_params.clone()).plot(&plt_dat_colortriangulated);
        let _ = PlotType::Scatter2D(plt_params.clone()).plot(&plt_dat_dim2);
        let _ = PlotType::TriangulatedSurface(plt_params.clone()).plot(&plt_dat_surf_triangle);
    }
}
