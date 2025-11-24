//! Test YAML file loading
//!
//! This example tests that we can load actual YAML configuration files

use hyperion::config::{ShipClassConfig, ModuleConfig, WeaponConfig, AmmunitionConfig};
use std::fs;

fn main() {
    println!("Testing YAML configuration loading...\n");
    
    // Test ship class loading
    println!("=== Ship Class Loading ===");
    match fs::read_to_string("data/ship-classes/cruiser.yaml") {
        Ok(yaml) => {
            match serde_yaml::from_str::<ShipClassConfig>(&yaml) {
                Ok(mut config) => {
                    config.set_id("cruiser".to_string());
                    println!("✓ Loaded ship class: {}", config.name);
                    println!("  Size: {:?}, Role: {:?}", config.size, config.role);
                    println!("  Base hull: {}, Max modules: {}", config.base_hull, config.max_modules);
                    println!("  Build points: {}", config.build_points);
                }
                Err(e) => println!("✗ Failed to parse cruiser.yaml: {}", e),
            }
        }
        Err(e) => println!("✗ Failed to read cruiser.yaml: {}", e),
    }
    
    println!("\n=== Module Loading (Power Core) ===");
    match fs::read_to_string("data/modules/power-cores/mark-iii-fission-reactor.yaml") {
        Ok(yaml) => {
            match serde_yaml::from_str::<ModuleConfig>(&yaml) {
                Ok(mut config) => {
                    config.set_id("mark-iii-fission-reactor".to_string());
                    println!("✓ Loaded module: {}", config.name);
                    println!("  Model: {}, Kind: {}", config.model, config.kind);
                    println!("  Max energy: {}, Production: {}", config.max_energy, config.production);
                    println!("  Weight: {}, Cost: {}", config.weight, config.cost);
                }
                Err(e) => println!("✗ Failed to parse reactor YAML: {}", e),
            }
        }
        Err(e) => println!("✗ Failed to read reactor YAML: {}", e),
    }
    
    println!("\n=== Module Loading (Impulse Engine) ===");
    match fs::read_to_string("data/modules/impulse-engines/solar-sail.yaml") {
        Ok(yaml) => {
            match serde_yaml::from_str::<ModuleConfig>(&yaml) {
                Ok(mut config) => {
                    config.set_id("solar-sail".to_string());
                    println!("✓ Loaded module: {}", config.name);
                    println!("  Model: {}, Kind: {}", config.model, config.kind);
                    println!("  Thrust: {}, Energy consumption: {}", config.thrust, config.energy_consumption);
                    println!("  Weight: {}, Cost: {}", config.weight, config.cost);
                }
                Err(e) => println!("✗ Failed to parse engine YAML: {}", e),
            }
        }
        Err(e) => println!("✗ Failed to read engine YAML: {}", e),
    }
    
    println!("\n=== Weapon Loading (Kinetic) ===");
    match fs::read_to_string("data/modules/kinetic-weapons/100mm-medium-cannon.yaml") {
        Ok(yaml) => {
            match serde_yaml::from_str::<WeaponConfig>(&yaml) {
                Ok(mut config) => {
                    config.set_id("100mm-medium-cannon".to_string());
                    println!("✓ Loaded weapon: {}", config.name);
                    println!("  Model: {}, Kind: {}", config.model, config.kind);
                    println!("  Reload time: {}, Accuracy: {}", config.reload_time, config.accuracy);
                    println!("  Weight: {}, Cost: {}", config.weight, config.cost);
                }
                Err(e) => println!("✗ Failed to parse weapon YAML: {}", e),
            }
        }
        Err(e) => println!("✗ Failed to read weapon YAML: {}", e),
    }
    
    println!("\n=== Ammunition Loading ===");
    match fs::read_to_string("data/modules/kinetic-weapons/ammo/he-100mm-shell.yaml") {
        Ok(yaml) => {
            match serde_yaml::from_str::<AmmunitionConfig>(&yaml) {
                Ok(mut config) => {
                    config.set_id("he-100mm-shell".to_string());
                    println!("✓ Loaded ammunition: {}", config.name);
                    println!("  Type: {}, Size: {}", config.ammo_type, config.size);
                    println!("  Impact damage: {}, Velocity: {}", config.impact_damage, config.velocity);
                    println!("  Armor penetration: {}", config.armor_penetration);
                }
                Err(e) => println!("✗ Failed to parse ammunition YAML: {}", e),
            }
        }
        Err(e) => println!("✗ Failed to read ammunition YAML: {}", e),
    }
    
    println!("\n=== All YAML loading tests completed ===");
}
