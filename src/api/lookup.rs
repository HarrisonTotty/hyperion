//! Shared lookup helpers that convert a missing entity into `Status::NotFound`.
//!
//! Handlers across [`crate::api`] repeatedly pair `get_<entity>(id)` with
//! `.ok_or(Status::NotFound)?`. Centralising the conversion here keeps that
//! behaviour in one place so future changes (for example, switching from a
//! bare status to a structured error body) only need to touch a single file.

use rocket::http::Status;

use crate::models::{Player, Ship, ShipBlueprint, Team};
use crate::state::GameWorld;
use crate::stations::Station;

/// Turn `GameWorld` entity lookups into `Result<_, Status::NotFound>`.
pub trait WorldLookup {
    fn find_blueprint(&self, id: &str) -> Result<&ShipBlueprint, Status>;
    fn find_blueprint_mut(&mut self, id: &str) -> Result<&mut ShipBlueprint, Status>;
    fn find_ship(&self, id: &str) -> Result<&Ship, Status>;
    fn find_ship_mut(&mut self, id: &str) -> Result<&mut Ship, Status>;
    fn find_player(&self, id: &str) -> Result<&Player, Status>;
    fn find_team(&self, id: &str) -> Result<&Team, Status>;
    fn find_station(&self, id: &str) -> Result<&Station, Status>;
}

impl WorldLookup for GameWorld {
    fn find_blueprint(&self, id: &str) -> Result<&ShipBlueprint, Status> {
        self.get_blueprint(id).ok_or(Status::NotFound)
    }

    fn find_blueprint_mut(&mut self, id: &str) -> Result<&mut ShipBlueprint, Status> {
        self.get_blueprint_mut(id).ok_or(Status::NotFound)
    }

    fn find_ship(&self, id: &str) -> Result<&Ship, Status> {
        self.get_ship(id).ok_or(Status::NotFound)
    }

    fn find_ship_mut(&mut self, id: &str) -> Result<&mut Ship, Status> {
        self.get_ship_mut(id).ok_or(Status::NotFound)
    }

    fn find_player(&self, id: &str) -> Result<&Player, Status> {
        self.get_player(id).ok_or(Status::NotFound)
    }

    fn find_team(&self, id: &str) -> Result<&Team, Status> {
        self.get_team(id).ok_or(Status::NotFound)
    }

    fn find_station(&self, id: &str) -> Result<&Station, Status> {
        self.get_station(id).ok_or(Status::NotFound)
    }
}
