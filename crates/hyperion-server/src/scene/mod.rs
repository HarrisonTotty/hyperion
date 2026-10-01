//! The scene: what a client subscribed to a universe's scene is told of the ship's surroundings
//! (rendering plan R03).
//!
//! Until sessions exist, each open universe has one ship stand-in and one scene clock, held here
//! and set by `scene_ship` (Design note 2). They are shared, because two clients of one scene can
//! agree only on one clock and one observer; the last setting the server accepts stands, and every
//! subscription of the universe sees it through the universe's [`watch`] channel. None of it is
//! saved. The scene's core reads them only through the [`Clock`] and [`Ship`] traits, which a
//! session will implement in their place.

mod clock;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the scene subscription (R03.T8) is the first to read it"
    )
)]
mod core;
#[expect(
    dead_code,
    reason = "the scene subscription (R03.T8) is the first to read it"
)]
mod sensing;
mod ship;

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::sync::{Mutex, PoisonError};

use hyperion_protocol::{FramePositionDto, KinematicsDto};
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::time::UniverseTime;
use tokio::sync::watch;
use tokio::time::Instant;

pub(crate) use self::clock::{ClockReading, SceneClock, TimeRate};
#[expect(
    unused_imports,
    reason = "the scene subscription (R03.T8) is the first to read it"
)]
pub(crate) use self::core::{
    Beat, FetchSystemError, SCENE_FRAME_ENTRY, SceneCore, SceneDelta, SceneInputs, SceneWorld,
    is_large_notification,
};
#[expect(
    unused_imports,
    reason = "the scene subscription (R03.T8) is the first to read it"
)]
pub(crate) use self::sensing::{CraftSource, CraftState, GrantAsked, NoCraft, SceneKnowledge};
pub(crate) use self::ship::{ShipPosition, ShipStandIn};
use crate::universe::UniverseId;

/// A scene's clock, as the scene's core reads it (Design note 2): the stand-in's [`SceneClock`]
/// until sessions, then the session's.
#[expect(
    dead_code,
    reason = "the scene subscription (R03.T8) is the first to read it"
)]
pub(crate) trait Clock {
    /// What the clock reads at `at`.
    fn reading_at(&self, at: Instant) -> ClockReading;

    /// The first instant at which the clock reads `time` or later, or `None` if it never will.
    fn instant_of(&self, time: UniverseTime) -> Option<Instant>;
}

/// A scene's ship, as the scene's core reads it (Design note 2): the [`ShipStandIn`] until
/// sessions, then the session's ship.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the scene subscription (R03.T8) is the first to read it"
    )
)]
pub(crate) trait Ship {
    /// Where the ship is at `t`, in the frame its pose is held in.
    ///
    /// # Panics
    ///
    /// May panic for a `t` outside the clock window, which no scene clock reads.
    fn position_at(&self, t: UniverseTime) -> ShipPosition;

    /// Its velocity relative to that frame's origin, m/s along the galactic axes.
    fn velocity_m_s(&self) -> [f64; 3];

    /// Its pose as the wire carries it.
    fn kinematics(&self) -> KinematicsDto;
}

/// One universe's scene setting: its clock and its ship stand-in, as the last `scene_ship` set
/// them.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SceneSetting {
    /// The scene clock.
    pub(crate) clock: SceneClock,
    /// The ship stand-in.
    pub(crate) ship: ShipStandIn,
}

impl SceneSetting {
    /// The setting of a universe nobody has set yet: the stand-in at rest at the galactic centre
    /// at the epoch, in the galactic frame, with the clock paused there.
    ///
    /// The centre is in no system's frame (plan 03's `frame_at` leaves it to the galactic frame),
    /// so a scene subscribed before any `scene_ship` holds no system and costs nothing.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the scene subscription (R03.T8) is the first to read it"
        )
    )]
    #[must_use]
    fn unset(anchor: Instant) -> Self {
        let position = GalacticPosition::ORIGIN;
        let wire = KinematicsDto {
            position: FramePositionDto::Galactic {
                position: hyperion_protocol::GalacticPosition::default(),
            },
            velocity_m_s: [0.0; 3],
            time: hyperion_protocol::UniverseTime::default(),
        };
        Self {
            clock: SceneClock::new(UniverseTime::EPOCH, anchor, TimeRate::PAUSED)
                .expect("the epoch is inside the clock window"),
            ship: ShipStandIn::new(
                ShipPosition::Galactic(position),
                [0.0; 3],
                UniverseTime::EPOCH,
                wire,
            ),
        }
    }
}

/// Every open universe's scene setting, each behind a [`watch`] channel.
#[derive(Debug, Default)]
pub(crate) struct SceneService {
    universes: Mutex<HashMap<UniverseId, watch::Sender<SceneSetting>>>,
}

impl SceneService {
    /// An empty service: no universe has a setting until it is watched or set.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// A receiver of `universe`'s setting, which sees every later change; the unset setting
    /// ([`SceneSetting::unset`]) if nobody has set one.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the scene subscription (R03.T8) is the first to read it"
        )
    )]
    #[must_use]
    pub(crate) fn watch(&self, universe: UniverseId) -> watch::Receiver<SceneSetting> {
        let mut universes = self
            .universes
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        universes
            .entry(universe)
            .or_insert_with(|| watch::Sender::new(SceneSetting::unset(Instant::now())))
            .subscribe()
    }

    /// Replaces `universe`'s setting with `setting`, which every receiver then sees.
    pub(crate) fn set(&self, universe: UniverseId, setting: SceneSetting) {
        let mut universes = self
            .universes
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        match universes.entry(universe) {
            Entry::Occupied(sender) => {
                sender.get().send_replace(setting);
            }
            Entry::Vacant(vacant) => {
                vacant.insert(watch::Sender::new(setting));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn an_unset_universe_is_paused_at_the_epoch_at_the_galactic_centre() {
        let service = SceneService::new();
        let receiver = service.watch(UniverseId::new(7));
        let setting = receiver.borrow().clone();
        let reading = setting.clock.now();
        assert_eq!(
            (reading.time(), reading.state()),
            (UniverseTime::EPOCH, clock::ClockState::Paused)
        );
        assert_eq!(
            setting.ship.position_at(UniverseTime::EPOCH),
            ShipPosition::Galactic(GalacticPosition::ORIGIN)
        );
    }
}
