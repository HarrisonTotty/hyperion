//! Contains the definition of a ship class.

use serde::{
    Deserialize,
    Deserializer,
    Serialize,
    Serializer
};

/// Represents a ship class bonus.
#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
#[serde(tag = "type")]
pub enum ShipBonus {
    /// Reduces the cost of the specified module by the specified percent
    /// amount.
    PercentModuleCostReduction { module: String, percent: u8 },

    /// Increases the base hit points of the specified module by the specified
    /// percent amount.
    PercentModuleHPBonus { module: String, percent: u16 },

    /// Reduces the tonnage of the specified module by the specified percent
    /// amount.
    PercentModuleTonnageReduction { module: String, percent: u8 }
}

/// Implements `std::fmt::Display` for `ShipBonus`.
impl std::fmt::Display for ShipBonus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShipBonus::PercentModuleCostReduction { module, percent } => write!(
                f,
                "-{p}% {m} module cost",
                p = percent,
                m = module
            ),
            ShipBonus::PercentModuleHPBonus { module, percent } => write!(
                f,
                "+{p}% {m} module hit points",
                p = percent,
                m = module
            ),
            ShipBonus::PercentModuleTonnageReduction { module , percent } => write!(
                f,
                "-{p}% {m} module tonnage",
                p = percent,
                m = module
            ),
        }
    }
}

/// Represents the definition of a ship class.
#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct ShipClass {
    /// Represents the bonuses provided by this ship class.
    pub bonuses: Vec<ShipBonus>,
    
    /// The base hit points provided by this ship class.
    pub hit_points: u32,

    /// The maximum number of modules allowed for a ship of this class.
    pub max_modules: u8,

    /// The maximum tonnage allowed for a ship of this class.
    pub max_tonnage: u32,
    
    /// Represents the name of this ship class.
    pub name: String,

    /// Represents the long description of this ship class.
    pub long_desc: String,

    /// Represents the role of this ship class.
    pub role: ShipRole,

    /// Represents the short description of this ship class.
    pub short_desc: String,

    /// Represents the size of this ship class.
    pub size: ShipSize,
}


/// Represents a ship class role.
#[derive(Debug)]
pub enum ShipRole {
    Defense,
    Offense,
    Support,
    Varied
}

impl Serialize for ShipRole {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        serializer.serialize_str(
            match *self {
                ShipRole::Defense => "Defense",
                ShipRole::Offense => "Offense",
                ShipRole::Support => "Support",
                _                 => "Varied",
            }
        )
    }
}
impl<'de> Deserialize<'de> for ShipRole {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        let s = String::deserialize(deserializer)?;
        Ok(match s.as_str() {
            "Defense" => ShipRole::Defense,
            "Offense" => ShipRole::Offense,
            "Support" => ShipRole::Support,
            _         => ShipRole::Varied,
        })
    }
}


/// Represents a ship class size.
#[derive(Debug)]
pub enum ShipSize {
    Large,
    Medium,
    Small
}

impl Serialize for ShipSize {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        serializer.serialize_str(
            match *self {
                ShipSize::Large  => "Large",
                ShipSize::Medium => "Medium",
                _                => "Small",
            }
        )
    }
}
impl<'de> Deserialize<'de> for ShipSize {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        let s = String::deserialize(deserializer)?;
        Ok(match s.as_str() {
            "Large"  => ShipSize::Large,
            "Medium" => ShipSize::Medium,
            _        => ShipSize::Small,
        })
    }
}
