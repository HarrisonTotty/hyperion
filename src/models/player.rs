//! Player and team models
//!
//! Defines structures for players and teams in the game.

use serde::{Deserialize, Serialize};

/// Represents a player in the game
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Player {
    /// Unique player identifier
    pub id: String,
    /// Player's display name
    pub name: String,
}

/// Represents a team of players
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    /// Unique team identifier
    pub id: String,
    /// Team name
    pub name: String,
    /// Faction affiliation
    pub faction: String,
    /// List of player IDs who are members of this team
    pub members: Vec<String>,
}

impl Player {
    /// Create a new player with the given name and auto-generated ID
    pub fn new(name: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
        }
    }
}

impl Team {
    /// Create a new team
    pub fn new(name: String, faction: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            faction,
            members: Vec::new(),
        }
    }
    
    /// Add a player to the team
    pub fn add_member(&mut self, player_id: String) {
        if !self.members.contains(&player_id) {
            self.members.push(player_id);
        }
    }
    
    /// Remove a player from the team
    pub fn remove_member(&mut self, player_id: &str) {
        self.members.retain(|id| id != player_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_player_creation() {
        let player = Player::new("Alice".to_string());
        assert_eq!(player.name, "Alice");
        assert!(!player.id.is_empty());
    }

    #[test]
    fn test_team_creation() {
        let mut team = Team::new("Red Team".to_string(), "Alliance".to_string());
        assert_eq!(team.name, "Red Team");
        assert_eq!(team.faction, "Alliance");
        assert!(team.members.is_empty());
        
        // Add members
        team.add_member("player1".to_string());
        team.add_member("player2".to_string());
        assert_eq!(team.members.len(), 2);
        
        // Adding same member twice should not duplicate
        team.add_member("player1".to_string());
        assert_eq!(team.members.len(), 2);
        
        // Remove member
        team.remove_member("player1");
        assert_eq!(team.members.len(), 1);
        assert!(team.members.contains(&"player2".to_string()));
    }
}
