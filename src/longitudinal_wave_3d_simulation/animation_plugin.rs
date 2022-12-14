use std::f32::consts::{PI, TAU};

use bevy::prelude::*;
use bevy::time::Stopwatch;
use bevy_rapier3d::prelude::*;

use crate::pan_orbit_camera::{update_pan_orbit_camera, PanOrbitCamera};
use crate::{AppCamera, AppState};

use super::{LongitudinalWave3dSimulationParameters, UiEvents};

#[derive(Default, Resource)]
struct Entities(Vec<Entity>);

#[derive(Resource)]
struct AnimationTimer(Stopwatch);

#[derive(Component)]
struct Particle {
    initial_translation: Vec3,
}

#[derive(Component)]
struct ApplyingForce;

pub struct AnimationPlugin;

impl Plugin for AnimationPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Entities::default())
            .insert_resource(AnimationTimer(Stopwatch::new()))
            .add_system_set(
                SystemSet::on_enter(AppState::LongitudinalWaveSimulation3d)
                    .with_system(setup),
            )
            .add_system_set(
                SystemSet::on_update(AppState::LongitudinalWaveSimulation3d)
                    .with_system(update_pan_orbit_camera)
                    .with_system(apply_impulse)
                    .with_system(apply_equilibrium_force)
                    .with_system(on_ui_events),
            )
            .add_system_set(
                SystemSet::on_exit(AppState::LongitudinalWaveSimulation3d)
                    .with_system(cleanup),
            );
    }
}

#[allow(clippy::too_many_arguments)]
fn setup(
    mut time: ResMut<Time>,
    mut commands: Commands,
    cameras: Query<Entity, With<AppCamera>>,
    mut mouse_button: ResMut<Input<MouseButton>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    parameters: Res<LongitudinalWave3dSimulationParameters>,
    mut entities: ResMut<Entities>,
    mut rapier_debug_config: ResMut<DebugRenderContext>,
    mut rapier_config: ResMut<RapierConfiguration>,
) {
    rapier_debug_config.enabled = true;
    rapier_config.gravity = Vec3::ZERO;

    mouse_button.reset_all();

    time.pause();

    if let Ok(camera_entity) = cameras.get_single() {
        commands.entity(camera_entity).despawn();
    }

    let max_x_z = parameters.dimx.max(parameters.dimz) as f32 * 2.0;

    let plane = commands.spawn((
        PbrBundle {
            mesh: meshes.add(Mesh::from(shape::Plane {
                size: max_x_z * 2.0,
            })),
            material: materials.add(Color::rgb(0.3, 0.5, 0.3).into()),
            transform: Transform::from_xyz(
                parameters.dimx as f32 / 2.0,
                -2.0,
                parameters.dimz as f32 / 2.0,
            ),
            ..default()
        },
        Collider::cuboid(max_x_z, 0.1, max_x_z),
    ));

    entities.0.push(plane.id());

    // spheres
    initialize_spheres(
        &mut commands,
        &mut meshes,
        &mut materials,
        &parameters,
        &mut entities,
    );

    // directional 'sun' light
    let sunlight = commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            shadows_enabled: true,
            illuminance: 10000.0,
            ..default()
        },
        transform: Transform {
            translation: Vec3::new(0.0, 2.0, 0.0),
            rotation: Quat::from_rotation_x(-PI / 4.0)
                .mul_quat(Quat::from_rotation_y(PI / 4.0)),
            ..default()
        },
        ..default()
    });
    entities.0.push(sunlight.id());

    // camera
    let translation = Vec3::new(-22.0, 17.0, 19.0);
    let radius = translation.length();

    commands
        .spawn((
            AppCamera,
            Camera3dBundle {
                transform: Transform::from_translation(translation)
                    .looking_at(Vec3::ZERO, Vec3::Y),
                ..default()
            },
        ))
        .insert(PanOrbitCamera {
            radius,
            ..Default::default()
        });
}

