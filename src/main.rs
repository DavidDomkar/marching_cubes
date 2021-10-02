mod marching_cubes;
mod plugins;

use std::convert::TryInto;
use noise::Value;
use noise::SuperSimplex;
use noise::OpenSimplex;
use crate::plugins::NoCameraPlayerPlugin;
use crate::plugins::FlyCam;
use bevy::render2::camera::PerspectiveCameraBundle;
use bevy::math::Vec3;
use bevy::transform::components::Transform;
use bevy::render2::mesh::shape::UVSphere;
use bevy::render2::mesh::Indices;
use noise::{NoiseFn, Perlin, Seedable};

use crate::marching_cubes::*;

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

fn value_from_noise(noise: noise::Perlin, translation: Vec3) -> f32 {
    (noise.get([translation.x as f64 / 50.0, translation.y as f64 / 50.0, translation.z as f64 / 50.0]) * 2.0) as f32
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);

    let cell_size = 2.5;
    let iso_level = 0.7;

    let number_of_cells = 200;

    let perlin = Perlin::new();

    perlin.set_seed(5225);

    let mut triangles: Vec<Triangle> = Vec::new();

    for z in 0 .. number_of_cells {
        for y in 0 .. number_of_cells {
            for x in 0 .. number_of_cells {
                let translation = Vec3::new(x as f32 * cell_size - number_of_cells as f32 / 2.0 * cell_size + cell_size / 2.0, y as f32 * cell_size - number_of_cells as f32 / 2.0 * cell_size + cell_size / 2.0, z as f32 * cell_size - number_of_cells as f32 / 2.0 * cell_size + cell_size / 2.0);

                let cell_points = [ translation + Vec3::new(-cell_size / 2.0, -cell_size / 2.0, -cell_size / 2.0),
                                    translation + Vec3::new(cell_size / 2.0, -cell_size / 2.0, -cell_size / 2.0),
                                    translation + Vec3::new(cell_size / 2.0, -cell_size / 2.0, cell_size / 2.0),
                                    translation + Vec3::new(-cell_size / 2.0, -cell_size / 2.0, cell_size / 2.0),
                                    translation + Vec3::new(-cell_size / 2.0, cell_size / 2.0, -cell_size / 2.0),
                                    translation + Vec3::new(cell_size / 2.0, cell_size / 2.0, -cell_size / 2.0),
                                    translation + Vec3::new(cell_size / 2.0, cell_size / 2.0, cell_size / 2.0),
                                    translation + Vec3::new(-cell_size / 2.0, cell_size / 2.0, cell_size / 2.0)];
                
                let cell_points = [ (cell_points[0], value_from_noise(perlin, cell_points[0])),
                                    (cell_points[1], value_from_noise(perlin, cell_points[1])),
                                    (cell_points[2], value_from_noise(perlin, cell_points[2])),
                                    (cell_points[3], value_from_noise(perlin, cell_points[3])),
                                    (cell_points[4], value_from_noise(perlin, cell_points[4])),
                                    (cell_points[5], value_from_noise(perlin, cell_points[5])),
                                    (cell_points[6], value_from_noise(perlin, cell_points[6])),
                                    (cell_points[7], value_from_noise(perlin, cell_points[7]))];

                triangles.append(&mut polygonise(cell_points, iso_level));
            }
        }
    }

    // let triangles = generate_mesh(15.0, 15.0, 15.0, 0.1);

    let vertices = triangles.iter().map(|triangle| { [triangle.a, triangle.b, triangle.c] }).flatten().map(|vector| { [vector.x, vector.y, vector.z] }).collect::<Vec<_>>();
    let mut indices = (0..vertices.len()).map(|index| { index as u32 }).collect::<Vec<u32>>();
    let uvs = (0..vertices.len()).map(|_| { [0.0, 0.0] }).collect::<Vec<[f32; 2]>>();

    /*
    for vertex in vertices.iter() {
        commands.spawn_bundle(PbrBundle {
            mesh: meshes.add(Mesh::from(UVSphere {
                radius: 0.5,
                ..Default::default()
            })),
            material: materials.add(StandardMaterial {
                base_color: Color::YELLOW,
                ..Default::default()
            }),
            transform: Transform::from_translation(Vec3::from(*vertex)),
            ..Default::default()
        });
    }
    */

    
    let mut normals: Vec<[f32; 3]> = Vec::new();

    for triangle in indices.chunks(3) {
        let a = Vec3::from(vertices[(triangle)[0] as usize]);
        let b = Vec3::from(vertices[(triangle)[1] as usize]);
        let c = Vec3::from(vertices[(triangle)[2] as usize]);

        let normal = (b - a).cross(c - a).normalize() * Vec3::new(-1.0, -1.0, -1.0);

        normals.push(normal.into());
        normals.push(normal.into());
        normals.push(normal.into());
    }

    indices.reverse();

    mesh.set_indices(Some(Indices::U32(indices)));

    mesh.set_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
    mesh.set_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.set_attribute(Mesh::ATTRIBUTE_UV_0, uvs);

    commands.spawn_bundle(PbrBundle {
        mesh: meshes.add(mesh),
        material: materials.add(StandardMaterial {
            base_color: Color::BLUE,
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
            transform: Transform::from_xyz(-40.0, 40.0, 40.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..Default::default()
        }).insert(FlyCam);
}