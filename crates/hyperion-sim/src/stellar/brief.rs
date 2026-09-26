//! A range query's brief of a system, routed by what its primary is (plan 06, P06.T38.e; rulings
//! 89 and 90).
//!
//! A range row's [`StellarBrief`] needs the primary's state at the query's time and the system's
//! star count, and nothing else of the system: no companion's model, no orbit, no photometry. The
//! [`BriefModel`] builds only that, by the cheapest route that gives the same brief as
//! [`SystemStars::brief_at`](crate::stellar::system::SystemStars::brief_at) bit for bit:
//!
//! - **The main sequence without knots** ([`BriefRoute::MainSequence`]): a primary whose main
//!   sequence has no knots and lasts beyond the clock window's end is read from its main sequence
//!   alone, the build of [`main_sequence_state`](crate::stellar::sse::main_sequence_state)
//!   (P06.T38.b), which is its track's state bit for bit at every age of the window. About nine
//!   rows in ten near the Sun.
//! - **Every other primary** ([`BriefRoute::Exact`]): its [`StarModel`], as
//!   [`SystemStars::generate`](crate::stellar::system::SystemStars::generate) builds it. Below 0.1 M☉ that is P06.T13's cooling fits, which cost
//!   nothing; for a living evolved star, a star leaving its main sequence within the window, or a
//!   dead star, it is the track. Ruling 89's fate table, which would take the dead stars, is held
//!   (ruling 90), so they cost their full track for now.
//!
//! The star count comes from [`draw_star_count`], plan 11's count-only draw, which equals the
//! hierarchy's count for every system and builds no orbit for a single one. The composition and
//! the primary's draws are the ones
//! [`SystemStars::generate`](crate::stellar::system::SystemStars::generate) reads, from the same
//! streams; a main-sequence primary reads η alone, the one draw its state and class depend on.
//!
//! Every route gives the brief [`SystemStars::brief_at`](crate::stellar::system::SystemStars::brief_at) gives, bit for bit: a brief is
//! [`StellarBrief::of`] the same state, composition, draws and count. A [`BriefModel`] holds epoch
//! state only and is the same whatever was asked before, so a server may cache it by system.

use crate::galaxy::Galaxy;
use crate::galaxy::placement::SystemRecord;
use crate::rng::Mark;
use crate::stellar::Composition;
use crate::stellar::classify::ClassExtras;
use crate::stellar::draws::{StandardNormal, StarDraws, StarDrawsParts, UnitUniform};
use crate::stellar::multiplicity::{MultiplicityContext, draw_star_count};
use crate::stellar::sse::{MAX_INITIAL_MASS, MIN_INITIAL_MASS, Track, TrackOptions};
use crate::stellar::system::{
    GRID_ATTEMPT, StarModel, StellarBrief, draw_metallicity, primary_draws, primary_eta,
    primary_rotation_draws,
};
use crate::time::{ClockWindow, UniverseTime};
use crate::units::Years;

/// Which way a [`BriefModel`] evaluates its primary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BriefRoute {
    /// A main sequence without knots that lasts beyond the clock window: its closed forms alone.
    MainSequence,
    /// The primary's [`StarModel`]: the cooling fits below 0.1 M☉, and otherwise its track.
    Exact,
}

/// What a range query's row needs of one grid system at any clock time: its primary, by the
/// cheapest route that gives [`SystemStars::brief_at`](crate::stellar::system::SystemStars::brief_at)'s brief, and its star count (plan 06,
/// P06.T38.e).
///
/// It holds epoch state and nothing that depends on a time asked about, so it can be cached by
/// system and asked about any time of the clock window.
///
/// # Examples
///
/// The brief of each system of a cell at the solar circle is the full system's:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::placement::{CellKey, generate_cell};
/// use hyperion_sim::id::Layer;
/// use hyperion_sim::stellar::brief::BriefModel;
/// use hyperion_sim::stellar::system::SystemStars;
/// use hyperion_sim::time::UniverseTime;
///
/// let galaxy = Galaxy::new(Seed::new(11));
/// let mut cell = Vec::new();
/// generate_cell(&galaxy, CellKey::new(Layer::C, [0, 812, 0])?, &mut cell);
/// for record in cell.iter().take(20) {
///     let brief = BriefModel::new(&galaxy, record).brief_at(UniverseTime::EPOCH);
///     assert_eq!(brief, SystemStars::generate(&galaxy, record).brief_at(UniverseTime::EPOCH));
/// }
/// # Ok::<(), hyperion_sim::galaxy::placement::BuildCellKeyError>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct BriefModel {
    primary: Primary,
    star_count: u8,
}

/// A [`BriefModel`]'s primary.
#[derive(Debug, Clone, PartialEq)]
enum Primary {
    /// [`BriefRoute::MainSequence`]: the knot-free main sequence's one-segment track.
    MainSequence {
        track: Box<Track>,
        composition: Composition,
        /// The primary's η, the one draw its main sequence reads.
        eta: StandardNormal,
        /// Its rotation rank and fossil-field mark, which its peculiar class reads (P06.T25).
        rotation: (UnitUniform, Mark),
        age_at_epoch: Years,
    },
    /// [`BriefRoute::Exact`].
    Exact(Box<StarModel>),
}

