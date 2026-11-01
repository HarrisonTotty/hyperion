//! Classes beyond the MK grid that follow from a star's state and track (plan 06, P06.T24.a and
//! T24.c): Wolf-Rayet stars, hot subdwarfs, stripped helium stars and luminous blue variables, all
//! derived and none rolled.
//!
//! - **Wolf-Rayet stars** are hot, luminous and nearly or fully stripped: above a luminosity floor
//!   of 10⁴·⁹ L☉ × (Z ÷ 0.02)^−0.4 ([`wolf_rayet_floor`]) and at 30,000 K or more
//!   ([`WOLF_RAYET_MIN_TEFF`]). A hydrogen-rich star is `WNh` while its envelope is under 10% of
//!   its mass, or where its electron-scattering Eddington factor at its surface hydrogen reaches
//!   0.5, the onset of Wolf-Rayet-type winds of Gräfener et al. (2011, A&A 535, A56, section 2;
//!   ruling 124.1: "the Wolf-Rayet stage should be identified by large Eddington parameters"). A
//!   naked helium star is `WN` until it has lost, as a helium star, the layer above its helium
//!   core's largest convective extent ([`carbon_shows_after`]), and `WC` after; `WO` from the
//!   helium Hertzsprung gap on above 10⁵ K. The subtype is read from the temperature on the
//!   stellar temperatures T* of the Potsdam models ([`WN_SUBTYPES`], [`WC_SUBTYPES`],
//!   [`WO_SUBTYPES`]).
//! - **Hot subdwarfs** are the naked helium stars below the floor, from 20,000 K, of up to 1.7 M☉
//!   ([`HOT_SUBDWARF_MAX_MASS`]): `sdB` below 40,000 K and `sdO` above (Heber 2016, PASP 128,
//!   082001, section 2). A single star reaches one only by losing its envelope on the giant
//!   branch; plan 11's stripped stars are the rest.
//! - **Stripped helium stars** are the heavier naked helium stars between the hot subdwarfs and the
//!   Wolf-Rayet floor (P06.T24.c), Drout et al.'s (2023, Science 382, 1287) "intermediate-mass
//!   helium stars": `He-sdO` and `He-sdB` on the hot subdwarfs' letter rule (Götberg et al. 2018,
//!   A&A 615, A78, sections 4 and 4.2; Heber 2016, section 2).
//! - **Luminous blue variables** lie beyond the Humphreys–Davidson limit as Hurley, Pols and Tout
//!   (2000, section 7.1) write it, and are hotter than 8,000 K ([`is_luminous_blue_variable`]);
//!   [`Track::window_where`](crate::stellar::sse::Track::window_where) gives the ages at which a
//!   star is one.
//!
//! The Wolf-Rayet temperatures are hydrostatic ones, which for helium stars run hotter than the
//! observed T* at τ = 20 (Crowther 2007, section 3.3), so the backbone's Wolf-Rayet stars are
//! mostly early types (a finding recorded in plan 06's Risks).
//!
//! P06.T24.b adds two families:
//!
//! - **Young stars** ([`young_class`]): on its contraction to the main sequence, after the
//!   protostar phase, a star of under 2 M☉
//!   ([`T_TAURI_MAX_MASS`]) is a T Tauri star, classical while its protostellar disc lasts
//!   ([`disc_lifetime`], P06.T15.c) and weak-lined after, and one of 2–8 M☉
//!   ([`HERBIG_AE_BE_MAX_MASS`]) a Herbig Ae/Be star (Herbig 1960, ApJS 4, 337; Waters and
//!   Waelkens 1998, ARA&A 36, 233, recalled).
//! - **S and carbon stars** ([`DredgeUp`], [`chemical_class`]): on the thermally pulsing AGB,
//!   third dredge-up raises the surface C/O by number as C/O = (C/O)₀ + δ × max(0, n − `n_min`),
//!   with n the star's pulse number (P06.T28.f), δ the C/O each later pulse adds and zero outside
//!   the initial masses where dredge-up works and hot bottom burning does not
//!   ([`carbon_star_window`]). Below C/O 0.95 the star stays an M star; MS, S and SC take
//!   0.95–1 and a carbon star the rest.
//!
//! [`disc_lifetime`]: crate::stellar::premain::disc_lifetime

use core::fmt;
use core::ops::Range;

use crate::math;
use crate::stellar::draws::StarDraws;
use crate::stellar::premain::disc_lifetime;
use crate::stellar::rules::StellarRules;
use crate::stellar::sse::{Track, wind};
use crate::stellar::{Composition, Phase, StarState};
use crate::substance::Element;
use crate::units::{Kelvin, SolarLuminosities, SolarMasses, Years};

use super::{Classification, LuminosityClass, PeculiarClass, SpectralCode, SpectralType};

/// The least luminosity of a luminous blue variable, 6 × 10⁵ L☉: the luminosity half of the
/// Humphreys–Davidson limit as Hurley, Pols and Tout (2000, section 7.1) write it, after Humphreys
/// and Davidson (1994, PASP 106, 1025).
pub const LBV_MIN_LUMINOSITY: SolarLuminosities = SolarLuminosities::new(6.0e5);

/// The radius half of the Humphreys–Davidson limit, 10⁻⁵ R L^½ > 1 with R in R☉ and L in L☉
/// (Hurley, Pols and Tout 2000, section 7.1): per R☉ L☉^½.
const LBV_RADIUS_SCALE: f64 = 1e-5;

/// The coolest luminous blue variable, K: 8,000, the temperature of an S Doradus variable in
/// eruption, "∼7000–8000 K regardless of luminosity" (Humphreys and Davidson 1994, section 2.4),
/// below which a star beyond the limit is a cool hypergiant.
const LBV_MIN_TEFF: f64 = 8_000.0;

/// Whether a star in `state` is a luminous blue variable: living, beyond the Humphreys–Davidson
/// limit as Hurley, Pols and Tout (2000, section 7.1) write it (L above 6 × 10⁵ L☉ and
/// 10⁻⁵ R L^½ above 1), and hotter than 8,000 K (plan 06, P06.T24.a).
///
/// The limit is T(eff) below 5,772 K × (L ÷ 10⁵ L☉)^½: 14,100 K at 6 × 10⁵ L☉, 18,300 K at
/// 10⁶ L☉. It is the criterion of [`PhasePredicate::Lbv`](crate::stellar::sse::PhasePredicate),
/// whose window plan 09's catalogue class reads, and of the S Doradus cycles
/// ([`variability`](crate::stellar::variability::variability)).
#[must_use]
pub fn is_luminous_blue_variable(state: &StarState) -> bool {
    let (l, r) = (state.luminosity().value(), state.radius().value());
    state.phase().is_living()
        && l > LBV_MIN_LUMINOSITY.value()
        && LBV_RADIUS_SCALE * r * l.sqrt() > 1.0
        && state.effective_temperature().value() > LBV_MIN_TEFF
}

/// The Wolf-Rayet luminosity floor at Z = 0.02, 10⁴·⁹ L☉ (plan 06, P06.T24.a).
///
/// **Provisional, a finding:** the plan's floor is 10⁴·⁹ L☉ × (Z ÷ 0.02)^−0.4. The lowest
/// luminosities of observed single Wolf-Rayet stars are log L = 4.9, 5.25 and 5.6 in the Galaxy,
/// the LMC and the SMC (Shenar et al. 2020, A&A 634, A79, section 3), at Z = 0.014, 0.006 and
/// 0.002: 10⁴·⁹ L☉ × (Z ÷ 0.014)^−0.82, twice the plan's floor at the SMC's Z.
const WOLF_RAYET_FLOOR_L_SUN: f64 = 79_432.823_472_428_15;

/// The floor's slope in Z ÷ 0.02, −0.4 (plan 06, P06.T24.a; see [`WOLF_RAYET_FLOOR_L_SUN`]).
const WOLF_RAYET_FLOOR_Z_SLOPE: f64 = -0.4;

