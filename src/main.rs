mod marching_cubes;
mod plugins;

use bevy::ecs::query::With;
use bevy::asset::Handle;
use bevy::ecs::system::Query;
use bevy::ecs::system::Res;
use std::convert::TryInto;
use noise::Value;
use noise::SuperSimplex;
use noise::OpenSimplex;
use crate::plugins::NoCameraPlayerPlugin;
use crate::plugins::FlyCam;
use noise::{NoiseFn, Perlin, Seedable};

use crate::marching_cubes::*;

use bevy:: {
    core::Time,
    PipelinedDefaultPlugins,
    app::App,
    asset::Assets,
    ecs::system::Commands,
    ecs::system::ResMut,
    log::*,
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
    render2::camera::PerspectiveCameraBundle,
    math::Vec3,
    transform::components::Transform,
    render2::mesh::shape::UVSphere,
    render2::mesh::Indices,
    window::WindowDescriptor,
};

pub struct WorldMesh;

fn main() {
    let perlin = Perlin::new();

    perlin.set_seed(5225);

    App::new()
        .insert_resource(WindowDescriptor {
            width: 1920.0,
            height: 1080.0,
            title: "Lulw".to_string(),
            vsync: true,
            ..Default::default()
        })
        .insert_resource(LogSettings {
            level: Level::ERROR,
            ..Default::default()
        })
        .insert_resource(perlin)
        .add_plugins(PipelinedDefaultPlugins)
        .add_plugin(NoCameraPlayerPlugin)
        .add_startup_system(setup)
        .add_system(update)
        .run();
}

fn value_from_noise(noise: noise::Perlin, translation: Vec3) -> f32 {
    1.0 - (noise.get([translation.x as f64 / 50.0, translation.y as f64 / 50.0, translation.z as f64 / 50.0]) * 2.0) as f32
}

fn generate_triangles(perlin: &noise::Perlin, translation_offset: Vec3) -> Vec<Triangle> {
    let mut triangles: Vec<Triangle> = Vec::new();

    let cell_size = 5.0;
    let iso_level = 0.7;

    let number_of_cells = 10;

    let perlin = *perlin;

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
                
                let cell_points = [ (cell_points[0], value_from_noise(perlin, cell_points[0] + translation_offset)),
                                    (cell_points[1], value_from_noise(perlin, cell_points[1] + translation_offset)),
                                    (cell_points[2], value_from_noise(perlin, cell_points[2] + translation_offset)),
                                    (cell_points[3], value_from_noise(perlin, cell_points[3] + translation_offset)),
                                    (cell_points[4], value_from_noise(perlin, cell_points[4] + translation_offset)),
                                    (cell_points[5], value_from_noise(perlin, cell_points[5] + translation_offset)),
                                    (cell_points[6], value_from_noise(perlin, cell_points[6] + translation_offset)),
                                    (cell_points[7], value_from_noise(perlin, cell_points[7] + translation_offset))];

                triangles.append(&mut polygonise(cell_points, iso_level));
            }
        }
    }

    triangles
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, perlin: Res<Perlin>) {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);

    let triangles = generate_triangles(&perlin, Vec3::ZERO);

    let vertices = triangles.iter().map(|triangle| { [triangle.a, triangle.b, triangle.c] }).flatten().map(|vector| { [vector.x, vector.y, vector.z] }).collect::<Vec<_>>();
    let indices = (0..vertices.len()).map(|index| { index as u32 }).collect::<Vec<u32>>();
    let uvs = (0..vertices.len()).map(|_| { [0.0, 0.0] }).collect::<Vec<[f32; 2]>>();

    let mut normals: Vec<[f32; 3]> = Vec::new();

    for triangle in indices.chunks(3) {
        let a = Vec3::from(vertices[(triangle)[0] as usize]);
        let b = Vec3::from(vertices[(triangle)[1] as usize]);
        let c = Vec3::from(vertices[(triangle)[2] as usize]);

        let normal = (b - a).cross(c - a).normalize();

        normals.push(normal.into());
        normals.push(normal.into());
        normals.push(normal.into());
    }

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
    }).insert(WorldMesh);

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

fn update(time: Res<Time>, mut meshes: ResMut<Assets<Mesh>>, mut query: Query<&mut Handle<Mesh>, With<WorldMesh>>, perlin: Res<Perlin>) {
    for mut mesh in &mut query.iter_mut() {
        // println!("{:?}", mesh.id);

        let mesh = meshes.get_mut(&*mesh).unwrap();

        let triangles = generate_triangles(&perlin, time.seconds_since_startup() as f32 * Vec3::new(10.0, 10.0, 10.0));

        let vertices = triangles.iter().map(|triangle| { [triangle.a, triangle.b, triangle.c] }).flatten().map(|vector| { [vector.x, vector.y, vector.z] }).collect::<Vec<_>>();
        let indices = (0..vertices.len()).map(|index| { index as u32 }).collect::<Vec<u32>>();
        let uvs = (0..vertices.len()).map(|_| { [0.0, 0.0] }).collect::<Vec<[f32; 2]>>();

        let mut normals: Vec<[f32; 3]> = Vec::new();

        for triangle in indices.chunks(3) {
            let a = Vec3::from(vertices[(triangle)[0] as usize]);
            let b = Vec3::from(vertices[(triangle)[1] as usize]);
            let c = Vec3::from(vertices[(triangle)[2] as usize]);

            let normal = (b - a).cross(c - a).normalize();

            normals.push(normal.into());
            normals.push(normal.into());
            normals.push(normal.into());
        }

        mesh.set_indices(Some(Indices::U32(indices)));

        mesh.set_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
        mesh.set_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.set_attribute(Mesh::ATTRIBUTE_UV_0, uvs);

        /*
        mesh.set_attribute(Mesh::ATTRIBUTE_NORMAL, mesh.get_attribute(Mesh::ATTRIBUTE_NORMAL).unwrap().iter().map(|normal| {
            let mut normal = Vec3::from(normal);

            normal.y = 0.0;

            normal.normalize()
        }).collect::<Vec<_>>());
        */
    }

    time.delta_seconds();
}