//! A living star's luminosity class, and the temperature scale against which each class's
//! subtype is read (plan 06, P06.T23.b).
//!
//! **The class.** Straižys and Kuriliene (1981, Astrophysics and Space Science 80, 353) give, for
//! every spectral type from O5 to M6 and every class from V to Ia, the effective temperature (their
//! Table III), the bolometric magnitude (Table IV) and the surface gravity (Table VII). A boundary
//! between two classes joins, type by type, the points halfway between them in (log
//! T<sub>eff</sub>, value), linear in log T<sub>eff</sub> between types and held beyond the
//! table's hottest and coolest; a star is compared with the boundary at its own temperature. The
//! boundaries are the fitted table [`class_boundaries`](crate::tables::class_boundaries), which
//! `hyperion-fit`'s `class_boundaries` task draws from the paper's tables; the tables themselves
//! are not committed (P06.T23.b's licence fix, decision-r08-licences).
//!
//! - **Gravity** separates the dwarfs (V), the subgiants (IV) and the rest, at the V/IV and
//!   IV/III boundaries of the paper's Table VII. It is how spectra tell a subgiant from a dwarf,
//!   and how a star that is still contracting (IV) is told from one on the main sequence.
//! - **Luminosity** at the star's temperature orders the rest, from Table IV: III, II, Ib, Iab
//!   and Ia. These classes are calibrated in absolute magnitude, and the gravity of a class III
//!   star in Table VII is that of the paper's 2.1–3.5 M☉ tracks (Table VI): a giant such as
//!   Arcturus (4,300 K, log g 1.7, about 180 L☉), which is K1.5 III, lies 0.73 dex below that
//!   gravity, half for its lower mass and half for its greater luminosity, and falls by gravity
//!   among the bright giants, by luminosity among the giants.
//! - **Ia⁺**, the hypergiants, above 10⁵·⁷ L☉ and within 0.2 dex of the Humphreys–Davidson limit in
//!   the form Hurley, Pols and Tout (2000, MNRAS 315, 543, section 7.1) give it and the winds use:
//!   L<sub>HD</sub> = max(6 × 10⁵ L☉, 10⁵ L☉ (T<sub>eff</sub> ÷ T☉)²), their L > 6 × 10⁵ and 10⁻⁵ R
//!   L^½ > 1 with R from L and T<sub>eff</sub>.
//! - **The phase breaks ties**: a core-helium-burning or asymptotic-giant-branch star is at least
//!   a giant (III), never IV or V, however high its gravity (a horizontal-branch star at
//!   9,000 K and log g 3.9 is an A giant).
//! - **Subdwarfs**: a main-sequence star of class V with \[Fe/H\] below −1.0 is `sd`, below −1.7
//!   `esd`, the plan's thresholds for the metallicity classes of Lépine, Rich and Shara (2007, ApJ
//!   669, 1235), which define them by the weakening of the titanium oxide bands against those of
//!   calcium hydride rather than in \[Fe/H\].
//!
//! **The subtype.** A class V star (and a subdwarf) is read on the dwarf scale of Pecaut and
//! Mamajek exactly ([`pm13`](super::pm13)). A giant (III) is read on the giant scale and every
//! supergiant (Ib, Iab, Ia and Ia⁺) on the supergiant scale of [`scales`](super::scales); a
//! subgiant (IV) is given the subtype halfway between those a dwarf and a giant of its
//! temperature would have, and a bright giant (II) the one halfway between a giant's and a
//! supergiant's. Giants and supergiants are typed no later than M9.5.

use super::SpectralCode;
use super::pm13::DwarfSequence;
use super::scales::{self, Node};
use crate::math;
use crate::stellar::rules::StellarRules;
use crate::stellar::{Composition, Phase, StarState};
use crate::tables::class_boundaries::{
    IAB_IA_M_BOL, IB_IAB_M_BOL, II_IB_M_BOL, III_II_M_BOL, IV_III_LOG_G, V_IV_LOG_G,
};
use crate::units::Dex;
use crate::units::consts::SOLAR_EFFECTIVE_TEMPERATURE_K;

