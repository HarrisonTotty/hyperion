//! Classes beyond the MK grid that follow from a star's state and track (plan 06, P06.T24.a):
//! Wolf-Rayet stars, hot subdwarfs and luminous blue variables, all derived and none rolled.
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
//! - **Hot subdwarfs** are the naked helium stars below the floor, from 20,000 K: `sdB` below
//!   40,000 K and `sdO` above (Heber 2016, PASP 128, 082001, section 2). A single star reaches one
//!   only by losing its envelope on the giant branch; plan 11's stripped stars are the rest.
//! - **Luminous blue variables** lie beyond the Humphreys–Davidson limit as Hurley, Pols and Tout
//!   (2000, section 7.1) write it, and are hotter than 8,000 K ([`is_luminous_blue_variable`]);
//!   [`Track::window_where`](crate::stellar::sse::Track::window_where) gives the ages at which a
//!   star is one.
//!
//! The Wolf-Rayet temperatures are hydrostatic ones, which for helium stars run hotter than the
//! observed T* at τ = 20 (Crowther 2007, section 3.3), so the backbone's Wolf-Rayet stars are
//! mostly early types (a finding recorded in plan 06's Risks).

use core::fmt;

use crate::math;
use crate::stellar::sse::wind;
use crate::stellar::{Composition, Phase, StarState};
use crate::units::{Kelvin, SolarLuminosities, SolarMasses};

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
/// The limit is T(eff) below 5,772 K × (L ÷ 10⁵ L☉)^¼: 14,100 K at 6 × 10⁵ L☉, 18,300 K at
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
/// stars populate (Heber 2016, section 2.2).
const HOT_SUBDWARF_MIN_TEFF: f64 = 20_000.0;

/// The temperature from which a hot subdwarf is an sdO star rather than an sdB, K: 40,000, where
/// He II 4686 strengthens. The classes are spectroscopic (Heber 2016, section 2), so this is a
/// convention consistent with Heber's figures 3–8, not a quoted value.
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
/// Wolf-Rayet floor and from 20,000 K, or `None`: `sd` and the dwarf scale's type, held to B0 and
/// cooler below 40,000 K (an sdB star) and to O below it (an sdO star).
#[must_use]
pub(crate) fn hot_subdwarf(state: &StarState, composition: &Composition) -> Option<Classification> {
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
    let dwarf = super::subtype_from_teff(Kelvin::new(teff))?.value();
    // O ends below code 10 (B0): an sdB is held to B0 and cooler, an sdO to O9.5 and hotter.
    let code = if teff < SD_O_MIN_TEFF {
        dwarf.max(10.0)
    } else {
        dwarf.min(9.5)
    };
    let mut class =
        Classification::sequence(SpectralCode::new(code)?, Some(LuminosityClass::Subdwarf));
    class.peculiar = Some(PeculiarClass::HotSubdwarf);
    Some(class)
}

/// The classification of a Wolf-Rayet star of type `wr`.
#[must_use]
pub(crate) fn wolf_rayet_class(wr: WolfRayetType) -> Classification {
    let mut class = Classification::bare(SpectralType::WolfRayet(wr));
    class.peculiar = Some(PeculiarClass::WolfRayet);
    class
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stellar::classify::{ClassExtras, classify};
    use crate::stellar::draws::StarDraws;
    use crate::stellar::sse::{PhasePredicate, Track};
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
}
