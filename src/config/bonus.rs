use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for bonus type metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BonusConfig {
    /// All bonus type definitions
    pub bonuses: Vec<BonusMetadata>,
    /// Category definitions
    pub categories: Vec<CategoryMetadata>,
}

/// Metadata for a specific bonus type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BonusMetadata {
    /// Unique identifier (matches bonus key in ship classes)
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Description of what the bonus does
    pub description: String,
    /// Category this bonus belongs to
    pub category: String,
    /// How to format the value (percentage, absolute, multiplier)
    pub format: BonusFormat,
    /// What this bonus applies to
    pub applies_to: Vec<String>,
}

/// Category grouping for bonuses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryMetadata {
    /// Unique identifier
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Description of the category
    pub description: String,
    /// Color code for UI display
    pub color: String,
    /// Icon for UI display
    pub icon: String,
}

/// How to format bonus values
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BonusFormat {
    /// Display as percentage (0.25 → +25%)
    Percentage,
    /// Display as absolute value (+10)
    Absolute,
    /// Display as multiplier (1.5x)
    Multiplier,
}

impl BonusConfig {
    /// Load bonus configuration from YAML file
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: BonusConfig = serde_yaml::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }

    /// Validate the bonus configuration
    fn validate(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Check for duplicate bonus IDs
        let mut seen_ids = std::collections::HashSet::new();
        for bonus in &self.bonuses {
            if !seen_ids.insert(&bonus.id) {
                return Err(format!("Duplicate bonus ID: {}", bonus.id).into());
            }
        }

        // Check that all bonus categories exist
        let category_ids: std::collections::HashSet<_> =
            self.categories.iter().map(|c| c.id.as_str()).collect();
        for bonus in &self.bonuses {
            if !category_ids.contains(bonus.category.as_str()) {
                return Err(format!(
                    "Bonus '{}' references unknown category '{}'",
                    bonus.id, bonus.category
                )
                .into());
            }
        }

        Ok(())
    }

    /// Get metadata for a specific bonus ID
    pub fn get_bonus(&self, id: &str) -> Option<&BonusMetadata> {
        self.bonuses.iter().find(|b| b.id == id)
    }

    /// Get category metadata
    pub fn get_category(&self, id: &str) -> Option<&CategoryMetadata> {
        self.categories.iter().find(|c| c.id == id)
    }

    /// Format a bonus value for display
    pub fn format_bonus(&self, id: &str, value: f32) -> String {
        let bonus = match self.get_bonus(id) {
            Some(b) => b,
            None => return format!("{}: {}", id, value),
        };

        let sign = if value >= 0.0 { "+" } else { "" };
        
        match bonus.format {
            BonusFormat::Percentage => {
                format!("{}{:.0}%", sign, value * 100.0)
            }
            BonusFormat::Absolute => {
                format!("{}{}", sign, value)
            }
            BonusFormat::Multiplier => {
                format!("{}x", value)
            }
        }
    }

    /// Format multiple bonuses grouped by category
    pub fn format_bonuses_by_category(
        &self,
        bonuses: &HashMap<String, f32>,
    ) -> HashMap<String, Vec<FormattedBonus>> {
        let mut result: HashMap<String, Vec<FormattedBonus>> = HashMap::new();

        for (bonus_id, value) in bonuses {
            if let Some(bonus_meta) = self.get_bonus(bonus_id) {
                let formatted = FormattedBonus {
                    id: bonus_id.clone(),
                    name: bonus_meta.name.clone(),
                    description: bonus_meta.description.clone(),
                    value: *value,
                    formatted_value: self.format_bonus(bonus_id, *value),
                    applies_to: bonus_meta.applies_to.clone(),
                };

                result
                    .entry(bonus_meta.category.clone())
                    .or_insert_with(Vec::new)
                    .push(formatted);
            }
        }

        result
    }
}

/// A formatted bonus ready for display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormattedBonus {
    /// Bonus ID
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Description
    pub description: String,
    /// Raw numeric value
    pub value: f32,
    /// Formatted value string
    pub formatted_value: String,
    /// What the bonus applies to
    pub applies_to: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_percentage() {
        let config = BonusConfig {
            bonuses: vec![BonusMetadata {
                id: "test_bonus".to_string(),
                name: "Test Bonus".to_string(),
                description: "A test bonus".to_string(),
                category: "combat".to_string(),
                format: BonusFormat::Percentage,
                applies_to: vec!["weapons".to_string()],
            }],
            categories: vec![CategoryMetadata {
                id: "combat".to_string(),
                name: "Combat".to_string(),
                description: "Combat bonuses".to_string(),
                color: "#ff0000".to_string(),
                icon: "⚔️".to_string(),
            }],
        };

        assert_eq!(config.format_bonus("test_bonus", 0.25), "+25%");
        assert_eq!(config.format_bonus("test_bonus", -0.15), "-15%");
    }

    #[test]
    fn test_format_bonuses_by_category() {
        let config = BonusConfig {
            bonuses: vec![
                BonusMetadata {
                    id: "weapon_damage".to_string(),
                    name: "Weapon Damage".to_string(),
                    description: "Increases damage".to_string(),
                    category: "combat".to_string(),
                    format: BonusFormat::Percentage,
                    applies_to: vec!["weapons".to_string()],
                },
                BonusMetadata {
                    id: "shield_capacity".to_string(),
                    name: "Shield Capacity".to_string(),
                    description: "Increases shields".to_string(),
                    category: "defense".to_string(),
                    format: BonusFormat::Percentage,
                    applies_to: vec!["shields".to_string()],
                },
            ],
            categories: vec![
                CategoryMetadata {
                    id: "combat".to_string(),
                    name: "Combat".to_string(),
                    description: "Combat bonuses".to_string(),
                    color: "#ff0000".to_string(),
                    icon: "⚔️".to_string(),
                },
                CategoryMetadata {
                    id: "defense".to_string(),
                    name: "Defense".to_string(),
                    description: "Defense bonuses".to_string(),
                    color: "#0000ff".to_string(),
                    icon: "🛡️".to_string(),
                },
            ],
        };

        let mut bonuses = HashMap::new();
        bonuses.insert("weapon_damage".to_string(), 0.20);
        bonuses.insert("shield_capacity".to_string(), 0.30);

        let formatted = config.format_bonuses_by_category(&bonuses);
        
        assert_eq!(formatted.len(), 2);
        assert!(formatted.contains_key("combat"));
        assert!(formatted.contains_key("defense"));
        assert_eq!(formatted["combat"][0].formatted_value, "+20%");
        assert_eq!(formatted["defense"][0].formatted_value, "+30%");
    }

    #[test]
    fn test_validate_missing_category() {
        let config = BonusConfig {
            bonuses: vec![BonusMetadata {
                id: "test_bonus".to_string(),
                name: "Test".to_string(),
                description: "Test".to_string(),
                category: "nonexistent".to_string(),
                format: BonusFormat::Percentage,
                applies_to: vec![],
            }],
            categories: vec![],
        };

        assert!(config.validate().is_err());
    }
}
