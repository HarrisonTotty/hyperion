//! Belts and cometary halos of real systems (plan 14, P14.T21): the properties over a sample of
//! systems whose planets P14.T8 placed, and the slow statistic of T21.b, the share of FGK hosts
//! with a cold belt.
//!
//! P14.T30.a does not yet call the belts, so these tests build each host's belts themselves from
//! the host's disc, plane and placed planets, as T30.a will.

use super::*;
use crate::planetary::halo::{HaloHost, Scatterer, halo};
use crate::planetary::testing::{SampleFilter, sample_contexts};
use crate::planetary::{PlanetarySystem, SystemContext, generate_planets};
use crate::stellar::Phase;
use crate::time::UniverseTime;

/// The universe the samples come from.
const SEED: Seed = Seed::new(0x00be_1750);

/// One host's belts, from its disc, plane and placed planets, in the order the system lists its
/// hosts.
fn belts_of(system: &PlanetarySystem) -> Vec<Belt> {
    let mut slot = FIRST_BELT_SLOT;
    let mut out = Vec::new();
    for host in system.hosts() {
        let Some(disc) = host.disc().profile() else {
            continue;
        };
        let planets: Vec<Neighbour> = system
            .planets()
            .filter(|body| body.host() == host.host())
            .map(|body| {
                Neighbour::new(
                    body.mass(),
                    body.orbit().unwrap().semi_major_axis(),
                    body.orbit().unwrap().eccentricity().value(),
                )
            })
            .collect();
        let belt_host = BeltHost::new(host.host(), disc, &planets, host.plane());
        let belts = host_belts(SEED, system.system(), &belt_host, slot);
        slot = belts.next_slot();
        out.extend(belts.into_belts());
    }
    out
}

/// Whether any host of `system` has a planet over 10 M⊕ beyond its snow line (P14.T21.d).
fn scatterer_of(system: &PlanetarySystem) -> Scatterer {
    system.hosts().iter().fold(Scatterer::Absent, |s, host| {
        let Some(disc) = host.disc().profile() else {
            return s;
        };
        let planets: Vec<Neighbour> = system
            .planets()
            .filter(|b| b.host() == host.host())
            .map(|b| Neighbour::new(b.mass(), b.orbit().unwrap().semi_major_axis(), 0.0))
            .collect();
        s.or(Scatterer::among(&planets, disc.snow_line()))
    })
}

/// The contexts of single FGK stars, 0.6–1.5 M☉, on the main sequence at the epoch at 1–10 Gyr,
/// from the first `n` systems of that mass band nearest the solar circle.
fn fgk_singles(n: usize) -> Vec<SystemContext> {
    let filter = SampleFilter::primary_masses(SolarMasses::new(0.6), SolarMasses::new(1.5));
    sample_contexts(n, SEED, filter)
        .expect("the solar circle holds the sample")
        .into_iter()
        .filter(|ctx| {
            let age = ctx.age_at_epoch().value();
            ctx.stars().len() == 1
                && (1e9..=1e10).contains(&age)
                && ctx.stars()[0]
                    .state_at(UniverseTime::EPOCH)
                    .is_some_and(|s| s.phase() == Phase::MainSequence)
        })
        .collect()
}

