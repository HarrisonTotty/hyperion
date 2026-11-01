//! A cluster's tabulated remnants: what decides a remnant class's member attempt without its
//! star's track, from version 26 (plan 09, P09.T10; the hold of [`TABULATED_REMNANTS_VERSION`]).
//!
//! A remnant class's draw ([`draw_member`](super::draw_member)) keeps an attempt whose star leaves
//! a remnant of the class's kind kicked below the cluster's effective escape speed, and the star
//! the record later builds is the attempt's ([`remnant_fits`](super::remnant_fits)). Each attempt
//! costs a lifetime build, about 0.55 ms, so a globular core's 50 ly query, some 6,500
//! white-dwarf and 2,400 neutron-star attempts, takes 6.2 s under 26's rules held but this one;
//! with the table, 2.5 s, and 1.0 s once the stripping table's helium axis makes a helium-rich
//! star's share a lookup (a pod of 16 vCPU, release). Under
//! [`MemberRemnantRules::Tabulated`] the draw asks [`tabulated_fit`] first, and only an attempt it
//! cannot decide builds its star:
//!
//! - **A white dwarf** is kept below the cluster's white-dwarf ceiling, if its kick, which the
//!   star's draws alone set ([`StandardKickLaw::white_dwarf_kick`]), is below the escape speed.
//!   The ceiling lies [`CEILING_MARGIN`] below the heaviest star found to leave one under the
//!   lightest that leaves anything else ([`white_dwarf_edge`]), by exact fates at the corners of
//!   the members' compositions and at the draws that keep the most envelope, so a member kept
//!   below it is the star its record builds. Above it the star's companion-stripped mark, read as
//!   its track reads it, says which capture window it can open: an unstripped star is kept below
//!   a ceiling [`UNSTRIPPED_CEILING_MARGIN`] under that edge, and any star is refused
//!   [`WHITE_DWARF_TOP_MARGIN`] above the start of its window, `m_cc` less the window's width,
//!   where no corner's exact fate is a white dwarf.
//! - **A neutron star** is refused when the ordinary mode's slowest kick, the rank clamp's floor
//!   (about 33 km/s), is at or above the escape speed, in a cluster old enough that its star has
//!   surely died, and a low-mode kick, judged on its pair's systemic speed, either is too fast or
//!   cannot be the star's: its pair is at or above the escape speed, or the star is unstripped
//!   (its companion-stripped mark read against the share, a lookup at 26 with the stripping
//!   table's helium axis) and above the cluster's capture window, where only a stripped star's
//!   iron core takes the low mode, or the star is stripped and at or above the cluster's
//!   carbon–oxygen core floor ([`co_core_floor`]), from which no iron core takes it. Below the
//!   floor a stripped star's carbon–oxygen core lies between its early AGB's at its track mass and
//!   at that less the cluster's main-sequence loss ([`main_sequence_loss`]), and the attempt is
//!   kept or refused where its mode's and its remnant's marks decide the whole bracket alike
//!   ([`low_mode_fit`]).
//!
//! Refusing an attempt that the star's track would keep moves the draw on to the next one, so the
//! member is still the star its record builds; only a kept white dwarf leans on the ceiling. The
//! window's top allows the main sequence's wind, which lowers the mass the window is tested in, by
//! a margin, so a refusal the track would keep is rare, and the agreement test counts them. A
//! black hole, and every attempt these leave open, takes the track.
//!
//! The ceilings, the highest `m_cc` and the capture window's top are a cluster's [`RemnantTable`],
//! found once a model ([`ClusterModel::remnant_table`]); the carbon–oxygen core's floor is found
//! once a model too, only when asked ([`ClusterModel::co_core_floor`]).

use std::sync::OnceLock;

use crate::GeneratorVersion;
use crate::galaxy::Galaxy;
use crate::galaxy::displaced::binarity::{self, NEVER_STRIPPED};
use crate::galaxy::features::cluster::ClusterModel;
use crate::galaxy::features::interior::ClassKind;
use crate::galaxy::features::interior::abundances::{
    IRON_SPREAD_SHARE, IRON_SPREAD_SIGMA, helium_excess_max,
};
use crate::galaxy::features::interior::counts::WHITE_DWARF_PROGENITOR_MAX;
use crate::id::{BodyId, SystemId};
use crate::rng::Threshold;
use crate::stellar::Composition;
use crate::stellar::draws::{StandardNormal, StarDraws, StarDrawsParts};
use crate::stellar::remnant::collapse::{
    COMPANION_STRIPPED_WINDOW, CoreCollapse, ElectronCaptureWindows, RemnantDraws,
    SINGLE_STAR_WINDOW, core_collapse,
};
use crate::stellar::remnant::{DeathKind, RemnantKind, StandardKickLaw};
use crate::stellar::rules::StellarRules;
use crate::stellar::sse::{self, TrackOptions, ZCoeffs};
use crate::stellar::system::track_mass_of;
use crate::units::{Dex, HeliumExcess, KilometresPerSecond, SolarMasses, Years};

/// The generator version whose member draws read a cluster's tabulated remnants
/// ([`MemberRemnantRules::Tabulated`]). Members are first served at 26, so the arm moves nothing
/// served before it. The 25 → 26 bump deletes this constant, [`MemberRemnantRules`] with its
/// `EveryAttempt` arm, `GalaxyRules::member_remnants`, the `match` in [`remnant_attempt_fits`]
/// and the tripwire `the_tabulated_remnants_are_held_only_until_the_26_bump`.
pub(crate) const TABULATED_REMNANTS_VERSION: GeneratorVersion = GeneratorVersion::new(26);

/// How a remnant class's member draw decides an attempt (the hold of
/// [`TABULATED_REMNANTS_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum MemberRemnantRules {
    /// Version 25's and before: every attempt's star built ([`remnant_fits`](super::remnant_fits)).
    EveryAttempt,
    /// Version 26's: the cluster's tabulated remnants first ([`tabulated_fit`]), the star built
    /// only for an attempt they leave open.
    Tabulated,
}

/// How far below the lightest star that leaves anything but a white dwarf a cluster's
/// [`white_dwarf_ceiling`](RemnantTable::white_dwarf_ceiling) lies, M☉: the companion-stripped
/// star's electron-capture window,
/// 1 M☉, which the ceiling's unmarked draws do not open, and 0.2 M☉ for the members' draws and
/// compositions between the corners.
pub(crate) const CEILING_MARGIN: f64 = COMPANION_STRIPPED_WINDOW.value() + 0.2;

/// How far below the lightest star that leaves anything but a white dwarf a cluster's
/// [`unstripped_white_dwarf_ceiling`](RemnantTable::unstripped_white_dwarf_ceiling) lies, M☉:
/// [`CEILING_MARGIN`] less the stripped window an unstripped star cannot open, the 0.2 M☉ for the
/// members' draws and compositions between the corners.
pub(crate) const UNSTRIPPED_CEILING_MARGIN: f64 =
    CEILING_MARGIN - COMPANION_STRIPPED_WINDOW.value();

/// How far above the start of its capture window, `m_cc` less the window's width, a white dwarf
/// attempt is refused, M☉: the heaviest white dwarf on the generator's tracks under 26's rules lies
/// at most 0.13 M☉ above it unstripped and 0.07 M☉ stripped (\[Fe/H\] −2.3 to +0.3, with no helium
/// excess and 0.18, Reimers η 0 to 0.92; both at +0.3 with none), the main sequence's wind having
/// lowered the mass the window is tested in, so 0.3 M☉ leaves a margin of some 0.17.
pub(crate) const WHITE_DWARF_TOP_MARGIN: f64 = 0.3;

