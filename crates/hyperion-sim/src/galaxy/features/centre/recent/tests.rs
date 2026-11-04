//! The tests of the centre's recent classes (P09.T36.b) under every 26 hold.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use hyperion_testkit::golden::f64_digest;
use hyperion_testkit::lcg::Lcg;
use hyperion_testkit::order::assert_order_independent;

use super::*;
use crate::Seed;
use crate::galaxy::features::centre::members::resolve_centre_member_with;
use crate::galaxy::features::members::ListEntry;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::ResolveSystemError;
use crate::galaxy::rules::GalaxyRules;

/// The seed of the tests' Milky Way, the feature lists' (`members::recent`'s tests).
const SEED: u64 = 0x0935_0000_0000_0001;

/// The Milky Way at [`SEED`] under `rules`.
fn milky_way(rules: GalaxyRules) -> Galaxy {
    Galaxy::from_params_with_rules(Seed::new(SEED), GalaxyParams::milky_way_like(), rules).unwrap()
}

/// The Milky Way under every 26 hold with its retargets
/// (`GalaxyRules::every_26_hold_with_its_retargets`): the classes listed and the cells' claim.
fn listed() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| milky_way(GalaxyRules::every_26_hold_with_its_retargets()))
}

/// The same galaxy with features that list no class: the reference without the lists.
fn held() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| {
        milky_way(GalaxyRules {
            feature_classes: FeatureClassesRules::Unlisted,
            ..GalaxyRules::every_26_hold_with_its_retargets()
        })
    })
}

/// The ID of the centre's list entry `index`.
fn list_id(index: u16) -> CentreMemberId {
    CentreMemberId::new(MemberSlot::FeatureLevel { index }).unwrap()
}

/// Whether `component` is one of the centre's young classes'.
fn young(component: u8) -> bool {
    is_young(&age_components()[usize::from(component)])
}

/// A handful at the centre at Milky Way values (P09.T36.b): its list's classes start past its
/// victims, at N + 1 (30), in Design note 16's order, and the classes' tests keep 25 of its 567
/// candidates, past the plan's 1–20 (pinned as measured at generator version 26 with P09.T37's
/// draw over the reach, a finding: 25 core collapses, none of the 11.3 Type Ia expected and none
/// of the 23.7 variables'; 18 before the draw); each kept entry resolves to its member bit for bit,
/// with a site and a window for a death; a rejected candidate resolves to "no such system", every
/// index of them under the held arm holds no recent entry (plan 11's binary classes follow the
/// victims there from 27), and the victims stay.
#[test]
fn the_centre_lists_a_handful_past_its_victims() {
    let (off, on) = (held(), listed());
    let model = on.centre().unwrap();
    let victims = victim_count(on).unwrap();
    let recent = CentreRecent::new(on, model, victims);
    let list = recent.list(on);
    let entries: Vec<ListEntry> = list
        .entries()
        .take_while(|e| RecentClass::of(e.class()).is_some())
        .collect();
    assert_eq!(entries.first().map(ListEntry::index), Some(victims + 1));
    let order = [
        RecentClass::CoreCollapse,
        RecentClass::TypeIa,
        RecentClass::LuminousBlueVariable,
    ];
    let places: Vec<usize> = entries
        .iter()
        .map(|e| order.iter().position(|c| c.class() == e.class()).unwrap())
        .collect();
    assert!(places.windows(2).all(|w| w[0] <= w[1]));
    let (mut kept, mut rejected) = (BTreeMap::new(), 0_u32);
    for entry in &entries {
        let class = RecentClass::of(entry.class()).unwrap();
        let resolved = resolve_centre_member_with(on, model, list_id(entry.index()));
        if let Some(recent) = recent.entry(on, model, class, entry.index()) {
            assert_eq!(resolved, Ok(*recent.member()));
            assert_eq!(recent.class(), class);
            let death = class != RecentClass::LuminousBlueVariable;
            assert_eq!(recent.site().is_some(), death);
            assert_eq!(recent.window().is_some(), death);
            *kept.entry(class).or_insert(0_u32) += 1;
        } else {
            assert_eq!(resolved, Err(ResolveSystemError::NoSuchSystem));
            rejected += 1;
        }
    }
    let total: u32 = kept.values().sum();
    eprintln!(
        "{victims} victims, then {} candidates expecting {:?}: kept {kept:?}, {rejected} rejected",
        entries.len(),
        recent.expected()
    );
    // The plan's handful is 1–20; drawn over the reach (P09.T37's fix of the radius) the centre
    // keeps 25, a finding pinned as measured.
    assert_eq!(total, 25);
    let model_off = off.centre().unwrap();
    let recent_off = CentreRecent::new(off, model_off, victims);
    assert_eq!(recent_off.expected(), Vec::new());
    for entry in &entries {
        let class = RecentClass::of(entry.class()).unwrap();
        assert_eq!(recent_off.entry(off, model_off, class, entry.index()), None);
    }
    let victim = resolve_centre_member_with(off, model_off, list_id(victims)).unwrap();
    assert_eq!(
        resolve_centre_member_with(on, model, list_id(victims)),
        Ok(victim)
    );
}

