//! Galaxy generation
//!
//! This module generates a procedural 3D galaxy with sectors, stars, and spatial distribution.

use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use serde::{Deserialize, Serialize};

/// A sector of the galaxy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GalaxySector {
    /// Sector coordinates (x, y, z)
    pub coordinates: (i32, i32, i32),
    /// Star density (0.0 - 1.0)
    pub star_density: f32,
    /// Sector type
    pub sector_type: SectorType,
    /// Notable features
    pub features: Vec<String>,
}

/// Type of galaxy sector
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SectorType {
    /// Dense core region
    Core,
    /// Spiral arm
    Arm,
    /// Space between arms
    InterArm,
    /// Outer rim
    Rim,
    /// Empty void
    Void,
}

/// A star in the galaxy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Star {
    /// Unique identifier
    pub id: String,
    /// Star name
    pub name: String,
    /// Position in 3D space
    pub position: [f64; 3],
    /// Star type
    pub star_type: StarType,
    /// Sector containing this star
    pub sector: (i32, i32, i32),
    /// Has inhabited planets
    pub inhabited: bool,
}

/// Type of star
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StarType {
    /// Blue giant
    BlueGiant,
    /// White main sequence
    White,
    /// Yellow main sequence (like Sol)
    Yellow,
    /// Orange dwarf
    Orange,
    /// Red dwarf
    RedDwarf,
    /// Neutron star
    Neutron,
    /// Black hole
    BlackHole,
}

/// Procedurally generated galaxy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Galaxy {
    /// Galaxy name
    pub name: String,
    /// Random seed used for generation
    pub seed: u64,
    /// Galaxy radius (in light years)
    pub radius: f64,
    /// Number of sectors per dimension
    pub sectors_per_dimension: i32,
    /// All sectors in the galaxy
    pub sectors: Vec<GalaxySector>,
    /// All stars in the galaxy
    pub stars: Vec<Star>,
}

