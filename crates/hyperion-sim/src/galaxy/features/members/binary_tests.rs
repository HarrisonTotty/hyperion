//! The tests of a feature's members' multiplicity under every 27 hold (plan 11, P11.T8.f): the
//! hard–soft boundary, the open clusters' binary fractions, and the conditional draws of the
//! members plan 09 marks as recycled objects, in a 47 Tucanae-like cluster.

use std::fmt::Write as _;
use std::sync::OnceLock;

use hyperion_testkit::order::assert_order_independent;

use super::placement::recycled_shares;
use super::*;
use crate::Seed;
use crate::galaxy::catalogue_classes::binary::feature::{FeatureBinariesRules, globular_hosts};
use crate::galaxy::features::FeatureProcess;
use crate::galaxy::features::catalogue::FeatureCatalogue;
use crate::galaxy::features::interior::MemberClassTable;
use crate::galaxy::features::interior::counts::{BINARY_FRACTIONS, open_cluster_binary_fraction};
use crate::galaxy::features::testing::{TUC_47, catalogue_parameters, named_cluster};
use crate::galaxy::imf::MassBand;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::rules::GalaxyRules;
use crate::id::MemberSlot;
use crate::stellar::binary::PulsarAt;
use crate::stellar::multiplicity::RedrawAttempt;
use crate::stellar::multiplicity::draw_hierarchy_of_composition;
use crate::time::UniverseTime;

/// The Milky Way under every 27 hold, or under every 27 hold but this task's if `held`.
fn milky_way(held: bool) -> &'static Galaxy {
    static LISTED: OnceLock<Galaxy> = OnceLock::new();
    static HELD: OnceLock<Galaxy> = OnceLock::new();
    let build = |rules| {
        Galaxy::from_params_with_rules(
            Seed::new(0x1108_f000_0000_0002),
            GalaxyParams::milky_way_like(),
            rules,
        )
        .unwrap()
    };
    if held {
        HELD.get_or_init(|| {
            build(GalaxyRules {
                feature_binaries: FeatureBinariesRules::Unlisted,
                ..GalaxyRules::every_27_hold()
            })
        })
    } else {
        LISTED.get_or_init(|| build(GalaxyRules::every_27_hold()))
    }
}

/// A globular of the galaxy, whose ID and origin 47 Tucanae's members take, 47 Tucanae's model and
/// its class table.
fn tucanae(galaxy: &Galaxy) -> (FeatureRecord, ClusterModel, MemberClassTable) {
    let feature = FeatureCatalogue::walk_process(galaxy, FeatureProcess::Globular)
        .next()
        .expect("the fixture has globulars");
    let model = ClusterModel::new(galaxy, &catalogue_parameters(named_cluster(TUC_47)));
    let table = MemberClassTable::new(galaxy, &model, feature.reach(), [1.0, 0.0, 0.0]);
    (feature, model, table)
}

