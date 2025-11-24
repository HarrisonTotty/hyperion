# Configuration Compatibility Update

## Overview

This document describes the changes made to ensure Rust configuration structs are compatible with the actual YAML configuration files in the `data/` directory.

## Problem

The original Rust configuration structs used different field names and structures than the actual YAML files. This prevented the game from loading its configuration data.

## Solution

Updated all configuration structs to match the YAML schemas using Serde attributes:
- `#[serde(rename = "field")]` - Map YAML field names to Rust field names
- `#[serde(skip)]` - Mark derived fields that don't exist in YAML
- `#[serde(default)]` - Allow optional fields with default values

## Changes Made

### 1. Ship Class Configuration (`src/config/ship_class.rs`)

**Added Enums:**
- `ShipSize` - Small, Medium, Large
- `ShipClassRole` - Versatile, Combat, Support, Transport, Exploration

**Updated Fields:**
- `description` ← YAML field `desc`
- `base_hull` ← YAML field `base_hp`
- `size: ShipSize` - Ship size category
- `role: ShipClassRole` - Primary ship role
- `build_points: f32` - Construction cost in build points
- `bonuses: HashMap<String, f32>` - Ship class stat bonuses
- `id: String` - Now derived from filename (not in YAML)
- `base_shields: f32` - Made optional with default

**Removed Fields:**
- `available_roles` - Not used in actual YAML files

**New Methods:**
- `set_id(id: String)` - Populate id field from filename

### 2. Module Configuration (`src/config/module.rs`)

**Updated to Unified Schema:**
All module types (power cores, engines, thrusters, etc.) now use a single `ModuleConfig` struct with common base fields and type-specific optional fields.

**Common Fields:**
- `name: String` - Display name
- `model: String` - Model designation
- `kind: String` - Module kind/category
- `manufacturer: String` - Manufacturer name
- `description` ← YAML field `desc`
- `cost: f32` - Build cost
- `weight: f32` - Weight in kg
- `id: String` - Derived from filename

**Power Core Specific:**
- `max_energy: f32` - Maximum energy capacity
- `production: f32` - Energy production rate

**Engine Specific:**
- `thrust: f32` - Thrust output
- `energy_consumption: f32` - Energy consumption rate

**New Methods:**
- `set_id(id: String)` - Populate id from filename
- `is_power_core() -> bool` - Check if module is a power core
- `is_engine() -> bool` - Check if module is an engine

### 3. Weapon Configuration (`src/config/weapon.rs`)

**Updated to Flexible Schema:**
Weapons now support multiple types (missiles, directed energy, kinetic) with a unified struct containing all possible fields.

**Common Fields:**
- `name: String` - Display name
- `model: String` - Model designation
- `kind: String` - Weapon kind/category
- `manufacturer: String` - Manufacturer name
- `description` ← YAML field `desc`
- `cost: f32` - Build cost
- `weight: f32` - Weight (optional, defaults to 0)
- `tags: Vec<WeaponTag>` - Weapon tags
- `id: String` - Derived from filename

**Type-Specific Fields (all optional):**
- Fire timing: `fire_delay`, `reload_time`, `recharge_time`
- Speed: `speed`, `velocity`
- Range: `max_range`, `effective_range`
- Damage: `damage`, `impact_damage`, `blast_damage`, `blast_radius`
- Energy: `energy_consumption`
- Missiles: `forward_thrust`, `max_turn_rate`, `load_time`, `lifetime`, `max_speed`
- Kinetic: `num_projectiles`, `ammo_consumption`, `accuracy`

**New Methods:**
- `set_id(id: String)` - Populate id from filename

### 4. Ammunition Configuration (`src/config/weapon.rs`)

**Updated Fields:**
- `ammo_type` ← YAML field `type`
- `size: String` - Ammunition size (e.g., "100mm")
- `description` ← YAML field `desc`
- `cost: f32` - Build cost
- `weight: f32` - Weight in kg
- `impact_damage: f32` - Direct impact damage
- `blast_radius: f32` - Blast radius (optional)
- `blast_damage: f32` - Blast damage (optional)
- `velocity: f32` - Projectile velocity
- `armor_penetration: f32` - Armor penetration rating
- `id: String` - Derived from filename

**New Methods:**
- `set_id(id: String)` - Populate id from filename

### 5. Weapon Tag Enum (`src/models/weapon.rs`)

**Updated:**
- Added `#[serde(rename = "Single-Fire")]` to `SingleFire` variant to match YAML format

## YAML File Compatibility

Successfully verified loading of:
- ✅ Ship classes (16 files)
- ✅ Power cores (6 files)
- ✅ Impulse engines (6 files)
- ✅ Kinetic weapons (18 files)
- ✅ Ammunition (21 files)

## Migration Impact

**Files Updated:**
- `src/config/ship_class.rs` - Ship class configuration
- `src/config/module.rs` - Module configuration
- `src/config/weapon.rs` - Weapon and ammunition configuration
- `src/config.rs` - Configuration exports
- `src/models/weapon.rs` - Weapon tag enum
- `src/blueprint.rs` - Test helper updated
- `src/compiler.rs` - Test helper updated
- `src/api/ships.rs` - Test helper updated

**Test Files:**
- All existing tests updated to use new structures
- New example added: `examples/test_yaml_loading.rs`

**Test Results:**
- ✅ All 89 tests passing
- ✅ Zero compilation warnings
- ✅ Zero compilation errors
- ✅ Verified YAML file loading works correctly

## Usage Example

```rust
use hyperion::config::ShipClassConfig;
use std::fs;

// Load a ship class configuration
let yaml = fs::read_to_string("data/ship-classes/cruiser.yaml")?;
let mut config: ShipClassConfig = serde_yaml::from_str(&yaml)?;

// Populate the id from the filename
config.set_id("cruiser".to_string());

// Access configuration
println!("Ship: {} ({})", config.name, config.description);
println!("Size: {:?}, Role: {:?}", config.size, config.role);
println!("Hull: {}, Shields: {}", config.base_hull, config.base_shields);
```

## Future Considerations

1. **Configuration Loading**: Update the configuration loader to automatically populate `id` fields from filenames when loading YAML files.

2. **Validation**: Consider adding validation to ensure type-specific fields are present (e.g., power cores must have `max_energy` > 0).

3. **Type Safety**: Could use enum-based approach with separate structs per module/weapon type for stronger type safety, but current approach provides flexibility for modding.

4. **Schema Documentation**: Generate schema documentation from Rust structs to help content creators.

## Testing

Run the YAML loading test:
```bash
cargo run --example test_yaml_loading
```

Run all library tests:
```bash
cargo test --lib
```

Both should pass with no errors or warnings.