impl BriefModel {
    /// The brief model of the grid system `record` in `galaxy`.
    ///
    /// # Panics
    ///
    /// As [`SystemStars::generate`](crate::stellar::system::SystemStars::generate) does, for a record of another galaxy.
    #[must_use]
    pub fn new(galaxy: &Galaxy, record: &SystemRecord) -> Self {
        let composition = draw_metallicity(galaxy, record);
        let star_count = draw_star_count(galaxy, record, MultiplicityContext::Free, GRID_ATTEMPT);
        let m0 = record.primary_initial_mass();
        let age_at_epoch = record.age_at_epoch();
        // The main sequence reads η alone of the primary's draws: its one stream, not all of
        // them (a quarter of a main-sequence row's instructions, P06.T38.e's measurement).
        let eta = primary_eta(galaxy, record);
        let rotation = primary_rotation_draws(galaxy, record);
        let on_main_sequence = (MIN_INITIAL_MASS..=MAX_INITIAL_MASS)
            .contains(&m0)
            .then(|| {
                Track::knot_free_main_sequence(
                    m0,
                    &composition,
                    &eta_draws(eta, rotation),
                    TrackOptions::default(),
                )
            })
            .flatten()
            // The main sequence must hold every age of the window: the age at its end, as
            // `StarModel` builds its track to.
            .filter(|track| age_at(age_at_epoch, ClockWindow::END) < track.built_until().value());
        let primary = match on_main_sequence {
            Some(track) => Primary::MainSequence {
                track: Box::new(track),
                composition,
                eta,
                rotation,
                age_at_epoch,
            },
            None => Primary::Exact(Box::new(
                StarModel::new(m0, composition, primary_draws(galaxy, record), age_at_epoch)
                    .expect("a grid record's primary is of 0.08-150 M_sun with a finite age"),
            )),
        };
        Self {
            primary,
            star_count,
        }
    }

    /// Which way the primary is evaluated.
    #[must_use]
    pub const fn route(&self) -> BriefRoute {
        match self.primary {
            Primary::MainSequence { .. } => BriefRoute::MainSequence,
            Primary::Exact(_) => BriefRoute::Exact,
        }
    }

    /// How many stars the system has, the primary included.
    #[must_use]
    pub const fn star_count(&self) -> u8 {
        self.star_count
    }

    /// The system's brief at `t`, or `None` before its primary forms: [`SystemStars::brief_at`](crate::stellar::system::SystemStars::brief_at)'s,
    /// bit for bit.
    ///
    /// # Panics
    ///
    /// In debug builds, for a `t` after the clock window's end, as [`StarModel::state_at`].
    #[must_use]
    pub fn brief_at(&self, t: UniverseTime) -> Option<StellarBrief> {
        match &self.primary {
            Primary::MainSequence {
                track,
                composition,
                eta,
                rotation,
                age_at_epoch,
            } => {
                let age = age_at(*age_at_epoch, t);
                (age > 0.0).then(|| {
                    let state = track.state_at(Years::new(age));
                    StellarBrief::of(
                        &state,
                        composition,
                        &eta_draws(*eta, *rotation),
                        ClassExtras::NONE,
                        self.star_count,
                    )
                })
            }
            Primary::Exact(model) => model.state_at(t).map(|state| {
                StellarBrief::of(
                    &state,
                    model.composition(),
                    model.draws(),
                    model.class_extras_at(&state, t),
                    self.star_count,
                )
            }),
        }
    }

    /// The bytes the model owns on the heap, beyond `size_of::<BriefModel>()`: what a server's
    /// byte-bounded cache charges it. A main-sequence model owns a few kilobytes; an exact one its
    /// [`StarModel`]'s track, tens of kilobytes for a dead star.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        match &self.primary {
            Primary::MainSequence { track, .. } => size_of::<Track>() + track.heap_bytes(),
            Primary::Exact(model) => size_of::<StarModel>() + model.heap_bytes(),
        }
    }
}

/// The brief of the grid system `record` in `galaxy` at `t`, or `None` before it forms:
/// [`BriefModel::new`] then [`BriefModel::brief_at`], for a caller that keeps no model.
///
/// # Panics
///
/// As [`BriefModel::new`] and [`BriefModel::brief_at`].
#[must_use]
pub fn range_brief(
    galaxy: &Galaxy,
    record: &SystemRecord,
    t: UniverseTime,
) -> Option<StellarBrief> {
    BriefModel::new(galaxy, record).brief_at(t)
}

/// Draws of η `eta`, the rotation rank and fossil mark `rotation`, and every other draw at its
/// median: what a main-sequence primary's track and class read of the primary's own draws. A main
/// sequence's builder reads η (and holds the remnant draws, which it reads only at a death);
/// [`classify`] reads the rotation and magnetism draws for a main-sequence star's peculiar class
/// (P06.T25) and the white dwarf marks for a white dwarf. So on the main sequence these give the
/// state and brief of [`StarDraws::for_star`]'s draws bit for bit, which the tests check route by
/// route.
///
/// [`classify`]: crate::stellar::classify::classify
#[must_use]
fn eta_draws(eta: StandardNormal, (rotation, magnetism): (UnitUniform, Mark)) -> StarDraws {
    StarDraws::from_parts(StarDrawsParts {
        eta,
        rotation,
        magnetism,
        ..StarDrawsParts::MEDIAN
    })
}

