//! Weapon Tags System
//!
//! Implements weapon tag-based damage calculation, status effects, and combat modifiers.
//! 
//! Weapon tags modify how weapons behave in combat:
//! - Fire patterns (Beam, Burst, Pulse, SingleFire)
//! - Projectile types (Missile, Torpedo)
//! - Energy types (Photon, Plasma, Positron)
//! - Status effects (Ion, Graviton, Tachyon)
//! - Countermeasures (Decoy, Antimissile, Antitorpedo, Chaff)
//! - Fire modes (Manual, Automatic)

use crate::models::WeaponTag;
use std::collections::HashMap;

/// Damage calculation result with modifiers applied
#[derive(Debug, Clone, PartialEq)]
pub struct DamageResult {
    /// Total hull damage to apply
    pub hull_damage: f32,
    /// Total shield damage to apply (before shield modifiers)
    pub shield_damage: f32,
    /// Percentage of damage that bypasses shields (0.0 to 1.0)
    pub shield_bypass: f32,
    /// Number of projectiles/rounds fired
    pub projectile_count: u32,
    /// Whether this is continuous damage (beam weapons)
    pub is_continuous: bool,
    /// Status effect to apply, if any
    pub status_effect: Option<StatusEffect>,
}

/// Status effect that can be applied by weapon tags
#[derive(Debug, Clone, PartialEq)]
pub struct StatusEffect {
    /// Type of status effect
    pub effect_type: StatusEffectType,
    /// Duration in seconds
    pub duration: f32,
    /// Magnitude/strength of the effect
    pub magnitude: f32,
    /// Chance to apply (0.0 to 1.0)
    pub application_chance: f32,
}

/// Types of status effects
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatusEffectType {
    /// Jams communications and science, disables targeting
    IonJam,
    /// Increases effective weight by magnitude percentage
    GravitonWeight,
    /// Disables warp and jump drives
    TachyonWarpBlock,
}

/// Weapon tag damage calculator
pub struct WeaponTagCalculator {
    /// Tag configuration overrides
    tag_configs: HashMap<WeaponTag, TagModifiers>,
}

/// Modifiers for a specific weapon tag
#[derive(Debug, Clone)]
struct TagModifiers {
    /// Damage multiplier (1.0 = normal, 2.0 = double damage)
    damage_multiplier: f32,
    /// Shield damage multiplier (applied after base damage multiplier)
    shield_multiplier: f32,
    /// Percentage of damage that bypasses shields (0.0 to 1.0)
    shield_bypass: f32,
    /// Number of projectiles for fire pattern tags
    projectile_count: u32,
    /// Whether this is continuous damage
    is_continuous: bool,
    /// Status effect to apply
    status_effect: Option<StatusEffect>,
}