/// P14.T21.a–c on placed systems: every belt clear of every planet's chaotic zone, inside its
/// disc, members inside their belt's bounds, at most eight, decoding to its slot; and P14.T21.d:
/// halos inside the strip radius, and none without a scatterer.
#[test]
fn belts_and_halos_of_placed_systems_keep_their_bounds() {
    let contexts = sample_contexts(300, SEED, SampleFilter::ALL).expect("a sample");
    let mut belts_seen = 0;
    for ctx in &contexts {
        let system = generate_planets(SEED, ctx);
        let belts = belts_of(&system);
        // P14.T30.a wires these belts into `generate` as they are built here.
        let whole = crate::planetary::generate(SEED, ctx);
        assert_eq!(whole.belts(), belts.as_slice());
        if scatterer_of(&system) == Scatterer::Absent {
            assert!(whole.halo().is_none());
        }
        if let Some(halo) = whole.halo() {
            assert!(halo.outer_edge() <= ctx.strip_radius());
        }
        for belt in &belts {
            belts_seen += 1;
            let BodySlot::Belt(slot) = belt.index().slot() else {
                panic!("a belt in slot {:?}", belt.index().slot());
            };
            assert!(belt.members().len() <= 8);
            // Ruling 100: no member over 0.1 M⊕, the largest at most 0.4 of the belt's primordial
            // mass, and the members together at most 0.75 of it.
            let primordial = belt.initial_mass().value();
            assert!(
                belt.members_mass().value() <= MEMBER_MASS_SHARE_CAP * primordial * (1.0 + 1e-12)
            );
            if let Some(first) = belt.members().first() {
                assert!(
                    first.mass().value() <= 0.4 * primordial * (1.0 + 1e-9),
                    "{belt:?}"
                );
                assert_eq!(first.diameter(), belt.largest_diameter());
            }
            for member in belt.members() {
                assert!(member.mass() <= ROCKY_MEMBER_MASS_CAP, "{member:?}");
                assert_eq!(member.index().slot(), BodySlot::Belt(slot));
                assert_eq!(
                    BodyIndex::decode(member.index().get()),
                    Ok((BodySlot::Belt(slot), member.index().sub()))
                );
                assert!(matches!(member.index().sub(), BodySub::Member(1..=8)));
                let a = member.orbit().semi_major_axis();
                assert!(belt.inner_edge() <= a && a <= belt.outer_edge());
            }
            let disc = system
                .disc(belt.host())
                .and_then(|d| d.profile())
                .expect("a disc");
            assert!(disc.inner_edge() <= belt.inner_edge());
            assert!(belt.outer_edge() <= disc.outer_edge());
            // (a, b): no belt overlaps a planet's chaotic zone.
            for body in system.planets().filter(|b| b.host() == belt.host()) {
                let planet = Neighbour::new(
                    body.mass(),
                    body.orbit().unwrap().semi_major_axis(),
                    body.orbit().unwrap().eccentricity().value(),
                );
                let (lo, hi) = chaotic_zone(&planet, disc.host_mass());
                for part in belt.components() {
                    let (inner, outer) = (part.inner_edge().value(), part.outer_edge().value());
                    assert!(hi <= inner || lo >= outer, "{belt:?} overlaps {planet:?}");
                }
            }
        }
        let scatterer = scatterer_of(&system);
        let Some(primary) = system.hosts().first() else {
            continue;
        };
        let solids = primary.disc().solid_mass();
        let host = HaloHost::new(
            ctx.stars()[0].initial_mass(),
            solids,
            ctx.strip_radius(),
            None,
        );
        let halo = halo(SEED, system.system(), &host, scatterer);
        if scatterer == Scatterer::Absent {
            assert!(halo.is_none());
        }
        if let Some(halo) = halo {
            assert!(halo.outer_edge() <= ctx.strip_radius());
            assert!(halo.inner_edge() < halo.outer_edge());
        }
    }
    assert!(belts_seen > 100, "only {belts_seen} belts in the sample");
}

