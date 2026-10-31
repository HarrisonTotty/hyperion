//! Absolute photometry from a star's state: the bolometric correction to V, the absolute V
//! magnitude and the B − V colour (plan 06, P06.T23.a).
//!
//! All three read the mean dwarf sequence of Pecaut and Mamajek (2013, ApJS 208, 9) in Mamajek's
//! version 2022.04.16, the table [`classify`](crate::stellar::classify) types dwarfs on,
//! interpolated linearly in log₁₀ T<sub>eff</sub>: its rows to version 25 and its fitted grids from
//! 26 (`classify::pm13`'s hold), which the `_under` forms take from the stellar rules of a star's
//! galaxy and the public ones from the version's. The table's bolometric corrections are on the IAU
//! 2015 system, with M<sub>bol</sub>☉ = 4.74 ([`SOLAR_ABSOLUTE_BOLOMETRIC_MAGNITUDE`]), so the Sun
//! (5,772 K) reads BC<sub>V</sub> = −0.085 and M<sub>V</sub> = 4.825. They are the dwarfs'
//! corrections, read at the star's temperature whatever its gravity: a giant's differs little from
//! G to early K, but not for M giants. At Straižys and Kuriliene's (1981, Ap&SS 80, 353) class III
//! temperatures and gravities (their Tables III and VII), Worthey and Lee's (2011) giants lie 0.27,
//! 0.95 and 1.59 mag below their own dwarfs at M3 III, M5 III and M6 III, so a late M giant's
//! M<sub>V</sub> read from this table is up to that much too bright.
//! [`giant_bolometric_correction_v`] lies 0.16, 0.77 and 1.34 mag below this table there. Straižys
//! and Kuriliene's own bolometric corrections (zero point BC<sub>V</sub>☉ = −0.07; Torres 2010, AJ
//! 140, 1158) give 0.1–0.5, 1.1 and 1.7 mag, as P06.T23 read them from the paper's scan; the table
//! that holds them was not re-read (plan 06's Risks). To version 25 every star but a white dwarf
//! reads this table so; from 26 (`V_MAGNITUDE_VERSION`) an evolved star reads the giants'
//! correction instead, and that difference ends.
//!
//! - Above the table's hottest row (O3V, 44,900 K), BC<sub>V</sub> falls by 7.5 mag per dex of
//!   temperature from O3V's −4.01, the Rayleigh–Jeans limit, where the flux in V grows as
//!   T<sub>eff</sub> and the bolometric flux as T<sub>eff</sub>⁴, and B − V stays at O3V's −0.330.
//!   A blackbody at 550 nm falls only 6.7–7.3 mag per dex over 45,000–150,000 K; the calibration is
//!   the Montreal 0.6 M☉ DA models (Bédard et al. 2020, ApJ 901, 93), whose BC<sub>V</sub> falls
//!   4.01 mag over that range against this continuation's 3.93 (−4.15 against −4.01 at the anchor,
//!   −8.16 against −7.94 at 150,000 K), and whose B − V moves only from −0.30 to −0.36.
//! - The table gives BC<sub>V</sub> down to L5V (1,710 K) and B − V down to M9V (2,380 K); cooler
//!   objects have none here, as none is measured: a T dwarf is not observed in V.
//! - Neutron stars and black holes have no absolute V magnitude here, nor do white dwarfs to
//!   version 25. A degenerate atmosphere is not on the dwarf sequence: at 4,000 K the Montreal DA
//!   models give BC<sub>V</sub> = −0.40 where the M dwarfs' is −1.02, and B − V differs by up to
//!   0.25 mag from 8,000 to 40,000 K. Their photometry needs the white-dwarf model grids, which
//!   this table is not.
//!
//! # Ask A4's photometry (P06.T43.a, held for 26 by P06.T43.b)
//!
//! Two corrections, which [`absolute_magnitude_v`] reads under the stellar rules' `v_magnitude`
//! field from `V_MAGNITUDE_VERSION` (`VMagnitudeRules::A4`; A4's interim to 25):
//!
//! - [`white_dwarf_absolute_magnitude_v`] and [`white_dwarf_bolometric_correction_v`]: a white
//!   dwarf's V from the Montreal group's pure-hydrogen grid at its temperature and gravity, the
//!   photometry beside the evolutionary sequences of Bédard et al. (2020, ApJ 901, 93) that its
//!   cooling follows. Every white dwarf reads the hydrogen grid: which atmosphere a star has is
//!   `wd_spectral`'s mark, which a state does not carry. Over the grid's gravities the pure-helium
//!   grid's correction (`Table_DB`, the same release) differs from it by up to 0.29 mag from 4,000
//!   to 45,000 K, 0.45 mag above, and 1.1 mag below 4,000 K.
//! - [`giant_bolometric_correction_v`]: a cool giant's or supergiant's correction at its
//!   temperature and gravity, from Worthey and Lee's (2011, ApJS 193, 1) colour–temperature table
//!   and Levesque et al.'s (2005, ApJ 628, 973) column for red supergiants, handed over to this
//!   table's from 5,000 to 6,000 K.
//!
//! Each source is moved onto this module's zero point (M<sub>bol</sub>☉ = 4.74, BC<sub>V</sub>☉ =
//! −0.085), in the way its numbers are made. The Montreal tables are synthetic photometry of models
//! of known luminosity on M<sub>bol</sub>☉ = 4.75, so they move by the difference in
//! M<sub>bol</sub>☉, which keeps each model's V. Worthey and Lee's and Levesque et al.'s
//! corrections are scales set by the Sun's correction (−0.09 and −0.07), so they move to this
//! module's −0.085, which keeps a star's correction against the Sun's, and so against this table's
//! where the giants' hand over to it.
//!
//! Nothing here takes the logarithm of a luminosity that may be zero: a black hole has none
//! (ruling 40), and a zero luminosity has no magnitude.
//!
//! The sky reads a state through [`absolute_v_of_state`] and [`is_dark_in_v`] (rendering plan R06,
//! Design note 7), which add the interims of plan 06's asks A3 and A4 to
//! [`absolute_magnitude_v`]:
//!
//! - **A3, protostars dark in V.** A Class 0 protostar sits inside an envelope of A<sub>V</sub>
//!   ≳ 100 (André, Ward-Thompson and Barsony 1993; Whitney et al. 2003b, ApJ 598, 1079, §3), a
//!   Class I one inside A<sub>V</sub> 32–225 towards most inclinations (Whitney et al. 2003a, ApJ
//!   591, 1049, §2 and Fig. 3), so neither has a V magnitude here. A Class I source seen pole-on,
//!   down its outflow cavity (A<sub>V</sub> about 1.5), would show; the interim treats every one as
//!   dark, a few per cent of them wrongly (R06's Risks). Plan 06's protostar phase,
//!   [`Phase::Protostar`], is exactly the accretion that
//!   [`protostar_class`](crate::stellar::premain::protostar_class) calls Class 0 or I (it ends at
//!   [`PROTOSTAR_DURATION`](crate::stellar::premain::PROTOSTAR_DURATION)), so the phase alone
//!   decides. A Class II star, on the pre-main sequence, takes no circumstellar term: its birth
//!   cloud's extinction is plan 07's.
//! - **A4, white dwarfs (the interim, to version 25).** Under `VMagnitudeRules::Interim` plan
//!   06's photometry gives no white dwarf a V magnitude ([`absolute_magnitude_v`]), so a white
//!   dwarf is dark here; the sky's census counts it in its tallies. M giants are as bright as the
//!   dwarf table makes them (up to 1.34 mag too bright at M6 III by the giants' correction, 1.7 by
//!   Straižys and Kuriliene's). Under `VMagnitudeRules::A4`, from 26, the interim ends: a white
//!   dwarf takes its V from the Montreal grid through `absolute_magnitude_v_under`, and
//!   `absolute_v_of_state_under` reads it as it reads every other living star. Neutron stars and
//!   black holes stay dark.
//!
//! When plan 06 answers A3, [`absolute_v_of_state`] switches to its photometry. These functions
//! sat in the sky's own `photometry` module until the crate split (CS.T1) moved them here, beside
//! the photometry they stand in for; the sky re-exports them, and the census restates the bounds
//! that rest on A4's interim under the A4 arm (`sky::census`'s `BoundStar::brightest`).
//!
//! Extinction and apparent magnitudes belong to plan 07.