/// The digest (`f64_digest`) of the centre's kept entries ([`the_centre_s_entries_are_pinned`]):
/// each one's ID (high, low 32 bits), class, initial mass, age, local position, velocity, death
/// and window, in index order. Blessed natively on x86-64; the wasip1 suite compares it. Pinned at
/// generator version 25 under every 26 hold (P09.T36's tests), and re-pinned at 26 with P09.T37's
/// entries drawn over the reach (it was `0x236e_004f_0fff_a27f` at 26 before).
const CENTRE_ENTRIES_DIGEST: u64 = 0x045b_0cd0_0f31_084f;

/// The centre's entries' bits, pinned before the bump, each the member its ID resolves to in any
/// order.
#[test]
fn the_centre_s_entries_are_pinned() {
    let on = listed();
    let model = on.centre().unwrap();
    let recent = CentreRecent::new(on, model, victim_count(on).unwrap());
    let list = recent.list(on);
    let indices: Vec<u16> = list
        .entries()
        .filter(|e| RecentClass::of(e.class()).is_some())
        .map(|e| e.index())
        .collect();
    assert_order_independent(&indices, |&index| {
        resolve_centre_member_with(on, model, list_id(index))
    });
    let mut values = Vec::new();
    for entry in list.entries() {
        // From 27 plan 11's binary classes follow the recent ones.
        let Some(class) = RecentClass::of(entry.class()) else {
            continue;
        };
        let Some(kept) = recent.entry(on, model, class, entry.index()) else {
            continue;
        };
        let member = kept.member();
        let raw = member.record().id().raw();
        values.push(f64::from(u32::try_from(raw >> 32).unwrap()));
        values.push(f64::from(u32::try_from(raw & 0xffff_ffff).unwrap()));
        values.push(match class {
            RecentClass::CoreCollapse => 0.0,
            RecentClass::TypeIa => 1.0,
            RecentClass::LuminousBlueVariable => 2.0,
        });
        values.push(member.record().primary_initial_mass().value());
        values.push(member.record().age_at_epoch().value());
        let at = member.local_position();
        values.extend([at.x, at.y, at.z]);
        values.extend(member.velocity_km_s());
        values.push(
            kept.site()
                .map_or(0.0, |s| s.death().since_epoch().as_seconds_f64()),
        );
        values.push(kept.window().map_or(-1.0, |w| w.duration().value()));
    }
    let digest = f64_digest(&values);
    eprintln!("{} values, digest {digest:#018x}", values.len());
    assert_eq!(digest, CENTRE_ENTRIES_DIGEST, "{digest:#018x}");
}

/// Both or neither (research-leans-10, item 4, condition 1): the centre's list holds its core
/// collapses and its luminous blue variables exactly where its band-E cells carve them, by the
/// one switch ([`FeatureClassesRules`]). With the lists every young class's cells claim and no old
/// class's, the two classes' candidates are the young classes' alone and the Type Ia the old
/// classes'; without them the list holds neither and no cell claims.
#[test]
fn the_centre_lists_its_variables_exactly_where_its_cells_carve_them() {
    for (galaxy, listing) in [(held(), false), (listed(), true)] {
        let model = galaxy.centre().unwrap();
        let recent = CentreRecent::new(galaxy, model, victim_count(galaxy).unwrap());
        for component in (0_u8..).take(age_components().len()) {
            assert_eq!(
                centre_cells_claim(galaxy, component),
                listing && young(component),
                "{component}"
            );
        }
        for (class, parts) in &recent.classes {
            for part in parts {
                let of_young = young(part.component);
                assert_eq!(
                    of_young,
                    *class != RecentClass::TypeIa,
                    "{class:?} {part:?}"
                );
                assert_eq!(part.candidates.is_some(), of_young);
            }
        }
        let holds = |class| recent.classes.iter().any(|(c, _)| *c == class);
        assert_eq!(holds(RecentClass::CoreCollapse), listing);
        assert_eq!(holds(RecentClass::LuminousBlueVariable), listing);
    }
}

/// P09.T38's complementarity at the centre, over 10⁴ mark sets of its young classes' band-E
/// members, each drawn as the list draws an entry (its component by the classes' band-E shares, its
/// position and velocity from its tracer) but with its mass uniform in ln m over band E and its age
/// uniform over its component's: the cells give up a member ([`list_claims`]) exactly where
/// [`claimed_at_centre`] claims it, and then its mass and age lie in its component's candidates of
/// that class, so that the list draws it; without the lists none is claimed. The cells themselves
/// are not walked: at version 24 one of the young burst's neutron-star classes exhausts its
/// attempts there (`draw_centre_member`'s debug assertion), a finding of its own.
#[test]
#[ignore = "slow: 10⁴ of the centre's young mark sets under either arm"]
fn the_centre_s_cells_and_its_list_are_complementary() {
    complementary_at_the_centre(held(), listed());
}