/// A luminosity class of the MK system, from the hypergiants to the dwarfs, and the metal-poor
/// subdwarfs below the main sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LuminosityClass {
    /// Ia⁺, the hypergiants, near the Humphreys–Davidson limit; written `Ia+`.
    Hypergiant,
    /// Ia, luminous supergiants.
    LuminousSupergiant,
    /// Iab, intermediate supergiants.
    Supergiant,
    /// Ib, less luminous supergiants.
    LessLuminousSupergiant,
    /// II, bright giants.
    BrightGiant,
    /// III, giants.
    Giant,
    /// IV, subgiants.
    Subgiant,
    /// V, dwarfs: the main sequence.
    Dwarf,
    /// sd, subdwarfs: metal-poor dwarfs, written as a prefix (`sdM3`).
    Subdwarf,
    /// esd, extreme subdwarfs, written as a prefix (`esdK7`).
    ExtremeSubdwarf,
}

impl LuminosityClass {
    /// Every class, most luminous first.
    pub const ALL: [Self; 10] = [
        Self::Hypergiant,
        Self::LuminousSupergiant,
        Self::Supergiant,
        Self::LessLuminousSupergiant,
        Self::BrightGiant,
        Self::Giant,
        Self::Subgiant,
        Self::Dwarf,
        Self::Subdwarf,
        Self::ExtremeSubdwarf,
    ];

    /// The class as written: `Ia+`, `Ia`, `Iab`, `Ib`, `II`, `III`, `IV`, `V`, `sd`, `esd`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Hypergiant => "Ia+",
            Self::LuminousSupergiant => "Ia",
            Self::Supergiant => "Iab",
            Self::LessLuminousSupergiant => "Ib",
            Self::BrightGiant => "II",
            Self::Giant => "III",
            Self::Subgiant => "IV",
            Self::Dwarf => "V",
            Self::Subdwarf => "sd",
            Self::ExtremeSubdwarf => "esd",
        }
    }

    /// Whether the class is written before the spectral type rather than after it: the
    /// subdwarfs.
    #[must_use]
    pub const fn is_prefix(self) -> bool {
        match self {
            Self::Subdwarf | Self::ExtremeSubdwarf => true,
            Self::Hypergiant
            | Self::LuminousSupergiant
            | Self::Supergiant
            | Self::LessLuminousSupergiant
            | Self::BrightGiant
            | Self::Giant
            | Self::Subgiant
            | Self::Dwarf => false,
        }
    }

    /// Whether the class is a supergiant's, Ib or brighter.
    #[must_use]
    pub const fn is_supergiant(self) -> bool {
        match self {
            Self::Hypergiant
            | Self::LuminousSupergiant
            | Self::Supergiant
            | Self::LessLuminousSupergiant => true,
            Self::BrightGiant
            | Self::Giant
            | Self::Subgiant
            | Self::Dwarf
            | Self::Subdwarf
            | Self::ExtremeSubdwarf => false,
        }
    }
}

/// The Sun's bolometric magnitude on which the magnitudes of
/// [`class_boundaries`](crate::tables::class_boundaries) stand, mag: Straižys and Kuriliene's
/// (1981, section 3) zero point M<sub>bol</sub>☉ = +4.72, as Torres (2010, AJ 140, 1158, section
/// 4) also reads it.
const SOLAR_M_BOL_MAG: f64 = 4.72;

/// \[Fe/H\] below which a main-sequence dwarf is a subdwarf, dex: the plan's threshold
/// (P06.T23.c).
const SUBDWARF_FE_H: f64 = -1.0;

/// \[Fe/H\] below which a main-sequence dwarf is an extreme subdwarf, dex: the plan's threshold
/// (P06.T23.c).
const EXTREME_SUBDWARF_FE_H: f64 = -1.7;