/// The lightest initial mass the ceiling's search starts from, M☉: every corner's star of it must
/// leave a white dwarf, or the cluster has no ceiling.
const CEILING_FLOOR: f64 = 3.0;

/// Halvings of the ceiling's search between [`CEILING_FLOOR`] and the heaviest white dwarf's
/// progenitor: seven, to 0.04 M☉.
const CEILING_HALVINGS: u32 = 7;

/// The iron spreads from a cluster's [Fe/H] the ceiling's corners take, in units of
/// [`IRON_SPREAD_SIGMA`]: four, which a member's normal offset passes once in 16,000.
const IRON_CORNER_SIGMAS: f64 = 4.0;

/// The draws of the ceiling's corners: Reimers η at 0, its floor (`reimers_eta`'s 0.5 + 0.07 z
/// held to zero), which keeps the most envelope and so lets a core grow longest, and the
/// companion-stripped mark never set; every other draw at its median.
fn corner_draws() -> StarDraws {
    StarDraws::from_parts(StarDrawsParts {
        eta: StandardNormal::new(-8.0).expect("a finite draw"),
        stripped: NEVER_STRIPPED,
        ..StarDrawsParts::MEDIAN
    })
}

/// The corners of the compositions of `model`'s members: its \[Fe/H\] (± four iron spreads, if it
/// has one) with no helium excess and, if it has a second population, with its greatest
/// ([`helium_excess_max`]).
#[must_use]
fn corners(model: &ClusterModel) -> Vec<Composition> {
    let fe_h = model.parameters().fe_h.value();
    let spread = model
        .marks()
        .iron_spread()
        .is_below(Threshold::from_probability(IRON_SPREAD_SHARE));
    let irons: &[f64] = if spread {
        &[-IRON_CORNER_SIGMAS, IRON_CORNER_SIGMAS]
    } else {
        &[0.0]
    };
    let heliums: &[f64] = if model.first_population_share() < 1.0 {
        &[0.0, helium_excess_max(model.initial_mass())]
    } else {
        &[0.0]
    };
    irons
        .iter()
        .flat_map(|iron| {
            heliums.iter().map(move |helium| {
                Composition::from_fe_h(
                    Dex::new(fe_h + iron * IRON_SPREAD_SIGMA),
                    HeliumExcess::new(*helium),
                )
            })
        })
        .collect()
}

/// How far above a cluster's highest `m_cc` its capture window's top
/// ([`RemnantTable::capture_window_top`]) lies, M☉: the main
/// sequence's wind lowers the mass the capture window is tested in from a star's initial mass
/// (`Builder::captures_electrons`), and an unstripped star's electron capture reaches at most
/// 0.14 M☉ above `m_cc` on the generator's tracks under 26's rules (\[Fe/H\] −2.3 to +0.3, with
/// no helium excess and 0.18, Reimers η 0 to 0.71; at +0.3 with none), so 0.5 M☉ leaves a margin
/// of some three.
pub(crate) const CAPTURE_MARGIN: f64 = 0.5;

/// The highest `m_cc` (`ElectronCaptureWindows::iron_core_mass`, the lowest mass that makes an
/// iron core) at the corners of `model`'s members' compositions, M☉: the top of every capture
/// window of theirs, above which none of them leaves a white dwarf. It costs one to four
/// bisections of Hurley, Pols and Tout's (2000, MNRAS 315, 543) equation 66, some microseconds.
#[must_use]
pub(crate) fn highest_iron_core_mass(model: &ClusterModel) -> SolarMasses {
    SolarMasses::new(
        corners(model)
            .iter()
            .map(|comp| {
                ElectronCaptureWindows::new(&ZCoeffs::new(comp.z_fit()))
                    .iron_core_mass()
                    .value()
            })
            .fold(0.0, f64::max),
    )
}

/// A cluster's tabulated remnants under one set of stellar rules: what [`tabulated_fit`] reads
/// for its members' attempts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RemnantTable {
    /// [`CEILING_MARGIN`] below the cluster's [`white_dwarf_edge`], M☉, or zero: below it every
    /// member leaves a white dwarf, the stripped window, which may open up to 1 M☉ under the
    /// edge, among the margin.
    white_dwarf_ceiling: SolarMasses,
    /// [`UNSTRIPPED_CEILING_MARGIN`] below the cluster's [`white_dwarf_edge`], M☉, or zero: below
    /// it every unstripped member leaves a white dwarf, since only a companion-stripped star opens
    /// the wide capture window.
    unstripped_white_dwarf_ceiling: SolarMasses,
    /// [`highest_iron_core_mass`], M☉.
    highest_iron_core_mass: SolarMasses,
}

impl RemnantTable {
    /// The table of the cluster `model` under the stellar `rules`.
    #[must_use]
    pub(crate) fn of(model: &ClusterModel, rules: StellarRules) -> Self {
        let edge = white_dwarf_edge(model, rules).value();
        let highest_iron_core_mass = highest_iron_core_mass(model);
        Self {
            white_dwarf_ceiling: SolarMasses::new((edge - CEILING_MARGIN).max(0.0)),
            unstripped_white_dwarf_ceiling: SolarMasses::new(
                (edge - UNSTRIPPED_CEILING_MARGIN).max(0.0),
            ),
            highest_iron_core_mass,
        }
    }

    /// [`CAPTURE_MARGIN`] above the cluster's [`highest_iron_core_mass`], M☉: above it no
    /// unstripped member collapses by electron capture.
    #[must_use]
    pub(crate) fn capture_window_top(&self) -> SolarMasses {
        SolarMasses::new(self.highest_iron_core_mass.value() + CAPTURE_MARGIN)
    }
}