/// P09.T38: [`the_centre_s_cells_and_its_list_are_complementary`] under every 27 hold, galaxy and
/// stellar, against every 27 hold but this one's.
#[test]
#[ignore = "slow: 10⁴ of the centre's young mark sets under either arm, every 27 hold"]
fn the_centre_s_cells_and_its_list_are_complementary_under_every_27_hold() {
    let on = milky_way(GalaxyRules::every_27_hold());
    let off = milky_way(GalaxyRules {
        feature_classes: FeatureClassesRules::Unlisted,
        ..GalaxyRules::every_27_hold()
    });
    complementary_at_the_centre(&off, &on);
}

/// The centre's complementarity of [`the_centre_s_cells_and_its_list_are_complementary`], in `on`
/// against `off`, the reference without the lists.
fn complementary_at_the_centre(off: &Galaxy, on: &Galaxy) {
    const SETS: u16 = 10_000;
    let model = on.centre().unwrap();
    let recent = CentreRecent::new(on, model, victim_count(on).unwrap());
    let young: Vec<u8> = (0_u8..)
        .take(age_components().len())
        .filter(|&c| young(c))
        .collect();
    let mut lcg = Lcg::new(0x0936_b000);
    let (lo, hi) = (MassBand::E.lo(), MassBand::E.hi());
    let mut claims = BTreeMap::new();
    for k in 0..SETS {
        let component =
            young[usize::try_from(lcg.next_below(u64::try_from(young.len()).unwrap())).unwrap()];
        let ages = age_components()[usize::from(component)];
        let class = centre_class(model, component).unwrap();
        let index = 30_000 + k;
        let system = SystemId::from(list_id(index));
        let key = ObjectKey::from(system);
        let at = position(model, &ages, key, on.seed());
        let mut stream = Stream::open(on.seed(), tags::CENTRE_VELOCITY, key);
        let Some(velocity) =
            model
                .distribution(class.tracer)
                .draw_velocity(model.profile(), at, &mut stream)
        else {
            continue;
        };
        let mass = lo * math::exp(math::ln(hi / lo) * lcg.next_f64());
        let (youngest, oldest) = (ages.youngest.value().max(0.0), ages.oldest.value());
        let age = oldest - (oldest - youngest) * lcg.next_f64();
        let member = CentreMemberRecord::from_parts(
            record(
                system,
                galactic(at),
                SolarMasses::new(mass),
                Years::new(age),
            ),
            class,
            centre_composition(),
            velocity,
        );
        let claim = claimed_at_centre(on, member.record(), member.composition(), member.velocity());
        assert_eq!(list_claims(on, &member), claim.is_some(), "{system:?}");
        assert!(!list_claims(off, &member), "{system:?}");
        if let Some(class) = claim {
            let parts = &recent.classes.iter().find(|(c, _)| *c == class).unwrap().1;
            let covered = parts.iter().any(|p| {
                p.component == component
                    && p.candidates
                        .as_ref()
                        .is_some_and(|c| c.contains(SolarMasses::new(mass), Years::new(age)))
            });
            assert!(
                covered,
                "{class:?} claims {system:?}, outside its candidates"
            );
        }
        *claims.entry((component, claim)).or_insert(0_u32) += 1;
    }
    eprintln!("the centre's young mark sets by component and claim: {claims:?}");
    for class in [RecentClass::CoreCollapse, RecentClass::LuminousBlueVariable] {
        assert!(
            claims.keys().any(|&(_, claim)| claim == Some(class)),
            "{class:?}"
        );
    }
}

/// P09.T37's fix: the centre's candidates are placed over its reach on their tracers' profiles,
/// not below the distribution function's inner cut (10⁻³ ly, 0.1 ly for the young disc), where
/// every one lay before; each place is the one `position` draws on a profile built afresh, bit for
/// bit, and every kept entry is its place's.
#[test]
fn the_centre_s_candidates_are_placed_over_its_reach() {
    let on = listed();
    let model = on.centre().unwrap();
    let recent = CentreRecent::new(on, model, victim_count(on).unwrap());
    let places = recent.places(on, model);
    let mut radii: Vec<f64> = places
        .iter()
        .map(|p| LightYears::from(GalacticPosition::ORIGIN.distance_to(p.position())).value())
        .collect();
    radii.sort_by(f64::total_cmp);
    let median = radii[radii.len() / 2];
    eprintln!(
        "{} places from {:.4} to {:.1} ly, median {median:.2} ly",
        radii.len(),
        radii[0],
        radii[radii.len() - 1]
    );
    assert!(
        radii.iter().all(|&r| r <= REACH.value()),
        "inside the reach"
    );
    assert!(
        median > 1.0,
        "the median place at {median} ly, past the old cut"
    );
    for place in places.iter().take(16) {
        let key = ObjectKey::from(place.system);
        let fresh = galactic(position(model, &place.component, key, on.seed()));
        assert_eq!(&fresh, place.position(), "{:?}", place.system);
    }
    for place in &places {
        let crate::id::SystemIdKind::Centre(id) = place.system.kind() else {
            unreachable!("a centre ID");
        };
        let MemberSlot::FeatureLevel { index } = id.slot() else {
            unreachable!("a list entry");
        };
        assert_eq!(
            place.kept(on),
            recent.entry(on, model, place.class(), index),
            "{index}"
        );
    }
}