/// log₁₀ L (L☉) above which a star near the Humphreys–Davidson limit is a hypergiant: the plan's
/// 10⁵·⁷ L☉ (P06.T23.b).
const HYPERGIANT_LOG_L: f64 = 5.7;

/// How close below the Humphreys–Davidson limit a hypergiant lies, dex: the plan's 0.2.
const HYPERGIANT_DEPTH_DEX: f64 = 0.2;

/// Hurley, Pols and Tout's (2000, section 7.1) luminosity floor of the Humphreys–Davidson limit,
/// L☉.
const HD_LUMINOSITY_LSUN: f64 = 6e5;

/// The factor of Hurley, Pols and Tout's second condition, 10⁻⁵ R L^½ > 1 (R☉, L☉).
const HD_RADIUS_FACTOR: f64 = 1e-5;

/// The spectral code and luminosity class of a living star (see the [module](self)
/// documentation), a dwarf's and a subgiant's code on the dwarf sequence the stellar stage's
/// `rules` choose.
///
/// A star without luminosity, which has no temperature, is read at the scale's cool end as a
/// dwarf.
#[must_use]
pub(crate) fn classify_living(
    state: &StarState,
    composition: &Composition,
    rules: StellarRules,
) -> (SpectralCode, LuminosityClass) {
    let teff = state.effective_temperature().value();
    let l = state.luminosity().value();
    if !(teff.is_finite() && teff > 0.0 && l.is_finite() && l > 0.0) {
        return (SpectralCode::COOLEST, LuminosityClass::Dwarf);
    }
    let log_teff = math::log10(teff);
    let class = luminosity_class(state, log_teff, math::log10(l), composition.fe_h());
    (
        subtype(class, teff, log_teff, DwarfSequence::of(rules)),
        class,
    )
}

/// The luminosity class of a living star at log₁₀ T<sub>eff</sub> `log_teff` and log₁₀ L `log_l`.
#[must_use]
fn luminosity_class(state: &StarState, log_teff: f64, log_l: f64, fe_h: Dex) -> LuminosityClass {
    let by_gravity = match state.surface_gravity() {
        Some(g) if g.value() >= boundary(log_teff, &V_IV_LOG_G) => LuminosityClass::Dwarf,
        Some(g) if g.value() >= boundary(log_teff, &IV_III_LOG_G) => LuminosityClass::Subgiant,
        Some(_) | None => by_luminosity(log_teff, log_l),
    };
    let below_giant = matches!(
        by_gravity,
        LuminosityClass::Dwarf | LuminosityClass::Subgiant
    );
    let class = if below_giant && is_giant_phase(state.phase()) {
        LuminosityClass::Giant
    } else {
        by_gravity
    };
    if is_hypergiant(log_teff, log_l) {
        return LuminosityClass::Hypergiant;
    }
    if class == LuminosityClass::Dwarf && state.phase() == Phase::MainSequence {
        if fe_h.value() < EXTREME_SUBDWARF_FE_H {
            return LuminosityClass::ExtremeSubdwarf;
        }
        if fe_h.value() < SUBDWARF_FE_H {
            return LuminosityClass::Subdwarf;
        }
    }
    class
}

/// Whether a star of `phase` is at least a giant whatever its gravity: burning helium in its core
/// or on the asymptotic giant branch (the plan's tie rule, extended from the red clump to the
/// AGB).
#[must_use]
const fn is_giant_phase(phase: Phase) -> bool {
    match phase {
        Phase::CoreHeliumBurning | Phase::EarlyAgb | Phase::ThermallyPulsingAgb => true,
        Phase::Protostar
        | Phase::PreMainSequence
        | Phase::MainSequence
        | Phase::HertzsprungGap
        | Phase::FirstGiantBranch
        | Phase::HeliumMainSequence
        | Phase::HeliumHertzsprungGap
        | Phase::HeliumGiantBranch
        | Phase::PostAgb
        | Phase::HeliumWhiteDwarf
        | Phase::CarbonOxygenWhiteDwarf
        | Phase::OxygenNeonWhiteDwarf
        | Phase::NeutronStar
        | Phase::BlackHole
        | Phase::NoRemnant
        | Phase::Substellar => false,
    }
}

