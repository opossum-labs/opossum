use approx::assert_relative_eq;
use nalgebra::{MatrixXx3, Vector3};
use uom::si::length::{millimeter, nanometer};

use super::*;
use crate::{
    core_optics::PortType,
    distributions::position::Hexapolar,
    error::OpmResult,
    joule,
    light::{Rays, spectrum_helper::create_he_ne_spec},
    millimeter, nanometer,
    nodes::test_helper::helper::*,
};

#[test]
fn default() {
    let node = RayPropagationVisualizer::default();
    assert!(node.light_data.is_none());
    assert_eq!(node.name(), "ray propagation");
    assert_eq!(node.node_type(), "ray propagation");
    assert!(!node.inverted());
    assert_eq!(node.node_color(), "darkgreen");
}

#[test]
fn new() -> OpmResult<()> {
    let meter = RayPropagationVisualizer::new("test", None)?;
    assert_eq!(meter.name(), "test");
    assert!(meter.light_data.is_none());
    Ok(())
}

#[test]
fn ports() {
    let meter = RayPropagationVisualizer::default();
    assert_eq!(meter.ports().names(&PortType::Input), vec!["input_1"]);
    assert_eq!(meter.ports().names(&PortType::Output), vec!["output_1"]);
}

#[test]
fn ports_inverted() -> OpmResult<()> {
    let mut meter = RayPropagationVisualizer::default();
    meter.set_inverted(true)?;
    assert_eq!(meter.ports().names(&PortType::Input), vec!["output_1"]);
    assert_eq!(meter.ports().names(&PortType::Output), vec!["input_1"]);
    Ok(())
}

#[test]
fn inverted() -> OpmResult<()> {
    test_inverted::<RayPropagationVisualizer>()
}

#[test]
fn analyze_empty() -> OpmResult<()> {
    test_analyze_empty::<RayPropagationVisualizer>()
}

#[test]
fn analyze_wrong() -> OpmResult<()> {
    let mut node = RayPropagationVisualizer::default();
    let mut input = LightResult::default();
    let input_light = LightData::Geometric(Rays::default());
    input.insert("output_1".into(), input_light.clone());
    let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default())?;
    assert!(output.is_empty());
    Ok(())
}

#[test]
fn analyze_ok() -> OpmResult<()> {
    let mut node = RayPropagationVisualizer::default();
    let mut input = LightResult::default();
    let input_light = LightData::Energy(create_he_ne_spec(1.0)?);
    input.insert("input_1".into(), input_light.clone());
    let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default())?;
    assert!(output.contains_key("output_1"));
    assert_eq!(output.len(), 1);
    let output = output.get("output_1");
    assert_eq!(output, Some(&input_light));
    Ok(())
}

#[test]
fn analyze_inverse() -> OpmResult<()> {
    let mut node = RayPropagationVisualizer::default();
    node.set_inverted(true)?;
    let mut input = LightResult::default();
    let input_light = LightData::Energy(create_he_ne_spec(1.0)?);
    input.insert("output_1".into(), input_light.clone());

    let output = AnalysisEnergy::analyze(&mut node, input, &EnergyConfig::default())?;
    assert!(output.contains_key("input_1"));
    assert_eq!(output.len(), 1);
    let output = output.get("input_1");
    assert_eq!(output, Some(&input_light));
    Ok(())
}

#[test]
fn report() -> OpmResult<()> {
    let mut fd = RayPropagationVisualizer::default();
    let NodeReportResult::Report(node_report) = fd.node_report("", AnalyzerKind::RayTrace)? else {
        panic!("Report should not be `None`");
    };
    assert_eq!(node_report.node_type(), "ray propagation");
    assert_eq!(node_report.name(), "ray propagation");
    let node_props = node_report.properties();
    let nr_of_props = node_props.iter().fold(0, |c, _p| c + 1);
    assert_eq!(nr_of_props, 0);

    fd.light_data = Some(LightData::Geometric(Rays::default()));
    let NodeReportResult::Report(node_report) = fd.node_report("", AnalyzerKind::RayTrace)? else {
        panic!("Report should not be `None`");
    };
    assert!(!node_report.properties().contains("Ray plot"));

    fd.light_data = Some(LightData::Geometric(Rays::new_uniform_collimated(
        nanometer!(1053.0),
        joule!(1.0),
        &Hexapolar::new(millimeter!(1.), 1)?,
    )?));
    let NodeReportResult::Report(node_report) = fd.node_report("", AnalyzerKind::RayTrace)? else {
        panic!("Report should not be `None`");
    };
    assert!(node_report.properties().contains("Ray plot"));
    let node_props = node_report.properties();
    let nr_of_props = node_props.iter().fold(0, |c, _p| c + 1);
    assert_eq!(nr_of_props, 1);
    assert!(node_report.notes().is_empty());

    // Apodization warning test
    fd.set_apodization_warning(true);
    let NodeReportResult::Report(node_report) = fd.node_report("", AnalyzerKind::RayTrace)? else {
        panic!("Report should not be `None`");
    };
    assert_eq!(node_report.notes().len(), 1);
    assert_eq!(
        node_report.notes()[0].level,
        crate::reporting::report_note::ReportLevel::Warning
    );
    assert!(node_report.notes()[0].message.contains("Rays were lost"));

    // Energy warning test
    fd.set_apodization_warning(false);
    fd.light_data = Some(LightData::Energy(crate::light::Spectrum::default()));
    let NodeReportResult::Incompatible(report_note) = fd.node_report("", AnalyzerKind::Energy)?
    else {
        panic!("Report should not be `None`");
    };
    assert!(
        report_note
            .message
            .contains("propagation plot can only be calculated")
    );
    Ok(())
}

