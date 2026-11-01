//! The recurrent walk's luminous blue variables (P12.T5.c): plan 06's giant eruptions (P06.T28.e)
//! of the hosts of plan 09's class (P09.T43, [`LbvProcess`]).
//!
//! A host erupts only inside its LBV window, the clock interval from the first to the last time
//! its primary is a luminous blue variable, which its entry carries
//! ([`LbvEntry::window`](crate::galaxy::catalogue_classes::lbv::LbvEntry::window)). The walk asks
//! each host of a cell back by the longest eruption, plan 06's 20 years, and passes over one whose
//! window misses its emission interval, or which stays beyond the giant eruptions' reach, before
//! its star is built. The rest build their primary as the stellar stage does ([`StarRecipe`]), and
//! plan 06 lists its eruptions over the interval on the star's own series, `star.ev.lbv_eruption`
//! of body 0 ([`lbv::eruptions_in`]).
//!
//! An eruption's light is its own, the star's luminosity above its quiescent one at the onset,
//! bolometric ([`LightCurveSource::GiantEruption`]), each band taking a 5,000 K blackbody's share
//! of it ([`GIANT_ERUPTION_SHARES`]).
//!
//! [`lbv::eruptions_in`]: crate::stellar::events::lbv::eruptions_in
//! [`LightCurveSource::GiantEruption`]: super::super::LightCurveSource::GiantEruption

use std::error::Error;
use std::fmt;

use crate::events::{EventSeries, TimeWindow, tags};
use crate::galaxy::Galaxy;
use crate::galaxy::catalogue_classes::lbv::{LbvEntry, LbvProcess};
use crate::galaxy::catalogue_classes::{
    CatalogueCellKey, CatalogueClassCell, CatalogueMotion, ClassProcess,
};
use crate::id::{BodyId, SystemId};
use crate::stellar::events::lbv::{LbvModel, MAX_DURATION};
use crate::time::{Span, UniverseTime};
use crate::units::consts::SPEED_OF_LIGHT;

use super::super::reach::kind_reach;
use super::super::registry::TransientKind;
use super::super::{AlertContext, AlertQuery, Detection};
use super::lit::{LitEvent, LitHost, emission_interval, host_detections};
use super::{StarRecipe, sort_detections};

#[cfg(doc)]
use super::GIANT_ERUPTION_SHARES;

/// One luminous blue variable, as the walk reads it: its system and star, the window in which the
/// star is one, and how the system moves (P12.T5.c).
#[derive(Debug, Clone, PartialEq)]
pub struct EruptionHost {
    host: SystemId,
    window: (UniverseTime, UniverseTime),
    motion: CatalogueMotion,
    star: StarRecipe,
}

/// An [`EruptionHost`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildEruptionHostError {
    /// The window ends before it starts.
    WindowReversed,
    /// The system moves at or above light's speed, or at no speed a number.
    NotSlowerThanLight,
}

impl fmt::Display for BuildEruptionHostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::WindowReversed => "a luminous blue variable's window ends before it starts",
            Self::NotSlowerThanLight => "a luminous blue variable moves no slower than light",
        })
    }
}

impl Error for BuildEruptionHostError {}

impl EruptionHost {
    /// The luminous blue variable of system `host`, whose primary, `star`, is one from
    /// `window.0` to `window.1`, the clock interval its star's model gives
    /// ([`StarModel::lbv_window`](crate::stellar::system::StarModel::lbv_window)), the system
    /// moving by `motion`.
    ///
    /// # Errors
    ///
    /// [`BuildEruptionHostError::WindowReversed`] for a window that ends before it starts, and
    /// [`BuildEruptionHostError::NotSlowerThanLight`] for a motion whose speed before or after its
    /// break is not below light's.
    pub fn new(
        host: SystemId,
        window: (UniverseTime, UniverseTime),
        motion: CatalogueMotion,
        star: StarRecipe,
    ) -> Result<Self, BuildEruptionHostError> {
        if window.1 < window.0 {
            return Err(BuildEruptionHostError::WindowReversed);
        }
        let before = motion
            .at()
            .checked_sub(Span::from_seconds(1))
            .unwrap_or(motion.at());
        let slower = [before, motion.at()]
            .map(|t| motion.velocity_at(t).speed().value())
            .iter()
            .all(|&speed| speed < SPEED_OF_LIGHT);
        if !slower {
            return Err(BuildEruptionHostError::NotSlowerThanLight);
        }
        Ok(Self {
            host,
            window,
            motion,
            star,
        })
    }