/// The class of a star that gravity puts among the giants or brighter, by its luminosity against
/// the III/II, II/Ib, Ib/Iab and Iab/Ia boundaries of Table IV at its temperature.
#[must_use]
fn by_luminosity(log_teff: f64, log_l: f64) -> LuminosityClass {
    let steps: [(&[(f64, f64)], LuminosityClass); 4] = [
        (&III_II_M_BOL, LuminosityClass::BrightGiant),
        (&II_IB_M_BOL, LuminosityClass::LessLuminousSupergiant),
        (&IB_IAB_M_BOL, LuminosityClass::Supergiant),
        (&IAB_IA_M_BOL, LuminosityClass::LuminousSupergiant),
    ];
    let mut class = LuminosityClass::Giant;
    for (nodes, next) in steps {
        let m_bol = boundary(log_teff, nodes);
        let log_l_boundary = (SOLAR_M_BOL_MAG - m_bol) / 2.5;
        if log_l > log_l_boundary {
            class = next;
        } else {
            break;
        }
    }
    class
}

/// Whether a star lies above 10⁵·⁷ L☉ and within 0.2 dex below the Humphreys–Davidson limit, or
/// beyond it.
#[must_use]
fn is_hypergiant(log_teff: f64, log_l: f64) -> bool {
    // 10⁻⁵ R L^½ > 1 with R = L^½ (T☉ ÷ T)² is log L > 5 + 2 log(T ÷ T☉).
    let radius_limit = -math::log10(HD_RADIUS_FACTOR)
        + 2.0 * (log_teff - math::log10(SOLAR_EFFECTIVE_TEMPERATURE_K));
    let log_l_hd = math::log10(HD_LUMINOSITY_LSUN).max(radius_limit);
    log_l > HYPERGIANT_LOG_L && log_l > log_l_hd - HYPERGIANT_DEPTH_DEX
}

/// A boundary of [`class_boundaries`](crate::tables::class_boundaries) at `log_teff`: linear in
/// log T<sub>eff</sub> between its `nodes` (log T<sub>eff</sub>, value), which fall strictly in
/// temperature (a test checks), and held beyond the first and the last.
#[must_use]
fn boundary(log_teff: f64, nodes: &[(f64, f64)]) -> f64 {
    let mut previous: Option<(f64, f64)> = None;
    for &(x, y) in nodes {
        if log_teff >= x {
            return match previous {
                None => y,
                Some((x0, y0)) => y0 + (x0 - log_teff) / (x0 - x) * (y - y0),
            };
        }
        previous = Some((x, y));
    }
    previous.map_or(0.0, |(_, y)| y)
}

/// The spectral code of a star of class `class` at T<sub>eff</sub> `teff_k` (log₁₀ `log_teff`), a
/// dwarf's and a subgiant's on `dwarfs`.
#[must_use]
fn subtype(
    class: LuminosityClass,
    teff_k: f64,
    log_teff: f64,
    dwarfs: DwarfSequence,
) -> SpectralCode {
    let dwarf = || dwarfs.code_at(teff_k);
    let giant = || scale_code(&scales::GIANT, teff_k, log_teff);
    let supergiant = || scale_code(&scales::SUPERGIANT, teff_k, log_teff);
    let code = match class {
        LuminosityClass::Dwarf | LuminosityClass::Subdwarf | LuminosityClass::ExtremeSubdwarf => {
            return SpectralCode::clamped(dwarf());
        }
        LuminosityClass::Subgiant => f64::midpoint(dwarf(), giant()),
        LuminosityClass::Giant => giant(),
        LuminosityClass::BrightGiant => f64::midpoint(giant(), supergiant()),
        LuminosityClass::LessLuminousSupergiant
        | LuminosityClass::Supergiant
        | LuminosityClass::LuminousSupergiant
        | LuminosityClass::Hypergiant => supergiant(),
    };
    SpectralCode::clamped(code.min(SpectralCode::LAST_MK.value()))
}

