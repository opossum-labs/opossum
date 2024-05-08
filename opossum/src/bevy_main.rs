use bevy::log::LogPlugin;
use bevy::prelude::*;

use crate::bevy_scene::MyScene;
use crate::scenery_bevy_data::SceneryBevyData;

pub fn bevy_main(scenery_data: SceneryBevyData) {
    App::new()
        .add_plugins((
            DefaultPlugins.build().disable::<LogPlugin>(),
            MyScene {
                scenery_bevy_data: scenery_data,
            },
        ))
        .run();
}