impl WeaponTagCalculator {
    /// Create a new calculator with default tag configurations
    pub fn new() -> Self {
        let mut tag_configs = HashMap::new();
        
        // Fire Pattern Tags
        tag_configs.insert(WeaponTag::Beam, TagModifiers {
            damage_multiplier: 1.0,
            shield_multiplier: 1.0,
            shield_bypass: 0.0,
            projectile_count: 1,
            is_continuous: true, // 1x damage per second
            status_effect: None,
        });
        
        tag_configs.insert(WeaponTag::Burst, TagModifiers {
            damage_multiplier: 1.0,
            shield_multiplier: 1.0,
            shield_bypass: 0.0,
            projectile_count: 3, // Fire 3 rounds
            is_continuous: false,
            status_effect: None,
        });
        
        tag_configs.insert(WeaponTag::Pulse, TagModifiers {
            damage_multiplier: 1.0,
            shield_multiplier: 1.0,
            shield_bypass: 0.0,
            projectile_count: 2, // Fire 2 rounds
            is_continuous: false,
            status_effect: None,
        });
        
        tag_configs.insert(WeaponTag::SingleFire, TagModifiers {
            damage_multiplier: 1.0,
            shield_multiplier: 1.0,
            shield_bypass: 0.0,
            projectile_count: 1, // Fire 1 round
            is_continuous: false,
            status_effect: None,
        });
        
        // Projectile Type Tags
        tag_configs.insert(WeaponTag::Missile, TagModifiers {
            damage_multiplier: 1.0,
            shield_multiplier: 1.0,
            shield_bypass: 0.0,
            projectile_count: 1,
            is_continuous: false,
            status_effect: None,
        });
        
        tag_configs.insert(WeaponTag::Torpedo, TagModifiers {
            damage_multiplier: 1.0,
            shield_multiplier: 1.0,
            shield_bypass: 0.0,
            projectile_count: 1,
            is_continuous: false,
            status_effect: None,
        });
        
        // Energy Type Tags
        tag_configs.insert(WeaponTag::Photon, TagModifiers {
            damage_multiplier: 1.0,
            shield_multiplier: 0.5, // Deal 0.5x damage to shields
            shield_bypass: 0.0,
            projectile_count: 1,
            is_continuous: false,
            status_effect: None,
        });
        
        tag_configs.insert(WeaponTag::Plasma, TagModifiers {
            damage_multiplier: 1.0,
            shield_multiplier: 2.0, // Deal 2x damage to shields
            shield_bypass: 0.0,
            projectile_count: 1,
            is_continuous: false,
            status_effect: None,
        });
        
        tag_configs.insert(WeaponTag::Positron, TagModifiers {
            damage_multiplier: 1.0,
            shield_multiplier: 1.0,
            shield_bypass: 0.25, // 25% damage bypasses shields
            projectile_count: 1,
            is_continuous: false,
            status_effect: None,
        });
        
        // Status Effect Tags
        tag_configs.insert(WeaponTag::Ion, TagModifiers {
            damage_multiplier: 0.6, // Reduced damage
            shield_multiplier: 1.0,
            shield_bypass: 0.0,
            projectile_count: 1,
            is_continuous: false,
            status_effect: Some(StatusEffect {
                effect_type: StatusEffectType::IonJam,
                duration: 10.0,
                magnitude: 1.0,
                application_chance: 0.8,
            }),
        });
        
        tag_configs.insert(WeaponTag::Graviton, TagModifiers {
            damage_multiplier: 0.5, // Reduced damage
            shield_multiplier: 1.0,
            shield_bypass: 0.0,
            projectile_count: 1,
            is_continuous: false,
            status_effect: Some(StatusEffect {
                effect_type: StatusEffectType::GravitonWeight,
                duration: 15.0,
                magnitude: 0.3, // 30% additional effective weight
                application_chance: 0.7,
            }),
        });
        
        tag_configs.insert(WeaponTag::Tachyon, TagModifiers {
            damage_multiplier: 0.4, // Reduced damage
            shield_multiplier: 1.0,
            shield_bypass: 0.0,
            projectile_count: 1,
            is_continuous: false,
            status_effect: Some(StatusEffect {
                effect_type: StatusEffectType::TachyonWarpBlock,
                duration: 20.0,
                magnitude: 1.0,
                application_chance: 0.9,
            }),
        });
        
        // Countermeasure Tags - these don't directly affect damage
        tag_configs.insert(WeaponTag::Decoy, TagModifiers {
            damage_multiplier: 0.0,
            shield_multiplier: 1.0,
            shield_bypass: 0.0,
            projectile_count: 1,
            is_continuous: false,
            status_effect: None,
        });
        
        tag_configs.insert(WeaponTag::Antimissile, TagModifiers {
            damage_multiplier: 0.3,
            shield_multiplier: 1.0,
            shield_bypass: 0.0,
            projectile_count: 1,
            is_continuous: false,
            status_effect: None,
        });
        
        tag_configs.insert(WeaponTag::Antitorpedo, TagModifiers {
            damage_multiplier: 0.5,
            shield_multiplier: 1.0,
            shield_bypass: 0.0,
            projectile_count: 1,
            is_continuous: false,
            status_effect: None,
        });
        
        tag_configs.insert(WeaponTag::Chaff, TagModifiers {
            damage_multiplier: 0.0,
            shield_multiplier: 1.0,
            shield_bypass: 0.0,
            projectile_count: 1,
            is_continuous: false,
            status_effect: None,
        });
        
        // Fire Mode Tags - these don't affect damage
        tag_configs.insert(WeaponTag::Manual, TagModifiers {
            damage_multiplier: 1.0,
            shield_multiplier: 1.0,
            shield_bypass: 0.0,
            projectile_count: 1,
            is_continuous: false,
            status_effect: None,
        });
        
        tag_configs.insert(WeaponTag::Automatic, TagModifiers {
            damage_multiplier: 1.0,
            shield_multiplier: 1.0,
            shield_bypass: 0.0,
            projectile_count: 1,
            is_continuous: false,
            status_effect: None,
        });
        
        Self { tag_configs }
    }
    