/// The spectral code a scale gives T<sub>eff</sub> `teff_k` (log₁₀ `log_teff`): linear in
/// log₁₀ T<sub>eff</sub> between the two nodes that bracket it, held at the first node's code
/// above it and the last's below.
#[must_use]
fn scale_code(nodes: &[Node], teff_k: f64, log_teff: f64) -> f64 {
    // Nodes `..hotter` are at or above `teff_k`, the rest below it.
    let hotter = nodes.partition_point(|n| n.teff_k >= teff_k);
    match (
        hotter.checked_sub(1).and_then(|i| nodes.get(i)),
        nodes.get(hotter),
    ) {
        (Some(hot), Some(cool)) => {
            let log_hot = math::log10(hot.teff_k);
            let fraction = (log_hot - log_teff) / (log_hot - math::log10(cool.teff_k));
            hot.code + fraction * (cool.code - hot.code)
        }
        (Some(only), None) | (None, Some(only)) => only.code,
        (None, None) => SpectralCode::LAST_MK.value(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{star, star_g};
    use super::*;
    use crate::GENERATOR_VERSION;
    use crate::units::HeliumExcess;

    /// The generator version's stellar rules: the held arm of the dwarf sequence.
    const VERSION: StellarRules = StellarRules::of_version(GENERATOR_VERSION);

    const GRAVITY_BOUNDARIES: [&[(f64, f64)]; 2] = [&V_IV_LOG_G, &IV_III_LOG_G];

    const LUMINOSITY_BOUNDARIES: [&[(f64, f64)]; 4] =
        [&III_II_M_BOL, &II_IB_M_BOL, &IB_IAB_M_BOL, &IAB_IA_M_BOL];

    fn class_of(state: &StarState) -> LuminosityClass {
        classify_living(state, &Composition::SOLAR, VERSION).1
    }

    /// The nodes of every boundary fall strictly in log T<sub>eff</sub>, which the interpolation
    /// assumes, and each spans at least 28 of the paper's types.
    #[test]
    fn every_boundarys_points_fall_in_temperature() {
        for nodes in GRAVITY_BOUNDARIES.into_iter().chain(LUMINOSITY_BOUNDARIES) {
            assert!(nodes.len() >= 28, "{}", nodes.len());
            assert!(nodes.windows(2).all(|w| w[0].0 > w[1].0), "{nodes:?}");
        }
    }

    /// At every temperature the boundaries are in order: V/IV above IV/III in gravity, and each
    /// luminosity boundary above the one before it.
    #[test]
    fn the_boundaries_never_cross() {
        let mut log_t = 3.3;
        while log_t < 4.9 {
            let v_iv = boundary(log_t, &V_IV_LOG_G);
            let iv_iii = boundary(log_t, &IV_III_LOG_G);
            assert!(v_iv > iv_iii, "log T {log_t}: {v_iv} ≤ {iv_iii}");
            let magnitudes: Vec<f64> = LUMINOSITY_BOUNDARIES
                .iter()
                .map(|nodes| boundary(log_t, nodes))
                .collect();
            assert!(
                magnitudes.windows(2).all(|w| w[0] > w[1]),
                "log T {log_t}: {magnitudes:?}"
            );
            log_t += 0.001;
        }
    }

    /// A boundary passes through the midpoint at each type the paper gives both classes, and is
    /// held beyond the table. The values asserted are the paper's (Table VII).
    #[test]
    fn a_boundary_passes_halfway_between_its_classes() {
        // A0: V 4.07 and IV 3.91 at log T 3.982.
        let at_a0 = boundary(3.982, &V_IV_LOG_G);
        assert!((at_a0 - 3.99).abs() < 1e-12, "{at_a0}");
        // Held above O5 (V 3.9, IV 3.86) and below K1, the last type with a class IV (IV 3.55,
        // III 2.78).
        let hot = boundary(4.9, &V_IV_LOG_G);
        assert!((hot - 3.88).abs() < 1e-12, "{hot}");
        let cool = boundary(3.4, &IV_III_LOG_G);
        assert!((cool - f64::midpoint(3.55, 2.78)).abs() < 1e-12, "{cool}");
    }

    /// For every class the subtype is monotone in temperature, and within O3–M9.5 for every
    /// class but the dwarfs', on either arm of the dwarf sequence.
    #[test]
    fn the_subtype_is_monotone_in_teff_for_every_class() {
        let arms =
            [VERSION, StellarRules::of_version(crate::GENERATOR_VERSION)].map(DwarfSequence::of);
        for (class, dwarfs) in LuminosityClass::ALL
            .into_iter()
            .flat_map(|class| arms.map(|arm| (class, arm)))
        {
            let mut previous = 0.0;
            let mut t = 60_000.0_f64;
            while t > 1_500.0 {
                let code = subtype(class, t, math::log10(t), dwarfs).value();
                assert!(
                    code >= previous,
                    "{class:?} on {dwarfs:?} at {t} K: {code} after {previous}"
                );
                if !matches!(
                    class,
                    LuminosityClass::Dwarf
                        | LuminosityClass::Subdwarf
                        | LuminosityClass::ExtremeSubdwarf
                ) {
                    assert!(code <= SpectralCode::LAST_MK.value(), "{class:?} at {t} K");
                }
                previous = code;
                t *= 0.999;
            }
        }
    }

    /// The inversion finds the scale's own nodes: a giant at a node's temperature has the
    /// node's subtype, to the bisection's resolution.
    #[test]
    fn a_giant_at_a_nodes_temperature_has_its_subtype() {
        for node in scales::GIANT {
            let code = subtype(
                LuminosityClass::Giant,
                node.teff_k,
                math::log10(node.teff_k),
                DwarfSequence::of(VERSION),
            );
            assert!(
                (code.value() - node.code).abs() < 1e-9,
                "{node:?}: {code:?}"
            );
        }
        for node in scales::SUPERGIANT {
            let code = subtype(
                LuminosityClass::Supergiant,
                node.teff_k,
                math::log10(node.teff_k),
                DwarfSequence::of(VERSION),
            );
            assert!(
                (code.value() - node.code).abs() < 1e-9,
                "{node:?}: {code:?}"
            );
        }
    }

    /// The phase breaks ties: a core-helium-burning star of high gravity is a giant, never IV or
    /// V; the same state on the main sequence is not.
    #[test]
    fn a_helium_burning_star_is_at_least_a_giant() {
        // A blue horizontal-branch star: 0.6 M☉, 9,000 K, log g 4.0.
        let bhb = star_g(Phase::CoreHeliumBurning, 0.6, 4.0, 9_000.0);
        assert_eq!(class_of(&bhb), LuminosityClass::Giant);
        let same_on_the_main_sequence = star_g(Phase::MainSequence, 0.6, 4.0, 9_000.0);
        assert_eq!(
            class_of(&same_on_the_main_sequence),
            LuminosityClass::Subgiant
        );
        // A red-clump star is a giant by its gravity alone.
        let clump = star_g(Phase::CoreHeliumBurning, 1.3, 2.4, 4_750.0);
        assert_eq!(class_of(&clump), LuminosityClass::Giant);
        let agb = star_g(Phase::EarlyAgb, 2.0, 4.2, 5_000.0);
        assert_eq!(class_of(&agb), LuminosityClass::Giant);
    }

    /// Ia⁺ above 10⁵·⁷ L☉ within 0.2 dex of the Humphreys–Davidson limit; not an O dwarf far
    /// below the limit's hot, rising part.
    #[test]
    fn hypergiants_lie_near_the_humphreys_davidson_limit() {
        let yellow = star(Phase::CoreHeliumBurning, 30.0, 7e5, 7_000.0);
        assert_eq!(class_of(&yellow), LuminosityClass::Hypergiant);
        let below_floor = star(Phase::CoreHeliumBurning, 25.0, 4.5e5, 7_000.0);
        assert_eq!(class_of(&below_floor), LuminosityClass::LuminousSupergiant);
        // At 45,000 K the limit is 10⁵ (45,000 ÷ 5,772)² = 6.1 × 10⁶ L☉.
        let o_dwarf = star_g(Phase::MainSequence, 60.0, 4.0, 45_000.0);
        assert!(o_dwarf.luminosity().value() > 5e5);
        assert_eq!(class_of(&o_dwarf), LuminosityClass::Dwarf);
        // At 38,000 K the limit is 10⁶·⁶⁴ L☉: 3.2 × 10⁶ L☉ lies within 0.2 dex of it.
        let near_tams = star(Phase::MainSequence, 100.0, 3.2e6, 38_000.0);
        assert_eq!(class_of(&near_tams), LuminosityClass::Hypergiant);
        assert!(!is_hypergiant(math::log10(40_000.0), 6.3));
        assert!(is_hypergiant(math::log10(40_000.0), 6.6));
    }

    /// Luminosity orders the stars that gravity puts among the giants: at 4,600 K, a 50 L☉ clump
    /// star is III, 1,000 L☉ II, 10⁴ L☉ Ib, 7 × 10⁴ L☉ Iab and 3 × 10⁵ L☉ Ia.
    #[test]
    fn luminosity_orders_the_giants_and_supergiants() {
        let cases = [
            (50.0, LuminosityClass::Giant),
            (1_000.0, LuminosityClass::BrightGiant),
            (1e4, LuminosityClass::LessLuminousSupergiant),
            (7e4, LuminosityClass::Supergiant),
            (3e5, LuminosityClass::LuminousSupergiant),
        ];
        for (l, expected) in cases {
            let s = star(Phase::CoreHeliumBurning, 5.0, l, 4_600.0);
            assert_eq!(class_of(&s), expected, "{l} L☉");
        }
    }

    /// The subdwarf thresholds apply to main-sequence dwarfs only, and at the plan's [Fe/H].
    #[test]
    fn the_subdwarf_thresholds_are_the_plans() {
        let dwarf = star(Phase::MainSequence, 0.5, 0.04, 3_800.0);
        let at = |fe_h: f64| {
            classify_living(
                &dwarf,
                &Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO),
                VERSION,
            )
            .1
        };
        assert_eq!(at(-0.99), LuminosityClass::Dwarf);
        assert_eq!(at(-1.01), LuminosityClass::Subdwarf);
        assert_eq!(at(-1.69), LuminosityClass::Subdwarf);
        assert_eq!(at(-1.71), LuminosityClass::ExtremeSubdwarf);
        let pre = star(Phase::PreMainSequence, 0.5, 0.04, 3_800.0);
        let metal_poor = Composition::from_fe_h(Dex::new(-2.5), HeliumExcess::ZERO);
        assert_eq!(
            classify_living(&pre, &metal_poor, VERSION).1,
            LuminosityClass::Dwarf
        );
    }

    #[test]
    fn classes_are_written_as_catalogues_write_them() {
        let written: Vec<&str> = LuminosityClass::ALL.iter().map(|c| c.as_str()).collect();
        assert_eq!(
            written,
            [
                "Ia+", "Ia", "Iab", "Ib", "II", "III", "IV", "V", "sd", "esd"
            ]
        );
        let supergiants = LuminosityClass::ALL
            .iter()
            .filter(|c| c.is_supergiant())
            .count();
        assert_eq!(supergiants, 4);
        let prefixes = LuminosityClass::ALL
            .iter()
            .filter(|c| c.is_prefix())
            .count();
        assert_eq!(prefixes, 2);
    }
}
