//! Map and galaxy generation configuration
//!
//! This module defines configuration for procedural galaxy, star system, and universe generation.

use serde::{Deserialize, Serialize};

/// Map and procedural generation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapConfig {
    /// Galaxy structure settings
    pub galaxy: GalaxyConfig,
    /// Star generation settings
    pub stars: StarConfig,
    /// Star system generation settings
    pub systems: SystemConfig,
    /// Procedural generation defaults
    pub generation: GenerationConfig,
}

/// Galaxy structure configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GalaxyConfig {
    /// Galaxy radius in light-years
    pub radius: f64,
    /// Number of sectors per dimension (creates NxNxN grid)
    pub sectors_per_dimension: usize,
    /// Z-axis flattening factor (0.0-1.0, lower = flatter)
    pub flattening_factor: f64,
    /// Spiral arm parameters
    pub spiral_arms: SpiralArmConfig,
}

/// Spiral arm configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpiralArmConfig {
    /// Number of major spiral arms
    pub count: usize,
    /// Spiral tightness (radians per unit radius)
    pub tightness: f64,
    /// Arm width as fraction of radius
    pub width: f64,
}

/// Star generation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StarConfig {
    /// Star type probabilities (should sum to 1.0)
    pub type_probabilities: StarTypeProbabilities,
    /// Star density by sector type
    pub sector_densities: SectorDensityConfig,
    /// Inhabited star probability (0.0-1.0)
    pub inhabited_probability: f64,
}

/// Probabilities for each star type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StarTypeProbabilities {
    pub blue_giant: f64,
    pub white: f64,
    pub yellow: f64,
    pub orange: f64,
    pub red_dwarf: f64,
    pub neutron: f64,
    pub black_hole: f64,
}

/// Star density multipliers for each sector type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorDensityConfig {
    /// Core density multiplier
    pub core: f64,
    /// Spiral arm density multiplier
    pub arm: f64,
    /// Inter-arm density multiplier
    pub inter_arm: f64,
    /// Rim density multiplier
    pub rim: f64,
    /// Void density multiplier
    pub void_density: f64,
}

/// Star system generation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    /// Planet generation settings
    pub planets: PlanetConfig,
    /// Moon generation settings
    pub moons: MoonConfig,
    /// Asteroid belt settings
    pub asteroids: AsteroidConfig,
    /// Space station settings
    pub stations: StationConfig,
}

/// Planet generation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanetConfig {
    /// Minimum planets per system
    pub min_planets: usize,
    /// Maximum planets per system
    pub max_planets: usize,
    /// Planet type probabilities
    pub type_probabilities: PlanetTypeProbabilities,
    /// Habitable zone settings
    pub habitable_zone: HabitableZoneConfig,
}

/// Probabilities for planet types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanetTypeProbabilities {
    pub terrestrial: f64,
    pub gas_giant: f64,
    pub ice_giant: f64,
    pub ice: f64,
    pub volcanic: f64,
    pub ocean: f64,
}

/// Habitable zone calculation settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HabitableZoneConfig {
    /// Inner boundary multiplier based on luminosity
    pub inner_multiplier: f64,
    /// Outer boundary multiplier based on luminosity
    pub outer_multiplier: f64,
    /// Base habitable zone in AU for Sun-like stars
    pub base_zone_au: f64,
}

/// Moon generation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoonConfig {
    /// Minimum moons for gas giants
    pub gas_giant_min_moons: usize,
    /// Maximum moons for gas giants
    pub gas_giant_max_moons: usize,
    /// Probability of terrestrial planets having moons
    pub terrestrial_moon_probability: f64,
    /// Max moons for terrestrial planets
    pub terrestrial_max_moons: usize,
}

/// Asteroid belt configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsteroidConfig {
    /// Probability of asteroid belt per system
    pub probability: f64,
    /// Maximum asteroid belts per system
    pub max_per_system: usize,
    /// Minimum density
    pub min_density: f32,
    /// Maximum density
    pub max_density: f32,
}

/// Space station configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StationConfig {
    /// Probability of station per inhabited system
    pub probability: f64,
    /// Maximum stations per system
    pub max_per_system: usize,
    /// Station type probabilities
    pub type_probabilities: StationTypeProbabilities,
}

