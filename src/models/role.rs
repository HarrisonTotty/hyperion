//! Ship role definitions
//!
//! Defines all crew positions available on ships.

use serde::{Deserialize, Serialize};

/// Enumeration of all ship crew positions
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ShipRole {
    /// Captain - Command authority, crew reassignment, captain's log
    Captain,
    /// Helm - Ship piloting, navigation, docking, FTL drives
    Helm,
    /// Engineering - Power/cooling allocation, repairs, damage monitoring
    Engineering,
    /// Science Officer - Scanning, threat detection, navigation support
    Science,
    /// Communications Officer - Ship-to-ship messages, docking requests, fighter commands
    #[serde(rename = "comms")]
    Communications,
    /// Countermeasures Officer - Shields, anti-missile systems, point defense
    Countermeasures,
    /// Directed-Energy Weapons Officer - Lasers, beams, radial weapons
    EnergyWeapons,
    /// Kinetic Weapons Officer - Railguns, cannons, ballistic weapons
    KineticWeapons,
    /// Missile Weapons Officer - Missiles, torpedos, guided munitions
    MissileWeapons,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ship_roles() {
        let roles = vec![
            ShipRole::Captain,
            ShipRole::Helm,
            ShipRole::Engineering,
            ShipRole::Science,
            ShipRole::Communications,
            ShipRole::Countermeasures,
            ShipRole::EnergyWeapons,
            ShipRole::KineticWeapons,
            ShipRole::MissileWeapons,
        ];
        assert_eq!(roles.len(), 9);
    }
}
