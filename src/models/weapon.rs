//! Weapon system models
//!
//! Defines weapon tags, fire modes, and weapon structures.

use serde::{Deserialize, Serialize};

/// Enumeration of all weapon tags/modifiers
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum WeaponTag {
    // Fire Pattern Tags
    /// Weapon fires in a continuous stream, dealing 1x damage per second
    Beam,
    /// Weapon fires in bursts of 3 rounds
    Burst,
    /// Weapon fires in bursts of 2 rounds
    Pulse,
    /// Weapon fires one projectile at a time
    #[serde(rename = "Single-Fire")]
    SingleFire,
    
    // Projectile Type Tags
    /// Guided projectile with high velocity and small warhead
    Missile,
    /// Unguided projectile with large warhead and slow velocity
    Torpedo,
    
    // Energy Type Tags
    /// Photon-based weapon, deals 0.5x damage to shields
    Photon,
    /// Plasma-based weapon, deals 2x damage to shields
    Plasma,
    /// Positron-based weapon, 25% damage bypasses shields
    Positron,
    
    // Status Effect Tags
    /// Ion-based weapon, jams communications and science, disables targeting
    Ion,
    /// Graviton-based weapon, adds 30% effective weight (non-stacking)
    Graviton,
    /// Tachyon-based weapon, disables warp and jump drives
    Tachyon,
    
    // Countermeasure Tags
    /// False scan signature to waste enemy countermeasures
    Decoy,
    /// Targets other missiles
    Antimissile,
    /// Targets other torpedos
    Antitorpedo,
    /// Jams missiles in area without detonating
    Chaff,
    
    // Fire Mode Tags
    /// Must be manually fired
    Manual,
    /// Can automatically fire when target is locked
    Automatic,
}

/// Fire mode for weapons
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum WeaponFireMode {
    /// Weapon must be manually triggered
    Manual,
    /// Weapon can fire automatically when target is acquired
    Automatic,
}

/// Represents a weapon system (legacy, use WeaponInstance for equipped weapons)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Weapon {
    pub id: String,
    pub name: String,
    pub damage: f32,
    pub range: f32,
    pub fire_rate: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weapon_tags() {
        // Test that weapon tags can be created and compared
        let tags = vec![WeaponTag::Beam, WeaponTag::Photon, WeaponTag::Automatic];
        assert!(tags.contains(&WeaponTag::Beam));
        assert!(tags.contains(&WeaponTag::Photon));
        assert!(!tags.contains(&WeaponTag::Missile));
    }

    #[test]
    fn test_weapon_creation() {
        let weapon = Weapon {
            id: "laser_1".to_string(),
            name: "Laser Cannon".to_string(),
            damage: 25.0,
            range: 1000.0,
            fire_rate: 2.0,
        };
        assert_eq!(weapon.name, "Laser Cannon");
    }
}