/// P14.T21.b and ruling 84 (amended): of FGK hosts of 1–10 Gyr, 0.15–0.30 have a Kuiper-like belt
/// a 100 µm survey detects, f weighted by [`detection_weight`] of at least 10⁻⁶ (Eiroa et al.
/// 2013, A&A 555, A11: 20.2 ± 2%; Montesinos et al. 2016, A&A 593, A51: 0.22 +0.08 −0.07;
/// Sibthorpe et al. 2018, 17.1%); at most 0.35 one over 10⁻⁷ by the same weighting (Montesinos
/// et al. §5.1); at most 0.03 a detected belt of any kind of blackbody temperature 100 K or more
/// (Sibthorpe et al., 5 of 275; Patel et al. 2014, 1.8 ± 0.2%). Every belt's f lies under Wyatt et
/// al.'s (2007a) collisional cap at its age.
#[test]
#[ignore = "slow: generates the planets of about 2,000 FGK systems"]
fn a_fifth_of_fgk_hosts_have_a_detected_cold_belt() {
    let contexts = fgk_singles(4_000);
    let mut hosts = 0_u32;
    let (mut detected, mut over_7, mut warm, mut warm_kuiper) = (0_u32, 0_u32, 0_u32, 0_u32);
    for ctx in &contexts {
        let system = generate_planets(SEED, ctx);
        let age = Years::new(ctx.age_at_epoch().value());
        let luminosity = ctx.stars()[0]
            .state_at(UniverseTime::EPOCH)
            .expect("a main-sequence star")
            .luminosity();
        let belts = belts_of(&system);
        let kuiper = || belts.iter().filter(|b| b.kind() == BeltKind::Kuiper);
        for belt in &belts {
            let part = belt.main();
            let cap = maximum_fractional_luminosity(
                part.radius(),
                part.relative_width(),
                belt.host_mass(),
                luminosity,
                age,
            );
            let f = belt.fractional_luminosity(age, luminosity);
            assert!(f <= cap * (1.0 + 1e-12), "{f} over the cap {cap}: {belt:?}");
        }
        let seen = |b: &&Belt| b.detectable_luminosity(age, luminosity) >= DETECTION_THRESHOLD;
        hosts += 1;
        detected += u32::from(kuiper().any(|b| seen(&b)));
        let warm_seen =
            |b: &Belt| seen(&b) && b.blackbody_temperature(luminosity) >= WARM_BELT_TEMPERATURE;
        warm += u32::from(belts.iter().any(warm_seen));
        warm_kuiper += u32::from(kuiper().any(warm_seen));
        over_7 += u32::from(kuiper().any(|b| b.detectable_luminosity(age, luminosity) > 1e-7));
    }
    let share = |n: u32| f64::from(n) / f64::from(hosts);
    eprintln!(
        "Kuiper-like belts of {hosts} FGK hosts: detected {detected} ({:.3}), weighted f > 1e-7 {over_7} ({:.3}), detected warm belts of any kind {warm} ({:.3}), of them Kuiper-like {warm_kuiper} ({:.3})",
        share(detected),
        share(over_7),
        share(warm),
        share(warm_kuiper)
    );
    assert!(hosts > 1_000, "only {hosts} hosts");
    assert!(
        (0.15..=0.30).contains(&share(detected)),
        "detected {:.3}",
        share(detected)
    );
    assert!(share(over_7) <= 0.35, "f > 1e-7 about {:.3}", share(over_7));
    assert!(share(warm) <= 0.03, "detected warm {:.3}", share(warm));
}

mod solar {
    use super::*;
    use crate::planetary::derive::{BodyHosts, HostLight, PlanetClass, derive_body};
    use crate::planetary::disc::{self, DiscDraws, DiscHost, Truncation};
    use crate::stellar::Composition;
    use crate::units::{AstronomicalUnits, Dex, Kelvin, Kilograms, Megayears, SolarRadii};

    pub(super) const SYSTEM: u64 = 0x0200_0800_2000_0000;

    pub(super) fn au(x: f64) -> Metres {
        Metres::from(AstronomicalUnits::new(x))
    }

    pub(super) fn in_au(m: Metres) -> f64 {
        AstronomicalUnits::from(m).value()
    }

    pub(super) fn system() -> SystemId {
        SystemId::from_raw(SYSTEM).unwrap()
    }

    /// The median disc of the zero-age Sun (P14.T3).
    pub(super) fn disc() -> DiscProfile {
        let host = DiscHost::new(
            SolarMasses::new(1.0),
            Dex::new(0.0),
            SolarLuminosities::new(0.7),
            SolarRadii::new(0.89),
        )
        .unwrap();
        *disc::derive(
            &host,
            Megayears::new(3.0),
            &DiscDraws::MEDIAN,
            Truncation::NONE,
        )
        .profile()
        .unwrap()
    }

    pub(super) fn plane() -> SystemPlane {
        SystemPlane::new(Radians::ZERO, Radians::ZERO).unwrap()
    }

    /// The eight planets: masses in M⊕, semi-major axes in au and eccentricities.
    pub(super) fn planets() -> Vec<Neighbour> {
        [
            (0.0553, 0.387, 0.206),
            (0.815, 0.723, 0.007),
            (1.0, 1.000, 0.017),
            (0.107, 1.524, 0.093),
            (317.8, 5.203, 0.048),
            (95.16, 9.537, 0.054),
            (14.54, 19.19, 0.047),
            (17.15, 30.07, 0.009),
        ]
        .into_iter()
        .map(|(m, a, e)| Neighbour::new(EarthMasses::new(m), au(a), e))
        .collect()
    }