/// The ID of member `index` of `band` in a cell of `feature`.
fn member_id(feature: &FeatureRecord, band: MassBand, index: u32) -> FeatureMemberId {
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

/// A position `offset` light-years from `feature`.
fn at(feature: &FeatureRecord, offset: [f64; 3]) -> GalacticPosition {
    let [x, y, z] = feature.position().to_light_years_f64();
    GalacticPosition::from_light_years([x + offset[0], y + offset[1], z + offset[2]]).unwrap()
}

/// The first `n` members of `class` drawn under `shares` at a light-year from the centre.
fn members(
    galaxy: &Galaxy,
    (feature, model): (&FeatureRecord, &ClusterModel),
    class: &MemberClass,
    shares: RecycledShares,
    n: u32,
) -> Vec<MemberRecord> {
    let position = at(feature, [1.0, 0.0, 0.0]);
    (0..n)
        .filter_map(|i| {
            draw_member_marked(
                galaxy,
                feature,
                model,
                (class, shares),
                &position,
                member_id(feature, class.band, i),
                GalacticVelocity::new([0.0; 3]),
            )
        })
        .collect()
}

/// A neutron star's pulsar at the epoch, from its member's pair: its state and whether its pair
/// recycled it, and whether it is in a pair still.
fn pulsar(galaxy: &Galaxy, member: &MemberRecord) -> Option<(PulsarAt, bool)> {
    let stars = member.stars(galaxy);
    let draws = crate::stellar::system::primary_draws(galaxy, member.record());
    let age = member.record().age_at_epoch();
    stars.pairs().iter().find_map(|pair| {
        (pair.stars()[0] == StarIndex::PRIMARY).then(|| {
            pulsar_at(pair.timeline(), Component::Primary, &draws, age).map(|p| {
                let bound = matches!(
                    pair.timeline().state_at(age).kind(),
                    crate::stellar::binary::SegmentKind::Detached
                        | crate::stellar::binary::SegmentKind::StableTransfer { .. }
                        | crate::stellar::binary::SegmentKind::Contact
                );
                (p, bound)
            })
        })?
    })
}

/// The characteristic age of a pulsar in `state`, `P ÷ 2Ṗ`, Julian years.
fn characteristic_age(state: &crate::stellar::remnant::PulsarState) -> f64 {
    state.period().value()
        / (2.0 * state.period_derivative())
        / crate::units::consts::SECONDS_PER_JULIAN_YEAR
}

/// 47 Tucanae (P09.T9.e's reference cluster): its neutron stars that plan 09's encounter-rate law
/// marks millisecond pulsars are drawn conditionally, and every one is a recycled pulsar alive at
/// the epoch, in a binary or on its own, none of them young: its characteristic age is over 10⁸
/// years. Its unmarked neutron stars are no young pulsars either. Its lists' expected hosts of
/// each class and its marked members are printed for the record.
#[test]
fn forty_seven_tucanae_s_marked_pulsars_are_recycled_and_none_young() {
    let galaxy = milky_way(false);
    let (feature, model, table) = tucanae(galaxy);
    let shares = recycled_shares(galaxy, &model, &table);
    let pulsars = table.millisecond_pulsars(&model);
    let neutron_stars = table.expected_of(ClassKind::NeutronStar);
    assert!(
        (shares.pulsars() - pulsars / neutron_stars).abs() < 1e-12 && shares.pulsars() > 0.0,
        "{shares:?}"
    );
    let mut report = String::new();
    writeln!(
        report,
        "47 Tuc: {pulsars:.1} pulsars among {neutron_stars:.0} neutron stars (share {:.4}), \
         {:.1} blue stragglers (share {:.4}), Γ {:.3}",
        shares.pulsars(),
        model.blue_stragglers(),
        shares.blue_stragglers(),
        model.encounter_rate()
    )
    .unwrap();
    // The list's hosts of each class: its systems, its present mass over the halo's mean system
    // mass, at the mass formed per system and the class's rate, and the X-ray binaries'
    // encounter-rate law in their place.
    for class in [
        ClassId::ACCRETING_WHITE_DWARF,
        ClassId::ACCRETING_WHITE_DWARF_SLOW,
        ClassId::XRAY_BINARY,
        ClassId::STELLAR_MERGER,
        ClassId::NEUTRON_STAR_MERGER,
    ] {
        let expected = if class == ClassId::XRAY_BINARY {
            model.xray_binaries()
        } else {
            globular_hosts(galaxy, class, model.population(), model.mass())
        };
        writeln!(report, "  {class}: {expected:.4}").unwrap();
    }
    let mut marked = Vec::new();
    let mut unmarked = 0;
    for band in [MassBand::E] {
        for (class, _) in table.classes(band) {
            if class.kind != ClassKind::NeutronStar {
                continue;
            }
            for member in members(galaxy, (&feature, &model), class, shares, 1_200) {
                match member.recycled() {
                    RecycledMark::MillisecondPulsar => marked.push(member),
                    RecycledMark::Unmarked => {
                        unmarked += 1;
                        if unmarked % 25 == 0 {
                            // Its pair's pulsar, or a single star's own.
                            let state =
                                pulsar(galaxy, &member)
                                    .map(|(p, _)| *p.state())
                                    .or_else(|| {
                                        member.stars(galaxy).stars()[0]
                                            .pulsar_at(UniverseTime::EPOCH)
                                    });
                            let young = state.is_some_and(|s| {
                                s.is_radio_alive() && characteristic_age(&s) < 1e8
                            });
                            assert!(!young, "{member:?}");
                        }
                    }
                    RecycledMark::BlueStraggler => panic!("a neutron star marked a straggler"),
                }
            }
        }
    }
    let (mut binaries, mut isolated, mut millisecond) = (0, 0, 0);
    for member in &marked {
        assert_eq!(
            member.multiplicity_context(),
            MultiplicityContext::ForcedMultiple {
                max_separation: member.max_separation()
            }
        );
        let (p, bound) = pulsar(galaxy, member).expect("a marked pulsar is a neutron star's");
        assert!(p.recycled(), "{member:?}");
        assert!(p.state().is_radio_alive(), "{member:?}");
        let tau = characteristic_age(p.state());
        assert!(tau > 1e8, "{member:?}: τ {tau:e} yr");
        if bound {
            binaries += 1;
        } else {
            isolated += 1;
        }
        millisecond += u32::from(crate::stellar::binary::is_millisecond_pulsar(p.state()));
    }
    writeln!(
        report,
        "  {} marked of {} neutron stars drawn: {binaries} in binaries, {isolated} isolated, \
         {millisecond} under 30 ms",
        marked.len(),
        marked.len() + unmarked
    )
    .unwrap();
    eprint!("{report}");
    assert!(marked.len() >= 10, "{} marked", marked.len());
}

/// 47 Tucanae's binaries at its turn-off that plan 09 marks blue stragglers hold a main-sequence
/// star above the turn-off at the epoch, drawn conditionally.
#[test]
fn forty_seven_tucanae_s_marked_stragglers_lie_above_the_turn_off() {
    let galaxy = milky_way(false);
    let (feature, model, table) = tucanae(galaxy);
    let shares = recycled_shares(galaxy, &model, &table);
    assert!(shares.blue_stragglers() > 0.0, "{shares:?}");
    let classes: Vec<MemberClass> = MassBand::ALL
        .iter()
        .flat_map(|&band| table.classes(band))
        .filter(|(class, _)| is_at_the_turn_off(class))
        .map(|(class, _)| *class)
        .collect();
    assert_ne!(classes, Vec::new());
    let n = 400;
    let mut marked = 0;
    for class in &classes {
        for member in members(galaxy, (&feature, &model), class, shares, n) {
            if member.recycled() != RecycledMark::BlueStraggler {
                continue;
            }
            marked += 1;
            let turn_off = crate::stellar::sse::turn_off_mass(
                member.record().age_at_epoch(),
                member.composition(),
            );
            let state = member.stars(galaxy).state_at(UniverseTime::EPOCH).unwrap();
            assert!(
                state.stars().iter().any(
                    |s| s.phase() == crate::stellar::Phase::MainSequence && s.mass() > turn_off
                ),
                "{member:?}"
            );
        }
    }
    eprintln!("{marked} marked stragglers");
    assert!(marked > 3, "{marked}");
}

/// A binary member's orbits lie inside its cluster's hard–soft boundary at its radius, the
/// separation at which a pair of its class's system mass orbits at the cluster's dispersion;
/// under the held arm it takes none.
#[test]
fn a_binary_member_s_orbits_lie_inside_the_hard_soft_boundary() {
    let galaxy = milky_way(false);
    let (feature, model, table) = tucanae(galaxy);
    let class = *table
        .classes(MassBand::B)
        .find(|(c, _)| c.kind == ClassKind::Living && c.multiplicity == Multiplicity::Binary)
        .unwrap()
        .0;
    let boundary = hard_soft_separation(&model, LightYears::new(1.0), class.mean_mass);
    let speed = (GravitationalParameter::from_solar_masses(class.mean_mass).value()
        / boundary.value())
    .sqrt();
    let sigma = model.sigma(LightYears::new(1.0)).value() * 1e3;
    assert!((speed / sigma - 1.0).abs() < 1e-12, "{speed} {sigma}");
    let mut widest: f64 = 0.0;
    for member in members(
        galaxy,
        (&feature, &model),
        &class,
        RecycledShares::NONE,
        300,
    ) {
        let held_to = member
            .max_separation()
            .expect("a binary member is held")
            .value();
        assert!((held_to / boundary.value() - 1.0).abs() < 1e-9, "{held_to}");
        let hierarchy = draw_hierarchy_of_composition(
            galaxy,
            member.record(),
            member.composition(),
            member.multiplicity_context(),
            RedrawAttempt::FIRST,
        );
        for (_, orbit) in hierarchy.pairs() {
            let a = orbit.semi_major_axis().value();
            assert!(a <= held_to * (1.0 + 1e-12), "{a} beyond {held_to}");
            widest = widest.max(a);
        }
    }
    eprintln!(
        "boundary {:.0} au, widest {:.0} au",
        boundary.value() / 1.495_978_707e11,
        widest / 1.495_978_707e11
    );
    let held = milky_way(true);
    let (feature, model, _) = tucanae(held);
    for member in members(held, (&feature, &model), &class, RecycledShares::NONE, 20) {
        assert_eq!(member.max_separation(), None);
        assert_eq!(member.recycled(), RecycledMark::Unmarked);
    }
}

/// From version 27 an open cluster's living binaries are plan 11's multiple fraction inside its
/// hard–soft boundary, band by band; a globular's keep 5% and 1%; and the held arm keeps the
/// scratch 30%.
#[test]
fn an_open_cluster_s_binaries_are_its_hard_pairs() {
    let (on, off) = (milky_way(false), milky_way(true));
    let feature = FeatureCatalogue::walk_process(
        on,
        FeatureProcess::OldOpenCluster(crate::galaxy::ages::SubDisc::First),
    )
    .find_map(|f| ClusterModel::from_record(on, &f).map(|m| (f, m)))
    .expect("the fixture has old open clusters");
    let (feature, model) = feature;
    let share_of = |galaxy: &Galaxy, band: MassBand| {
        let table = MemberClassTable::new(galaxy, &model, feature.reach(), [1.0, 0.0, 0.0]);
        let living = |m: Multiplicity| -> f64 {
            table
                .classes(band)
                .filter(|(c, _)| c.kind == ClassKind::Living && c.multiplicity == m)
                .map(|(_, e)| e)
                .sum()
        };
        let (single, binary) = (living(Multiplicity::Single), living(Multiplicity::Binary));
        let mean = table
            .classes(band)
            .find(|(c, _)| c.kind == ClassKind::Living && c.multiplicity == Multiplicity::Single)
            .map(|(c, _)| c.mean_mass);
        (binary / (single + binary), mean)
    };
    let mut report = String::new();
    for band in [MassBand::A, MassBand::B, MassBand::C] {
        let (share, mean) = share_of(on, band);
        let mean = mean.expect("an old open cluster holds living stars in bands A–C");
        let model_share = open_cluster_binary_fraction(&model, mean);
        assert!(
            (share - model_share).abs() < 1e-9,
            "{band:?}: {share} {model_share}"
        );
        assert!((0.05..0.7).contains(&share), "{band:?}: {share}");
        let (held, _) = share_of(off, band);
        assert!((held - BINARY_FRACTIONS.2).abs() < 1e-9, "{band:?}: {held}");
        writeln!(
            report,
            "{band:?}: mean {:.3} M☉, binaries {share:.4}",
            mean.value()
        )
        .unwrap();
    }
    let separation = hard_soft_separation(
        &model,
        model.half_mass_radius(),
        SolarMasses::new(BINARY_MASS_FACTOR),
    );
    writeln!(
        report,
        "{:.0} M☉ at r_h {:.1} ly: σ {:.3} km/s, boundary {:.0} au for 1.5 M☉",
        model.mass().value(),
        model.half_mass_radius().value(),
        model.sigma(model.half_mass_radius()).value(),
        separation.value() / 1.495_978_707e11
    )
    .unwrap();
    eprint!("{report}");
    // A globular's are its populations' fixed shares under either arm.
    let (tuc_feature, tuc, _) = tucanae(on);
    let table = MemberClassTable::new(on, &tuc, tuc_feature.reach(), [1.0, 0.0, 0.0]);
    for (generation, want) in [
        (Generation::First, BINARY_FRACTIONS.0),
        (Generation::Second, BINARY_FRACTIONS.1),
    ] {
        let of = |m: Multiplicity| -> f64 {
            table
                .classes(MassBand::B)
                .filter(|(c, _)| {
                    c.kind == ClassKind::Living && c.multiplicity == m && c.generation == generation
                })
                .map(|(_, e)| e)
                .sum()
        };
        let share =
            of(Multiplicity::Binary) / (of(Multiplicity::Single) + of(Multiplicity::Binary));
        assert!((share - want).abs() < 1e-9, "{generation:?}: {share}");
    }
}

/// From version 27 a marked member's brief is its stars': plan 06's brief model takes its paired
/// route from the attempt its conditional draw kept, so that the range row and the system summary
/// read one system. Its draw is a pure function of its ID, in whichever order members are drawn.
#[test]
fn a_marked_member_s_brief_is_its_stars_and_its_draw_its_id_s() {
    use crate::stellar::brief::{BriefModel, BriefRoute};
    let galaxy = milky_way(false);
    let (feature, model, table) = tucanae(galaxy);
    let shares = recycled_shares(galaxy, &model, &table);
    let class = *table
        .classes(MassBand::E)
        .find(|(c, _)| c.kind == ClassKind::NeutronStar)
        .unwrap()
        .0;
    let marked: Vec<MemberRecord> = members(galaxy, (&feature, &model), &class, shares, 600)
        .into_iter()
        .filter(|m| m.marked_attempt().is_some())
        .take(4)
        .collect();
    assert_ne!(marked.len(), 0);
    for member in &marked {
        let brief = BriefModel::of_member(galaxy, member);
        let stars = member.stars(galaxy);
        assert_eq!(brief.route(), BriefRoute::Paired);
        assert_eq!(brief.star_count(), stars.star_count());
        assert_eq!(
            brief.brief_at(UniverseTime::EPOCH),
            stars.brief_at(UniverseTime::EPOCH)
        );
    }
    let position = at(&feature, [1.0, 0.0, 0.0]);
    let ids: Vec<FeatureMemberId> = (0..60)
        .map(|i| member_id(&feature, MassBand::E, i))
        .collect();
    assert_order_independent(&ids, |&id| {
        draw_member_marked(
            galaxy,
            &feature,
            &model,
            (&class, shares),
            &position,
            id,
            GalacticVelocity::new([0.0; 3]),
        )
    });
}
