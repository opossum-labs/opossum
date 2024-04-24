//! A simple 3D scene with light shining over a cube sitting on a plane.
use crate::reporter::AnalysisReport;
use bevy::{
    prelude::*,
    render::{mesh::PrimitiveTopology, render_asset::RenderAssetUsages},
};
// use bevy_flycam::prelude::*;
use uom::si::length::meter;

#[derive(Resource)]
struct Report(AnalysisReport);

pub struct MyScene {
    pub report: AnalysisReport,
}
impl Plugin for MyScene {
    fn build(&self, app: &mut App) {
        app.insert_resource(Report(self.report.clone()))
            //.add_plugins(PlayerPlugin)
            .add_systems(Startup, (setup_scene, setup_rays, setup_nodes));
    }
}
fn setup_rays(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    report: Res<Report>,
) {
    if let Some(ray_pos_hist) = report.0.get_ray_hist() {
        for ray_hist_per_wvl in &ray_pos_hist.rays_pos_history {
            for ray_hist in &ray_hist_per_wvl.history {
                let pos: Vec<_> = ray_hist
                    .row_iter()
                    .map(|p| {
                        Vec3::new(
                            p[0].get::<meter>() as f32,
                            p[1].get::<meter>() as f32,
                            p[2].get::<meter>() as f32,
                        )
                    })
                    .collect();
                commands.spawn(MaterialMeshBundle {
                    mesh: meshes.add(LineStrip { points: pos }),
                    material: materials.add(Color::GREEN),
                    transform: Transform::from_scale(Vec3::new(1.0, 1.0, 1.0)),
                    ..default()
                });
            }
        }
    } else {
        println!("No ray history given")
    }
}

fn setup_nodes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    report: Res<Report>,
) {
    if let Some(scenery)=report.0.scenery() {
        for node in scenery.nodes() {
            let mesh=node.optical_ref.lock().unwrap().mesh();
            println!("add surface");
            commands.spawn(MaterialMeshBundle {
                mesh: meshes.add(mesh),
                material: materials.add(Color::ORANGE),
                ..default()
            });
        }
    }
}
fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
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
    // directional light
    // commands.spawn(DirectionalLightBundle {
    //     directional_light: DirectionalLight::default(),
    //     transform: Transform::from_rotation(Quat::from_rotation_x(0.2)),
    //     ..default()
    // });
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
        transform: Transform::from_xyz(3.0, 3.0, -5.0).looking_at(Vec3::new(0.0,0.0,1.0), Vec3::Y),
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
        let vertices: Vec<_> = line.lines.into_iter().flat_map(|(a, b)| [a, b]).collect();

        Mesh::new(
            // This tells wgpu that the positions are list of lines
            // where every pair is a start and end point
            PrimitiveTopology::LineList,
            RenderAssetUsages::RENDER_WORLD,
        )
        // Add the vertices positions as an attribute
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vertices)
    }
}

/// A list of points that will have a line drawn between each consecutive points
#[derive(Debug, Clone)]
struct LineStrip {
    points: Vec<Vec3>,
}

impl From<LineStrip> for Mesh {
    fn from(line: LineStrip) -> Self {
        Mesh::new(
            // This tells wgpu that the positions are a list of points
            // where a line will be drawn between each consecutive point
            PrimitiveTopology::LineStrip,
            RenderAssetUsages::RENDER_WORLD,
        )
        // Add the point positions as an attribute
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, line.points)
    }
}
