//! A system as its light shows it: a range query's hit read at the retarded time (plan 12,
//! P12.T2; Design note 4).

use super::error::{CurvatureError, curvature_error};
use super::retarded::{Drift, Observer, Retardation, TraceMotionError, retarded};
use crate::galaxy::Galaxy;
use crate::galaxy::placement::SystemRecord;
use crate::galaxy::query::SystemHit;
use crate::stellar::system::{StellarBrief, SystemExistence, SystemStars, SystemSummary};

/// A system found by a spatial search on its present position, and what an observer's sensors
/// receive from it: its state at the retarded time (plan 12's Provides).
///
/// The hit is the search's and is unchanged: observed mode changes what is reported, never what
/// is found (Design note 4). A system whose age at the emitted time is not yet positive is
/// [`SystemExistence::NotYetBorn`] then and has no brief, though it is found, because the chart is
/// the navigation computer's view of the present.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObservedSystem {
    hit: SystemHit,
    retardation: Retardation,
    existence_then: SystemExistence,
    brief_then: Option<StellarBrief>,
    error: CurvatureError,
}

impl ObservedSystem {
    /// The search's hit: the system and its present position.
    #[must_use]
    pub const fn hit(&self) -> &SystemHit {
        &self.hit
    }

    /// When and where the light left it.
    #[must_use]
    pub const fn retardation(&self) -> &Retardation {
        &self.retardation
    }

    /// Whether the system existed when the light left it.
    #[must_use]
    pub const fn existence_then(&self) -> SystemExistence {
        self.existence_then
    }

    /// Its brief when the light left it, or `None` if it was not yet born.
    #[must_use]
    pub const fn brief_then(&self) -> Option<StellarBrief> {
        self.brief_then
    }

    /// The stated error of its apparent position from neglected curvature.
    #[must_use]
    pub const fn error(&self) -> CurvatureError {
        self.error
    }
}

/// What `observer` sees of the system `hit`, whose stars are `stars`: its retardation, its
/// existence and brief at the emitted time, and the stated error (plan 12's Provides).
///
/// `observer`'s time must be the time the search was made at. The present is taken from the
/// system's own line, not from the hit, whose position plan 03's drift rounds differently by up to
/// a few metres, so that this and [`retarded`] give the same bits for the same system. The system
/// moves on its own line ([`Drift::of_record`]):
/// a grid system at plan 08's velocity, a feature member at its own, which is resolved again. A
/// member of the galactic centre is refused until plan 09's P09.T28 builds its Kepler regime, so
/// that its observed elements will be the orbit's.
///
/// # Errors
///
/// [`TraceMotionError::CentreOrbitNotBuilt`] for a member of the galactic centre.
///
/// # Panics
///
/// - If the hit is a feature member that does not resolve in `galaxy`, which only a record of
///   another galaxy can be ([`Drift::of_record`]).
/// - In debug builds, if `stars` are not `hit`'s system's.
///
/// # Examples
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::coords::GalacticPosition;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::placement::NoCache;
/// use hyperion_sim::galaxy::query::{RangeQuery, range_query};
/// use hyperion_sim::observe::{Observer, observe_hit};
/// use hyperion_sim::stellar::system::SystemStars;
/// use hyperion_sim::time::UniverseTime;
/// use hyperion_sim::units::LightYears;
///
/// let galaxy = Galaxy::new(Seed::new(12));
/// let sun = GalacticPosition::from_light_years([0.0, 26_000.0, 0.0]).ok_or("in range")?;
/// let query = RangeQuery::builder(sun, LightYears::new(20.0)).build()?;
/// let found = range_query(&galaxy, &mut NoCache::new(), &[], &query);
/// let observer = Observer::new(sun, UniverseTime::EPOCH)?;
/// let hit = found.systems().first().ok_or("twenty light-years hold a system")?;
/// let seen = observe_hit(&galaxy, hit, &SystemStars::generate(&galaxy, hit.record()), &observer)?;
/// // A neighbour a few light-years away is seen as it was a few years ago.
/// let age = seen.retardation().light_age().as_julian_years_f64();
/// assert!((age - hit.distance().value()).abs() < 1e-6);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn observe_hit(
    galaxy: &Galaxy,
    hit: &SystemHit,
    stars: &SystemStars,
    observer: &Observer,
) -> Result<ObservedSystem, TraceMotionError> {
    let line = Drift::of_record(galaxy, hit.record())?;
    debug_assert_eq!(
        stars.record().id(),
        hit.id(),
        "the stars given are not the hit's system's"
    );
    let retardation = retarded(observer, &line);
    let emitted = retardation.emitted();
    Ok(ObservedSystem {
        hit: *hit,
        retardation,
        existence_then: SystemExistence::from(hit.record().existence_at(emitted)),
        brief_then: stars.brief_at(emitted),
        error: curvature_error(galaxy, observer.position(), &retardation),
    })
}

