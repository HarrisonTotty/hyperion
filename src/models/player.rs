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
    /// Team's current credit balance
    #[serde(default)]
    pub credits: i64,
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
    /// Create a new team with zero starting credits
    pub fn new(name: String, faction: String) -> Self {
        Self::with_credits(name, faction, 0)
    }

    /// Create a new team with specified starting credits
    pub fn with_credits(name: String, faction: String, starting_credits: i64) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            faction,
            members: Vec::new(),
            credits: starting_credits,
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

    /// Deduct credits from the team
    ///
    /// Returns the new balance if successful, or an error if insufficient credits.
    pub fn deduct_credits(&mut self, amount: i64) -> Result<i64, String> {
        if self.credits < amount {
            return Err(format!(
                "Insufficient credits: have {}, need {}",
                self.credits, amount
            ));
        }
        self.credits -= amount;
        Ok(self.credits)
    }

    /// Add credits to the team (for rewards, refunds, etc.)
    ///
    /// Returns the new balance.
    pub fn add_credits(&mut self, amount: i64) -> i64 {
        self.credits += amount;
        self.credits
    }

    /// Refund credits to the team (100% refund rate)
    ///
    /// Returns the new balance.
    pub fn refund_credits(&mut self, amount: i64) -> i64 {
        self.add_credits(amount)
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
        assert_eq!(team.credits, 0);

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

    #[test]
    fn test_team_with_starting_credits() {
        let team = Team::with_credits("Blue Team".to_string(), "Federation".to_string(), 1_000_000);
        assert_eq!(team.name, "Blue Team");
        assert_eq!(team.faction, "Federation");
        assert_eq!(team.credits, 1_000_000);
    }

    #[test]
    fn test_credit_operations() {
        let mut team = Team::with_credits("Test Team".to_string(), "Test".to_string(), 10_000);

        // Test deduction
        let balance = team.deduct_credits(3_000).unwrap();
        assert_eq!(balance, 7_000);
        assert_eq!(team.credits, 7_000);

        // Test insufficient credits
        let result = team.deduct_credits(10_000);
        assert!(result.is_err());
        assert_eq!(team.credits, 7_000); // Balance unchanged

        // Test adding credits
        let balance = team.add_credits(5_000);
        assert_eq!(balance, 12_000);
        assert_eq!(team.credits, 12_000);

        // Test refund (same as add)
        let balance = team.refund_credits(1_000);
        assert_eq!(balance, 13_000);
        assert_eq!(team.credits, 13_000);
    }
}
