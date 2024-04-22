use bevy::log::LogPlugin;
use bevy::prelude::*;

use crate::reporter::AnalysisReport;
use crate::bevy_scene::MyScene;

pub fn bevy_main(report: &AnalysisReport) {
  
    App::new().add_plugins((
      DefaultPlugins.build().disable::<LogPlugin>(),
      MyScene{rays_hist: report.get_ray_hist().cloned()})).run();
}