/// The summary of `record`'s system, whose stars are `stars`, as `observer` sees it: at the
/// retarded time, with the retardation (plan 12's Provides).
///
/// # Errors
///
/// [`TraceMotionError::CentreOrbitNotBuilt`] for a member of the galactic centre, as
/// [`observe_hit`].
///
/// # Panics
///
/// - If `record` is a feature member that does not resolve in `galaxy` ([`Drift::of_record`]).
/// - In debug builds, if `stars` are not `record`'s system's.
pub fn summary_observed(
    galaxy: &Galaxy,
    record: &SystemRecord,
    stars: &SystemStars,
    observer: &Observer,
) -> Result<(SystemSummary, Retardation), TraceMotionError> {
    let line = Drift::of_record(galaxy, record)?;
    debug_assert_eq!(
        stars.record().id(),
        record.id(),
        "the stars given are not the record's system's"
    );
    let retardation = retarded(observer, &line);
    Ok((stars.summary_at(retardation.emitted()), retardation))
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use super::*;
    use crate::Seed;
    use crate::coords::{GalacticDisplacement, GalacticPosition};
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::placement::{CellKey, SystemRecord, generate_cell};
    use crate::galaxy::query::position_at;
    use crate::id::Layer;
    use crate::observe::{Trajectory, extrapolate_to_present};
    use crate::stellar::ObjectKind;
    use crate::stellar::remnant::DeathKind;
    use crate::stellar::system::ClockDeath;
    use crate::time::{Span, UniverseTime};
    use crate::units::consts::METRES_PER_LIGHT_YEAR;
    use crate::units::{LightYears, Years};

    fn galaxy() -> &'static Galaxy {
        static GALAXY: OnceLock<Galaxy> = OnceLock::new();
        GALAXY.get_or_init(|| {
            Galaxy::from_params(Seed::new(0x1202_0000), GalaxyParams::milky_way_like())
                .expect("the Milky Way fixture's gas is mostly neutral")
                .with_full_potential()
        })
    }

    fn years(y: i64) -> UniverseTime {
        UniverseTime::from_julian_years(y).unwrap()
    }

    fn up(from: &GalacticPosition, ly: f64) -> GalacticPosition {
        from.translated(GalacticDisplacement::new([
            0.0,
            0.0,
            ly * METRES_PER_LIGHT_YEAR,
        ]))
        .unwrap()
    }

    fn hit_of(record: &SystemRecord, observer: &Observer) -> SystemHit {
        let position = position_at(galaxy(), record, observer.time());
        let distance = LightYears::from(observer.position().distance_to(&position));
        SystemHit::new(*record, position, distance)
    }

    /// A layer-E primary near the solar circle that dies as a core collapse, a supergiant a
    /// century before and a neutron star or black hole a century after, re-aged so that it dies
    /// 20,000 years before the epoch: its record, stars and death time.
    ///
    /// A supernova falls in a given 40,000 years in about one layer-E cell in 3,000 at the solar
    /// circle, so rather than search for one the test takes a real record and changes only its age
    /// at the epoch, which moves its death on the clock and nothing else.
    fn dying_supergiant() -> (SystemRecord, SystemStars, UniverseTime) {
        let galaxy = galaxy();
        let mut cell = Vec::new();
        let target = years(-20_000);
        for x in -40..40 {
            generate_cell(
                galaxy,
                CellKey::new(Layer::E, [x, 203, 0]).unwrap(),
                &mut cell,
            );
            for grown in &cell {
                let ClockDeath::At(death, DeathKind::CoreCollapse { .. }) =
                    SystemStars::generate(galaxy, grown).death_time()
                else {
                    continue;
                };
                // Lifetime = T + age at the epoch; the new age puts T at the target.
                let lifetime =
                    death.since_epoch().as_julian_years_f64() + grown.age_at_epoch().value();
                let record = SystemRecord::from_parts(
                    grown.id(),
                    *grown.epoch_position(),
                    grown.origin(),
                    grown.population(),
                    grown.primary_initial_mass(),
                    Years::new(lifetime + 20_000.0),
                );
                let stars = SystemStars::generate(galaxy, &record);
                let ClockDeath::At(death, _) = stars.death_time() else {
                    continue;
                };
                let off = death.checked_since(target).unwrap().as_julian_years_f64();
                assert!(off.abs() < 1.0, "re-aged death {off} yr from the target");
                let kind = |t| stars.brief_at(t).map(|b| b.kind());
                let century = Span::from_julian_years(100).unwrap();
                let before = kind(death.checked_sub(century).unwrap());
                let after = kind(death.checked_add(century).unwrap());
                if before == Some(ObjectKind::Supergiant)
                    && matches!(after, Some(ObjectKind::NeutronStar | ObjectKind::BlackHole))
                {
                    return (record, stars, death);
                }
            }
        }
        panic!("no layer-E core collapse at the solar circle is a supergiant before it dies");
    }

    /// P12.T2: a layer-E system that dies at T, seen from d, is a living supergiant for
    /// t < T + d ÷ c and a remnant of age t − d ÷ c − T after it, with one ID throughout.
    #[test]
    fn observed_system_is_a_supergiant_until_its_death_arrives() {
        let galaxy = galaxy();
        let (record, stars, death) = dying_supergiant();
        // An observer d = |T| light-years north of where it died, so that the news arrives at the
        // epoch: T + d ÷ c = 0.
        let line = Drift::of_record(galaxy, &record).unwrap();
        let d = -death.since_epoch().as_julian_years_f64();
        let site = line.position_at(death);
        let place = up(&site, d);
        for (t, alive) in [(-50, true), (-5, true), (5, false), (50, false)] {
            let observer = Observer::new(place, years(t)).unwrap();
            let seen = observe_hit(galaxy, &hit_of(&record, &observer), &stars, &observer).unwrap();
            assert_eq!(seen.hit().id(), record.id(), "one ID throughout");
            assert_eq!(seen.existence_then(), SystemExistence::Exists);
            let emitted = seen.retardation().emitted();
            let kind = seen.brief_then().map(|b| b.kind());
            // The age since T at the emitted time is t − d ÷ c − T, with the light age as d ÷ c.
            let since = emitted.checked_since(death).unwrap().as_julian_years_f64();
            let expected = years(t)
                .checked_sub(seen.retardation().light_age())
                .unwrap()
                .checked_since(death)
                .unwrap()
                .as_julian_years_f64();
            assert!((since - expected).abs() < 1e-9);
            // The light left within a fraction of a year of t years after T.
            assert!(
                (since - f64::from(i32::try_from(t).unwrap())).abs() < 0.5,
                "{since}"
            );
            if alive {
                assert!(emitted < death);
                assert_eq!(kind, Some(ObjectKind::Supergiant), "at {t} yr");
            } else {
                assert!(emitted > death);
                assert!(
                    matches!(kind, Some(ObjectKind::NeutronStar | ObjectKind::BlackHole)),
                    "at {t} yr: {kind:?}"
                );
            }
            // The chart's present is the remnant's, whatever the light shows.
            assert!(
                stars
                    .brief_at(years(t))
                    .is_some_and(|b| b.kind() != ObjectKind::Supergiant)
            );
        }
        // The summary at the same instant agrees with the brief.
        let observer = Observer::new(place, years(-50)).unwrap();
        let (summary, r) = summary_observed(galaxy, &record, &stars, &observer).unwrap();
        assert_eq!(summary.time(), r.emitted());
        assert_eq!(summary.stars()[0].kind(), ObjectKind::Supergiant);
    }

    /// P12.T2: a system born 100 years ago is not yet born seen from 5,000 ly, and is found all the
    /// same; seen from 50 ly it exists.
    #[test]
    fn observed_system_born_a_century_ago_is_not_yet_born_from_afar() {
        let galaxy = galaxy();
        let mut cell = Vec::new();
        generate_cell(
            galaxy,
            CellKey::new(Layer::B, [0, 1_625, 0]).unwrap(),
            &mut cell,
        );
        let grown = cell
            .first()
            .expect("a layer-B cell at the solar circle holds systems");
        let young = SystemRecord::from_parts(
            grown.id(),
            *grown.epoch_position(),
            grown.origin(),
            grown.population(),
            grown.primary_initial_mass(),
            Years::new(100.0),
        );
        let stars = SystemStars::generate(galaxy, &young);
        let far = Observer::new(up(young.epoch_position(), 5_000.0), UniverseTime::EPOCH).unwrap();
        let seen = observe_hit(galaxy, &hit_of(&young, &far), &stars, &far).unwrap();
        assert_eq!(seen.existence_then(), SystemExistence::NotYetBorn);
        assert_eq!(seen.brief_then(), None);
        assert_eq!(seen.hit().id(), young.id());
        let near = Observer::new(up(young.epoch_position(), 50.0), UniverseTime::EPOCH).unwrap();
        let seen = observe_hit(galaxy, &hit_of(&young, &near), &stars, &near).unwrap();
        assert_eq!(seen.existence_then(), SystemExistence::Exists);
        assert!(seen.brief_then().is_some());
        let (summary, _) = summary_observed(galaxy, &young, &stars, &far).unwrap();
        assert_eq!(summary.existence(), SystemExistence::NotYetBorn);
        assert!(summary.stars().is_empty());
    }

    /// P12.T2: an extrapolated jump lands on the star. Observe from `x_o`, extrapolate, place a
    /// second observer there at the same time, and the star is under a metre away.
    #[test]
    fn observed_extrapolated_jump_lands_on_the_star() {
        let galaxy = galaxy();
        let mut cell = Vec::new();
        generate_cell(
            galaxy,
            CellKey::new(Layer::C, [3, 812, -1]).unwrap(),
            &mut cell,
        );
        assert!(
            cell.len() >= 3,
            "a layer-C cell at the solar circle holds a few systems"
        );
        let mut worst = 0.0_f64;
        for record in cell.iter().take(8) {
            let stars = SystemStars::generate(galaxy, record);
            for (distance, t) in [(3.0, 0), (400.0, -700), (5_000.0, 300), (40_000.0, 1_000)] {
                let observer =
                    Observer::new(up(record.epoch_position(), distance), years(t)).unwrap();
                let hit = hit_of(record, &observer);
                let seen = observe_hit(galaxy, &hit, &stars, &observer).unwrap();
                let landed = extrapolate_to_present(seen.retardation(), observer.time());
                let second = Observer::new(landed, observer.time()).unwrap();
                // The star is on its line; plan 03's drift rounds its product once more, which
                // parts it from the line by a few metres at most inside the clock window.
                let star = Drift::of_record(galaxy, record)
                    .unwrap()
                    .position_at(observer.time());
                let miss = second.position().distance_to(&star).value();
                worst = worst.max(miss);
                let charted = position_at(galaxy, record, observer.time());
                assert!(charted.distance_to(&star).value() < 5.0);
                // The light shows the star where it was: a drift of 0.1-1 ly over 40,000 years.
                if distance > 1_000.0 {
                    assert!(
                        seen.retardation()
                            .apparent_position()
                            .distance_to(&star)
                            .value()
                            > 1e9
                    );
                }
                assert!(seen.error().length().value() < 1.0);
            }
        }
        assert!(worst < 1.0, "a jump missed the star by {worst} m");
    }

    /// A member of the galactic centre is refused with a typed error until plan 09's P09.T28
    /// builds its Kepler regime: the black hole, member zero, which resolves without the centre's
    /// model.
    #[test]
    fn observed_centre_member_is_refused_until_its_orbit_is_built() {
        use crate::galaxy::placement::resolve;
        use crate::id::{CentreMemberId, SystemId};
        let galaxy = galaxy();
        let id = SystemId::from(CentreMemberId::CENTRAL_BLACK_HOLE);
        let black_hole = resolve(galaxy, id).expect("the black hole resolves");
        assert_eq!(
            Drift::of_record(galaxy, &black_hole),
            Err(TraceMotionError::CentreOrbitNotBuilt(id))
        );
        // Any stars will do: the refusal comes before they are read.
        let mut cell = Vec::new();
        generate_cell(
            galaxy,
            CellKey::new(Layer::B, [0, 1_625, 0]).unwrap(),
            &mut cell,
        );
        let stars = SystemStars::generate(galaxy, &cell[0]);
        let observer =
            Observer::new(up(&GalacticPosition::ORIGIN, 5_000.0), UniverseTime::EPOCH).unwrap();
        let hit = SystemHit::new(
            black_hole,
            GalacticPosition::ORIGIN,
            LightYears::new(5_000.0),
        );
        assert_eq!(
            observe_hit(galaxy, &hit, &stars, &observer),
            Err(TraceMotionError::CentreOrbitNotBuilt(id))
        );
        assert_eq!(
            summary_observed(galaxy, &black_hole, &stars, &observer),
            Err(TraceMotionError::CentreOrbitNotBuilt(id))
        );
        let message = TraceMotionError::CentreOrbitNotBuilt(id).to_string();
        assert!(!message.chars().next().unwrap().is_uppercase() && !message.ends_with('.'));
    }
}