/// The luminosity above which a hot, nearly or fully stripped star of `composition` is a
/// Wolf-Rayet star: 10⁴·⁹ L☉ × (Z ÷ 0.02)^−0.4, with Z the metal fraction the formulae see
/// (plan 06, P06.T24.a; provisional, see [`WOLF_RAYET_FLOOR_L_SUN`]).
#[must_use]
pub(crate) fn wolf_rayet_floor(composition: &Composition) -> SolarLuminosities {
    SolarLuminosities::new(
        WOLF_RAYET_FLOOR_L_SUN
            * math::powf(composition.z_fit().value() / 0.02, WOLF_RAYET_FLOOR_Z_SLOPE),
    )
}

/// The coolest Wolf-Rayet star, K: 30,000, below the WN9 stars' stellar temperatures of 32,000
/// (Crowther 2007, ARA&A 45, 177, table 2) to 38,000 K (the median of Hamann et al. 2019, A&A
/// 625, A57, table 1), where the WN9–11 stars pass to the Ofpe stars (the lane's choice).
pub const WOLF_RAYET_MIN_TEFF: Kelvin = Kelvin::new(30_000.0);

/// The share of a hydrogen-rich star's mass below which its envelope makes it a `WNh` star: 10%
/// (plan 06, P06.T24.a).
const WNH_ENVELOPE_SHARE: f64 = 0.1;

/// The electron-scattering Eddington factor, at the star's surface hydrogen, from which a
/// hydrogen-rich star has Wolf-Rayet-type winds and is a `WNh` star: 0.5, where "Γ approaches
/// unity in deep atmospheric layers" and "the onset of WR-type mass loss thus occurs" (Gräfener et
/// al. 2011, section 2, after Gräfener and Hamann 2008). Vink et al.'s (2011) kink at 0.7 is the
/// upper bracket (the lane's choice, provisional).
const WNH_EDDINGTON: f64 = 0.5;

/// The coolest hot subdwarf, K: 20,000, the cool end of the extended horizontal branch that sdB
/// stars populate (Heber 2016, section 2.2). The coolest stripped helium star too.
const HOT_SUBDWARF_MIN_TEFF: f64 = 20_000.0;

/// The heaviest hot subdwarf, 1.7 M☉ (P06.T24.c): a heavier naked helium star below the Wolf-Rayet
/// floor is a stripped helium star (`stripped_helium_star`).
///
/// Götberg et al. (2018, A&A 615, A78, abstract and section 7, item 4) find stripped stars'
/// spectra "undistinguishable from subdwarfs" only up to 1.7 M☉ (log L ≤ 3.2), and Drout et al.
/// (2023, Science 382, 1287) bracket it, with the subdwarfs at ≲ 1.5 M☉ and their
/// "intermediate-mass helium stars" at 2–8 M☉.
///
/// **Provisional at every Z:** Götberg's grid behind it is at Z = 0.014. It is an edge in mass,
/// not in luminosity: log L = 3.2 falls at 1.76, 1.58 and 1.37 M☉ across the helium main sequence
/// (Hurley, Pols and Tout 2000, equations 77 and 80), so an edge in luminosity would relabel a
/// star in the middle of its life.
pub const HOT_SUBDWARF_MAX_MASS: SolarMasses = SolarMasses::new(1.7);

/// The temperature from which a hot subdwarf or a stripped helium star is an sdO star rather than
/// an sdB, K: 40,000, above which He II lines dominate over He I lines in stripped stars' spectra
/// (Götberg et al. 2018, section 4.3), the letter of the subdwarf classes (Heber 2016, section 2).
const SD_O_MIN_TEFF: f64 = 40_000.0;

/// The helium Hertzsprung gap's or giant branch's temperature from which a Wolf-Rayet star is a
/// WO star, K: 10⁵ (plan 06, P06.T24.a).
///
/// **Provisional, a finding:** on the stellar temperature T* that the hydrostatic backbone's is
/// closest to, WO stars are 150,000–210,000 K (Tramper et al. 2015, A&A 581, A110, table 4) and
/// WC4 stars already 117,000 K (Sander et al. 2019, A&A 621, A92, table 5); 10⁵ K is the WO
/// stars' temperature at τ = 2/3 (Aadland et al. 2022, ApJ 931, 157).
const WO_MIN_TEFF: f64 = 1e5;

/// The WN subtypes' stellar temperatures T*, kK, WN2 to WN9: the medians of Hamann et al.'s (2019,
/// table 1) single Galactic WN stars, WN3 raised to the mean of its three stars (94) and WN9
/// lowered to 36 between Hamann's 38 and Crowther's (2007, table 2) 32, to keep the scale
/// monotone.
const WN_SUBTYPES: [(u8, f64); 8] = [
    (2, 141.0),
    (3, 94.0),
    (4, 89.0),
    (5, 63.0),
    (6, 56.0),
    (7, 47.0),
    (8, 45.0),
    (9, 36.0),
];

/// The WC subtypes' stellar temperatures T*, kK, WC4 to WC9 (Sander et al. 2019, table 5).
const WC_SUBTYPES: [(u8, f64); 6] = [
    (4, 117.0),
    (5, 83.0),
    (6, 78.0),
    (7, 71.0),
    (8, 60.0),
    (9, 44.0),
];

/// The WO subtypes' stellar temperatures T*, kK, WO1 to WO4: after Tramper et al.'s (2015,
/// table 4) six WO stars, 150,000 K for WO4 to 210,000 K and above for WO1, and Sander et al.'s
/// (2019) 200,000 K for WO2 (the lane's reading, provisional).
const WO_SUBTYPES: [(u8, f64); 4] = [(1, 220.0), (2, 200.0), (3, 165.0), (4, 150.0)];

/// The largest helium-star mass, M☉, at which [`carbon_shows_after`]'s fit is read: 60, the heaviest
/// of Langer's (1989a) models; heavier helium stars take its value there.
const CARBON_FIT_MAX_MASS: f64 = 60.0;

/// (M₀, p) of [`carbon_shows_after`]'s fit f = 1 ÷ (1 + (M ÷ M₀)^p).
const CARBON_FIT: (f64, f64) = (6.30, 0.855);

/// A Wolf-Rayet star's sequence: nitrogen, carbon or oxygen lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum WolfRayetSequence {
    /// A WN star: helium and nitrogen, the CNO-processed layers; with hydrogen, a `WNh` star.
    Nitrogen,
    /// A WC star: helium, carbon and oxygen, the layers once in the helium-burning core.
    Carbon,
    /// A WO star: the hottest, oxygen-rich, after core helium exhaustion.
    Oxygen,
}

/// A Wolf-Rayet star's spectral type: its sequence, subtype and whether it shows hydrogen, written
/// `WN6`, `WN7h`, `WC5` or `WO2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WolfRayetType {
    sequence: WolfRayetSequence,
    subtype: u8,
    hydrogen: bool,
}

impl WolfRayetType {
    /// The sequence.
    #[must_use]
    pub const fn sequence(&self) -> WolfRayetSequence {
        self.sequence
    }

    /// The subtype: 2–9 for WN, 4–9 for WC, 1–4 for WO.
    #[must_use]
    pub const fn subtype(&self) -> u8 {
        self.subtype
    }

    /// Whether the star shows hydrogen: a `WNh` star.
    #[must_use]
    pub const fn shows_hydrogen(&self) -> bool {
        self.hydrogen
    }

    /// The type of `sequence` and `subtype`, with hydrogen for a `WNh` star, or `None` if the
    /// subtype is outside the sequence's range or hydrogen is given outside WN.
    #[must_use]
    pub fn new(sequence: WolfRayetSequence, subtype: u8, hydrogen: bool) -> Option<Self> {
        let table: &[(u8, f64)] = match sequence {
            WolfRayetSequence::Nitrogen => &WN_SUBTYPES,
            WolfRayetSequence::Carbon => &WC_SUBTYPES,
            WolfRayetSequence::Oxygen => &WO_SUBTYPES,
        };
        let known = table.iter().any(|&(n, _)| n == subtype);
        (known && (!hydrogen || sequence == WolfRayetSequence::Nitrogen)).then_some(Self {
            sequence,
            subtype,
            hydrogen,
        })
    }
}

