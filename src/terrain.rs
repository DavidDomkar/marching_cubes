use crate::marching_cubes::{polygonise, Triangle};

use bevy::{
    app::{App, Plugin},
    asset::Assets,
    ecs::{
        entity::Entity,
        system::{Commands, Query, Res, ResMut},
    },
    math::Vec3,
    pbr2::{PbrBundle, StandardMaterial},
    render2::{
        camera::Camera,
        color::Color,
        mesh::{shape::Plane, Indices, Mesh},
        render_resource::PrimitiveTopology,
    },
    tasks::{AsyncComputeTaskPool, Task},
    transform::components::Transform,
};

use std::collections::{HashMap, HashSet};

use noise::{NoiseFn, OpenSimplex, Perlin, Seedable};

use futures_lite::future;

pub struct TerrainPlugin;
impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Terrain::new());
        app.add_system(update_chunks);
        app.add_system(handle_terrain_chunk_tasks);
    }
}

struct Terrain {
    chunk_view_distance: u32,
    chunk_size: u32,
    chunks: HashMap<(i32, i32), Entity>,
}

impl Terrain {
    fn new() -> Self {
        Self {
            chunk_view_distance: 20,
            chunk_size: 64,
            chunks: HashMap::new(),
        }
    }

    fn get_chunk_coords_at_translation(&self, translation: &Vec3) -> (i32, i32) {
        (
            (translation.x / self.chunk_size as f32).round() as i32,
            (translation.z / self.chunk_size as f32).round() as i32,
        )
    }

    fn get_chunk(&self, x: i32, z: i32) -> Option<&Entity> {
        self.chunks.get(&(x, z))
    }

    fn has_chunk(&self, x: i32, z: i32) -> bool {
        self.chunks.contains_key(&(x, z))
    }

    fn set_chunk(&mut self, x: i32, z: i32, chunk: Entity) {
        self.chunks.insert((x, z), chunk);
    }

    fn remove_chunk(&mut self, x: i32, z: i32) {
        self.chunks.remove(&(x, z));
    }
}

pub struct TerrainChunk {
    coords: (i32, i32),
}

impl TerrainChunk {
    fn value_from_noise(noise: Perlin, translation: Vec3) -> f32 {
        1.0 - (noise.get([
            translation.x as f64 / 100.0,
            translation.y as f64 / 100.0,
            translation.z as f64 / 100.0,
        ]) * 2.0) as f32
    }

    fn generate_mesh(chunk_coords: (i32, i32), chunk_size: u32) -> Mesh {
        let noise = Perlin::new();

        noise.set_seed(5225);

        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);

        let mut triangles: Vec<Triangle> = Vec::new();

        let cell_size = 1.0;
        let iso_level = 0.7;

        let translation_offset = Vec3::new(
            chunk_coords.0 as f32 * chunk_size as f32 - chunk_size as f32 / 2.0,
            0.0,
            chunk_coords.1 as f32 * chunk_size as f32 - chunk_size as f32 / 2.0,
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
    task_pool: Res<AsyncComputeTaskPool>,
    camera_query: Query<(&Camera, &Transform)>,
    terrain_chunks_query: Query<(Entity, &TerrainChunk)>,
) {
    let mut visible_chunk_coords: HashSet<(i32, i32)> = HashSet::new();

    for (_, transform) in camera_query.iter() {
        let (center_x, center_z) = terrain.get_chunk_coords_at_translation(&transform.translation);

        let chunk_view_distance = terrain.chunk_view_distance as i32;

        for z in -chunk_view_distance..chunk_view_distance + 1 {
            let mut x = 0;

            visible_chunk_coords.insert((center_x + x, center_z + z));

            loop {
                let squared_distance_from_center = z * z + x * x;
                let squared_view_distance = chunk_view_distance * chunk_view_distance;

                if squared_distance_from_center > squared_view_distance {
                    break;
                }

                visible_chunk_coords.insert((center_x + x, center_z + z));
                visible_chunk_coords.insert((center_x - x, center_z + z));

                x += 1;
            }
        }
    }

    for (entity, terrain_chunk) in terrain_chunks_query.iter() {
        if !visible_chunk_coords.contains(&terrain_chunk.coords) {
            terrain.remove_chunk(terrain_chunk.coords.0, terrain_chunk.coords.1);

            let mut entity = commands.entity(entity);

            entity.despawn();
        } else {
            visible_chunk_coords.remove(&terrain_chunk.coords);
        }
    }

    let chunk_size = terrain.chunk_size;

    for (x, z) in visible_chunk_coords {
        let task = task_pool.spawn(async move {
            let mesh = TerrainChunk::generate_mesh((x, z), chunk_size);

            let material = StandardMaterial {
                base_color: Color::BLUE,
                ..Default::default()
            };

            (mesh, material)
        });

        let chunk_entity = commands
            .spawn()
            .insert(TerrainChunk { coords: (x, z) })
            .insert(task)
            .id();

        terrain.set_chunk(x, z, chunk_entity);
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

            commands.entity(entity).insert_bundle(PbrBundle {
                mesh: meshes.add(mesh),
                material: material.clone(),
                transform: Transform::from_xyz(
                    chunk.coords.0 as f32 * terrain.chunk_size as f32,
                    0.0,
                    chunk.coords.1 as f32 * terrain.chunk_size as f32,
                ),
                ..Default::default()
            });

            commands
                .entity(entity)
                .remove::<Task<(Mesh, StandardMaterial)>>();
        }
    }
}
