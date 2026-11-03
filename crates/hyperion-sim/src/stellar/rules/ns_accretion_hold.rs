//! The tests of P11.T4's neutron-star accretion fix: a neutron star fed by stable Roche-lobe
//! overflow keeps [`NS_ACCRETION_EFFICIENCY`] of it, at one choke point, `Engine::accretion`. Its
//! hold is deleted (the 27 fold); these are its tests of the new arm, which the bump re-pins by
//! name.
//!
//! - A pinned pair, a 1.5 M☉ main-sequence donor at its lobe beside a 1.35 M☉ neutron star: under
//!   every 27 hold it keeps under a tenth and stays a neutron star.
//! - The same pair twice gives the same timeline.
//! - Slow: a census of 10⁷ slab records under every 27 hold.

use super::*;
use crate::GENERATOR_VERSION;
use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::rules::GalaxyRules;
use crate::planetary::testing::with_every_27_hold;
use crate::rng::Seed;
use crate::stellar::Phase;
use crate::stellar::binary::ns_accretion::NS_ACCRETION_EFFICIENCY;
use crate::stellar::binary::testing::census::{Census, CensusMember, slab_census};
use crate::stellar::binary::testing::remnant_pair::{ONSET_AGE, donor_onto_remnant};
use crate::stellar::binary::{BinaryClass, BinaryTimeline, CarvedClass, Component, SegmentKind};
use crate::stellar::remnant::RemnantKind;
use crate::units::Years;

/// The census's Milky Way seed: the binary fits' (P15.T10.b's manifests).
const CENSUS_SEED: u64 = 0x0b17_15a0_0000_0001;

/// The records of the slow census: the fit's slab's 10⁷, since 10⁶ hold about one black hole in
/// an X-ray binary that was born one under every 27 hold.
const CENSUS_RECORDS: u64 = 10_000_000;

/// The pinned pair: a solar 1.5 M☉ main-sequence donor at τ = 0.3 filling its lobe beside a
/// 1.35 M☉ neutron star.
const PINNED: (f64, f64, f64) = (1.5, 0.3, 1.35);

/// The most a neutron star in a low-mass X-ray binary has gained, M☉: 0.2, the top of the
/// 0.1–0.2 M☉ by which the millisecond pulsars' masses exceed the slow pulsars' (Antoniadis et al.
/// 2016, arXiv:1605.01665; recalled, not read at source; [`NS_ACCRETION_EFFICIENCY`]).
const MOST_GAINED: f64 = 0.2;

/// The version's rules.
fn version() -> StellarRules {
    StellarRules::of_version(GENERATOR_VERSION)
}

/// The pinned pair's timeline under `rules`, to 12 Gyr after its onset.
fn pinned(rules: StellarRules) -> BinaryTimeline {
    let (donor, tau, star) = PINNED;
    donor_onto_remnant(donor, tau, Phase::NeutronStar, star, rules, 1.2e10).0
}

/// The pinned pair's neutron star at the end of the donor's first transfer, and its compact star's
/// mass 12 Gyr on, M☉, under every 27 hold, as measured (2026-11-02, once P11.T4's heavy-donor
/// hold took the overfilling donor's spin at its lobe).
const PINNED_FIGURES: (f64, f64) = (1.392_403_839_204_236_6, 1.474_077_743_736_024_2);