    /// The system.
    #[must_use]
    pub const fn host(&self) -> SystemId {
        self.host
    }

    /// The body whose eruptions they are: the system's primary.
    #[must_use]
    pub fn body(&self) -> BodyId {
        BodyId::new(self.host, 0)
    }

    /// The clock interval in which the star is a luminous blue variable.
    #[must_use]
    pub const fn window(&self) -> (UniverseTime, UniverseTime) {
        self.window
    }

    /// How the system moves.
    #[must_use]
    pub const fn motion(&self) -> &CatalogueMotion {
        &self.motion
    }

    /// What builds the star.
    #[must_use]
    pub const fn star(&self) -> &StarRecipe {
        &self.star
    }

    /// Whether an eruption can have its onset in `interval`: whether the window, in which every
    /// eruption begins, meets it.
    #[must_use]
    fn may_erupt_in(&self, interval: TimeWindow) -> bool {
        let (start, end) = self.window;
        start < interval.end() && interval.start() < end
    }
}

/// A class whose hosts are luminous blue variables, as the walk reads it (P12.T5.c).
///
/// Implemented for plan 09's class ([`LbvProcess`]); a test's class implements it the same way.
pub trait EruptionClass: ClassProcess {
    /// The luminous blue variable `entry` is, or `None` for an entry that is not one.
    #[must_use]
    fn eruption_host(&self, galaxy: &Galaxy, entry: &Self::Entry) -> Option<EruptionHost>;
}

impl EruptionClass for LbvProcess {
    /// The entry's system, its primary as the stellar stage builds it, and the window its class's
    /// test read; every entry is one.
    ///
    /// # Panics
    ///
    /// For an entry moving no slower than light or whose window runs backwards, which plan 09's
    /// draws and its class's test never give: a broken invariant of the class.
    fn eruption_host(&self, galaxy: &Galaxy, entry: &LbvEntry) -> Option<EruptionHost> {
        let host = EruptionHost::new(
            entry.record().id(),
            entry.window(),
            *entry.motion(),
            StarRecipe::of_record(galaxy, entry.record(), *entry.composition()),
        )
        .expect("plan 09's LBV motions are below light's speed and their windows run forward");
        Some(host)
    }
}

