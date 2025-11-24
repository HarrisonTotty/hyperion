//! Star system generation
//!
//! This module generates star systems with planets, moons, asteroid belts, and stations.

use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::galaxy::StarType;

/// A complete star system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StarSystem {
    /// Unique identifier
    pub id: String,
    /// System name
    pub name: String,
    /// Central star
    pub star: StarInfo,
    /// Planets in the system
    pub planets: Vec<Planet>,
    /// Asteroid belts
    pub asteroid_belts: Vec<AsteroidBelt>,
    /// Space stations
    pub stations: Vec<StationInfo>,
    /// Inhabited by sentient species
    pub inhabited: bool,
}

/// Star information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StarInfo {
    /// Star type
    pub star_type: StarType,
    /// Mass (solar masses)
    pub mass: f32,
    /// Luminosity (solar luminosities)
    pub luminosity: f32,
}

/// A planet in a star system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Planet {
    /// Planet name
    pub name: String,
    /// Orbital radius (AU)
    pub orbital_radius: f64,
    /// Planet type
    pub planet_type: PlanetType,
    /// Mass (Earth masses)
    pub mass: f32,
    /// Radius (Earth radii)
    pub radius: f32,
    /// Has atmosphere
    pub atmosphere: bool,
    /// Habitable zone
    pub in_habitable_zone: bool,
    /// Inhabited
    pub inhabited: bool,
    /// Moons
    pub moons: Vec<Moon>,
}

/// Type of planet
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanetType {
    /// Rocky terrestrial planet
    Terrestrial,
    /// Gas giant
    GasGiant,
    /// Ice giant
    IceGiant,
    /// Frozen ice world
    Ice,
    /// Volcanic world
    Volcanic,
    /// Ocean world
    Ocean,
}

/// A moon orbiting a planet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Moon {
    /// Moon name
    pub name: String,
    /// Mass (lunar masses)
    pub mass: f32,
    /// Radius (lunar radii)
    pub radius: f32,
}

/// An asteroid belt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsteroidBelt {
    /// Belt name
    pub name: String,
    /// Inner radius (AU)
    pub inner_radius: f64,
    /// Outer radius (AU)
    pub outer_radius: f64,
    /// Density
    pub density: f32,
}

/// Space station in system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StationInfo {
    /// Station ID
    pub id: Uuid,
    /// Station name
    pub name: String,
    /// Orbital body (planet name or "Star")
    pub orbiting: String,
    /// Station type
    pub station_type: StationType,
}

/// Type of space station
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StationType {
    /// Trading hub
    Trade,
    /// Military base
    Military,
    /// Research station
    Research,
    /// Mining operation
    Mining,
    /// Shipyard
    Shipyard,
}

