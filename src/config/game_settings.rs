//! Game settings configuration
//!
//! This module defines global game settings loaded from `data/game.yaml`,
//! including economy parameters like team starting credits.

use serde::{Deserialize, Serialize};

/// Global game settings
///
/// These settings control game-wide parameters that affect all teams and players.
/// Loaded from `data/game.yaml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSettings {
    /// Starting credits granted to newly created teams
    ///
    /// Default: 1,000,000 credits
    #[serde(default = "default_team_starting_credits")]
    pub team_starting_credits: i64,
}

/// Default starting credits for teams (1,000,000)
fn default_team_starting_credits() -> i64 {
    1_000_000
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            team_starting_credits: default_team_starting_credits(),
        }
    }
}

impl GameSettings {
    /// Validate the game settings
    ///
    /// Returns an error if any settings are invalid.
    pub fn validate(&self) -> Result<(), String> {
        if self.team_starting_credits < 0 {
            return Err("team_starting_credits cannot be negative".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_game_settings() {
        let settings = GameSettings::default();
        assert_eq!(settings.team_starting_credits, 1_000_000);
    }

    #[test]
    fn test_validate_valid_settings() {
        let settings = GameSettings {
            team_starting_credits: 500_000,
        };
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn test_validate_zero_credits() {
        let settings = GameSettings {
            team_starting_credits: 0,
        };
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn test_validate_negative_credits() {
        let settings = GameSettings {
            team_starting_credits: -100,
        };
        let result = settings.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot be negative"));
    }

    #[test]
    fn test_deserialize_from_yaml() {
        let yaml = r#"
team_starting_credits: 2000000
"#;
        let settings: GameSettings = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(settings.team_starting_credits, 2_000_000);
    }

    #[test]
    fn test_deserialize_with_defaults() {
        let yaml = r#"
# Empty config, should use defaults
"#;
        let settings: GameSettings = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(settings.team_starting_credits, 1_000_000);
    }

    #[test]
    fn test_serialize_to_yaml() {
        let settings = GameSettings {
            team_starting_credits: 1_500_000,
        };
        let yaml = serde_yaml::to_string(&settings).unwrap();
        assert!(yaml.contains("team_starting_credits: 1500000"));
    }
}
