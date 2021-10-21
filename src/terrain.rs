use crate::marching_cubes::{polygonise, Triangle as OtherTriangle};
use bevy::render2::render_resource::{
    BindGroupDescriptor, BindGroupEntry, CommandEncoderDescriptor, ComputePassDescriptor,
};

use bytemuck::{Pod, Zeroable};

use crevice::std140::AsStd140;

use bevy::{
    app::{App, Plugin},
    asset::Assets,
    core::{bytes_of, Time},
    ecs::{
        entity::Entity,
        schedule::SystemLabel,
        system::{Commands, Query, Res, ResMut},
        world::{FromWorld, World},
    },
    math::Vec3,
    pbr2::{PbrBundle, StandardMaterial},
    prelude::ParallelSystemDescriptorCoercion,
    render2::{
        camera::Camera,
        color::Color,
        mesh::{Indices, Mesh},
        render_resource::{
            BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType,
            BufferAddress, BufferBindingType, BufferDescriptor, BufferInitDescriptor, BufferUsages,
            ComputePipeline, ComputePipelineDescriptor, MapMode, PipelineLayoutDescriptor,
            PrimitiveTopology, ShaderStages,
        },
        renderer::{RenderDevice, RenderQueue},
        shader::Shader,
    },
    tasks::{AsyncComputeTaskPool, Task},
    transform::components::Transform,
};

use std::collections::{HashMap, HashSet};

use noise::{NoiseFn, Perlin, Seedable, SuperSimplex};

use futures_lite::future;

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemLabel)]
enum TerrainSystemLabels {
    UpdateChunks,
}

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
        app.insert_resource(Terrain::new());
        app.add_system(update_chunks.label(TerrainSystemLabels::UpdateChunks));
        app.add_system(handle_terrain_chunk_tasks.after(TerrainSystemLabels::UpdateChunks));
    }
}

struct Terrain {
    chunk_view_distance: u32,
    chunk_size: u32,
    chunks: HashMap<(i32, i32, i32), Entity>,
}

impl Terrain {
    fn new() -> Self {
        Self {
            chunk_view_distance: 10,
            chunk_size: 64,
            chunks: HashMap::new(),
        }
    }

    fn get_chunk_coords_at_translation(&self, translation: &Vec3) -> (i32, i32, i32) {
        (
            (translation.x / self.chunk_size as f32).round() as i32,
            (translation.y / self.chunk_size as f32).round() as i32,
            (translation.z / self.chunk_size as f32).round() as i32,
        )
    }

    fn get_chunk(&self, x: i32, y: i32, z: i32) -> Option<&Entity> {
        self.chunks.get(&(x, y, z))
    }

    fn has_chunk(&self, x: i32, y: i32, z: i32) -> bool {
        self.chunks.contains_key(&(x, y, z))
    }

    fn set_chunk(&mut self, x: i32, y: i32, z: i32, chunk: Entity) {
        self.chunks.insert((x, y, z), chunk);
    }

    fn remove_chunk(&mut self, x: i32, y: i32, z: i32) {
        self.chunks.remove(&(x, y, z));
    }
}

pub struct TerrainChunk {
    coords: (i32, i32, i32),
}

impl TerrainChunk {
    fn value_from_noise(noise: SuperSimplex, translation: Vec3) -> f32 {
        1.0 - (noise.get([
            translation.x as f64 / 32.0,
            translation.y as f64 / 32.0,
            translation.z as f64 / 32.0,
        ]) * 2.0) as f32
    }

    fn generate_mesh(chunk_coords: (i32, i32, i32), chunk_size: u32) -> Mesh {
        let noise = SuperSimplex::new();

        noise.set_seed(5225);

        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);

        let mut triangles: Vec<OtherTriangle> = Vec::new();

        let cell_size = 1.0;
        let iso_level = 0.7;

        let translation_offset = Vec3::new(
            chunk_coords.0 as f32 * chunk_size as f32 - chunk_size as f32 / 2.0,
            chunk_coords.1 as f32 * chunk_size as f32 - chunk_size as f32 / 2.0,
            chunk_coords.2 as f32 * chunk_size as f32 - chunk_size as f32 / 2.0,
        );