impl Galaxy {
    /// Generate a new galaxy with the given parameters
    pub fn generate(name: String, seed: u64, radius: f64, num_stars: usize) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);
        let sectors_per_dimension = 10; // 10x10x10 grid of sectors
        
        // Generate sectors
        let mut sectors = Vec::new();
        for x in -5..5 {
            for y in -5..5 {
                for z in -5..5 {
                    let sector = Self::generate_sector(&mut rng, (x, y, z), radius);
                    sectors.push(sector);
                }
            }
        }
        
        // Generate stars
        let mut stars = Vec::new();
        for i in 0..num_stars {
            let star = Self::generate_star(&mut rng, i, &sectors, radius);
            stars.push(star);
        }
        
        Galaxy {
            name,
            seed,
            radius,
            sectors_per_dimension,
            sectors,
            stars,
        }
    }
    
    /// Generate a single sector
    fn generate_sector(rng: &mut StdRng, coordinates: (i32, i32, i32), radius: f64) -> GalaxySector {
        let (x, y, z) = coordinates;
        let distance_from_center = ((x * x + y * y + z * z) as f64).sqrt();
        let normalized_distance = distance_from_center / 7.0; // Normalize to 0-1 range
        
        // Determine sector type based on distance from center
        let sector_type = if normalized_distance < 0.2 {
            SectorType::Core
        } else if normalized_distance < 0.6 {
            // Spiral arms vs inter-arm based on angle
            let angle = (y as f64).atan2(x as f64);
            let arm_angle = (angle / std::f64::consts::PI * 2.0).sin().abs();
            if arm_angle > 0.5 {
                SectorType::Arm
            } else {
                SectorType::InterArm
            }
        } else if normalized_distance < 0.9 {
            SectorType::Rim
        } else {
            SectorType::Void
        };
        
        // Star density based on sector type
        let base_density = match sector_type {
            SectorType::Core => 0.9,
            SectorType::Arm => 0.7,
            SectorType::InterArm => 0.3,
            SectorType::Rim => 0.2,
            SectorType::Void => 0.05,
        };
        
        let star_density = base_density * rng.gen_range(0.8..1.2);
        
        // Generate features
        let mut features = Vec::new();
        if rng.gen_bool(0.1) {
            features.push("Nebula".to_string());
        }
        if rng.gen_bool(0.05) {
            features.push("Black Hole".to_string());
        }
        if rng.gen_bool(0.03) {
            features.push("Asteroid Field".to_string());
        }
        
        GalaxySector {
            coordinates,
            star_density,
            sector_type,
            features,
        }
    }
    
    /// Generate a single star
    fn generate_star(rng: &mut StdRng, index: usize, sectors: &[GalaxySector], radius: f64) -> Star {
        // Choose a random sector weighted by star density
        let total_density: f32 = sectors.iter().map(|s| s.star_density).sum();
        let mut roll = rng.gen_range(0.0..total_density);
        let mut chosen_sector = &sectors[0];
        
        for sector in sectors {
            roll -= sector.star_density;
            if roll <= 0.0 {
                chosen_sector = sector;
                break;
            }
        }
        
        // Generate position within sector
        let (sx, sy, sz) = chosen_sector.coordinates;
        let sector_size = radius / 5.0; // 10 sectors across diameter
        
        let x = (sx as f64 + rng.gen_range(-0.5..0.5)) * sector_size;
        let y = (sy as f64 + rng.gen_range(-0.5..0.5)) * sector_size;
        let z = (sz as f64 + rng.gen_range(-0.5..0.5)) * sector_size * 0.1; // Flatter galaxy
        
        // Determine star type
        let star_type = match rng.gen_range(0..100) {
            0..=1 => StarType::BlueGiant,
            2..=10 => StarType::White,
            11..=40 => StarType::Yellow,
            41..=70 => StarType::Orange,
            71..=95 => StarType::RedDwarf,
            96..=98 => StarType::Neutron,
            _ => StarType::BlackHole,
        };
        
        // Inhabited chance (higher for yellow/orange stars)
        let inhabited = match star_type {
            StarType::Yellow => rng.gen_bool(0.3),
            StarType::Orange => rng.gen_bool(0.2),
            StarType::White => rng.gen_bool(0.1),
            _ => rng.gen_bool(0.01),
        };
        
        Star {
            id: format!("STAR-{:06}", index),
            name: Self::generate_star_name(rng, index),
            position: [x, y, z],
            star_type,
            sector: chosen_sector.coordinates,
            inhabited,
        }
    }
    
    /// Generate a random star name
    fn generate_star_name(rng: &mut StdRng, index: usize) -> String {
        let prefixes = ["Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Zeta", "Eta", "Theta"];
        let suffixes = ["Centauri", "Draconis", "Orionis", "Cygni", "Lyrae", "Aquilae"];
        
        if rng.gen_bool(0.3) {
            // Catalog designation
            format!("HD {}", 100000 + index)
        } else {
            // Greek letter designation
            let prefix = prefixes[rng.gen_range(0..prefixes.len())];
            let suffix = suffixes[rng.gen_range(0..suffixes.len())];
            format!("{} {}", prefix, suffix)
        }
    }
    
    /// Get all stars in a sector
    pub fn stars_in_sector(&self, sector: (i32, i32, i32)) -> Vec<&Star> {
        self.stars.iter()
            .filter(|s| s.sector == sector)
            .collect()
    }
    
    /// Get nearby stars within radius
    pub fn nearby_stars(&self, position: [f64; 3], radius: f64) -> Vec<&Star> {
        self.stars.iter()
            .filter(|s| {
                let dx = s.position[0] - position[0];
                let dy = s.position[1] - position[1];
                let dz = s.position[2] - position[2];
                (dx * dx + dy * dy + dz * dz).sqrt() <= radius
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_galaxy_generation() {
        let galaxy = Galaxy::generate("Milky Way".to_string(), 42, 50000.0, 1000);
        
        assert_eq!(galaxy.name, "Milky Way");
        assert_eq!(galaxy.seed, 42);
        assert_eq!(galaxy.radius, 50000.0);
        assert_eq!(galaxy.sectors.len(), 1000); // 10x10x10
        assert_eq!(galaxy.stars.len(), 1000);
    }
    
    #[test]
    fn test_sector_types() {
        let galaxy = Galaxy::generate("Test".to_string(), 123, 50000.0, 100);
        
        // Should have core sectors
        let core_sectors: Vec<_> = galaxy.sectors.iter()
            .filter(|s| s.sector_type == SectorType::Core)
            .collect();
        assert!(!core_sectors.is_empty());
        
        // Core sectors should have high density
        for sector in core_sectors {
            assert!(sector.star_density > 0.7);
        }
    }
    
    #[test]
    fn test_star_distribution() {
        let galaxy = Galaxy::generate("Test".to_string(), 456, 50000.0, 500);
        
        // Most stars should be yellow, orange, or red dwarfs
        let common_stars = galaxy.stars.iter()
            .filter(|s| matches!(s.star_type, StarType::Yellow | StarType::Orange | StarType::RedDwarf))
            .count();
        
        assert!(common_stars > 400); // >80% should be common types
    }
    
    #[test]
    fn test_stars_in_sector() {
        let galaxy = Galaxy::generate("Test".to_string(), 789, 50000.0, 1000);
        
        let sector = (0, 0, 0); // Center sector
        let stars = galaxy.stars_in_sector(sector);
        
        // Center sector should have some stars
        assert!(!stars.is_empty());
    }
    
    #[test]
    fn test_nearby_stars() {
        let galaxy = Galaxy::generate("Test".to_string(), 321, 50000.0, 500);
        
        let position = [0.0, 0.0, 0.0];
        let nearby = galaxy.nearby_stars(position, 5000.0);
        
        // Should find some stars near center
        assert!(!nearby.is_empty());
        
        // All should be within radius
        for star in nearby {
            let dx = star.position[0] - position[0];
            let dy = star.position[1] - position[1];
            let dz = star.position[2] - position[2];
            let distance = (dx * dx + dy * dy + dz * dz).sqrt();
            assert!(distance <= 5000.0);
        }
    }
    
    #[test]
    fn test_deterministic_generation() {
        let galaxy1 = Galaxy::generate("Test".to_string(), 42, 50000.0, 100);
        let galaxy2 = Galaxy::generate("Test".to_string(), 42, 50000.0, 100);
        
        // Same seed should produce same results
        assert_eq!(galaxy1.stars.len(), galaxy2.stars.len());
        assert_eq!(galaxy1.stars[0].position, galaxy2.stars[0].position);
        assert_eq!(galaxy1.stars[0].star_type, galaxy2.stars[0].star_type);
    }
}
