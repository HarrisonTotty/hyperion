# HYPERION

A modular and customizable spaceship bridge simulation game.

## Overview

HYPERION is an incredibly detailed spaceship bridge simulation game where players operate different positions on a spaceship (like in Star Trek) that they design and upgrade. Players are dropped into a procedurally-generated galaxy with generated alien races, factions, languages, and history. Think Dwarf Fortress meets No Man's Sky meets Artemis.

## Features

- **Modular Design**: Ship classes, modules, and weapons are loaded at runtime from easy-to-configure YAML files
- **REST & GraphQL API**: The Hyperion server exposes a comprehensive API that game clients connect to
- **BYOF (Bring Your Own Frontend)**: The open nature of the server enables community-created frontends
- **3D Physics Simulation**: Realistic physics simulation using Bevy ECS
- **Procedural Galaxy**: Dynamically generated star systems, alien races, and factions

## Architecture

The HYPERION server is a Rust program built with:

- **`rocket`** - REST API framework and web server
- **`juniper` & `juniper_rocket`** - GraphQL API handling
- **`bevy_ecs`** - Entity Component System for simulation
- **`clap`** - Command-line interface parsing

## Installation

### Prerequisites

- Rust 1.70 or higher
- Cargo (comes with Rust)

### Building from Source

```bash
# Clone the repository
git clone https://github.com/yourusername/hyperion.git
cd hyperion

# Build the project
cargo build --release

# The binary will be available at target/release/hyperion
```

## Usage

Start the HYPERION server:

```bash
hyperion start
```

### Command-Line Options

- `-d`, `--data-dir <PATH>` - Path to the data directory (default: `./data`)
- `-l`, `--log-level <LEVEL>` - Log level: error, warn, info, debug, trace (default: `info`)

### Examples

```bash
# Start with custom data directory
hyperion start --data-dir /path/to/custom/data

# Start with debug logging
hyperion start --log-level debug

# Combine options
hyperion start -d ./custom-data -l trace
```

## Development

### Running Tests

```bash
cargo test
```

### Running with Development Data

```bash
# Uses ./data directory by default
cargo run -- start

# Or specify a different directory
cargo run -- start --data-dir ./data
```