    /// Calculate damage with weapon tags applied
    /// 
    /// # Arguments
    /// * `base_damage` - Base weapon damage before modifiers
    /// * `tags` - List of weapon tags to apply
    /// 
    /// # Returns
    /// DamageResult with all modifiers applied
    pub fn calculate_damage(&self, base_damage: f32, tags: &[WeaponTag]) -> Result<DamageResult, String> {
        // Validate tag combinations
        self.validate_tags(tags)?;
        
        let mut damage_multiplier = 1.0;
        let mut shield_multiplier = 1.0;
        let mut shield_bypass = 0.0;
        let mut projectile_count = 1;
        let mut is_continuous = false;
        let mut status_effect = None;
        
        // Apply all tag modifiers
        for tag in tags {
            if let Some(modifiers) = self.tag_configs.get(tag) {
                // Multiply damage modifiers (multiplicative stacking)
                damage_multiplier *= modifiers.damage_multiplier;
                
                // Shield multipliers are multiplicative
                shield_multiplier *= modifiers.shield_multiplier;
                
                // Shield bypass is additive (capped at 1.0)
                shield_bypass = (shield_bypass + modifiers.shield_bypass).min(1.0);
                
                // Fire pattern determines projectile count (only one should be present)
                if modifiers.projectile_count > 1 || modifiers.is_continuous {
                    projectile_count = modifiers.projectile_count;
                    is_continuous = modifiers.is_continuous;
                }
                
                // Status effects don't stack - use the first one found
                if status_effect.is_none() && modifiers.status_effect.is_some() {
                    status_effect = modifiers.status_effect.clone();
                }
            }
        }
        
        // Calculate final damage values
        let modified_damage = base_damage * damage_multiplier;
        let shield_damage = modified_damage * shield_multiplier;
        let hull_damage = modified_damage;
        
        Ok(DamageResult {
            hull_damage,
            shield_damage,
            shield_bypass,
            projectile_count,
            is_continuous,
            status_effect,
        })
    }
    
    /// Validate that weapon tag combinations are valid
    /// 
    /// Checks for mutually exclusive tags:
    /// - Fire patterns: Beam, Burst, Pulse, SingleFire
    /// - Projectile types: Missile, Torpedo
    /// - Fire modes: Manual, Automatic
    pub fn validate_tags(&self, tags: &[WeaponTag]) -> Result<(), String> {
        // Check for multiple fire patterns
        let fire_patterns = [WeaponTag::Beam, WeaponTag::Burst, WeaponTag::Pulse, WeaponTag::SingleFire];
        let fire_pattern_count = tags.iter().filter(|t| fire_patterns.contains(t)).count();
        if fire_pattern_count > 1 {
            return Err("Weapon cannot have multiple fire pattern tags (Beam, Burst, Pulse, SingleFire)".to_string());
        }
        
        // Check for both Missile and Torpedo
        if tags.contains(&WeaponTag::Missile) && tags.contains(&WeaponTag::Torpedo) {
            return Err("Weapon cannot be both Missile and Torpedo".to_string());
        }
        
        // Check for both Manual and Automatic
        if tags.contains(&WeaponTag::Manual) && tags.contains(&WeaponTag::Automatic) {
            return Err("Weapon cannot be both Manual and Automatic".to_string());
        }
        
        Ok(())
    }
    
    /// Get the fire pattern tag from a list of tags
    pub fn get_fire_pattern(&self, tags: &[WeaponTag]) -> Option<WeaponTag> {
        let fire_patterns = [WeaponTag::Beam, WeaponTag::Burst, WeaponTag::Pulse, WeaponTag::SingleFire];
        tags.iter().find(|t| fire_patterns.contains(t)).copied()
    }
    
    /// Check if weapon has a specific status effect tag
    pub fn has_status_effect(&self, tags: &[WeaponTag]) -> bool {
        tags.iter().any(|t| matches!(t, WeaponTag::Ion | WeaponTag::Graviton | WeaponTag::Tachyon))
    }
    
    /// Check if weapon is a countermeasure
    pub fn is_countermeasure(&self, tags: &[WeaponTag]) -> bool {
        tags.iter().any(|t| matches!(t, 
            WeaponTag::Decoy | WeaponTag::Antimissile | WeaponTag::Antitorpedo | WeaponTag::Chaff
        ))
    }
}

impl Default for WeaponTagCalculator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_basic_damage_calculation() {
        let calculator = WeaponTagCalculator::new();
        let result = calculator.calculate_damage(100.0, &[]).unwrap();
        
