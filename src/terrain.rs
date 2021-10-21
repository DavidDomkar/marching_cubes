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
        mesh::{shape::Plane, Mesh},
    },
    transform::components::Transform,
};

use std::collections::{HashMap, HashSet};

pub struct TerrainPlugin;
impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Terrain::new());
        app.add_system(update_chunks);
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
            chunk_view_distance: 5,
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

fn update_chunks(
    mut commands: Commands,
    mut terrain: ResMut<Terrain>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
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
            commands.entity(entity).despawn();
        } else {
            visible_chunk_coords.remove(&terrain_chunk.coords);
        }
    }

    for (x, z) in visible_chunk_coords {
        let chunk_entity = commands
            .spawn_bundle(PbrBundle {
                mesh: meshes.add(Mesh::from(Plane {
                    size: terrain.chunk_size as f32,
                })),
                material: materials.add(StandardMaterial {
                    base_color: Color::BLUE,
                    ..Default::default()
                }),
                transform: Transform::from_xyz(
                    x as f32 * terrain.chunk_size as f32,
                    0.0,
                    z as f32 * terrain.chunk_size as f32,
                ),
                ..Default::default()
            })
            .insert(TerrainChunk { coords: (x, z) })
            .id();

        terrain.set_chunk(x, z, chunk_entity);
    }
}