#[test]
fn new_ray_pos_hist_spec() {
    let h = vec![
        MatrixXx3::from_vec(vec![millimeter!(1.), millimeter!(0.), millimeter!(0.)]),
        MatrixXx3::from_vec(vec![millimeter!(0.), millimeter!(1.), millimeter!(0.)]),
        MatrixXx3::from_vec(vec![millimeter!(0.), millimeter!(0.), millimeter!(1.)]),
    ];
    let wb = nanometer!(1.);
    let w = nanometer!(1053.);
    assert!(RayPositionHistorySpectrum::new(h.clone(), w, wb).is_ok());

    let wb = nanometer!(0.);
    assert!(RayPositionHistorySpectrum::new(h.clone(), w, wb).is_err());
    let wb = nanometer!(-1.);
    assert!(RayPositionHistorySpectrum::new(h.clone(), w, wb).is_err());
    let wb = nanometer!(f64::NAN);
    assert!(RayPositionHistorySpectrum::new(h.clone(), w, wb).is_err());
    let wb = nanometer!(f64::INFINITY);
    assert!(RayPositionHistorySpectrum::new(h.clone(), w, wb).is_err());

    let w = nanometer!(0.);
    assert!(RayPositionHistorySpectrum::new(h.clone(), w, wb).is_err());
    let w = nanometer!(-1.);
    assert!(RayPositionHistorySpectrum::new(h.clone(), w, wb).is_err());
    let w = nanometer!(f64::NAN);
    assert!(RayPositionHistorySpectrum::new(h.clone(), w, wb).is_err());
}

#[test]
fn ray_pos_hist_spec_get_history() -> OpmResult<()> {
    let history = vec![
        MatrixXx3::from_vec(vec![millimeter!(1.), millimeter!(0.), millimeter!(0.)]),
        MatrixXx3::from_vec(vec![millimeter!(0.), millimeter!(1.), millimeter!(0.)]),
        MatrixXx3::from_vec(vec![millimeter!(0.), millimeter!(0.), millimeter!(1.)]),
    ];
    let wavelength_bin_size = nanometer!(1.);
    let wavelength = nanometer!(1053.);
    let pos_hist =
        RayPositionHistorySpectrum::new(history.clone(), wavelength, wavelength_bin_size)?;

    let pos_hist_get = pos_hist.get_history();
    assert_relative_eq!(
        history[0][0].get::<millimeter>(),
        pos_hist_get[0][0].get::<millimeter>()
    );
    assert_relative_eq!(
        history[1][1].get::<millimeter>(),
        pos_hist_get[1][1].get::<millimeter>()
    );
    assert_relative_eq!(
        history[2][2].get::<millimeter>(),
        pos_hist_get[2][2].get::<millimeter>()
    );
    Ok(())
}

#[test]
fn ray_pos_hist_spec_get_wavelength() {
    let history = vec![MatrixXx3::from_vec(vec![
        millimeter!(1.),
        millimeter!(0.),
        millimeter!(0.),
    ])];
    let wavelength_bin_size = nanometer!(1.);
    let wavelength = nanometer!(1053.);
    let pos_hist = RayPositionHistorySpectrum {
        history,
        center_wavelength: wavelength,
        wavelength_bin_size,
    };
    assert_relative_eq!(
        pos_hist.get_center_wavelength().get::<nanometer>(),
        wavelength.get::<nanometer>()
    );
}

#[test]
fn ray_pos_hist_spec_get_bin_size() {
    let history = vec![MatrixXx3::from_vec(vec![
        millimeter!(1.),
        millimeter!(0.),
        millimeter!(0.),
    ])];
    let wavelength_bin_size = nanometer!(1.);
    let pos_hist = RayPositionHistorySpectrum {
        history,
        center_wavelength: nanometer!(1053.),
        wavelength_bin_size,
    };
    assert_relative_eq!(
        pos_hist.get_wavelength_bin_size().get::<nanometer>(),
        wavelength_bin_size.get::<nanometer>()
    );
}

