mod marching_cubes;
mod plugins;

use crate::plugins::{FlyCam, NoCameraPlayerPlugin};
use bevy::{
    app::App,
    asset::{Assets, Handle},
    core::Time,
    ecs::{
        entity::Entity,
        query::With,
        system::{Commands, Query, Res, ResMut},
    },
    log::*,
    math::Vec3,
    pbr2::{AmbientLight, PbrBundle, StandardMaterial},
    render2::{
        camera::PerspectiveCameraBundle,
        color::Color,
        mesh::{Indices, Mesh},
        render_resource::{MapMode, PrimitiveTopology, *},
        renderer::{RenderDevice, RenderQueue},
        shader::Shader,
    },
    tasks::{AsyncComputeTaskPool, Task},
    transform::components::Transform,
    window::WindowDescriptor,
    PipelinedDefaultPlugins,
};
use bytemuck;
use noise::{NoiseFn, Perlin, Seedable};

use futures_lite::future;

use crate::marching_cubes::*;

use bevy_inspector_egui::WorldInspectorPlugin;

pub struct WorldMesh;

fn main() {
    let perlin = Perlin::new();

    perlin.set_seed(5225);

    let read_buffer: Option<Buffer> = None;

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
        .insert_resource(read_buffer)
        .add_plugins(PipelinedDefaultPlugins)
        .add_plugin(WorldInspectorPlugin::new())
        .add_plugin(NoCameraPlayerPlugin)
        // .add_startup_system(setup)
        .add_startup_system(gpu_setup)
        // .add_system(update)
        .add_system(gpu_update)
        .run();
}

fn value_from_noise(noise: noise::Perlin, translation: Vec3) -> f32 {
    1.0 - (noise.get([
        translation.x as f64 / 50.0,
        translation.y as f64 / 50.0,
        translation.z as f64 / 50.0,
    ]) * 2.0) as f32
}

fn generate_triangles(perlin: &noise::Perlin, translation_offset: Vec3) -> Vec<Triangle> {
    let mut triangles: Vec<Triangle> = Vec::new();

    let cell_size = 2.5;
    let iso_level = 0.7;

    let number_of_cells = 20;

    let perlin = *perlin;

    for z in 0..number_of_cells {
        for y in 0..number_of_cells {
            for x in 0..number_of_cells {
                let translation = Vec3::new(
                    x as f32 * cell_size - number_of_cells as f32 / 2.0 * cell_size
                        + cell_size / 2.0,
                    y as f32 * cell_size - number_of_cells as f32 / 2.0 * cell_size
                        + cell_size / 2.0,
                    z as f32 * cell_size - number_of_cells as f32 / 2.0 * cell_size
                        + cell_size / 2.0,
                );

                let cell_points = [
                    translation + Vec3::new(-cell_size / 2.0, -cell_size / 2.0, -cell_size / 2.0),
                    translation + Vec3::new(cell_size / 2.0, -cell_size / 2.0, -cell_size / 2.0),
                    translation + Vec3::new(cell_size / 2.0, -cell_size / 2.0, cell_size / 2.0),
                    translation + Vec3::new(-cell_size / 2.0, -cell_size / 2.0, cell_size / 2.0),
                    translation + Vec3::new(-cell_size / 2.0, cell_size / 2.0, -cell_size / 2.0),
                    translation + Vec3::new(cell_size / 2.0, cell_size / 2.0, -cell_size / 2.0),
                    translation + Vec3::new(cell_size / 2.0, cell_size / 2.0, cell_size / 2.0),
                    translation + Vec3::new(-cell_size / 2.0, cell_size / 2.0, cell_size / 2.0),
                ];

                let cell_points = [
                    (
                        cell_points[0],
                        value_from_noise(perlin, cell_points[0] + translation_offset),
                    ),
                    (
                        cell_points[1],
                        value_from_noise(perlin, cell_points[1] + translation_offset),
                    ),
                    (
                        cell_points[2],
                        value_from_noise(perlin, cell_points[2] + translation_offset),
                    ),
                    (
                        cell_points[3],
                        value_from_noise(perlin, cell_points[3] + translation_offset),
                    ),
                    (
                        cell_points[4],
                        value_from_noise(perlin, cell_points[4] + translation_offset),
                    ),
                    (
                        cell_points[5],
                        value_from_noise(perlin, cell_points[5] + translation_offset),
                    ),
                    (
                        cell_points[6],
                        value_from_noise(perlin, cell_points[6] + translation_offset),
                    ),
                    (
                        cell_points[7],
                        value_from_noise(perlin, cell_points[7] + translation_offset),
                    ),
                ];
                triangles.append(&mut polygonise(cell_points, iso_level));
            }
        }
    }

    triangles
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    perlin: Res<Perlin>,
) {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);

    let triangles = generate_triangles(&perlin, Vec3::ZERO);

    let vertices = triangles
        .iter()
        .map(|triangle| [triangle.a, triangle.b, triangle.c])
        .flatten()
        .map(|vector| [vector.x, vector.y, vector.z])
        .collect::<Vec<_>>();
    let indices = (0..vertices.len())
        .map(|index| index as u32)
        .collect::<Vec<u32>>();
    let uvs = (0..vertices.len())
        .map(|_| [0.0, 0.0])
        .collect::<Vec<[f32; 2]>>();

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

    commands
        .spawn_bundle(PbrBundle {
            mesh: meshes.add(mesh),
            material: materials.add(StandardMaterial {
                base_color: Color::BLUE,
                ..Default::default()
            }),
            ..Default::default()
        })
        .insert(WorldMesh);

    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 1.0,
    });

    commands
        .spawn_bundle(PerspectiveCameraBundle {
            transform: Transform::from_xyz(-40.0, 40.0, 40.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..Default::default()
        })
        .insert(FlyCam);
}

