//! Weapon configuration
//!
//! Defines weapon specifications with tag support.

use serde::{Deserialize, Serialize};
use crate::models::WeaponTag;

/// Weapon configuration from YAML files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaponConfig {
    /// Display name
    pub name: String,
    /// Model designation
    pub model: String,
    /// Weapon kind/category
    pub kind: String,
    /// Manufacturer
    pub manufacturer: String,
    /// Description (field name 'desc' in YAML)
    #[serde(rename = "desc")]
    pub description: String,
    /// Build cost
    pub cost: f32,
    /// Weight in kg (optional for some weapon types)
    #[serde(default)]
    pub weight: f32,
    
    // Weapon type-specific fields (missiles, DE weapons, kinetic)
    /// Fire delay / reload time
    #[serde(default)]
    pub fire_delay: f32,
    #[serde(default)]
    pub reload_time: f32,
    #[serde(default)]
    pub recharge_time: f32,
    
    /// Projectile speed
    #[serde(default)]
    pub speed: f32,
    #[serde(default)]
    pub velocity: f32,
    
    /// Range
    #[serde(default)]
    pub max_range: f32,
    #[serde(default)]
    pub effective_range: f32,
    
    /// Damage fields
    #[serde(default)]
    pub damage: f32,
    #[serde(default)]
    pub impact_damage: f32,
    #[serde(default)]
    pub blast_damage: f32,
    #[serde(default)]
    pub blast_radius: f32,
    
    /// Energy consumption
    #[serde(default)]
    pub energy_consumption: f32,
    
    /// Missile-specific
    #[serde(default)]
    pub forward_thrust: f32,
    #[serde(default)]
    pub max_turn_rate: f32,
    #[serde(default)]
    pub load_time: f32,
    #[serde(default)]
    pub lifetime: f32,
    #[serde(default)]
    pub max_speed: f32,
    
    /// Kinetic-specific
    #[serde(default)]
    pub num_projectiles: u32,
    #[serde(default)]
    pub ammo_consumption: u32,
    #[serde(default)]
    pub accuracy: f32,
    
    /// Weapon tags (fire patterns, energy types, status effects, etc.)
    #[serde(default)]
    pub tags: Vec<WeaponTag>,
    
    /// Derived field (not in YAML, populated from filename)
    #[serde(skip)]
    pub id: String,
}

impl WeaponConfig {
    /// Set the ID from the filename
    pub fn set_id(&mut self, id: String) {
        self.id = id;
    }
}

/// Ammunition type configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmmunitionConfig {
    /// Display name
    pub name: String,
    /// Ammunition type (e.g., "shell", "slug")
    #[serde(rename = "type")]
    pub ammo_type: String,
    /// Ammunition size (e.g., "50mm", "100mm")
    pub size: String,
    /// Description (field name 'desc' in YAML)
    #[serde(rename = "desc")]
    pub description: String,
    /// Build cost
    pub cost: f32,
    /// Weight in kg
    pub weight: f32,
    /// Impact damage
    pub impact_damage: f32,
    /// Blast radius
    #[serde(default)]
    pub blast_radius: f32,
    /// Blast damage
    #[serde(default)]
    pub blast_damage: f32,
    /// Projectile velocity
    pub velocity: f32,
    /// Armor penetration rating
    pub armor_penetration: f32,
    
    /// Derived field (not in YAML, populated from filename)
    #[serde(skip)]
    pub id: String,
}

impl AmmunitionConfig {
    /// Set the ID from the filename
    pub fn set_id(&mut self, id: String) {
        self.id = id;
    }
}

/// Kinetic weapon "kind" configuration (railgun, cannon, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KineticWeaponKind {
    /// Unique kind identifier
    pub id: String,
    /// Display name
    pub name: String,
    /// Description
    pub description: String,
    /// Compatible ammunition types
    #[serde(default)]
    pub compatible_ammunition: Vec<String>,
    /// Damage modifier for this kind
    #[serde(default = "default_damage_modifier")]
    pub damage_modifier: f32,
    /// Fire rate modifier for this kind
    #[serde(default = "default_fire_rate_modifier")]
    pub fire_rate_modifier: f32,
}

fn default_damage_modifier() -> f32 {
    1.0
}

fn default_fire_rate_modifier() -> f32 {
    1.0
}