        for z in 0..chunk_size {
            for y in 0..chunk_size {
                for x in 0..chunk_size {
                    let translation = Vec3::new(
                        x as f32 * cell_size - chunk_size as f32 / 2.0 * cell_size
                            + cell_size / 2.0,
                        y as f32 * cell_size - chunk_size as f32 / 2.0 * cell_size
                            + cell_size / 2.0,
                        z as f32 * cell_size - chunk_size as f32 / 2.0 * cell_size
                            + cell_size / 2.0,
                    );

                    let cell_points = [
                        translation
                            + Vec3::new(-cell_size / 2.0, -cell_size / 2.0, -cell_size / 2.0),
                        translation
                            + Vec3::new(cell_size / 2.0, -cell_size / 2.0, -cell_size / 2.0),
                        translation + Vec3::new(cell_size / 2.0, -cell_size / 2.0, cell_size / 2.0),
                        translation
                            + Vec3::new(-cell_size / 2.0, -cell_size / 2.0, cell_size / 2.0),
                        translation
                            + Vec3::new(-cell_size / 2.0, cell_size / 2.0, -cell_size / 2.0),
                        translation + Vec3::new(cell_size / 2.0, cell_size / 2.0, -cell_size / 2.0),
                        translation + Vec3::new(cell_size / 2.0, cell_size / 2.0, cell_size / 2.0),
                        translation + Vec3::new(-cell_size / 2.0, cell_size / 2.0, cell_size / 2.0),
                    ];

                    let cell_points = [
                        (
                            cell_points[0],
                            Self::value_from_noise(noise, cell_points[0] + translation_offset),
                        ),
                        (
                            cell_points[1],
                            Self::value_from_noise(noise, cell_points[1] + translation_offset),
                        ),
                        (
                            cell_points[2],
                            Self::value_from_noise(noise, cell_points[2] + translation_offset),
                        ),
                        (
                            cell_points[3],
                            Self::value_from_noise(noise, cell_points[3] + translation_offset),
                        ),
                        (
                            cell_points[4],
                            Self::value_from_noise(noise, cell_points[4] + translation_offset),
                        ),
                        (
                            cell_points[5],
                            Self::value_from_noise(noise, cell_points[5] + translation_offset),
                        ),
                        (
                            cell_points[6],
                            Self::value_from_noise(noise, cell_points[6] + translation_offset),
                        ),
                        (
                            cell_points[7],
                            Self::value_from_noise(noise, cell_points[7] + translation_offset),
                        ),
                    ];
                    triangles.append(&mut polygonise(cell_points, iso_level));
                }
            }
        }

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

fn update_chunks(
    mut commands: Commands,
    mut terrain: ResMut<Terrain>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    task_pool: Res<AsyncComputeTaskPool>,
    camera_query: Query<(&Camera, &Transform)>,
    terrain_chunks_query: Query<(Entity, &TerrainChunk)>,
) {
    let mut visible_chunk_coords: HashSet<(i32, i32, i32)> = HashSet::new();

    for (_, transform) in camera_query.iter() {
        let (center_x, center_y, center_z) =
            terrain.get_chunk_coords_at_translation(&transform.translation);

        let chunk_view_distance = terrain.chunk_view_distance as i32;

        for y in -chunk_view_distance..chunk_view_distance + 1 {
            let mut z = 0;

            {
                let mut x = 0;

                visible_chunk_coords.insert((center_x, center_y + y, center_z));

                x += 1;

                loop {
                    let cubic_distance_from_center = x * x + z * z + y * y;
                    let squared_view_distance = chunk_view_distance * chunk_view_distance;
                    if cubic_distance_from_center > squared_view_distance {
                        break;
                    }

                    visible_chunk_coords.insert((center_x + x, center_y + y, center_z));
                    visible_chunk_coords.insert((center_x - x, center_y + y, center_z));

                    x += 1;
                }
            }

            z += 1;

            loop {
                let squared_distance_from_center = z * z + y * y;
                let squared_view_distance = chunk_view_distance * chunk_view_distance;
                if squared_distance_from_center > squared_view_distance {
                    break;
                }

                {
                    let mut x = 0;
                    visible_chunk_coords.insert((center_x + x, center_y + y, center_z + z));
                    x += 1;
                    loop {
                        let cubic_distance_from_center = x * x + z * z + y * y;
                        let squared_view_distance = chunk_view_distance;
                        if cubic_distance_from_center > squared_view_distance {
                            break;
                        }

                        visible_chunk_coords.insert((center_x + x, center_y + y, center_z + z));
                        visible_chunk_coords.insert((center_x - x, center_y + y, center_z + z));
                        x += 1;
                    }
                }

                {
                    let mut x = 0;
                    visible_chunk_coords.insert((center_x + x, center_y + y, center_z - z));
                    x += 1;
                    loop {
                        let cubic_distance_from_center = x * x + z * z + y * y;
                        let squared_view_distance = chunk_view_distance;
                        if cubic_distance_from_center > squared_view_distance {
                            break;
                        }
                        visible_chunk_coords.insert((center_x + x, center_y + y, center_z - z));
                        visible_chunk_coords.insert((center_x - x, center_y + y, center_z - z));
                        x += 1;
                    }
                }

                z += 1;
            }
        }
    }

    for (entity, terrain_chunk) in terrain_chunks_query.iter() {
        if !visible_chunk_coords.contains(&terrain_chunk.coords) {
            terrain.remove_chunk(
                terrain_chunk.coords.0,
                terrain_chunk.coords.1,
                terrain_chunk.coords.2,
            );

            let mut entity = commands.entity(entity);

            entity.despawn();
        } else {
            visible_chunk_coords.remove(&terrain_chunk.coords);
        }
    }

    let chunk_size = terrain.chunk_size;

    for (x, y, z) in visible_chunk_coords {
        let render_device = render_device.clone();
        let render_queue = render_queue.clone();

        let task = task_pool.spawn(async move {
            let buffer_size =
                (chunk_size * chunk_size * chunk_size * (Cube::std140_size_static() as u32))
                    as BufferAddress;

            let shader = Shader::from_wgsl(include_str!("../assets/chunk.wgsl"));
            let shader_module = render_device.create_shader_module(&shader);

            let input_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
                contents: bytes_of(
                    &InputBuffer {
                        chunk_size: chunk_size,
                        position: Vec3::new(
                            x as f32 * chunk_size as f32 - (chunk_size as f32 / 2.0),
                            y as f32 * chunk_size as f32 - (chunk_size as f32 / 2.0),
                            z as f32 * chunk_size as f32 - (chunk_size as f32 / 2.0),
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
                size: buffer_size,
            });

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

            let compute_pipeline =
                render_device.create_compute_pipeline(&ComputePipelineDescriptor {
                    label: None,
                    layout: Some(&pipeline_layout),
                    module: &shader_module,
                    entry_point: "main",
                });

            let bind_group = render_device.create_bind_group(&BindGroupDescriptor {
                label: None,
                layout: &bind_group_layout,
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
                compute_pass.set_pipeline(&compute_pipeline);
                compute_pass.set_bind_group(0, &*bind_group, &[]);

                compute_pass.dispatch(chunk_size / 8, chunk_size / 8, chunk_size / 8);
            }

            let buffer = render_device.create_buffer(&BufferDescriptor {
                label: None,
                usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
                mapped_at_creation: false,
                size: buffer_size,
            });

            command_encoder.copy_buffer_to_buffer(&output_buffer, 0, &buffer, 0, buffer_size);

            let gpu_commands = command_encoder.finish();

            render_queue.submit([gpu_commands]);

            let buffer_slice = buffer.slice(..);

            let buffer_future = buffer_slice.map_async(MapMode::Read);

            let result = buffer_future.await;

            let mut triangles: Vec<Triangle> = Vec::new();

            if let Ok(_) = result {
                let buffer_data = buffer_slice.get_mapped_range();

                let cubes: &[Std140Cube] = bytemuck::cast_slice(&buffer_data);

                for cube in cubes.iter() {
                    let cube = Cube::from_std140(*cube);

                    for i in 0..cube.triangle_count {
                        triangles.push(cube.triangles[i as usize]);
                    }
                }

                drop(buffer_data);
            }

            buffer.unmap();
            buffer.destroy();

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

            // let mesh = TerrainChunk::generate_mesh((x, y, z), chunk_size);

            let material = StandardMaterial {
                base_color: Color::BLUE,
                perceptual_roughness: 1.0,
                ..Default::default()
            };

            (mesh, material)
        });

        let chunk_entity = commands
            .spawn()
            .insert(TerrainChunk { coords: (x, y, z) })
            .insert(task)
            .id();

        terrain.set_chunk(x, y, z, chunk_entity);
    }
}

fn handle_terrain_chunk_tasks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    terrain: Res<Terrain>,
    mut terrain_chunk_tasks: Query<(Entity, &TerrainChunk, &mut Task<(Mesh, StandardMaterial)>)>,
) {
    for (entity, chunk, mut task) in terrain_chunk_tasks.iter_mut() {
        if let Some((mesh, material)) = future::block_on(future::poll_once(&mut *task)) {
            let material = materials.add(material);

            if terrain.has_chunk(chunk.coords.0, chunk.coords.1, chunk.coords.2) {
                commands.entity(entity).insert_bundle(PbrBundle {
                    mesh: meshes.add(mesh),
                    material: material.clone(),
                    transform: Transform::from_xyz(
                        chunk.coords.0 as f32 * terrain.chunk_size as f32,
                        chunk.coords.1 as f32 * terrain.chunk_size as f32,
                        chunk.coords.2 as f32 * terrain.chunk_size as f32,
                    ),
                    ..Default::default()
                });

                commands
                    .entity(entity)
                    .remove::<Task<(Mesh, StandardMaterial)>>();
            }
        }
    }
}
