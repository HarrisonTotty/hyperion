//! A dwarf core's members (P10.T7.b): every member of a small core, placed once, and the laws,
//! profiles, IDs, capacity, order independence and golden of its members.

use std::sync::OnceLock;

use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::order::assert_order_independent;
use hyperion_testkit::stats::{ALPHA, assert_p_value, ks_one_sample};

use super::*;
use crate::galaxy::features::interior::{ClassKind, Generation, Multiplicity};
use crate::galaxy::global_list::spec::{DwarfCoreSpec, SphericalOrbit};
use crate::galaxy::params::GalaxyParams;
use crate::id::{Layer, SystemIdKind};
use crate::rng::Seed;
use crate::stellar::ObjectKind;
use crate::time::UniverseTime;
use crate::units::LightYears;

fn milky_way() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        Galaxy::from_params(Seed::new(0x0a10_7b00), GalaxyParams::milky_way_like()).unwrap()
    })
}

/// A core of `mass` M☉ still bound, `infall` at accretion `accreted` years ago, at `at` ly about
/// the centre, numbered `number`.
fn spec(mass: f64, infall: f64, accreted: f64, at: [f64; 3], number: u8) -> DwarfCoreSpec {
    let galaxy = milky_way();
    let orbit = SphericalOrbit::between(
        galaxy.potential(),
        LightYears::new(30_000.0),
        LightYears::new(120_000.0),
    )
    .unwrap();
    DwarfCoreSpec {
        number: DwarfCoreNumber::new(number).unwrap(),
        progenitor: 2,
        mass: SolarMasses::new(mass),
        systems: mass / galaxy.mean_system_mass(Population::Halo).value(),
        accreted: Years::new(accreted),
        accreted_mass: SolarMasses::new(infall),
        position: GalacticDisplacement::new(at.map(|x| x * METRES_PER_LIGHT_YEAR)),
        velocity: GalacticVelocity::new([1.2e5, -8e4, 3e4]),
        orbit,
        stripping_fraction: 0.3,
        pericentres: 4,
    }
}

/// The small core every property reads: 2 × 10⁴ M☉ left of 6 × 10⁴ at an accretion 3 Gyr ago,
/// so its tail ran from 10 to 2 Gyr ago, about 33,000 systems.
fn small() -> &'static DwarfCoreModel {
    static CORE: OnceLock<DwarfCoreModel> = OnceLock::new();
    CORE.get_or_init(|| {
        let spec = spec(2e4, 6e4, 3e9, [30_000.0, 20_000.0, 5_000.0], 1);
        DwarfCoreModel::from_spec(milky_way(), &spec)
    })
}

/// Every member of [`small`], band by band in its grid's owned cells, with its band and cell.
fn members() -> &'static [(MassBand, NestedCell, GlobalMemberRecord, PointLy)] {
    static MEMBERS: OnceLock<Vec<(MassBand, NestedCell, GlobalMemberRecord, PointLy)>> =
        OnceLock::new();
    MEMBERS.get_or_init(|| {
        let (galaxy, core) = (milky_way(), small());
        let mut all = Vec::new();
        let mut cell_members = Vec::new();
        for band in MassBand::ALL {
            for cell in core.grid().owned_cells() {
                core.members_in_cell(galaxy, band, cell, &mut cell_members);
                all.extend(cell_members.iter().map(|&(m, p)| (band, cell, m, p)));
            }
        }
        all
    })
}

/// The ellipsoidal radius of the local point `p` of a core flattened by [`FLATTENING`].
fn ellipsoidal(p: &PointLy) -> f64 {
    let z = p.z / super::super::FLATTENING;
    (p.x * p.x + p.y * p.y + z * z).sqrt()
}