/// Under every 27 hold the pinned pair's neutron star keeps less than [`NS_ACCRETION_EFFICIENCY`]
/// of what the donor gives (its wind gives none on the main sequence): the donor, the heavier,
/// transfers at its thermal rate at first (`heavy_donor`), over the star's Eddington rate, which
/// then cuts its share. It is a neutron star 12 Gyr on, having gained less than [`MOST_GAINED`].
/// (Under the version's rules with this fix alone it kept a tenth to a part in 10⁶; under the held
/// arm it took the transfer whole and became a black hole.)
#[test]
fn the_pinned_neutron_star_keeps_under_a_tenth() {
    let (donor, _, star) = PINNED;
    let timeline = pinned(StellarRules::every_27_hold());
    let first = timeline
        .segments()
        .iter()
        .find(|s| s.start().value() >= ONSET_AGE)
        .expect("the transfer from the onset");
    assert_eq!(
        first.kind(),
        SegmentKind::StableTransfer {
            donor: Component::Primary
        }
    );
    let end = timeline.state_at(Years::new(first.end().value() * (1.0 - 1e-12)));
    let [d, a] = end.stars();
    let (given, gained) = (donor - d.mass().value(), a.mass().value() - star);
    let last = timeline.state_at(Years::new(ONSET_AGE + 1.2e10));
    let compact = last.stars()[1];
    println!(
        "the donor gave {given:.6} M_sun by {:.4e} yr, the star gained {gained:.6} ({:.6} of it); \
         12 Gyr on a {:?} of {:.6} M_sun",
        first.end().value(),
        gained / given,
        compact.phase(),
        compact.mass().value()
    );
    assert!(
        gained / given / NS_ACCRETION_EFFICIENCY < 1.0,
        "{gained} of {given}"
    );
    assert_eq!(compact.phase(), Phase::NeutronStar);
    assert!(compact.mass().value() - star < MOST_GAINED);
    let m = (a.mass().value(), compact.mass().value());
    assert!(
        (m.0 - PINNED_FIGURES.0).abs() < 1e-12 && (m.1 - PINNED_FIGURES.1).abs() < 1e-12,
        "{m:?}"
    );
}

/// The same pair twice gives the same timeline.
#[test]
fn the_pinned_pair_is_deterministic() {
    assert_eq!(pinned(version()), pinned(version()));
}

/// What a census says of its X-ray binaries' compact stars.
#[derive(Debug, Clone, Copy, PartialEq)]
struct CompactFigures {
    /// The X-ray binaries.
    members: usize,
    /// Those whose compact star is a black hole.
    black_holes: usize,
    /// Of those, the black holes born neutron stars.
    grown: usize,
    /// Of those, the ones fed by a white dwarf or a substellar object: the relics of a donor
    /// ground down.
    grown_by_dwarfs: usize,
    /// The median mass of the black holes in low-mass X-ray binaries, M☉.
    median_black_hole: f64,
    /// The neutron stars in low-mass X-ray binaries.
    neutron_stars: usize,
    /// The most any of them has gained since its birth, M☉.
    most_gained: f64,
}

/// Whether `member`'s compact star is a black hole born a neutron star.
fn grown(member: &CensusMember) -> bool {
    matches!(member.compact_birth(), Some((RemnantKind::NeutronStar, _)))
}

/// `census`'s figures.
fn figures(census: &Census) -> CompactFigures {
    let marks = |m: &CensusMember| m.xray.expect("an X-ray binary's marks");
    let xray: Vec<&CensusMember> = census.members.iter().filter(|m| m.xray.is_some()).collect();
    let low_mass = |m: &CensusMember| matches!(marks(m).class, BinaryClass::LowMassXrayBinary(_));
    let by_dwarf = |m: &CensusMember| {
        matches!(
            marks(m).donor_phase,
            Phase::HeliumWhiteDwarf
                | Phase::CarbonOxygenWhiteDwarf
                | Phase::OxygenNeonWhiteDwarf
                | Phase::Substellar
        )
    };
    let black_holes: Vec<&CensusMember> = xray
        .iter()
        .copied()
        .filter(|m| marks(m).accretor_phase == Phase::BlackHole)
        .collect();
    let mut masses: Vec<f64> = black_holes
        .iter()
        .filter(|m| low_mass(m))
        .map(|m| marks(m).accretor_mass.value())
        .collect();
    masses.sort_by(f64::total_cmp);
    let gains: Vec<f64> = xray
        .iter()
        .filter(|m| low_mass(m) && marks(m).accretor_phase == Phase::NeutronStar)
        .filter_map(|m| {
            let (_, birth) = m.compact_birth()?;
            Some(marks(m).accretor_mass.value() - birth.value())
        })
        .collect();
    CompactFigures {
        members: xray.len(),
        black_holes: black_holes.len(),
        grown: black_holes.iter().filter(|m| grown(m)).count(),
        grown_by_dwarfs: black_holes
            .iter()
            .filter(|m| grown(m) && by_dwarf(m))
            .count(),
        median_black_hole: masses.get(masses.len() / 2).copied().unwrap_or(f64::NAN),
        neutron_stars: gains.len(),
        most_gained: gains.iter().copied().fold(0.0, f64::max),
    }
}