/// A primary's age at `t`, Julian years, from its age at the epoch: [`StarModel::age_at`]'s
/// arithmetic.
#[must_use]
fn age_at(age_at_epoch: Years, t: UniverseTime) -> f64 {
    age_at_epoch.value() + t.since_epoch().as_julian_years_f64()
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::bits;

    use super::*;
    use crate::Seed;
    use crate::galaxy::placement::{CellKey, generate_cell};
    use crate::id::Layer;
    use crate::stellar::system::SystemStars;

    fn records(galaxy: &Galaxy) -> Vec<SystemRecord> {
        let mut all = Vec::new();
        let mut cell = Vec::new();
        for layer in [Layer::A, Layer::B, Layer::C, Layer::D, Layer::E] {
            let size = i32::try_from(layer.cell_size_ly()).expect("small");
            for step in 0..3 {
                let key = CellKey::new(layer, [step, 26_000 / size, 0]).expect("a cell");
                generate_cell(galaxy, key, &mut cell);
                all.extend(cell.drain(..).take(12));
            }
        }
        all
    }

    fn years(y: i64) -> UniverseTime {
        UniverseTime::from_julian_years(y).expect("inside the clock")
    }

    /// Every route gives the full system's brief, bit for bit, at times across the window, and
    /// both routes are taken.
    #[test]
    fn every_route_is_the_full_systems_brief() {
        let galaxy = Galaxy::new(Seed::new(0x6272_6965));
        let mut routes = [0_u32; 2];
        for record in records(&galaxy) {
            let model = BriefModel::new(&galaxy, &record);
            let stars = SystemStars::generate(&galaxy, &record);
            assert_eq!(model.star_count(), stars.star_count(), "{record:?}");
            routes[usize::from(model.route() == BriefRoute::Exact)] += 1;
            for y in [-200_000, -1_000, -500, 0, 500, 1_000] {
                let (a, b) = (model.brief_at(years(y)), stars.brief_at(years(y)));
                assert_eq!(a, b, "{record:?} at {y} yr");
                if let (Some(a), Some(b)) = (a, b) {
                    assert_eq!(
                        bits(a.effective_temperature().value()),
                        bits(b.effective_temperature().value())
                    );
                    assert_eq!(
                        a.log_luminosity().map(|l| bits(l.value())),
                        b.log_luminosity().map(|l| bits(l.value()))
                    );
                }
            }
        }
        assert!(routes.iter().all(|&n| n > 0), "{routes:?}");
    }

    /// A system not yet born has no brief, on either route, and has one once it is.
    #[test]
    fn a_system_not_yet_born_has_no_brief_on_either_route() {
        let galaxy = Galaxy::new(Seed::new(0x6272_6967));
        let template = records(&galaxy)[0];
        let mut routes = Vec::new();
        for mass in [0.5, 0.09, 12.0] {
            // Born 400 years after the epoch.
            let record = SystemRecord::from_parts(
                template.id(),
                *template.epoch_position(),
                template.origin(),
                template.population(),
                crate::units::SolarMasses::new(mass),
                Years::new(-400.0),
            );
            let model = BriefModel::new(&galaxy, &record);
            let stars = SystemStars::generate(&galaxy, &record);
            routes.push(model.route());
            for y in [-1_000, 0, 399] {
                assert_eq!(model.brief_at(years(y)), None, "{mass} M☉ at {y} yr");
                assert_eq!(stars.brief_at(years(y)), None);
            }
            let born = model.brief_at(years(500));
            assert!(born.is_some(), "{mass} M☉ once born");
            assert_eq!(born, stars.brief_at(years(500)));
        }
        assert_eq!(
            routes,
            [
                BriefRoute::MainSequence,
                BriefRoute::Exact,
                BriefRoute::Exact
            ]
        );
    }

    /// A model is the same built twice, and answers the same whatever was asked before.
    #[test]
    fn a_model_does_not_depend_on_what_was_asked_before() {
        let galaxy = Galaxy::new(Seed::new(0x6272_6966));
        for record in records(&galaxy).iter().take(40) {
            let model = BriefModel::new(&galaxy, record);
            assert_eq!(model, BriefModel::new(&galaxy, record));
            let times = [years(700), years(-300), years(0)];
            let forward: Vec<_> = times.iter().map(|&t| model.brief_at(t)).collect();
            let backward: Vec<_> = times.iter().rev().map(|&t| model.brief_at(t)).collect();
            assert_eq!(forward, backward.into_iter().rev().collect::<Vec<_>>());
            assert_eq!(
                range_brief(&galaxy, record, years(0)),
                model.brief_at(years(0))
            );
        }
    }
}
