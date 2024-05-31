#![warn(missing_docs)]
//! Trait for adding the possibility to generate a (x/y) plot of an element.
use crate::error::{OpmResult, OpossumError};
use crate::utils::filter_data::filter_nan_infinite;
use crate::utils::griddata::create_valued_voronoi_cells;
use crate::utils::{filter_data::get_min_max_filter_nonfinite, griddata::linspace};
use approx::{abs_diff_ne, relative_ne, RelativeEq};
use colorous::Gradient;
use image::RgbImage;
use itertools::{iproduct, izip, Itertools};
use kahan::KahanSum;
use log::warn;
use nalgebra::{
    DMatrix, DVector, DVectorSlice, Matrix3xX, MatrixXx1, MatrixXx2, MatrixXx3, Vector3,
};
use num::ToPrimitive;
use plotters::{
    backend::DrawingBackend,
    backend::PixelFormat,
    chart::{ChartBuilder, ChartContext, LabelAreaPosition, MeshStyle, SeriesLabelPosition},
    coord::{cartesian::Cartesian2d, ranged3d::Cartesian3d, types::RangedCoordf64, Shift},
    element::{Circle, PathElement, Polygon, Rectangle},
    prelude::{BitMapBackend, DrawingArea, IntoDrawingArea, SVGBackend},
    series::LineSeries,
    style::{Color, IntoFont, RGBAColor, ShapeStyle, BLACK, WHITE},
};
use std::{collections::HashMap, env::current_dir, f64::consts::PI, path::Path, path::PathBuf};
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
    /// 3D surface plot of ungridded data
    TriangulatedSurface(PlotParameters),
}
impl PlotType {
    const fn get_plot_params(&self) -> &PlotParameters {
        match self {
            Self::ColorMesh(p)
            | Self::Scatter2D(p)
            | Self::Line2D(p)
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
            | Self::MultiLine3D(p)
            | Self::MultiLine2D(p)
            | Self::TriangulatedSurface(p) => p,
        }
    }
    fn create_plot<B: DrawingBackend>(&self, backend: &DrawingArea<B, Shift>, plot: &mut Plot) {
        plot.define_axes_bounds();
        let _ = backend.fill(&WHITE);
        match self {
            Self::ColorMesh(_) => Self::plot_color_mesh(plot, backend),
            Self::TriangulatedSurface(_) => Self::plot_triangulated_surface(plot, backend),
            Self::Scatter2D(_) => Self::plot_2d_scatter(plot, backend),
            Self::Line2D(_) => Self::plot_2d_line(plot, backend),
            Self::MultiLine3D(_) => Self::plot_3d_multi_line(plot, backend),
            Self::MultiLine2D(_) => Self::plot_2d_multi_line(plot, backend),
        };
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
    /// - `plt_series`: vector of plot series. See [`PlotSeries`]
    /// # Returns
    /// This method returns an [`OpmResult<Option<RgbImage>>`]. It is None if a new file (such as svg, png, bmp or jpg) is created. It is Some(RgbImage) if the image is written to a buffer
    /// # Errors
    /// This method throws an error if
    /// - some plot parameters contradict each other
    /// - the file path can not be extracted
    /// - the plotting backend can not be extracted
    /// - the plot can not be created inside the `create_plot()` method
    /// - the image buffer is too small
    pub fn plot(&self, plt_series: &Vec<PlotSeries>) -> OpmResult<Option<RgbImage>> {
        let params = self.get_plot_params();
        params.check_backend_file_ext_compatibility()?;
        let path = params.get_fpath()?;
        let mut plot = Plot::new(plt_series, params);
        plot.add_margin_to_figure_size(self);

        match params.get_backend()? {
            PltBackEnd::BMP => {
                let backend = BitMapBackend::new(&path, plot.fig_size).into_drawing_area();
                self.create_plot(&backend, &mut plot);
                Ok(None)
            }
            PltBackEnd::SVG => {
                let backend = SVGBackend::new(&path, plot.fig_size).into_drawing_area();
                self.create_plot(&backend, &mut plot);
                Ok(None)
            }
            PltBackEnd::Buf => {
                let mut image_buffer = vec![
                    0;
                    (plot.fig_size.0 * plot.fig_size.1) as usize
                        * plotters::backend::RGBPixel::PIXEL_SIZE
                ];
                {
                    let backend = BitMapBackend::with_buffer(&mut image_buffer, plot.fig_size)
                        .into_drawing_area();
                    self.create_plot(&backend, &mut plot);
                }
                let img = RgbImage::from_raw(plot.fig_size.0, plot.fig_size.1, image_buffer)
                    .ok_or_else(|| OpossumError::Other("image buffer size too small".into()))?;
                Ok(Some(img))
            }
        }
    }

    fn draw_line_2d<'a, 'b, T: DrawingBackend + 'a + 'b>(
        chart: &'a mut ChartContext<'b, T, Cartesian2d<RangedCoordf64, RangedCoordf64>>,
        x: &DVectorSlice<'_, f64>,
        y: &DVectorSlice<'_, f64>,
        line_color: RGBAColor,
        label: Option<String>,
    ) {
        let series_anno = chart
            .draw_series(LineSeries::new(
                izip!(x, y).map(|xy| (*xy.0, *xy.1)),
                line_color,
            ))
            .unwrap();

        if let Some(l) = label {
            series_anno
                .label(&l)
                .legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], line_color));
        }
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
        line_color: RGBAColor,
        label: Option<String>,
    ) {
        let series_anno = chart
            .draw_series(LineSeries::new(
                izip!(x, y, z).map(|xyz| (*xyz.0, *xyz.1, *xyz.2)),
                line_color,
            ))
            .unwrap();

        if let Some(l) = label {
            series_anno
                .label(&l)
                .legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], line_color));
        }
    }

    fn draw_points<'a, 'b, T: DrawingBackend + 'a + 'b>(
        chart: &'a mut ChartContext<'b, T, Cartesian2d<RangedCoordf64, RangedCoordf64>>,
        x: &DVectorSlice<'_, f64>,
        y: &DVectorSlice<'_, f64>,
        marker_color: RGBAColor,
        label: Option<String>,
    ) {
        let series_anno = chart
            .draw_series(izip!(x, y).map(|x| {
                Circle::new(
                    (*x.0, *x.1),
                    3,
                    Into::<ShapeStyle>::into(marker_color).filled(),
                )
            }))
            .unwrap();

        if let Some(l) = label {
            series_anno.label(&l).legend(move |(x, y)| {
                Circle::new((x, y), 3, Into::<ShapeStyle>::into(marker_color).filled())
            });
        }
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
        triangle_color: RGBAColor,
        _triangle_normals: &MatrixXx3<f64>,
    ) {
        let _view = Vector3::new(-1., -1., -1.);
        let series = triangle_index
            .row_iter()
            // .filter(|(_, n)| n.transpose().dot(&view) > 0.)
            .map(|idx| {
                Polygon::new(
                    vec![
                        (x[idx[0]], y[idx[0]], z[idx[0]]),
                        (x[idx[1]], y[idx[1]], z[idx[1]]),
                        (x[idx[2]], y[idx[2]], z[idx[2]]),
                    ],
                    Into::<ShapeStyle>::into(triangle_color).filled(),
                )
            });
        chart.draw_series(series).unwrap();
        let series = triangle_index
            .row_iter()
            // .filter(|(_, n)| n.transpose().dot(&view) > 0.)
            .map(|idx| {
                PathElement::new(
                    vec![
                        (x[idx[0]], y[idx[0]], z[idx[0]]),
                        (x[idx[1]], y[idx[1]], z[idx[1]]),
                        (x[idx[2]], y[idx[2]], z[idx[2]]),
                    ],
                    ShapeStyle {
                        color: RGBAColor(0, 0, 0, 1.),
                        filled: false,
                        stroke_width: 1,
                    },
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
        let mut dist = ax_vals[1] - ax_vals[0]; // / 2.;
        if Self::check_equistancy_of_mesh(ax_vals) {
            if dist <= 2. * f64::EPSILON {
                dist = 0.5;
            }
        } else {
            warn!(
                "Warning! The points on this axis are not equidistant!\n This may distort the plot!"
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

    fn config_series_label_2d<'a, 'b, T: DrawingBackend + 'a + 'b>(
        chart: &'a mut ChartContext<'b, T, Cartesian2d<RangedCoordf64, RangedCoordf64>>,
    ) {
        chart
            .configure_series_labels()
            .position(SeriesLabelPosition::UpperLeft)
            .legend_area_size(50)
            .background_style(BLACK.mix(0.05))
            .border_style(BLACK)
            .label_font(("Calibri", 30).into_font())
            .draw()
            .unwrap();
    }

    fn config_series_label_3d<'a, 'b, T: DrawingBackend + 'a + 'b>(
        chart: &'a mut ChartContext<
            'b,
            T,
            Cartesian3d<RangedCoordf64, RangedCoordf64, RangedCoordf64>,
        >,
    ) {
        chart
            .configure_series_labels()
            .position(SeriesLabelPosition::UpperLeft)
            .legend_area_size(50)
            .background_style(BLACK.mix(0.05))
            .border_style(BLACK)
            .label_font(("Calibri", 30).into_font())
            .draw()
            .unwrap();
    }

    fn plot_2d_line<B: DrawingBackend>(plt: &Plot, root: &DrawingArea<B, Shift>) {
        if let Some(plt_series_vec) = plt.get_plot_series_vec() {
            let mut chart = Self::create_2d_plot_chart(
                root,
                plt.bounds.x.unwrap(),
                plt.bounds.y.unwrap(),
                &plt.label,
                true,
                true,
            );

            let mut label_flag = false;
            for plt_series in plt_series_vec {
                if let PlotData::Dim2 { xy_data } = plt_series.get_plot_series_data() {
                    Self::draw_line_2d(
                        &mut chart,
                        &xy_data.column(0),
                        &xy_data.column(1),
                        *plt_series.get_series_color(),
                        plt_series.get_series_label(),
                    );
                    label_flag |= plt_series.get_series_label().is_some();
                } else {
                    warn!("Wrong PlotData stored for this plot type! Must use Dim2! Not all series will be plotted!");
                }
            }
            if label_flag {
                Self::config_series_label_2d(&mut chart);
            }
        } else {
            warn!("No plot series defined! Cannot create plot!!");
        }
        root.present().unwrap();
    }

    fn plot_2d_scatter<B: DrawingBackend>(plt: &Plot, root: &DrawingArea<B, Shift>) {
        if let Some(plt_series_vec) = plt.get_plot_series_vec() {
            let mut chart = Self::create_2d_plot_chart(
                root,
                plt.bounds.x.unwrap(),
                plt.bounds.y.unwrap(),
                &plt.label,
                true,
                true,
            );

            let mut label_flag = false;
            for plt_series in plt_series_vec {
                if let PlotData::Dim2 { xy_data } = plt_series.get_plot_series_data() {
                    Self::draw_points(
                        &mut chart,
                        &xy_data.column(0),
                        &xy_data.column(1),
                        *plt_series.get_series_color(),
                        plt_series.get_series_label(),
                    );
                    label_flag |= plt_series.get_series_label().is_some();
                } else {
                    warn!("Wrong PlotData stored for this plot type! Must use Dim2! Not all series will be plotted!");
                }
            }

            if label_flag {
                Self::config_series_label_2d(&mut chart);
            }
        } else {
            warn!("No plot series defined! Cannot create plot!!");
        }

        root.present().unwrap();
    }

    fn plot_2d_multi_line<B: DrawingBackend>(plt: &Plot, root: &DrawingArea<B, Shift>) {
        if let Some(plt_series_vec) = plt.get_plot_series_vec() {
            let mut label_flag = false;
            let mut chart = Self::create_2d_plot_chart(
                root,
                plt.bounds.x.unwrap(),
                plt.bounds.y.unwrap(),
                &plt.label,
                true,
                true,
            );
            for plt_series in plt_series_vec {
                if let PlotData::MultiDim2 { vec_of_xy_data } = plt_series.get_plot_series_data() {
                    //main plot
                    //currently there is no support for axes labels in 3d plots
                    let color = plt_series.get_series_color();

                    for (i, line_dat) in vec_of_xy_data.iter().enumerate() {
                        let label = if i == 0 {
                            label_flag |= plt_series.get_series_label().is_some();
                            plt_series.get_series_label()
                        } else {
                            None
                        };

                        Self::draw_line_2d(
                            &mut chart,
                            &line_dat.column(0),
                            &line_dat.column(1),
                            *color,
                            label,
                        );
                    }
                } else {
                    warn!("Wrong PlotData stored for this plot type! Must use MultiDim2! Not all series will be plotted!");
                }
            }
            if label_flag {
                Self::config_series_label_2d(&mut chart);
            }
        } else {
            warn!("No plot series defined! Cannot create plot!");
        }
    }

    fn plot_3d_multi_line<B: DrawingBackend>(plt: &Plot, root: &DrawingArea<B, Shift>) {
        if let Some(plt_series_vec) = plt.get_plot_series_vec() {
            let mut label_flag = false;
            let mut chart = Self::create_3d_plot_chart(root, plt);

            for plt_series in plt_series_vec {
                if let PlotData::MultiDim3 { vec_of_xyz_data } = plt_series.get_plot_series_data() {
                    //main plot
                    //currently there is no support for axes labels in 3d plots
                    for (i, line_dat) in vec_of_xyz_data.iter().enumerate() {
                        let label = if i == 0 {
                            label_flag |= plt_series.get_series_label().is_some();
                            plt_series.get_series_label()
                        } else {
                            None
                        };

                        Self::draw_line_3d(
                            &mut chart,
                            &line_dat.column(0),
                            &line_dat.column(2),
                            &line_dat.column(1),
                            RGBAColor(255, 0, 0, 0.3),
                            label,
                        );
                    }
                } else {
                    warn!("Wrong PlotData stored for this plot type! Must use MultiDim3! Not all series will be plotted!");
                }
            }
            if label_flag {
                Self::config_series_label_3d(&mut chart);
            }
        } else {
            warn!("No plot series defined! Cannot create plot!");
        }
    }

    fn plot_triangulated_surface<B: DrawingBackend>(plt: &Plot, root: &DrawingArea<B, Shift>) {
        if let Some(plt_series_vec) = plt.get_plot_series_vec() {
            if plt_series_vec.len() > 1 {
                warn!("For this type of plot only one series can be plotted at a time. Only the first series will be used!");
            }
            if let PlotData::TriangulatedSurface {
                triangle_idx,
                xyz_dat,
                triangle_face_normals: triangle_normals,
            } = plt_series_vec[0].get_plot_series_data()
            {
                //main plot
                //currently there is no support for axes labels in 3d plots
                let mut chart = Self::create_3d_plot_chart(root, plt);

                Self::draw_triangle_surf(
                    &mut chart,
                    triangle_idx,
                    &xyz_dat.column(0),
                    &xyz_dat.column(1),
                    &xyz_dat.column(2),
                    plt_series_vec[0].color,
                    triangle_normals,
                );
            } else {
                warn!("Wrong PlotData stored for this plot type! Must use TriangulatedSurface! Not all series will be plotted!");
            }
        } else {
            warn!("No plot series defined! Cannot create plot!");
        }
    }
    fn plot_color_mesh<B: DrawingBackend>(plt: &Plot, root: &DrawingArea<B, Shift>) {
        if let Some(plt_series_vec) = plt.get_plot_series_vec() {
            if plt_series_vec.len() > 1 {
                warn!("For this type of plot only one series can be plotted at a time. Only the first series will be used!");
            }
            if let PlotData::ColorMesh {
                x_dat_n,
                y_dat_m,
                z_dat_nxm,
            } = plt_series_vec[0].get_plot_series_data()
            {
                let split_pixel = plt.fig_size.0 - 170;
                //split root for main plot and colorbar
                let (main_root, cbar_root) = root.split_horizontally(split_pixel);

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
                    linspace(plt.bounds.z.unwrap().min, plt.bounds.z.unwrap().max, 100).unwrap();
                let d_mat = DMatrix::<f64>::from_columns(&[c_dat.clone(), c_dat]);
                let xxx = DVector::<f64>::from_vec(vec![0., 1.]);
                Self::draw_2d_colormesh(
                    &mut chart,
                    &xxx,
                    &linspace(plt.bounds.z.unwrap().min, plt.bounds.z.unwrap().max, 100).unwrap(),
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

                Self::draw_2d_colormesh(
                    &mut chart,
                    x_dat_n,
                    y_dat_m,
                    z_dat_nxm,
                    &plt.cbar.cmap,
                    plt.bounds.z.unwrap(),
                );
            } else {
                warn!("Wrong PlotData stored for this plot type! Must use ColorMesh! Not all series will be plotted!");
            }
        } else {
            warn!("No plot series defined! Cannot create plot!");
        }
        root.present().unwrap();
    }

    fn create_3d_plot_chart<'a, T: DrawingBackend>(
        root: &'a DrawingArea<T, Shift>,
        plot: &Plot,
    ) -> ChartContext<'a, T, Cartesian3d<RangedCoordf64, RangedCoordf64, RangedCoordf64>> {
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
                pb.pitch = 45. / 180. * PI;
                pb.yaw = 45. / 180. * PI;
                pb.pitch = 0. / 180. * PI;
                pb.yaw = 0. / 180. * PI;
                pb.scale = 0.7;
                pb.into_matrix()
            },
        );

        chart.configure_axes().draw().unwrap();

        chart
    }

    fn tick_formatter(range: core::ops::Range<f64>) -> impl Fn(&f64) -> String {
        let log_val = range
            .end
            .abs()
            .max(range.start.abs())
            .log10()
            .floor()
            .to_i32()
            .unwrap();

        move |v: &_| match log_val {
            -3 | -2 => format!("{v:.3}"),
            -1 | 0 => format!("{v:.2}"),
            1 => format!("{v:.1}"),
            2 => format!("{v:.0}"),
            _ => format!("{v}"),
        }
    }

    fn create_2d_plot_chart<'a, T: DrawingBackend>(
        root: &'a DrawingArea<T, Shift>,
        x_bounds: AxLims,
        y_bounds: AxLims,
        label_desc: &[LabelDescription; 2],
        y_ax: bool,
        x_ax: bool,
    ) -> ChartContext<'a, T, Cartesian2d<RangedCoordf64, RangedCoordf64>> {
        let mut chart_builder = ChartBuilder::on(root);
        chart_builder.margin(30).margin_top(40).margin_left(10);

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

        let x_format = Self::tick_formatter(chart.x_range());
        let y_format = Self::tick_formatter(chart.y_range());
        let mut mesh = chart.configure_mesh();
        mesh.x_labels(5).y_labels(5);

        mesh.x_label_formatter(&x_format)
            .y_label_formatter(&y_format);

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

    fn calc_pixel_margin(bounds: AxLims) -> u32 {
        let log_val_max = if relative_ne!(bounds.max.abs(), 0.) {
            bounds.max.abs().log10().floor().to_i32().unwrap()
        } else {
            -1
        };
        let log_val_min = if relative_ne!(bounds.min.abs(), 0.) {
            bounds.min.abs().log10().floor().to_i32().unwrap()
        } else {
            -1
        };

        let mut digits_min = match log_val_min {
            -3 | -2 => 5,
            _ => 4,
        };
        let mut digits_max = match log_val_max {
            -3 | -2 => 5,
            _ => 4,
        };

        digits_min += i32::from(bounds.min.is_sign_negative());
        digits_max += i32::from(bounds.max.is_sign_negative());

        let digits = digits_max.max(digits_min).to_u32().unwrap();

        // //absolutely ugly "automation" of margin. not done nicely and not accurate, works only for sans serif with 30 pt
        // let mut digits_max =
        //     bounds.max.abs().log10().abs().floor() + 2. + f64::from(bounds.max.is_sign_negative());
        // if digits_max.is_infinite() {
        //     digits_max = 4.;
        // }
        // let mut digits_min =
        //     bounds.min.abs().log10().abs().floor() + 2. + f64::from(bounds.min.is_sign_negative());
        // if digits_min.is_infinite() {
        //     digits_min = 4.;
        // }
        // let digits = if digits_max >= digits_min {
        //     digits_max.to_u32()
        // } else {
        //     digits_min.to_u32()
        // }
        // .unwrap();

        digits * 13 + 20
    }
}

#[derive(Debug, Clone)]
///Enum to define the type of plot that should be created
pub enum PlotData {
    ///[`PlotData`] for [`PlotType::Scatter2D`] & [`PlotType::Line2D`]
    Dim2 {
        /// Pairwise 2D data (e.g. x, y data), structured as Matrix with N rows and two columns (x,y)
        xy_data: MatrixXx2<f64>,
    },
    ///[`PlotData`] for `PlotType::Scatter3D` & `PlotType::Line3D`
    Dim3 {
        ///Triplet 3D data (e.g. x, y, z data), structured as Matrix with N rows and three columns (x,y,z)
        xyz_data: MatrixXx3<f64>,
    },
    ///[`PlotData`] for [`PlotType::MultiLine2D`]
    MultiDim2 {
        /// Vector of pairwise 2D data (e.g. x, y data), structured as Vector filled with Matrices with N rows and two columns (x,y)
        vec_of_xy_data: Vec<MatrixXx2<f64>>,
    },
    ///[`PlotData`] for [`PlotType::MultiLine3D`]
    MultiDim3 {
        ///Vector of triplet 3D data (e.g. x, y, z data), structured as Vector filled with Matrices with N rows and three columns (x,y,z)
        vec_of_xyz_data: Vec<MatrixXx3<f64>>,
    },
    /// [`PlotData`] for [`PlotType::ColorMesh`]
    /// Data to create a 2d colormesh plot. Vector with N entries for x, Vector with M entries for y and a Matrix with nxm entries for the colordata
    ColorMesh {
        /// xdata: Vector with `N` entries
        x_dat_n: DVector<f64>,
        /// ydata: Vector with `M` entries
        y_dat_m: DVector<f64>,
        /// zdata: Matrix with nxm entries for the color
        z_dat_nxm: DMatrix<f64>,
    },
    /// [`PlotData`] for [`PlotType::TriangulatedSurface`]
    TriangulatedSurface {
        ///Matrix with 3 columns and N rows that is filled with the indices that correspond to the data points that ave been triangulated
        triangle_idx: MatrixXx3<usize>,
        /// - Matrix with 3 columns and N rows that hold the x,y,z data
        xyz_dat: MatrixXx3<f64>,
        ///normal vectors of each triangle
        triangle_face_normals: MatrixXx3<f64>,
    },
}

impl PlotData {
    /// Creates a new [`PlotData::Dim2`] enum variant
    ///
    /// # Errors
    /// This function will return an error if the length of the input matrix is zero
    pub fn new_dim2(xy_data: MatrixXx2<f64>) -> OpmResult<Self> {
        if xy_data.is_empty() {
            Err(OpossumError::Other(
                "No data provided! Cannot create PlotData::Dim2!".into(),
            ))
        } else {
            Ok(Self::Dim2 { xy_data })
        }
    }
    /// Creates a new [`PlotData::Dim3`] enum variant
    ///
    /// # Errors
    /// This function will return an error if the length of the input matrix is zero
    pub fn new_dim3(xyz_data: MatrixXx3<f64>) -> OpmResult<Self> {
        if xyz_data.is_empty() {
            Err(OpossumError::Other(
                "No data provided! Cannot create PlotData::Dim3!".into(),
            ))
        } else {
            Ok(Self::Dim3 { xyz_data })
        }
    }

    /// Creates a new [`PlotData::MultiDim2`] enum variant
    ///
    /// # Errors
    /// This function will return an error if the length of the input vector is zero
    pub fn new_multi_dim2(vec_of_xy_data: Vec<MatrixXx2<f64>>) -> OpmResult<Self> {
        if vec_of_xy_data.is_empty() {
            Err(OpossumError::Other(
                "No data provided! Cannot create `PlotData::MultiDim2`!".into(),
            ))
        } else {
            Ok(Self::MultiDim2 { vec_of_xy_data })
        }
    }
    /// Creates a new [`PlotData::MultiDim3`] enum variant
    ///
    /// # Errors
    /// This function will return an error if the length of the input vector is zero
    pub fn new_multi_dim3(vec_of_xyz_data: Vec<MatrixXx3<f64>>) -> OpmResult<Self> {
        if vec_of_xyz_data.is_empty() {
            Err(OpossumError::Other(
                "No data provided! Cannot create `PlotData::MultiDim3`!".into(),
            ))
        } else {
            Ok(Self::MultiDim3 { vec_of_xyz_data })
        }
    }

    /// Creates a new [`PlotData::ColorMesh`] enum variant
    ///
    /// # Errors
    /// This function will return an error if
    /// - the length of x data: `x_dat_n` is zero
    /// - the length of y data: `y_dat_m` is zero
    /// - the length of z data: `z_dat_nxm` is zero
    /// - the shape of the data sets does not match
    pub fn new_colormesh(
        x_dat_n: DVector<f64>,
        y_dat_m: DVector<f64>,
        z_dat_nxm: DMatrix<f64>,
    ) -> OpmResult<Self> {
        if x_dat_n.is_empty() {
            return Err(OpossumError::Other(
                "No x-data provided! Cannot create `PlotData::Colormesh`!".into(),
            ));
        }
        if y_dat_m.is_empty() {
            return Err(OpossumError::Other(
                "No y-data provided! Cannot create `PlotData::Colormesh`!".into(),
            ));
        }
        if z_dat_nxm.is_empty() {
            return Err(OpossumError::Other(
                "No z-data provided! Cannot create `PlotData::Colormesh`!".into(),
            ));
        }
        if x_dat_n.len() != z_dat_nxm.shape().1 || y_dat_m.len() != z_dat_nxm.shape().0 {
            return Err(OpossumError::Other(
                "shape of x, y and z does not match! z must be x.len() columns and y.len() rows!"
                    .into(),
            ));
        }
        Ok(Self::ColorMesh {
            x_dat_n,
            y_dat_m,
            z_dat_nxm,
        })
    }

    /// Creates a new [`PlotData::TriangulatedSurface`] enum variant
    ///
    /// # Errors
    /// This function will return an error if
    /// - the length of xyz data: `xyz_dat` is zero
    /// - no axis bounds for x or y can be determined
    #[allow(clippy::too_many_lines)]
    pub fn new_triangulatedsurface(
        xyz_dat: &MatrixXx3<f64>,
        triangle_idx_opt: Option<&MatrixXx3<usize>>,
        triangle_face_normals_opt: Option<&MatrixXx3<f64>>,
    ) -> OpmResult<Self> {
        if xyz_dat.is_empty() {
            return Err(OpossumError::Other(
                "No z-data provided! Cannot create `PlotData::TriangulatedSurface`!".into(),
            ));
        }
        if let (Some(triangle_idx), Some(triangle_face_normals)) =
            (triangle_idx_opt, triangle_face_normals_opt)
        {
            if triangle_idx.shape().0 != triangle_face_normals.shape().0 {
                Err(OpossumError::Other("Shapes of triangle indices and face normals does not match! Cannot create `PlotData::TriangulatedSurface`!"        .into()))
            } else if triangle_idx.iter().fold(0, |arg0, idx| *idx.max(&arg0))
                > xyz_dat.shape().0 - 1
            {
                Err(OpossumError::Other("Maximum triangle index is larger than number of points! Cannot create `PlotData::TriangulatedSurface`!"        .into()))
            } else {
                Ok(Self::TriangulatedSurface {
                    triangle_idx: triangle_idx.clone(),
                    xyz_dat: xyz_dat.clone(),
                    triangle_face_normals: triangle_face_normals.clone(),
                })
            }
        } else if let Some(triangle_idx) = triangle_idx_opt {
            if triangle_idx.iter().fold(0, |arg0, idx| *idx.max(&arg0)) > xyz_dat.shape().0 - 1 {
                Err(OpossumError::Other("Maximum triangle index is larger than number of points! Cannot create `PlotData::TriangulatedSurface`!"        .into()))
            } else {
                let triangle_face_normals = Matrix3xX::from_vec(
                    triangle_idx
                        .row_iter()
                        .flat_map(|tri_idx| {
                            let p1 = xyz_dat.row(tri_idx[0]);
                            let p2 = xyz_dat.row(tri_idx[1]);
                            let p3 = xyz_dat.row(tri_idx[2]);
                            let normal = ((p2 - p1).cross(&(p3 - p1))).normalize();
                            [normal[0], normal[1], normal[2]]
                        })
                        .collect_vec(),
                )
                .transpose();
                Ok(Self::TriangulatedSurface {
                    triangle_idx: triangle_idx.clone(),
                    xyz_dat: xyz_dat.clone(),
                    triangle_face_normals,
                })
            }
        } else {
            let min_max_x = get_min_max_filter_nonfinite(
            xyz_dat
            .column(0)
            .as_slice())
            .ok_or_else(|| OpossumError::Other("Axes bounds could not be determined! Cannot create `PlotData::TriangulatedSurface`!"        .into()))?;
            let min_max_y = get_min_max_filter_nonfinite(
            xyz_dat
            .column(1)
            .as_slice())
            .ok_or_else(|| OpossumError::Other("Axes bounds could not be determined! Cannot create `PlotData::TriangulatedSurface`!"        .into()))?;

            let voronoi = create_valued_voronoi_cells(
            xyz_dat,
            &AxLims::new(min_max_x.0, min_max_x.1).ok_or_else(|| OpossumError::Other(
                    "Cannot voronoi data with None-valued axis limits! Cannot create `PlotData::TriangulatedSurface`!"
                        .into()
                ))?,
            &AxLims::new(min_max_y.0, min_max_y.1).ok_or_else(|| OpossumError::Other(
                    "Cannot voronoi data with None-valued axis limits! Cannot create `PlotData::TriangulatedSurface`!"
                        .into()
                ))?,
        )?;
            let z_data = voronoi.get_z_data().as_ref().map_or_else(
            || {
                Err(OpossumError::Other(
                    "Could not extract z data from voronoi diagram! Cannot create `PlotData::TriangulatedSurface`!"
                        .into(),
                ))
            },
            |z_data| Ok(DVector::from(z_data.column(0))),
        )?;
            // let (x, y): (Vec<f64>, Vec<f64>) = voronoi.get_voronoi_diagram().sites.iter().cloned().map(|p| (p.x, p.y)).unzip();
            let triangles = voronoi.get_voronoi_diagram().delaunay.triangles.clone();
            let mut filtered_triangles = Vec::<usize>::with_capacity(triangles.len());
            let triangle_idx = Matrix3xX::from_vec(triangles).transpose();
            let len_dat = xyz_dat.shape().0;
            for row in triangle_idx.row_iter() {
                if row[0] < len_dat && row[1] < len_dat && row[2] < len_dat {
                    filtered_triangles.push(row[0]);
                    filtered_triangles.push(row[1]);
                    filtered_triangles.push(row[2]);
                }
            }
            let triangle_idx_filtered = Matrix3xX::from_vec(filtered_triangles).transpose();
            let xyz_dat = MatrixXx3::from_columns(&[
                xyz_dat.column(0),
                xyz_dat.column(1),
                z_data.rows(0, len_dat),
            ]);

            let triangle_normals = Matrix3xX::from_vec(
                triangle_idx_filtered
                    .row_iter()
                    .flat_map(|tri_idx| {
                        let p1 = xyz_dat.row(tri_idx[0]);
                        let p2 = xyz_dat.row(tri_idx[1]);
                        let p3 = xyz_dat.row(tri_idx[2]);
                        let normal = ((p2 - p1).cross(&(p3 - p1))).normalize();
                        [normal[0], normal[1], normal[2]]
                    })
                    .collect_vec(),
            )
            .transpose();

            Ok(Self::TriangulatedSurface {
                triangle_idx: triangle_idx_filtered,
                xyz_dat,
                triangle_face_normals: triangle_normals,
            })
        }
    }
}

impl PlotData {
    /// Gets the minimum and maximum values of all axes
    /// # Returns
    /// This method returns a vector of tuples [`Vec<Option<(f64, f64)>>`], where the tuple contains (min, max) if these values could be determined and None if not
    #[must_use]
    pub fn get_axes_min_max_values(&self) -> Vec<Option<(f64, f64)>> {
        match self {
            Self::Dim2 { xy_data } => vec![
                get_min_max_filter_nonfinite(xy_data.column(0).into()),
                get_min_max_filter_nonfinite(xy_data.column(1).into()),
            ],
            Self::Dim3 { xyz_data: xyz_dat } | Self::TriangulatedSurface { xyz_dat, .. } => vec![
                get_min_max_filter_nonfinite(xyz_dat.column(0).into()),
                get_min_max_filter_nonfinite(xyz_dat.column(1).into()),
                get_min_max_filter_nonfinite(xyz_dat.column(2).into()),
            ],
            Self::ColorMesh {
                x_dat_n,
                y_dat_m,
                z_dat_nxm,
            } => {
                let z_flat =
                    DVector::from_vec(z_dat_nxm.into_iter().copied().collect::<Vec<f64>>());
                vec![
                    get_min_max_filter_nonfinite(DVectorSlice::from(x_dat_n).into()),
                    get_min_max_filter_nonfinite(DVectorSlice::from(y_dat_m).into()),
                    get_min_max_filter_nonfinite(z_flat.column(0).into()),
                ]
            }
            Self::MultiDim3 { vec_of_xyz_data } => {
                let num_cols = vec_of_xyz_data[0].row(0).len();
                let mut max = vec![f64::NEG_INFINITY; num_cols];
                let mut min = vec![f64::INFINITY; num_cols];
                for d in vec_of_xyz_data {
                    for col in 0..num_cols {
                        if let Some((ax_min, ax_max)) =
                            get_min_max_filter_nonfinite(d.column(col).into())
                        {
                            min[col] = min[col].min(ax_min);
                            max[col] = max[col].max(ax_max);
                        }
                    }
                }

                let mut ax_lim_vec = Vec::<Option<(f64, f64)>>::with_capacity(num_cols);
                for col in 0..num_cols {
                    if !min[col].is_finite() || !max[col].is_finite() {
                        ax_lim_vec.push(None);
                    } else {
                        ax_lim_vec.push(Some((min[col], max[col])));
                    }
                }
                ax_lim_vec
            }

            Self::MultiDim2 { vec_of_xy_data } => {
                let num_cols = vec_of_xy_data[0].row(0).len();
                let mut max = vec![f64::NEG_INFINITY; num_cols];
                let mut min = vec![f64::INFINITY; num_cols];
                for d in vec_of_xy_data {
                    for col in 0..num_cols {
                        if let Some((ax_min, ax_max)) =
                            get_min_max_filter_nonfinite(d.column(col).into())
                        {
                            min[col] = min[col].min(ax_min);
                            max[col] = max[col].max(ax_max);
                        }
                    }
                }

                let mut ax_lim_vec = Vec::<Option<(f64, f64)>>::with_capacity(num_cols);
                for col in 0..num_cols {
                    if !min[col].is_finite() || !max[col].is_finite() {
                        ax_lim_vec.push(None);
                    } else {
                        ax_lim_vec.push(Some((min[col], max[col])));
                    }
                }
                ax_lim_vec
            }
        }
    }

    /// Defines the plot-axes bounds of this [`PlotData`].
    /// # Attributes
    /// - `expand_flag`: true if the ax bounds should expand by +- 10%, such that the data is not on the edge of the plot. false for no expansion
    /// # Returns
    /// This function returns a Vector of optional [`AxLims`]
    /// # Panics
    /// This function panics if the `expand_lims` function fails. As this only happens for a non-normal number this cannnot happen here.
    #[must_use]
    fn define_data_based_axes_bounds(&self, expand_flag: bool) -> PlotBounds {
        let ax_min_max_vals = self.get_axes_min_max_values();
        let mut axlims = Vec::<Option<AxLims>>::with_capacity(ax_min_max_vals.len());
        //check if the limits are useful for visualization
        for min_max_vals_opt in &ax_min_max_vals {
            if let Some((min, max)) = min_max_vals_opt {
                axlims.push(AxLims::create_useful_axlims(*min, *max));
            } else {
                axlims.push(AxLims::new(0., 1.));
            }
        }

        if expand_flag {
            for axlim in axlims.iter_mut().flatten() {
                axlim.expand_lim_range_by_factor(1.1);
            }
        };

        let mut axlim_opt = [None; 3];
        for (i, lim) in axlims.iter().enumerate() {
            axlim_opt[i] = *lim;
        }

        PlotBounds::new(axlim_opt[0], axlim_opt[1], axlim_opt[2])
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

impl TryFrom<Option<(f64, f64)>> for AxLims {
    type Error = OpossumError;

    fn try_from(value: Option<(f64, f64)>) -> OpmResult<Self> {
        if let Some((min, max)) = value {
            Self::new(min, max).map_or_else(
                || {
                    Err(OpossumError::Other(format!(
                        "Cannot create AxLim from values (min:{min}, max:{max})"
                    )))
                },
                Ok,
            )
        } else {
            Err(OpossumError::Other("Cannot create AxLim from None".into()))
        }
    }
}

impl AxLims {
    ///Creates a new [`AxLims`] struct
    /// # Attributes
    /// -`min`: minimum value of the ax limit
    /// -`max`: maximum value of the ax limit
    ///
    /// # Returns
    /// This function retuns Some([`AxLims`]) or None if the chosen minimum or maximum valus is NaN or infinite
    #[must_use]
    pub fn new(min: f64, max: f64) -> Option<Self> {
        let axlim = Self { min, max };
        if axlim.check_validity() {
            Some(axlim)
        } else {
            warn!("Invalid ax limit! Must be finite, not NaN, not equal and min must be smaller than max! AxLimit is set to None!");
            None
        }
    }

    ///Creates a new [`AxLims`] struct from a provided `DVector` filtering out all non finite values
    /// # Attributes
    /// -`dat_vec`: data vector
    ///
    /// # Returns
    /// This function retuns Some([`AxLims`]) or None if non of the dvector entries is finite
    #[must_use]
    pub fn finite_from_dvector(dat_vec: &DVectorSlice<'_, f64>) -> Option<Self> {
        let filtered_data = DVector::from_vec(filter_nan_infinite(dat_vec.as_slice()));
        if filtered_data.len() < 2 {
            warn!("Length of input data after filtering out non-finite values is below 2! Useful Axlims cannot be returned! AxLimit is set to None!");
            None
        } else {
            let axlim = Self {
                min: filtered_data.min(),
                max: filtered_data.max(),
            };
            if axlim.check_validity() {
                Some(axlim)
            } else {
                warn!("Invalid ax limit! Must be finite, not NaN, not equal and min must be smaller than max! AxLimit is set to None!");
                None
            }
        }
    }

    /// Checks the validity of the delivered min and max values and returns a true if it is valid, false otherwise
    #[must_use]
    pub fn check_validity(self) -> bool {
        self.max.is_finite()
            && self.min.is_finite()
            && abs_diff_ne!(self.max, self.min)
            && self.max > self.min
    }

    /// Shifts the minimum and the maximum to lower and higher values, respectively.
    /// The range expands by the `expansion_factor`, therefore, each limit is shifted by `range` * (`expansion_ratio`-1.)/2.
    /// # Attributes
    /// -`ratio`: relative extension of the range. must be positive, non-zero, not NAN and finite
    /// # Errors
    /// This function errors if the expansion ration is neither positive nor normal
    pub fn expand_lim_range_by_factor(&mut self, expansion_factor: f64) {
        if expansion_factor.is_normal() && expansion_factor.is_sign_positive() {
            let range = self.max - self.min;
            self.max += range * (expansion_factor - 1.) / 2.;
            self.min -= range * (expansion_factor - 1.) / 2.;
        } else {
            warn!("Cannot expand ax limits! Expansion factor must be normal and positive!");
        }
    }

    /// This function creates an [`AxLims`] struct from the provided `min` and `max` values
    /// # Attributes
    /// - `min`: minimum value for the ax limit
    /// - `max`: maximum value for the ax limit
    /// # Returns
    /// If the minimum and maximum value are chosen such that min < max and both finite, the function returns Some([`AxLims`]) with these limits
    /// If these criteria are not fulfilled, the values are changed accordingly to provide valid axlims. If for some reason, these values are still no okay, teh function returns None
    #[must_use]
    pub fn create_useful_axlims(min_in: f64, max_in: f64) -> Option<Self> {
        if !min_in.is_finite() && !max_in.is_finite() {
            return Self::new(-0.5, 0.5);
        }

        let (min, max) = if !min_in.is_finite() {
            (max_in, max_in)
        } else if !max_in.is_finite() {
            (min_in, min_in)
        } else {
            (min_in, max_in)
        };

        let (mut min, mut max) = if max < min { (max, min) } else { (min, max) };

        let mut ax_range = max - min;

        //check if minimum and maximum values are approximately equal. if so, take the max value as range
        if max.relative_eq(&min, f64::EPSILON, f64::EPSILON) {
            ax_range = max.abs();
            min = max - ax_range / 2.;
            max += ax_range * 0.5;
        };

        //check if for some reason maximum is 0, then set it to 1, so that the axis spans at least some distance
        if ax_range < f64::EPSILON {
            max = 0.5;
            min = -0.5;
        };

        Self::new(min, max)
    }

    /// Joins the minimum and maximum values of this [`AxLims`] struct with another [`AxLims`] struct, such that the maximum and minimum of both structs are used
    /// # Attributes
    /// - `ax_lim`: [`AxLims`] struct to integrate
    pub fn join(&mut self, ax_lim: Self) {
        if self.min > ax_lim.min {
            self.min = ax_lim.min;
        }

        if self.max < ax_lim.max {
            self.max = ax_lim.max;
        }
    }

    /// Joins the minimum and maximum values of this [`AxLims`] struct with an [`AxLims`] struct Option, such that the maximum and minimum of both structs are used
    /// Convenience function to join plotbounds
    /// # Attributes
    /// - `ax_lim_opt`: [`AxLims`] struct Option to join
    pub fn join_opt(&mut self, ax_lim_opt: Option<Self>) {
        if let Some(ax_lim) = ax_lim_opt {
            self.join(ax_lim);
        }
    }
}

/// Trait for adding the possibility to generate a (x/y) plot of an element.
pub trait Plottable {
    /// This method must be implemented in order to retrieve the plot data, plot color and data label.
    /// As the plot data may differ, the implementation must be done for each kind of plot type [`PlotType`]
    /// # Attributes
    /// - `plt_type`: plot type to be used. See [`PlotType`]
    /// # Returns
    /// This method returns an [`OpmResult<Option<PlotSeries>>`]. Whether Some(PlotData) or None is returned depends on the individual implementation
    /// # Errors
    /// Whether an error is thrown depends on the individual implementation of the method
    fn get_plot_series(&self, plt_type: &PlotType) -> OpmResult<Option<Vec<PlotSeries>>>;

    /// This method handles the plot creation for a specific data type or node type
    /// # Attributes
    /// - `f_path`: path to the file
    /// - `img_size`: the size of the image in pixels: (width, height)
    /// - `backend`: used backend to create the plot. See [`PltBackEnd`]
    /// # Errors
    /// Whether an error is thrown depends on the individual implementation of the method
    fn to_plot(&self, f_path: &Path, backend: PltBackEnd) -> OpmResult<Option<RgbImage>> {
        let mut plt_params = PlotParameters::default();
        if backend == PltBackEnd::BMP || backend == PltBackEnd::SVG {
            plt_params
                .set(&PlotArgs::FName(
                    f_path.file_name().unwrap().to_str().unwrap().to_owned(),
                ))?
                .set(&PlotArgs::FDir(f_path.parent().unwrap().into()))?;
        };

        plt_params.set(&PlotArgs::Backend(backend))?;

        let _ = self.add_plot_specific_params(&mut plt_params);

        let plt_type = self.get_plot_type(&plt_params);

        let mut plt_series_opt = self.get_plot_series(&plt_type)?;

        if let Some(plt_series) = &mut plt_series_opt {
            if plt_series.len() == 1 {
                plt_series[0].color = RGBAColor(255, 0, 0, 0.8);
            }
        }
        plt_series_opt.map_or(Ok(None), |plt_series| plt_type.plot(&plt_series))
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
}

///Enum to describe which type of plotting backend should be used
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum PltBackEnd {
    /// `BitmapBackend`. Used to create .png, .bmp, .jpg
    #[default]
    BMP,
    /// `SVGBackend`. Used to create .svg
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
        txt.clone_into(&mut self.label);
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
    /// Creates a new [`ColorBar`] struct with default values
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
#[derive(Clone, Default)]
pub struct PlotBounds {
    x: Option<AxLims>,
    y: Option<AxLims>,
    z: Option<AxLims>,
}

impl PlotBounds {
    /// Creates a new [`PlotBounds`] struct
    #[must_use]
    pub const fn new(x: Option<AxLims>, y: Option<AxLims>, z: Option<AxLims>) -> Self {
        Self { x, y, z }
    }

    /// Joins another [`PlotBounds`] struct into the existing one by setting the corresponding minimum or maximum values of the axis to the new max or new min
    /// # Attributes
    /// - `plot_bounds`: [`PlotBounds`] struct to integrate
    pub fn join(&mut self, plot_bounds: &Self) {
        if let Some(x_bounds) = &mut self.x {
            x_bounds.join_opt(plot_bounds.x);
        } else {
            self.x = plot_bounds.x;
        }
        if let Some(y_bounds) = &mut self.y {
            y_bounds.join_opt(plot_bounds.y);
        } else {
            self.y = plot_bounds.y;
        }
        if let Some(z_bounds) = &mut self.z {
            z_bounds.join_opt(plot_bounds.z);
        } else {
            self.z = plot_bounds.z;
        }
    }

    /// Returns the x boundary values of these [`PlotBounds`]
    #[must_use]
    pub const fn get_x_bounds(&self) -> Option<AxLims> {
        self.x
    }

    /// Returns the x boundary range of these [`PlotBounds`]
    #[must_use]
    pub fn get_x_range(&self) -> Option<f64> {
        self.x.map(|x| x.max - x.min)
    }

    /// Returns the y boundary values of these [`PlotBounds`]
    #[must_use]
    pub const fn get_y_bounds(&self) -> Option<AxLims> {
        self.y
    }

    /// Returns the y boundary range of these [`PlotBounds`]
    #[must_use]
    pub fn get_y_range(&self) -> Option<f64> {
        self.y.map(|y| y.max - y.min)
    }

    /// Returns the z boundary values of these [`PlotBounds`]
    #[must_use]
    pub const fn get_z_bounds(&self) -> Option<AxLims> {
        self.z
    }

    /// Returns the z boundary range of these [`PlotBounds`]
    #[must_use]
    pub fn get_z_range(&self) -> Option<f64> {
        self.z.map(|z| z.max - z.min)
    }
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
    /// - `PlotArgs::AxisEqual`: `true`
    /// - `PlotArgs::ExpandBounds`: `true`
    /// - `PlotArgs::CMap`: `colorous::TURBO`
    /// - `PlotArgs::Color`: `RGBAColor(255, 0, 0, 1.)`
    /// - `PlotArgs::FDir`: `current directory`
    /// - `PlotArgs::FName`: `opossum_default_plot_{i}.png`. Here, i is chosen such that no file is overwritten, but a new file is generated
    /// - `PlotArgs::PlotSize`: `(800, 800)`
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
                PlotArgs::AxisEqual(_) => plt_params.set(&PlotArgs::AxisEqual(true)).unwrap(),
                PlotArgs::ExpandBounds(_) => plt_params.set(&PlotArgs::ExpandBounds(true)).unwrap(),
                PlotArgs::CMap(_) => plt_params
                    .set(&PlotArgs::CMap(CGradient::default()))
                    .unwrap(),
                PlotArgs::FName(_) => {
                    let (_, fname) = Self::create_unused_filename_dir();

                    plt_params.set(&PlotArgs::FName(fname)).unwrap()
                }
                PlotArgs::PlotSize(_) => plt_params.set(&PlotArgs::PlotSize((800, 800))).unwrap(),
                PlotArgs::FDir(_) => {
                    let (current_dir, _) = Self::create_unused_filename_dir();

                    plt_params.set(&PlotArgs::FDir(current_dir)).unwrap()
                }
                PlotArgs::ViewDirection3D(_) => plt_params
                    .set(&PlotArgs::ViewDirection3D(Vector3::new(-1., -1., -1.)))
                    .unwrap(),
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
    pub fn get_3d_view(&self) -> OpmResult<Vector3<f64>> {
        if let Some(PlotArgs::ViewDirection3D(view_vec)) = self.params.get("view3d") {
            Ok(*view_vec)
        } else {
            Err(OpossumError::Other("view3d argument not found!".into()))
        }
    }

    ///This method gets the 3d view of a 3d plot which is stored in the [`PlotParameters`]
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
    pub fn get_fpath(&self) -> OpmResult<PathBuf> {
        let fdir = self.get_fdir()?;
        let fname = self.get_fname()?;

        Ok(fdir.join(fname))
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

    ///This method gets the flag which defines whether the plot axis should have the same range
    /// # Returns
    /// This method returns an [`OpmResult<bool>`] with the min and max of the z values as f64
    /// # Errors
    /// This method throws an error if the argument is not found
    pub fn get_axis_equal_flag(&self) -> OpmResult<bool> {
        if let Some(PlotArgs::AxisEqual(equal)) = self.params.get("axisequal") {
            Ok(*equal)
        } else {
            Err(OpossumError::Other("axisequal argument not found!".into()))
        }
    }

    ///This method gets the flag which defines whether the plot axis should expand, such that the values do not lie on the boundary
    /// # Returns
    /// This method returns an [`OpmResult<bool>`] with the min and max of the z values as f64
    /// # Errors
    /// This method throws an error if the argument is not found
    pub fn get_expand_bounds_flag(&self) -> OpmResult<bool> {
        if let Some(PlotArgs::ExpandBounds(expand)) = self.params.get("expandbounds") {
            Ok(*expand)
        } else {
            Err(OpossumError::Other(
                "expandbounds argument not found!".into(),
            ))
        }
    }

    ///This method gets the image size which is stored in the [`PlotParameters`]
    /// # Returns
    /// This method returns an [`OpmResult<(u32, u32)>`] with the width and height in number of pixels as u32 of the actual plot area
    /// # Errors
    /// This method throws an error if the argument is not found
    pub fn get_plotsize(&self) -> OpmResult<(u32, u32)> {
        if let Some(PlotArgs::PlotSize(plotsize)) = self.params.get("plotsize") {
            Ok(*plotsize)
        } else {
            Err(OpossumError::Other("plotsize argument not found!".into()))
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
            PlotArgs::PlotSize(plotsize) => !(plotsize.0 == 0 || plotsize.1 == 0),
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
            PlotArgs::CMap(_) => "cmap".to_owned(),
            PlotArgs::XLim(_) => "xlim".to_owned(),
            PlotArgs::YLim(_) => "ylim".to_owned(),
            PlotArgs::ZLim(_) => "zlim".to_owned(),
            PlotArgs::AxisEqual(_) => "axisequal".to_owned(),
            PlotArgs::ExpandBounds(_) => "expandbounds".to_owned(),
            PlotArgs::PlotSize(_) => "plotsize".to_owned(),
            PlotArgs::CBarLabelPos(_) => "cbarlabelpos".to_owned(),
            PlotArgs::CBarLabel(_) => "cbarlabel".to_owned(),
            PlotArgs::FDir(_) => "fdir".to_owned(),
            PlotArgs::FName(_) => "fname".to_owned(),
            PlotArgs::Backend(_) => "backend".to_owned(),
            PlotArgs::ViewDirection3D(_) => "view3d".to_owned(),
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
            PlotArgs::CMap(_) => self.params.insert("cmap".to_owned(), plt_arg.clone()),
            PlotArgs::XLim(_) => self.params.insert("xlim".to_owned(), plt_arg.clone()),
            PlotArgs::YLim(_) => self.params.insert("ylim".to_owned(), plt_arg.clone()),
            PlotArgs::ZLim(_) => self.params.insert("zlim".to_owned(), plt_arg.clone()),
            PlotArgs::AxisEqual(_) => self.params.insert("axisequal".to_owned(), plt_arg.clone()),
            PlotArgs::ExpandBounds(_) => self
                .params
                .insert("expandbounds".to_owned(), plt_arg.clone()),
            PlotArgs::PlotSize(_) => self.params.insert("plotsize".to_owned(), plt_arg.clone()),
            PlotArgs::CBarLabelPos(_) => self
                .params
                .insert("cbarlabelpos".to_owned(), plt_arg.clone()),
            PlotArgs::CBarLabel(_) => self.params.insert("cbarlabel".to_owned(), plt_arg.clone()),
            PlotArgs::FDir(_) => self.params.insert("fdir".to_owned(), plt_arg.clone()),
            PlotArgs::FName(_) => self.params.insert("fname".to_owned(), plt_arg.clone()),
            PlotArgs::Backend(_) => self.params.insert("backend".to_owned(), plt_arg.clone()),
            PlotArgs::ViewDirection3D(_) => {
                self.params.insert("view3d".to_owned(), plt_arg.clone())
            }
        };
    }
}

/// Struct that holds all necessary attributes to describe a plot series
#[derive(Clone)]
pub struct PlotSeries {
    data: PlotData,
    color: RGBAColor,
    series_label: Option<String>,
}

impl PlotSeries {
    #[must_use]
    /// creates a new [`PlotSeries`]
    /// # Attributes
    /// - `data`: reference to a [`PlotData`] enum variant
    /// - `color`: [`RGBAColor`] of the series
    /// - `series_label`: optional label of this [`PlotSeries`]. Will be shown in a legend, if provided
    /// # Returns
    /// This function returns a [`PlotSeries`] struct
    pub fn new(data: &PlotData, color: RGBAColor, series_label: Option<String>) -> Self {
        Self {
            data: data.clone(),
            color,
            series_label,
        }
    }

    /// Sets the color of this [`PlotSeries`]
    pub fn set_series_color(&mut self, color: RGBAColor) {
        self.color = color;
    }
    /// gets the color of this [`PlotSeries`]
    #[must_use]
    pub const fn get_series_color(&self) -> &RGBAColor {
        &self.color
    }

    /// Sets the series label of this [`PlotSeries`]
    pub fn set_series_label(&mut self, label: String) {
        self.series_label = Some(label);
    }
    /// gets the series label of this [`PlotSeries`]
    #[must_use]
    pub fn get_series_label(&self) -> Option<String> {
        self.series_label.clone()
    }

    /// Sets the data of this [`PlotSeries`]
    pub fn set_plot_series_data(&mut self, data: &PlotData) {
        self.data = data.clone();
    }
    /// gets the data of this [`PlotSeries`]
    #[must_use]
    pub const fn get_plot_series_data(&self) -> &PlotData {
        &self.data
    }

    /// defines the axis bouns of this [`PlotSeries`].
    /// Basically just wraps the same function for the plot data
    #[must_use]
    pub fn define_data_based_axes_bounds(&self, expand_flag: bool) -> PlotBounds {
        self.get_plot_series_data()
            .define_data_based_axes_bounds(expand_flag)
    }
}

/// Struct that holds all necessary attributes to create a plot, such as [`PlotData`], [`PlotBounds`] etc
#[derive(Clone)]
pub struct Plot {
    label: [LabelDescription; 2],
    cbar: ColorBar,
    bounds: PlotBounds,
    ax_equal: bool,
    expand_bounds: bool,
    plot_size: (u32, u32),
    fig_size: (u32, u32),
    plot_series: Option<Vec<PlotSeries>>,
    _view_3d: Vector3<f64>,
}

impl Plot {
    /// creates a new [`Plot`]
    /// # Attributes
    /// - `plt_series`: reference to a [`PlotSeries`]
    /// - `plt_params`: reference to [`PlotParameters`]
    /// # Returns
    /// This function returns a [`Plot`] struct
    /// # Panics
    /// This method panics if the [`Plot`] can not be created from [`PlotParameters`]
    #[must_use]
    pub fn new(plt_series: &Vec<PlotSeries>, plt_params: &PlotParameters) -> Self {
        let mut plot = Self::try_from(plt_params).unwrap();
        plot.add_plot_series(plt_series, false);

        plot
    }

    fn add_margin_to_figure_size(&mut self, plt_type: &PlotType) {
        let height_add: u32 = 65 + 70;
        let mut width_add: u32 = 0;

        let mut add_left = 10;
        let mut add_right = 30;
        let pixel_margin = PlotType::calc_pixel_margin(self.bounds.y.unwrap_or_else(|| AxLims {
            min: -0.5,
            max: 0.5,
        }));

        if LabelPos::Right == self.label[1].label_pos {
            add_right += 21 + pixel_margin;
            if pixel_margin < 72 {
                add_right = 82 - pixel_margin;
            }
        }
        if LabelPos::Left == self.label[1].label_pos {
            add_left += 21 + pixel_margin;
            if pixel_margin < 72 {
                add_left = 82 - pixel_margin;
            }
        }

        width_add += add_right + add_left;

        if let PlotType::ColorMesh(_) = plt_type {
            width_add += 170;
        }

        self.fig_size.0 += width_add;
        self.fig_size.1 += height_add;
    }

    /// Adds another [`PlotSeries`] to the [`Plot`] struct
    /// # Attributes
    /// - `plt_series_vec`: Vector of [`PlotSeries`] structs that should be added
    /// - `define_bounds`: flag to define if the plot boundaries should be updated according to the new plots series data. true to re-evaluate, false otherwise
    pub fn add_plot_series(&mut self, plt_series_vec: &Vec<PlotSeries>, join_bounds: bool) {
        if let Some(stored_plt_series) = &mut self.plot_series {
            for plt_series in plt_series_vec {
                stored_plt_series.push(plt_series.clone());
            }
        } else {
            self.plot_series = Some(plt_series_vec.clone());
        }

        let mut bounds = PlotBounds::default();
        for plt_series in plt_series_vec {
            bounds.join(&plt_series.define_data_based_axes_bounds(self.expand_bounds));
        }
        if join_bounds {
            self.bounds = bounds;
        } else {
            if self.bounds.get_x_bounds().is_none() {
                self.bounds.x = bounds.get_x_bounds();
            }
            if self.bounds.get_y_bounds().is_none() {
                self.bounds.y = bounds.get_y_bounds();
            }
            if self.bounds.get_z_bounds().is_none() {
                self.bounds.z = bounds.get_z_bounds();
            }
        }
    }

    /// Returns a reference to the vector of [`PlotSeries`] of this [`Plot`]
    #[must_use]
    pub const fn get_plot_series_vec(&self) -> Option<&Vec<PlotSeries>> {
        self.plot_series.as_ref()
    }

    fn set_xy_axes_ranges_equal(&mut self) {
        let (plot_width, plot_height) = self.plot_size;
        let plot_ratio_w_h = plot_width.to_f64().unwrap() / plot_height.to_f64().unwrap();

        let x_range = self.bounds.get_x_range();
        let y_range = self.bounds.get_y_range();
        if let (Some(x_range), Some(y_range), Some(x_bounds), Some(y_bounds)) =
            (x_range, y_range, &mut self.bounds.x, &mut self.bounds.y)
        {
            if x_range > y_range {
                y_bounds.expand_lim_range_by_factor(x_range / y_range);
            } else {
                x_bounds.expand_lim_range_by_factor(y_range / x_range);
            }
            if plot_width > plot_height {
                x_bounds.expand_lim_range_by_factor(plot_ratio_w_h);
            } else {
                y_bounds.expand_lim_range_by_factor(1. / plot_ratio_w_h);
            }
        } else if x_range.is_some() {
            self.bounds.y = self.bounds.x;
        } else if y_range.is_some() {
            self.bounds.x = self.bounds.y;
        }
    }
    /// Defines the axes bounds of this [`Plot`] if the limit is not already defined by the initial [`PlotParameters`].
    ///
    /// # Errors
    /// - if `define_data_based_axes_bounds` fails
    ///
    /// # Warns
    /// - if `plot_series` is empty
    /// - if `self.plot_series` is None
    pub fn define_axes_bounds(&mut self) {
        if let Some(plot_series) = &self.plot_series {
            if plot_series.is_empty() {
                warn!("No plot series defined! Cannot define axes bounds!");
            } else {
                let mut plt_bounds_series = PlotBounds::default();
                for plt_series in plot_series {
                    plt_bounds_series
                        .join(&plt_series.define_data_based_axes_bounds(self.expand_bounds));
                }
                if self.bounds.get_x_bounds().is_none() {
                    self.bounds.x = plt_bounds_series.get_x_bounds();
                }
                if self.bounds.get_y_bounds().is_none() {
                    self.bounds.y = plt_bounds_series.get_y_bounds();
                }
                if self.bounds.get_z_bounds().is_none() {
                    self.bounds.z = plt_bounds_series.get_z_bounds();
                }
                if self.ax_equal {
                    self.set_xy_axes_ranges_equal();
                }
            }
        } else {
            warn!("No plot series defined! Cannot define axes bounds!");
        }
    }
}

impl TryFrom<&PlotParameters> for Plot {
    type Error = OpossumError;
    fn try_from(plt_params: &PlotParameters) -> OpmResult<Self> {
        let cmap_gradient = plt_params.get_cmap()?;
        let cbar_label_str = plt_params.get_cbar_label()?;
        let cbar_label_pos = plt_params.get_cbar_label_pos()?;
        let plot_size = plt_params.get_plotsize()?;
        let x_lim = plt_params.get_xlim()?;
        let y_lim = plt_params.get_ylim()?;
        let z_lim = plt_params.get_zlim()?;
        let ax_equal = plt_params.get_axis_equal_flag()?;
        let expand_bounds = plt_params.get_expand_bounds_flag()?;
        let x_label_str = plt_params.get_x_label()?;
        let y_label_str = plt_params.get_y_label()?;
        let x_label_pos = plt_params.get_x_label_pos()?;
        let y_label_pos = plt_params.get_y_label_pos()?;
        let view_3d = plt_params.get_3d_view()?;

        let x_label = LabelDescription::new(&x_label_str, x_label_pos);
        let y_label = LabelDescription::new(&y_label_str, y_label_pos);
        let cbarlabel = LabelDescription::new(&cbar_label_str, cbar_label_pos);

        let cbar = ColorBar {
            cmap: cmap_gradient.get_gradient(),
            label: cbarlabel,
        };

        Ok(Self {
            label: [x_label, y_label],
            cbar,
            bounds: PlotBounds::new(x_lim, y_lim, z_lim),
            ax_equal,
            expand_bounds,
            plot_size,
            fig_size: plot_size,
            plot_series: None,
            _view_3d: view_3d,
        })
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
    ///defines wheter the axis bounds should be equal or not
    AxisEqual(bool),
    ///defines wheter the axis bounds should expand or not
    ExpandBounds(bool),
    ///image size in pixels. Holds an `(usize, usize)` tuple
    PlotSize((u32, u32)),
    ///Path to the save directory of the image. Only necessary if the data is not written into a buffer. Holds a String
    FDir(PathBuf),
    ///Name of the file to be written. Holds a String
    FName(String),
    ///Plotting backend that should be used. Holds a [`PltBackEnd`] enum
    Backend(PltBackEnd),
    ///Vector of the viewpoint for a 3d plot
    ViewDirection3D(Vector3<f64>),
}

#[cfg(test)]
mod test {
    use super::*;
    use approx::{assert_relative_eq, relative_eq};
    use log::Level;
    use tempfile::NamedTempFile;
    #[test]
    fn add_plot_series() {
        let mut plt = Plot::try_from(&PlotParameters::default()).unwrap();
        let data1 = &PlotData::Dim2 {
            xy_data: MatrixXx2::from_vec(vec![0., 1., 2., 3., 4., 5.]),
        };
        let data2 = &PlotData::Dim2 {
            xy_data: MatrixXx2::from_vec(vec![4., 5., 6., -10., 8., 10.]),
        };
        let data3 = &PlotData::Dim2 {
            xy_data: MatrixXx2::from_vec(vec![4., 5., 6., -1., 8., 9.]),
        };
        let plt_series1 = PlotSeries::new(
            data1,
            RGBAColor(255, 0, 0, 1.),
            Some("series_label1".to_owned()),
        );
        let plt_series2 = PlotSeries::new(data2, RGBAColor(0, 255, 0, 1.), None);
        let plt_series3 = PlotSeries::new(data3, RGBAColor(0, 0, 255, 1.), None);

        plt.add_plot_series(&vec![plt_series1], true);

        let x_bounds = plt.bounds.get_x_bounds().unwrap();
        let y_bounds = plt.bounds.get_y_bounds().unwrap();
        let z_bounds = plt.bounds.get_z_bounds();

        assert_relative_eq!(x_bounds.min, -0.1);
        assert_relative_eq!(x_bounds.max, 2.1);
        assert_relative_eq!(y_bounds.min, 2.9);
        assert_relative_eq!(y_bounds.max, 5.1);

        assert!(z_bounds.is_none());

        let plt_series_vec = plt.get_plot_series_vec();
        assert!(plt_series_vec.is_some());

        let plt_series_vec = plt_series_vec.unwrap();
        if let PlotData::Dim2 { xy_data } = &plt_series_vec[0].data {
            let datx = DVector::from_vec(vec![0., 1., 2.]);
            assert_relative_eq!(xy_data.column(0), DVectorSlice::from(datx.as_slice()));
            let daty = DVector::from_vec(vec![3., 4., 5.]);
            assert_relative_eq!(xy_data.column(1), DVectorSlice::from(daty.as_slice()));
        }

        assert_eq!(
            RGBAColor(255, 0, 0, 1.),
            *plt_series_vec[0].get_series_color()
        );
        assert_eq!(
            "series_label1".to_owned(),
            plt_series_vec[0].get_series_label().unwrap()
        );

        plt.add_plot_series(&vec![plt_series2], false);
        let x_bounds = plt.bounds.get_x_bounds().unwrap();
        let y_bounds = plt.bounds.get_y_bounds().unwrap();

        assert_relative_eq!(x_bounds.min, -0.1);
        assert_relative_eq!(x_bounds.max, 2.1);
        assert_relative_eq!(y_bounds.min, 2.9);
        assert_relative_eq!(y_bounds.max, 5.1);

        plt.add_plot_series(&vec![plt_series3], true);
        let x_bounds = plt.bounds.get_x_bounds().unwrap();
        let y_bounds = plt.bounds.get_y_bounds().unwrap();

        assert_relative_eq!(x_bounds.min, 3.9);
        assert_relative_eq!(x_bounds.max, 6.1);
        assert!(relative_eq!(
            y_bounds.min,
            -1.5,
            max_relative = 2. * f64::EPSILON
        ));
        assert_relative_eq!(y_bounds.max, 9.5);

        let plt_series_vec = plt.get_plot_series_vec().unwrap();

        assert_eq!(
            RGBAColor(0, 255, 0, 1.),
            *plt_series_vec[1].get_series_color()
        );
        assert!(plt_series_vec[1].get_series_label().is_none());

        assert_eq!(
            RGBAColor(0, 0, 255, 1.),
            *plt_series_vec[2].get_series_color()
        );
        assert!(plt_series_vec[2].get_series_label().is_none());
    }
    #[test]
    fn join_axlims() {
        let mut axlim1 = AxLims::new(0., 1.).unwrap();
        let axlim2 = AxLims::new(-1., 2.).unwrap();
        axlim1.join(axlim2);
        assert_relative_eq!(axlim1.min, -1.);
        assert_relative_eq!(axlim1.max, 2.);

        let mut axlim1 = AxLims::new(0., 1.).unwrap();
        let axlim2 = AxLims::new(0., 2.).unwrap();
        axlim1.join(axlim2);
        assert_relative_eq!(axlim1.min, 0.);
        assert_relative_eq!(axlim1.max, 2.);

        let mut axlim1 = AxLims::new(0., 1.).unwrap();
        let axlim2 = AxLims::new(-10., 1.).unwrap();
        axlim1.join(axlim2);
        assert_relative_eq!(axlim1.min, -10.);
        assert_relative_eq!(axlim1.max, 1.);
    }
    #[test]
    fn join_opt_axlims() {
        let mut axlim1 = AxLims::new(0., 1.).unwrap();
        let axlim2 = AxLims::new(-1., 2.);
        axlim1.join_opt(axlim2);
        assert_relative_eq!(axlim1.min, -1.);
        assert_relative_eq!(axlim1.max, 2.);

        let mut axlim1 = AxLims::new(0., 1.).unwrap();
        let axlim2 = AxLims::new(0., 2.);
        axlim1.join_opt(axlim2);
        assert_relative_eq!(axlim1.min, 0.);
        assert_relative_eq!(axlim1.max, 2.);

        let mut axlim1 = AxLims::new(0., 1.).unwrap();
        let axlim2 = AxLims::new(-10., 1.);
        axlim1.join_opt(axlim2);
        assert_relative_eq!(axlim1.min, -10.);
        assert_relative_eq!(axlim1.max, 1.);
    }
    #[test]
    fn get_x_bounds() {
        assert_eq!(PlotBounds::new(None, None, None).get_x_bounds(), None);
        let x = PlotBounds::new(AxLims::new(0., 1.), None, None).get_x_bounds();
        assert!(x.is_some());
        assert_relative_eq!(x.unwrap().min, 0.);
        assert_relative_eq!(x.unwrap().max, 1.);
    }
    #[test]
    fn get_y_bounds() {
        assert_eq!(PlotBounds::new(None, None, None).get_y_bounds(), None);
        let y = PlotBounds::new(None, AxLims::new(0., 1.), None).get_y_bounds();
        assert!(y.is_some());
        assert_relative_eq!(y.unwrap().min, 0.);
        assert_relative_eq!(y.unwrap().max, 1.);
    }
    #[test]
    fn get_z_bounds() {
        assert_eq!(PlotBounds::new(None, None, None).get_z_bounds(), None);
        let z = PlotBounds::new(None, None, AxLims::new(0., 1.)).get_z_bounds();
        assert!(z.is_some());
        assert_relative_eq!(z.unwrap().min, 0.);
        assert_relative_eq!(z.unwrap().max, 1.);
    }
    #[test]
    fn get_x_range() {
        assert_eq!(PlotBounds::new(None, None, None).get_x_range(), None);
        let x = PlotBounds::new(AxLims::new(0., 1.), None, None).get_x_range();
        assert!(x.is_some());
        assert_relative_eq!(x.unwrap(), 1.);
    }
    #[test]
    fn get_y_range() {
        assert_eq!(PlotBounds::new(None, None, None).get_y_range(), None);
        let y = PlotBounds::new(None, AxLims::new(0., 1.), None).get_y_range();
        assert!(y.is_some());
        assert_relative_eq!(y.unwrap(), 1.);
    }
    #[test]
    fn get_z_range() {
        assert_eq!(PlotBounds::new(None, None, None).get_z_range(), None);
        let z = PlotBounds::new(None, None, AxLims::new(0., 1.)).get_z_range();
        assert!(z.is_some());
        assert_relative_eq!(z.unwrap(), 1.);
    }
    #[test]
    fn new_plotbounds() {
        let _ = PlotBounds::new(None, None, None);
        let _ = PlotBounds::new(AxLims::new(0., 1.), None, None);
        let _ = PlotBounds::new(AxLims::new(0., f64::NEG_INFINITY), None, None);
        let _ = PlotBounds::new(AxLims::new(0., f64::INFINITY), None, None);
        let _ = PlotBounds::new(AxLims::new(0., f64::NAN), None, None);

        let _ = PlotBounds::new(None, AxLims::new(0., 1.), None);
        let _ = PlotBounds::new(None, AxLims::new(0., f64::NEG_INFINITY), None);
        let _ = PlotBounds::new(None, AxLims::new(0., f64::INFINITY), None);
        let _ = PlotBounds::new(None, AxLims::new(0., f64::NAN), None);

        let _ = PlotBounds::new(None, None, AxLims::new(0., 1.));
        let _ = PlotBounds::new(None, None, AxLims::new(0., f64::NEG_INFINITY));
        let _ = PlotBounds::new(None, None, AxLims::new(0., f64::INFINITY));
        let _ = PlotBounds::new(None, None, AxLims::new(0., f64::NAN));
    }

    #[test]
    fn default_plot_bounds() {
        let plt_bounds = PlotBounds::default();
        assert!(plt_bounds.x.is_none());
        assert!(plt_bounds.y.is_none());
        assert!(plt_bounds.z.is_none());
    }
    #[test]
    fn join_plot_bounds_none() {
        let mut plt_bounds = PlotBounds::new(
            Some(AxLims::new(0., 1.).unwrap()),
            Some(AxLims::new(0., 1.).unwrap()),
            Some(AxLims::new(0., 1.).unwrap()),
        );
        let plt_bounds_default = PlotBounds::default();
        plt_bounds.join(&plt_bounds_default);

        assert_relative_eq!(plt_bounds.x.unwrap().min, 0.);
        assert_relative_eq!(plt_bounds.x.unwrap().max, 1.);
        assert_relative_eq!(plt_bounds.y.unwrap().min, 0.);
        assert_relative_eq!(plt_bounds.y.unwrap().max, 1.);
        assert_relative_eq!(plt_bounds.z.unwrap().min, 0.);
        assert_relative_eq!(plt_bounds.z.unwrap().max, 1.);
    }
    #[test]
    fn join_plot_bounds_larger_bounds() {
        let mut plt_bounds1 = PlotBounds::new(
            Some(AxLims::new(0., 1.).unwrap()),
            Some(AxLims::new(0., 1.).unwrap()),
            Some(AxLims::new(0., 1.).unwrap()),
        );
        let plt_bounds2 = PlotBounds::new(
            Some(AxLims::new(-1., 2.).unwrap()),
            Some(AxLims::new(-1., 2.).unwrap()),
            Some(AxLims::new(-1., 2.).unwrap()),
        );
        plt_bounds1.join(&plt_bounds2);

        assert_relative_eq!(plt_bounds1.x.unwrap().min, -1.);
        assert_relative_eq!(plt_bounds1.x.unwrap().max, 2.);
        assert_relative_eq!(plt_bounds1.y.unwrap().min, -1.);
        assert_relative_eq!(plt_bounds1.y.unwrap().max, 2.);
        assert_relative_eq!(plt_bounds1.z.unwrap().min, -1.);
        assert_relative_eq!(plt_bounds1.z.unwrap().max, 2.);
    }
    #[test]
    fn join_plot_bounds_smaller_bounds() {
        let mut plt_bounds1 = PlotBounds::new(
            Some(AxLims::new(0., 1.).unwrap()),
            Some(AxLims::new(0., 1.).unwrap()),
            Some(AxLims::new(0., 1.).unwrap()),
        );
        let plt_bounds2 = PlotBounds::new(
            Some(AxLims::new(0.5, 2.).unwrap()),
            Some(AxLims::new(0.5, 2.).unwrap()),
            Some(AxLims::new(0.5, 2.).unwrap()),
        );
        plt_bounds1.join(&plt_bounds2);

        assert_relative_eq!(plt_bounds1.x.unwrap().min, 0.);
        assert_relative_eq!(plt_bounds1.x.unwrap().max, 2.);
        assert_relative_eq!(plt_bounds1.y.unwrap().min, 0.);
        assert_relative_eq!(plt_bounds1.y.unwrap().max, 2.);
        assert_relative_eq!(plt_bounds1.z.unwrap().min, 0.);
        assert_relative_eq!(plt_bounds1.z.unwrap().max, 2.);
    }
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
        assert_eq!(plt_params.get_fdir().is_err(), true);
        assert_eq!(plt_params.get_fname().is_err(), true);
        assert_eq!(plt_params.get_cmap().is_err(), true);
        assert_eq!(plt_params.get_plotsize().is_err(), true);
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
        assert_eq!(
            format!("{:?}", plt_params.get_cmap().unwrap().get_gradient()),
            "Gradient(Turbo)".to_owned()
        );
        assert_eq!(plt_params.get_fdir().unwrap(), current_dir().unwrap());
        assert_eq!(
            plt_params.get_fname().unwrap(),
            format!("opossum_default_plot_0.png")
        );
        assert_eq!(plt_params.get_plotsize().unwrap(), (800, 800));
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
    fn plot_params_fpath() {
        let mut plt_params = PlotParameters::default();
        plt_params
            .set(&PlotArgs::FName("test_name.png".to_owned()))
            .unwrap();
        let path = plt_params
            .get_fdir()
            .unwrap()
            .join(plt_params.get_fname().unwrap());

        assert_eq!(plt_params.get_fpath().unwrap(), path);
    }
    #[test]
    fn get_plot_params() {
        let plt_params = PlotParameters::default();

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
        let mut plt_type = PlotType::ColorMesh(plt_params);

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
        assert!(plot.plot_series.is_none());
        assert_eq!(
            format!("{:?}", plot.cbar.cmap),
            format!("{:?}", plt_params.get_cmap().unwrap().get_gradient())
        );
        assert_eq!(plot.cbar.label.label, plt_params.get_cbar_label().unwrap());
        assert_eq!(
            plot.cbar.label.label_pos,
            plt_params.get_cbar_label_pos().unwrap()
        );
        assert_eq!(plot.plot_size, plt_params.get_plotsize().unwrap());
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
    fn check_plot_arg_validity_plotsize() {
        //already covered in other test, as here only a function is called which is already tested
        assert!(!PlotParameters::check_plot_arg_validity(
            &PlotArgs::PlotSize((0, 0))
        ));
        assert!(!PlotParameters::check_plot_arg_validity(
            &PlotArgs::PlotSize((0, 1))
        ));
        assert!(!PlotParameters::check_plot_arg_validity(
            &PlotArgs::PlotSize((1, 0))
        ));
        assert!(PlotParameters::check_plot_arg_validity(
            &PlotArgs::PlotSize((1, 1))
        ));
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
    fn new_plot() {
        let plt_params = PlotParameters::default();
        let x = linspace(0., 2., 3).unwrap();
        let y = linspace(3., 5., 3).unwrap();
        let plt_series_dim2 = PlotSeries::new(
            &PlotData::new_dim2(MatrixXx2::from_columns(&[x, y])).unwrap(),
            RGBAColor(0, 0, 0, 1.),
            None,
        );

        let plot = Plot::new(&vec![plt_series_dim2], &plt_params);
        assert!(plot.get_plot_series_vec().is_some());

        if let Some(vec) = plot.get_plot_series_vec() {
            if let PlotData::Dim2 { xy_data } = vec[0].get_plot_series_data() {
                assert!((xy_data[(0, 0)] - 0.).abs() < f64::EPSILON);
                assert!((xy_data[(0, 1)] - 3.).abs() < f64::EPSILON)
            }
        }
    }
    #[test]
    fn get_series_labels_test() {
        //define test data
        let x = linspace(0., 2., 3).unwrap();
        let y = DVector::from_vec(vec![2., 0., 1.7]);
        let z = linspace(2., 4., 3).unwrap();
        let z_mat = x.clone() * y.clone().transpose();

        let dat_2d = MatrixXx2::from_columns(&[x.clone(), y.clone()]);
        let dat_3d = MatrixXx3::from_columns(&[x.clone(), y.clone(), z.clone()]);

        //define PlotSeries
        let plt_series_dim2 = PlotSeries::new(
            &PlotData::new_dim2(dat_2d).unwrap(),
            RGBAColor(0, 0, 0, 1.),
            Some("dim2".to_owned()),
        );
        let plt_series_dim3 = PlotSeries::new(
            &PlotData::new_dim3(dat_3d.clone()).unwrap(),
            RGBAColor(0, 0, 0, 1.),
            Some("dim3".to_owned()),
        );
        let plt_series_colormesh = PlotSeries::new(
            &PlotData::new_colormesh(x.clone(), y.clone(), z_mat.clone()).unwrap(),
            RGBAColor(0, 0, 0, 1.),
            Some("colormesh".to_owned()),
        );
        let plt_series_surf_triangle = PlotSeries::new(
            &PlotData::new_triangulatedsurface(&dat_3d, None, None).unwrap(),
            RGBAColor(0, 0, 0, 1.),
            Some("tri_surf".to_owned()),
        );

        assert_eq!(
            plt_series_dim2.get_series_label().unwrap(),
            "dim2".to_owned()
        );
        assert_eq!(
            plt_series_dim3.get_series_label().unwrap(),
            "dim3".to_owned()
        );
        assert_eq!(
            plt_series_colormesh.get_series_label().unwrap(),
            "colormesh".to_owned()
        );
        assert_eq!(
            plt_series_surf_triangle.get_series_label().unwrap(),
            "tri_surf".to_owned()
        );
    }
    #[test]
    fn get_axes_min_max_values_test() {
        let x = linspace(0., 2., 3).unwrap();
        let y = DVector::from_vec(vec![2., 0., 1.7]);
        let z = linspace(2., 4., 3).unwrap();
        let z_mat = x.clone() * y.clone().transpose();

        let dat_2d = MatrixXx2::from_columns(&[x.clone(), y.clone()]);
        let dat_3d = MatrixXx3::from_columns(&[x.clone(), y.clone(), z.clone()]);

        let plt_dat_dim2 = PlotData::new_dim2(dat_2d).unwrap();
        let min_max = plt_dat_dim2.get_axes_min_max_values();
        assert_relative_eq!(min_max[0].unwrap().0, 0.0);
        assert_relative_eq!(min_max[0].unwrap().1, 2.0);
        assert_relative_eq!(min_max[1].unwrap().0, 0.0);
        assert_relative_eq!(min_max[1].unwrap().1, 2.0);
        assert_eq!(min_max.len(), 2);

        let plt_dat_dim3 = PlotData::new_dim3(dat_3d.clone()).unwrap();
        let min_max = plt_dat_dim3.get_axes_min_max_values();
        assert_relative_eq!(min_max[0].unwrap().0, 0.0);
        assert_relative_eq!(min_max[0].unwrap().1, 2.0);
        assert_relative_eq!(min_max[1].unwrap().0, 0.0);
        assert_relative_eq!(min_max[1].unwrap().1, 2.0);
        assert_relative_eq!(min_max[2].unwrap().0, 2.);
        assert_relative_eq!(min_max[2].unwrap().1, 4.);

        let plt_dat_colormesh =
            PlotData::new_colormesh(x.clone(), y.clone(), z_mat.clone()).unwrap();
        let min_max: Vec<Option<(f64, f64)>> = plt_dat_colormesh.get_axes_min_max_values();
        assert_relative_eq!(min_max[0].unwrap().0, 0.0);
        assert_relative_eq!(min_max[0].unwrap().1, 2.0);
        assert_relative_eq!(min_max[1].unwrap().0, 0.0);
        assert_relative_eq!(min_max[1].unwrap().1, 2.0);
        assert_relative_eq!(min_max[2].unwrap().0, -0.0);
        assert_relative_eq!(min_max[2].unwrap().1, 4.0);

        let plt_dat_surf_triangle = PlotData::new_triangulatedsurface(&dat_3d, None, None).unwrap();
        let min_max: Vec<Option<(f64, f64)>> = plt_dat_surf_triangle.get_axes_min_max_values();
        assert_relative_eq!(min_max[0].unwrap().0, 0.0);
        assert_relative_eq!(min_max[0].unwrap().1, 2.0);
        assert_relative_eq!(min_max[1].unwrap().0, 0.0);
        assert_relative_eq!(min_max[1].unwrap().1, 2.0);
        assert_relative_eq!(min_max[2].unwrap().0, 2.);
        assert_relative_eq!(min_max[2].unwrap().1, 4.);
    }
    #[test]
    fn define_data_based_axes_bounds_test() {
        let x = linspace(0., 2., 3).unwrap();
        let y = DVector::from_vec(vec![2., 0., 1.7]);
        let z = linspace(2., 4., 3).unwrap();
        let z_mat = x.clone() * y.clone().transpose();

        let dat_2d = MatrixXx2::from_columns(&[x.clone(), y.clone()]);
        let dat_3d = MatrixXx3::from_columns(&[x.clone(), y.clone(), z.clone()]);

        let plt_dat_dim2 = PlotData::new_dim2(dat_2d).unwrap();
        let axlims = plt_dat_dim2.define_data_based_axes_bounds(true);
        assert_relative_eq!(axlims.x.unwrap().min, -0.1);
        assert_relative_eq!(axlims.x.unwrap().max, 2.1);
        assert_relative_eq!(axlims.y.unwrap().min, -0.1);
        assert_relative_eq!(axlims.y.unwrap().max, 2.1);
        assert!(axlims.z.is_none());

        let plt_dat_dim3 = PlotData::new_dim3(dat_3d.clone()).unwrap();
        let axlims = plt_dat_dim3.define_data_based_axes_bounds(true);
        assert_relative_eq!(axlims.x.unwrap().min, -0.1);
        assert_relative_eq!(axlims.x.unwrap().max, 2.1);
        assert_relative_eq!(axlims.y.unwrap().min, -0.1);
        assert_relative_eq!(axlims.y.unwrap().max, 2.1);
        assert_relative_eq!(axlims.z.unwrap().min, 1.9);
        assert_relative_eq!(axlims.z.unwrap().max, 4.1);

        let plt_dat_colormesh =
            PlotData::new_colormesh(x.clone(), y.clone(), z_mat.clone()).unwrap();
        let axlims = plt_dat_colormesh.define_data_based_axes_bounds(true);
        assert_relative_eq!(axlims.x.unwrap().min, -0.1);
        assert_relative_eq!(axlims.x.unwrap().max, 2.1);
        assert_relative_eq!(axlims.y.unwrap().min, -0.1);
        assert_relative_eq!(axlims.y.unwrap().max, 2.1);
        assert_relative_eq!(axlims.z.unwrap().min, -0.2);
        assert_relative_eq!(axlims.z.unwrap().max, 4.2);

        let plt_dat_tri_surf = PlotData::new_triangulatedsurface(&dat_3d, None, None).unwrap();
        let axlims = plt_dat_tri_surf.define_data_based_axes_bounds(true);
        assert_relative_eq!(axlims.x.unwrap().min, -0.1);
        assert_relative_eq!(axlims.x.unwrap().max, 2.1);
        assert_relative_eq!(axlims.y.unwrap().min, -0.1);
        assert_relative_eq!(axlims.y.unwrap().max, 2.1);
        assert_relative_eq!(axlims.z.unwrap().min, 1.9);
        assert_relative_eq!(axlims.z.unwrap().max, 4.1);
    }
    #[test]
    fn define_plot_axes_bounds() {
        //define test data
        let x = linspace(0., 2., 3).unwrap();
        let y = DVector::from_vec(vec![2., 0., 1.7]);
        let z = linspace(2., 4., 3).unwrap();
        let z_mat = x.clone() * y.clone().transpose();

        let dat_2d = MatrixXx2::from_columns(&[x.clone(), y.clone()]);
        let dat_3d = MatrixXx3::from_columns(&[x.clone(), y.clone(), z.clone()]);

        //define PlotSeries
        let plt_series_dim2 = PlotSeries::new(
            &PlotData::new_dim2(dat_2d).unwrap(),
            RGBAColor(0, 0, 0, 1.),
            None,
        );
        let plt_series_dim3 = PlotSeries::new(
            &PlotData::new_dim3(dat_3d.clone()).unwrap(),
            RGBAColor(0, 0, 0, 1.),
            None,
        );
        let plt_series_colormesh = PlotSeries::new(
            &PlotData::new_colormesh(x.clone(), y.clone(), z_mat.clone()).unwrap(),
            RGBAColor(0, 0, 0, 1.),
            None,
        );
        let plt_series_surf_triangle = PlotSeries::new(
            &PlotData::new_triangulatedsurface(&dat_3d, None, None).unwrap(),
            RGBAColor(0, 0, 0, 1.),
            None,
        );

        let mut plot = Plot::try_from(&PlotParameters::default()).unwrap();
        testing_logger::setup();
        plot.define_axes_bounds();
        testing_logger::validate(|captured_logs| {
            assert_eq!(captured_logs.len(), 1);
            assert_eq!(
                captured_logs[0].body,
                "No plot series defined! Cannot define axes bounds!"
            );
            assert_eq!(captured_logs[0].level, Level::Warn);
        });

        let mut plot = Plot::new(&vec![plt_series_dim2], &PlotParameters::default());
        let _ = plot.define_axes_bounds();
        assert_relative_eq!(plot.bounds.x.unwrap().min, -0.1);
        assert_relative_eq!(plot.bounds.x.unwrap().max, 2.1);
        assert_relative_eq!(plot.bounds.y.unwrap().min, -0.1);
        assert_relative_eq!(plot.bounds.y.unwrap().max, 2.1);
        assert!(plot.bounds.z.is_none());

        let mut plot = Plot::new(&vec![plt_series_dim3], &PlotParameters::default());
        let _ = plot.define_axes_bounds();
        assert_relative_eq!(plot.bounds.x.unwrap().min, -0.1);
        assert_relative_eq!(plot.bounds.x.unwrap().max, 2.1);
        assert_relative_eq!(plot.bounds.y.unwrap().min, -0.1);
        assert_relative_eq!(plot.bounds.y.unwrap().max, 2.1);
        assert_relative_eq!(plot.bounds.z.unwrap().min, 1.9);
        assert_relative_eq!(plot.bounds.z.unwrap().max, 4.1);

        let mut plot = Plot::new(&vec![plt_series_colormesh], &PlotParameters::default());
        let _ = plot.define_axes_bounds();
        assert_relative_eq!(plot.bounds.x.unwrap().min, -0.1);
        assert_relative_eq!(plot.bounds.x.unwrap().max, 2.1);
        assert_relative_eq!(plot.bounds.y.unwrap().min, -0.1);
        assert_relative_eq!(plot.bounds.y.unwrap().max, 2.1);
        assert_relative_eq!(plot.bounds.z.unwrap().min, -0.2);
        assert_relative_eq!(plot.bounds.z.unwrap().max, 4.2);

        let mut plot = Plot::new(&vec![plt_series_surf_triangle], &PlotParameters::default());
        let _ = plot.define_axes_bounds();
        assert_relative_eq!(plot.bounds.x.unwrap().min, -0.1);
        assert_relative_eq!(plot.bounds.x.unwrap().max, 2.1);
        assert_relative_eq!(plot.bounds.y.unwrap().min, -0.1);
        assert_relative_eq!(plot.bounds.y.unwrap().max, 2.1);
        assert_relative_eq!(plot.bounds.z.unwrap().min, 1.9);
        assert_relative_eq!(plot.bounds.z.unwrap().max, 4.1);
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
        assert!(AxLims::new(-10., 10.).is_some());
        assert!(AxLims::new(0., f64::NAN).is_none());

        assert!((AxLims::new(-10., 10.).unwrap().min + 10.).abs() < f64::EPSILON);
        assert!((AxLims::new(-10., 10.).unwrap().max - 10.).abs() < f64::EPSILON);
    }
    #[test]
    fn axlim_expand() {
        let mut axlim = AxLims::new(-10., 10.).unwrap();
        let _ = axlim.expand_lim_range_by_factor(1.2);

        assert!((axlim.min + 12.).abs() < f64::EPSILON);
        assert!((axlim.max - 12.).abs() < f64::EPSILON);

        testing_logger::setup();
        axlim.expand_lim_range_by_factor(-1.);
        axlim.expand_lim_range_by_factor(f64::NAN);
        axlim.expand_lim_range_by_factor(f64::INFINITY);
        axlim.expand_lim_range_by_factor(0.);

        let warning = "Cannot expand ax limits! Expansion factor must be normal and positive!";
        testing_logger::validate(|captured_logs| {
            assert_eq!(captured_logs.len(), 4);
            assert_eq!(captured_logs[0].body, warning);
            assert_eq!(captured_logs[0].level, Level::Warn);
            assert_eq!(captured_logs[1].body, warning);
            assert_eq!(captured_logs[1].level, Level::Warn);
            assert_eq!(captured_logs[2].body, warning);
            assert_eq!(captured_logs[2].level, Level::Warn);
            assert_eq!(captured_logs[3].body, warning);
            assert_eq!(captured_logs[3].level, Level::Warn);
        });
    }
    #[test]
    fn create_useful_axlims_test() {
        let axlim = AxLims::create_useful_axlims(0., 10.).unwrap();
        assert_relative_eq!(axlim.min, 0.);
        assert_relative_eq!(axlim.max, 10.);

        let axlim = AxLims::create_useful_axlims(10., 10.).unwrap();
        assert_relative_eq!(axlim.min, 5.);
        assert_relative_eq!(axlim.max, 15.);

        let axlim = AxLims::create_useful_axlims(0., 0.).unwrap();
        assert_relative_eq!(axlim.min, -0.5);
        assert_relative_eq!(axlim.max, 0.5);

        let axlim = AxLims::create_useful_axlims(f64::NAN, 0.).unwrap();
        assert_relative_eq!(axlim.min, -0.5);
        assert_relative_eq!(axlim.max, 0.5);

        let axlim = AxLims::create_useful_axlims(f64::NAN, 10.).unwrap();
        assert_relative_eq!(axlim.min, 5.);
        assert_relative_eq!(axlim.max, 15.);

        let axlim = AxLims::create_useful_axlims(0., f64::NAN).unwrap();
        assert_relative_eq!(axlim.min, -0.5);
        assert_relative_eq!(axlim.max, 0.5);

        let axlim = AxLims::create_useful_axlims(-10., f64::NAN).unwrap();
        assert_relative_eq!(axlim.min, -15.);
        assert_relative_eq!(axlim.max, -5.);

        let axlim = AxLims::create_useful_axlims(10., -10.).unwrap();
        assert_relative_eq!(axlim.min, -10.);
        assert_relative_eq!(axlim.max, 10.);

        let axlim = AxLims::create_useful_axlims(10., f64::NAN).unwrap();
        assert_relative_eq!(axlim.min, 5.);
        assert_relative_eq!(axlim.max, 15.);
    }
    #[test]
    fn get_ax_val_distance_if_equidistant_test() {
        let x = linspace(0., 1., 101).unwrap();
        let dist = PlotType::get_ax_val_distance_if_equidistant(&x);
        assert!((dist - 0.01).abs() < f64::EPSILON);

        let x = linspace(0., f64::EPSILON, 101).unwrap();
        let dist = PlotType::get_ax_val_distance_if_equidistant(&x);
        assert!((dist - 0.5).abs() < f64::EPSILON);
    }
    #[test]
    fn check_equistancy_of_mesh_test() {
        let x = linspace(0., 1., 101).unwrap();
        assert!(PlotType::check_equistancy_of_mesh(&x));

        let x = linspace(-118.63435185555608, 0.000000000000014210854715202004, 100).unwrap();
        assert!(PlotType::check_equistancy_of_mesh(&x));

        let x = MatrixXx1::from_vec(vec![0., 1., 3.]);
        assert!(!PlotType::check_equistancy_of_mesh(&x));

        let x = MatrixXx1::from_vec(vec![0.]);
        assert!(PlotType::check_equistancy_of_mesh(&x));
    }
    #[test]
    fn calc_pixel_margin_test() {
        let axlims = AxLims::new(1e-4, 2e-4).unwrap();
        assert!(PlotType::calc_pixel_margin(axlims) == 72);

        let axlims = AxLims::new(-2e-4, -1e-4).unwrap();
        assert!(PlotType::calc_pixel_margin(axlims) == 85);

        let axlims = AxLims::new(1e-3, 2e-3).unwrap();
        assert!(PlotType::calc_pixel_margin(axlims) == 85);

        let axlims = AxLims::new(-2e-3, -1e-3).unwrap();
        assert!(PlotType::calc_pixel_margin(axlims) == 98);

        let axlims = AxLims::new(1e-2, 2e-2).unwrap();
        assert!(PlotType::calc_pixel_margin(axlims) == 85);

        let axlims = AxLims::new(-2e-2, -1e-2).unwrap();
        assert!(PlotType::calc_pixel_margin(axlims) == 98);

        let axlims = AxLims::new(1e-1, 2e-1).unwrap();
        assert!(PlotType::calc_pixel_margin(axlims) == 72);

        let axlims = AxLims::new(-2e-1, -1e-1).unwrap();
        assert!(PlotType::calc_pixel_margin(axlims) == 85);

        let axlims = AxLims::new(1e-0, 2e-0).unwrap();
        assert!(PlotType::calc_pixel_margin(axlims) == 72);

        let axlims = AxLims::new(-2e-0, -1e-0).unwrap();
        assert!(PlotType::calc_pixel_margin(axlims) == 85);

        let axlims = AxLims::new(1e1, 2e1).unwrap();
        assert!(PlotType::calc_pixel_margin(axlims) == 72);

        let axlims = AxLims::new(-2e1, -1e1).unwrap();
        assert!(PlotType::calc_pixel_margin(axlims) == 85);

        let axlims = AxLims::new(1e2, 2e2).unwrap();
        assert!(PlotType::calc_pixel_margin(axlims) == 72);

        let axlims = AxLims::new(-2e2, -1e2).unwrap();
        assert!(PlotType::calc_pixel_margin(axlims) == 85);

        let axlims = AxLims::new(1e3, 2e3).unwrap();
        assert!(PlotType::calc_pixel_margin(axlims) == 72);

        let axlims = AxLims::new(-2e3, -1e3).unwrap();
        assert!(PlotType::calc_pixel_margin(axlims) == 85);

        let axlims = AxLims::new(1e4, 2e4).unwrap();
        assert!(PlotType::calc_pixel_margin(axlims) == 72);

        let axlims = AxLims::new(-2e4, -1e4).unwrap();
        assert!(PlotType::calc_pixel_margin(axlims) == 85);
    }

    #[test]
    fn create_plots_png_test() {
        //define test data
        let x = DVector::from_vec(vec![0., -3., 20., 15.]);
        let y = DVector::from_vec(vec![10., -13., 25., 5.]);
        let z = linspace(4., 5., 4).unwrap();
        let z_mat = x.clone() * y.clone().transpose();

        let dat_2d = MatrixXx2::from_columns(&[x.clone(), y.clone()]);
        let dat_3d = MatrixXx3::from_columns(&[x.clone(), y.clone(), z.clone()]);

        //define PlotData
        let plt_series_dim2 = PlotSeries::new(
            &PlotData::new_dim2(dat_2d).unwrap(),
            RGBAColor(0, 0, 0, 1.),
            None,
        );
        let plt_series_colormesh = PlotSeries::new(
            &PlotData::new_colormesh(x.clone(), y.clone(), z_mat.clone()).unwrap(),
            RGBAColor(0, 0, 0, 1.),
            None,
        );
        let plt_series_surf_triangle = PlotSeries::new(
            &PlotData::new_triangulatedsurface(&dat_3d, None, None).unwrap(),
            RGBAColor(0, 0, 0, 1.),
            None,
        );

        let mut plt_params = PlotParameters::default();
        let path = NamedTempFile::new().unwrap();
        let _ = plt_params.set(&PlotArgs::FDir(path.path().parent().unwrap().into()));
        let _ = PlotType::Line2D(plt_params.clone()).plot(&vec![plt_series_dim2.clone()]);
        let _ = PlotType::ColorMesh(plt_params.clone()).plot(&vec![plt_series_colormesh]);
        let _ = PlotType::Scatter2D(plt_params.clone()).plot(&vec![plt_series_dim2]);
        let _ =
            PlotType::TriangulatedSurface(plt_params.clone()).plot(&vec![plt_series_surf_triangle]);
    }
    #[test]
    fn create_plots_svg_test() {
        //define test data
        let x = DVector::from_vec(vec![0., -3., 20., 15.]);
        let y = DVector::from_vec(vec![10., -13., 25., 5.]);
        let z = linspace(4., 5., 4).unwrap();
        let z_mat = x.clone() * y.clone().transpose();

        let dat_2d = MatrixXx2::from_columns(&[x.clone(), y.clone()]);
        let dat_3d = MatrixXx3::from_columns(&[x.clone(), y.clone(), z.clone()]);

        //define PlotData
        let plt_series_dim2 = PlotSeries::new(
            &PlotData::new_dim2(dat_2d).unwrap(),
            RGBAColor(0, 0, 0, 1.),
            None,
        );
        let plt_series_colormesh = PlotSeries::new(
            &PlotData::new_colormesh(x.clone(), y.clone(), z_mat.clone()).unwrap(),
            RGBAColor(0, 0, 0, 1.),
            None,
        );
        let plt_series_surf_triangle = PlotSeries::new(
            &PlotData::new_triangulatedsurface(&dat_3d, None, None).unwrap(),
            RGBAColor(0, 0, 0, 1.),
            None,
        );

        let mut plt_params = PlotParameters::default();
        let path = NamedTempFile::new().unwrap();
        let _ = plt_params
            .set(&PlotArgs::FDir(path.path().parent().unwrap().into()))
            .unwrap()
            .set(&PlotArgs::Backend(PltBackEnd::SVG))
            .unwrap()
            .set(&PlotArgs::FName("test.svg".into()));
        let _ = PlotType::Line2D(plt_params.clone()).plot(&vec![plt_series_dim2.clone()]);
        let _ = PlotType::ColorMesh(plt_params.clone()).plot(&vec![plt_series_colormesh]);
        let _ = PlotType::Scatter2D(plt_params.clone()).plot(&vec![plt_series_dim2]);
        let _ =
            PlotType::TriangulatedSurface(plt_params.clone()).plot(&vec![plt_series_surf_triangle]);
    }
    #[test]
    fn create_plots_buffer_test() {
        //define test data
        let x = DVector::from_vec(vec![0., -3., 20., 15.]);
        let y = DVector::from_vec(vec![10., -13., 25., 5.]);
        let z = linspace(4., 5., 4).unwrap();
        let z_mat = x.clone() * y.clone().transpose();

        let dat_2d = MatrixXx2::from_columns(&[x.clone(), y.clone()]);
        let dat_3d = MatrixXx3::from_columns(&[x.clone(), y.clone(), z.clone()]);

        //define PlotData
        let plt_series_dim2 = PlotSeries::new(
            &PlotData::Dim2 { xy_data: dat_2d },
            RGBAColor(0, 0, 0, 1.),
            None,
        );
        let plt_series_colormesh = PlotSeries::new(
            &PlotData::ColorMesh {
                x_dat_n: x.clone(),
                y_dat_m: y.clone(),
                z_dat_nxm: z_mat.clone(),
            },
            RGBAColor(0, 0, 0, 1.),
            None,
        );
        let plt_series_surf_triangle = PlotSeries::new(
            &PlotData::new_triangulatedsurface(&dat_3d, None, None).unwrap(),
            RGBAColor(0, 0, 0, 1.),
            None,
        );

        let mut plt_params = PlotParameters::default();
        let path = NamedTempFile::new().unwrap();
        let _ = plt_params
            .set(&PlotArgs::FDir(path.path().parent().unwrap().into()))
            .unwrap()
            .set(&PlotArgs::Backend(PltBackEnd::Buf));
        let _ = PlotType::Line2D(plt_params.clone()).plot(&vec![plt_series_dim2.clone()]);
        let _ = PlotType::ColorMesh(plt_params.clone()).plot(&vec![plt_series_colormesh]);
        let _ = PlotType::Scatter2D(plt_params.clone()).plot(&vec![plt_series_dim2]);
        let _ =
            PlotType::TriangulatedSurface(plt_params.clone()).plot(&vec![plt_series_surf_triangle]);
    }
}
