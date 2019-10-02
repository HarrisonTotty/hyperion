//! Hyperion

#![feature(box_syntax, decl_macro, proc_macro_hygiene)]

#[macro_use] extern crate log;
#[macro_use] extern crate rocket;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate specs_derive;

pub mod cli;
pub mod config;
pub mod ecs;
pub mod json;
pub mod logging;
pub mod math;
pub mod server;
pub mod server_routes;
pub mod ship;
pub mod ship_class;

use specs::prelude::*;
use crate::ecs::components::*;
use crate::ecs::systems::*;
use crate::ecs::resources::*;
use crate::math::*;

/// The entrypoint of the program.
fn main() {
    // Parse CLI arguments.
    let args = cli::get_arguments();

    // Set-up logging.
    match logging::setup(
        args.value_of("log_file").unwrap(),
        args.value_of("log_level").unwrap(),
        args.value_of("log_mode").unwrap()
    ) {
        Ok(_)  => debug!("Initialized logging subsystem."),
        Err(e) => panic!("Unable to initialize logging subsystem - {}", e)
    }

    // Configure the server.
    let server_config = server::configure(
        args.value_of("address").unwrap(),
        args.value_of("log_level").unwrap(),
        args.value_of("port").unwrap().parse::<u16>().unwrap()
    );

    let mut world = World::new();

    world.register::<ecs::components::Collision>();
    world.register::<ecs::components::Dynamics>();
    world.register::<ecs::components::Orientation>();
    world.register::<ecs::components::Physicality>();

    let d = Dynamics {
        acceleration: Vector(0.1, 0.3, -0.6),
        position: Vector(1.0, 1.0, 1.0),
        velocity: Vector(0.0, 0.0, 0.0)
    };
    world.create_entity()
        .with(d)
        .with(Orientation::default())
        .with(Physicality { shape: Shape::Sphere(5.0), collisions_enabled: true })
        .build();

    world.create_entity()
        .with(Dynamics::default())
        .with(Orientation::default())
        .with(Physicality::default())
        .build();

    world.insert(CollisionLimits::default());
    world.insert(DeltaTime(0.5));
    world.insert(DynamicsLimits::default());
    world.insert(OrientationLimits::default());
    
    let mut dispatcher = DispatcherBuilder::new()
        .with(
            HandleDynamics,
            "handle_dynamics",
            &[]
        )
        .with(
            HandleOrientation,
            "handle_orientation",
            &[]
        )
        .with(
            CollisionDetection,
            "collision_detection",
            &["handle_dynamics", "handle_orientation"]
        )
        .with(
            HandleCollisions,
            "handle_collisions",
            &["collision_detection"]
        )
        .build();
    dispatcher.dispatch(&mut world);
    dispatcher.dispatch(&mut world);
    world.maintain();

    server::start(server_config);
}