    pub(super) fn belts_about(planets: &[Neighbour], disc: &DiscProfile, first: u8) -> HostBelts {
        let host = BeltHost::new(OrbitHost::Star(0), disc, planets, plane());
        host_belts(SEED, system(), &host, first)
    }

    /// P14.T21.a–b (a, b): a Solar System input gives belts at 2.1–3.3 au and 39–48 au, the main
    /// belt with the Kirkwood gaps at 2.50, 2.82 and 2.96 au, the Kuiper belt's scattered part massless.
    #[test]
    fn the_solar_system_has_its_two_belts() {
        let disc = disc();
        let belts = belts_about(&planets(), &disc, FIRST_BELT_SLOT);
        let [main, kuiper] = belts.belts() else {
            panic!("{:?}", belts.belts());
        };
        assert_eq!(
            (main.kind(), main.site()),
            (BeltKind::Asteroid, BeltSite::InsideGiant)
        );
        assert!((in_au(main.inner_edge()) - 2.065).abs() < 0.005);
        assert!((in_au(main.outer_edge()) - 3.279).abs() < 0.005);
        let gaps: Vec<f64> = main.gaps().iter().map(|g| in_au(g.radius())).collect();
        for (gap, expected) in gaps.iter().zip([2.502, 2.825, 2.958]) {
            assert!((gap - expected).abs() < 0.005, "{gaps:?}");
        }
        assert!((DEPLETION_WITH_GIANT.0..=DEPLETION_WITH_GIANT.1).contains(&main.depletion()));
        // The median solar disc's solids in the band, before depletion: 1.2 M⊕.
        let solids = main.initial_mass().value() / main.depletion();
        assert!((1.1..1.3).contains(&solids), "{solids}");
        assert_eq!(
            (kuiper.kind(), kuiper.site()),
            (BeltKind::Kuiper, BeltSite::BeyondPlanets)
        );
        assert!((in_au(kuiper.inner_edge()) - 39.4).abs() < 0.1);
        assert!((in_au(kuiper.main().outer_edge()) - 47.7).abs() < 0.1);
        let scattered = kuiper.scattered().expect("the disc reaches beyond 48 au");
        assert_eq!(scattered.inner_edge(), kuiper.main().outer_edge());
        assert_eq!(scattered.outer_edge(), disc.outer_edge());
        assert_eq!(scattered.initial_mass(), EarthMasses::ZERO);
        assert_eq!(kuiper.composition(), BeltComposition::Icy);
        assert_eq!(belts.next_slot(), FIRST_BELT_SLOT + 2);
    }

    /// Ruling 84.1: the Solar System input's cold belt, when faint as the Sun's is, keeps 10⁻³ of
    /// the median disc's 2.4 M⊕ at 39–48 au, a tenth of the 0.02 M⊕ Pitjeva and Pitjev (2018)
    /// measure, and shines at f of order 10⁻⁷ (Vitense et al. 2012, A&A 540, A30), within a
    /// factor of ten.
    #[test]
    fn the_solar_system_s_cold_belt_shines_at_ten_to_the_minus_seven() {
        let disc = disc();
        let host = BeltHost::new(OrbitHost::Star(0), &disc, &[], plane());
        let planets = planets();
        let host = BeltHost {
            planets: &planets,
            ..host
        };
        let faint = (0..64_u64)
            .map(|n| {
                let id = SystemId::from_raw(SYSTEM + (n << 8)).unwrap();
                host_belts(SEED, id, &host, FIRST_BELT_SLOT).into_belts()
            })
            .filter_map(|belts| belts.into_iter().find(|b| b.kind() == BeltKind::Kuiper))
            .find(|b| (b.depletion() - FAINT_KUIPER_EFFICIENCY).abs() < 1e-15)
            .expect("a faint belt in 64 systems");
        let solids = faint.main().solids().value();
        assert!((2.0..3.0).contains(&solids), "{solids}");
        let classical = faint.mass_at(Years::new(4.6e9));
        assert!((2e-3..4e-3).contains(&classical.value()), "{classical:?}");
        let f = faint.fractional_luminosity(Years::new(4.6e9), SolarLuminosities::new(1.0));
        // 2.4 × 10⁻⁸, a quarter of Vitense et al.'s 10⁻⁷: its 2.4 × 10⁻³ M⊕ is a tenth of the
        // Kuiper belt's 0.02 M⊕.
        assert!((1e-8..3e-7).contains(&f), "{f}");
    }