mod levesque05;
mod montreal_da;
mod wl11;

use crate::math;
use crate::stellar::classify::pm13::{Column, DwarfSequence};
use crate::stellar::rules::StellarRules;
use crate::stellar::{Phase, StarState};
use crate::units::{Dex, Kelvin, Magnitudes, SolarLuminosities};
use crate::{GENERATOR_VERSION, GeneratorVersion};

/// The generator version whose stellar rules give white dwarfs a V magnitude and evolved stars
/// the giants' bolometric correction (plan 06, P06.T43.b's swap of rendering plan R06's ask A4;
/// ruling 29 of signoff 12): [`absolute_magnitude_v_under`] reads
/// [`white_dwarf_absolute_magnitude_v`] and [`giant_bolometric_correction_v`] under
/// [`VMagnitudeRules::A4`].
///
/// Below it every star takes the held arm, [`VMagnitudeRules::Interim`], the code of version 24
/// verbatim, so every generated value keeps its bits; the tests reach the new arm through
/// `StellarRules::every_26_hold()`. The bump to 26 deletes this hold: [`VMagnitudeRules`] with its
/// `Interim` arm, the stellar rules' `v_magnitude` field, the `match` in
/// [`absolute_magnitude_v_under`], [`VMagnitudeRules::white_dwarfs_brightest_v`]'s `None` and the
/// tripwire (`the_a4_magnitudes_are_held_only_until_the_26_bump`).
pub(crate) const V_MAGNITUDE_VERSION: GeneratorVersion = GeneratorVersion::new(26);

/// Which V magnitudes plan 06's photometry gives white dwarfs and evolved stars (the stellar rules'
/// `v_magnitude` field, the hold of [`V_MAGNITUDE_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum VMagnitudeRules {
    /// Version 25's and before, A4's interim: no white dwarf has a V magnitude, and every living
    /// star reads the dwarf table's correction at its temperature, whatever its gravity.
    Interim,
    /// Version 26's: a white dwarf reads the Montreal grid
    /// ([`white_dwarf_absolute_magnitude_v`]), and every living star past the hydrogen main
    /// sequence reads the giants' correction at its gravity ([`giant_bolometric_correction_v`]).
    A4,
}

impl VMagnitudeRules {
    /// The brightest absolute V a white dwarf can have under this arm, which the sky's census
    /// bounds a star that may be one by: `None` under the interim, where white dwarfs are dark, and
    /// [`WHITE_DWARF_BRIGHTEST_V`] under A4.
    #[must_use]
    pub(crate) const fn white_dwarfs_brightest_v(self) -> Option<Magnitudes> {
        match self {
            Self::Interim => None,
            Self::A4 => Some(WHITE_DWARF_BRIGHTEST_V),
        }
    }
}

/// The brightest absolute V, mag, that any white dwarf of the generator reads under
/// [`VMagnitudeRules::A4`], margin included: the bound the sky's census gives a star that may be a
/// white dwarf until the 26 bump refits its tables with white-dwarf rows.
///
/// A white dwarf is brightest in V as it forms: at its progenitor's last luminosity, hot, and not
/// yet shrunk to its degenerate radius. Beyond the Montreal grid's 150,000 K its correction falls
/// at the Rayleigh–Jeans 7.5 mag per dex while its bolometric magnitude falls at 10 per dex at a
/// fixed radius, so its V brightens only 2.5 mag per dex of temperature. The slow test
/// `no_white_dwarf_is_brighter_than_the_census_bound` read 1.2 million white-dwarf states of single
/// tracks of 0.7–12 M☉ over the phase envelope's \[Fe/H\] and η nodes, from a thousandth of a year
/// after each death, and 4.4 million of the binary engine's pairs (two samples of every pair-light
/// cell). The brightest was M<sub>V</sub> 4.639, a 0.51 M☉ carbon–oxygen dwarf at its birth (1,160
/// L☉, 0.074 R☉, 124,000 K) from a 0.77 M☉ star of \[Fe/H\] +0.18 with no Reimers wind, which
/// outlives the Galaxy. The ceiling is the whole magnitude below it, 0.64 mag brighter.
pub(crate) const WHITE_DWARF_BRIGHTEST_V: Magnitudes = Magnitudes::new(4.0);

/// The Sun's absolute bolometric magnitude, 4.74 mag: IAU 2015 Resolution B2's zero point
/// (M<sub>bol</sub> = 0 at 3.0128 × 10²⁸ W) applied to the nominal solar luminosity of Resolution
/// B3, 3.828 × 10²⁶ W.
pub const SOLAR_ABSOLUTE_BOLOMETRIC_MAGNITUDE: Magnitudes = Magnitudes::new(4.74);

/// How far BC<sub>V</sub> falls per dex of effective temperature beyond the table's hottest row,
/// mag: the Rayleigh–Jeans limit, 2.5 × (4 − 1), where the V flux grows as T and the bolometric as
/// T⁴.
const RAYLEIGH_JEANS_BC_SLOPE: f64 = 7.5;

/// What moves the Montreal tables' BC<sub>V</sub> onto this module's scale: their
/// M<sub>bol</sub>☉ is 4.75 (`montreal_da`'s documentation), so for the same star their
/// M<sub>bol</sub>, and with it BC<sub>V</sub> = M<sub>bol</sub> − V, is 0.01 mag above this
/// module's.
const MONTREAL_TO_MODULE: Magnitudes = Magnitudes::new(-0.01);

/// What moves Worthey and Lee's (2011) BC<sub>V</sub> onto this module's scale: theirs puts the
/// Sun at −0.09 mag, VandenBerg and Clem's (2003) zero point (their section 2.5), this module's at
/// −0.085.
const WL11_TO_MODULE: Magnitudes = Magnitudes::new(0.005);

/// What moves Levesque et al.'s (2005) BC<sub>V</sub> onto this module's scale: theirs puts the
/// Sun at −0.07 mag, Bessell, Castelli and Plez's (1998) zero point (their section 3.2), this
/// module's at −0.085.
const LEVESQUE05_TO_MODULE: Magnitudes = Magnitudes::new(-0.015);

/// The band of effective temperature, K, across which [`giant_bolometric_correction_v`] hands the
/// giants' correction over to the dwarf table's: the giants' alone to 5,000 K, the table's alone
/// from 6,000 K, linear in temperature between. The two differ by 0.01–0.06 mag there at log g 0–3,
/// the giants' the less negative (0.03–0.06 mag at log g 0, 0.01–0.03 at 3), as this module's
/// documentation says of G and early K giants. The band is our choice.
const GIANT_HANDOVER_K: (f64, f64) = (5_000.0, 6_000.0);

/// The band of log₁₀ g (cm s⁻²) across which [`giant_bolometric_correction_v`] hands Levesque et
/// al.'s red supergiants' column over to Worthey and Lee's giants: the column alone at log g 0.0,
/// the gravity of its MARCS models, and below; Worthey and Lee's alone from 1.0; linear in log g
/// between. Gravity cannot tell a supergiant from a late M giant of the AGB (Straižys and
/// Kuriliene's M2 Iab and M6 III both lie near log g 0.1–0.5), and the column changes "< 0.05 mag
/// over 0.5 dex in log g" from 3,500 to 4,300 K (Levesque et al., section 3.2). The band is our
/// choice.
const SUPERGIANT_LOG_G: (f64, f64) = (0.0, 1.0);

/// The temperatures, K, at which [`giant_bolometric_correction_v`] reads Levesque et al.'s column:
/// from 3,400 K, the row below their coolest supergiant (M5 I, 3,450 K, Table 5), to the table's
/// 4,300 K. Its 3,200 and 3,300 K rows, models cooler than any supergiant they measured, stand
/// 0.57 and 0.23 mag below Worthey and Lee's log g 0.0 correction there and are left out; beyond
/// the range the column's offset from Worthey and Lee's log g 0.0 correction at its nearer end,
/// +0.006 mag at 3,400 K and +0.044 at 4,300 K, is held.
const SUPERGIANT_RANGE_K: (f64, f64) = (3_400.0, 4_300.0);

