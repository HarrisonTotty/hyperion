//! A cluster's members: conditional draws of a member of a class, its velocity and its sphere of
//! influence (plan 09, P09.T10; Design note 20).
//!
//! A member is a plan 03 [`SystemRecord`] with extras ([`MemberRecord`]): its origin is
//! `FeatureMember` and its population the budget it is drawn from; it carries no density component,
//! so no consumer can read a component's laws for it by mistake. Its stellar stage reads the
//! cluster's composition through [`SystemStars::generate_with`], never plan 06's
//! `draw_metallicity`.
//!
//! # The draw ([`draw_member`])
//!
//! - **Initial mass** on `member.mass`: attempt `k` reads words `6k` and `6k + 1`, a mass from the
//!   galaxy's mass function in the class's range and a uniform. A living star is kept with the
//!   depletion factor's odds (P09.T9.a), a tail's with the odds of having been lost, `1 − d(m)`
//!   where the cluster is depleted.
//! - **Remnant classes** redraw, attempt after attempt, until the star's remnant is of the class's
//!   kind and its kick is below the cluster's effective escape speed, a low-mode neutron star
//!   judged instead on its pair's systemic speed, a Maxwellian of σ = 12 km/s
//!   ([`PAIR_SYSTEMIC_SIGMA`], ruling 126.3) drawn on words 2–5 of the attempt's six. Attempt
//!   `k`'s star reads its draws at
//!   [`StarDraws::for_attempt`] `k`, and the record's origin keeps `k` so that the star generated
//!   later is the same. The loop is bounded at [`MAX_ATTEMPTS`] with a debug assertion; its
//!   expected length is 1 ÷ retention.
//! - **Age and metallicity** are the cluster's, a nursery's members younger by up to its age
//!   spread (`member.abundance`'s word 2); a second-population member draws its enrichment
//!   (word 0) and a spread cluster's member its iron offset (words 3–4).
//! - **Velocity**: the cluster's bulk motion plus an isotropic normal of `σ(r) g(m) ÷ g(m_TO)`,
//!   Bianchini et al. 2016's partial equipartition with `m_eq` = 1.5 M☉ (ruling 126.7), on
//!   `member.velocity`, cut at the local escape speed of the cluster's Plummer sphere as plan 08
//!   cuts a field star's (16 attempts, then 0.99 of the cut); a tail member's is at the tidal
//!   radius's σ, uncut.

use crate::coords::{GalacticPosition, GalacticVelocity};
use crate::galaxy::placement::{SystemOrigin, SystemRecord};
use crate::galaxy::{Galaxy, PointLy};
use crate::id::{BodyId, FeatureMemberId, SystemId};
use crate::math;
use crate::rng::{ObjectKey, Stream, Threshold, tags};
use crate::stellar::Composition;
use crate::stellar::draws::StarDraws;
use crate::stellar::multiplicity::MultiplicityContext;
use crate::stellar::remnant::{KickMode, RemnantKind};
use crate::stellar::system::{StarModel, SystemStars};
use crate::units::consts::METRES_PER_LIGHT_YEAR;
use crate::units::{Dex, KilometresPerSecond, LightYears, Metres, SolarMasses, Years};

use super::catalogue::FeatureRecord;
use super::cluster::ClusterModel;
use super::ids::PackedFeature;
use super::interior::abundances::{IRON_SPREAD_SHARE, IRON_SPREAD_SIGMA};
use super::interior::counts::{depleted_slope, depletion};
use super::interior::retention::PAIR_SYSTEMIC_SIGMA;
use super::interior::{ClassKind, Generation, MemberAbundances, MemberClass, Multiplicity};

pub mod level_list;
pub mod placement;
pub mod source;

pub use level_list::{FeatureLevelList, ListClass, ListEntry, MemberZero};
pub use placement::{FeatureInterior, resolve_member};
pub use source::{FeatureInteriorCache, FeatureMemberSource, KeepInteriors, NoInteriorCache};

/// The most attempts a member's conditional draw makes (P09.T10).
pub const MAX_ATTEMPTS: u32 = 4_096;

/// The attempts of the velocity's escape cut, as plan 08's.
const VELOCITY_ATTEMPTS: u32 = 16;

/// A member: its record and what a member carries beyond one (Design note 20).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MemberRecord {
    record: SystemRecord,
    class: MemberClass,
    composition: Composition,
    abundances: MemberAbundances,
    velocity: GalacticVelocity,
}