impl fmt::Display for WolfRayetType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let letter = match self.sequence {
            WolfRayetSequence::Nitrogen => 'N',
            WolfRayetSequence::Carbon => 'C',
            WolfRayetSequence::Oxygen => 'O',
        };
        write!(f, "W{letter}{}", self.subtype)?;
        if self.hydrogen {
            f.write_str("h")?;
        }
        Ok(())
    }
}

/// What a naked helium star's surface shows, which only its track knows: the helium and nitrogen
/// of its outer layers, or the carbon of the layers once in its convective core (plan 06,
/// P06.T24.a), handed to [`classify`](super::classify) through
/// [`ClassExtras`](super::ClassExtras).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HeliumSurface {
    /// The layers above the helium core's largest convective extent: a WN star.
    Nitrogen,
    /// The layers once in the convective core: a WC star.
    Carbon,
}

impl HeliumSurface {
    /// The surface of a naked helium star that entered its helium-star life with `entry_mass` and
    /// now has `mass`: carbon once the mass lost exceeds [`carbon_shows_after`] of the entry mass.
    #[must_use]
    pub fn of(entry_mass: SolarMasses, mass: SolarMasses) -> Self {
        let entry = entry_mass.value();
        if entry > 0.0 && entry - mass.value() > carbon_shows_after(entry_mass) * entry {
            Self::Carbon
        } else {
            Self::Nitrogen
        }
    }
}

/// The share of a naked helium star's initial mass `entry_mass` above its helium core's largest
/// convective extent, which it must lose before the carbon of its convective core shows: f =
/// 1 ÷ (1 + (M ÷ 6.30 M☉)^0.855), held at its 60 M☉ value above 60 M☉.
///
/// Langer (1989b, A&A 220, 135, section 3.1) gives the criterion, the surface staying helium-rich
/// while the mass exceeds the convective core's at the helium star's start, which a core that
/// recedes under mass loss (Woosley 2019, ApJ 878, 49, section 4.1) makes its largest extent. The
/// fit is to the convective cores of Langer's (1989a, A&A 210, 93, table 1) helium zero-age models
/// at 2–60 M☉ (f = 0.730 at 2 M☉ to 0.148 at 60), within 0.021 (rms 0.012), and agrees within
/// about 0.03 with the carbon's appearance in Woosley's (2019, table 4) and Yoon's (2017, MNRAS
/// 470, 3970, table 1) evolved helium stars.
#[must_use]
pub fn carbon_shows_after(entry_mass: SolarMasses) -> f64 {
    let (m0, p) = CARBON_FIT;
    let m = entry_mass.value().min(CARBON_FIT_MAX_MASS);
    1.0 / (1.0 + math::powf_positive(m / m0, p))
}

/// The subtype of `table` (subtype, T* in kK, hottest first) at temperature `teff` K: the nearest
/// subtype in log T*, held at the table's ends.
#[must_use]
fn subtype_at(table: &[(u8, f64)], teff: f64) -> u8 {
    let log_t = math::log10(teff / 1e3);
    let mut best = table[0];
    let mut distance = f64::INFINITY;
    for &(subtype, t) in table {
        let d = (math::log10(t) - log_t).abs();
        if d < distance {
            (best, distance) = ((subtype, t), d);
        }
    }
    best.0
}

/// The electron-scattering Eddington factor of `state` at the hydrogen fraction X = 0.76 − 3Z of
/// its composition (Hurley, Pols and Tout 2000, section 2), the surface hydrogen of a star whose
/// envelope the tracks do not follow: Γₑ(X = 0) × (1 + X).
#[must_use]
fn eddington_at_initial_hydrogen(state: &StarState, composition: &Composition) -> f64 {
    let x = 0.76 - 3.0 * composition.z_fit().value();
    wind::eddington_factor(state.luminosity(), state.mass()) * (1.0 + x)
}

/// The Wolf-Rayet type of a living star in `state`, of `composition`, whose helium-star surface
/// is `helium` where the track knows it, or `None` if it is not a Wolf-Rayet star (see the module
/// documentation). A helium star whose surface is not known shows nitrogen.
#[must_use]
pub(crate) fn wolf_rayet(
    state: &StarState,
    composition: &Composition,
    helium: Option<HeliumSurface>,
) -> Option<WolfRayetType> {
    let teff = state.effective_temperature().value();
    if !(teff >= WOLF_RAYET_MIN_TEFF.value() && state.luminosity() > wolf_rayet_floor(composition))
    {
        return None;
    }
    let (m, mc) = (state.mass().value(), state.core_mass().value());
    let (sequence, hydrogen) = match state.phase() {
        Phase::MainSequence
        | Phase::HertzsprungGap
        | Phase::FirstGiantBranch
        | Phase::CoreHeliumBurning
        | Phase::EarlyAgb
        | Phase::ThermallyPulsingAgb => {
            let stripped = mc > 0.0 && m - mc < WNH_ENVELOPE_SHARE * m;
            if !(stripped || eddington_at_initial_hydrogen(state, composition) >= WNH_EDDINGTON) {
                return None;
            }
            (WolfRayetSequence::Nitrogen, true)
        }
        Phase::HeliumMainSequence | Phase::HeliumHertzsprungGap | Phase::HeliumGiantBranch => {
            let shell_burning = state.phase() != Phase::HeliumMainSequence;
            if shell_burning && teff > WO_MIN_TEFF {
                (WolfRayetSequence::Oxygen, false)
            } else if helium == Some(HeliumSurface::Carbon) {
                (WolfRayetSequence::Carbon, false)
            } else {
                (WolfRayetSequence::Nitrogen, false)
            }
        }
        Phase::Protostar
        | Phase::PreMainSequence
        | Phase::PostAgb
        | Phase::HeliumWhiteDwarf
        | Phase::CarbonOxygenWhiteDwarf
        | Phase::OxygenNeonWhiteDwarf
        | Phase::NeutronStar
        | Phase::BlackHole
        | Phase::NoRemnant
        | Phase::Substellar => return None,
    };
    let table: &[(u8, f64)] = match sequence {
        WolfRayetSequence::Nitrogen => &WN_SUBTYPES,
        WolfRayetSequence::Carbon => &WC_SUBTYPES,
        WolfRayetSequence::Oxygen => &WO_SUBTYPES,
    };
    Some(WolfRayetType {
        sequence,
        subtype: subtype_at(table, teff),
        hydrogen,
    })
}

/// The hot subdwarf's classification of a naked helium star in `state` of `composition` below the
/// Wolf-Rayet floor, from 20,000 K and of up to [`HOT_SUBDWARF_MAX_MASS`], or `None`: `sd` and the
/// dwarf scale's type, held to B0 and cooler below 40,000 K (an sdB star) and to O above it (an
/// sdO star).
///
/// The type is read on the dwarf sequence the stellar stage's `rules` choose.
#[must_use]
pub(crate) fn hot_subdwarf(
    state: &StarState,
    composition: &Composition,
    rules: StellarRules,
) -> Option<Classification> {
    (state.mass() <= HOT_SUBDWARF_MAX_MASS)
        .then(|| helium_subdwarf(state, composition, PeculiarClass::HotSubdwarf, rules))
        .flatten()
}

