use bevy::log::LogPlugin;
use bevy::prelude::*;

use crate::bevy_scene::MyScene;
use crate::reporter::AnalysisReport;

pub fn bevy_main(report: &AnalysisReport) {
    App::new()
        .add_plugins((
            DefaultPlugins.build().disable::<LogPlugin>(),
            MyScene {
                rays_hist: report.get_ray_hist().cloned(),
            },
        ))
        .run();
}
