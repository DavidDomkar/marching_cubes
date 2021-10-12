use crate::{Cube, Std140Cube, Triangle};
use bevy::{
    app::{App, Plugin},
    ecs::{
        entity::Entity,
        system::{Commands, Query, Res},
        world::{FromWorld, World},
    },
    math::Vec3,
    render2::{
        camera::PerspectiveCameraBundle,
        color::Color,
        mesh::{Indices, Mesh},
        render_resource::{MapMode, PrimitiveTopology, *},
        renderer::{RenderDevice, RenderQueue},
        shader::Shader,
    },
    tasks::{AsyncComputeTaskPool, Task},
};
use std::collections::HashMap;

use crevice::std140::AsStd140;

use futures_lite::future;

const CHUNK_SIZE: u32 = 64;

pub struct TerrainPlugin;
impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TerrainComputePipeline>();
        app.add_startup_system(setup_terrain);
        app.add_system(update_terrain);
    }
}

pub struct TerrainComputePipeline {
    bind_group_layout: BindGroupLayout,
    compute_pipeline: ComputePipeline,
}

impl FromWorld for TerrainComputePipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.get_resource::<RenderDevice>().unwrap();

        let shader = Shader::from_wgsl(include_str!("../../assets/terrain.wgsl"));
        let shader_module = render_device.create_shader_module(&shader);

        let bind_group_layout =
            render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
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

        TerrainComputePipeline {
            bind_group_layout,
            compute_pipeline,
        }
    }
}

struct Chunk {
    buffer: Buffer,
    compute_task: Task<Result<(), BufferAsyncError>>,
}

impl Chunk {
    fn new(
        render_device: &RenderDevice,
        render_queue: &RenderQueue,
        terrain_compute_pipeline: &TerrainComputePipeline,
        task_pool: &AsyncComputeTaskPool,
    ) -> Self {
        let buffer_size =
            CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE * (Cube::std140_size_static() as u32);

        let gpu_buffer = render_device.create_buffer(&BufferDescriptor {
            label: None,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
            size: buffer_size as BufferAddress,
        });

        let bind_group = render_device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &terrain_compute_pipeline.bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: gpu_buffer.as_entire_binding(),
            }],
        });

        let mut command_encoder =
            render_device.create_command_encoder(&CommandEncoderDescriptor { label: None });

        {
            let mut compute_pass =
                command_encoder.begin_compute_pass(&ComputePassDescriptor { label: None });
            compute_pass.set_pipeline(&terrain_compute_pipeline.compute_pipeline);
            compute_pass.set_bind_group(0, &*bind_group, &[]);

            compute_pass.dispatch(CHUNK_SIZE / 8, CHUNK_SIZE / 8, CHUNK_SIZE / 8);
        }

        let buffer = render_device.create_buffer(&BufferDescriptor {
            label: None,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
            size: buffer_size as BufferAddress,
        });

        command_encoder.copy_buffer_to_buffer(
            &gpu_buffer,
            0,
            &buffer,
            0,
            buffer_size as BufferAddress,
        );

        let gpu_commands = command_encoder.finish();

        render_queue.submit([gpu_commands]);

        let buffer_slice = buffer.slice(..);

        let buffer_future = buffer_slice.map_async(MapMode::Read);
        let task = task_pool.spawn(buffer_future);

        Self {
            buffer: buffer,
            compute_task: task,
        }
    }

    fn create_mesh(&self) -> Mesh {
        let buffer_slice = self.buffer.slice(..);

        let data = buffer_slice.get_mapped_range();

        let cubes: &[Std140Cube] = bytemuck::cast_slice(&data);

        let mut triangles: Vec<Triangle> = Vec::new();

        for cube in cubes.iter() {
            let cube = Cube::from_std140(*cube);

            for i in 0..cube.triangle_count {
                triangles.push(cube.triangles[i as usize]);
            }
        }

        drop(data);
        self.buffer.unmap();
        self.buffer.destroy();

        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);

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

        mesh
    }
}

fn setup_terrain(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    terrain_compute_pipeline: Res<TerrainComputePipeline>,
    task_pool: Res<AsyncComputeTaskPool>,
) {
    commands.spawn().insert(Chunk::new(
        &*render_device,
        &*render_queue,
        &*terrain_compute_pipeline,
        &*task_pool,
    ));
}

fn update_terrain(mut commands: Commands, mut chunks: Query<(Entity, &mut Chunk)>) {
    for (entity, mut chunk) in chunks.iter_mut() {
        if let Some(_) = future::block_on(future::poll_once(&mut chunk.compute_task)) {
            let mesh = chunk.create_mesh();

            commands.entity(entity).insert(mesh);
        }
    }
}