/// The stripped helium star's classification of a naked helium star in `state` of `composition`
/// below the Wolf-Rayet floor, from 20,000 K and heavier than [`HOT_SUBDWARF_MAX_MASS`], or `None`
/// (P06.T24.c): the hot subdwarf's letter rule after a `He-` prefix, `He-sdO` from 40,000 K and
/// `He-sdB` below.
///
/// - **The class** is Götberg et al.'s (2018, A&A 615, A78) He-sdO: the `He-` prefix marks their
///   models' super-solar helium (section 4's classification scheme, after Stroeer et al. 2007,
///   A&A 462, 269), and the `sd` stays, since the high gravity makes the linear Stark broadening
///   of the H I and He II lines severe, as in a subdwarf (section 4.2). `He-sdB` is the
///   helium-rich subdwarfs' class where He I lines dominate (Heber 2016, section 2), here below
///   the letter rule's 40,000 K.
/// - **HYPERION's convention:** the literature also writes `He-sdO` and `He-sdB` for helium-rich
///   subdwarfs below the edge (Götberg et al. find them from 0.75 M☉, section 4.3), which read
///   `sdO` and `sdB` here; the prefix marks the stripped helium stars' class above it.
/// - **No `f`:** most of the stripped stars observed so far lose ≲ 10⁻⁹ M☉ yr⁻¹ (Götberg et al.
///   2023, ApJ 959, 125), too little wind for the He II 4686 emission an `f` marks (Götberg et al.
///   2018, section 4.2).
/// - **Neither `WN3/O3` nor a bare O type:** the LMC's WN3/O3 stars, to which Götberg et al.
///   (2018, section 4.2) liken their stripped stars with wind emission, are "about an order of
///   magnitude fainter than an O3-type star" (their discoverers' argument), and these show no
///   emission (no `f`).
///
/// Its upper end is the Wolf-Rayet floor ([`wolf_rayet_floor`]), whose 10⁴·⁹ L☉ falls at
/// 5.9–7.7 M☉ across the helium main sequence on Hurley, Pols and Tout's (2000) fits and at
/// 6.72 M☉ in Götberg et al.'s (2018) grid, against Shenar et al.'s (2020, A&A 634, A79) 7.5 M☉
/// and Drout et al.'s (2023) ~8 M☉.
///
/// The type is read on the dwarf sequence the stellar stage's `rules` choose.
#[must_use]
pub(crate) fn stripped_helium_star(
    state: &StarState,
    composition: &Composition,
    rules: StellarRules,
) -> Option<Classification> {
    (state.mass() > HOT_SUBDWARF_MAX_MASS)
        .then(|| helium_subdwarf(state, composition, PeculiarClass::StrippedHeliumStar, rules))
        .flatten()
}

/// The subdwarf classification of a naked helium star in `state` of `composition` below the
/// Wolf-Rayet floor and from 20,000 K, as `peculiar`, or `None`: `sd` and the dwarf scale's type,
/// held to B0 and cooler below 40,000 K (an sdB star) and to O above it (an sdO star).
#[must_use]
fn helium_subdwarf(
    state: &StarState,
    composition: &Composition,
    peculiar: PeculiarClass,
    rules: StellarRules,
) -> Option<Classification> {
    let teff = state.effective_temperature().value();
    let helium_star = matches!(
        state.phase(),
        Phase::HeliumMainSequence | Phase::HeliumHertzsprungGap | Phase::HeliumGiantBranch
    );
    if !(helium_star
        && teff >= HOT_SUBDWARF_MIN_TEFF
        && state.luminosity() <= wolf_rayet_floor(composition))
    {
        return None;
    }
    let dwarf = super::subtype_from_teff_under(Kelvin::new(teff), rules)?.value();
    // O ends below code 10 (B0): an sdB is held to B0 and cooler, an sdO to O9.5 and hotter.
    let code = if teff < SD_O_MIN_TEFF {
        dwarf.max(10.0)
    } else {
        dwarf.min(9.5)
    };
    let mut class =
        Classification::sequence(SpectralCode::new(code)?, Some(LuminosityClass::Subdwarf));
    class.peculiar = Some(peculiar);
    Some(class)
}

/// The classification of a Wolf-Rayet star of type `wr`.
#[must_use]
pub(crate) fn wolf_rayet_class(wr: WolfRayetType) -> Classification {
    let mut class = Classification::bare(SpectralType::WolfRayet(wr));
    class.peculiar = Some(PeculiarClass::WolfRayet);
    class
}

/// The heaviest T Tauri star, M☉ of initial mass, exclusive: 2, where the Herbig Ae/Be stars
/// begin (plan 06, P06.T24.b; Waters and Waelkens 1998, section 1, recalled). It is the 2 M☉ of
/// the FU Orionis outbursts' disc phase (P06.T28.d), which is the T Tauri stars' disc.
pub const T_TAURI_MAX_MASS: SolarMasses = SolarMasses::new(2.0);

/// The heaviest Herbig Ae/Be star, M☉ of initial mass, exclusive: 8 (plan 06, P06.T24.b), above
/// which a star reaches the main sequence still embedded (Palla and Stahler 1993, ApJ 418, 414,
/// recalled: the birthline meets the main sequence near 8 M☉).
///
/// In effect the class ends lower: from about 6.29 M☉ at solar Z a star arrives on the main
/// sequence ([`t_zams`](crate::stellar::premain::t_zams)) within the protostar's 0.5 Myr, and so
/// has no contraction to take it on.
pub const HERBIG_AE_BE_MAX_MASS: SolarMasses = SolarMasses::new(8.0);

/// The young star's class of a star of initial mass `initial_mass` and `draws` in `phase` at
/// `age` (P06.T24.b), or `None`: on its contraction to the main sequence, after the protostar's
/// accretion, a T Tauri star under [`T_TAURI_MAX_MASS`], classical while its age is under its
/// disc's lifetime ([`disc_lifetime`] of its own disc rank), and a Herbig Ae/Be star up to
/// [`HERBIG_AE_BE_MAX_MASS`].
///
/// A Class 0 or I protostar takes none: the brainstorm's row "Before the main sequence" has the
/// T Tauri and Herbig Ae/Be stars contracting after the protostar phase, as [`ObjectKind`] has a
/// pre-main-sequence star, and a classical T Tauri star is Class II (research-leans-12 §3).
///
/// [`ObjectKind`]: crate::stellar::ObjectKind
#[must_use]
pub(crate) fn young_class(
    phase: Phase,
    initial_mass: SolarMasses,
    age: Years,
    draws: &StarDraws,
) -> Option<PeculiarClass> {
    if phase != Phase::PreMainSequence {
        return None;
    }
    if initial_mass < T_TAURI_MAX_MASS {
        let disc = Years::new(disc_lifetime(initial_mass, draws.disc_lifetime()).value() * 1e6);
        Some(PeculiarClass::TTauri {
            classical: age < disc,
        })
    } else {
        (initial_mass < HERBIG_AE_BE_MAX_MASS).then_some(PeculiarClass::HerbigAeBe)
    }
}

/// An S-type star's place on the M–MS–S–SC–C sequence of rising C/O (Scalo and Ross 1976, A&A
/// 48, 219; Keenan and Boeshaar 1980, ApJS 43, 379, recalled).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SStarType {
    /// An MS star, an M star with the first `ZrO` bands: C/O from [`C_TO_O_MS`].
    Ms,
    /// An S star, with `ZrO` bands as strong as the `TiO`: C/O from [`C_TO_O_S`].
    S,
    /// An SC star, between the S and carbon stars: C/O from [`C_TO_O_SC`] to 1.
    Sc,
}

impl SStarType {
    /// The letters the type is written with: `MS`, `S`, `SC`.
    #[must_use]
    pub const fn letters(self) -> &'static str {
        match self {
            Self::Ms => "MS",
            Self::S => "S",
            Self::Sc => "SC",
        }
    }
}

/// The surface C/O by number from which an AGB star is an MS star: 0.95 (plan 06, P06.T24.b,
/// after Scalo and Ross 1976, recalled), below which it stays an M star.
pub const C_TO_O_MS: f64 = 0.95;

/// The C/O from which an AGB star is an S star: 0.96 (the lane's split of the plan's 0.95–1,
/// after Keenan and Boeshaar's (1980) C/O indices, recalled; provisional).
pub const C_TO_O_S: f64 = 0.96;

/// The C/O from which an AGB star is an SC star: 0.99 (the lane's split, as [`C_TO_O_S`]).
pub const C_TO_O_SC: f64 = 0.99;

/// The C/O above which an AGB star is a carbon star: 1 (plan 06, P06.T24.b), where carbon left
/// over from CO formation makes the C₂ and CN bands of a C-N star.
pub const CARBON_STAR_C_TO_O: f64 = 1.0;

/// The class a surface C/O of `c_to_o` gives a star of the thermally pulsing AGB (P06.T24.b):
/// `None` below [`C_TO_O_MS`], MS, S and SC up to [`CARBON_STAR_C_TO_O`], and a carbon star above.
#[must_use]
pub(crate) fn chemical_class(c_to_o: f64) -> Option<PeculiarClass> {
    if c_to_o > CARBON_STAR_C_TO_O {
        Some(PeculiarClass::Carbon)
    } else if c_to_o >= C_TO_O_SC {
        Some(PeculiarClass::SType(SStarType::Sc))
    } else if c_to_o >= C_TO_O_S {
        Some(PeculiarClass::SType(SStarType::S))
    } else if c_to_o >= C_TO_O_MS {
        Some(PeculiarClass::SType(SStarType::Ms))
    } else {
        None
    }
}