/// The census's figures under every 27 hold, as measured (2026-11-01, at generator version 25,
/// and again at 26).
const CENSUS_FIGURES: CompactFigures = CompactFigures {
    members: 413,
    black_holes: 11,
    grown: 0,
    grown_by_dwarfs: 0,
    median_black_hole: 3.514_700_978_122_044,
    neutron_stars: 402,
    most_gained: 0.166_682_517_111_517_38,
};

/// A census of 10⁷ slab records' X-ray binaries ([`slab_census`]) under every 27 hold. Under the
/// held arm, which kept a neutron star's whole transfer, most of the black holes were born neutron
/// stars that stable transfer grew past `MAX_NEUTRON_STAR_MASS`. Under every 27 hold:
///
/// - none of the black holes was born a neutron star at generator version 24, against the brief's
///   5%: the carving's floor (`xrb_class::XRB_MIN_TRANSFER_RATE`, 10⁻¹² M☉ yr⁻¹) takes the relics of
///   a donor ground down, fed by white dwarfs at 1.6 × 10⁻¹³ to 3 × 10⁻¹¹ M☉ yr⁻¹, that its first
///   floor of 10⁻¹³ left. At generator version 25 before P11.T4's heavy donors one was a neutron
///   star born at 1.861 M☉ that a 1.46 M☉ main-sequence donor fed for 7 Gyr, grinding itself down
///   to 0.068 M☉: 0.139 M☉ gained at the arm's tenth carries it past `MAX_NEUTRON_STAR_MASS` (a
///   lean of land-pod's, 2026-11-01, for research); at 26 there is none;
/// - no neutron star in a low-mass X-ray binary has gained more than [`MOST_GAINED`];
/// - the black holes' median mass in the low-mass X-ray binaries is pinned as measured, under the
///   brief's 5 M☉ (a finding): most are fallback black holes of 2–4 M☉ (plan 06's Mandel and
///   Müller law, 0.8 `M_CO` ± 0.5 M☉ held above 2 M☉), not neutron stars grown.
#[test]
#[ignore = "slow: 2 × 10⁷ grid records' pairs run through the binary engine"]
fn census_of_the_neutron_star_accretion() {
    let census = |rules: GalaxyRules| {
        let galaxy = Galaxy::from_params_with_rules(
            Seed::new(CENSUS_SEED),
            GalaxyParams::milky_way_like(),
            rules,
        )
        .expect("the Milky Way fixture's gas is mostly neutral");
        let census = slab_census(&galaxy, CENSUS_RECORDS, &[CarvedClass::XrayBinary]);
        (census.scanned, figures(&census))
    };
    let (scanned, f) = with_every_27_hold(|| census(GalaxyRules::every_27_hold()));
    println!("every 27 hold: {scanned:?} records by band, {f:?}");
    assert!(f.most_gained <= MOST_GAINED, "{f:?}");
    let pinned = CENSUS_FIGURES;
    assert_eq!(
        (
            f.members,
            f.black_holes,
            f.grown,
            f.grown_by_dwarfs,
            f.neutron_stars
        ),
        (
            pinned.members,
            pinned.black_holes,
            pinned.grown,
            pinned.grown_by_dwarfs,
            pinned.neutron_stars
        ),
        "{f:?}"
    );
    assert!(
        (f.median_black_hole - pinned.median_black_hole).abs() < 1e-12
            && (f.most_gained - pinned.most_gained).abs() < 1e-12,
        "{f:?}"
    );
}