/// The absolute bolometric magnitude of a luminosity: M<sub>bol</sub> = 4.74 − 2.5 log₁₀(L ÷ L☉).
///
/// `None` for a luminosity that is not finite and positive: a black hole, or nothing, has no
/// magnitude.
///
/// # Examples
///
/// ```
/// use hyperion_sim::stellar::photometry::absolute_bolometric_magnitude;
/// use hyperion_sim::units::SolarLuminosities;
///
/// let vega = absolute_bolometric_magnitude(SolarLuminosities::new(40.0)).ok_or("luminous")?;
/// assert!((vega.value() - 0.735).abs() < 1e-3);
/// assert_eq!(absolute_bolometric_magnitude(SolarLuminosities::ZERO), None);
/// # Ok::<(), &str>(())
/// ```
#[must_use]
pub fn absolute_bolometric_magnitude(luminosity: SolarLuminosities) -> Option<Magnitudes> {
    let l = luminosity.value();
    (l.is_finite() && l > 0.0).then(|| {
        Magnitudes::new(SOLAR_ABSOLUTE_BOLOMETRIC_MAGNITUDE.value() - 2.5 * math::log10(l))
    })
}

/// The V-band bolometric correction BC<sub>V</sub> = M<sub>bol</sub> − M<sub>V</sub> at effective
/// temperature `teff`, from the mean dwarf sequence (see the [module](self) documentation).
///
/// `None` below the table's last correction (L5V, 1,710 K) and for a temperature that is not
/// finite and positive.
///
/// # Examples
///
/// ```
/// use hyperion_sim::stellar::photometry::bolometric_correction_v;
/// use hyperion_sim::units::Kelvin;
///
/// let sun = bolometric_correction_v(Kelvin::new(5_772.0)).ok_or("tabulated")?;
/// assert!((sun.value() + 0.0847).abs() < 1e-4);
/// assert_eq!(bolometric_correction_v(Kelvin::new(1_000.0)), None);
/// # Ok::<(), &str>(())
/// ```
#[must_use]
pub fn bolometric_correction_v(teff: Kelvin) -> Option<Magnitudes> {
    bolometric_correction_v_under(teff, StellarRules::of_version(GENERATOR_VERSION))
}

/// [`bolometric_correction_v`] on the dwarf sequence the stellar stage's `rules` choose
/// ([`DwarfSequence::of`]).
#[must_use]
pub(crate) fn bolometric_correction_v_under(
    teff: Kelvin,
    rules: StellarRules,
) -> Option<Magnitudes> {
    column_with_hot_tail(
        teff,
        DwarfSequence::of(rules),
        Column::BcV,
        |t, hottest_k| -RAYLEIGH_JEANS_BC_SLOPE * (math::log10(t) - math::log10(hottest_k)),
    )
}

/// The intrinsic B − V colour at effective temperature `teff`, from the mean dwarf sequence (see
/// the [module](self) documentation).
///
/// `None` below the table's last colour (M9V, 2,380 K) and for a temperature that is not finite
/// and positive.
#[must_use]
pub fn colour_b_v(teff: Kelvin) -> Option<Magnitudes> {
    colour_b_v_under(teff, StellarRules::of_version(GENERATOR_VERSION))
}

/// [`colour_b_v`] on the dwarf sequence the stellar stage's `rules` choose
/// ([`DwarfSequence::of`]).
#[must_use]
pub(crate) fn colour_b_v_under(teff: Kelvin, rules: StellarRules) -> Option<Magnitudes> {
    column_with_hot_tail(teff, DwarfSequence::of(rules), Column::ColourBV, |_, _| 0.0)
}

/// The absolute V magnitude of a star: M<sub>V</sub> = M<sub>bol</sub> − BC<sub>V</sub> at its
/// effective temperature, under the version's rules (`absolute_magnitude_v_under` says what each
/// arm reads).
///
/// `None` for a white dwarf to version 25, a neutron star, a black hole or nothing (see the
/// [module](self) documentation), for a star without luminosity, and where the table has no
/// correction (below 1,710 K).
///
/// # Examples
///
/// ```
/// use hyperion_sim::stellar::photometry::absolute_magnitude_v;
/// use hyperion_sim::stellar::{Phase, StarState, StarStateParts};
/// use hyperion_sim::units::{SolarLuminosities, SolarMasses, SolarMassesPerYear, SolarRadii, Years};
///
/// let sun = StarState::new(StarStateParts {
///     phase: Phase::MainSequence,
///     age: Years::new(4.57e9),
///     mass: SolarMasses::new(1.0),
///     core_mass: SolarMasses::ZERO,
///     luminosity: SolarLuminosities::new(1.0),
///     radius: SolarRadii::new(1.0),
///     mass_loss_rate: SolarMassesPerYear::ZERO,
///     phase_fraction: 0.45,
/// });
/// let m_v = absolute_magnitude_v(&sun).ok_or("the Sun has a magnitude")?;
/// assert!((m_v.value() - 4.825).abs() < 1e-3);
/// # Ok::<(), &str>(())
/// ```
#[must_use]
pub fn absolute_magnitude_v(state: &StarState) -> Option<Magnitudes> {
    absolute_magnitude_v_under(state, StellarRules::of_version(GENERATOR_VERSION))
}

/// [`absolute_magnitude_v`] under the stellar stage's `rules`: what a star generated in a galaxy is
/// given, from its model's rules. Its dwarf sequence is theirs, and their `v_magnitude` field is
/// chosen here, the hold's one choke point ([`V_MAGNITUDE_VERSION`]):
///
/// - under [`VMagnitudeRules::Interim`], a living star reads the dwarf table's correction at its
///   temperature and a remnant has none;
/// - under [`VMagnitudeRules::A4`], a white dwarf reads [`white_dwarf_absolute_magnitude_v`], and
///   every living state past the hydrogen main sequence (the Hertzsprung gap, the giant branches,
///   core helium burning, the asymptotic giant branch, the naked helium stars and the post-AGB
///   crossing) reads [`giant_bolometric_correction_v`] at its surface gravity. A protostar, a
///   pre-main-sequence or main-sequence star and a substellar object keep the dwarf table's, bit
///   for bit, and so does every star from 6,000 K, where the giants' correction is the table's.
#[must_use]
pub(crate) fn absolute_magnitude_v_under(
    state: &StarState,
    rules: StellarRules,
) -> Option<Magnitudes> {
    let phase = state.phase();
    match rules.v_magnitude {
        VMagnitudeRules::Interim => {
            if !phase.is_living() {
                return None;
            }
            let m_bol = absolute_bolometric_magnitude(state.luminosity())?;
            let bc = bolometric_correction_v_under(state.effective_temperature(), rules)?;
            Some(m_bol - bc)
        }
        VMagnitudeRules::A4 => match phase {
            Phase::HeliumWhiteDwarf
            | Phase::CarbonOxygenWhiteDwarf
            | Phase::OxygenNeonWhiteDwarf => white_dwarf_absolute_magnitude_v(state),
            Phase::NeutronStar | Phase::BlackHole | Phase::NoRemnant => None,
            Phase::HertzsprungGap
            | Phase::FirstGiantBranch
            | Phase::CoreHeliumBurning
            | Phase::EarlyAgb
            | Phase::ThermallyPulsingAgb
            | Phase::HeliumMainSequence
            | Phase::HeliumHertzsprungGap
            | Phase::HeliumGiantBranch
            | Phase::PostAgb => {
                let m_bol = absolute_bolometric_magnitude(state.luminosity())?;
                let bc = giant_bolometric_correction_v_under(
                    state.effective_temperature(),
                    state.surface_gravity()?,
                    rules,
                )?;
                Some(m_bol - bc)
            }
            Phase::Protostar | Phase::PreMainSequence | Phase::MainSequence | Phase::Substellar => {
                let m_bol = absolute_bolometric_magnitude(state.luminosity())?;
                let bc = bolometric_correction_v_under(state.effective_temperature(), rules)?;
                Some(m_bol - bc)
            }
        },
    }
}