    /// P14.T21.a: a planet whose chaotic zone reaches into the band leaves no asteroid belt.
    #[test]
    fn a_planet_in_the_band_leaves_no_asteroid_belt() {
        let mut planets = planets();
        planets.push(Neighbour::new(EarthMasses::new(1.0), au(2.7), 0.0));
        let belts = belts_about(&planets, &disc(), FIRST_BELT_SLOT);
        assert!(belts.belts().iter().all(|b| b.kind() == BeltKind::Kuiper));
    }

    /// P14.T21.a: a host without giants has an asteroid belt in each gap over 40 mutual Hill
    /// radii, between the two chaotic zones, and none in a narrower one.
    #[test]
    fn a_host_without_giants_has_a_belt_in_each_wide_gap() {
        let planets = [
            Neighbour::new(EarthMasses::new(1.0), au(0.5), 0.0),
            Neighbour::new(EarthMasses::new(1.0), au(0.7), 0.0),
            Neighbour::new(EarthMasses::new(1.0), au(3.0), 0.0),
        ];
        let belts = belts_about(&planets, &disc(), FIRST_BELT_SLOT);
        let [gap, kuiper] = belts.belts() else {
            panic!("{:?}", belts.belts());
        };
        assert_eq!(gap.site(), BeltSite::Gap);
        let zone = |p: &Neighbour| chaotic_zone(p, SolarMasses::new(1.0));
        assert!((gap.inner_edge().value() - zone(&planets[1]).1).abs() < 1.0);
        assert!((gap.outer_edge().value() - zone(&planets[2]).0).abs() < 1.0);
        assert!((DEPLETION_WITHOUT_GIANT.0..=1.0).contains(&gap.depletion()));
        assert_eq!(kuiper.kind(), BeltKind::Kuiper);
    }

    /// P14.T21.b: a host without planets has a belt in its disc's outer third.
    #[test]
    fn a_host_without_planets_has_its_disc_s_outer_third() {
        let disc = disc();
        let belts = belts_about(&[], &disc, FIRST_BELT_SLOT);
        let [belt] = belts.belts() else {
            panic!("{:?}", belts.belts());
        };
        assert_eq!(
            (belt.kind(), belt.site()),
            (BeltKind::Kuiper, BeltSite::OuterDisc)
        );
        let (lo, hi) = (disc.inner_edge().value(), disc.outer_edge().value());
        assert!((belt.inner_edge().value() - (lo + 2.0 * (hi - lo) / 3.0)).abs() < 1.0);
        assert_eq!(belt.outer_edge(), disc.outer_edge());
        assert!(belt.scattered().is_none());
    }

    /// Ruling 95.2: a Kuiper-like belt inside the snow line is rocky, although its massless
    /// scattered component reaches beyond it and holds more solids: its composition follows its
    /// mass.
    #[test]
    fn a_cold_belt_inside_the_snow_line_is_rocky() {
        let disc = disc();
        let planets = [Neighbour::new(EarthMasses::new(1.0), au(0.3), 0.0)];
        let belts = belts_about(&planets, &disc, FIRST_BELT_SLOT);
        let [belt] = belts.belts() else {
            panic!("{:?}", belts.belts());
        };
        assert_eq!(belt.kind(), BeltKind::Kuiper);
        let scattered = belt.scattered().expect("the disc reaches beyond the 2:1");
        assert!(belt.main().outer_edge() < disc.snow_line());
        // The old rule, the solids between the belt's edges, would read it icy.
        let beyond = disc.solid_mass_between(disc.snow_line(), scattered.outer_edge());
        assert!(beyond * 2.0 >= belt.main().solids() + scattered.solids());
        assert_eq!(scattered.initial_mass(), EarthMasses::ZERO);
        assert_eq!(belt.composition(), BeltComposition::Rocky);
    }