impl StarSystem {
    /// Generate a star system from a star
    pub fn generate(star_id: String, star_name: String, star_type: StarType, inhabited: bool, seed: u64) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);
        
        // Generate star properties
        let star = Self::generate_star_info(&mut rng, star_type);
        
        // Determine number of planets
        let num_planets = match star_type {
            StarType::BlueGiant => rng.gen_range(0..3),
            StarType::White => rng.gen_range(2..6),
            StarType::Yellow => rng.gen_range(3..9),
            StarType::Orange => rng.gen_range(2..7),
            StarType::RedDwarf => rng.gen_range(1..5),
            StarType::Neutron | StarType::BlackHole => 0,
        };
        
        // Generate planets
        let mut planets = Vec::new();
        for i in 0..num_planets {
            let planet = Self::generate_planet(&mut rng, i, &star, inhabited);
            planets.push(planet);
        }
        
        // Generate asteroid belts
        let mut asteroid_belts = Vec::new();
        if rng.gen_bool(0.4) {
            let belt = Self::generate_asteroid_belt(&mut rng, num_planets);
            asteroid_belts.push(belt);
        }
        
        // Generate stations
        let mut stations = Vec::new();
        if inhabited || rng.gen_bool(0.3) {
            let num_stations = if inhabited { rng.gen_range(1..4) } else { rng.gen_range(0..2) };
            for _ in 0..num_stations {
                let station = Self::generate_station(&mut rng, &planets);
                stations.push(station);
            }
        }
        
        StarSystem {
            id: star_id,
            name: star_name,
            star,
            planets,
            asteroid_belts,
            stations,
            inhabited,
        }
    }
    
    fn generate_star_info(rng: &mut StdRng, star_type: StarType) -> StarInfo {
        let (mass, luminosity) = match star_type {
            StarType::BlueGiant => (rng.gen_range(10.0..50.0), rng.gen_range(1000.0..10000.0)),
            StarType::White => (rng.gen_range(1.4..2.5), rng.gen_range(5.0..25.0)),
            StarType::Yellow => (rng.gen_range(0.8..1.2), rng.gen_range(0.6..1.5)),
            StarType::Orange => (rng.gen_range(0.5..0.8), rng.gen_range(0.1..0.6)),
            StarType::RedDwarf => (rng.gen_range(0.1..0.5), rng.gen_range(0.001..0.1)),
            StarType::Neutron => (rng.gen_range(1.4..2.0), rng.gen_range(0.0001..0.001)),
            StarType::BlackHole => (rng.gen_range(3.0..20.0), 0.0),
        };
        
        StarInfo { star_type, mass, luminosity }
    }
    
    fn generate_planet(rng: &mut StdRng, index: usize, star: &StarInfo, system_inhabited: bool) -> Planet {
        // Orbital radius increases with each planet
        let base_radius = match star.star_type {
            StarType::BlueGiant => 5.0,
            StarType::White => 2.0,
            StarType::Yellow => 0.4,
            StarType::Orange => 0.3,
            StarType::RedDwarf => 0.1,
            _ => 1.0,
        };
        
        let orbital_radius = base_radius * (1.5_f64).powi(index as i32) * rng.gen_range(0.8..1.2);
        
        // Habitable zone (rough approximation)
        let hab_inner = (star.luminosity as f64).sqrt() * 0.95;
        let hab_outer = (star.luminosity as f64).sqrt() * 1.37;
        let in_habitable_zone = orbital_radius >= hab_inner && orbital_radius <= hab_outer;
        
        // Determine planet type based on orbital radius
        let planet_type = if orbital_radius < hab_inner * 0.5 {
            if rng.gen_bool(0.7) { PlanetType::Volcanic } else { PlanetType::Terrestrial }
        } else if in_habitable_zone {
            if rng.gen_bool(0.4) { PlanetType::Terrestrial } 
            else if rng.gen_bool(0.3) { PlanetType::Ocean }
            else { PlanetType::Ice }
        } else if orbital_radius < hab_outer * 2.0 {
            if rng.gen_bool(0.6) { PlanetType::GasGiant } else { PlanetType::Terrestrial }
        } else {
            if rng.gen_bool(0.5) { PlanetType::IceGiant } else { PlanetType::Ice }
        };
        
        // Mass and radius based on type
        let (mass, radius) = match planet_type {
            PlanetType::Terrestrial => (rng.gen_range(0.1..3.0), rng.gen_range(0.5..1.8)),
            PlanetType::GasGiant => (rng.gen_range(50.0..500.0), rng.gen_range(5.0..15.0)),
            PlanetType::IceGiant => (rng.gen_range(10.0..50.0), rng.gen_range(3.0..6.0)),
            PlanetType::Ice => (rng.gen_range(0.1..2.0), rng.gen_range(0.4..1.5)),
            PlanetType::Volcanic => (rng.gen_range(0.5..2.0), rng.gen_range(0.6..1.2)),
            PlanetType::Ocean => (rng.gen_range(0.8..1.5), rng.gen_range(0.9..1.3)),
        };
        
        let atmosphere = matches!(planet_type, PlanetType::Terrestrial | PlanetType::Ocean | PlanetType::Volcanic | PlanetType::GasGiant | PlanetType::IceGiant);
        
        // Inhabited chance (only if system is inhabited and planet is habitable)
        let inhabited = system_inhabited && in_habitable_zone && 
                        matches!(planet_type, PlanetType::Terrestrial | PlanetType::Ocean) &&
                        rng.gen_bool(0.5);
        
        // Generate moons for large planets
        let num_moons = match planet_type {
            PlanetType::GasGiant => rng.gen_range(2..20),
            PlanetType::IceGiant => rng.gen_range(1..10),
            PlanetType::Terrestrial if rng.gen_bool(0.3) => rng.gen_range(1..3),
            _ => 0,
        };
        
        let mut moons = Vec::new();
        for i in 0..num_moons {
            moons.push(Moon {
                name: format!("Moon {}", i + 1),
                mass: rng.gen_range(0.01..2.0),
                radius: rng.gen_range(0.1..1.5),
            });
        }
        
        Planet {
            name: format!("Planet {}", index + 1),
            orbital_radius,
            planet_type,
            mass,
            radius,
            atmosphere,
            in_habitable_zone,
            inhabited,
            moons,
        }
    }
    
    fn generate_asteroid_belt(rng: &mut StdRng, num_planets: usize) -> AsteroidBelt {
        let inner_radius = 2.0 + num_planets as f64 * 0.5;
        let outer_radius = inner_radius + rng.gen_range(0.5..2.0);
        let density = rng.gen_range(0.1..1.0);
        
        AsteroidBelt {
            name: "Asteroid Belt".to_string(),
            inner_radius,
            outer_radius,
            density,
        }
    }
    
    fn generate_station(rng: &mut StdRng, planets: &[Planet]) -> StationInfo {
        let station_type = match rng.gen_range(0..5) {
            0 => StationType::Trade,
            1 => StationType::Military,
            2 => StationType::Research,
            3 => StationType::Mining,
            _ => StationType::Shipyard,
        };
        
        let orbiting = if !planets.is_empty() && rng.gen_bool(0.7) {
            let planet_idx = rng.gen_range(0..planets.len());
            planets[planet_idx].name.clone()
        } else {
            "Star".to_string()
        };
        
        StationInfo {
            id: Uuid::new_v4(),
            name: format!("{:?} Station", station_type),
            orbiting,
            station_type,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_system_generation() {
        let system = StarSystem::generate(
            "STAR-000001".to_string(),
            "Alpha Centauri".to_string(),
            StarType::Yellow,
            true,
            42,
        );
        
        assert_eq!(system.id, "STAR-000001");
        assert_eq!(system.name, "Alpha Centauri");
        assert_eq!(system.star.star_type, StarType::Yellow);
        assert!(system.inhabited);
        assert!(!system.planets.is_empty());
    }
    
    #[test]
    fn test_planet_types() {
        let system = StarSystem::generate(
            "STAR-000002".to_string(),
            "Test Star".to_string(),
            StarType::Yellow,
            false,
            123,
        );
        
        // Should have variety of planet types
        let has_terrestrial = system.planets.iter().any(|p| p.planet_type == PlanetType::Terrestrial);
        let has_gas_giant = system.planets.iter().any(|p| p.planet_type == PlanetType::GasGiant);
        
        assert!(has_terrestrial || has_gas_giant);
    }
    
    #[test]
    fn test_habitable_zone() {
        let system = StarSystem::generate(
            "STAR-000003".to_string(),
            "Sol".to_string(),
            StarType::Yellow,
            true,
            456,
        );
        
        // At least one planet should be in habitable zone for inhabited system
        let habitable_planets: Vec<_> = system.planets.iter()
            .filter(|p| p.in_habitable_zone)
            .collect();
        
        assert!(!habitable_planets.is_empty());
    }
    
    #[test]
    fn test_gas_giant_moons() {
        let system = StarSystem::generate(
            "STAR-000004".to_string(),
            "Jupiter System".to_string(),
            StarType::Yellow,
            false,
            789,
        );
        
        // Gas giants should have moons
        let gas_giants: Vec<_> = system.planets.iter()
            .filter(|p| p.planet_type == PlanetType::GasGiant)
            .collect();
        
        if !gas_giants.is_empty() {
            assert!(!gas_giants[0].moons.is_empty());
        }
    }
    
    #[test]
    fn test_stations() {
        let system = StarSystem::generate(
            "STAR-000005".to_string(),
            "Trade Hub".to_string(),
            StarType::Yellow,
            true,
            321,
        );
        
        // Inhabited systems should have stations
        assert!(!system.stations.is_empty());
    }
    
    #[test]
    fn test_deterministic_generation() {
        let system1 = StarSystem::generate(
            "STAR-000006".to_string(),
            "Test".to_string(),
            StarType::Yellow,
            false,
            42,
        );
        
        let system2 = StarSystem::generate(
            "STAR-000006".to_string(),
            "Test".to_string(),
            StarType::Yellow,
            false,
            42,
        );
        
        // Same seed should produce same results
        assert_eq!(system1.planets.len(), system2.planets.len());
        assert_eq!(system1.planets[0].orbital_radius, system2.planets[0].orbital_radius);
    }
}