/// The V-band bolometric correction of a cool giant or supergiant at effective temperature `teff`
/// and surface gravity `log_g` (log₁₀ g in cm s⁻², as [`StarState::surface_gravity`] gives it):
/// plan 06's gravity-dependent correction for ask A4 (P06.T43.a), which [`absolute_magnitude_v`]
/// reads for evolved stars from version 26 (`V_MAGNITUDE_VERSION`).
///
/// - To 5,000 K it is Worthey and Lee's (2011) correction at solar abundance at `log_g` from 1.0
///   up, and at log g 0.0 and below Levesque et al.'s (2005, Table 6) column for red supergiants
///   from 3,400 to 4,300 K, carried beyond those temperatures by its offset from Worthey and Lee's
///   correction at the nearer end; linear in log g between 0.0 and 1.0.
/// - From 6,000 K it is the dwarf table's, [`bolometric_correction_v`], and between 5,000 and
///   6,000 K the two are blended linearly in temperature.
///
/// Worthey and Lee's correction is linear in log g between their gravities at a fixed V − K, as
/// their program has it, with log g held at 0.0 below and 5.5 above (the program extrapolates);
/// then linear in θ = 5,040 K ÷ T<sub>eff</sub> along their rows, and extrapolated in θ below their
/// coolest (2,650–2,690 K from log g 0.0 to 1.0), as the program does. Each source is moved onto
/// this module's zero point. The correction reads no abundance: it is the table's [Fe/H] = 0, as a
/// state carries none. At a dwarf's gravity it gives Worthey and Lee's dwarfs, which lie above the
/// mean dwarf sequence by 0.1–0.2 mag from 3,400 to 4,000 K, 0.3–0.45 mag from 2,800 to 3,200 K
/// and up to 1.0 mag near 2,600 K: it is for evolved stars.
///
/// `None` where the dwarf table has no correction (below 1,710 K, or a temperature that is not
/// finite and positive), and for a gravity that is not finite.
///
/// # Examples
///
/// ```
/// use hyperion_sim::stellar::photometry::{bolometric_correction_v, giant_bolometric_correction_v};
/// use hyperion_sim::units::{Dex, Kelvin};
///
/// // An M6 giant (3,250 K, log g 0.5) is more than a magnitude fainter in V than the dwarf
/// // table's correction at its temperature makes it.
/// let t = Kelvin::new(3_250.0);
/// let giant = giant_bolometric_correction_v(t, Dex::new(0.5)).ok_or("finite")?;
/// let dwarf = bolometric_correction_v(t).ok_or("tabulated")?;
/// assert!(dwarf.value() - giant.value() > 1.2);
/// // From 6,000 K a giant's correction is the dwarf table's.
/// let warm = Kelvin::new(6_500.0);
/// let giant = giant_bolometric_correction_v(warm, Dex::new(2.0)).ok_or("finite")?;
/// let dwarf = bolometric_correction_v(warm).ok_or("tabulated")?;
/// assert!(giant.total_cmp(&dwarf).is_eq());
/// # Ok::<(), &str>(())
/// ```
#[must_use]
pub fn giant_bolometric_correction_v(teff: Kelvin, log_g: Dex) -> Option<Magnitudes> {
    giant_bolometric_correction_v_under(teff, log_g, StellarRules::of_version(GENERATOR_VERSION))
}

/// [`giant_bolometric_correction_v`] handed over to the dwarf sequence the stellar stage's `rules`
/// choose ([`DwarfSequence::of`]).
#[must_use]
pub(crate) fn giant_bolometric_correction_v_under(
    teff: Kelvin,
    log_g: Dex,
    rules: StellarRules,
) -> Option<Magnitudes> {
    let dwarf = bolometric_correction_v_under(teff, rules)?;
    let (teff_k, g) = (teff.value(), log_g.value());
    if !g.is_finite() {
        return None;
    }
    let (cool, warm) = GIANT_HANDOVER_K;
    if teff_k >= warm {
        return Some(dwarf);
    }
    let giant = cool_giant_bolometric_correction_v(teff_k, g);
    if teff_k <= cool {
        return Some(Magnitudes::new(giant));
    }
    let w = (teff_k - cool) / (warm - cool);
    Some(Magnitudes::new((1.0 - w) * giant + w * dwarf.value()))
}

/// [`giant_bolometric_correction_v`] below its hand-over to the dwarf table, mag on this module's
/// zero point, at `teff_k` (K, at least 1,710) and `log_g` (cm s⁻², finite).
#[must_use]
fn cool_giant_bolometric_correction_v(teff_k: f64, log_g: f64) -> f64 {
    let giants = wl11::bc_v_at(teff_k, log_g) + WL11_TO_MODULE.value();
    let (supergiant, giant) = SUPERGIANT_LOG_G;
    if log_g >= giant {
        return giants;
    }
    let edge = teff_k.clamp(SUPERGIANT_RANGE_K.0, SUPERGIANT_RANGE_K.1);
    let column = levesque05::bc_v_at(edge).expect("the range lies within Levesque et al.'s table")
        + LEVESQUE05_TO_MODULE.value();
    let offset = column - (wl11::bc_v_at(edge, supergiant) + WL11_TO_MODULE.value());
    let weight = ((giant - log_g) / (giant - supergiant)).min(1.0);
    giants + weight * offset
}

/// The V-band bolometric correction of a hydrogen-atmosphere white dwarf at effective temperature
/// `teff` and surface gravity `log_g` (log₁₀ g in cm s⁻²): the Montreal group's pure-hydrogen grid
/// (`Table_DA`, beside the sequences of Bédard et al. 2020; P06.T43.a, ask A4), on this module's
/// zero point (0.01 mag below the grid's, whose M<sub>bol</sub>☉ is 4.75).
///
/// Linear in log₁₀ T<sub>eff</sub> and in log g between the grid's nodes, a node's own value at a
/// node. Log g is held at 7.0 below the grid and at 9.0 above it, about 0.15 and 1.2 M☉ on the
/// radii of HPT's equation 91, whose oxygen–neon dwarfs reach log g 9.7 at 1.38 M☉; the correction
/// changes 0.03–0.04 mag per dex of log g at the grid's edges above 6,000 K, and up to 0.17–0.23
/// below 4,000 K. Above the grid's hottest,
/// 150,000 K, the correction falls at the Rayleigh–Jeans rate of 7.5 mag per dex from it, as
/// [`bolometric_correction_v`]'s does above O3V; the grid's own fall from 140,000 to 150,000 K is
/// 7.8 mag per dex at log g 8.0. Helium atmospheres are not modelled (see the [module](self)
/// documentation).
///
/// `None` below the grid's coolest, 1,500 K, for a temperature that is not finite and positive and
/// for a gravity that is not finite.
///
/// # Examples
///
/// ```
/// use hyperion_sim::stellar::photometry::{
///     bolometric_correction_v, white_dwarf_bolometric_correction_v,
/// };
/// use hyperion_sim::units::{Dex, Kelvin};
///
/// // A cool white dwarf's degenerate atmosphere is far from an M dwarf's of its temperature.
/// let t = Kelvin::new(4_000.0);
/// let white_dwarf = white_dwarf_bolometric_correction_v(t, Dex::new(8.0)).ok_or("on the grid")?;
/// let m_dwarf = bolometric_correction_v(t).ok_or("tabulated")?;
/// assert!((white_dwarf.value() + 0.41).abs() < 0.005);
/// assert!(white_dwarf.value() - m_dwarf.value() > 0.5);
/// // Below the grid's 1,500 K there is none.
/// assert_eq!(white_dwarf_bolometric_correction_v(Kelvin::new(1_000.0), Dex::new(8.0)), None);
/// # Ok::<(), &str>(())
/// ```
#[must_use]
pub fn white_dwarf_bolometric_correction_v(teff: Kelvin, log_g: Dex) -> Option<Magnitudes> {
    let (t, g) = (teff.value(), log_g.value());
    if !(t.is_finite() && t > 0.0 && g.is_finite()) {
        return None;
    }
    let hottest = montreal_da::hottest_k();
    let bc = if t > hottest {
        montreal_da::bc_v_at(hottest, g)?
            - RAYLEIGH_JEANS_BC_SLOPE * (math::log10(t) - math::log10(hottest))
    } else {
        montreal_da::bc_v_at(t, g)?
    };
    Some(Magnitudes::new(bc) + MONTREAL_TO_MODULE)
}

