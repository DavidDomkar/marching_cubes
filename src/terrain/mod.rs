use bevy::{
    app::{App, Plugin},
    asset::Assets,
    core::bytes_of,
    ecs::{
        entity::Entity,
        system::{Commands, Query, Res, ResMut},
        world::{FromWorld, World},
    },
    math::{IVec3, Vec3},
    pbr2::{PbrBundle, StandardMaterial},
    render2::{
        color::Color,
        mesh::{Indices, Mesh},
        render_resource::{MapMode, PrimitiveTopology, *},
        renderer::{RenderDevice, RenderQueue},
        shader::Shader,
    },
    tasks::{AsyncComputeTaskPool, Task},
    transform::components::Transform,
};
use bytemuck::{Pod, Zeroable};

use crevice::std140::AsStd140;

use futures_lite::future;

const CHUNK_SIZE: u32 = 64;

#[repr(C)]
#[derive(Debug, AsStd140, Copy, Clone, Zeroable, Pod)]
struct Triangle {
    pub a: Vec3,
    pub b: Vec3,
    pub c: Vec3,
}

#[repr(C)]
#[derive(Debug, AsStd140, Copy, Clone, Zeroable, Pod)]
struct Cube {
    pub triangle_count: u32,
    pub triangles: [Triangle; 5],
}

#[repr(C)]
#[derive(Debug, AsStd140, Copy, Clone, Zeroable, Pod)]
struct InputBuffer {
    pub chunk_size: u32,
    pub position: Vec3,
}

pub struct TerrainPlugin;
impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TerrainShaders>();
        app.add_startup_system(setup_terrain);
        app.add_system(update_terrain);
    }
}

pub struct TerrainShaders {
    bind_group_layout: BindGroupLayout,
    compute_pipeline: ComputePipeline,
}

impl FromWorld for TerrainShaders {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.get_resource::<RenderDevice>().unwrap();

        let shader = Shader::from_wgsl(include_str!("../../assets/chunk.wgsl"));
        let shader_module = render_device.create_shader_module(&shader);

        let bind_group_layout =
            render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
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

        Self {
            bind_group_layout,
            compute_pipeline,
        }
    }
}

struct Chunk {
    buffer: Buffer,
    buffer_size: BufferAddress,
    position: IVec3,
}

impl Chunk {
    fn new(position: IVec3, render_device: &RenderDevice) -> Self {
        let buffer_size =
            CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE * (Cube::std140_size_static() as u32);

        let buffer = render_device.create_buffer(&BufferDescriptor {
            label: None,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
            size: buffer_size as BufferAddress,
        });

        Self {
            buffer: buffer,
            buffer_size: buffer_size as BufferAddress,
            position,
        }
    }

    fn create_compute_task(
        &self,
        render_device: &RenderDevice,
        render_queue: &RenderQueue,
        terrain_shaders: &TerrainShaders,
        task_pool: &AsyncComputeTaskPool,
    ) -> Task<Result<(), BufferAsyncError>> {
        let input_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            contents: bytes_of(
                &InputBuffer {
                    chunk_size: CHUNK_SIZE,
                    position: Vec3::new(
                        self.position.x as f32 * CHUNK_SIZE as f32 - (CHUNK_SIZE as f32 / 2.0),
                        self.position.y as f32 * CHUNK_SIZE as f32 - (CHUNK_SIZE as f32 / 2.0),
                        self.position.z as f32 * CHUNK_SIZE as f32 - (CHUNK_SIZE as f32 / 2.0),
                    ),
                }
                .as_std140(),
            ),
            label: None,
            usage: BufferUsages::STORAGE,
        });

        let output_buffer = render_device.create_buffer(&BufferDescriptor {
            label: None,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
            size: self.buffer_size,
        });

        let bind_group = render_device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &terrain_shaders.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: input_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: output_buffer.as_entire_binding(),
                },
            ],
        });

        let mut command_encoder =
            render_device.create_command_encoder(&CommandEncoderDescriptor { label: None });

        {
            let mut compute_pass =
                command_encoder.begin_compute_pass(&ComputePassDescriptor { label: None });
            compute_pass.set_pipeline(&terrain_shaders.compute_pipeline);
            compute_pass.set_bind_group(0, &*bind_group, &[]);

            compute_pass.dispatch(CHUNK_SIZE / 8, CHUNK_SIZE / 8, CHUNK_SIZE / 8);
        }

        command_encoder.copy_buffer_to_buffer(&output_buffer, 0, &self.buffer, 0, self.buffer_size);

        let gpu_commands = command_encoder.finish();

        render_queue.submit([gpu_commands]);

        let buffer_slice = self.buffer.slice(..);

        let buffer_future = buffer_slice.map_async(MapMode::Read);
        let task = task_pool.spawn(buffer_future);

        task
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
    terrain_shaders: Res<TerrainShaders>,
    task_pool: Res<AsyncComputeTaskPool>,
) {
    for x in 0..1 {
        for y in 0..1 {
            for z in 0..1 {
                let chunk = Chunk::new(IVec3::new(x, y, z), &*render_device);

                let task = chunk.create_compute_task(
                    &*render_device,
                    &*render_queue,
                    &*terrain_shaders,
                    &*task_pool,
                );
                commands.spawn().insert(task).insert(chunk);
            }
        }
    }
}

fn update_terrain(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut chunks: Query<(Entity, &Chunk, &mut Task<Result<(), BufferAsyncError>>)>,
) {
    for (entity, chunk, mut task) in chunks.iter_mut() {
        if let Some(_) = future::block_on(future::poll_once(&mut *task)) {
            let mesh = chunk.create_mesh();

            commands.entity(entity).insert_bundle(PbrBundle {
                mesh: meshes.add(mesh),
                material: materials.add(StandardMaterial {
                    base_color: Color::BLUE,
                    ..Default::default()
                }),
                transform: Transform::from_xyz(
                    chunk.position.x as f32 * CHUNK_SIZE as f32 - (CHUNK_SIZE as f32 / 2.0),
                    chunk.position.y as f32 * CHUNK_SIZE as f32 - (CHUNK_SIZE as f32 / 2.0),
                    chunk.position.z as f32 * CHUNK_SIZE as f32 - (CHUNK_SIZE as f32 / 2.0),
                ),
                ..Default::default()
            });

            commands
                .entity(entity)
                .remove::<Task<Result<(), BufferAsyncError>>>();
        }
    }
}