/// The heaviest initial mass found to leave a white dwarf below the lightest that leaves anything
/// else, at every corner of `model`'s members' compositions and at [`corner_draws`], under the
/// stellar `rules`, M☉: the heaviest white dwarf's progenitor if none up to it leaves anything
/// else, or zero if a star of [`CEILING_FLOOR`] at a corner does. The ceilings lie below it
/// ([`RemnantTable`]).
///
/// It is found by halving between [`CEILING_FLOOR`] and that progenitor, by exact fates
/// (`sse::fate_of`) at the corners of the members' compositions ([`corners`]) and at
/// [`corner_draws`]. The halving's lower end is kept, so its 0.04 M☉ step takes none of the
/// margins. It costs some 8–30 lifetime builds, about 3–20 ms, once a cluster
/// ([`ClusterModel::remnant_table`]).
#[must_use]
pub(crate) fn white_dwarf_edge(model: &ClusterModel, rules: StellarRules) -> SolarMasses {
    let options = TrackOptions::default().with_rules(rules);
    let draws = corner_draws();
    // The heaviest mass found to leave a white dwarf at every corner so far.
    let mut edge = WHITE_DWARF_PROGENITOR_MAX;
    for comp in corners(model) {
        let dwarf = |m: f64| {
            let fate = sse::fate_of(SolarMasses::new(m), &comp, &draws, options);
            fate.remnant.kind() == RemnantKind::WhiteDwarf
                && fate.death.kind() == DeathKind::EnvelopeLoss
        };
        if !dwarf(CEILING_FLOOR) {
            return SolarMasses::new(0.0);
        }
        if dwarf(edge) {
            continue;
        }
        let (mut lo, mut hi) = (CEILING_FLOOR, edge);
        for _ in 0..CEILING_HALVINGS {
            let mid = f64::midpoint(lo, hi);
            if dwarf(mid) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        edge = lo;
    }
    SolarMasses::new(edge)
}

/// Whether a remnant class's attempt is kept in `galaxy`, for a member of the cluster `model`: of
/// class `kind` and initial mass `mass` (M☉), of `composition`, at `address` (the universe, system
/// and attempt) and age at the epoch `age`, judged on the effective escape speed and its pair's
/// systemic speed `speeds` (km/s), the arguments [`remnant_fits`](super::remnant_fits) takes.
/// Under [`MemberRemnantRules::Tabulated`] it is [`tabulated_fit`]'s answer where that gives one;
/// otherwise, and always below version 26, the star's own.
///
/// # Panics
///
/// In debug builds, if the tabulated remnants keep an attempt whose star's own answer refuses it.
#[must_use]
pub(crate) fn remnant_attempt_fits(
    galaxy: &Galaxy,
    model: &ClusterModel,
    (kind, mass): (ClassKind, f64),
    composition: &Composition,
    address: (crate::Seed, SystemId, u32),
    age: Years,
    speeds: (f64, f64),
) -> bool {
    let rules = galaxy.rules().stellar;
    let track = || super::remnant_fits(kind, mass, composition, address, age, speeds, rules);
    let tabulated = match galaxy.rules().member_remnants {
        MemberRemnantRules::EveryAttempt => None,
        MemberRemnantRules::Tabulated => tabulated_fit(
            (kind, mass),
            composition,
            address,
            (age, speeds),
            model,
            rules,
        ),
    };
    debug_assert!(
        tabulated != Some(true) || track(),
        "the tabulated remnants keep a {kind:?} of {mass} M☉ ({composition:?}, {address:?}) that \
         its star's track refuses"
    );
    tabulated.unwrap_or_else(track)
}

/// The age at the epoch, years, from which every star of a neutron-star class has died: the
/// lightest, of 8 M☉, lives under 60 Myr on the generator's tracks at any composition a cluster
/// has (`a_neutron_star_s_progenitor_dies_young`), so a gigayear leaves a margin of some sixteen.
/// A younger cluster's attempt may be a star still alive at the window's end, which the track
/// keeps, kickless.
const NEUTRON_STARS_DEAD_BY: f64 = 1e9;

/// What a cluster's tabulated remnants decide of an attempt of a remnant class of `kind`, whose
/// star has initial mass `mass` (M☉), `composition`, the draws of attempt `attempt` of `system` in
/// `universe` and the age at the epoch `age`, kicked against the cluster `model`'s effective escape
/// speed `v_eff` (km/s), a low-mode kick on its pair's `systemic` speed (km/s), under the stellar
/// `rules`: `Some` of whether it is kept, or `None` where its star's track decides (module
/// documentation). The arguments are [`remnant_fits`](super::remnant_fits)'s.
#[must_use]
pub(crate) fn tabulated_fit(
    (kind, mass): (ClassKind, f64),
    composition: &Composition,
    (universe, system, attempt): (crate::Seed, SystemId, u32),
    (age, (v_eff, systemic)): (Years, (f64, f64)),
    model: &ClusterModel,
    rules: StellarRules,
) -> Option<bool> {
    let draws = || StarDraws::for_attempt(universe, BodyId::new(system, 0), attempt);
    match kind {
        ClassKind::WhiteDwarf => {
            let table = model.remnant_table(rules);
            let draws = draws();
            let kept = || {
                let kick = StandardKickLaw::under(rules).white_dwarf_kick(&draws);
                (KilometresPerSecond::from(kick.speed()).value() < v_eff).then_some(true)
            };
            if mass < table.white_dwarf_ceiling.value() {
                return kept();
            }
            // Above the ceiling the star's companion-stripped mark says which capture window it
            // can open: a white dwarf of an unstripped star is kept below its own ceiling, and one
            // above the window's start is refused.
            // The mark as the star's track reads it, a lookup at 26 with the stripping table's
            // helium axis.
            let m0 = track_mass_of(SolarMasses::new(mass));
            let stripped = sse::is_companion_stripped_under(m0, composition, &draws, rules);
            if !stripped && mass < table.unstripped_white_dwarf_ceiling.value() {
                return kept();
            }
            let window = if stripped {
                COMPANION_STRIPPED_WINDOW
            } else {
                SINGLE_STAR_WINDOW
            };
            let top =
                table.highest_iron_core_mass.value() - window.value() + WHITE_DWARF_TOP_MARGIN;
            (mass >= top).then_some(false)
        }
        ClassKind::NeutronStar => {
            let law = StandardKickLaw::under(rules);
            let slowest = law.params().ordinary_speed_km_s(law.params().rank_clamp.0);
            // Neither kick can be judged surely too fast in a young cluster or a fast one, nor
            // for an age or speed that is not a number.
            let judged = age.value() >= NEUTRON_STARS_DEAD_BY && slowest >= v_eff;
            if !judged {
                return None;
            }
            if systemic >= v_eff {
                return Some(false);
            }
            // A slow pair keeps a low-mode kick, which takes electron capture, below the window's
            // top, or a companion's stripping.
            if mass < model.remnant_table(rules).capture_window_top().value() {
                return None;
            }
            let m0 = track_mass_of(SolarMasses::new(mass));
            let draws = draws();
            let stripped = binarity::is_stripped_under(draws.stripped(), m0, composition, rules);
            // A stripped star keeps the low mode only below its carbon–oxygen core's floor.
            if !stripped || mass >= model.co_core_floor(rules).value() {
                return Some(false);
            }
            if mass >= CO_CORE_SEARCH_TOP {
                return None;
            }
            low_mode_fit(
                m0,
                composition,
                &draws,
                model.main_sequence_loss(rules),
                rules,
            )
        }
        ClassKind::BlackHole | ClassKind::Living | ClassKind::Tail => None,
    }
}

/// The highest initial mass the CO core's search reaches, M☉: past it a cluster has no floor.
const CO_CORE_SEARCH_TOP: f64 = 40.0;

/// Halvings of the CO core's search between the capture window's top and [`CO_CORE_SEARCH_TOP`]:
/// seven, to some 0.25 M☉.
const CO_CORE_HALVINGS: u32 = 7;

/// The lightest initial mass from which every member of `model` dies with a carbon–oxygen core of
/// at least the low mode's ramp's top (`KickLawParams::low_ramp`, 3 M☉), under the stellar
/// `rules`, M☉, or infinity if a star of [`CO_CORE_SEARCH_TOP`] at a corner does not: from it a
/// companion-stripped star's iron core takes the ordinary mode alone
/// (`KickLawParams::low_mode_probability` is zero).
///
/// It is found by halving between the cluster's capture window's top and [`CO_CORE_SEARCH_TOP`],
/// by exact fates (`sse::fate_of`) at the corners of the members' compositions ([`corners`]) and at
/// Reimers η of 0 and 0.92, unstripped (the mark moves no core). The halving's upper end is kept.
/// The core stays past the ramp above it
/// (`a_neutron_star_s_core_is_past_the_ramp_above_its_floor`). It costs some 20–40 lifetime builds, about 10–20 ms, once a cluster, and only a
/// cluster with a slow pair's stripped neutron star asks for it ([`ClusterModel::co_core_floor`]).
#[must_use]
pub(crate) fn co_core_floor(model: &ClusterModel, rules: StellarRules) -> SolarMasses {
    let options = TrackOptions::default().with_rules(rules);
    let ramp_top = StandardKickLaw::under(rules).params().low_ramp.1;
    let bottom = model.remnant_table(rules).capture_window_top().value();
    let mut floor = bottom;
    for comp in corners(model) {
        for eta in [-8.0, 6.0] {
            let draws = StarDraws::from_parts(StarDrawsParts {
                eta: StandardNormal::new(eta).expect("a finite draw"),
                stripped: NEVER_STRIPPED,
                ..StarDrawsParts::MEDIAN
            });
            let past = |m: f64| {
                sse::fate_of(SolarMasses::new(m), &comp, &draws, options)
                    .death
                    .progenitor()
                    .co_core_mass()
                    .value()
                    >= ramp_top
            };
            if !past(CO_CORE_SEARCH_TOP) {
                return SolarMasses::new(f64::INFINITY);
            }
            if past(floor) {
                continue;
            }
            let (mut lo, mut hi) = (floor, CO_CORE_SEARCH_TOP);
            for _ in 0..CO_CORE_HALVINGS {
                let mid = f64::midpoint(lo, hi);
                if past(mid) {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            floor = hi;
        }
    }
    SolarMasses::new(floor)
}

/// What a slow pair's companion-stripped neutron-star attempt below its cluster's carbon–oxygen
/// core floor decides, with track mass `m0`, `composition` and `draws`, its main sequence losing
/// at most `loss` (the cluster's [`main_sequence_loss`]), under the stellar `rules`: `Some` of
/// whether it is kept, or `None` where its star's track decides.
///
/// Its early AGB reads the mass its main sequence ends with, between `m0` less `loss` and `m0`,
/// and collapses its iron core with that mass's cores (`sse::early_agb_cores`), which grow with
/// it; so its carbon–oxygen core lies between theirs at the two masses. It is kept only as a
/// neutron star in the low mode: refused where its mode's mark or its remnant's sends even the
/// lightest core elsewhere (the low mode's probability falls, and Mandel and Müller's black
/// holes' rises, with the core), and kept where neither sends the heaviest.
#[must_use]
fn low_mode_fit(
    m0: SolarMasses,
    composition: &Composition,
    draws: &StarDraws,
    loss: SolarMasses,
    rules: StellarRules,
) -> Option<bool> {
    let coeffs = ZCoeffs::new(composition.z_fit());
    let law = StandardKickLaw::under(rules);
    let params = law.params();
    let remnant = RemnantDraws::of(draws);
    // Whether a core of `co` within a helium core of `helium` takes the low mode, and leaves a
    // neutron star.
    let low = |co: SolarMasses| {
        draws
            .kick_mode()
            .is_below(Threshold::from_probability(params.low_mode_probability(co)))
    };
    let neutron_star = |(co, helium): (SolarMasses, SolarMasses)| {
        matches!(
            core_collapse(co, helium, remnant),
            CoreCollapse::NeutronStar { .. }
        )
    };
    let lightest = sse::early_agb_cores(SolarMasses::new(m0.value() - loss.value()), &coeffs);
    if !low(lightest.0) || !neutron_star(lightest) {
        return Some(false);
    }
    let heaviest = sse::early_agb_cores(m0, &coeffs);
    (low(heaviest.0) && neutron_star(heaviest)).then_some(true)
}

/// How far a cluster's main-sequence loss ([`main_sequence_loss`]) lies above the largest its
/// corners lose: a factor, and an addition in M☉.
const LOSS_MARGIN: (f64, f64) = (1.25, 0.005);

/// The most mass, M☉, a member of `model` of a slow pair's stripped neutron-star attempt loses on
/// its main sequence under the stellar `rules`, with a margin ([`LOSS_MARGIN`]): the largest loss
/// at the cluster's carbon–oxygen core floor ([`co_core_floor`], held to [`CO_CORE_SEARCH_TOP`]),
/// the heaviest such member, at the corners of its members' compositions and at Reimers η 0, 0.5
/// and 0.92, since the main sequence's wind grows with the mass
/// (`the_main_sequence_loses_most_at_the_floor`). It costs nine main sequences at most, some
/// 0.8 ms.
#[must_use]
pub(crate) fn main_sequence_loss(model: &ClusterModel, rules: StellarRules) -> SolarMasses {
    let options = TrackOptions::default().with_rules(rules);
    let top = track_mass_of(SolarMasses::new(
        model.co_core_floor(rules).value().min(CO_CORE_SEARCH_TOP),
    ));
    let mut loss = 0.0_f64;
    for comp in corners(model) {
        for eta in [-8.0, 0.0, 6.0] {
            let draws = StarDraws::from_parts(StarDrawsParts {
                eta: StandardNormal::new(eta).expect("a finite draw"),
                stripped: NEVER_STRIPPED,
                ..StarDrawsParts::MEDIAN
            });
            let end = sse::main_sequence_end_mass(top, &comp, &draws, options);
            loss = loss.max(top.value() - end.value());
        }
    }
    SolarMasses::new(loss * LOSS_MARGIN.0 + LOSS_MARGIN.1)
}

/// A value of a cluster model's found when first asked under one set of stellar rules, with the
/// rules it was found under (the sim-determinism skill's lazy exception, as the galaxy's centre
/// motion is held): [`ClusterModel::remnant_table`]'s and [`ClusterModel::co_core_floor`]'s.
#[derive(Clone)]
pub(crate) struct RulesCell<T>(OnceLock<(StellarRules, T)>);

/// An empty cell, whatever the value's type.
impl<T> Default for RulesCell<T> {
    fn default() -> Self {
        Self(OnceLock::new())
    }
}

impl<T: Copy> RulesCell<T> {
    /// The value under `rules`: the cell's, found by `find` the first time, or found afresh under
    /// rules other than the cell's.
    #[must_use]
    pub(crate) fn get(&self, rules: StellarRules, find: impl Fn() -> T) -> T {
        let &(under, value) = self.0.get_or_init(|| (rules, find()));
        if under == rules { value } else { find() }
    }
}

/// Equal always: the value is a pure function of the model and the rules, which a model's
/// equality compares in the first place, so comparing it would make two equal models unequal
/// because only one of them had been asked.
impl<T> PartialEq for RulesCell<T> {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

/// The same text whether or not the value has been found, so that a model's `Debug` text depends
/// on what it is and not on its history.
impl<T> std::fmt::Debug for RulesCell<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RulesCell")
    }
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::order::assert_order_independent;

    use super::*;
    use crate::coords::{GalacticPosition, GalacticVelocity};
    use crate::galaxy::features::FeatureProcess;
    use crate::galaxy::features::catalogue::{FeatureCatalogue, FeatureRecord};
    use crate::galaxy::features::interior::{
        MemberAbundances, MemberClass, MemberClassTable, Multiplicity,
    };
    use crate::galaxy::features::members::{attempt_systemic_speed, draw_member, remnant_fits};
    use crate::galaxy::imf::MassBand;
    use crate::galaxy::params::GalaxyParams;
    use crate::galaxy::rules::GalaxyRules;
    use crate::id::{FeatureMemberId, MemberSlot};
    use crate::stellar::fates::STRIPPED_WINDOW;
    use crate::stellar::remnant::KickMode;
    use crate::{GENERATOR_VERSION, Seed};

    /// The fixture's most massive globular's white-dwarf ceiling under 26's rules, 6.2958 M☉.
    const MASSIVE_CEILING: f64 = f64::from_bits(0x4019_2ef3_3333_3333);

    /// The fixture's slowest globular's, 5.7453 M☉.
    const SLOW_CEILING: f64 = f64::from_bits(0x4016_fb33_3333_3333);

    /// The fixture's most massive globular's highest `m_cc`, 7.6224 M☉.
    const MASSIVE_IRON_CORE: f64 = f64::from_bits(0x401e_7d4e_6a9b_c6f6);

    /// The fixture's slowest globular's, 7.0710 M☉.
    const SLOW_IRON_CORE: f64 = f64::from_bits(0x401c_48bc_43a0_ece8);

    /// The fixture's most massive globular's capture window's top under 26's rules, 8.1224 M☉.
    const MASSIVE_CAPTURE_TOP: f64 = f64::from_bits(0x4020_3ea7_354d_e37b);

    /// The fixture's slowest globular's, 7.5710 M☉.
    const SLOW_CAPTURE_TOP: f64 = f64::from_bits(0x401e_48bc_43a0_ece8);

    /// The fixture's most massive globular's unstripped white-dwarf ceiling under 26's rules,
    /// 7.2958 M☉.
    const MASSIVE_UNSTRIPPED_CEILING: f64 = f64::from_bits(0x401d_2ef3_3333_3333);

    /// The fixture's slowest globular's, 6.7453 M☉.
    const SLOW_UNSTRIPPED_CEILING: f64 = f64::from_bits(0x401a_fb33_3333_3333);

    /// The fixture's most massive globular's carbon–oxygen core floor under 26's rules,
    /// 13.103 M☉.
    const MASSIVE_CO_CORE_FLOOR: f64 = f64::from_bits(0x402a_34dd_14f9_b7f0);

    /// The fixture's slowest globular's, 12.638 M☉.
    const SLOW_CO_CORE_FLOOR: f64 = f64::from_bits(0x4029_46af_6c87_e3f2);

    /// The Milky Way fixture of the members' tests, under every 26 hold.
    fn galaxy() -> &'static Galaxy {
        static GALAXY: OnceLock<Galaxy> = OnceLock::new();
        GALAXY.get_or_init(|| {
            Galaxy::from_params_with_rules(
                Seed::new(0x0910_0000),
                GalaxyParams::milky_way_like(),
                GalaxyRules::every_26_hold(),
            )
            .expect("the Milky Way fixture builds")
        })
    }

    /// A cluster of the fixture: its record, model and class table.
    type Cluster = (FeatureRecord, ClusterModel, MemberClassTable);

    /// The fixture's most massive globular, the one with the greatest helium excess, whose
    /// escape speed is above the ordinary kicks' floor, and its globular of the lowest effective
    /// escape speed, below it.
    fn clusters(galaxy: &Galaxy) -> [Cluster; 2] {
        let models: Vec<(FeatureRecord, ClusterModel)> =
            FeatureCatalogue::walk_process(galaxy, FeatureProcess::Globular)
                .filter_map(|f| ClusterModel::from_record(galaxy, &f).map(|m| (f, m)))
                .collect();
        let massive = models
            .iter()
            .max_by(|a, b| {
                a.1.initial_mass()
                    .value()
                    .total_cmp(&b.1.initial_mass().value())
            })
            .expect("the fixture has globulars");
        let slow = models
            .iter()
            .min_by(|a, b| {
                let v = |m: &ClusterModel| m.escape_speed_effective().value();
                v(&a.1).total_cmp(&v(&b.1))
            })
            .expect("the fixture has globulars");
        [massive, slow].map(|(feature, model)| {
            let table = MemberClassTable::new(galaxy, model, feature.reach(), [1.0, 0.0, 0.0]);
            (*feature, model.clone(), table)
        })
    }

    /// The table's single classes of `kind`.
    fn classes_of(table: &MemberClassTable, kind: ClassKind) -> Vec<MemberClass> {
        MassBand::ALL
            .into_iter()
            .flat_map(|band| table.classes(band).map(|(c, _)| *c).collect::<Vec<_>>())
            .filter(|c| c.kind == kind && c.multiplicity == Multiplicity::Single)
            .collect()
    }

    #[test]
    fn the_tabulated_remnants_are_held_only_until_the_26_bump() {
        assert!(
            GENERATOR_VERSION < TABULATED_REMNANTS_VERSION,
            "the 25 → 26 bump puts the tabulated remnants on the record: a remnant class's member \
             draw asks the cluster's ceiling and the kicks' floor before it builds an attempt's \
             star. Delete `TABULATED_REMNANTS_VERSION`, `MemberRemnantRules` with its \
             `EveryAttempt` arm, `GalaxyRules::member_remnants`, the `match` in \
             `remnant_attempt_fits` and this test, in the bump's own commit"
        );
        assert_eq!(
            GalaxyRules::of_version(GENERATOR_VERSION).member_remnants,
            MemberRemnantRules::EveryAttempt
        );
    }

    /// The fixture's most massive globular, of [Fe/H] −0.54 and a helium excess up to 0.18 in its
    /// second population, and its slowest, of [Fe/H] −1.09 with no second population, have
    /// white-dwarf ceilings of 6.30 and 5.75 M☉ (7.30 and 6.75 M☉ an unstripped star's) and capture
    /// windows' tops of 8.12 and 7.57 M☉ and carbon–oxygen core floors of 13.10 and 12.64 M☉ under
    /// 26's rules, pinned to the bit; the massive one's corners take its helium excess, the slow
    /// one's none.
    #[test]
    fn the_fixture_s_remnant_tables_are_pinned() {
        let galaxy = galaxy();
        let rules = galaxy.rules().stellar;
        let [massive, slow] = clusters(galaxy).map(|(_, model, _)| {
            let heliums: Vec<f64> = corners(&model)
                .iter()
                .map(|c| c.helium_excess().value())
                .collect();
            (model.remnant_table(rules), heliums)
        });
        assert_same_bits(massive.0.highest_iron_core_mass.value(), MASSIVE_IRON_CORE);
        assert_same_bits(slow.0.highest_iron_core_mass.value(), SLOW_IRON_CORE);
        assert_same_bits(massive.0.white_dwarf_ceiling.value(), MASSIVE_CEILING);
        assert_same_bits(slow.0.white_dwarf_ceiling.value(), SLOW_CEILING);
        assert_same_bits(massive.0.capture_window_top().value(), MASSIVE_CAPTURE_TOP);
        assert_same_bits(slow.0.capture_window_top().value(), SLOW_CAPTURE_TOP);
        assert_same_bits(
            massive.0.unstripped_white_dwarf_ceiling.value(),
            MASSIVE_UNSTRIPPED_CEILING,
        );
        assert_same_bits(
            slow.0.unstripped_white_dwarf_ceiling.value(),
            SLOW_UNSTRIPPED_CEILING,
        );
        let [massive_floor, slow_floor] =
            clusters(galaxy).map(|(_, model, _)| model.co_core_floor(rules).value());
        assert_same_bits(massive_floor, MASSIVE_CO_CORE_FLOOR);
        assert_same_bits(slow_floor, SLOW_CO_CORE_FLOOR);
        assert!(massive.1.contains(&0.18), "{:?}", massive.1);
        assert!(slow.1.iter().all(|&h| h <= 0.0), "{:?}", slow.1);
    }

    /// A remnant table and a carbon–oxygen core's floor are pure functions of the model and the
    /// rules: the same whether the model is shared, asked under other rules first, built fresh or
    /// asked one before the other, and whichever members are drawn first, white dwarfs and neutron
    /// stars, the slow globular's on slow pairs among them.
    #[test]
    fn the_ceiling_and_the_members_do_not_depend_on_what_was_asked_first() {
        let galaxy = galaxy();
        let [_, (feature, shared, table)] = clusters(galaxy);
        let version = StellarRules::of_version(GENERATOR_VERSION);
        assert_ne!(version, galaxy.rules().stellar);
        let floor_under_version = shared.co_core_floor(version);
        assert_same_bits(
            shared.co_core_floor(galaxy.rules().stellar).value(),
            SLOW_CO_CORE_FLOOR,
        );
        assert_same_bits(
            shared.co_core_floor(version).value(),
            floor_under_version.value(),
        );
        let fresh = ClusterModel::from_record(galaxy, &feature).expect("a globular");
        assert_same_bits(
            fresh.co_core_floor(galaxy.rules().stellar).value(),
            SLOW_CO_CORE_FLOOR,
        );
        assert_same_bits(
            fresh
                .remnant_table(galaxy.rules().stellar)
                .white_dwarf_ceiling
                .value(),
            SLOW_CEILING,
        );
        let under_version = shared.remnant_table(version).white_dwarf_ceiling;
        assert_same_bits(
            shared
                .remnant_table(galaxy.rules().stellar)
                .white_dwarf_ceiling
                .value(),
            SLOW_CEILING,
        );
        assert_same_bits(
            shared.remnant_table(version).white_dwarf_ceiling.value(),
            under_version.value(),
        );
        let [x, y, z] = feature.position().to_light_years_f64();
        let position = GalacticPosition::from_light_years([x + 1.0, y, z]).unwrap();
        let classes =
            [ClassKind::WhiteDwarf, ClassKind::NeutronStar].map(|k| classes_of(&table, k)[0]);
        let ids: Vec<(MemberClass, FeatureMemberId)> = classes
            .iter()
            .flat_map(|class| {
                (0..12).map(|index| {
                    let id = FeatureMemberId::new(
                        feature.id().feature_ref(),
                        MemberSlot::InCell {
                            band: class.band.layer(),
                            level: 0,
                            cell: [8, 8, 8],
                            index,
                        },
                    )
                    .unwrap();
                    (*class, id)
                })
            })
            .collect();
        let draw = |model: &ClusterModel, (class, id): &(MemberClass, FeatureMemberId)| {
            draw_member(
                galaxy,
                &feature,
                model,
                class,
                &position,
                *id,
                GalacticVelocity::new([0.0; 3]),
            )
        };
        assert_order_independent(&ids, |key| draw(&shared, key));
        assert_order_independent(&ids, |key| {
            let fresh = ClusterModel::from_record(galaxy, &feature).expect("a globular");
            draw(&fresh, key)
        });
        let fresh = ClusterModel::from_record(galaxy, &feature).expect("a globular");
        assert!(
            ids.iter()
                .all(|key| draw(&fresh, key) == draw(&shared, key))
        );
    }

    /// Every exact fate above a white dwarf's refusal, [`WHITE_DWARF_TOP_MARGIN`] above the start
    /// of its capture window, is not a white dwarf: every 0.05 M☉ from it to 0.6 M☉ above `m_cc`,
    /// past the capture window's top ([`CAPTURE_MARGIN`]), unstripped and companion-stripped, at
    /// \[Fe/H\] −2.3 to +0.3 with no helium excess and 0.18, and Reimers η 0 and 0.92, under 26's
    /// rules.
    #[test]
    fn no_white_dwarf_lies_above_its_window_s_start_and_the_margin() {
        let rules = galaxy().rules().stellar;
        let options = TrackOptions::default().with_rules(rules);
        let mut fates = 0_u32;
        for fe_h in [-2.3, -1.0, 0.0, 0.3] {
            for helium in [0.0, 0.18] {
                let comp = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::new(helium));
                let m_cc = ElectronCaptureWindows::new(&ZCoeffs::new(comp.z_fit()))
                    .iron_core_mass()
                    .value();
                // The masses from the refusal to m_cc + 0.6 M☉, 0.05 M☉ apart.
                for (mark, window, masses) in [
                    (NEVER_STRIPPED, SINGLE_STAR_WINDOW, 9),
                    (
                        crate::rng::Mark::from_word(0),
                        COMPANION_STRIPPED_WINDOW,
                        27,
                    ),
                ] {
                    for z in [-8.0, 6.0] {
                        let draws = StarDraws::from_parts(StarDrawsParts {
                            eta: StandardNormal::new(z).unwrap(),
                            stripped: mark,
                            ..StarDrawsParts::MEDIAN
                        });
                        let top = m_cc - window.value() + WHITE_DWARF_TOP_MARGIN;
                        for i in 0..masses {
                            let m = top + 0.05 * f64::from(i);
                            let fate = sse::fate_of(SolarMasses::new(m), &comp, &draws, options);
                            fates += 1;
                            assert_ne!(
                                fate.remnant.kind(),
                                RemnantKind::WhiteDwarf,
                                "{m} M☉, {comp:?}, mark {mark:?}, η normal {z}"
                            );
                        }
                    }
                }
            }
        }
        assert_eq!(fates, 576);
    }

    /// Above each fixture globular's carbon–oxygen core floor every star dies with a core past the
    /// low mode's ramp, 3 M☉, at the corners of its members' compositions and at Reimers η 0, 0.5
    /// and 0.92, over ten masses even in ln m from the floor to 150 M☉, under 26's rules. The
    /// floors lie above `STRIPPED_WINDOW`, outside which the companion-stripped mark moves no fate,
    /// so an unstripped star's core is a stripped one's.
    #[test]
    fn a_neutron_star_s_core_is_past_the_ramp_above_its_floor() {
        let galaxy = galaxy();
        let rules = galaxy.rules().stellar;
        let options = TrackOptions::default().with_rules(rules);
        let ramp_top = StandardKickLaw::under(rules).params().low_ramp.1;
        for (_, model, _) in clusters(galaxy) {
            let floor = model.co_core_floor(rules).value();
            assert!(
                STRIPPED_WINDOW.1 < floor && floor < CO_CORE_SEARCH_TOP,
                "{floor} M☉"
            );
            for comp in corners(&model) {
                for eta in [-8.0, 0.0, 6.0] {
                    let draws = StarDraws::from_parts(StarDrawsParts {
                        eta: StandardNormal::new(eta).unwrap(),
                        stripped: NEVER_STRIPPED,
                        ..StarDrawsParts::MEDIAN
                    });
                    for i in 0..10 {
                        let m = floor * crate::math::powf(150.0 / floor, f64::from(i) / 9.0);
                        let fate = sse::fate_of(SolarMasses::new(m), &comp, &draws, options);
                        let core = fate.death.progenitor().co_core_mass().value();
                        assert!(core >= ramp_top, "{m} M☉, {comp:?}, η normal {eta}: {core}");
                    }
                }
            }
        }
    }

    /// Between each fixture globular's capture window's top and its carbon–oxygen core floor, a
    /// star's iron core collapses with the cores of its early AGB at the mass its main sequence
    /// ends with, bit for bit, and its main sequence loses no more than the cluster's
    /// [`main_sequence_loss`]: at the corners of its members' compositions and at Reimers η 0, 0.5
    /// and 0.92, over twelve masses even in ln m, under 26's rules.
    #[test]
    fn the_main_sequence_loses_most_at_the_floor() {
        let galaxy = galaxy();
        let rules = galaxy.rules().stellar;
        let options = TrackOptions::default().with_rules(rules);
        for (_, model, _) in clusters(galaxy) {
            let loss = model.main_sequence_loss(rules).value();
            let bottom = model.remnant_table(rules).capture_window_top().value();
            let top = model.co_core_floor(rules).value().min(CO_CORE_SEARCH_TOP);
            assert!(0.0 < loss && loss < 0.5, "{loss} M☉");
            for comp in corners(&model) {
                let coeffs = ZCoeffs::new(comp.z_fit());
                for eta in [-8.0, 0.0, 6.0] {
                    let draws = StarDraws::from_parts(StarDrawsParts {
                        eta: StandardNormal::new(eta).unwrap(),
                        stripped: NEVER_STRIPPED,
                        ..StarDrawsParts::MEDIAN
                    });
                    for i in 0..12 {
                        let m = bottom * crate::math::powf(top / bottom, f64::from(i) / 11.0);
                        let m0 = track_mass_of(SolarMasses::new(m));
                        let end = sse::main_sequence_end_mass(m0, &comp, &draws, options);
                        assert!(
                            end <= m0 && m0.value() - end.value() <= loss,
                            "{m} M☉, {comp:?}, η normal {eta}: {end:?} of {m0:?}"
                        );
                        let progenitor =
                            sse::fate_of(m0, &comp, &draws, options).death.progenitor();
                        let (co, helium) = sse::early_agb_cores(end, &coeffs);
                        assert_same_bits(progenitor.co_core_mass().value(), co.value());
                        assert_same_bits(progenitor.helium_core_mass().value(), helium.value());
                    }
                }
            }
        }
    }

    /// The lightest neutron star's progenitor, of 8 M☉, dies within 60 Myr at the corners of the
    /// compositions a cluster has, so a cluster a gigayear old has no living one; a younger
    /// cluster's neutron-star attempts are left to the track.
    #[test]
    fn a_neutron_star_s_progenitor_dies_young() {
        let rules = galaxy().rules().stellar;
        let options = TrackOptions::default().with_rules(rules);
        for fe_h in [-2.5, 0.2] {
            for helium in [0.0, 0.18] {
                let comp = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::new(helium));
                let life = sse::fate_of(SolarMasses::new(8.0), &comp, &corner_draws(), options)
                    .death
                    .age();
                assert!(life.value() < 6e7, "{comp:?}: {life:?}");
            }
        }
        let [_, (feature, model, _)] = clusters(galaxy());
        let v_eff = model.escape_speed_effective().value();
        let member = FeatureMemberId::new(
            feature.id().feature_ref(),
            MemberSlot::InCell {
                band: MassBand::E.layer(),
                level: 0,
                cell: [8, 8, 8],
                index: 0,
            },
        )
        .unwrap();
        let address = (galaxy().seed(), SystemId::from(member), 0);
        let comp = Composition::from_fe_h(model.parameters().fe_h, HeliumExcess::ZERO);
        let fit = |age: f64| {
            tabulated_fit(
                (ClassKind::NeutronStar, 9.0),
                &comp,
                address,
                (Years::new(age), (v_eff, 2.0 * v_eff)),
                &model,
                rules,
            )
        };
        assert_eq!(fit(1.2e10), Some(false));
        assert_eq!(fit(3e7), None);
    }

    /// The remnant attempts the agreement test asks of a cluster with `table`, by kind and initial
    /// mass (M☉): 48 white dwarfs from its lightest white dwarf's progenitor to 8 M☉, and neutron
    /// stars at 24 masses even in ln m over 8–150 M☉ and every 0.1 M☉ over 8–11.
    fn agreement_attempts(table: &MemberClassTable) -> impl Iterator<Item = (ClassKind, f64)> {
        let lo = classes_of(table, ClassKind::WhiteDwarf)
            .iter()
            .map(|c| c.initial_mass_range.0.value())
            .fold(8.0, f64::min);
        let dwarfs = (0..48).map(move |i| lo + (8.0 - lo) * f64::from(i) / 48.0);
        let neutron_stars = (0..24)
            .map(|i| 8.0 * crate::math::powf(150.0 / 8.0, f64::from(i) / 23.0))
            .chain((0..30).map(|i| 8.0 + 0.1 * f64::from(i)));
        dwarfs
            .map(|m| (ClassKind::WhiteDwarf, m))
            .chain(neutron_stars.map(|m| (ClassKind::NeutronStar, m)))
    }

    /// Wherever the tabulated remnants keep an attempt the star's track keeps it, so the member is
    /// the star its record builds; and a refusal the track would keep, which only moves the draw
    /// on to the next attempt, is counted and rare: white dwarfs over the cluster's white-dwarf
    /// range and neutron stars over 8–150 M☉ (densely over 8–11), at no, half the greatest and the
    /// greatest helium excess, each at four attempts' draws, on a pair half and twice the escape
    /// speed.
    #[test]
    fn the_tabulated_remnants_keep_only_what_the_track_keeps() {
        let galaxy = galaxy();
        let rules = galaxy.rules().stellar;
        let (mut kept, mut refused, mut refused_kept) = ([0_u32; 2], [0_u32; 2], [0_u32; 2]);
        // The neutron stars refused on a slow pair, by the stripped mark and the capture window.
        let mut slow = 0_u32;
        // The white dwarfs kept above the ceiling, unstripped below their own.
        let mut above = 0_u32;
        for (feature, model, table) in clusters(galaxy) {
            let v_eff = model.escape_speed_effective().value();
            let system = SystemId::from(
                FeatureMemberId::new(
                    feature.id().feature_ref(),
                    MemberSlot::InCell {
                        band: MassBand::C.layer(),
                        level: 0,
                        cell: [8, 8, 8],
                        index: 0,
                    },
                )
                .unwrap(),
            );
            for (kind, mass) in agreement_attempts(&table) {
                let k = usize::from(kind == ClassKind::NeutronStar);
                // A member's helium excess is the greatest times its enrichment squared.
                for enrichment in [0.0, 0.5_f64.sqrt(), 1.0] {
                    let abundances = MemberAbundances::of(
                        enrichment,
                        model.initial_mass(),
                        model.parameters().fe_h,
                        Dex::new(0.0),
                    );
                    let comp =
                        Composition::from_fe_h(model.parameters().fe_h, abundances.helium_excess);
                    for attempt in 0..4 {
                        for systemic in [0.5 * v_eff, 2.0 * v_eff] {
                            let address = (galaxy.seed(), system, attempt);
                            let speeds = (v_eff, systemic);
                            let Some(fits) = tabulated_fit(
                                (kind, mass),
                                &comp,
                                address,
                                (model.age(), speeds),
                                &model,
                                rules,
                            ) else {
                                continue;
                            };
                            let track = remnant_fits(
                                kind,
                                mass,
                                &comp,
                                address,
                                model.age(),
                                speeds,
                                rules,
                            );
                            if fits {
                                kept[k] += 1;
                                above += u32::from(
                                    kind == ClassKind::WhiteDwarf
                                        && mass
                                            >= model
                                                .remnant_table(rules)
                                                .white_dwarf_ceiling
                                                .value(),
                                );
                                assert!(
                                    track,
                                    "{kind:?} of {mass} M☉, {comp:?}, attempt {attempt}, \
                                     {systemic} km/s against {v_eff}: kept, but its track refuses"
                                );
                            } else {
                                refused[k] += 1;
                                refused_kept[k] += u32::from(track);
                                slow +=
                                    u32::from(kind == ClassKind::NeutronStar && systemic < v_eff);
                            }
                        }
                    }
                }
            }
        }
        eprintln!(
            "white dwarfs: {} kept ({above} above the ceiling), {} refused ({} the track keeps); \
             neutron stars: {} kept, {} refused ({slow} on a slow pair; {} the track keeps)",
            kept[0], refused[0], refused_kept[0], kept[1], refused[1], refused_kept[1]
        );
        assert!(
            kept[0] > 100 && above > 0 && refused[1] > 100 && slow > 100,
            "{kept:?} kept ({above} above the ceiling), {refused:?} refused, {slow} on a slow pair"
        );
        assert!(
            refused_kept
                .iter()
                .zip(&refused)
                .all(|(&n, &of)| 100 * n <= of),
            "{refused_kept:?} of {refused:?} refusals are kept by the track"
        );
        assert_eq!(
            refused_kept[1], 0,
            "{refused_kept:?} refusals kept by the track"
        );
    }

    /// A slow pair's companion-stripped neutron star below the slowest fixture globular's
    /// carbon–oxygen core floor is decided only as its track decides it, and most are: over 64
    /// members' first four attempts at eight masses even in ln m from the capture window's top to
    /// the floor, at no and the greatest helium excess, on a pair half the escape speed.
    #[test]
    fn a_slow_pair_s_stripped_neutron_star_is_decided_as_its_track_decides() {
        let galaxy = galaxy();
        let rules = galaxy.rules().stellar;
        let [_, (feature, model, _)] = clusters(galaxy);
        let v_eff = model.escape_speed_effective().value();
        let (bottom, top) = (
            model.remnant_table(rules).capture_window_top().value(),
            model.co_core_floor(rules).value(),
        );
        let (mut kept, mut refused, mut left) = (0_u32, 0_u32, 0_u32);
        for index in 0..64 {
            let system = SystemId::from(
                FeatureMemberId::new(
                    feature.id().feature_ref(),
                    MemberSlot::InCell {
                        band: MassBand::C.layer(),
                        level: 0,
                        cell: [8, 8, 8],
                        index,
                    },
                )
                .unwrap(),
            );
            for enrichment in [0.0, 1.0] {
                let abundances = MemberAbundances::of(
                    enrichment,
                    model.initial_mass(),
                    model.parameters().fe_h,
                    Dex::new(0.0),
                );
                let comp =
                    Composition::from_fe_h(model.parameters().fe_h, abundances.helium_excess);
                for attempt in 0..4 {
                    let address = (galaxy.seed(), system, attempt);
                    let draws = StarDraws::for_attempt(address.0, BodyId::new(system, 0), attempt);
                    for i in 0..8 {
                        let mass = bottom * crate::math::powf(top / bottom, f64::from(i) / 8.0);
                        let m0 = track_mass_of(SolarMasses::new(mass));
                        if !binarity::is_stripped_under(draws.stripped(), m0, &comp, rules) {
                            continue;
                        }
                        let speeds = (v_eff, 0.5 * v_eff);
                        let fit = tabulated_fit(
                            (ClassKind::NeutronStar, mass),
                            &comp,
                            address,
                            (model.age(), speeds),
                            &model,
                            rules,
                        );
                        let Some(fits) = fit else {
                            left += 1;
                            continue;
                        };
                        let track = remnant_fits(
                            ClassKind::NeutronStar,
                            mass,
                            &comp,
                            address,
                            model.age(),
                            speeds,
                            rules,
                        );
                        assert_eq!(
                            fits, track,
                            "{mass} M☉, {comp:?}, attempt {attempt} of member {index}"
                        );
                        if fits {
                            kept += 1;
                        } else {
                            refused += 1;
                        }
                    }
                }
            }
        }
        eprintln!(
            "slow pairs' stripped neutron stars: {kept} kept, {refused} refused, {left} left"
        );
        assert!(
            kept > 20 && refused > 20 && 10 * left < kept + refused,
            "{kept}, {refused}, {left}"
        );
    }

    /// Under 26's rules a drawn white dwarf or neutron star is still the star its record builds:
    /// its remnant is of its class's kind and its kick below the cluster's escape speed.
    #[test]
    fn a_tabulated_member_is_the_star_its_record_builds() {
        let galaxy = galaxy();
        for (feature, model, table) in clusters(galaxy) {
            let v_eff = model.escape_speed_effective().value();
            let [x, y, z] = feature.position().to_light_years_f64();
            let position = GalacticPosition::from_light_years([x + 1.0, y, z]).unwrap();
            for (kind, want, n) in [
                (ClassKind::WhiteDwarf, RemnantKind::WhiteDwarf, 24_u32),
                (ClassKind::NeutronStar, RemnantKind::NeutronStar, 12),
            ] {
                for class in classes_of(&table, kind) {
                    for index in 0..n {
                        let id = FeatureMemberId::new(
                            feature.id().feature_ref(),
                            MemberSlot::InCell {
                                band: class.band.layer(),
                                level: 0,
                                cell: [8, 8, 8],
                                index,
                            },
                        )
                        .unwrap();
                        let member = draw_member(
                            galaxy,
                            &feature,
                            &model,
                            &class,
                            &position,
                            id,
                            GalacticVelocity::new([0.0; 3]),
                        )
                        .expect("a remnant is kept within the attempts");
                        let stars = member.stars(galaxy);
                        let primary = &stars.stars()[0];
                        let what = format!("{class:?}, member {index}");
                        assert_eq!(primary.remnant().map(|r| r.kind()), Some(want), "{what}");
                        let judged = primary.natal_kick().map(|kick| {
                            if kick.mode() == KickMode::Low {
                                attempt_systemic_speed(galaxy, id, member.attempt())
                            } else {
                                KilometresPerSecond::from(kick.speed()).value()
                            }
                        });
                        assert!(judged.is_none_or(|v| v < v_eff), "{what}: {judged:?}");
                    }
                }
            }
        }
    }

    /// Before the 25 → 26 bump (the determinism audit's ask): every white-dwarf attempt the
    /// tabulated remnants keep below a cluster's ceilings, over every cluster of the fixture whose
    /// turn-off lies below them (its globulars, old open clusters and the nurseries' bound
    /// clusters), is one its star's track keeps. Each cluster's attempts are at masses even from
    /// its turn-off to its unstripped ceiling, twelve a globular's and three another's, at the
    /// corners of its members' compositions, and at the draws of three attempts a globular's
    /// member and two another's, the marks of which decide between the ceilings, on a pair at half
    /// the escape speed.
    #[test]
    #[ignore = "slow: some 56,000 clusters' ceilings and their white dwarfs' tracks"]
    fn every_fixture_cluster_s_white_dwarfs_below_its_ceiling_are_their_tracks() {
        let galaxy = galaxy();
        let rules = galaxy.rules().stellar;
        let (mut clusters, mut kept) = (0_u32, 0_u32);
        for process in FeatureProcess::ALL {
            let globular = process == FeatureProcess::Globular;
            let (masses, attempts) = if globular { (12_u32, 3) } else { (3, 2) };
            for feature in FeatureCatalogue::walk_process(galaxy, process) {
                let Some(model) = ClusterModel::from_record(galaxy, &feature) else {
                    continue;
                };
                let lo = model.turn_off_mass().value();
                if lo >= WHITE_DWARF_PROGENITOR_MAX - UNSTRIPPED_CEILING_MARGIN {
                    continue;
                }
                // Up to the unstripped ceiling, under which an unstripped star is kept too.
                let ceiling = model
                    .remnant_table(rules)
                    .unstripped_white_dwarf_ceiling
                    .value();
                if lo >= ceiling {
                    continue;
                }
                clusters += 1;
                let v_eff = model.escape_speed_effective().value();
                let system = SystemId::from(
                    FeatureMemberId::new(
                        feature.id().feature_ref(),
                        MemberSlot::InCell {
                            band: MassBand::C.layer(),
                            level: 0,
                            cell: [8, 8, 8],
                            index: 0,
                        },
                    )
                    .expect("a member's slot of every cluster"),
                );
                let speeds = (v_eff, 0.5 * v_eff);
                for comp in corners(&model) {
                    for i in 0..masses {
                        let mass = lo + (ceiling - lo) * (f64::from(i) + 0.5) / f64::from(masses);
                        for attempt in 0..attempts {
                            let address = (galaxy.seed(), system, attempt);
                            let fit = tabulated_fit(
                                (ClassKind::WhiteDwarf, mass),
                                &comp,
                                address,
                                (model.age(), speeds),
                                &model,
                                rules,
                            );
                            if fit != Some(true) {
                                continue;
                            }
                            kept += 1;
                            assert!(
                                remnant_fits(
                                    ClassKind::WhiteDwarf,
                                    mass,
                                    &comp,
                                    address,
                                    model.age(),
                                    speeds,
                                    rules,
                                ),
                                "{:?}: a white dwarf of {mass} M☉ ({comp:?}, attempt {attempt}) \
                                 below its ceiling of {ceiling} M☉ that its track refuses",
                                feature.id()
                            );
                        }
                    }
                }
            }
        }
        eprintln!("{kept} white dwarfs kept below the ceilings of {clusters} clusters");
        assert!(
            clusters > 10_000 && kept > 50_000,
            "{clusters} clusters, {kept} kept"
        );
    }
}