/// Appends to `out` the detections of the giant eruptions of `process`'s cell `cell` that
/// `query`'s sensor sees in its window, sorted by arrival and then by event (module
/// documentation). Nothing for a cell of another class, or one the query does not ask about.
///
/// # Panics
///
/// As [`CatalogueClassCell::generate`] and the class's process do.
pub fn giant_eruption_scan_cell<P: EruptionClass>(
    galaxy: &Galaxy,
    ctx: &mut AlertContext<'_>,
    query: &AlertQuery,
    process: &P,
    cell: CatalogueCellKey,
    out: &mut Vec<Detection>,
) {
    let class = process.class();
    if cell.class() != class || !query.classes().contains(class) {
        return;
    }
    let observer = *query.observer_position();
    let reach = kind_reach(TransientKind::GiantEruption, query.sensor());
    let start = out.len();
    let (mut listed, mut events) = (Vec::new(), Vec::new());
    for entry in CatalogueClassCell::generate(galaxy, process, cell).entries() {
        let Some(host) = process.eruption_host(galaxy, entry.value()) else {
            continue;
        };
        let Some(interval) = emission_interval(
            host.motion(),
            &observer,
            query.window(),
            MAX_DURATION,
            reach,
        ) else {
            continue;
        };
        if !host.may_erupt_in(interval) {
            continue;
        }
        let star = host.star().to_star();
        let Some(model) = LbvModel::of(&star) else {
            continue;
        };
        let series = EventSeries::new(galaxy.seed(), tags::STAR_LBV_ERUPTION, host.body().into());
        listed.clear();
        model.eruptions_in(&series, interval, &mut listed);
        events.clear();
        events.extend(listed.iter().map(LitEvent::of_eruption));
        let lit = LitHost {
            host: host.host(),
            class,
            motion: *host.motion(),
        };
        host_detections(galaxy, ctx, query, &lit, &events, out);
    }
    sort_detections(&mut out[start..]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Seed;
    use crate::galaxy::catalogue_classes::ClassId;

    /// The luminous blue variables' host is the entry's own: its star is the stellar stage's
    /// primary of the entry's system, whose LBV window is the entry's, and every entry is one. On
    /// the class's cells along +y through the disc of the galaxy of seed 9.
    #[test]
    fn a_luminous_blue_variables_host_is_its_entrys_own() {
        let galaxy = Galaxy::new(Seed::new(9));
        let process = LbvProcess::new(&galaxy);
        let mut hosts = 0;
        for y in 1..8 {
            for z in [-1, 0] {
                let key =
                    CatalogueCellKey::new(ClassId::LUMINOUS_BLUE_VARIABLE, [0, y, z]).unwrap();
                for entry in CatalogueClassCell::generate(&galaxy, &process, key).entries() {
                    let entry = entry.value();
                    let host = process
                        .eruption_host(&galaxy, entry)
                        .expect("every entry is one");
                    let star = host.star().to_star();
                    assert_eq!(&star, entry.stars(&galaxy).primary());
                    assert_eq!(star.lbv_window(), Some(entry.window()));
                    assert_eq!(host.window(), entry.window());
                    assert_eq!(host.body(), BodyId::new(entry.record().id(), 0));
                    assert_eq!(host.motion(), entry.motion());
                    hosts += 1;
                }
            }
        }
        assert!(hosts > 0, "the disc has luminous blue variables");
    }

    /// A host's window runs forward and its motion below light's speed.
    #[test]
    fn an_eruption_host_is_checked() {
        use crate::coords::{GalacticPosition, GalacticVelocity};
        use crate::stellar::Composition;
        use crate::stellar::draws::StarDraws;
        use crate::units::{SolarMasses, Years};

        let site = GalacticPosition::from_light_years([0.0, 26_000.0, 20.0]).unwrap();
        let system = SystemId::from_raw(1).unwrap();
        let recipe = StarRecipe::new(
            SolarMasses::new(60.0),
            Composition::SOLAR,
            StarDraws::for_star(Seed::new(1), BodyId::new(system, 0)),
            Years::new(4e6),
        )
        .unwrap();
        let (early, late) = (UniverseTime::EPOCH, UniverseTime::new(1, 0).unwrap());
        let host = |window, speed: f64| {
            let motion = CatalogueMotion::straight(site, GalacticVelocity::new([speed, 0.0, 0.0]));
            EruptionHost::new(system, window, motion, recipe.clone())
        };
        assert!(host((early, late), 2e5).is_ok());
        assert!(host((early, early), 2e5).is_ok());
        assert_eq!(
            host((late, early), 2e5),
            Err(BuildEruptionHostError::WindowReversed)
        );
        for speed in [SPEED_OF_LIGHT, f64::NAN] {
            assert_eq!(
                host((early, late), speed),
                Err(BuildEruptionHostError::NotSlowerThanLight)
            );
        }
    }
}