impl WeaponConfig {
    /// Validate weapon configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() {
            return Err("Weapon ID cannot be empty".to_string());
        }
        if self.name.is_empty() {
            return Err("Weapon name cannot be empty".to_string());
        }
        if self.kind.is_empty() {
            return Err(format!("Weapon {} must have a kind", self.id));
        }
        if self.weight < 0.0 {
            return Err(format!("Weapon {} cannot have negative weight", self.id));
        }
        if self.cost < 0.0 {
            return Err(format!("Weapon {} cannot have negative cost", self.id));
        }
        
        // Validate tag combinations
        self.validate_tags()?;
        
        Ok(())
    }

    /// Validate weapon tag combinations
    fn validate_tags(&self) -> Result<(), String> {
        let has_beam = self.tags.contains(&WeaponTag::Beam);
        let has_burst = self.tags.contains(&WeaponTag::Burst);
        let has_pulse = self.tags.contains(&WeaponTag::Pulse);
        let has_single = self.tags.contains(&WeaponTag::SingleFire);
        
        // Check for mutually exclusive fire patterns
        let fire_pattern_count = [has_beam, has_burst, has_pulse, has_single].iter().filter(|&&x| x).count();
        if fire_pattern_count > 1 {
            return Err(format!("Weapon {} has multiple fire pattern tags (Beam, Burst, Pulse, SingleFire are mutually exclusive)", self.id));
        }

        let has_missile = self.tags.contains(&WeaponTag::Missile);
        let has_torpedo = self.tags.contains(&WeaponTag::Torpedo);
        
        // Missiles and torpedos are mutually exclusive
        if has_missile && has_torpedo {
            return Err(format!("Weapon {} cannot be both Missile and Torpedo", self.id));
        }

        let has_manual = self.tags.contains(&WeaponTag::Manual);
        let has_automatic = self.tags.contains(&WeaponTag::Automatic);
        
        // Manual and Automatic are mutually exclusive
        if has_manual && has_automatic {
            return Err(format!("Weapon {} cannot be both Manual and Automatic", self.id));
        }

        Ok(())
    }

    /// Check if weapon is a beam weapon
    pub fn is_beam(&self) -> bool {
        self.tags.contains(&WeaponTag::Beam)
    }

    /// Check if weapon can fire automatically
    pub fn is_automatic(&self) -> bool {
        self.tags.contains(&WeaponTag::Automatic)
    }
}

/// Configuration for weapon tag effects
/// 
/// Defines how each weapon tag modifies weapon behavior, damage, and status effects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaponTagConfig {
    /// The weapon tag this config applies to
    pub tag: WeaponTag,
    
    /// Display name for this tag
    pub name: String,
    
    /// Description of the tag's effects
    pub description: String,
    
    /// Damage multiplier applied (1.0 = no change, 2.0 = double damage)
    pub damage_multiplier: f32,
    
    /// Status effect applied on hit (if any)
    pub status_effect: Option<StatusEffectConfig>,
    
    /// Additional power draw multiplier (1.0 = normal, 1.5 = 50% more power)
    pub power_multiplier: f32,
    
    /// Range multiplier (1.0 = normal, 0.5 = half range)
    pub range_multiplier: f32,
}

/// Configuration for a status effect applied by weapon tags
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusEffectConfig {
    /// Type of status effect (Ion, Graviton, Tachyon)
    pub effect_type: String,
    
    /// Duration of the effect in seconds
    pub duration: f32,
    
    /// Chance to apply effect (0.0 to 1.0)
    pub application_chance: f32,
    
    /// Magnitude of the effect (e.g., 0.3 for 30% weight increase)
    pub magnitude: f32,
}

