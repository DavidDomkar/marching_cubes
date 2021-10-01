mod marching_cubes;
mod plugins;

use bevy::asset::AssetServer;
use bevy::ecs::system::Res;
use crate::plugins::NoCameraPlayerPlugin;
use crate::plugins::FlyCam;
use bevy::render2::camera::PerspectiveCameraBundle;
use bevy::math::Vec3;
use bevy::transform::components::Transform;
use bevy::render2::mesh::shape::UVSphere;
use bevy::render2::mesh::Indices;
use bevy::render2::mesh::shape::Cube;

use crate::marching_cubes::TRI_TABLE;

use bevy:: {
    PipelinedDefaultPlugins,
    app::App,
    asset::Assets,
    ecs::system::Commands,
    ecs::system::ResMut,
    pbr2::{
        StandardMaterial,
        AmbientLight,
        PbrBundle,
    },
    render2:: {
        mesh::Mesh, 
        color::Color,
        render_resource::PrimitiveTopology,
    },
    window::WindowDescriptor,
};

fn main() {
    App::new()
        .insert_resource(WindowDescriptor {
            width: 1920.0,
            height: 1080.0,
            title: "Lulw".to_string(),
            vsync: true,
            ..Default::default()
        })
        .add_plugins(PipelinedDefaultPlugins)
        .add_plugin(NoCameraPlayerPlugin)
        .add_startup_system(setup)
        .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);
    let mut cube = Mesh::from(Cube { size: 10.0 });

    let current_lookup = TRI_TABLE[1];

    let vertices = vec![
        [0.0, -5.0, 5.0],
        [5.0, 0.0, 5.0],
        [0.0, 5.0, 5.0],
        [-5.0, 0.0, 5.0],
        [0.0, -5.0, -5.0],
        [5.0, 0.0, -5.0],
        [0.0, 5.0, -5.0],
        [-5.0, 0.0, -5.0],
        [-5.0, -5.0, 0.0],
        [5.0, -5.0, 0.0],
        [5.0, 5.0, 0.0],
        [-5.0, 5.0, 0.0],
    ];

    let mut indices: Vec<u32> = Vec::new();

    for (_, index) in current_lookup.iter().enumerate() {
        if *index == -1 {
            break;
        }

        indices.push(*index as u32);
    }

    for (i, vertex) in vertices.iter().enumerate() {
        commands.spawn_bundle(PbrBundle {
            mesh: meshes.add(Mesh::from(UVSphere {
                radius: 0.5,
                ..Default::default()
            })),
            material: materials.add(StandardMaterial {
                base_color: if indices.contains(&(i as u32)) { Color::RED } else { Color::YELLOW },
                ..Default::default()
            }),
            transform: Transform::from_translation(Vec3::from(*vertex)),
            ..Default::default()
        });
    }

    mesh.set_attribute(Mesh::ATTRIBUTE_POSITION, vertices.clone());
    mesh.set_attribute(Mesh::ATTRIBUTE_NORMAL, vertices.clone());
    mesh.set_attribute(Mesh::ATTRIBUTE_UV_0, vec![
        [0.0, 0.0],
        [0.0, 0.0],
        [0.0, 0.0],
        [0.0, 0.0],
        [0.0, 0.0],
        [0.0, 0.0],
        [0.0, 0.0],
        [0.0, 0.0],
        [0.0, 0.0],
        [0.0, 0.0],
        [0.0, 0.0],
        [0.0, 0.0],
    ]);

    mesh.set_indices(Some(Indices::U32(indices))); 

    commands.spawn_bundle(PbrBundle {
        mesh: meshes.add(mesh),
        material: materials.add(StandardMaterial {
            base_color_texture: Some(asset_server.load("triangle.png")),
            ..Default::default()
        }),
        ..Default::default()
    });

    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 1.0,
    });

    commands
        .spawn_bundle(PerspectiveCameraBundle {
            transform: Transform::from_xyz(-15.0, 10.0, 15.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..Default::default()
        }).insert(FlyCam);
}