    /// Ruling 100.2: members are sized from the belt's primordial mass and do not wear. The belt's
    /// cascade is its mass less its members', and it alone wears, so an old belt close in, its
    /// cascade worn to a sliver, keeps its members and reports them in its mass; no member
    /// outweighs its belt (ruling 95.2), and together they hold at most 0.75 of its start.
    #[test]
    fn members_are_sized_from_the_primordial_mass_and_do_not_wear() {
        let disc = disc();
        let planets = [Neighbour::new(EarthMasses::new(1.0), au(1.0), 0.02)];
        let host = BeltHost::new(OrbitHost::Star(0), &disc, &planets, plane());
        let belts = host_belts(SEED, system(), &host, FIRST_BELT_SLOT);
        let belt = &belts.belts()[0];
        assert!(!belt.members().is_empty());
        let primordial = belt.main().solids() * belt.depletion();
        assert!((belt.initial_mass().value() / primordial.value() - 1.0).abs() < 1e-12);
        let cascade = belt.main().initial_mass();
        let members = belt.members_mass();
        assert!(((cascade + members).value() / primordial.value() - 1.0).abs() < 1e-12);
        assert!(members.value() <= MEMBER_MASS_SHARE_CAP * primordial.value());
        let (young, old) = (
            belt.mass_at(Years::new(1e6)),
            belt.mass_at(Years::new(4.6e9)),
        );
        let worn = belt
            .main()
            .mass_at(Years::new(4.6e9), SolarMasses::new(1.0));
        assert!(worn < cascade * 1e-2, "{worn:?} of {cascade:?}");
        assert!(((old - worn - members).value() / old.value()).abs() < 1e-12);
        assert!(young > old && old > members);
        for member in belt.members() {
            assert!(member.mass() <= old, "{member:?} outweighs {old:?}");
        }
        let sizes = member_sizes(
            primordial,
            belt.composition(),
            nebula_ratio(
                belt.main().solids(),
                belt.main().inner_edge(),
                belt.main().outer_edge(),
                SolarMasses::new(1.0),
            ),
            &SizeDraws {
                largest_share: BeltDraws::for_slot(SEED, system(), FIRST_BELT_SLOT).largest_share,
                spreads: core::array::from_fn(|k| {
                    let sub = u8::try_from(k + 1).unwrap();
                    let index =
                        BodyIndex::new(BodySlot::Belt(FIRST_BELT_SLOT), BodySub::Member(sub))
                            .unwrap();
                    MemberDraws::for_member(SEED, system(), index).growth_spread
                }),
            },
        );
        assert_eq!(sizes.largest_diameter(), belt.largest_diameter());
        let expected: Vec<Metres> = sizes.members().iter().map(MemberSize::diameter).collect();
        let found: Vec<Metres> = belt.members().iter().map(BeltMember::diameter).collect();
        assert_eq!(found, expected);
    }

    /// Ruling 84.1: a Kuiper-like belt is bright with probability 0.60, keeping a median 10^−0.25
    /// of its solids, and otherwise keeps 10⁻³.
    #[test]
    fn kuiper_belts_are_bright_or_faint() {
        let n = 8_000_u64;
        let mut bright = Vec::new();
        for i in 0..n {
            let id = SystemId::from_raw(SYSTEM + (i << 8)).unwrap();
            let e = BeltDraws::for_slot(SEED, id, 2).kuiper_efficiency();
            if (e - FAINT_KUIPER_EFFICIENCY).abs() > 1e-15 {
                bright.push(math::ln(e) / core::f64::consts::LN_10);
            }
        }
        #[expect(clippy::cast_precision_loss, reason = "a count of 8,000")]
        let mean = BRIGHT_KUIPER_BELT_PROBABILITY * n as f64;
        hyperion_testkit::stats::assert_poisson_count(
            "bright belts",
            u64::try_from(bright.len()).unwrap(),
            mean,
            hyperion_testkit::stats::ALPHA,
        );
        bright.sort_by(f64::total_cmp);
        let median = bright[bright.len() / 2];
        assert!(
            (median - BRIGHT_KUIPER_EFFICIENCY_DEX).abs() < 0.05,
            "{median}"
        );
    }

    /// Belt slots: a host's belts take consecutive slots from the one given, and none goes beyond
    /// the last belt slot.
    #[test]
    fn belts_take_consecutive_slots_and_stop_at_the_last() {
        let from_twelve = belts_about(&planets(), &disc(), 12);
        let slots: Vec<BodySlot> = from_twelve
            .belts()
            .iter()
            .map(|b| b.index().slot())
            .collect();
        assert_eq!(slots, [BodySlot::Belt(12), BodySlot::Belt(13)]);
        let from_thirteen = belts_about(&planets(), &disc(), LAST_BELT_SLOT);
        assert_eq!(from_thirteen.belts().len(), 1);
        assert_eq!(from_thirteen.next_slot(), LAST_BELT_SLOT + 1);
    }