fn initialize_spheres(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parameters: &LongitudinalWave3dSimulationParameters,
    entities: &mut Entities,
) {
    let mesh = meshes.add(Mesh::from(shape::Icosphere {
        radius: parameters.radius,
        subdivisions: 6,
    }));

    let material1_handle = materials.add(Color::rgb(0.6, 0.6, 0.6).into());
    let material2_handle = materials.add(Color::rgb(0.7, 0.5, 0.5).into());

    for x in 0..parameters.dimx {
        for y in 0..parameters.dimy {
            for z in 0..parameters.dimz {
                let material = if z == 0 {
                    material2_handle.clone()
                } else {
                    material1_handle.clone()
                };

                let translation = Vec3::new(x as f32, y as f32, z as f32);

                let mut particle = commands.spawn((
                    Particle {
                        initial_translation: translation,
                    },
                    PbrBundle {
                        mesh: mesh.clone(),
                        material,
                        transform: Transform::from_translation(translation),
                        ..default()
                    },
                    Collider::ball(parameters.radius),
                    Restitution::coefficient(0.7),
                    ExternalImpulse::default(),
                    ExternalForce::default(),
                ));

                if z == 0 {
                    particle.insert(ApplyingForce);
                    particle.insert(RigidBody::Fixed);
                } else {
                    particle.insert(RigidBody::Dynamic);
                }

                entities.0.push(particle.id());
            }
        }
    }
}

fn apply_impulse(
    time: Res<Time>,
    mut animation_timer: ResMut<AnimationTimer>,
    mut force_sources: Query<
        (&Particle, &mut ExternalImpulse, &mut Transform),
        With<ApplyingForce>,
    >,
    parameters: Res<LongitudinalWave3dSimulationParameters>,
) {
    animation_timer.0.tick(time.delta());

    let elapsed = animation_timer.0.elapsed();
    let z =
        (elapsed.as_secs_f32() * parameters.applying_force_freq * TAU).sin();

    for (particle, _, mut transform) in force_sources.iter_mut() {
        transform.translation.z = particle.initial_translation.z
            + (z * parameters.applying_force_factor);
    }
}

fn apply_equilibrium_force(
    mut force_sources: Query<(&Particle, &Transform, &mut ExternalForce)>,
    parameters: Res<LongitudinalWave3dSimulationParameters>,
) {
    for (particle, transform, mut external_force) in force_sources.iter_mut() {
        let equilizing_force_direction =
            particle.initial_translation - transform.translation;

        external_force.force =
            equilizing_force_direction * parameters.equilibrium_force_factor;
    }
}

#[allow(clippy::too_many_arguments)]
fn on_ui_events(
    mut time: ResMut<Time>,
    mut ui_events: EventReader<UiEvents>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    parameters: Res<LongitudinalWave3dSimulationParameters>,
    mut entities: ResMut<Entities>,
    particles: Query<Entity, With<Particle>>,
) {
    for event in ui_events.iter() {
        match event {
            UiEvents::StartStop => {
                if time.is_paused() {
                    time.unpause();
                } else {
                    time.pause();
                }
            }
            UiEvents::Reset => {
                for entity in particles.iter() {
                    if let Some(mut entity) = commands.get_entity(entity) {
                        entity.despawn();
                    }
                }

                initialize_spheres(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    &parameters,
                    &mut entities,
                );
            }
        }
    }
}

fn cleanup(
    mut commands: Commands,
    mut entities: ResMut<Entities>,
    mut rapier_debug_config: ResMut<DebugRenderContext>,
    mut rapier_config: ResMut<RapierConfiguration>,
) {
    for entity in entities.0.drain(..) {
        if let Some(mut entity) = commands.get_entity(entity) {
            entity.despawn();
        }
    }

    *rapier_debug_config = DebugRenderContext::default();
    *rapier_config = RapierConfiguration::default();
}