/// P10.T7.b: the members sum to the core's systems, are not coeval, take their core's
/// metallicity and motion and are the halo's, with no density component.
#[test]
fn members_are_not_coeval_and_follow_their_core_s_laws() {
    let core = small();
    let members = members();
    let n = f64::from(u32::try_from(members.len()).unwrap());
    // The candidates expected sum to more than the members, the thinning keeping the profile's
    // share; the members are the systems.
    assert!(
        (n / core.systems() - 1.0).abs() < 0.03,
        "{n} members against {} systems",
        core.systems()
    );
    let (tail_end, _) = core.history().tail();
    let mut ages: Vec<f64> = members
        .iter()
        .map(|(_, _, m, _)| m.record().age_at_epoch().value())
        .collect();
    ages.sort_by(f64::total_cmp);
    let quantile = |percent: usize| ages[(ages.len() - 1) * percent / 100];
    assert!(ages[0] >= tail_end.value() - 1.0 && ages[ages.len() - 1] <= 13e9 + 1.0);
    assert!(
        quantile(95) - quantile(5) > 2e9,
        "{} to {}",
        quantile(5),
        quantile(95)
    );
    let mean_of = |f: &dyn Fn(&GlobalMemberRecord) -> f64| {
        members.iter().map(|(_, _, m, _)| f(m)).sum::<f64>() / n
    };
    let fe_h = mean_of(&|m| m.composition().fe_h().value());
    let spread = mean_of(&|m| {
        let d = m.composition().fe_h().value() - fe_h;
        d * d
    })
    .sqrt();
    let (want, want_spread) = (core.fe_h().value(), core.fe_h_spread().value());
    assert!(
        (fe_h - want).abs() < 4.0 * want_spread / n.sqrt(),
        "{fe_h} {want}"
    );
    assert!((spread / want_spread - 1.0).abs() < 0.03, "{spread}");
    let sigma = core.dispersion().value() * 1e3;
    let bulk = core.velocity().metres_per_second();
    for (axis, &centre) in bulk.iter().enumerate() {
        let mean = mean_of(&|m| m.velocity().metres_per_second()[axis] - centre);
        let rms = mean_of(&|m| {
            let d = m.velocity().metres_per_second()[axis] - centre;
            d * d
        })
        .sqrt();
        assert!(mean.abs() < 4.0 * sigma / n.sqrt(), "axis {axis}: {mean}");
        assert!(
            (rms / sigma - 1.0).abs() < 0.03,
            "axis {axis}: {rms} {sigma}"
        );
    }
    for (_, _, m, _) in members {
        let record = m.record();
        assert_eq!(record.population(), Population::Halo);
        assert_eq!(record.component(), None);
        assert!(matches!(
            record.origin(),
            SystemOrigin::GlobalListMember { .. }
        ));
        assert_eq!(m.class().generation, Generation::First);
        assert_eq!(m.class().multiplicity, Multiplicity::Single);
    }
}

/// P10.T7.b: each class's members follow its profile in ellipsoidal radius, by a one-sample
/// Kolmogorov–Smirnov test, in every band with a class of 300 members or more.
#[test]
fn members_follow_their_class_profiles() {
    let core = small();
    let members = members();
    let mut tested = 0;
    for band in MassBand::ALL {
        for (index, (class, _)) in core.table().classes(band).enumerate() {
            let profile = core.table().profile(band, index).unwrap();
            let mut radii: Vec<f64> = members
                .iter()
                .filter(|(b, _, m, _)| *b == band && m.class() == class)
                .map(|(_, _, _, p)| ellipsoidal(p))
                .collect();
            if radii.len() < 300 {
                continue;
            }
            let ks = ks_one_sample(&mut radii, |r| profile.cumulative(r));
            assert_p_value(
                &format!("{band:?} {:?}'s radii", class.kind),
                ks.p_value,
                ALPHA,
            );
            tested += 1;
        }
    }
    assert!(tested >= 3, "{tested} classes tested");
}

/// P10.T7.b: every member's ID round-trips through the canonical decode and resolves to the same
/// member; a cell an inner level owns has no ID, and an index past its cell's count, a
/// feature-level ID and another core's ID resolve to nothing.
#[test]
fn every_member_s_id_round_trips_and_resolves_to_it() {
    let (galaxy, core) = (milky_way(), small());
    let members = members();
    // Every ID round-trips; a sample of them resolves, the rest placed by the same function.
    for (_, _, member, _) in members {
        let id = member.record().id();
        assert_eq!(SystemId::from_raw(id.raw()), Ok(id));
    }
    for (band, cell, member, _) in members.iter().step_by(37) {
        let id = member.record().id();
        let decoded = SystemId::from_raw(id.raw()).unwrap();
        assert_eq!(decoded, id);
        let SystemIdKind::DwarfCore(core_id) = decoded.kind() else {
            panic!("a core member's ID decodes as one: {id:?}");
        };
        assert_eq!(core_id.number(), 1);
        let MemberSlot::InCell {
            band: layer,
            level,
            cell: at,
            ..
        } = core_id.slot()
        else {
            panic!("a placed member is in a cell")
        };
        assert_eq!(
            (layer, level, at),
            (band.layer(), cell.level(), cell.cell())
        );
        assert_eq!(
            resolve_dwarf_core_member(galaxy, core, core_id),
            Ok(*member)
        );
    }
    let (band, cell, ..) = members[0];
    let past = core.member_id(band, cell, core.candidate_count(galaxy, band, cell));
    // A cell an inner level owns has no ID: the canonical form refuses it, and so does the
    // decode of its bits.
    let inner = MemberSlot::InCell {
        band: Layer::A,
        level: 1,
        cell: [8, 8, 8],
        index: 0,
    };
    assert_eq!(
        DwarfCoreMemberId::new(1, inner),
        Err(crate::id::BuildSystemIdError::NotCanonical(
            crate::id::DecodeSystemIdError::InnerCellOwnedByLowerLevel
        ))
    );
    let listed = DwarfCoreMemberId::new(1, MemberSlot::FeatureLevel { index: 1 }).unwrap();
    let other = DwarfCoreMemberId::new(2, core.member_id(band, cell, 0).slot()).unwrap();
    for id in [past, listed, other] {
        assert_eq!(
            resolve_dwarf_core_member(galaxy, core, id),
            Err(ResolveSystemError::NoSuchSystem),
            "{id:?}"
        );
    }
    assert!(core.feature_level_list(galaxy).is_empty());
}

