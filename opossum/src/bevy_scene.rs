//! A simple 3D scene with light shining over a cube sitting on a plane.
use crate::scenery_bevy_data::SceneryBevyData;
use bevy::{
    prelude::*,
    render::{mesh::PrimitiveTopology, render_asset::RenderAssetUsages},
};
use uom::si::length::meter;

pub struct MyScene {
    pub scenery_bevy_data: SceneryBevyData,
}
impl Plugin for MyScene {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.scenery_bevy_data.clone())
            .add_systems(Startup, (setup_scene, setup_rays, setup_nodes));
    }
}
#[allow(clippy::cast_possible_truncation)]
const fn as_f32(x: f64) -> f32 {
    x as f32
}
#[allow(clippy::needless_pass_by_value)]
fn setup_rays(
    mut commands: Commands<'_, '_>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut materials: ResMut<'_, Assets<StandardMaterial>>,
    scenery_data: Res<'_, SceneryBevyData>,
) {
    if let Some(ray_pos_hist) = scenery_data.ray_history() {
        for ray_hist_per_wvl in &ray_pos_hist.rays_pos_history {
            for ray_hist in &ray_hist_per_wvl.history {
                let pos: Vec<_> = ray_hist
                    .row_iter()
                    .map(|p| {
                        Vec3::new(
                            as_f32(p[0].get::<meter>()),
                            as_f32(p[1].get::<meter>()),
                            as_f32(p[2].get::<meter>()),
                        )
                    })
                    .collect();
                commands.spawn(MaterialMeshBundle {
                    mesh: meshes.add(LineStrip { points: pos }),
                    material: materials.add(Color::GREEN),
                    ..default()
                });
            }
        }
    } else {
        println!("No ray history given");
    }
}

#[allow(clippy::needless_pass_by_value)]
fn setup_nodes(
    mut commands: Commands<'_, '_>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut materials: ResMut<'_, Assets<StandardMaterial>>,
    scenery_data: Res<'_, SceneryBevyData>,
) {
    for mesh in scenery_data.node_meshes() {
        commands.spawn(MaterialMeshBundle {
            mesh: meshes.add(mesh.clone()),
            material: materials.add(Color::ORANGE),
            ..default()
        });
    }
}
fn setup_scene(
    mut commands: Commands<'_, '_>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut materials: ResMut<'_, Assets<StandardMaterial>>,
) {
    // base plane
    commands.spawn(PbrBundle {
        mesh: meshes.add(Plane3d::new(Vec3::new(0.0, 1.0, 0.0))),
        material: materials.add(Color::WHITE),
        transform: Transform::from_xyz(0.0, -1.0, 0.0).with_scale(Vec3::new(10.0, 10.0, 10.0)),
        ..default()
    });
    // point light
    commands.spawn(PointLightBundle {
        point_light: PointLight {
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(4.0, 8.0, -4.0),
        ..default()
    });
    // optical axis
    commands.spawn(MaterialMeshBundle {
        mesh: meshes.add(LineStrip {
            points: vec![Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0)],
        }),
        material: materials.add(Color::RED),
        ..default()
    });
    // static camera
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(-1.0, 0.0, 0.3)
            .looking_at(Vec3::new(0.0, 0.0, 0.3), Vec3::Y),
        ..default()
    });
}

/// A list of lines with a start and end position
#[derive(Debug, Clone)]
struct LineList {
    lines: Vec<(Vec3, Vec3)>,
}

impl From<LineList> for Mesh {
    fn from(line: LineList) -> Self {
        #[allow(clippy::tuple_array_conversions)]
        let vertices: Vec<_> = line.lines.into_iter().flat_map(|(a, b)| [a, b]).collect();

        Self::new(
            // This tells wgpu that the positions are list of lines
            // where every pair is a start and end point
            PrimitiveTopology::LineList,
            RenderAssetUsages::RENDER_WORLD,
        )
        // Add the vertices positions as an attribute
        .with_inserted_attribute(Self::ATTRIBUTE_POSITION, vertices)
    }
}

/// A list of points that will have a line drawn between each consecutive points
#[derive(Debug, Clone)]
struct LineStrip {
    points: Vec<Vec3>,
}

impl From<LineStrip> for Mesh {
    fn from(line: LineStrip) -> Self {
        Self::new(
            // This tells wgpu that the positions are a list of points
            // where a line will be drawn between each consecutive point
            PrimitiveTopology::LineStrip,
            RenderAssetUsages::RENDER_WORLD,
        )
        // Add the point positions as an attribute
        .with_inserted_attribute(Self::ATTRIBUTE_POSITION, line.points)
    }
}
