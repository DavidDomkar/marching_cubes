mod marching_cubes;
mod plugins;
mod terrain;

use crate::{
    plugins::{FlyCam, NoCameraPlayerPlugin},
    terrain::TerrainPlugin,
};
use bevy::{
    pbr2::{DirectionalLight, DirectionalLightBundle},
    render2::camera::OrthographicProjection,
};

use bevy::{
    app::App,
    ecs::system::Commands,
    log::*,
    math::Vec3,
    pbr2::AmbientLight,
    render2::{camera::PerspectiveCameraBundle, color::Color},
    transform::components::Transform,
    window::WindowDescriptor,
    PipelinedDefaultPlugins,
};

use bevy_inspector_egui::WorldInspectorPlugin;

fn main() {
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
        .add_plugins(PipelinedDefaultPlugins)
        .add_plugin(WorldInspectorPlugin::new())
        .add_plugin(NoCameraPlayerPlugin)
        .add_plugin(TerrainPlugin)
        .add_startup_system(setup_environment)
        .run();
}

fn setup_environment(mut commands: Commands) {
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

    commands.spawn_bundle(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 100000.0,
            shadow_projection: OrthographicProjection {
                left: -10.0,
                right: 10.0,
                bottom: -10.0,
                top: 10.0,
                near: -50.0,
                far: 50.0,
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    });
}