impl MemberRecord {
    /// Its plan 03 record, of origin `FeatureMember`.
    #[must_use]
    pub const fn record(&self) -> &SystemRecord {
        &self.record
    }

    /// Its class.
    #[must_use]
    pub const fn class(&self) -> &MemberClass {
        &self.class
    }

    /// Its composition: the cluster's [Fe/H], its iron offset and its helium excess.
    #[must_use]
    pub const fn composition(&self) -> &Composition {
        &self.composition
    }

    /// Its light-element offsets.
    #[must_use]
    pub const fn abundances(&self) -> &MemberAbundances {
        &self.abundances
    }

    /// Its velocity at the epoch, galactic.
    #[must_use]
    pub const fn velocity(&self) -> GalacticVelocity {
        self.velocity
    }

    /// The attempt its primary's draws are read at.
    #[must_use]
    pub fn attempt(&self) -> u32 {
        match self.record.origin() {
            SystemOrigin::FeatureMember { attempt, .. }
            | SystemOrigin::CentreMember { attempt } => u32::from(attempt),
            SystemOrigin::Grid(_) => 0,
        }
    }

    /// Its stars: [`SystemStars::generate_with`] at its composition, forced multiple for a binary
    /// class and single otherwise.
    #[must_use]
    pub fn stars(&self, galaxy: &Galaxy) -> SystemStars {
        let ctx = match self.class.multiplicity {
            Multiplicity::Single => MultiplicityContext::ForcedSingle,
            Multiplicity::Binary => MultiplicityContext::ForcedMultiple {
                max_separation: None,
            },
        };
        SystemStars::generate_with(galaxy, &self.record, &self.composition, ctx)
    }
}

/// The member of `class` at `position` with the ID `id` of `feature`, whose model is `model` and
/// bulk velocity `bulk`, or `None` if its conditional draw exhausts [`MAX_ATTEMPTS`] (module
/// documentation).
///
/// # Panics
///
/// In debug builds, if the draw exhausts its attempts.
#[must_use]
pub fn draw_member(
    galaxy: &Galaxy,
    feature: &FeatureRecord,
    model: &ClusterModel,
    class: &MemberClass,
    position: &GalacticPosition,
    id: FeatureMemberId,
    bulk: GalacticVelocity,
) -> Option<MemberRecord> {
    let system = SystemId::from(id);
    let key = ObjectKey::from(system);
    let seed = galaxy.seed();
    // Age, metallicity and the second population's enrichment.
    let mut abundance = Stream::open(seed, tags::MEMBER_ABUNDANCE, key);
    let enrichment = abundance.uniform_open_low();
    let _reserved = abundance.next_u64();
    let spread_u = abundance.uniform();
    let iron_normal = abundance.standard_normal();
    let spread = model
        .marks()
        .iron_spread()
        .is_below(Threshold::from_probability(IRON_SPREAD_SHARE));
    let iron = Dex::new(if spread {
        IRON_SPREAD_SIGMA * iron_normal
    } else {
        0.0
    });
    let enriched = match class.generation {
        Generation::First => 0.0,
        Generation::Second => enrichment,
    };
    let abundances = MemberAbundances::of(
        enriched,
        model.initial_mass(),
        model.parameters().fe_h,
        iron,
    );
    let composition = Composition::from_fe_h(
        Dex::new(model.parameters().fe_h.value() + iron.value()),
        abundances.helium_excess,
    );
    let age = Years::new(model.age().value() - model.parameters().age_spread.value() * spread_u);
    // The initial mass, conditional on the class.
    let (lo, hi) = (
        class.initial_mass_range.0.value(),
        class.initial_mass_range.1.value(),
    );
    let alpha = depleted_slope(model);
    let v_eff = model.escape_speed_effective().value();
    let imf = galaxy.mass_function();
    let mut mass_stream = Stream::open(seed, tags::MEMBER_MASS, key);
    let mut chosen = None;
    for attempt in 0..MAX_ATTEMPTS {
        mass_stream.seek(ATTEMPT_WORDS * u64::from(attempt));
        let mass = imf.quantile_in(lo, hi, mass_stream.uniform());
        let mark = mass_stream.mark();
        let systemic = systemic_speed(&mut mass_stream);
        let keep = match class.kind {
            ClassKind::Living => mark.is_below(Threshold::from_probability(depletion(mass, alpha))),
            ClassKind::Tail => {
                let kept = depletion(mass, alpha);
                kept >= 1.0 || mark.is_below(Threshold::from_probability(1.0 - kept))
            }
            ClassKind::WhiteDwarf | ClassKind::NeutronStar | ClassKind::BlackHole => {
                let address = (seed, system, attempt);
                remnant_fits(
                    class.kind,
                    mass,
                    &composition,
                    address,
                    age,
                    (v_eff, systemic),
                )
            }
        };
        if keep {
            chosen = Some((mass, attempt));
            break;
        }
    }
    debug_assert!(
        chosen.is_some(),
        "a member of {class:?} exhausted its {MAX_ATTEMPTS} attempts"
    );
    let (primary_mass, attempt) = chosen?;
    let record = SystemRecord::from_parts(
        system,
        *position,
        SystemOrigin::FeatureMember {
            feature: PackedFeature::from(feature.id()),
            attempt: u16::try_from(attempt).expect("an attempt below 4,096 fits in u16"),
        },
        model.population(),
        SolarMasses::new(primary_mass),
        age,
    );
    let velocity = member_velocity(galaxy, feature, model, class, position, key, bulk);
    Some(MemberRecord {
        record,
        class: *class,
        composition,
        abundances,
        velocity,
    })
}