/// The initial masses where third dredge-up works and hot bottom burning does not, at
/// Z = 0.02: 1.5–4 M☉ (plan 06, P06.T24.b).
///
/// Below 1.5 M☉ dredge-up is too weak to make a carbon star of a solar-metallicity star, and from
/// 4 M☉ hot bottom burning turns the dredged carbon to nitrogen (Karakas and Lattanzio 2014, PASA
/// 31, e030, sections 3.4 and 3.5; Marigo et al. 2013, MNRAS 434, 488, recalled, not read at
/// source in this build).
const CARBON_STAR_WINDOW_SOLAR: (SolarMasses, SolarMasses) =
    (SolarMasses::new(1.5), SolarMasses::new(4.0));

/// How the window's edges move with Z: both as (Z ÷ 0.02)^0.1, 1.2–3.2 M☉ at Z = 0.002 and
/// 0.95–2.5 M☉ at 0.0002, lower at low Z as dredge-up's onset and hot bottom burning's both are
/// (the same sources; the lane's power, provisional).
const CARBON_STAR_WINDOW_Z_SLOPE: f64 = 0.1;

/// The initial masses, M☉, at which a star of `composition` can become an S or carbon star by
/// third dredge-up (P06.T24.b): 1.5–4 M☉ at Z = 0.02, both edges as (Z ÷ 0.02)^0.1 with Z the
/// metal fraction the formulae see.
#[must_use]
pub fn carbon_star_window(composition: &Composition) -> Range<SolarMasses> {
    let scale = math::powf_positive(
        composition.z_fit().value() / 0.02,
        CARBON_STAR_WINDOW_Z_SLOPE,
    );
    let (low, high) = CARBON_STAR_WINDOW_SOLAR;
    SolarMasses::new(low.value() * scale)..SolarMasses::new(high.value() * scale)
}

/// `n_min`, the last pulse number with no third dredge-up: 5, so that the first six pulses
/// (numbers 0–5) dredge nothing (the lane's, after the few pulses detailed models take to start
/// it, Karakas, Lattanzio and Pols 2002, PASA 19, 515, recalled, who start it by a core mass;
/// provisional).
pub const MIN_DREDGE_UP_PULSES: i64 = 5;

/// (C/O)₀, the surface C/O by number on the thermally pulsing AGB before third dredge-up: 0.7 of
/// the protosolar 0.457 (Lodders 2010, the element registry's), first dredge-up having taken some
/// 30% of the surface ¹²C (Karakas and Lattanzio 2014, section 3.1, recalled).
const FIRST_DREDGE_UP_CARBON_KEPT: f64 = 0.7;

/// The mass fraction of ¹²C in the intershell that third dredge-up mixes into the envelope: 0.23
/// (Herwig 2005, ARA&A 43, 435, section 3, recalled: about a quarter carbon to three quarters
/// helium).
const INTERSHELL_CARBON: f64 = 0.23;

/// How third dredge-up raises an AGB star's surface C/O, pulse by pulse (P06.T24.b): C/O =
/// (C/O)₀ + δ × max(0, n − `n_min`), with n the star's pulse number since its first (P06.T28.f).
///
/// - **(C/O)₀** is 0.7 of the protosolar C/O ([`FIRST_DREDGE_UP_CARBON_KEPT`]), the same at every
///   \[Fe/H\]: carbon and oxygen scale together with the iron.
/// - **δ** is the carbon one dredge-up adds over the envelope's oxygen,
///   δ = `X_C` ΔM ÷ `A_C` ÷ (X (O/H) `M_env` ÷ `A_H`): ΔM the mass the dredge-up of T8.b returns
///   per pulse, λ ÷ (1 − λ) of the core's growth over the phase, λ the track's own (HPT equation
///   73), shared over the pulses after the `n_min` + 1 that dredge nothing, so that the track's
///   dredged mass is conserved; `X_C` the intershell's carbon ([`INTERSHELL_CARBON`]); X =
///   0.76 − 3Z, the envelope's hydrogen as HPT write it; (O/H) the protosolar oxygen
///   (Lodders 2010) times Z ÷ 0.02, Z as the formulae see it; and `M_env` the envelope at the
///   phase's start. It is one number a star, so the C/O of a fixed star is non-decreasing in n,
///   and so in time. The oxygen the intershell brings up and the α-enhancement of metal-poor
///   stars are left out, so a metal-poor star's C/O climbs far past observed values (a finding);
///   only its class reads it.
/// - **The window**: only a star whose initial mass lies in [`carbon_star_window`] dredges up;
///   any other keeps (C/O)₀ and is no S or carbon star.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DredgeUp {
    per_pulse: f64,
}