/// The absolute V magnitude of a white dwarf from the Montreal grid: M<sub>bol</sub> −
/// BC<sub>V</sub> with [`white_dwarf_bolometric_correction_v`] at its effective temperature and
/// surface gravity (P06.T43.a, ask A4), which [`absolute_magnitude_v`] reads from version 26
/// (`V_MAGNITUDE_VERSION`).
///
/// `None` for anything but a white dwarf, for one without luminosity, mass or radius, and below
/// the grid's 1,500 K.
///
/// # Examples
///
/// ```
/// use hyperion_sim::stellar::photometry::{absolute_magnitude_v, white_dwarf_absolute_magnitude_v};
/// use hyperion_sim::stellar::{Phase, StarState, StarStateParts};
/// use hyperion_sim::units::{SolarLuminosities, SolarMasses, SolarMassesPerYear, SolarRadii, Years};
///
/// // A 0.6 M☉ white dwarf of 0.0128 R☉ (log g 8.0) at 10,000 K.
/// let dwarf = StarState::new(StarStateParts {
///     phase: Phase::CarbonOxygenWhiteDwarf,
///     age: Years::new(1e9),
///     mass: SolarMasses::new(0.6),
///     core_mass: SolarMasses::new(0.6),
///     luminosity: SolarLuminosities::new(1.476e-3),
///     radius: SolarRadii::new(0.0128),
///     mass_loss_rate: SolarMassesPerYear::ZERO,
///     phase_fraction: 0.5,
/// });
/// let m_v = white_dwarf_absolute_magnitude_v(&dwarf).ok_or("on the grid")?;
/// assert!((m_v.value() - 12.18).abs() < 0.02);
/// // The version's rules give it none until the 26 bump.
/// assert_eq!(absolute_magnitude_v(&dwarf), None);
/// # Ok::<(), &str>(())
/// ```
#[must_use]
pub fn white_dwarf_absolute_magnitude_v(state: &StarState) -> Option<Magnitudes> {
    match state.phase() {
        Phase::HeliumWhiteDwarf | Phase::CarbonOxygenWhiteDwarf | Phase::OxygenNeonWhiteDwarf => {}
        Phase::Protostar
        | Phase::PreMainSequence
        | Phase::MainSequence
        | Phase::HertzsprungGap
        | Phase::FirstGiantBranch
        | Phase::CoreHeliumBurning
        | Phase::EarlyAgb
        | Phase::ThermallyPulsingAgb
        | Phase::HeliumMainSequence
        | Phase::HeliumHertzsprungGap
        | Phase::HeliumGiantBranch
        | Phase::PostAgb
        | Phase::Substellar
        | Phase::NeutronStar
        | Phase::BlackHole
        | Phase::NoRemnant => return None,
    }
    let m_bol = absolute_bolometric_magnitude(state.luminosity())?;
    let bc = white_dwarf_bolometric_correction_v(
        state.effective_temperature(),
        state.surface_gravity()?,
    )?;
    Some(m_bol - bc)
}

/// Whether the sky treats `state` as dark in V under the version's rules: a protostar (A3's
/// interim), a white dwarf (A4's interim, to version 25), a neutron star, a black hole, nothing
/// left, or any state plan 06's photometry gives no V magnitude (an object cooler than its table's
/// 1,710 K, or one of no luminosity).
#[must_use]
pub fn is_dark_in_v(state: &StarState) -> bool {
    absolute_v_of_state(state).is_none()
}

/// The absolute V magnitude the sky gives `state`, or `None` where it is dark in V
/// ([`is_dark_in_v`]): plan 06's [`absolute_magnitude_v`] for every other state.
///
/// # Examples
///
/// ```
/// use hyperion_sim::stellar::photometry::{absolute_v_of_state, is_dark_in_v};
/// use hyperion_sim::stellar::sse::Track;
/// use hyperion_sim::stellar::draws::StarDraws;
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::units::{SolarMasses, Years};
///
/// let sun = Track::full(SolarMasses::new(1.0), &Composition::SOLAR, &StarDraws::median());
/// // Deep in its envelope at 0.1 Myr, a solar-mass protostar shows nothing in V.
/// assert!(is_dark_in_v(&sun.state_at(Years::new(1e5))));
/// let today = absolute_v_of_state(&sun.state_at(Years::new(4.6e9))).ok_or("visible")?;
/// assert!((today.value() - 4.8).abs() < 0.3);
/// # Ok::<(), &str>(())
/// ```
#[must_use]
pub fn absolute_v_of_state(state: &StarState) -> Option<Magnitudes> {
    absolute_v_of_state_under(state, StellarRules::of_version(GENERATOR_VERSION))
}

/// [`absolute_v_of_state`] under the stellar stage's `rules`: what the sky census gives a star of
/// its galaxy, from the galaxy's rules. The sky's fitted tables take the version's.
///
/// A white dwarf reads [`absolute_magnitude_v_under`], which gives it none under A4's interim and
/// the Montreal grid's V under [`VMagnitudeRules::A4`]: the hold is chosen there alone.
#[must_use]
pub(crate) fn absolute_v_of_state_under(
    state: &StarState,
    rules: StellarRules,
) -> Option<Magnitudes> {
    match state.phase() {
        Phase::Protostar | Phase::NeutronStar | Phase::BlackHole | Phase::NoRemnant => None,
        Phase::HeliumWhiteDwarf
        | Phase::CarbonOxygenWhiteDwarf
        | Phase::OxygenNeonWhiteDwarf
        | Phase::PreMainSequence
        | Phase::MainSequence
        | Phase::HertzsprungGap
        | Phase::FirstGiantBranch
        | Phase::CoreHeliumBurning
        | Phase::EarlyAgb
        | Phase::ThermallyPulsingAgb
        | Phase::HeliumMainSequence
        | Phase::HeliumHertzsprungGap
        | Phase::HeliumGiantBranch
        | Phase::PostAgb
        | Phase::Substellar => absolute_magnitude_v_under(state, rules),
    }
}

