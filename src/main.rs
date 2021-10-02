mod marching_cubes;
mod plugins;

use noise::OpenSimplex;
use crate::plugins::NoCameraPlayerPlugin;
use crate::plugins::FlyCam;
use bevy::render2::camera::PerspectiveCameraBundle;
use bevy::math::Vec3;
use bevy::transform::components::Transform;
use bevy::render2::mesh::shape::UVSphere;
use bevy::render2::mesh::Indices;
use noise::{NoiseFn, Perlin};

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

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);

    let triangles = generate_mesh(100.0, 100.0, 100.0, 0.1);

    /*
    let triangles = polygonise([
        (Vec3::new(-5.0, -5.0, -5.0), 5.0),
        (Vec3::new(5.0, -5.0, -5.0), 5.0),
        (Vec3::new(5.0, -5.0, 5.0), 0.0),
        (Vec3::new(-5.0, -5.0, 5.0), 5.0),
        (Vec3::new(-5.0, 5.0, -5.0), 0.0),
        (Vec3::new(5.0, 5.0, -5.0), 5.0),
        (Vec3::new(5.0, 5.0, 5.0), 0.0),
        (Vec3::new(-5.0, 5.0, 5.0), 5.0),
    ], 3.0);
    */

    let vertices = triangles.iter().map(|triangle| { [triangle.a, triangle.b, triangle.c] }).flatten().map(|vector| { [vector.x, vector.y, vector.z] }).collect::<Vec<_>>();
    let indices = (0..vertices.len()).map(|index| { index as u32 }).collect::<Vec<u32>>();
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
            base_color: Color::PINK,
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

fn generate_mesh(x: f32, y: f32, z: f32, iso_level: f32) -> Vec<Triangle> {
    let cell_size = 5.0;

    let perlin = OpenSimplex::new();

    let mut triangles: Vec<Triangle> = Vec::new();

    let mut current_y = -y / 2.0 + cell_size / 2.0; 

    while current_y < y / 2.0 - cell_size / 2.0 {

        let mut current_z = -z / 2.0 + cell_size / 2.0;

        while current_z < z / 2.0 - cell_size / 2.0 {

            let mut current_x = -x / 2.0 + cell_size / 2.0; 

            while current_x < x / 2.0 - cell_size / 2.0 {

                /*
                triangles.append(&mut polygonise([
                    (Vec3::new(current_x + -5.0, current_y + -5.0, current_z + -5.0), 5.0),
                    (Vec3::new(current_x + 5.0, current_y + -5.0, current_z + -5.0), 5.0),
                    (Vec3::new(current_x + 5.0, current_y + -5.0, current_z + 5.0), 0.0),
                    (Vec3::new(current_x + -5.0, current_y + -5.0, current_z + 5.0), 5.0),
                    (Vec3::new(current_x + -5.0, current_y + 5.0, current_z + -5.0), 0.0),
                    (Vec3::new(current_x + 5.0, current_y + 5.0, current_z + -5.0), 5.0),
                    (Vec3::new(current_x + 5.0, current_y + 5.0, current_z + 5.0), 0.0),
                    (Vec3::new(current_x + -5.0, current_y + 5.0,current_z +  5.0), 5.0),
                ], 3.0));
                */

                triangles.append(&mut polygonise([
                    (Vec3::new(current_x - cell_size / 2.0, current_y - cell_size / 2.0, current_z - cell_size / 2.0), perlin.get([(current_x - cell_size / 2.0) as f64, (current_y - cell_size / 2.0) as f64, (current_z - cell_size / 2.0) as f64]) as f32),
                    (Vec3::new(current_x + cell_size / 2.0, current_y - cell_size / 2.0, current_z - cell_size / 2.0), perlin.get([(current_x + cell_size / 2.0) as f64, (current_y - cell_size / 2.0) as f64, (current_z - cell_size / 2.0) as f64]) as f32),
                    (Vec3::new(current_x + cell_size / 2.0, current_y - cell_size / 2.0, current_z + cell_size / 2.0), perlin.get([(current_x + cell_size / 2.0) as f64, (current_y - cell_size / 2.0) as f64, (current_z + cell_size / 2.0) as f64]) as f32),
                    (Vec3::new(current_x - cell_size / 2.0, current_y - cell_size / 2.0, current_z + cell_size / 2.0), perlin.get([(current_x - cell_size / 2.0) as f64, (current_y - cell_size / 2.0) as f64, (current_z + cell_size / 2.0) as f64]) as f32),
                    (Vec3::new(current_x - cell_size / 2.0, current_y + cell_size / 2.0, current_z - cell_size / 2.0), perlin.get([(current_x - cell_size / 2.0) as f64, (current_y + cell_size / 2.0) as f64, (current_z - cell_size / 2.0) as f64]) as f32),
                    (Vec3::new(current_x + cell_size / 2.0, current_y + cell_size / 2.0, current_z - cell_size / 2.0), perlin.get([(current_x + cell_size / 2.0) as f64, (current_y + cell_size / 2.0) as f64, (current_z - cell_size / 2.0) as f64]) as f32),
                    (Vec3::new(current_x + cell_size / 2.0, current_y + cell_size / 2.0, current_z + cell_size / 2.0), perlin.get([(current_x + cell_size / 2.0) as f64, (current_y + cell_size / 2.0) as f64, (current_z + cell_size / 2.0) as f64]) as f32),
                    (Vec3::new(current_x - cell_size / 2.0, current_y + cell_size / 2.0, current_z + cell_size / 2.0), perlin.get([(current_x - cell_size / 2.0) as f64, (current_y + cell_size / 2.0) as f64, (current_z + cell_size / 2.0) as f64]) as f32),
                ], iso_level));

                // println!("{:?}", perlin.get([(current_x - 5.0) as f64, (current_y - 5.0) as f64, (current_z - 5.0) as f64]));

                current_x += cell_size;
            }

            current_z += cell_size;
        }

        current_y += cell_size;
    }

    triangles
}