# HYPERION Spaceship Bridge Simulation Game

HYPERION is an incredibly detailed, modular, and customizable spaceship bridge simulation game. Players operate different positions on a spaceship (like in Star Trek) that they design and upgrade, and are dropped into a procedurally-generated galaxy with generated alien races, factions, languages, and history. Think Dwarf Fortress meets No Man's Sky meets Artimis.

## High-Level Design Features

1. The Hyperion server is a Rust program that uses `rocket` to expose a GraphQL/REST API and streaming service that game clients connect to.
2. Game data (ship classes, modules, and weapons) and simulation parameters are loaded at runtime from easy to configure YAML files, allowing players to fully tailor their experience.
3. The open nature of the `hyperion` server enables a BYOF (Bring Your Own Frontend) community.
4. Game world is 3D with realistic physics.
5. Players can design and customize their own spaceships using a modular blueprint system.
6. The game features a rich simulation of space travel, combat, and exploration.

## Development Guidelines

1. Document all functions and modules using Rust's `//!` and `///` doc comments feature.
2. Create extensive unit tests for all features, methods, etc.
3. Stick to modern Rust design principles, writing clear and concise code.
4. Prefer splitting large files into smaller modules for clarity.

## High-Level Game Design Guidelines

1. The gameplay loop should feel like a real spaceship bridge operation, with players taking on specific roles and responsibilities.
2. The UI should be designed to be intimidating to first-time users, leveraging complexity to convey depth.
3. The game should prioritize realism and immersion, using hard sci-fi aesthetics and terminology.
4. The game should be modular and extensible, allowing for easy addition of new ship classes, modules, and gameplay features.
5. The game should support both local and online multiplayer, allowing players to team up and operate a ship together.
6. The game should feature a rich and dynamic universe, with procedurally generated factions, alien species, and events.
7. The game should encourage teamwork and communication among players, with each position on the ship having unique abilities and responsibilities.
8. Operation of each ship position should emulate real-world and near-future systems as closely as possible, using realistic data displays, controls, and feedback mechanisms.