/// `column` of `dwarfs` at `teff`, continued above the hottest point by `tail`, the change from
/// that point's value as a function of the temperature and the hottest point's, K.
#[must_use]
fn column_with_hot_tail(
    teff: Kelvin,
    dwarfs: DwarfSequence,
    column: Column,
    tail: impl Fn(f64, f64) -> f64,
) -> Option<Magnitudes> {
    let t = teff.value();
    if !(t.is_finite() && t > 0.0) {
        return None;
    }
    let hottest = dwarfs.hottest();
    let value = if t > hottest.teff_k {
        let at_hottest = match column {
            Column::BcV => hottest.bc_v,
            Column::ColourBV => hottest.colour_b_v,
        };
        at_hottest + tail(t, hottest.teff_k)
    } else {
        dwarfs.column_at(t, column)?
    };
    Some(Magnitudes::new(value))
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::bits;

    use super::*;
    use crate::stellar::classify::pm13;
    use crate::stellar::draws::StarDraws;
    use crate::stellar::sse::Track;
    use crate::stellar::{Composition, StarStateParts};
    use crate::units::consts::{GM_SUN, SOLAR_RADIUS_M};
    use crate::units::{SolarMasses, SolarMassesPerYear, SolarRadii, Years};

    fn state(phase: Phase, luminosity: f64, radius: f64) -> StarState {
        let mass = SolarMasses::new(1.0);
        StarState::new(StarStateParts {
            phase,
            age: Years::new(4.57e9),
            mass,
            core_mass: if phase.is_remnant() {
                mass
            } else {
                SolarMasses::ZERO
            },
            luminosity: SolarLuminosities::new(luminosity),
            radius: SolarRadii::new(radius),
            mass_loss_rate: SolarMassesPerYear::ZERO,
            phase_fraction: 0.5,
        })
    }

    /// The plan's figure: the Sun is G2V with M<sub>V</sub> = 4.81 ± 0.05. The table puts it at
    /// 4.825.
    #[test]
    fn the_suns_absolute_magnitude_is_4_8() {
        let m_v = absolute_magnitude_v(&state(Phase::MainSequence, 1.0, 1.0)).unwrap();
        assert!((m_v.value() - 4.81).abs() < 0.05, "{m_v:?}");
        assert!((m_v.value() - 4.8247).abs() < 1e-3, "{m_v:?}");
        let b_v = colour_b_v(Kelvin::new(5_772.0)).unwrap();
        assert!((b_v.value() - 0.65).abs() < 1e-3, "{b_v:?}");
    }

    /// At every row the table's own numbers come back, bit for bit, since the interpolation's
    /// fraction is then exactly zero.
    #[test]
    fn the_rows_are_reproduced_exactly() {
        for row in pm13::DWARF_SEQUENCE {
            let t = Kelvin::new(row.teff_k);
            let bc = bolometric_correction_v(t).map(|m| bits(m.value()));
            assert_eq!(bc, row.bc_v.map(bits), "{row:?}");
            let colour = colour_b_v(t).map(|m| bits(m.value()));
            assert_eq!(colour, row.colour_b_v.map(bits), "{row:?}");
        }
    }

    /// Continuous across the hottest row, and falling at the Rayleigh–Jeans rate beyond it.
    #[test]
    fn the_hot_tail_continues_the_table() {
        let edge = pm13::DWARF_SEQUENCE[0].teff_k;
        let below = bolometric_correction_v(Kelvin::new(edge * (1.0 - 1e-12))).unwrap();
        let above = bolometric_correction_v(Kelvin::new(edge * (1.0 + 1e-12))).unwrap();
        assert!((below.value() - above.value()).abs() < 1e-9);
        let ten_times = bolometric_correction_v(Kelvin::new(edge * 10.0)).unwrap();
        assert!((ten_times.value() - (-4.01 - 7.5)).abs() < 1e-9);
        let hot = colour_b_v(Kelvin::new(1e6)).map(|m| bits(m.value()));
        assert_eq!(hot, Some(bits(-0.330)));
    }

    /// Between two rows, a correction and a colour lie between the rows' own values, from the
    /// last row with a value to the hottest.
    #[test]
    fn values_lie_between_their_rows() {
        let rows = &pm13::DWARF_SEQUENCE;
        let mut t = 1_710.0_f64;
        while t < rows[0].teff_k {
            let hot = rows.iter().rposition(|r| r.teff_k >= t).unwrap();
            let (a, b) = (&rows[hot], &rows[(hot + 1).min(rows.len() - 1)]);
            let within = |value: f64, x: f64, y: f64| value >= x.min(y) && value <= x.max(y);
            let bc = bolometric_correction_v(Kelvin::new(t)).unwrap().value();
            assert!(
                within(bc, a.bc_v.unwrap(), b.bc_v.unwrap_or(bc)),
                "{t} K: {bc}"
            );
            if let Some(colour) = colour_b_v(Kelvin::new(t)) {
                let colour = colour.value();
                assert!(
                    within(colour, a.colour_b_v.unwrap(), b.colour_b_v.unwrap()),
                    "{t} K: {colour}"
                );
            }
            t *= 1.001;
        }
    }

    #[test]
    fn nothing_without_a_calibration_has_a_magnitude() {
        assert_eq!(bolometric_correction_v(Kelvin::new(1_700.0)), None);
        assert_eq!(colour_b_v(Kelvin::new(2_370.0)), None);
        assert_eq!(bolometric_correction_v(Kelvin::ZERO), None);
        assert_eq!(bolometric_correction_v(Kelvin::new(f64::NAN)), None);
        assert_eq!(colour_b_v(Kelvin::new(-5.0)), None);
        // A white dwarf of 10,000 K and a black hole, which has no luminosity at all.
        let wd = state(Phase::CarbonOxygenWhiteDwarf, 3e-3, 0.0126);
        assert!(wd.effective_temperature().value() > 9_000.0);
        assert_eq!(absolute_magnitude_v(&wd), None);
        assert_eq!(
            absolute_magnitude_v(&state(Phase::BlackHole, 0.0, 1e-5)),
            None
        );
        assert_eq!(
            absolute_bolometric_magnitude(SolarLuminosities::new(f64::INFINITY)),
            None
        );
    }

    /// A 1,700 K L dwarf is fainter in V than any table row gives, so it has no M<sub>V</sub>; one
    /// at L4 (1,870 K) does.
    #[test]
    fn brown_dwarfs_have_a_magnitude_only_where_the_table_gives_one() {
        let l4 = state(Phase::Substellar, math::exp10(-3.7), 0.1);
        let t = l4.effective_temperature().value();
        assert!(t > 1_710.0, "{t}");
        assert!(absolute_magnitude_v(&l4).is_some());
        let cooler = state(Phase::Substellar, math::exp10(-4.4), 0.09);
        assert!(cooler.effective_temperature().value() < 1_710.0);
        assert_eq!(absolute_magnitude_v(&cooler), None);
    }

    /// The correction [`white_dwarf_bolometric_correction_v`] gives at `t` (K) and `log_g`.
    fn wd_bc(t: f64, log_g: f64) -> Option<f64> {
        white_dwarf_bolometric_correction_v(Kelvin::new(t), Dex::new(log_g)).map(Magnitudes::value)
    }

    /// The correction [`giant_bolometric_correction_v`] gives at `t` (K) and `log_g`.
    fn giant_bc(t: f64, log_g: f64) -> f64 {
        giant_bolometric_correction_v(Kelvin::new(t), Dex::new(log_g))
            .unwrap()
            .value()
    }

    /// P06.T43.a: at each of the Montreal grid's 305 nodes a white dwarf of the node's luminosity
    /// (from its M<sub>bol</sub>, on the grid's M<sub>bol</sub>☉ = 4.75) reads the grid's V to its
    /// printed precision, the 0.001 mag of its last digit; and the correction is the grid's, 0.01
    /// mag lower, bit for bit.
    #[test]
    fn a_white_dwarfs_v_at_the_grids_nodes_is_the_grids_to_its_printed_precision() {
        let mut worst = 0.0_f64;
        for (k, &log_g) in montreal_da::LOG_G.iter().enumerate() {
            for (j, &t) in montreal_da::TEFF_K.iter().enumerate() {
                let bc = wd_bc(t, log_g).unwrap();
                assert_eq!(
                    bits(bc),
                    bits(montreal_da::BC_V[k][j] - 0.01),
                    "{t} K, {log_g}"
                );
                let (m_bol, v) = montreal_da::M_BOL_AND_V[k][j];
                let l = SolarLuminosities::new(math::exp10((4.75 - m_bol) / 2.5));
                let m_v = absolute_bolometric_magnitude(l).unwrap().value() - bc;
                worst = worst.max((m_v - v).abs());
            }
        }
        // The worst is 0.001 mag, at 73 of the 305 nodes: the last printed digit.
        assert!(worst <= 0.001 + 1e-9, "{worst}");
    }

    /// A white dwarf state reads [`white_dwarf_bolometric_correction_v`] at its own temperature
    /// and gravity, for a state built at a node and for one from a track; other phases read
    /// nothing.
    #[test]
    fn a_white_dwarfs_magnitude_reads_its_temperature_and_gravity() {
        // The node at 12,000 K and log g 8.0: R from L and T, M from g and R.
        let (teff, log_g, luminosity) = (12_000.0_f64, 8.0_f64, 3.0e-3_f64);
        let ratio = 5_772.0 / teff;
        let radius = ratio * ratio * luminosity.sqrt();
        let r_m = radius * SOLAR_RADIUS_M;
        let mass = math::exp10(log_g) / 100.0 * r_m * r_m / GM_SUN;
        let wd = StarState::new(StarStateParts {
            phase: Phase::CarbonOxygenWhiteDwarf,
            age: Years::new(1e9),
            mass: SolarMasses::new(mass),
            core_mass: SolarMasses::new(mass),
            luminosity: SolarLuminosities::new(luminosity),
            radius: SolarRadii::new(radius),
            mass_loss_rate: SolarMassesPerYear::ZERO,
            phase_fraction: 0.5,
        });
        assert!((wd.effective_temperature().value() - teff).abs() < 1e-9);
        assert!((wd.surface_gravity().unwrap().value() - log_g).abs() < 1e-12);
        let m_v = white_dwarf_absolute_magnitude_v(&wd).unwrap().value();
        let expected = absolute_bolometric_magnitude(wd.luminosity())
            .unwrap()
            .value()
            - wd_bc(teff, log_g).unwrap();
        assert!((m_v - expected).abs() < 1e-9, "{m_v} {expected}");
        // A 1 M☉ star's white dwarf at 15 Gyr, on the grid.
        let sun = Track::full(
            SolarMasses::new(1.0),
            &Composition::SOLAR,
            &StarDraws::median(),
        );
        let late = sun.state_at(Years::new(1.5e10));
        assert_eq!(late.phase(), Phase::CarbonOxygenWhiteDwarf);
        let gravity = late.surface_gravity().unwrap().value();
        assert!((7.5..8.5).contains(&gravity), "{gravity}");
        let m_v = white_dwarf_absolute_magnitude_v(&late).unwrap().value();
        let expected = absolute_bolometric_magnitude(late.luminosity())
            .unwrap()
            .value()
            - wd_bc(late.effective_temperature().value(), gravity).unwrap();
        assert_eq!(bits(m_v), bits(expected));
        assert!((10.0..18.0).contains(&m_v), "{m_v}");
        for state in [
            sun.state_at(Years::new(4.6e9)),
            state(Phase::NeutronStar, 1e-2, 1.4e-5),
            state(Phase::BlackHole, 0.0, 1e-5),
        ] {
            assert_eq!(white_dwarf_absolute_magnitude_v(&state), None, "{state:?}");
        }
    }

    /// Below the grid's 1,500 K there is no correction; above its 150,000 K the correction falls
    /// from the grid's at the Rayleigh–Jeans 7.5 mag per dex, continuously; log g is held at the
    /// grid's 7.0 and 9.0 beyond them; between nodes the correction lies between theirs.
    #[test]
    fn the_white_dwarf_correction_is_held_or_continued_beyond_the_grid() {
        assert_eq!(wd_bc(1_499.0, 8.0), None);
        assert!(wd_bc(1_500.0, 8.0).is_some());
        assert_eq!(wd_bc(f64::NAN, 8.0), None);
        assert_eq!(wd_bc(0.0, 8.0), None);
        assert_eq!(wd_bc(10_000.0, f64::INFINITY), None);
        let edge = 150_000.0;
        let below = wd_bc(edge * (1.0 - 1e-12), 8.0).unwrap();
        let above = wd_bc(edge * (1.0 + 1e-12), 8.0).unwrap();
        assert!((below - above).abs() < 1e-9);
        let twice = wd_bc(2.0 * edge, 8.0).unwrap();
        let expected = wd_bc(edge, 8.0).unwrap() - 7.5 * math::log10(2.0);
        assert!((twice - expected).abs() < 1e-12, "{twice} {expected}");
        for t in [4_000.0, 10_000.0, 60_000.0] {
            assert_eq!(bits(wd_bc(t, 6.5).unwrap()), bits(wd_bc(t, 7.0).unwrap()));
            assert_eq!(bits(wd_bc(t, 9.6).unwrap()), bits(wd_bc(t, 9.0).unwrap()));
        }
        // Between nodes: 10,250 K and log g 8.25 lies within its four corners.
        let corners = [
            wd_bc(10_000.0, 8.0).unwrap(),
            wd_bc(10_500.0, 8.0).unwrap(),
            wd_bc(10_000.0, 8.5).unwrap(),
            wd_bc(10_500.0, 8.5).unwrap(),
        ];
        let mid = wd_bc(10_250.0, 8.25).unwrap();
        let (lo, hi) = corners
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &c| {
                (a.min(c), b.max(c))
            });
        assert!(lo <= mid && mid <= hi, "{mid} not in {lo}..{hi}");
    }

    /// P06.T43.a: up to 5,000 K and from log g 1.0 the giants' correction is Worthey and Lee's
    /// table at its rows, 0.005 mag higher, at three gravities. The rows quoted are the table's as
    /// printed (`mash2.out`, [Fe/H] = 0), which also checks their transcription.
    #[test]
    fn the_giants_correction_is_worthey_and_lees_at_three_gravities() {
        for (g, rows) in [
            (
                1.0,
                [(3_256.0, -3.576), (3_760.0, -1.313), (4_505.0, -0.520)],
            ),
            (
                2.0,
                [(3_240.0, -3.232), (3_734.0, -1.321), (4_499.0, -0.531)],
            ),
            (
                3.0,
                [(3_244.0, -2.183), (3_756.0, -1.192), (4_498.0, -0.545)],
            ),
        ] {
            for (t, bc) in rows {
                assert_eq!(bits(giant_bc(t, g)), bits(bc + 0.005), "{t} K, log g {g}");
            }
            let k = wl11::LOG_G
                .iter()
                .position(|&x| (x - g).abs() < 1e-12)
                .unwrap();
            for &(t, bc) in wl11::ROWS[k].iter().filter(|(t, _)| *t <= 5_000.0) {
                assert_eq!(bits(giant_bc(t, g)), bits(bc + 0.005), "{t} K, log g {g}");
            }
        }
    }

    /// P06.T43.a: at log g 0.0 and below, from 3,400 to 4,300 K, the correction is Levesque et
    /// al.'s column, 0.015 mag lower; beyond those temperatures its offset from Worthey and Lee's
    /// log g 0.0 correction at the nearer end is held; at log g 0.5 half of it applies.
    #[test]
    fn supergiants_take_levesques_column() {
        for &(t, bc) in levesque05::MARCS_BC_V.iter().filter(|(t, _)| *t >= 3_400.0) {
            for g in [0.0, -0.5, -1.0] {
                let ours = giant_bc(t, g);
                assert!((ours - (bc - 0.015)).abs() < 1e-12, "{t} K, {g}: {ours}");
            }
        }
        let wl = |t: f64| wl11::bc_v_at(t, 0.0) + 0.005;
        let offset_cool = (-2.81 - 0.015) - wl(3_400.0);
        let offset_warm = (-0.61 - 0.015) - wl(4_300.0);
        assert!((offset_cool - 0.006).abs() < 5e-4, "{offset_cool}");
        assert!((offset_warm - 0.044).abs() < 5e-4, "{offset_warm}");
        for t in [2_800.0, 3_200.0, 3_300.0] {
            assert!(
                (giant_bc(t, 0.0) - (wl(t) + offset_cool)).abs() < 1e-12,
                "{t}"
            );
        }
        for t in [4_400.0, 4_900.0] {
            assert!(
                (giant_bc(t, -0.3) - (wl(t) + offset_warm)).abs() < 1e-12,
                "{t}"
            );
        }
        let half = giant_bc(3_600.0, 0.5);
        let expected = wl11::bc_v_at(3_600.0, 0.5) + 0.005 + 0.5 * ((-1.75 - 0.015) - wl(3_600.0));
        assert!((half - expected).abs() < 1e-12, "{half} {expected}");
    }

    /// The giants' correction has no jump in temperature (2,000–8,000 K, across the hand-over to
    /// the dwarf table and the ends of Levesque et al.'s range) or in gravity (log g −1.5 to 6.5,
    /// across Worthey and Lee's gravities and the supergiants' band), and from 6,000 K it is the
    /// dwarf table's.
    #[test]
    fn the_giants_correction_is_continuous_and_meets_the_dwarf_table() {
        for g in [-1.0, 0.0, 0.3, 0.5, 1.0, 1.7, 2.5, 3.5, 4.5, 6.0] {
            let mut t = 2_000.0_f64;
            let mut last = giant_bc(t, g);
            while t < 8_000.0 {
                t *= 1.000_5;
                let next = giant_bc(t, g);
                assert!(
                    (next - last).abs() < 0.02,
                    "log g {g} at {t} K: {last} to {next}"
                );
                last = next;
            }
        }
        for t in [
            2_500.0, 3_000.0, 3_250.0, 3_400.0, 3_700.0, 4_300.0, 4_700.0, 5_500.0,
        ] {
            let mut g = -1.5_f64;
            let mut last = giant_bc(t, g);
            while g < 6.5 {
                g += 1e-3;
                let next = giant_bc(t, g);
                assert!(
                    (next - last).abs() < 5e-3,
                    "{t} K at log g {g}: {last} to {next}"
                );
                last = next;
            }
        }
        for t in [6_000.0, 6_500.0, 10_000.0, 30_000.0, 60_000.0] {
            let dwarf = bolometric_correction_v(Kelvin::new(t)).map(|m| bits(m.value()));
            for g in [0.0, 2.0, 4.0] {
                assert_eq!(Some(bits(giant_bc(t, g))), dwarf, "{t} K, log g {g}");
            }
        }
        assert_eq!(
            giant_bolometric_correction_v(Kelvin::new(f64::NAN), Dex::new(1.0)),
            None
        );
        // Below the dwarf table's last correction, 1,710 K, there is none; above it Worthey and
        // Lee's coolest rows are extrapolated in θ, finitely.
        assert_eq!(
            giant_bolometric_correction_v(Kelvin::new(1_700.0), Dex::new(0.0)),
            None
        );
        assert_eq!(
            giant_bolometric_correction_v(Kelvin::new(1e-300), Dex::new(0.0)),
            None
        );
        assert!(giant_bc(1_710.0, 0.0).is_finite());
        assert_eq!(
            giant_bolometric_correction_v(Kelvin::new(3_000.0), Dex::new(f64::NAN)),
            None
        );
    }

    /// P06.T43.a's M6 III case of the module's note, before and after: at the class III
    /// temperatures and gravities of Straižys and Kuriliene (1981, Tables III and VII, as P06.T23.b
    /// read them from the paper's scan: log₁₀ T<sub>eff</sub> 3.562, 3.531 and 3.512, log g 1.12,
    /// 0.76 and 0.52), the dwarf table's correction (before) and the giants' (after), mag, so that
    /// M3, M5 and M6 giants read 0.16, 0.77 and 1.34 mag fainter in V; and Worthey and Lee's own
    /// giant-minus-dwarf difference at log g 4.5, 0.27, 0.95 and 1.59 mag, against the note's
    /// 0.1–0.5, 1.1 and 1.7.
    #[test]
    fn late_m_giants_read_fainter_by_the_giants_correction_than_by_the_dwarf_table() {
        for (name, log_t, g, before, after, own) in [
            ("M3 III", 3.562, 1.12, -1.4448, -1.6082, 0.265),
            ("M5 III", 3.531, 0.76, -2.0024, -2.7672, 0.953),
            ("M6 III", 3.512, 0.52, -2.3529, -3.6918, 1.589),
        ] {
            let t = math::exp10(log_t);
            let dwarf = bolometric_correction_v(Kelvin::new(t)).unwrap().value();
            let giant = giant_bc(t, g);
            assert!((dwarf - before).abs() < 5e-4, "{name} before: {dwarf}");
            assert!((giant - after).abs() < 5e-4, "{name} after: {giant}");
            let wl11_own = wl11::bc_v_at(t, 4.5) - wl11::bc_v_at(t, g);
            assert!(
                (wl11_own - own).abs() < 5e-4,
                "{name} in Worthey and Lee: {wl11_own}"
            );
        }
        // M6 III is 1.34 mag fainter in V than this module now makes it.
        let t = math::exp10(3.512);
        let shift = bolometric_correction_v(Kelvin::new(t)).unwrap().value() - giant_bc(t, 0.52);
        assert!((shift - 1.339).abs() < 1e-3, "{shift}");
    }

    #[test]
    fn protostars_and_white_dwarfs_are_dark_and_main_sequence_stars_are_not() {
        let sun = Track::full(
            SolarMasses::new(1.0),
            &Composition::SOLAR,
            &StarDraws::median(),
        );
        for age in [1.0, 1e4, 1e5, 4.9e5] {
            let state = sun.state_at(Years::new(age));
            assert_eq!(state.phase(), Phase::Protostar, "age {age}");
            assert!(is_dark_in_v(&state));
        }
        let main_sequence = sun.state_at(Years::new(4.6e9));
        assert_eq!(main_sequence.phase(), Phase::MainSequence);
        assert!(!is_dark_in_v(&main_sequence));
        let white_dwarf = sun.state_at(Years::new(1.5e10));
        assert_eq!(white_dwarf.phase(), Phase::CarbonOxygenWhiteDwarf);
        assert!(is_dark_in_v(&white_dwarf));
    }

    /// The determinism audit's pin for P06.T43.b (P06.T43.a's record): once the swap puts the two
    /// corrections on the record, the form of their interpolations becomes output, so both are
    /// pinned on a fixed grid off every source's nodes, under the version's rules. The giants'
    /// grid crosses the 1,710 K floor, the extrapolation below Worthey and Lee's coolest rows,
    /// both ends of Levesque et al.'s column (3,400 and 4,300 K), the supergiants' blend in log g
    /// (0.0–1.0), the hold beyond log g 5.5 and the hand-over to the dwarf table (5,000–6,000 K);
    /// the white dwarfs' crosses the grid's 1,500 K floor, its gravities held beyond 7.0 and 9.0,
    /// and its hot tail beyond 150,000 K.
    #[test]
    fn the_a4_corrections_are_pinned_off_their_nodes() {
        use hyperion_testkit::golden;
        use hyperion_testkit::golden::GoldenWriter;

        let mut w = GoldenWriter::new();
        w.header(GENERATOR_VERSION.get());
        let write = |w: &mut GoldenWriter, label: String, m: Option<Magnitudes>| match m {
            Some(m) => w.f64(&label, m.value()),
            None => w.line(&format!("{label} = none")),
        };
        w.line("");
        w.line("# giant_bolometric_correction_v, mag, at T_eff (K) and log g (cm s^-2)");
        for t in [
            1_705.0, 1_715.0, 2_013.0, 2_407.0, 2_611.0, 2_903.0, 3_251.0, 3_397.0, 3_409.0,
            3_777.0, 4_291.0, 4_313.0, 4_666.0, 4_999.0, 5_013.0, 5_487.0, 5_987.0, 6_013.0,
            6_513.0,
        ] {
            for g in [-0.31, 0.07, 0.37, 0.52, 0.93, 1.07, 2.31, 3.69, 4.93, 5.61] {
                let bc = giant_bolometric_correction_v(Kelvin::new(t), Dex::new(g));
                write(&mut w, format!("giant {t} K log g {g}"), bc);
            }
        }
        w.line("");
        w.line("# white_dwarf_bolometric_correction_v, mag, at T_eff (K) and log g (cm s^-2)");
        for t in [
            1_480.0, 1_520.0, 2_345.0, 4_107.0, 7_777.0, 13_333.0, 26_000.0, 61_000.0, 149_000.0,
            151_000.0, 300_000.0, 1e6,
        ] {
            for g in [6.8, 7.25, 7.93, 8.61, 9.3] {
                let bc = white_dwarf_bolometric_correction_v(Kelvin::new(t), Dex::new(g));
                write(&mut w, format!("white dwarf {t} K log g {g}"), bc);
            }
        }
        golden!("stellar/a4_corrections", w.as_str());
    }
}
