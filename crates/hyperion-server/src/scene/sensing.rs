//! The scene's seams for what the ship knows and what flies near it (rendering plan R03, Design
//! notes 4 and 13): the level granted for each body and the craft that are contacts
//! ([`SceneKnowledge`]), and the craft in a universe ([`CraftSource`]).
//!
//! Until the sensors plan, the server grants the level asked for every body and no craft
//! ([`GrantAsked`]), as plan 14's handlers do; until sessions, no craft exist ([`NoCraft`]). Both
//! are traits so that tests can restrict the scene before anything in the game does: the
//! Knowledge-bound test drives it with a fake.

use std::fmt;

use hyperion_protocol::SceneCraftDto;
use hyperion_sim::id::BodyId;
use hyperion_sim::planetary::record::DetailLevel;
use hyperion_sim::time::UniverseTime;

use crate::universe::UniverseId;

/// What the ship knows of its surroundings, as far as the scene asks.
pub(crate) trait SceneKnowledge: fmt::Debug + Send + Sync {
    /// The level granted for `body` when a client asked for `asked`. The scene holds the answer to
    /// `asked`, so a grant above it grants `asked`.
    fn grant(&self, body: BodyId, asked: DetailLevel) -> DetailLevel;

    /// Whether `craft` is one of the ship's contacts, and so in the scene.
    fn is_contact(&self, craft: &CraftState) -> bool;
}

/// The knowledge until the sensors plan: every body at the level asked, and no craft.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub(crate) struct GrantAsked;

impl SceneKnowledge for GrantAsked {
    fn grant(&self, _body: BodyId, asked: DetailLevel) -> DetailLevel {
        asked
    }

    fn is_contact(&self, _craft: &CraftState) -> bool {
        false
    }
}

/// The craft in a universe.
pub(crate) trait CraftSource: fmt::Debug + Send + Sync {
    /// Every craft of `universe` at scene time `t`.
    fn craft_at(&self, universe: UniverseId, t: UniverseTime) -> Vec<CraftState>;
}

/// The craft until sessions: none.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub(crate) struct NoCraft;

impl CraftSource for NoCraft {
    fn craft_at(&self, _universe: UniverseId, _t: UniverseTime) -> Vec<CraftState> {
        Vec::new()
    }
}

/// One craft as a [`CraftSource`] supplies it: for now exactly the draft wire record, which the
/// sessions plan owns and may reshape freely while nothing in the game sends it (Design note 4).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CraftState(SceneCraftDto);

impl CraftState {
    /// The craft `record` describes.
    #[must_use]
    pub(crate) fn new(record: SceneCraftDto) -> Self {
        Self(record)
    }

    /// The craft's ID.
    #[must_use]
    pub(crate) fn id(&self) -> &str {
        &self.0.craft
    }
}

impl From<CraftState> for SceneCraftDto {
    fn from(craft: CraftState) -> Self {
        craft.0
    }
}