    /// P14.T21.c (c): a massive belt's members are its bodies over 400 km, at most eight, largest
    /// first, inside its bounds and clear of the planets' chaotic zones, in its slot from
    /// sub-index 1; and P14.T16 derives each as a dwarf planet.
    #[test]
    fn a_belt_s_largest_members_are_dwarf_planets_inside_it() {
        let disc = disc();
        let planets = [Neighbour::new(EarthMasses::new(1.0), au(1.0), 0.02)];
        let host = BeltHost::new(OrbitHost::Star(0), &disc, &planets, plane());
        let belts = host_belts(SEED, system(), &host, FIRST_BELT_SLOT);
        let belt = &belts.belts()[0];
        let members = belt.members();
        assert!(!members.is_empty() && members.len() <= usize::from(MAX_MEMBERS));
        let sun = [HostLight::new(
            SolarLuminosities::new(1.0),
            Kelvin::new(5_772.0),
            SolarRadii::new(1.0),
        )
        .unwrap()];
        let hosts = BodyHosts::new(
            Kilograms::from(SolarMasses::new(1.0)),
            Composition::SOLAR,
            &sun,
            &[],
        )
        .unwrap();
        let zone = chaotic_zone(&planets[0], SolarMasses::new(1.0));
        for (k, member) in members.iter().enumerate() {
            assert_eq!(member.index().slot(), belt.index().slot());
            assert_eq!(
                member.index().sub(),
                BodySub::Member(u8::try_from(k + 1).unwrap())
            );
            assert_eq!(member.index().parent(), Some(belt.index()));
            assert!(member.diameter() > MEMBER_MIN_DIAMETER);
            if k > 0 {
                assert!(member.diameter() <= members[k - 1].diameter());
            }
            let orbit = member.orbit();
            let a = orbit.semi_major_axis();
            assert!(belt.inner_edge() <= a && a <= belt.outer_edge());
            let e = orbit.eccentricity().value();
            assert!(a.value() * (1.0 - e) >= zone.1);
            let derived = derive_body(
                &member.placed_body(),
                &hosts,
                &disc,
                Years::new(4.6e9),
                UniverseTime::EPOCH,
            )
            .unwrap();
            assert!(matches!(
                derived.class(),
                PlanetClass::Rocky | PlanetClass::Icy
            ));
            // The derived radius and the size distribution's agree to a factor of two.
            let radius = Metres::from(derived.radius()).value();
            let ratio = 2.0 * radius / member.diameter().value();
            assert!((0.5..2.0).contains(&ratio), "{ratio}");
        }
    }
}