/// Whether the attempt's star of initial mass `mass` leaves a remnant of `kind` kicked below `v_eff`
/// (km/s), a low-mode neutron star judged on its pair's `systemic` speed instead (km/s).
pub(crate) fn remnant_fits(
    kind: ClassKind,
    mass: f64,
    composition: &Composition,
    (universe, system, attempt): (crate::Seed, SystemId, u32),
    age: Years,
    (v_eff, systemic): (f64, f64),
) -> bool {
    let want = match kind {
        ClassKind::WhiteDwarf => RemnantKind::WhiteDwarf,
        ClassKind::NeutronStar => RemnantKind::NeutronStar,
        ClassKind::BlackHole => RemnantKind::BlackHole,
        ClassKind::Living | ClassKind::Tail => return true,
    };
    let draws = StarDraws::for_attempt(universe, BodyId::new(system, 0), attempt);
    let Ok(star) = StarModel::new(SolarMasses::new(mass), *composition, draws, age) else {
        return false;
    };
    let Some(remnant) = star.remnant() else {
        return false;
    };
    if remnant.kind() != want {
        return false;
    }
    star.natal_kick().is_none_or(|kick| {
        let judged = if kick.mode() == KickMode::Low {
            systemic
        } else {
            KilometresPerSecond::from(kick.speed()).value()
        };
        judged < v_eff
    })
}

/// The words each attempt of a member's conditional draw reads on `member.mass`: the mass, the
/// mark, and a pair's systemic velocity (two normals as a pair, then one).
pub(crate) const ATTEMPT_WORDS: u64 = 6;

/// A pair's systemic speed on `stream` from its current word, km/s: an isotropic Maxwellian of σ
/// [`PAIR_SYSTEMIC_SIGMA`].
pub(crate) fn systemic_speed(stream: &mut Stream) -> f64 {
    let (a, b) = stream.standard_normal_pair();
    let c = stream.standard_normal();
    PAIR_SYSTEMIC_SIGMA * (a * a + b * b + c * c).sqrt()
}

/// The systemic speed attempt `attempt` of member `id`'s conditional draw read, km/s: what judges a
/// low-mode neutron star (for tests and the retained-kick check).
#[must_use]
pub fn attempt_systemic_speed(galaxy: &Galaxy, id: FeatureMemberId, attempt: u32) -> f64 {
    let mut stream = Stream::open(
        galaxy.seed(),
        tags::MEMBER_MASS,
        ObjectKey::from(SystemId::from(id)),
    );
    stream.seek(ATTEMPT_WORDS * u64::from(attempt) + 2);
    systemic_speed(&mut stream)
}