/// P10.T7.b: the members' stars are of their classes at the epoch: a living member's primary is
/// a star, a white dwarf's a white dwarf, a neutron star's and a black hole's theirs.
#[test]
fn a_member_s_star_is_of_its_class() {
    let galaxy = milky_way();
    let members = members();
    let mut seen = [false; 4];
    for kind in [
        ClassKind::Living,
        ClassKind::WhiteDwarf,
        ClassKind::NeutronStar,
        ClassKind::BlackHole,
    ] {
        for (_, _, member, _) in members
            .iter()
            .filter(|(_, _, m, _)| m.class().kind == kind)
            .take(5)
        {
            let brief = member
                .stars(galaxy)
                .brief_at(UniverseTime::EPOCH)
                .expect("a member exists at the epoch");
            let ok = match kind {
                ClassKind::Living => !matches!(
                    brief.kind(),
                    ObjectKind::WhiteDwarf | ObjectKind::NeutronStar | ObjectKind::BlackHole
                ),
                ClassKind::WhiteDwarf => brief.kind() == ObjectKind::WhiteDwarf,
                ClassKind::NeutronStar => brief.kind() == ObjectKind::NeutronStar,
                ClassKind::BlackHole => brief.kind() == ObjectKind::BlackHole,
                ClassKind::Tail => unreachable!("a core has no tail"),
            };
            assert!(ok, "{kind:?}: {:?}", brief.kind());
            seen[kind as usize] = true;
        }
    }
    assert!(
        seen[0] && seen[1],
        "living stars and white dwarfs among the members"
    );
}

/// P10.T7.b: no cell of a core that passes the headroom check expects 8,192 candidates, its peak
/// the greatest over every owned cell; a core of 10⁷ M☉ fails the check.
#[test]
fn no_cell_of_a_core_under_the_headroom_expects_the_index() {
    let galaxy = milky_way();
    for mass in [2e3, 1e5, 1e6, 3e6] {
        let core = DwarfCoreModel::from_spec(
            galaxy,
            &spec(mass, 3.0 * mass, 3e9, [30_000.0, 20_000.0, 5_000.0], 0),
        );
        assert_eq!(core.check_index_headroom(), Ok(()), "{mass:e}");
        let peak = core.peak_candidates();
        assert!(
            peak < f64::from(MemberSlot::INDEX_LIMIT),
            "{mass:e}: {peak}"
        );
        if mass <= 1e5 {
            let fullest = core
                .grid()
                .owned_cells()
                .flat_map(|c| MassBand::ALL.map(|b| core.expected_candidates(b, c)))
                .fold(0.0, f64::max);
            assert!(fullest <= peak, "{mass:e}: {fullest} against {peak}");
        }
    }
    let big = DwarfCoreModel::from_spec(
        galaxy,
        &spec(1e7, 3e7, 3e9, [30_000.0, 20_000.0, 5_000.0], 3),
    );
    let error = big.check_index_headroom().unwrap_err();
    assert_eq!(error.number.get(), 3);
    assert!(error.expected > 8_192.0, "{error:?}");
    let message = error.to_string();
    assert!(!message.starts_with(char::is_uppercase) && !message.ends_with('.'));
}

/// P10.T7.b: no member of the core moves faster than its speed bound, the range query's pad.
#[test]
fn no_member_moves_faster_than_the_core_s_speed_bound() {
    let bound = small().member_speed_bound().value();
    let fastest = members()
        .iter()
        .map(|(_, _, m, _)| KilometresPerSecond::from(m.velocity().speed()).value())
        .fold(0.0, f64::max);
    assert!(fastest <= bound, "{fastest} km/s against {bound}");
    let bulk = KilometresPerSecond::from(small().velocity().speed()).value();
    assert!(fastest > bulk, "the members move about the bulk");
}