/// Wyatt et al. (2007a): a belt's mass halves at its collisional lifetime and fades as 1 ÷ age
/// long after, and its fractional luminosity at the maximum mass is their eq. 18's `f_max`, with
/// the full G (ruling 84.1).
#[test]
fn belts_wear_down_as_wyatt_s_model_has_it() {
    let component = BeltComponent {
        part: BeltPart::Main,
        inner_edge: solar::au(4.0),
        outer_edge: solar::au(6.0),
        solids: EarthMasses::new(1.0),
        initial_mass: EarthMasses::new(1.0),
    };
    let sun = SolarMasses::new(1.0);
    let t_c = component.collisional_time(sun);
    let half = component.mass_at(t_c, sun).value();
    assert!((half - 0.5).abs() < 1e-12);
    let late = |t: f64| component.mass_at(Years::new(t_c.value() * t), sun).value();
    assert!((late(1e4) / late(1e5) - 10.0).abs() < 0.01);

    let (r, width, age) = (solar::au(40.0), 0.5, Years::new(1e9));
    let luminosity = SolarLuminosities::new(1.0);
    // M_max(t) is the mass whose collisional lifetime is t.
    let t_unit = collisional_time(r, width, sun, EarthMasses::new(1.0)).value();
    let m_max = EarthMasses::new(t_unit / age.value());
    let f_max = fractional_luminosity(m_max, r, luminosity, sun);
    // Eq. 18, f_max = 0.004 r^1.5 (dr ÷ r) D_c^0.5 L^−0.5 ÷ (t_age G), whose 0.004 rounds this
    // module's 0.373 × 0.009 ÷ √0.8 = 0.00375.
    let g = cascade_factor(r, sun);
    let eq_18 = 0.00375 * math::powf(40.0, 1.5) * width * CASCADE_TOP_DIAMETER.sqrt()
        / (age.value() / 1e6 * g);
    assert!(
        (f_max / eq_18 - 1.0).abs() < 0.01,
        "{f_max} against {eq_18}"
    );
    // G at 40 au: X_c = 10⁻³ (40 × 495 ÷ 0.0025)^⅓ = 0.20, where the small-X_c form is low.
    let x: f64 = 1e-3 * math::cbrt(40.0 * DISPERSAL_THRESHOLD / 0.0025);
    assert!((x - 0.199).abs() < 0.001, "{x}");
    let small = 0.2 * math::powf(x, -2.5);
    assert!(small / g > 0.5 && small / g < 0.8, "{small} {g}");
    // The FGK prefactor: D_c^½ Q_D*^⅚ e^(−5⁄3) is Sibthorpe et al.'s 5.5 × 10⁵ (ruling 84, amended).
    let a = CASCADE_TOP_DIAMETER.sqrt()
        * math::powf(DISPERSAL_THRESHOLD, 5.0 / 6.0)
        * math::powf(CASCADE_ECCENTRICITY, -5.0 / 3.0);
    assert!((a / 5.5e5 - 1.0).abs() < 0.01, "{a}");
    // The cap at 3 au (dr ÷ r = 0.2) about the Sun at 1 Gyr: 6.5 × 10⁻⁷.
    let cap = maximum_fractional_luminosity(
        solar::au(3.0),
        0.2,
        sun,
        SolarLuminosities::new(1.0),
        Years::new(1e9),
    );
    assert!((6.0e-7..7.0e-7).contains(&cap), "{cap}");
    assert!(cascade_factor(solar::au(1e4), sun).abs() < 1e-300);
    assert!(fractional_luminosity(m_max, r, SolarLuminosities::ZERO, sun).abs() < 1e-300);
}

/// Design note 4: a belt's draws are words 8n to 8n + 5 of the system's `belt.population`
/// stream for belt slot n, and a member's are words 0–8 of its own `belt.member` stream (ruling
/// 100.5 added words 8n + 5 and 8).
#[test]
fn belt_and_member_draws_are_their_own_words() {
    let system = solar::system();
    let stream = Stream::open(SEED, tags::BELT_POPULATION, ObjectKey::from(system));
    let draws = BeltDraws::for_slot(SEED, system, 3);
    let mut at = stream.clone();
    at.seek(24);
    assert!((draws.depletion.value() - at.uniform_open()).abs() < 1e-300);
    assert!((draws.size_slope.value() - at.uniform_open()).abs() < 1e-300);
    assert_eq!(draws.bright, Mark::from_word(at.next_u64()));
    assert!((draws.efficiency.value() - at.standard_normal()).abs() < 1e-300);
    assert_eq!(at.position(), 29);
    assert!((draws.largest_share.value() - at.uniform_open()).abs() < 1e-300);
    assert_eq!(at.position(), 30);
    let member = BodyIndex::new(BodySlot::Belt(3), BodySub::Member(2)).unwrap();
    let mut own = Stream::open(
        SEED,
        tags::BELT_MEMBER,
        ObjectKey::from(member.body_id(system)),
    );
    let words = MemberDraws::for_member(SEED, system, member);
    assert!((words.part.value() - own.uniform_open()).abs() < 1e-300);
    own.seek(7);
    assert!((words.radius_rank.value() - own.uniform_open()).abs() < 1e-300);
    assert!((words.growth_spread.value() - own.uniform_open()).abs() < 1e-300);
    assert_eq!(own.position(), MEMBER_WORDS);
    let disc = solar::disc();
    assert_eq!(
        solar::belts_about(&solar::planets(), &disc, FIRST_BELT_SLOT),
        solar::belts_about(&solar::planets(), &disc, FIRST_BELT_SLOT)
    );
}