        assert_eq!(result.hull_damage, 100.0);
        assert_eq!(result.shield_damage, 100.0);
        assert_eq!(result.shield_bypass, 0.0);
        assert_eq!(result.projectile_count, 1);
        assert!(!result.is_continuous);
        assert!(result.status_effect.is_none());
    }
    
    #[test]
    fn test_beam_weapon() {
        let calculator = WeaponTagCalculator::new();
        let result = calculator.calculate_damage(100.0, &[WeaponTag::Beam]).unwrap();
        
        assert_eq!(result.hull_damage, 100.0);
        assert_eq!(result.projectile_count, 1);
        assert!(result.is_continuous);
    }
    
    #[test]
    fn test_burst_weapon() {
        let calculator = WeaponTagCalculator::new();
        let result = calculator.calculate_damage(100.0, &[WeaponTag::Burst]).unwrap();
        
        assert_eq!(result.hull_damage, 100.0);
        assert_eq!(result.projectile_count, 3); // Fires 3 rounds
        assert!(!result.is_continuous);
    }
    
    #[test]
    fn test_pulse_weapon() {
        let calculator = WeaponTagCalculator::new();
        let result = calculator.calculate_damage(100.0, &[WeaponTag::Pulse]).unwrap();
        
        assert_eq!(result.hull_damage, 100.0);
        assert_eq!(result.projectile_count, 2); // Fires 2 rounds
        assert!(!result.is_continuous);
    }
    
    #[test]
    fn test_single_fire_weapon() {
        let calculator = WeaponTagCalculator::new();
        let result = calculator.calculate_damage(100.0, &[WeaponTag::SingleFire]).unwrap();
        
        assert_eq!(result.hull_damage, 100.0);
        assert_eq!(result.projectile_count, 1); // Fires 1 round
        assert!(!result.is_continuous);
    }
    
    #[test]
    fn test_photon_shield_damage() {
        let calculator = WeaponTagCalculator::new();
        let result = calculator.calculate_damage(100.0, &[WeaponTag::Photon]).unwrap();
        
        assert_eq!(result.hull_damage, 100.0);
        assert_eq!(result.shield_damage, 50.0); // 0.5x to shields
        assert_eq!(result.shield_bypass, 0.0);
    }
    
    #[test]
    fn test_plasma_shield_damage() {
        let calculator = WeaponTagCalculator::new();
        let result = calculator.calculate_damage(100.0, &[WeaponTag::Plasma]).unwrap();
        
        assert_eq!(result.hull_damage, 100.0);
        assert_eq!(result.shield_damage, 200.0); // 2x to shields
        assert_eq!(result.shield_bypass, 0.0);
    }
    
    #[test]
    fn test_positron_shield_bypass() {
        let calculator = WeaponTagCalculator::new();
        let result = calculator.calculate_damage(100.0, &[WeaponTag::Positron]).unwrap();
        
        assert_eq!(result.hull_damage, 100.0);
        assert_eq!(result.shield_damage, 100.0);
        assert_eq!(result.shield_bypass, 0.25); // 25% bypass
    }
    
    #[test]
    fn test_ion_status_effect() {
        let calculator = WeaponTagCalculator::new();
        let result = calculator.calculate_damage(100.0, &[WeaponTag::Ion]).unwrap();
        
        assert!((result.hull_damage - 60.0).abs() < 0.01); // 0.6x damage (with floating point tolerance)
        assert!(result.status_effect.is_some());
        
        let effect = result.status_effect.unwrap();
        assert_eq!(effect.effect_type, StatusEffectType::IonJam);
        assert_eq!(effect.duration, 10.0);
        assert_eq!(effect.application_chance, 0.8);
    }
    
    #[test]
    fn test_graviton_status_effect() {
        let calculator = WeaponTagCalculator::new();
        let result = calculator.calculate_damage(100.0, &[WeaponTag::Graviton]).unwrap();
        
        assert_eq!(result.hull_damage, 50.0); // 0.5x damage
        assert!(result.status_effect.is_some());
        
        let effect = result.status_effect.unwrap();
        assert_eq!(effect.effect_type, StatusEffectType::GravitonWeight);
        assert_eq!(effect.duration, 15.0);
        assert_eq!(effect.magnitude, 0.3); // 30% weight increase
        assert_eq!(effect.application_chance, 0.7);
    }
    
    #[test]
    fn test_tachyon_status_effect() {
        let calculator = WeaponTagCalculator::new();
        let result = calculator.calculate_damage(100.0, &[WeaponTag::Tachyon]).unwrap();
        
        assert_eq!(result.hull_damage, 40.0); // 0.4x damage
        assert!(result.status_effect.is_some());
        
        let effect = result.status_effect.unwrap();
        assert_eq!(effect.effect_type, StatusEffectType::TachyonWarpBlock);
        assert_eq!(effect.duration, 20.0);
        assert_eq!(effect.application_chance, 0.9);
    }
    
    #[test]
    fn test_combined_tags() {
        let calculator = WeaponTagCalculator::new();
        // Burst Photon weapon
        let result = calculator.calculate_damage(100.0, &[WeaponTag::Burst, WeaponTag::Photon]).unwrap();
        
        assert_eq!(result.hull_damage, 100.0);
        assert_eq!(result.shield_damage, 50.0); // Photon: 0.5x to shields
        assert_eq!(result.projectile_count, 3); // Burst: 3 rounds
    }
    
    #[test]
    fn test_validate_tags_conflicting_fire_patterns() {
        let calculator = WeaponTagCalculator::new();
        let result = calculator.validate_tags(&[WeaponTag::Beam, WeaponTag::Burst]);
        
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("fire pattern"));
    }
    
    #[test]
    fn test_validate_tags_missile_and_torpedo() {
        let calculator = WeaponTagCalculator::new();
        let result = calculator.validate_tags(&[WeaponTag::Missile, WeaponTag::Torpedo]);
        
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missile and Torpedo"));
    }
    
    #[test]
    fn test_validate_tags_manual_and_automatic() {
        let calculator = WeaponTagCalculator::new();
        let result = calculator.validate_tags(&[WeaponTag::Manual, WeaponTag::Automatic]);
        
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Manual and Automatic"));
    }
    
    #[test]
    fn test_get_fire_pattern() {
        let calculator = WeaponTagCalculator::new();
        
        assert_eq!(calculator.get_fire_pattern(&[WeaponTag::Beam]), Some(WeaponTag::Beam));
        assert_eq!(calculator.get_fire_pattern(&[WeaponTag::Burst, WeaponTag::Photon]), Some(WeaponTag::Burst));
        assert_eq!(calculator.get_fire_pattern(&[WeaponTag::Photon]), None);
    }
    
    #[test]
    fn test_has_status_effect() {
        let calculator = WeaponTagCalculator::new();
        
        assert!(calculator.has_status_effect(&[WeaponTag::Ion]));
        assert!(calculator.has_status_effect(&[WeaponTag::Graviton]));
        assert!(calculator.has_status_effect(&[WeaponTag::Tachyon]));
        assert!(!calculator.has_status_effect(&[WeaponTag::Beam]));
    }
    
    #[test]
    fn test_is_countermeasure() {
        let calculator = WeaponTagCalculator::new();
        
        assert!(calculator.is_countermeasure(&[WeaponTag::Decoy]));
        assert!(calculator.is_countermeasure(&[WeaponTag::Antimissile]));
        assert!(calculator.is_countermeasure(&[WeaponTag::Antitorpedo]));
        assert!(calculator.is_countermeasure(&[WeaponTag::Chaff]));
        assert!(!calculator.is_countermeasure(&[WeaponTag::Beam]));
    }
    
    #[test]
    fn test_antimissile_damage() {
        let calculator = WeaponTagCalculator::new();
        let result = calculator.calculate_damage(100.0, &[WeaponTag::Antimissile]).unwrap();
        
        assert!((result.hull_damage - 30.0).abs() < 0.01); // 0.3x damage (with floating point tolerance)
    }
    
    #[test]
    fn test_antitorpedo_damage() {
        let calculator = WeaponTagCalculator::new();
        let result = calculator.calculate_damage(100.0, &[WeaponTag::Antitorpedo]).unwrap();
        
        assert_eq!(result.hull_damage, 50.0); // 0.5x damage
    }
    
    #[test]
    fn test_decoy_no_damage() {
        let calculator = WeaponTagCalculator::new();
        let result = calculator.calculate_damage(100.0, &[WeaponTag::Decoy]).unwrap();
        
        assert_eq!(result.hull_damage, 0.0); // No damage
    }
    
    #[test]
    fn test_chaff_no_damage() {
        let calculator = WeaponTagCalculator::new();
        let result = calculator.calculate_damage(100.0, &[WeaponTag::Chaff]).unwrap();
        
        assert_eq!(result.hull_damage, 0.0); // No damage
    }
}