#[test]
fn project_to_plane() -> OpmResult<()> {
    let history = vec![
        MatrixXx3::from_vec(vec![millimeter!(1.), millimeter!(0.), millimeter!(0.)]),
        MatrixXx3::from_vec(vec![millimeter!(0.), millimeter!(1.), millimeter!(0.)]),
        MatrixXx3::from_vec(vec![millimeter!(0.), millimeter!(0.), millimeter!(1.)]),
    ];

    let pos_hist = RayPositionHistorySpectrum {
        history,
        center_wavelength: nanometer!(1053.),
        wavelength_bin_size: nanometer!(1.),
    };

    // 1. Zero-length normal vector must return an error
    assert!(pos_hist.project_to_plane(Vector3::zeros()).is_err());

    // 2. Projection along X axis (co_ax_1 = Z, co_ax_2 = Y)
    let projected_x = pos_hist.project_to_plane(Vector3::x())?;
    assert_relative_eq!(projected_x[0][(0, 0)].get::<millimeter>(), 0.);
    assert_relative_eq!(projected_x[0][(0, 1)].get::<millimeter>(), 0.);
    assert_relative_eq!(projected_x[1][(0, 0)].get::<millimeter>(), 0.);
    assert_relative_eq!(projected_x[1][(0, 1)].get::<millimeter>(), 1.);
    assert_relative_eq!(projected_x[2][(0, 0)].get::<millimeter>(), 1.);
    assert_relative_eq!(projected_x[2][(0, 1)].get::<millimeter>(), 0.);

    // 3. Projection along Y axis (co_ax_1 = Z, co_ax_2 = X)
    let projected_y = pos_hist.project_to_plane(Vector3::y())?;
    assert_relative_eq!(projected_y[0][(0, 0)].get::<millimeter>(), 0.);
    assert_relative_eq!(projected_y[0][(0, 1)].get::<millimeter>(), 1.);
    assert_relative_eq!(projected_y[1][(0, 0)].get::<millimeter>(), 0.);
    assert_relative_eq!(projected_y[1][(0, 1)].get::<millimeter>(), 0.);
    assert_relative_eq!(projected_y[2][(0, 0)].get::<millimeter>(), 1.);
    assert_relative_eq!(projected_y[2][(0, 1)].get::<millimeter>(), 0.);

    // 4. Projection along Z axis (co_ax_1 = X, co_ax_2 = Y)
    let projected_z = pos_hist.project_to_plane(Vector3::z())?;
    assert_relative_eq!(projected_z[0][(0, 0)].get::<millimeter>(), 1.);
    assert_relative_eq!(projected_z[0][(0, 1)].get::<millimeter>(), 0.);
    assert_relative_eq!(projected_z[1][(0, 0)].get::<millimeter>(), 0.);
    assert_relative_eq!(projected_z[1][(0, 1)].get::<millimeter>(), 1.);
    assert_relative_eq!(projected_z[2][(0, 0)].get::<millimeter>(), 0.);
    assert_relative_eq!(projected_z[2][(0, 1)].get::<millimeter>(), 0.);

    // 5. General non-unit oblique normal vector (1, 1, 0) with norm = sqrt(2):
    // co_ax_1 = (1/sqrt(2), -1/sqrt(2), 0), co_ax_2 = (0, 0, 1)
    let oblique_normal = Vector3::new(1.0, 1.0, 0.0);
    let projected_oblique = pos_hist.project_to_plane(oblique_normal)?;
    assert_relative_eq!(
        projected_oblique[0][(0, 0)].get::<millimeter>(),
        std::f64::consts::FRAC_1_SQRT_2
    );
    assert_relative_eq!(projected_oblique[0][(0, 1)].get::<millimeter>(), 0.);
    assert_relative_eq!(
        projected_oblique[1][(0, 0)].get::<millimeter>(),
        -std::f64::consts::FRAC_1_SQRT_2
    );
    assert_relative_eq!(projected_oblique[1][(0, 1)].get::<millimeter>(), 0.);
    assert_relative_eq!(projected_oblique[2][(0, 0)].get::<millimeter>(), 0.);
    assert_relative_eq!(projected_oblique[2][(0, 1)].get::<millimeter>(), 1.);

    Ok(())
}
#[test]
fn node_report_applies_plot_view_direction_from_properties() -> OpmResult<()> {
    // Arrange: Create visualizer with a custom non-default view direction
    let expected_view_dir = Vector3::new(0.0, 1.0, 0.0);
    let mut visualizer = RayPropagationVisualizer::new("test_visualizer", Some(expected_view_dir))?;

    visualizer
        .node_attr
        .set_property("ray transparency", 0.75.into())?;

    // Populate with valid collimated rays so position history extraction succeeds
    let rays = Rays::new_uniform_collimated(
        nanometer!(1053.0),
        joule!(1.0),
        &Hexapolar::new(millimeter!(1.0), 1)?,
    )?;
    visualizer.set_light_data(Some(LightData::Geometric(rays)));

    // Act: Generate the node report for a raytrace analysis
    let NodeReportResult::Report(report) =
        visualizer.node_report("test-uuid", AnalyzerKind::RayTrace)?
    else {
        panic!("Expected NodeReportResult::Report, but received a warning");
    };

    // Assert: Verify the report contains RayPositionHistories with the configured properties
    let ray_plot_prop = report.properties().get("Ray plot")?;
    let Proptype::RayPositionHistory(histories) = ray_plot_prop else {
        panic!("Expected Proptype::RayPositionHistory for 'Ray plot'");
    };

    assert_eq!(histories.plot_view_direction, Some(expected_view_dir));
    assert_relative_eq!(histories.ray_transparency, 0.75);

    Ok(())
}