fn gpu_setup(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    thread_pool: Res<AsyncComputeTaskPool>,
    mut read_buffer_option: ResMut<Option<Buffer>>,
) {
    let number_of_cells = 128;

    let output_buffer_size = number_of_cells * number_of_cells * number_of_cells * 5 * 3;
    let shader = Shader::from_wgsl(include_str!("../assets/shader.wgsl"));
    let shader_module = render_device.create_shader_module(&shader);

    let buffer = render_device.create_buffer(&BufferDescriptor {
        label: None,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
        size: output_buffer_size as BufferAddress,
    });

    let bind_group_layout = render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: None,
        entries: &[BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Storage { read_only: false },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });

    let bind_group = render_device.create_bind_group(&BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    });

    let pipeline_layout = render_device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: None,
        push_constant_ranges: &[],
        bind_group_layouts: &[&bind_group_layout],
    });

    let compute_pipeline = render_device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: None,
        layout: Some(&pipeline_layout),
        module: &shader_module,
        entry_point: "main",
    });

    let mut command_encoder =
        render_device.create_command_encoder(&CommandEncoderDescriptor { label: None });

    {
        let mut compute_pass =
            command_encoder.begin_compute_pass(&ComputePassDescriptor { label: None });
        compute_pass.set_pipeline(&compute_pipeline);
        compute_pass.set_bind_group(0, &*bind_group, &[]);

        compute_pass.dispatch(
            number_of_cells / 8,
            number_of_cells / 8,
            number_of_cells / 8,
        );
    }

    let read_buffer = render_device.create_buffer(&BufferDescriptor {
        label: None,
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
        size: output_buffer_size as BufferAddress,
    });

    command_encoder.copy_buffer_to_buffer(
        &buffer,
        0,
        &read_buffer,
        0,
        output_buffer_size as BufferAddress,
    );

    let gpu_commands = command_encoder.finish();

    render_queue.submit([gpu_commands]);

    let buffer_slice = read_buffer.slice(..);

    let buffer_future = buffer_slice.map_async(MapMode::Read);

    let task = thread_pool.spawn(buffer_future);

    commands.spawn().insert(task);

    *read_buffer_option = Some(read_buffer);
}

fn gpu_update(
    mut commands: Commands,
    mut compute_tasks: Query<(Entity, &mut Task<Result<(), BufferAsyncError>>)>,
    read_buffer: Res<Option<Buffer>>,
) {
    for (entity, mut task) in compute_tasks.iter_mut() {
        if let Some(_) = future::block_on(future::poll_once(&mut *task)) {
            if let Some(buffer) = &*read_buffer {
                let buffer_slice = buffer.slice(..);

                let data = buffer_slice.get_mapped_range();

                let result: &[f32] = bytemuck::cast_slice(&data);

                println!("{:?}", result[0]);

                drop(data);
                buffer.unmap();
            }

            commands
                .entity(entity)
                .remove::<Task<Result<(), BufferAsyncError>>>();
        }
    }
}

fn update(
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut query: Query<&mut Handle<Mesh>, With<WorldMesh>>,
    perlin: Res<Perlin>,
) {
    for mesh in &mut query.iter_mut() {
        let mesh = meshes.get_mut(&*mesh).unwrap();

        let triangles = generate_triangles(
            &perlin,
            time.seconds_since_startup() as f32 * Vec3::new(10.0, 10.0, 10.0),
        );

        let vertices = triangles
            .iter()
            .map(|triangle| [triangle.a, triangle.b, triangle.c])
            .flatten()
            .map(|vector| [vector.x, vector.y, vector.z])
            .collect::<Vec<_>>();
        let indices = (0..vertices.len())
            .map(|index| index as u32)
            .collect::<Vec<u32>>();
        let uvs = (0..vertices.len())
            .map(|_| [0.0, 0.0])
            .collect::<Vec<[f32; 2]>>();

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
    }

    time.delta_seconds();
}