impl WeaponTagConfig {
    /// Get the default configuration for a weapon tag
    pub fn default_for_tag(tag: WeaponTag) -> Self {
        match tag {
            // Firing pattern tags
            WeaponTag::Beam => Self {
                tag,
                name: "Beam".to_string(),
                description: "Continuous energy beam with instant travel".to_string(),
                damage_multiplier: 0.8,
                status_effect: None,
                power_multiplier: 1.5,
                range_multiplier: 1.2,
            },
            WeaponTag::Burst => Self {
                tag,
                name: "Burst".to_string(),
                description: "Fires multiple shots in rapid succession".to_string(),
                damage_multiplier: 0.7,
                status_effect: None,
                power_multiplier: 1.2,
                range_multiplier: 1.0,
            },
            WeaponTag::Pulse => Self {
                tag,
                name: "Pulse".to_string(),
                description: "Fires discrete energy pulses".to_string(),
                damage_multiplier: 1.0,
                status_effect: None,
                power_multiplier: 1.0,
                range_multiplier: 1.0,
            },
            WeaponTag::SingleFire => Self {
                tag,
                name: "Single Fire".to_string(),
                description: "Fires one shot per trigger".to_string(),
                damage_multiplier: 1.2,
                status_effect: None,
                power_multiplier: 0.8,
                range_multiplier: 1.0,
            },
            
            // Projectile type tags
            WeaponTag::Missile => Self {
                tag,
                name: "Missile".to_string(),
                description: "Self-propelled guided projectile".to_string(),
                damage_multiplier: 1.5,
                status_effect: None,
                power_multiplier: 0.5,
                range_multiplier: 1.5,
            },
            WeaponTag::Torpedo => Self {
                tag,
                name: "Torpedo".to_string(),
                description: "Heavy slow-moving projectile with high damage".to_string(),
                damage_multiplier: 2.5,
                status_effect: None,
                power_multiplier: 0.3,
                range_multiplier: 1.3,
            },
            
            // Energy type tags
            WeaponTag::Photon => Self {
                tag,
                name: "Photon".to_string(),
                description: "Light-based energy weapon".to_string(),
                damage_multiplier: 1.1,
                status_effect: None,
                power_multiplier: 1.1,
                range_multiplier: 1.3,
            },
            WeaponTag::Plasma => Self {
                tag,
                name: "Plasma".to_string(),
                description: "Superheated ionized gas".to_string(),
                damage_multiplier: 1.3,
                status_effect: None,
                power_multiplier: 1.4,
                range_multiplier: 0.8,
            },
            WeaponTag::Positron => Self {
                tag,
                name: "Positron".to_string(),
                description: "Antimatter particle beam".to_string(),
                damage_multiplier: 1.5,
                status_effect: None,
                power_multiplier: 2.0,
                range_multiplier: 1.0,
            },
            
            // Status effect tags
            WeaponTag::Ion => Self {
                tag,
                name: "Ion".to_string(),
                description: "Jams communications and science systems".to_string(),
                damage_multiplier: 0.6,
                status_effect: Some(StatusEffectConfig {
                    effect_type: "Ion".to_string(),
                    duration: 10.0,
                    application_chance: 0.8,
                    magnitude: 1.0,
                }),
                power_multiplier: 1.2,
                range_multiplier: 1.0,
            },
            WeaponTag::Graviton => Self {
                tag,
                name: "Graviton".to_string(),
                description: "Increases target's effective weight by 30%".to_string(),
                damage_multiplier: 0.5,
                status_effect: Some(StatusEffectConfig {
                    effect_type: "Graviton".to_string(),
                    duration: 15.0,
                    application_chance: 0.7,
                    magnitude: 0.3,
                }),
                power_multiplier: 1.5,
                range_multiplier: 0.9,
            },
            WeaponTag::Tachyon => Self {
                tag,
                name: "Tachyon".to_string(),
                description: "Disables FTL/warp drive capabilities".to_string(),
                damage_multiplier: 0.4,
                status_effect: Some(StatusEffectConfig {
                    effect_type: "Tachyon".to_string(),
                    duration: 20.0,
                    application_chance: 0.9,
                    magnitude: 1.0,
                }),
                power_multiplier: 1.8,
                range_multiplier: 1.1,
            },
            
            // Countermeasure tags
            WeaponTag::Decoy => Self {
                tag,
                name: "Decoy".to_string(),
                description: "Distracts incoming missiles".to_string(),
                damage_multiplier: 0.0,
                status_effect: None,
                power_multiplier: 0.5,
                range_multiplier: 0.5,
            },
            WeaponTag::Antimissile => Self {
                tag,
                name: "Anti-Missile".to_string(),
                description: "Intercepts incoming missiles".to_string(),
                damage_multiplier: 0.3,
                status_effect: None,
                power_multiplier: 0.8,
                range_multiplier: 0.7,
            },
            WeaponTag::Antitorpedo => Self {
                tag,
                name: "Anti-Torpedo".to_string(),
                description: "Intercepts incoming torpedoes".to_string(),
                damage_multiplier: 0.5,
                status_effect: None,
                power_multiplier: 1.0,
                range_multiplier: 0.8,
            },
            WeaponTag::Chaff => Self {
                tag,
                name: "Chaff".to_string(),
                description: "Deploys interference particles".to_string(),
                damage_multiplier: 0.0,
                status_effect: None,
                power_multiplier: 0.3,
                range_multiplier: 0.3,
            },
            
            // Fire control tags (these don't have direct effects)
            WeaponTag::Manual => Self {
                tag,
                name: "Manual".to_string(),
                description: "Requires manual targeting by crew".to_string(),
                damage_multiplier: 1.0,
                status_effect: None,
                power_multiplier: 1.0,
                range_multiplier: 1.0,
            },
            WeaponTag::Automatic => Self {
                tag,
                name: "Automatic".to_string(),
                description: "Automatically targets threats".to_string(),
                damage_multiplier: 1.0,
                status_effect: None,
                power_multiplier: 1.0,
                range_multiplier: 1.0,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weapon_validation() {
        let mut valid = WeaponConfig {
            name: "Phaser Array Mk1".to_string(),
            model: "Mk1".to_string(),
            kind: "directed-energy".to_string(),
            manufacturer: "Starfleet".to_string(),
            description: "Standard phaser".to_string(),
            cost: 5000.0,
            weight: 500.0,
            fire_delay: 0.0,
            reload_time: 0.0,
            recharge_time: 1.5,
            speed: 0.0,
            velocity: 0.0,
            max_range: 1000.0,
            effective_range: 0.0,
            damage: 50.0,
            impact_damage: 0.0,
            blast_damage: 0.0,
            blast_radius: 0.0,
            energy_consumption: 25.0,
            forward_thrust: 0.0,
            max_turn_rate: 0.0,
            load_time: 0.0,
            lifetime: 0.0,
            max_speed: 0.0,
            num_projectiles: 0,
            ammo_consumption: 0,
            accuracy: 0.0,
            tags: vec![WeaponTag::Beam, WeaponTag::Photon, WeaponTag::Automatic],
            id: String::new(),
        };
        valid.set_id("phaser_mk1".to_string());
        assert!(valid.validate().is_ok());
        assert!(valid.is_beam());
        assert!(valid.is_automatic());
    }

    #[test]
    fn test_weapon_invalid_tags() {
        let mut conflicting_fire_patterns = WeaponConfig {
            name: "Bad".to_string(),
            model: "v1".to_string(),
            kind: "energy".to_string(),
            manufacturer: "Test".to_string(),
            description: "".to_string(),
            cost: 1000.0,
            weight: 100.0,
            fire_delay: 0.0,
            reload_time: 0.0,
            recharge_time: 0.0,
            speed: 0.0,
            velocity: 0.0,
            max_range: 100.0,
            effective_range: 0.0,
            damage: 10.0,
            impact_damage: 0.0,
            blast_damage: 0.0,
            blast_radius: 0.0,
            energy_consumption: 5.0,
            forward_thrust: 0.0,
            max_turn_rate: 0.0,
            load_time: 0.0,
            lifetime: 0.0,
            max_speed: 0.0,
            num_projectiles: 0,
            ammo_consumption: 0,
            accuracy: 0.0,
            tags: vec![WeaponTag::Beam, WeaponTag::Burst], // Conflicting!
            id: String::new(),
        };
        conflicting_fire_patterns.set_id("bad_weapon".to_string());
        assert!(conflicting_fire_patterns.validate().is_err());

        let mut conflicting_projectile = conflicting_fire_patterns.clone();
        conflicting_projectile.tags = vec![WeaponTag::Missile, WeaponTag::Torpedo]; // Conflicting!
        conflicting_projectile.set_id("bad_weapon2".to_string());
        assert!(conflicting_projectile.validate().is_err());
    }

    #[test]
    fn test_ammunition_config() {
        let mut ammo = AmmunitionConfig {
            name: "Armor Piercing Rounds".to_string(),
            ammo_type: "slug".to_string(),
            size: "50mm".to_string(),
            description: "High velocity rounds".to_string(),
            cost: 100.0,
            weight: 5.0,
            impact_damage: 150.0,
            blast_radius: 0.0,
            blast_damage: 0.0,
            velocity: 2000.0,
            armor_penetration: 0.8,
            id: String::new(),
        };
        ammo.set_id("armor_piercing".to_string());
        assert_eq!(ammo.velocity, 2000.0);
    }

    #[test]
    fn test_weapon_tag_config_defaults() {
        let ion_config = WeaponTagConfig::default_for_tag(WeaponTag::Ion);
        assert_eq!(ion_config.name, "Ion");
        assert!(ion_config.status_effect.is_some());
        
        let status = ion_config.status_effect.unwrap();
        assert_eq!(status.effect_type, "Ion");
        assert_eq!(status.duration, 10.0);
        assert_eq!(status.application_chance, 0.8);
        
        let beam_config = WeaponTagConfig::default_for_tag(WeaponTag::Beam);
        assert_eq!(beam_config.power_multiplier, 1.5);
        assert_eq!(beam_config.range_multiplier, 1.2);
    }

    #[test]
    fn test_weapon_tag_config_status_effects() {
        let graviton = WeaponTagConfig::default_for_tag(WeaponTag::Graviton);
        assert!(graviton.status_effect.is_some());
        let effect = graviton.status_effect.unwrap();
        assert_eq!(effect.magnitude, 0.3); // 30% weight increase
        
        let tachyon = WeaponTagConfig::default_for_tag(WeaponTag::Tachyon);
        assert!(tachyon.status_effect.is_some());
        
        let photon = WeaponTagConfig::default_for_tag(WeaponTag::Photon);
        assert!(photon.status_effect.is_none()); // No status effect
    }
}