/// The member's velocity (module documentation).
fn member_velocity(
    galaxy: &Galaxy,
    feature: &FeatureRecord,
    model: &ClusterModel,
    class: &MemberClass,
    position: &GalacticPosition,
    key: ObjectKey,
    bulk: GalacticVelocity,
) -> GalacticVelocity {
    let local = local_offset(feature.position(), position);
    let radius = (local.x * local.x + local.y * local.y + local.z * local.z).sqrt();
    let tail = class.kind == ClassKind::Tail;
    let read_at = if tail {
        model.tidal_radius().value()
    } else {
        radius
    };
    let sigma = model.sigma(LightYears::new(read_at)).value()
        * equipartition_factor(class.mean_mass, model.turn_off_mass());
    let cut = if tail {
        f64::INFINITY
    } else {
        model.escape_speed_at(LightYears::new(radius)).value()
    };
    let norm = |w: &[f64; 3]| (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt();
    let mut stream = Stream::open(galaxy.seed(), tags::MEMBER_VELOCITY, key);
    let mut last = [0.0; 3];
    let mut kept = None;
    for attempt in 0..VELOCITY_ATTEMPTS {
        stream.seek(4 * u64::from(attempt));
        let (first, second) = stream.standard_normal_pair();
        let third = stream.standard_normal();
        let internal = [first * sigma, second * sigma, third * sigma];
        if norm(&internal) < cut {
            kept = Some(internal);
            break;
        }
        last = internal;
    }
    let internal = kept.unwrap_or_else(|| {
        let speed = norm(&last);
        last.map(|c| c * 0.99 * cut / speed)
    });
    let [bx, by, bz] = bulk.metres_per_second();
    GalacticVelocity::new([
        bx + internal[0] * 1e3,
        by + internal[1] * 1e3,
        bz + internal[2] * 1e3,
    ])
}

/// Bianchini et al.'s (2016, MNRAS 458, 3644) partial equipartition's mass for which velocities
/// turn from exponential to a power law, `m_eq` = 1.5 M☉, their maximum (ruling 126.7).
pub const EQUIPARTITION_MASS: f64 = 1.5;

/// Bianchini et al. 2016's `g(m)`: `e^(−m ÷ 2m_eq)` up to `m_eq`, `e^(−½) (m ÷ m_eq)^(−½)` above.
#[must_use]
pub fn equipartition_g(m: SolarMasses) -> f64 {
    let x = m.value() / EQUIPARTITION_MASS;
    if x <= 1.0 {
        math::exp(-0.5 * x)
    } else {
        math::exp(-0.5) / x.sqrt()
    }
}

/// A class's velocity dispersion over the stars' at the turn-off, `g(m) ÷ g(m_TO)` (ruling
/// 126.7): 1.18 for band A's 0.3 M☉ at a 0.8 M☉ turn-off, against full equipartition's 1.63.
#[must_use]
pub fn equipartition_factor(m: SolarMasses, turn_off: SolarMasses) -> f64 {
    equipartition_g(m) / equipartition_g(turn_off)
}

/// `position` relative to `centre`, ly.
#[must_use]
pub fn local_offset(centre: &GalacticPosition, position: &GalacticPosition) -> PointLy {
    let [x, y, z] = centre
        .displacement_to(position)
        .metres()
        .map(|m| m / METRES_PER_LIGHT_YEAR);
    PointLy::new(x, y, z)
}

/// A member's sphere of influence: the smaller of its galactic tidal radius and the same formula
/// about its cluster's centre, `(m (r² + a²)^(3/2) ÷ 3M)^(1/3)` in the cluster's Plummer sphere,
/// which keeps its floor `a (m ÷ 3M)^(1/3)` in the harmonic core (P09.T10). The member's mass is
/// its class's mean.
#[must_use]
pub fn sphere_of_influence(
    galaxy: &Galaxy,
    feature: &FeatureRecord,
    model: &ClusterModel,
    member: &MemberRecord,
) -> Metres {
    let position = member.record().epoch_position();
    let m = member.class().mean_mass;
    let galactic = galaxy
        .potential()
        .tidal_radius(m, &PointLy::from(position))
        .value();
    let local = local_offset(feature.position(), position);
    let r2 = local.x * local.x + local.y * local.y + local.z * local.z;
    let a = model.plummer_scale();
    let cluster_ly =
        math::cbrt(m.value() * math::powf(r2 + a * a, 1.5) / (3.0 * model.mass().value()));
    Metres::new(galactic.min(cluster_ly * METRES_PER_LIGHT_YEAR))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Seed;
    use crate::galaxy::features::FeatureProcess;
    use crate::galaxy::features::catalogue::FeatureCatalogue;
    use crate::galaxy::features::interior::MemberClassTable;
    use crate::galaxy::imf::MassBand;
    use crate::galaxy::params::GalaxyParams;
    use crate::id::MemberSlot;
    use hyperion_testkit::stats::regularised_gamma_p;

    fn galaxy() -> Galaxy {
        Galaxy::from_params(Seed::new(0x0910_0000), GalaxyParams::milky_way_like()).unwrap()
    }

    /// The most massive globular of the fixture, its model and class table.
    fn cluster(galaxy: &Galaxy) -> (FeatureRecord, ClusterModel, MemberClassTable) {
        let feature = FeatureCatalogue::walk_process(galaxy, FeatureProcess::Globular)
            .max_by(|a, b| {
                let m = |f: &FeatureRecord| match f.marks() {
                    super::super::catalogue::FeatureMarks::Globular(g) => g.mass().value(),
                    _ => 0.0,
                };
                m(a).total_cmp(&m(b))
            })
            .expect("the fixture has globulars");
        let model = ClusterModel::from_record(galaxy, &feature).expect("a globular is a cluster");
        let table = MemberClassTable::new(galaxy, &model, feature.reach(), [1.0, 0.0, 0.0]);
        (feature, model, table)
    }

    fn member_id(feature: &FeatureRecord, band: MassBand, index: u16) -> FeatureMemberId {
        FeatureMemberId::new(
            feature.id().feature_ref(),
            MemberSlot::InCell {
                band: band.layer(),
                level: 0,
                cell: [8, 8, 8],
                index,
            },
        )
        .unwrap()
    }

    fn class_of(table: &MemberClassTable, band: MassBand, kind: ClassKind) -> MemberClass {
        *table
            .classes(band)
            .find(|(c, _)| c.kind == kind && c.multiplicity == Multiplicity::Single)
            .expect("the class exists")
            .0
    }

    fn at(feature: &FeatureRecord, offset: [f64; 3]) -> GalacticPosition {
        let [x, y, z] = feature.position().to_light_years_f64();
        GalacticPosition::from_light_years([x + offset[0], y + offset[1], z + offset[2]]).unwrap()
    }

    #[test]
    fn retained_neutron_stars_are_kicked_below_the_escape_speed() {
        let galaxy = galaxy();
        let (feature, model, table) = cluster(&galaxy);
        let class = class_of(&table, MassBand::E, ClassKind::NeutronStar);
        let position = at(&feature, [1.0, 0.0, 0.0]);
        let v_eff = model.escape_speed_effective().value();
        for i in 0..40 {
            let id = member_id(&feature, MassBand::E, i);
            let member = draw_member(
                &galaxy,
                &feature,
                &model,
                &class,
                &position,
                id,
                GalacticVelocity::new([0.0; 3]),
            )
            .expect("a neutron star is retained within the attempts");
            let stars = member.stars(&galaxy);
            let primary = &stars.stars()[0];
            assert_eq!(
                primary.remnant().map(|r| r.kind()),
                Some(RemnantKind::NeutronStar)
            );
            let kick = primary.natal_kick().expect("a neutron star is kicked");
            let judged = if kick.mode() == KickMode::Low {
                attempt_systemic_speed(&galaxy, id, member.attempt())
            } else {
                KilometresPerSecond::from(kick.speed()).value()
            };
            assert!(judged < v_eff, "{judged} km/s against {v_eff}");
        }
    }

    #[test]
    fn members_disperse_in_partial_equipartition() {
        let galaxy = galaxy();
        let (feature, model, table) = cluster(&galaxy);
        let r = 0.5 * model.half_mass_radius().value();
        let position = at(&feature, [r, 0.0, 0.0]);
        for band in [MassBand::A, MassBand::C] {
            let class = class_of(&table, band, ClassKind::Living);
            let n = 3_000_u16;
            let mut sum2 = 0.0;
            for i in 0..n {
                let id = member_id(&feature, band, i);
                let m = draw_member(
                    &galaxy,
                    &feature,
                    &model,
                    &class,
                    &position,
                    id,
                    GalacticVelocity::new([0.0; 3]),
                )
                .unwrap();
                let [vx, vy, vz] = m.velocity().metres_per_second();
                sum2 += (vx * vx + vy * vy + vz * vz) * 1e-6;
            }
            let sigma = (sum2 / (3.0 * f64::from(n))).sqrt();
            let free = model.sigma(LightYears::new(r)).value()
                * equipartition_factor(class.mean_mass, model.turn_off_mass());
            // The escape cut trims the Maxwellian: E[v² | v < c] = 3σ² F₅(x) ÷ F₃(x), x = c² ÷ σ²,
            // F_k the χ² distribution of k degrees.
            let cut = model.escape_speed_at(LightYears::new(r)).value();
            let x = cut * cut / (free * free);
            let ratio = regularised_gamma_p(2.5, 0.5 * x) / regularised_gamma_p(1.5, 0.5 * x);
            let want = free * ratio.sqrt();
            assert!(
                (sigma / want - 1.0).abs() < 0.04,
                "{band:?}: {sigma} against {want}"
            );
        }
    }

    #[test]
    fn band_a_moves_1_18_times_as_fast_as_the_turn_off() {
        let f = equipartition_factor(SolarMasses::new(0.3), SolarMasses::new(0.8));
        assert!((f - 1.18).abs() < 0.005, "{f}");
        // Continuous at m_eq, and falling above it as m^(−½).
        let below = equipartition_g(SolarMasses::new(1.5 * (1.0 - 1e-12)));
        assert!((below - equipartition_g(SolarMasses::new(1.5))).abs() < 1e-9);
        let ratio = equipartition_g(SolarMasses::new(6.0)) / equipartition_g(SolarMasses::new(1.5));
        assert!((ratio - 0.5).abs() < 1e-12);
    }

    #[test]
    fn a_member_does_not_depend_on_the_order_members_are_drawn_in() {
        let galaxy = galaxy();
        let (feature, model, table) = cluster(&galaxy);
        let class = class_of(&table, MassBand::B, ClassKind::Living);
        let position = at(&feature, [0.0, 2.0, -1.0]);
        let draw = |i: u16| {
            draw_member(
                &galaxy,
                &feature,
                &model,
                &class,
                &position,
                member_id(&feature, MassBand::B, i),
                GalacticVelocity::new([0.0; 3]),
            )
        };
        let forward: Vec<_> = (0..20).map(draw).collect();
        let backward: Vec<_> = (0..20).rev().map(draw).collect();
        assert!(forward.iter().eq(backward.iter().rev()));
        let m = forward[3].unwrap();
        assert_eq!(m.record().component(), None);
        assert_eq!(m.record().population(), model.population());
        assert!(
            (class.initial_mass_range.0.value()..=class.initial_mass_range.1.value())
                .contains(&m.record().primary_initial_mass().value())
        );
    }

    #[test]
    fn generate_with_is_generate_in_for_a_grid_record() {
        use crate::galaxy::placement::{CellKey, generate_cell};
        use crate::id::Layer;
        use crate::stellar::system::draw_metallicity;
        let galaxy = galaxy();
        let mut cell = Vec::new();
        generate_cell(
            &galaxy,
            CellKey::new(Layer::C, [0, 812, 0]).unwrap(),
            &mut cell,
        );
        for record in cell.iter().take(5) {
            let a = SystemStars::generate_in(&galaxy, record, MultiplicityContext::Free);
            let b = SystemStars::generate_with(
                &galaxy,
                record,
                &draw_metallicity(&galaxy, record),
                MultiplicityContext::Free,
            );
            assert_eq!(a, b);
        }
    }

    #[test]
    fn a_member_s_sphere_of_influence_is_the_smaller_radius() {
        let galaxy = galaxy();
        let (feature, model, table) = cluster(&galaxy);
        let class = class_of(&table, MassBand::A, ClassKind::Living);
        let centre = draw_member(
            &galaxy,
            &feature,
            &model,
            &class,
            &at(&feature, [0.01, 0.0, 0.0]),
            member_id(&feature, MassBand::A, 1),
            GalacticVelocity::new([0.0; 3]),
        )
        .unwrap();
        let soi =
            sphere_of_influence(&galaxy, &feature, &model, &centre).value() / METRES_PER_LIGHT_YEAR;
        // In the harmonic core the floor: a (m ÷ 3M)^(1/3).
        let floor = model.plummer_scale()
            * math::cbrt(class.mean_mass.value() / (3.0 * model.mass().value()));
        assert!((soi / floor - 1.0).abs() < 1e-3, "{soi} against {floor}");
    }
}
