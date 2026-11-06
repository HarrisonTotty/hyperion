//! The tests of P11.T4's neutron-star accretion fix: a neutron star fed by stable Roche-lobe
//! overflow keeps [`NS_ACCRETION_EFFICIENCY`] of it, at one choke point, `Engine::accretion`. Its
//! hold is deleted (the 27 fold); these are its tests of the new arm, which the bump re-pins by
//! name.
//!
//! - A pinned pair, a 1.5 M☉ main-sequence donor at its lobe beside a 1.35 M☉ neutron star: under
//!   every 27 hold it keeps under a tenth and stays a neutron star.
//! - The same pair twice gives the same timeline.
//! - Slow: a census of 10⁷ slab records under the version's rules, which the white-dwarf union's
//!   29 folds moved (P11.T4's swollen dwarf, dwarf limit and kept-core endings), and one under
//!   every 30 hold, which P11.T4's `co_chandrasekhar` moves.

use hyperion_testkit::pins::Tolerance;

use super::*;
use crate::GENERATOR_VERSION;
use crate::galaxy::Galaxy;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::rules::GalaxyRules;
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

/// Under every 27 hold the pinned pair's neutron star keeps less than [`NS_ACCRETION_EFFICIENCY`]
/// of what the donor gives (its wind gives none on the main sequence): the donor, the heavier,
/// transfers at its thermal rate at first (`heavy_donor`), over the star's Eddington rate, which
/// then cuts its share. It is a neutron star 12 Gyr on, having gained less than [`MOST_GAINED`].
/// (Under the version's rules with this fix alone it kept a tenth to a part in 10⁶; under the held
/// arm it took the transfer whole and became a black hole.)
#[test]
fn the_pinned_neutron_star_keeps_under_a_tenth() {
    let (donor, _, star) = PINNED;
    let timeline = pinned(StellarRules::of_version(crate::GENERATOR_VERSION));
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
    // The neutron star at the end of the donor's first transfer, and its compact star's mass 12 Gyr
    // on, M☉, as measured.
    let mut pins = hyperion_testkit::pins!(
        "stellar/rules/ns_accretion_hold",
        "the_pinned_neutron_star_keeps_under_a_tenth"
    );
    let figures = (a.mass().value(), compact.mass().value());
    pins.within("figures", figures, Tolerance::absolute(1e-12));
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

/// The census's figures at generator version 27, as measured (2026-11-05), against the 413
/// members and 402 neutron stars pinned at 25 and 26.
///
/// By cause, each step's members (neutron stars) on train 27's own trees, in the order they
/// landed:
///
/// - the pin's tree is not in the clone this was measured in: train 27's tree before its folds,
///   without the pinned white dwarfs, the wide pair's wind, the dwarf limit and the `radx` fix
///   (the last two steps came after it), gives 251 (242), −162 (−160), left unsplit: the 27
///   arms' revisions between the two trees are not held apart on any tree there;
/// - P11.T4's pinned white dwarfs, +8 (+8);
/// - P11.T12's wind onto a wide pair's star, 0;
/// - P11.T4's dwarf limit, +129 (+129): carbon–oxygen dwarfs fed helium past the edge-lit mark,
///   which the held arm let grow on, now collapse at the Chandrasekhar mass (a pooled Type Ia
///   candidate, then the accretion-induced collapse) to 1.26 M☉ neutron stars beside their
///   donors, now helium dwarfs ground to about 0.01 M☉ at orbital periods of about an hour. Its
///   lane's 399 → 1,190, on an older tree, counted the swollen helium dwarf too, which moves this
///   census by nothing on this tree;
/// - P11.T10's kicks at version 25, the heavy donors' corotation at the donor's radius held to its
///   lobe (BSE's `radx`), +1,350 (+1,343): neutron stars that a common envelope left beside a
///   2.8–5.9 M☉ main-sequence star (its initial mass, 10–90%), whose thermal-rate transfer no
///   longer drains their orbit to a merger through the overfilling donor's spin, and which live
///   on as low-mass X-ray binaries (the intermediate-mass channel; Tauris, van den Heuvel and
///   Savonije 2000, ApJ 530, L93; Pfahl, Rappaport and Podsiadlowski 2003, ApJ 597, 1036;
///   recalled). Its lane's re-pin, 411 → 1,854, was lost at its landing;
/// - P11.T4's heavy donors' white dwarfs' wind, +77 (+77);
/// - the accreting white dwarfs' samplers refitted under the Chandrasekhar mass (P15.T10.b), +1
///   (+1): a record the carve-out no longer takes;
/// - the 27 folds, plan 08's displaced records among them, the bump and the segment cap of 256, 0;
/// - the 29 folds, 1,816 (1,800) → 1,834 (1,818), +18 (+18), measured under every 29 hold
///   (2026-11-05, at generator version 28) by hold, each added in turn: the galaxy's holds at 29
///   (P09.T36's alert walks' lists and P09.T5's superbubbles), 0; P11.T4's swollen helium dwarf,
///   0; P11.T4's dwarf limit made whole, 0; P11.T4's kept-core endings and the
///   helium stars' electron capture, +18 (+18), not split further. The black holes, the grown, the
///   median black hole and the most gained stay 28's.
const CENSUS_FIGURES: CompactFigures = CompactFigures {
    members: 1_834,
    black_holes: 12,
    grown: 2,
    grown_by_dwarfs: 1,
    median_black_hole: 3.053_147_766_835_219,
    neutron_stars: 1_818,
    most_gained: 0.175_378_456_071_606_3,
};

/// A census of 10⁷ slab records' X-ray binaries ([`slab_census`]) under the version's rules. Under
/// the held arm, which kept a neutron star's whole transfer, most of the black holes were born neutron
/// stars that stable transfer grew past `MAX_NEUTRON_STAR_MASS`. Under every 27 hold:
///
/// - none of the black holes was born a neutron star at generator version 24, against the brief's
///   5%: the carving's floor (`xrb_class::XRB_MIN_TRANSFER_RATE`, 10⁻¹² M☉ yr⁻¹) takes the relics of
///   a donor ground down, fed by white dwarfs at 1.6 × 10⁻¹³ to 3 × 10⁻¹¹ M☉ yr⁻¹, that its first
///   floor of 10⁻¹³ left. At generator version 25 before P11.T4's heavy donors one was a neutron
///   star born at 1.861 M☉ that a 1.46 M☉ main-sequence donor fed for 7 Gyr, grinding itself down
///   to 0.068 M☉: 0.139 M☉ gained at the arm's tenth carries it past `MAX_NEUTRON_STAR_MASS` (a
///   lean of land-pod's, 2026-11-01, for research); at 26 there is none; at 27 two of 12 (a
///   finding): neutron stars born at 1.85–1.86 M☉ of 13.3 M☉ primaries (initial masses here),
///   carried past `MAX_NEUTRON_STAR_MASS` by 0.2–0.26 M☉ of a 4.8–6.3 M☉ donor's transfer, one
///   donor now substellar;
/// - no neutron star in a low-mass X-ray binary has gained more than [`MOST_GAINED`];
/// - the black holes' median mass in the low-mass X-ray binaries is pinned as measured, under the
///   brief's 5 M☉ (a finding): most are fallback black holes of 2–4 M☉ (plan 06's Mandel and
///   Müller law, 0.8 `M_CO` ± 0.5 M☉ held above 2 M☉), not neutron stars grown.
#[test]
#[ignore = "slow: 10⁷ grid records' pairs run through the binary engine"]
fn census_of_the_neutron_star_accretion() {
    let (scanned, f) = census(GalaxyRules::of_version(crate::GENERATOR_VERSION));
    println!("the version's rules: {scanned:?} records by band, {f:?}");
    assert_the_figures(f, CENSUS_FIGURES);
}

/// The census of 10⁷ slab records' X-ray binaries under `rules`: the records scanned by band and
/// the figures.
fn census(rules: GalaxyRules) -> ([u64; 5], CompactFigures) {
    let galaxy = Galaxy::from_params_with_rules(
        Seed::new(CENSUS_SEED),
        GalaxyParams::milky_way_like(),
        rules,
    )
    .expect("the Milky Way fixture's gas is mostly neutral");
    let census = slab_census(&galaxy, CENSUS_RECORDS, &[CarvedClass::XrayBinary]);
    (census.scanned, figures(&census))
}

/// No neutron star in a low-mass X-ray binary of `f` has gained more than [`MOST_GAINED`], and
/// `f` is `pinned`.
fn assert_the_figures(f: CompactFigures, pinned: CompactFigures) {
    assert!(f.most_gained <= MOST_GAINED, "{f:?}");
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

/// The census's figures under every 30 hold, as measured (2026-11-05, at generator version 28
/// under every 29 hold; 432 s, pod, 16 vCPU), against [`CENSUS_FIGURES`]: 1,834 → 1,658 members
/// (1,818 → 1,642 neutron stars), −176, P11.T4's carbon–oxygen dwarfs at the Chandrasekhar mass
/// (`co_chandrasekhar`), which disrupt and leave nothing where under the version's rules they
/// collapse into the 1.26 M☉ neutron stars of accretion-induced collapses beside their donors;
/// P11.T4's `merged_capture`, added to every 30 hold on 2026-11-06, moves none of them (measured
/// again then, 380 s). The black holes, the grown, the median black hole and the most gained stay
/// the version's.
const CENSUS_FIGURES_30: CompactFigures = CompactFigures {
    members: 1_658,
    black_holes: 12,
    grown: 2,
    grown_by_dwarfs: 1,
    median_black_hole: 3.053_147_766_835_219,
    neutron_stars: 1_642,
    most_gained: 0.175_378_456_071_606_3,
};

/// The census of 10⁷ slab records' X-ray binaries ([`slab_census`]) under every 30 hold: the
/// figures of [`census_of_the_neutron_star_accretion`], pinned as measured
/// ([`CENSUS_FIGURES_30`]).
#[test]
#[ignore = "slow: 10⁷ grid records' pairs run through the binary engine"]
fn census_of_the_neutron_star_accretion_under_every_30_hold() {
    let (scanned, f) = census(GalaxyRules::every_30_hold());
    println!("every 30 hold: {scanned:?} records by band, {f:?}");
    assert_the_figures(f, CENSUS_FIGURES_30);
}