/// P10.T7.b: a member is a pure function of its ID and its core, whatever was placed before it.
#[test]
fn members_are_order_independent() {
    let (galaxy, core) = (milky_way(), small());
    let keys: Vec<(MassBand, NestedCell, u16)> = members()
        .iter()
        .step_by(211)
        .map(|(band, cell, m, _)| {
            let SystemIdKind::DwarfCore(id) = m.record().id().kind() else {
                unreachable!("a core member's ID")
            };
            let MemberSlot::InCell { index, .. } = id.slot() else {
                unreachable!("in a cell")
            };
            (*band, *cell, index)
        })
        .collect();
    assert!(keys.len() > 20);
    assert_order_independent(&keys, |&(band, cell, index)| {
        core.candidate(galaxy, band, cell, index)
    });
}

/// P10.T7.b's golden: the small core's model and its members' counts by band, and the first three
/// members of each band in full. They move with the core's model (its history, profile and
/// metallicity), plan 09's placement and the star's draw.
#[test]
fn dwarf_core_members_golden() {
    let core = small();
    let mut w = GoldenWriter::new();
    w.header(crate::GENERATOR_VERSION.get());
    w.f64("core.systems", core.systems());
    w.f64("core.extent", core.extent().value());
    w.f64("core.scale", core.plummer_scale().value());
    w.f64("core.dispersion", core.dispersion().value());
    w.f64("core.fe_h", core.fe_h().value());
    w.f64("core.fe_h_spread", core.fe_h_spread().value());
    w.f64("core.tail_end", core.history().tail().0.value());
    w.f64("core.peak", core.peak_candidates());
    let members = members();
    for band in MassBand::ALL {
        let of_band: Vec<&GlobalMemberRecord> = members
            .iter()
            .filter(|(b, ..)| *b == band)
            .map(|(_, _, m, _)| m)
            .collect();
        w.line(&format!("{band:?}.members {}", of_band.len()));
        for (k, m) in of_band.iter().take(3).enumerate() {
            let label = format!("{band:?}.{k}");
            let record = m.record();
            w.u64_hex(&format!("{label}.id"), record.id().raw());
            w.line(&format!(
                "{label}.class {:?} attempt {}",
                m.class().kind,
                m.attempt()
            ));
            for (axis, c) in ["x", "y", "z"]
                .iter()
                .zip(record.epoch_position().to_light_years_f64())
            {
                w.f64(&format!("{label}.position.{axis}"), c);
            }
            w.f64(
                &format!("{label}.mass"),
                record.primary_initial_mass().value(),
            );
            w.f64(&format!("{label}.age"), record.age_at_epoch().value());
            w.f64(&format!("{label}.fe_h"), m.composition().fe_h().value());
            for (axis, v) in ["x", "y", "z"].iter().zip(m.velocity().metres_per_second()) {
                w.f64(&format!("{label}.velocity.{axis}"), v);
            }
        }
    }
    golden!("galaxy/global_list/dwarf_core_members", w.as_str());
}

/// A lookup whose tube tables must not be read: no core member needs one.
struct NoTubes(crate::galaxy::global_list::GlobalList);

impl crate::galaxy::global_list::TubeLookup for NoTubes {
    fn global_list(&self) -> &crate::galaxy::global_list::GlobalList {
        &self.0
    }

    fn tube(
        &self,
        stream: crate::galaxy::global_list::StreamNumber,
    ) -> std::sync::Arc<crate::galaxy::global_list::TubeTable> {
        panic!("a core member read stream {}'s tube", stream.get())
    }
}

/// P10.T7.b: `resolve_with` reaches a drawn core's members, through the galaxy's debris, and
/// gives each the record placing it gave; plan 03's `resolve` still does not.
#[test]
#[ignore = "slow: builds a galaxy's kinematic tables and its debris"]
fn resolve_with_reaches_a_drawn_core_s_members() {
    use crate::galaxy::global_list::Debris;
    use crate::galaxy::placement::{resolve, resolve_with};
    let galaxy = Galaxy::from_params(Seed::new(0x0a10_7b00), GalaxyParams::milky_way_like())
        .unwrap()
        .with_full_potential();
    let debris = Debris::generate(&galaxy);
    let spec = debris.cores().first().expect("the fixture keeps a core");
    let core = DwarfCoreModel::from_spec(&galaxy, spec);
    assert_eq!(core.check_index_headroom(), Ok(()));
    let lookup = NoTubes(crate::galaxy::global_list::GlobalList::default());
    let mut members = Vec::new();
    let mut resolved = 0;
    for cell in core.grid().owned_cells().filter(|c| c.level() <= 1) {
        core.members_in_cell(&galaxy, MassBand::A, cell, &mut members);
        for (member, _) in members.iter().take(2) {
            let id = member.record().id();
            assert_eq!(resolve_with(&galaxy, id, &lookup), Ok(*member.record()));
            assert_eq!(
                resolve(&galaxy, id),
                Err(ResolveSystemError::KindNotGenerated)
            );
            resolved += 1;
        }
        if resolved >= 20 {
            break;
        }
    }
    assert!(resolved >= 20, "{resolved} members resolved");
}