/// Probabilities for station types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StationTypeProbabilities {
    pub trade: f64,
    pub military: f64,
    pub research: f64,
    pub mining: f64,
    pub shipyard: f64,
}

/// Default generation parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationConfig {
    /// Default number of stars for new universes
    pub default_stars: usize,
    /// Default number of factions
    pub default_factions: usize,
    /// Enable deterministic generation
    pub deterministic: bool,
}

impl Default for MapConfig {
    fn default() -> Self {
        Self {
            galaxy: GalaxyConfig {
                radius: 50000.0,
                sectors_per_dimension: 10,
                flattening_factor: 0.15,
                spiral_arms: SpiralArmConfig {
                    count: 4,
                    tightness: 0.3,
                    width: 0.15,
                },
            },
            stars: StarConfig {
                type_probabilities: StarTypeProbabilities {
                    blue_giant: 0.01,
                    white: 0.04,
                    yellow: 0.10,
                    orange: 0.15,
                    red_dwarf: 0.65,
                    neutron: 0.03,
                    black_hole: 0.02,
                },
                sector_densities: SectorDensityConfig {
                    core: 3.0,
                    arm: 2.0,
                    inter_arm: 1.0,
                    rim: 0.5,
                    void_density: 0.1,
                },
                inhabited_probability: 0.15,
            },
            systems: SystemConfig {
                planets: PlanetConfig {
                    min_planets: 1,
                    max_planets: 8,
                    type_probabilities: PlanetTypeProbabilities {
                        terrestrial: 0.30,
                        gas_giant: 0.20,
                        ice_giant: 0.15,
                        ice: 0.15,
                        volcanic: 0.10,
                        ocean: 0.10,
                    },
                    habitable_zone: HabitableZoneConfig {
                        inner_multiplier: 0.95,
                        outer_multiplier: 1.37,
                        base_zone_au: 1.0,
                    },
                },
                moons: MoonConfig {
                    gas_giant_min_moons: 2,
                    gas_giant_max_moons: 20,
                    terrestrial_moon_probability: 0.3,
                    terrestrial_max_moons: 3,
                },
                asteroids: AsteroidConfig {
                    probability: 0.6,
                    max_per_system: 3,
                    min_density: 0.1,
                    max_density: 0.9,
                },
                stations: StationConfig {
                    probability: 0.7,
                    max_per_system: 4,
                    type_probabilities: StationTypeProbabilities {
                        trade: 0.35,
                        military: 0.20,
                        research: 0.15,
                        mining: 0.20,
                        shipyard: 0.10,
                    },
                },
            },
            generation: GenerationConfig {
                default_stars: 1000,
                default_factions: 5,
                deterministic: true,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_map_config() {
        let config = MapConfig::default();
        assert_eq!(config.galaxy.radius, 50000.0);
        assert_eq!(config.galaxy.sectors_per_dimension, 10);
        assert!(config.generation.deterministic);
    }

    #[test]
    fn test_star_type_probabilities_sum() {
        let config = MapConfig::default();
        let sum = config.stars.type_probabilities.blue_giant
            + config.stars.type_probabilities.white
            + config.stars.type_probabilities.yellow
            + config.stars.type_probabilities.orange
            + config.stars.type_probabilities.red_dwarf
            + config.stars.type_probabilities.neutron
            + config.stars.type_probabilities.black_hole;
        
        // Should sum to approximately 1.0
        assert!((sum - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_planet_type_probabilities_sum() {
        let config = MapConfig::default();
        let sum = config.systems.planets.type_probabilities.terrestrial
            + config.systems.planets.type_probabilities.gas_giant
            + config.systems.planets.type_probabilities.ice_giant
            + config.systems.planets.type_probabilities.ice
            + config.systems.planets.type_probabilities.volcanic
            + config.systems.planets.type_probabilities.ocean;
        
        assert!((sum - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_serialization() {
        let config = MapConfig::default();
        let yaml = serde_yaml::to_string(&config).unwrap();
        let deserialized: MapConfig = serde_yaml::from_str(&yaml).unwrap();
        
        assert_eq!(config.galaxy.radius, deserialized.galaxy.radius);
        assert_eq!(config.generation.default_stars, deserialized.generation.default_stars);
    }
}
