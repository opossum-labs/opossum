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

    let projected_rays = pos_hist.project_to_plane(Vector3::x())?;
    assert_eq!(projected_rays[0][(0, 0)].get::<millimeter>(), 0.);
    assert_eq!(projected_rays[0][(0, 1)].get::<millimeter>(), 0.);
    assert_eq!(projected_rays[1][(0, 0)].get::<millimeter>(), 0.);
    assert_eq!(projected_rays[1][(0, 1)].get::<millimeter>(), 1.);
    assert_eq!(projected_rays[2][(0, 0)].get::<millimeter>(), 1.);
    assert_eq!(projected_rays[2][(0, 1)].get::<millimeter>(), 0.);
    Ok(())
}

// /// TODO: Reenable
// #[test]
// fn test_legend_and_colorbar_thresholds() -> OpmResult<()> {
//     let single_point_hist = vec![MatrixXx3::from_vec(vec![
//         millimeter!(0.),
//         millimeter!(0.),
//         millimeter!(0.),
//     ])];

//     let make_histories = |count: usize| -> RayPositionHistories {
//         let histories = (0..count)
//             .map(|i| {
//                 RayPositionHistorySpectrum::new(
//                     single_point_hist.clone(),
//                     nanometer!(500.0 + (i as f64) * 50.0),
//                     nanometer!(10.0),
//                 )
//                 .unwrap()
//             })
//             .collect();
//         RayPositionHistories {
//             rays_pos_history: histories,
//             plot_view_direction: Some(Vector3::x()),
//             ray_transparency: 0.5,
//         }
//     };

//     // 1. Monochromatic (1 wavelength): No series label in legend
//     let mono = make_histories(1);
//     let mut plt_params = PlotParameters::default();
//     mono.add_plot_specific_params(&mut plt_params)?;
//     let mut plt_type = mono.get_plot_type(&plt_params);
//     let series = mono.get_plot_series(&mut plt_type, true)?.unwrap();
//     assert_eq!(series.len(), 1);
//     assert!(series[0].get_series_label().is_none());

//     // 2. Polychromatic small (3 wavelengths <= 5): Series labels present for legend
//     let poly_small = make_histories(3);
//     let mut plt_params = PlotParameters::default();
//     poly_small.add_plot_specific_params(&mut plt_params)?;
//     let mut plt_type = poly_small.get_plot_type(&plt_params);
//     let series = poly_small.get_plot_series(&mut plt_type, true)?.unwrap();
//     assert_eq!(series.len(), 3);
//     assert_eq!(series[0].get_series_label(), Some("500.0 nm".to_string()));
//     assert_eq!(series[2].get_series_label(), Some("600.0 nm".to_string()));

//     // 3. Polychromatic large (6 wavelengths > 5): Colorbar configured, series labels are None
//     let poly_large = make_histories(6);
//     let mut plt_params = PlotParameters::default();
//     poly_large.add_plot_specific_params(&mut plt_params)?;
//     let mut plt_type = poly_large.get_plot_type(&plt_params);
//     let series = poly_large.get_plot_series(&mut plt_type, true)?.unwrap();
//     assert_eq!(series.len(), 6);
//     assert!(series[0].get_series_label().is_none());

//     if let PlotType::MultiLine2D(params) = plt_type {
//         assert_eq!(params.get_cbar_label()?, "wavelength (nm)".to_string());
//         assert!(!params.get_legend_flag()?);
//         assert!(params.get_zlim()?.is_some());
//     } else {
//         panic!("Expected PlotType::MultiLine2D");
//     }
//     Ok(())
// }