impl DredgeUp {
    /// The dredge-up of a star of initial mass `initial_mass` and `composition` on `track`, or
    /// `None` outside [`carbon_star_window`] or for a track with no thermally pulsing AGB with
    /// pulses.
    #[must_use]
    pub(crate) fn of(
        initial_mass: SolarMasses,
        composition: &Composition,
        track: &Track,
    ) -> Option<Self> {
        if !carbon_star_window(composition).contains(&initial_mass) {
            return None;
        }
        let span = track.pulsing_agb()?;
        let pulses = track.pulse_phase_at(span.end().value(), 0.0)?;
        let lambda = track.pulsing_agb_dredge_up()?;
        #[expect(
            clippy::cast_precision_loss,
            reason = "a pulse number of a few, exact in f64"
        )]
        let dredging = pulses - (MIN_DREDGE_UP_PULSES + 1) as f64;
        if !(dredging > 0.0 && dredging.is_finite()) {
            return None;
        }
        let (start, end) = (track.state_at(span.start()), track.state_at(span.end()));
        let growth = (end.core_mass().value() - start.core_mass().value()).max(0.0);
        // A phase that ends part-way to its first dredging pulse's next gives that one pulse the
        // whole of the dredged mass, never more.
        let dredged_per_pulse = lambda / (1.0 - lambda) * growth / dredging.max(1.0);
        let envelope = start.envelope_mass().value();
        if envelope <= 0.0 {
            return None;
        }
        let z = composition.z_fit().value();
        let hydrogen = 0.76 - 3.0 * z;
        let oxygen = Element::O.protosolar_abundance_per_hydrogen() * z / 0.02;
        let per_pulse = INTERSHELL_CARBON * dredged_per_pulse
            / Element::C.standard_atomic_weight_g_per_mol()
            / (hydrogen * oxygen * envelope / Element::H.standard_atomic_weight_g_per_mol());
        Some(Self { per_pulse })
    }

    /// (C/O)₀, the surface C/O before third dredge-up begins.
    #[must_use]
    pub(crate) fn initial_c_to_o() -> f64 {
        FIRST_DREDGE_UP_CARBON_KEPT
            * math::exp10(
                (Element::C.protosolar_log_abundance() - Element::O.protosolar_log_abundance())
                    .value(),
            )
    }

    /// The surface C/O by number after pulse `pulse_number` (0 for the first):
    /// (C/O)₀ + δ × max(0, n − `n_min`).
    #[must_use]
    pub(crate) fn c_to_o(self, pulse_number: i64) -> f64 {
        let dredged = pulse_number.saturating_sub(MIN_DREDGE_UP_PULSES).max(0);
        #[expect(
            clippy::cast_precision_loss,
            reason = "a pulse count under 2⁵³, exact in f64"
        )]
        let dredged = dredged as f64;
        math::mul_add(self.per_pulse, dredged, Self::initial_c_to_o())
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::Seed;
    use crate::events::tags as event_tags;
    use crate::id::{BodyId, SystemId};
    use crate::stellar::classify::{ClassExtras, classify};
    use crate::stellar::draws::{StarDraws, StarDrawsParts, UnitUniform};
    use crate::stellar::events::thermal_pulse::{ThermalPulseClock, ThermalPulseModel};
    use crate::stellar::rules::StellarRules;
    use crate::stellar::sse::{PhasePredicate, Track};
    use crate::stellar::system::{Pulses, StarModel, StarStreams};
    use crate::time::{Span, UniverseTime};
    use crate::units::{Dex, HeliumExcess, Years};

    fn composition(z: f64) -> Composition {
        Composition::from_fe_h(Dex::new(math::log10(z / 0.02)), HeliumExcess::ZERO)
    }

    /// The classification of `track`'s star at `age`, with the helium surface its track gives,
    /// as `StarModel::class_extras_at` gives it.
    fn class_on(track: &Track, age: f64) -> (StarState, Classification) {
        let state = track.state_at(Years::new(age));
        let extras = match (state.phase(), track.helium_star_entry_mass()) {
            (
                Phase::HeliumMainSequence | Phase::HeliumHertzsprungGap | Phase::HeliumGiantBranch,
                Some(entry),
            ) => ClassExtras::helium_star(HeliumSurface::of(entry, state.mass())),
            _ => ClassExtras::NONE,
        };
        let class = classify(&state, track.composition(), &StarDraws::median(), &extras);
        (state, class)
    }

    /// The stages a track's star passes through, in order of first appearance: `LBV`, `WNh`,
    /// `WN`, `WC` and `WO`, sampled at 20,000 ages from the end of its main sequence's first half
    /// to its death.
    fn stages(track: &Track) -> Vec<&'static str> {
        let death = track.lifetime().expect("dies").value();
        let from = 0.5 * track.main_sequence_end().expect("evolves").value();
        let mut seen = Vec::new();
        for i in 0..20_000 {
            let age = from + (death - from) * (f64::from(i) + 0.5) / 20_000.0;
            let (_, class) = class_on(track, age);
            let stage = match (class.spectral_type(), class.peculiar_class()) {
                (_, Some(PeculiarClass::LuminousBlueVariable)) => "LBV",
                (SpectralType::WolfRayet(wr), _) => match (wr.sequence(), wr.shows_hydrogen()) {
                    (WolfRayetSequence::Nitrogen, true) => "WNh",
                    (WolfRayetSequence::Nitrogen, false) => "WN",
                    (WolfRayetSequence::Carbon, _) => "WC",
                    (WolfRayetSequence::Oxygen, _) => "WO",
                },
                _ => continue,
            };
            if !seen.contains(&stage) {
                seen.push(stage);
            }
        }
        seen
    }

    /// P06.T24.a's test: a 60 M☉ star at Z = 0.02 passes through LBV, WN and WC in that order,
    /// and at Z = 0.0001 never becomes WC.
    #[test]
    fn a_60_solar_mass_star_is_an_lbv_then_wn_then_wc_at_solar_metallicity() {
        let solar = Track::full(
            SolarMasses::new(60.0),
            &composition(0.02),
            &StarDraws::median(),
        );
        let seen = stages(&solar);
        let at = |stage: &str| seen.iter().position(|s| *s == stage);
        let (lbv, wn, wc) = (at("LBV"), at("WN"), at("WC"));
        assert!(
            lbv.is_some() && wn.is_some() && wc.is_some() && lbv < wn && wn < wc,
            "{seen:?}"
        );
        let poor = Track::full(
            SolarMasses::new(60.0),
            &composition(1e-4),
            &StarDraws::median(),
        );
        let seen = stages(&poor);
        assert!(!seen.contains(&"WC"), "{seen:?}");
    }

    /// A giant-branch star stripped of its envelope leaves a naked helium star of its core, some
    /// half a solar mass, which classifies as sdB on its helium main sequence (P06.T24.a's test).
    #[test]
    fn a_stripped_giant_branch_star_is_an_sdb_star() {
        let star = Track::helium_star_full(
            SolarMasses::new(0.5),
            &Composition::SOLAR,
            &StarDraws::median(),
        );
        let (state, class) = class_on(&star, 5e7);
        assert_eq!(state.phase(), Phase::HeliumMainSequence);
        assert_eq!(
            class.peculiar_class(),
            Some(PeculiarClass::HotSubdwarf),
            "{class}"
        );
        assert!(class.to_string().starts_with("sdB"), "{class} at {state:?}");
    }

    /// A living helium main-sequence state of mass `m` M☉, luminosity `l` L☉ and effective
    /// temperature `teff` K.
    fn helium_main_sequence(m: f64, l: f64, teff: f64) -> StarState {
        let ratio = 5_772.0 / teff;
        let r = ratio * ratio * l.sqrt();
        StarState::new(crate::stellar::StarStateParts {
            phase: Phase::HeliumMainSequence,
            age: Years::new(1e6),
            mass: SolarMasses::new(m),
            core_mass: SolarMasses::ZERO,
            luminosity: SolarLuminosities::new(l),
            radius: crate::units::SolarRadii::new(r),
            mass_loss_rate: crate::units::SolarMassesPerYear::ZERO,
            phase_fraction: 0.5,
        })
    }

    fn classify_bare(state: &StarState, composition: &Composition) -> Classification {
        classify(state, composition, &StarDraws::median(), &ClassExtras::NONE)
    }

    /// P06.T24.c's edge: on the helium main sequence a 1.6 M☉ helium star is a hot subdwarf and a
    /// 1.8 M☉ one a stripped helium star, each on its own track at mid-life, at the same letter.
    #[test]
    fn the_hot_subdwarfs_end_at_1_7_solar_masses() {
        let class_at_mid_life = |m: f64| {
            let track = Track::helium_star_full(
                SolarMasses::new(m),
                &Composition::SOLAR,
                &StarDraws::median(),
            );
            let mid_life = track
                .age_in_phase(Phase::HeliumMainSequence, 0.5)
                .expect("a helium main sequence");
            let (state, class) = class_on(&track, mid_life);
            assert_eq!(state.phase(), Phase::HeliumMainSequence);
            class
        };
        let light = class_at_mid_life(1.6);
        assert_eq!(
            light.peculiar_class(),
            Some(PeculiarClass::HotSubdwarf),
            "{light}"
        );
        assert!(light.to_string().starts_with("sdO"), "{light}");
        let heavy = class_at_mid_life(1.8);
        assert_eq!(
            heavy.peculiar_class(),
            Some(PeculiarClass::StrippedHeliumStar),
            "{heavy}"
        );
        assert!(heavy.to_string().starts_with("He-sdO"), "{heavy}");
        assert_eq!(heavy.luminosity_class(), Some(LuminosityClass::Subdwarf));
        // The edge itself is a hot subdwarf's.
        let edge = classify_bare(
            &helium_main_sequence(1.7, 1_500.0, 50_000.0),
            &Composition::SOLAR,
        );
        assert_eq!(edge.peculiar_class(), Some(PeculiarClass::HotSubdwarf));
    }

    /// A 2.3 M☉ helium star at about 65,000 K, look-72's stripped star (plan 12's Risks), is
    /// `He-sdO3`, and below 40,000 K the same star is `He-sdB`.
    #[test]
    fn a_stripped_helium_star_is_written_he_sd() {
        let hot = classify_bare(
            &helium_main_sequence(2.3, 5_000.0, 65_000.0),
            &Composition::SOLAR,
        );
        assert_eq!(hot.to_string(), "He-sdO3");
        assert_eq!(
            hot.peculiar_class(),
            Some(PeculiarClass::StrippedHeliumStar)
        );
        assert_eq!(PeculiarClass::StrippedHeliumStar.name(), "He-sd");
        let cool = classify_bare(
            &helium_main_sequence(2.3, 5_000.0, 30_000.0),
            &Composition::SOLAR,
        );
        assert!(cool.to_string().starts_with("He-sdB"), "{cool}");
        // Below 20,000 K it is no subdwarf of either kind.
        let cold = classify_bare(
            &helium_main_sequence(2.3, 5_000.0, 15_000.0),
            &Composition::SOLAR,
        );
        assert_eq!(cold.peculiar_class(), None, "{cold}");
    }

    /// Above the Wolf-Rayet floor a naked helium star is a Wolf-Rayet star whatever its mass, as
    /// before T24.c: the floor, not the edge, ends the stripped helium stars.
    #[test]
    fn above_the_floor_a_helium_star_is_still_a_wolf_rayet_star() {
        let luminous = classify_bare(
            &helium_main_sequence(10.0, 1.0e5, 90_000.0),
            &Composition::SOLAR,
        );
        assert_eq!(luminous.peculiar_class(), Some(PeculiarClass::WolfRayet));
        assert_eq!(luminous.to_string(), "WN4");
        let faint = classify_bare(
            &helium_main_sequence(10.0, 6.0e4, 90_000.0),
            &Composition::SOLAR,
        );
        assert_eq!(
            faint.peculiar_class(),
            Some(PeculiarClass::StrippedHeliumStar)
        );
    }

    /// `window_where(PhasePredicate::Lbv)` brackets exactly the ages at which the state passes
    /// the criterion, at 1,000 sampled ages across the star's life (P06.T24.a's test), for a
    /// 60 M☉ star at Z = 0.02; a 20 M☉ star has no window.
    #[test]
    fn the_lbv_window_brackets_the_ages_that_pass_the_criterion() {
        let track = Track::full(
            SolarMasses::new(60.0),
            &composition(0.02),
            &StarDraws::median(),
        );
        let window = track
            .window_where(PhasePredicate::Lbv)
            .expect("an LBV phase");
        let death = track.lifetime().expect("dies").value();
        let (start, end) = (window.start().value(), window.end().value());
        // Half the samples across the whole life, half across the window and its surroundings.
        let span = end - start;
        let ages = (0..500)
            .map(|i| death * (f64::from(i) + 0.5) / 500.0)
            .chain((0..500).map(|i| start - span + 3.0 * span * (f64::from(i) + 0.5) / 500.0));
        let mut inside = 0;
        for age in ages {
            let state = track.state_at(Years::new(age));
            let passes = is_luminous_blue_variable(&state);
            assert_eq!(
                passes,
                window.contains(Years::new(age)),
                "at {age}: window {start}–{end}, {state:?}"
            );
            inside += u32::from(passes);
        }
        assert!(inside > 100, "{inside}");
        assert!(PhasePredicate::Lbv.holds(&track.state_at(window.start())));
        assert!(PhasePredicate::Lbv.holds(&track.state_at(window.end())));
        let light = Track::full(
            SolarMasses::new(20.0),
            &Composition::SOLAR,
            &StarDraws::median(),
        );
        assert_eq!(light.window_where(PhasePredicate::Lbv), None);
    }

    /// Every class this module gives round-trips through `Display` and the test parser.
    #[test]
    fn wolf_rayet_subdwarf_and_lbv_classes_round_trip() {
        for track in [
            Track::full(
                SolarMasses::new(60.0),
                &composition(0.02),
                &StarDraws::median(),
            ),
            Track::full(
                SolarMasses::new(120.0),
                &composition(0.001),
                &StarDraws::median(),
            ),
            Track::helium_star_full(
                SolarMasses::new(0.6),
                &Composition::SOLAR,
                &StarDraws::median(),
            ),
            Track::helium_star_full(
                SolarMasses::new(2.3),
                &Composition::SOLAR,
                &StarDraws::median(),
            ),
        ] {
            let death = track.lifetime().expect("dies").value();
            for i in 0..2_000 {
                let (_, class) = class_on(&track, death * (f64::from(i) + 0.5) / 2_000.0);
                let text = class.to_string();
                let parsed = super::super::tests::parse(&text);
                assert_eq!(
                    parsed.map(|c| c.to_string()),
                    Some(text.clone()),
                    "{class:?}"
                );
            }
        }
    }

    #[test]
    fn the_carbon_fit_follows_langers_convective_cores() {
        // Langer 1989a, table 1, Y = 1: f = 1 − M_cc ÷ M.
        for (m, f) in [
            (2.0, 0.730),
            (3.0, 0.673),
            (5.0, 0.548),
            (7.0, 0.477),
            (10.0, 0.388),
            (15.0, 0.313),
            (20.0, 0.260),
            (30.0, 0.200),
            (40.0, 0.162),
            (60.0, 0.148),
        ] {
            let fit = carbon_shows_after(SolarMasses::new(m));
            assert!((fit - f).abs() < 0.025, "{m} M☉: {fit} against {f}");
        }
        let top = carbon_shows_after(SolarMasses::new(60.0));
        assert!((carbon_shows_after(SolarMasses::new(100.0)) - top).abs() < 1e-15);
    }

    #[test]
    fn subtypes_follow_the_temperature_scales() {
        assert_eq!(subtype_at(&WN_SUBTYPES, 150_000.0), 2);
        assert_eq!(subtype_at(&WN_SUBTYPES, 60_000.0), 5);
        assert_eq!(subtype_at(&WN_SUBTYPES, 30_000.0), 9);
        assert_eq!(subtype_at(&WC_SUBTYPES, 72_000.0), 7);
        assert_eq!(subtype_at(&WO_SUBTYPES, 120_000.0), 4);
        assert_eq!(subtype_at(&WO_SUBTYPES, 300_000.0), 1);
        let written = |sequence, n, h| WolfRayetType::new(sequence, n, h).unwrap().to_string();
        assert_eq!(written(WolfRayetSequence::Nitrogen, 7, true), "WN7h");
        assert_eq!(written(WolfRayetSequence::Carbon, 5, false), "WC5");
        assert_eq!(written(WolfRayetSequence::Oxygen, 2, false), "WO2");
        assert_eq!(
            WolfRayetType::new(WolfRayetSequence::Carbon, 3, false),
            None
        );
        assert_eq!(WolfRayetType::new(WolfRayetSequence::Carbon, 5, true), None);
    }

    /// The streams the tests' stars open their pulses from.
    fn streams() -> StarStreams {
        let system = SystemId::from_raw(0x0200_0800_2000_0000).expect("a well-formed ID");
        StarStreams::new(Seed::new(0x0600_0024_b000_5eed), BodyId::new(system, 0))
    }

    /// A star of `m` M☉ and Z `z`, 12 Gyr old at the epoch so that every star that pulses has
    /// pulsed, under every 26 hold.
    fn long_dead(m: f64, z: f64) -> StarModel {
        StarModel::new_under(
            SolarMasses::new(m),
            composition(z),
            StarDraws::median(),
            Years::new(1.2e10),
            StellarRules::every_26_hold(),
        )
        .expect("in range")
    }

    /// `n` times evenly through `clock`'s thermally pulsing AGB, in order, its first instant
    /// included.
    fn pulsing_times(clock: &ThermalPulseClock<'_>, n: u32) -> Vec<UniverseTime> {
        let span = clock
            .end()
            .checked_since(clock.start())
            .expect("a phase the clock holds")
            .as_seconds_f64();
        (0..n)
            .map(|k| {
                let offset = Span::from_seconds_f64(span * f64::from(k) / f64::from(n))
                    .expect("inside the phase");
                clock.start().checked_add(offset).expect("inside the clock")
            })
            .collect()
    }

    /// The class of `star` at `t`, its own pulses opened.
    fn class_of(star: &StarModel, t: UniverseTime) -> Classification {
        let state = star.state_at(t).expect("formed");
        let extras = star.class_extras_at(&state, t, Pulses::Opened(streams()));
        classify(&state, star.composition(), star.draws(), &extras)
    }

    /// The plan's test (P06.T24.b): carbon and S stars appear only inside the mass window and
    /// only after `n_min` pulses, at 200 times through the thermally pulsing AGB of each star of
    /// 1–5 M☉ at four metallicities; and they do appear, at solar Z and below.
    #[test]
    fn carbon_stars_appear_only_inside_the_window_and_after_the_first_pulses() {
        let mut carbon = 0;
        for z in [0.03, 0.02, 0.004, 0.0005] {
            let window = carbon_star_window(&composition(z));
            let mut found = 0;
            for k in 0..17 {
                let m = 1.0 + 0.25 * f64::from(k);
                let star = long_dead(m, z);
                let Some(clock) = ThermalPulseClock::of(&star) else {
                    continue;
                };
                let series = streams().series(event_tags::STAR_THERMAL_PULSE);
                let pulses = ThermalPulseModel::of(&star, &series).expect("a star that pulses");
                for t in pulsing_times(&clock, 200) {
                    let class = class_of(&star, t);
                    if matches!(
                        class.peculiar_class(),
                        Some(PeculiarClass::Carbon | PeculiarClass::SType(_))
                    ) {
                        assert!(
                            window.contains(&SolarMasses::new(m)),
                            "{class} at {m} M☉, Z {z}"
                        );
                        let n = pulses.pulse_number_at(t).expect("on the phase");
                        assert!(n > MIN_DREDGE_UP_PULSES, "{class} after {n} pulses");
                        found += u32::from(class.peculiar_class() == Some(PeculiarClass::Carbon));
                    }
                }
            }
            if z <= 0.02 {
                assert!(found > 0, "no carbon star at Z {z}");
            }
            carbon += found;
        }
        assert!(carbon > 100, "{carbon}");
    }

    /// The plan's test (P06.T24.b): the C/O of a fixed star never falls as it pulses, and its class
    /// climbs M, MS, S, SC, C in that order, at 500 times through the phase.
    #[test]
    fn a_fixed_star_s_c_to_o_never_falls() {
        let rank = |class: Option<PeculiarClass>| match class {
            Some(PeculiarClass::SType(SStarType::Ms)) => 1,
            Some(PeculiarClass::SType(SStarType::S)) => 2,
            Some(PeculiarClass::SType(SStarType::Sc)) => 3,
            Some(PeculiarClass::Carbon) => 4,
            None => 0,
            Some(
                other @ (PeculiarClass::Be
                | PeculiarClass::Ap
                | PeculiarClass::Bp
                | PeculiarClass::Am
                | PeculiarClass::WolfRayet
                | PeculiarClass::HotSubdwarf
                | PeculiarClass::StrippedHeliumStar
                | PeculiarClass::LuminousBlueVariable
                | PeculiarClass::TTauri { .. }
                | PeculiarClass::HerbigAeBe),
            ) => panic!("an AGB star of class {other:?}"),
        };
        for (m, z) in [(2.0, 0.02), (3.0, 0.02), (3.5, 0.03), (1.5, 0.004)] {
            let star = long_dead(m, z);
            let track = star.track().expect("a star of over 0.1 M☉");
            let dredge_up = DredgeUp::of(SolarMasses::new(m), star.composition(), track)
                .expect("inside the window");
            let clock = ThermalPulseClock::of(&star).expect("a star that pulses");
            let series = streams().series(event_tags::STAR_THERMAL_PULSE);
            let pulses = ThermalPulseModel::of(&star, &series).expect("a star that pulses");
            let (mut c_to_o, mut class) = (0.0, 0);
            for t in pulsing_times(&clock, 500) {
                let now = dredge_up.c_to_o(pulses.pulse_number_at(t).expect("on the phase"));
                let written = rank(class_of(&star, t).peculiar_class());
                assert!(
                    now >= c_to_o && written >= class,
                    "{m} M☉, Z {z}: {now} after {c_to_o}"
                );
                (c_to_o, class) = (now, written);
            }
            assert_eq!(
                class, 4,
                "{m} M☉, Z {z} ends a carbon star, at C/O {c_to_o}"
            );
        }
    }

    /// The plan's test (P06.T24.b, amended by research-leans-12 §3): a 1 M☉ star at 2 Myr is a
    /// T Tauri star, classical while its disc lasts by its disc draw, a 4 M☉ one at 0.8 Myr, on its
    /// contraction, a Herbig Ae/Be star; the protostars of 4 M☉ at 0.3 Myr and 1 M☉ at 0.1 Myr, a
    /// 9 M☉ star at 0.8 Myr and the Sun take neither.
    #[test]
    fn young_stars_are_t_tauri_or_herbig_ae_be_stars_by_mass() {
        let young = |m: f64, age: f64, rank: f64, rules: StellarRules| {
            let draws = StarDraws::from_parts(StarDrawsParts {
                disc_lifetime: UnitUniform::new(rank).expect("in (0, 1)"),
                ..StarDrawsParts::MEDIAN
            });
            let star = StarModel::new_under(
                SolarMasses::new(m),
                Composition::SOLAR,
                draws,
                Years::new(age),
                rules,
            )
            .expect("in range");
            class_of(&star, UniverseTime::EPOCH)
        };
        let phase = |m: f64, age: f64| {
            StarModel::new(
                SolarMasses::new(m),
                Composition::SOLAR,
                StarDraws::median(),
                Years::new(age),
            )
            .expect("in range")
            .state_at(UniverseTime::EPOCH)
            .expect("formed")
            .phase()
        };
        let full = StellarRules::every_26_hold();
        for rank in [0.05, 0.5, 0.95] {
            let disc = disc_lifetime(SolarMasses::new(1.0), UnitUniform::new(rank).unwrap());
            let class = young(1.0, 2e6, rank, full);
            assert_eq!(
                class.peculiar_class(),
                Some(PeculiarClass::TTauri {
                    classical: disc.value() > 2.0
                }),
                "{class}, disc {disc:?}"
            );
        }
        // 0.8 Myr is inside the 4 M☉ star's contraction, after its 0.5 Myr of accretion and
        // before its arrival on the main sequence.
        assert_eq!(phase(4.0, 8e5), Phase::PreMainSequence);
        assert_eq!(
            young(4.0, 8e5, 0.5, full).peculiar_class(),
            Some(PeculiarClass::HerbigAeBe)
        );
        // A Class I protostar, still accreting, and a Class 0 one take no young class.
        for (m, age) in [(4.0, 3e5), (1.0, 1e5)] {
            assert_eq!(phase(m, age), Phase::Protostar, "{m} M☉ at {age} years");
            assert_eq!(young(m, age, 0.5, full).peculiar_class(), None);
        }
        assert_eq!(young(9.0, 8e5, 0.5, full).peculiar_class(), None);
        assert_eq!(young(1.0, 4.57e9, 0.5, full).peculiar_class(), None);
    }

    /// The T Tauri stars' upper mass is the FU Orionis outbursts' disc phase's (P06.T28.d), and
    /// the classes' C/O edges climb.
    #[test]
    fn the_young_and_chemical_edges_agree_with_their_neighbours() {
        assert_eq!(
            T_TAURI_MAX_MASS,
            crate::stellar::events::fu_orionis::DISC_PHASE_MASS_LIMIT
        );
        const { assert!(C_TO_O_MS < C_TO_O_S && C_TO_O_S < C_TO_O_SC && C_TO_O_SC < CARBON_STAR_C_TO_O) };
        assert_eq!(chemical_class(0.94), None);
        assert_eq!(
            chemical_class(1.0),
            Some(PeculiarClass::SType(SStarType::Sc))
        );
        assert_eq!(chemical_class(1.01), Some(PeculiarClass::Carbon));
        let initial = DredgeUp::initial_c_to_o();
        let protosolar = crate::planetary::chem::solar_carbon_to_oxygen();
        assert!((initial - 0.7 * protosolar).abs() < 1e-12, "{initial}");
        let window = carbon_star_window(&Composition::SOLAR);
        assert!((window.start.value() - 1.5).abs() < 1e-12);
        assert!((window.end.value() - 4.0).abs() < 1e-12);
    }

    /// The C/O's bits for three fixed stars, pinned before any version reads them (the arithmetic
    /// is output): its form, δ's quotient and the fused (C/O)₀ + δ × n, may not be rewritten but
    /// as a change of output.
    #[test]
    fn the_c_to_o_s_bits_are_pinned() {
        for (m, z, n, bits) in [
            (2.0, 0.02, 12, 0x3ff0_6772_ad24_3c0e_u64),
            (3.0, 0.02, 30, 0x3ff9_72b7_b815_5b22),
            (1.5, 0.004, 9, 0x4016_3ee0_cd40_75a3),
        ] {
            let star = long_dead(m, z);
            let track = star.track().expect("a star of over 0.1 M☉");
            let c_to_o = DredgeUp::of(SolarMasses::new(m), star.composition(), track)
                .expect("inside the window")
                .c_to_o(n);
            assert_same_bits(c_to_o, f64::from_bits(bits));
        }
    }
}
