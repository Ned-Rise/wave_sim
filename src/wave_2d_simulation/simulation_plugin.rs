use bevy::prelude::*;
use ndarray::prelude::*;
use ndarray::Zip;

use crate::AppState;

use super::animation_plugin::PlotClickedEvent;
use super::finite_difference::update_with_absorbing_boundary;
use super::finite_difference::{
    update_with_laplace_operator_1, update_with_laplace_operator_4,
};
use super::SimulationGrid;
use super::SimulationParameters;

/// A field containing the factor for the Laplace Operator that
/// combines Velocity and Grid Constants for the `Wave Equation`
#[derive(Default, Resource)]
struct Tau(Array2<f32>);

/// A field containing the factor for the Laplace Operator that
/// combines Velocity and Grid Constants for the `Boundary Condition`
#[derive(Default, Resource)]
struct Kappa(Array2<f32>);

#[derive(Resource)]
struct ApplyingForceTimer(Timer);

pub struct SimulationPlugin;

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        let mut timer = Timer::default();
        timer.pause();

        app.insert_resource(SimulationGrid::default())
            .insert_resource(Tau::default())
            .insert_resource(Kappa::default())
            .insert_resource(ApplyingForceTimer(timer))
            .add_system_set(
                SystemSet::on_enter(AppState::Wave2dSimulation)
                    .with_system(setup),
            )
            .add_system_set(
                SystemSet::on_update(AppState::Wave2dSimulation)
                    .with_system(apply_force)
                    .with_system(update_wave),
            );
    }
}

fn setup(
    mut tau: ResMut<Tau>,
    mut kappa: ResMut<Kappa>,
    mut u: ResMut<SimulationGrid>,
    parameters: Res<SimulationParameters>,
) {
    tau.0 = Array::from_elem(
        (parameters.dimx, parameters.dimy),
        ((parameters.wave_velocity * parameters.time_step_width)
            / parameters.spatial_step_width)
            .powi(2),
    );

    kappa.0 = Array2::from_elem(
        (parameters.dimx, parameters.dimy),
        parameters.time_step_width * parameters.wave_velocity
            / parameters.spatial_step_width,
    );

    u.0 = Array3::zeros((3, parameters.dimx, parameters.dimy));
}

fn apply_force(
    time: Res<Time>,
    mut timer: ResMut<ApplyingForceTimer>,
    mut u: ResMut<SimulationGrid>,
    parameters: Res<SimulationParameters>,
    mut plot_clicked_events: EventReader<PlotClickedEvent>,
) {
    if timer.0.tick(time.delta()).just_finished() {
        let init_x = 4 * parameters.dimx / 6;
        let init_y = 4 * parameters.dimy / 6;

        *u.0.get_mut((0, init_x, init_y)).unwrap() =
            parameters.applied_force_amplitude;
    }

    for event in plot_clicked_events.iter() {
        let event_x: usize = event.x.round() as usize;
        let event_y: usize = event.y.round() as usize;

        if 0 < event_x
            && event_x < parameters.dimx
            && 0 < event_y
            && event_y < parameters.dimy
        {
            *u.0.get_mut((0, event_x, event_y)).unwrap() =
                parameters.applied_force_amplitude;
        }
    }
}

fn update_wave(
    mut u: ResMut<SimulationGrid>,
    tau: Res<Tau>,
    kappa: Res<Kappa>,
    parameters: Res<SimulationParameters>,
) {
    let boundary_size = parameters.boundary_size;

    let (u_2, mut u_1, u_0) =
        u.0.multi_slice_mut((s![2, .., ..], s![1, .., ..], s![0, .., ..]));

    Zip::from(u_2).and(&mut u_1).for_each(std::mem::swap);

    Zip::from(u_1).and(u_0).for_each(std::mem::swap);

    let new_u = if boundary_size == 1 {
        update_with_laplace_operator_1(
            parameters.dimx,
            parameters.dimy,
            &tau.0,
            &u.0,
        )
    } else if boundary_size == 4 {
        update_with_laplace_operator_4(
            parameters.dimx,
            parameters.dimy,
            &tau.0,
            &u.0,
        )
    } else {
        todo!("boundary_size of size {}", boundary_size);
    };

    u.0.slice_mut(s![
        0,
        boundary_size..(parameters.dimx - boundary_size),
        boundary_size..(parameters.dimy - boundary_size)
    ])
    .assign(&new_u);

    if parameters.use_absorbing_boundary {
        update_with_absorbing_boundary(
            parameters.dimx,
            parameters.dimy,
            boundary_size,
            &kappa.0,
            &mut u.0,
        );
    } else {
        u.0.mapv_inplace(|u| u * 0.995);
    }
}
