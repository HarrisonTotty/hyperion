//! The area-mix Bond albedo (plan 14, P14.T51.b; decision-p14-t49c-split §1.5 and §2): a body's
//! Bond albedo as the area-weighted sum of its surface's components, A = Σ aᵢAᵢ, with the cloud
//! over them, and a gas envelope's from its class.
//!
//! Every piece is a pure function of the registry's rows and of plain values. T13.c's passes call
//! it on the record since P14.T54.a deleted the `p14-snapshot` lane's hold; T49.c's tripwire lists
//! this file as wired for that reason. Every optical value is read through the registry when it is
//! used (`Substance::condensed_optics` and its spherical albedos, and [`liquid_properties`]), with
//! no copied value, so that P14.T49.i's rows flow through with no edit here.
//!
//! # The host's light (decision-p14-t49c-split §2)
//!
//! Each component's Bond albedo is its spherical albedo by bin weighted by the light its hosts
//! send in each bin's edges, normalised over the bins given ([`band_weighted`]): a Planck curve at
//! each luminous host's effective temperature, its share of the host's light in each bin
//! integrated by an 8-node Gauss–Legendre rule ([`planck_share`]), weighted by the host's flux at
//! the body ([`HostShadeSum`]). The sim's stellar spectrum (`sky::colour::star_colour`) is not
//! used: a body's hosts reach it as P14.T12's `HostLight`, which carries no surface gravity. Both
//! weightings take the host's whole spectrum, so that a host whose light lies in the infrared, an
//! M dwarf's, darkens an ice (decision-p14-t49c-split §5.2):
//!
//! - **The solids' bins** are P14.T49.i's twelve, contiguous over 0.3–5 µm
//!   ([`SPECTRAL_BIN_EDGES_NM`]), whose first and last stand for the light outside them
//!   (`substance::optics`'s module documentation): the first bin takes all the light below 390 nm
//!   and the last all the light above 3,500 nm ([`optics_shares`]; main's ruling, 2026-10-19).
//! - **The liquids' bins** are P14.T49.h's fifteen, R08's bake bins over 380–760 nm
//!   ([`LIQUID_BIN_EDGES_NM`]), with the light below 380 nm and the light above 760 nm beside them
//!   ([`liquid_shares`], [`LIQUID_WEIGHTS`]).
//!
//! # The components
//!
//! - **A solid** ([`solid_bond`]): its row's spherical albedo by bin
//!   (`CondensedOptics::spherical_albedo_bins`), never its normal albedo `A_N`, a radiance factor,
//!   and never a normal-incidence Fresnel reflectance.
//! - **A liquid** ([`liquid_bond`]): per bin the cosine-weighted Fresnel reflectance of a flat
//!   surface `r_d`(n) ([`diffuse_fresnel_reflectance`]) plus the light its body returns,
//!   (1 − `r_d`)(1 − `r_i`) R∞ ÷ (1 − `r_i` R∞), with R∞ = 0.33 `b_b` ÷ (a + `b_b`) and `b_b` = b ÷ 2
//!   ([`SUBSURFACE_REFLECTANCE_COEFFICIENT`]'s sources) and `r_i` the surface's
//!   internal diffuse reflectance ([`internal_diffuse_reflectance`]), from P14.T49.h's record at
//!   the temperature its condensate lies at, each bin at its own index. A record without a or b
//!   has no body term: a deep, flat, pure liquid ([`BodyTerm`]). Its first bin stands for the light
//!   below 380 nm, as a solid's first bin does for the light below its data; the light above 760 nm
//!   takes `r_d` at its last bin's index, where the body term vanishes (decision-p14-t49c-split
//!   §2).
//! - **The regolith** ([`Regolith`]) on the surface the condensates leave: its lid's row, or while
//!   the passes do not know the lid a stand-in, on an airless rocky body the Moon's 0.11 exactly
//!   and a known lid's row at most 0.11 ([`airless_silicate_stand_in`]); from generator version 24
//!   a known lid's row weathered at the body's maturity ([`Regolith::weathered_lid`]), but an
//!   airless silicate lid younger than a regolith, which keeps the 0.11 at most.
//! - **A weathering deposit** (P14.T51.j's `ferric`, from 24): its area at its row's optics, a
//!   solid's ([`surface_bond`]'s `deposits`).
//! - **The cloud's condensable** weighs by 1 − d, its dominance ([`cloud_condensable`]).
//! - **The cloud** ([`cloud_fraction`], [`cloud_albedo`]): the state's cloud fraction times the
//!   condensable's availability, at the state's cloud albedo.
//!
//! # A gas envelope (Sudarsky, Burrows and Pinto 2000)
//!
//! [`giant_bond`]: the "Fiducial" Bond albedos of Sudarsky, Burrows and Pinto's (2000, ApJ 538,
//! 885) Tables 1A–1B by the host's effective temperature, flux-weighted over the hosts, and then by
//! the planet's effective temperature across the five classes' model planets, piecewise-linear so
//! that the albedo and the temperature converge together. Two labelled stand-ins replace class I's
//! column (decision-p14-t54a-ganymede §2–3) until P14.T24.h gives an envelope its albedo from its
//! own decks and hazes:
//!
//! - **class I's hazes** ([`CLASS_I_HAZE_RATIO`]): the Fiducial column times their own tholin-haze
//!   case's 0.48 ÷ 0.57 about every host, since the Fiducial, with no photochemical haze, is
//!   brighter than both measured cold H/He giants;
//! - **an ice giant's class I** ([`ICE_GIANT_CLASS_I_RATIO`]): the measured ice giants' mean, 0.32
//!   about a G2V star, scaled across hosts as the Fiducial class I is, since their classes hold no
//!   methane-absorbing giant.
//!
//! # The stand-ins ([`StandIn`])
//!
//! Each is labelled, never a silent default: a solid without optics takes water frost's; a
//! solution its solvent's record; a liquid without a record `r_d` at liquid nitrogen's index; a
//! liquid at a temperature no bound of its range holds its record at its first melt; an airless
//! rocky body whose lid is not known the Moon's 0.11, a known lid's airless silicate
//! regolith at most 0.11 (and, under the weathered area mix, a lava younger than a regolith), any
//! other lid not known a row by its material; a body with no luminous host a Sun's light.
//!
//! # The weathered area mix (P14.T54.b.e; from generator version 24)
//!
//! R10.T1.d's law ([`weathered_spherical_albedo`]) weathers a known lid's row at the body's
//! maturity m (P14.T51.d's, which a pass computes from the context it reads), each of the optics'
//! twelve bins in their order, at the bin's midpoint wavelength ([`bin_wavelength_nm`]), under the
//! row's own law (`CondensedColumns::phase_law`), at the grain density and `FeO_T` its palette
//! entry carries ([`Regolith::weathered_lid`]). A row whose `FeO_T` is not modelled, or whose albedo
//! is an observed, weathered value (`mars_dust`), is not weathered again: nothing darkens twice
//! (decision-p14-t51d-weathering §1.5). T51.b's space-weathering cap is deleted with it, but on
//! `basalt_reduced` ([`CAPPED_ROWS`]), which keeps it (decision-p14-t49k Q5): its graphite-bearing
//! row lies above 0.11, and the law, npFe⁰ per `FeO_T`, does not weather an Fe-poor, carbon-bearing
//! regolith (Shackelford et al. 2025, PSJ 6, 254, a deferred finding), so that it takes one
//! darkening, the cap's. An airless silicate lid on a surface younger than
//! [`REGOLITH_FORMATION_TIME`] (1 Myr, global volcanic resurfacing's class) is a lava with no
//! regolith yet, and keeps the cap too, under its own label ([`StandIn::UnweatheredLava`];
//! decision-p14-t54b-e-findings §A.5): the rows are fine powders, and its glassy or slab surface is
//! not modelled. An airless rocky body whose lid is not known keeps the Moon's 0.11, and a body
//! whose maturity is not modelled its stand-in, each labelled. `ferric`'s weathering deposit joins
//! the area mix at its row's optics ([`surface_bond`]).
//!
//! # The icy lid's regolith (P14.T49.m; from generator version 26)
//!
//! T49.i's `H2O` row is fresh 100 µm frost, 0.832 about a Sun, where measured icy satellites run
//! from 0.10 to 0.76 by their non-ice fraction, radiolysis and resurfacing
//! (decision-p14-t54a-ganymede §1.5). Behind its hold ([`ICY_REGOLITH_VERSION`]), an airless or
//! tenuous body's `H2O` regolith, a known lid's or the stand-in's row for ice, is ice with its
//! rock's lag ([`icy_lag_bins`]): T49.i's two-size intimate mixture (decision-p14-t49i-rock-mixtures
//! (b)) of the frost and an icy body's rock, `chondrite_ci`, at [`ICY_LAG_NON_ICE_SHARE`] by
//! volume, each grain's single-scattering albedo and phase asymmetry weighted by its cross-section
//! (Hapke 2012's rule, as the ruling states it) into van de Hulst's similarity relation. Fresh frost
//! stays where resurfacing outruns the lag: a tidal heat flux of at least
//! [`RESURFACING_TIDAL_FLUX`] ([`IcyResurfacing`]). Neither size, density nor exposure age is read
//! (the ruling: Enceladus and Ganymede are both about 60% rock; an exposure age is a weathering
//! law, P14.T54.b's kind).
//!
//! # The Rayleigh floor (P14.T24.h.a; decision-p14-t54a-passes §G3.5)
//!
//! The area mix has no Rayleigh term, so a thick air's Bond albedo is its surface's alone, which
//! under several bar can be physically impossible (`filler_c` e201's 0.046 under 23 bar at 23).
//! [`rayleigh_floor`] is the air's own Bond albedo over a black surface: its Rayleigh optical depth
//! at 550 nm from each gas row's cross section in the registry
//! ([`rayleigh`](crate::substance::rayleigh)) over the column `p_s` ÷ (g m̄), taken to each of the
//! optics' twelve bins at λ^−4.08, and each bin's conservative layer in the Eddington closure
//! ([`conservative_spherical_albedo`]), weighted by the hosts' light ([`HostShade::bands`]). The
//! snapshot path's Bond albedo is the larger of the area mix and the floor
//! (`surface_mix::floored`), from version 24. P14.T24.h.b replaces the max with the adding rule,
//! and adds the haze and the absorbers.
//!
//! [`liquid_properties`]: crate::substance::liquid_properties

use hyperion_surface::field::{LIQUID_BINS, LiquidProperties, MechanicsFamily};
use hyperion_surface::material::weathered_spherical_albedo;
use hyperion_surface::tables::space_weathering as form_tables;

use crate::math;
use crate::planetary::chem::partition::{
    Condensate, CondensateEntry, CondensatePhase, Condensates, Partition, WATER_SURFACE_HUMIDITY,
};
use crate::planetary::derive::PlanetClass;
use crate::planetary::derive::atmosphere::{
    Gas, PartialPressures, RunawayDeck, SurfaceMaterial, SurfaceState,
};
use crate::planetary::hooks::surface::regime::climate_condensable;
use crate::substance::optics::{
    CondensedOptics, ReferenceForm, SPECTRAL_BIN_EDGES_NM, SPECTRAL_BINS, held_at_first_melt,
    palette_feo_mass_fraction,
};
use crate::substance::rayleigh::{RAYLEIGH_REFERENCE_WAVELENGTH_NM, gas_cross_section};
use crate::substance::{
    Composition, CondensedPhase, LIQUID_BIN_EDGES_NM, ResolveLiquidError, SubstanceId, by_key,
    diagram, liquid_properties_at_pressure,
};
use crate::tables::gauss_legendre::{GL8_NODES, GL8_WEIGHTS};
use crate::units::consts::SOLAR_EFFECTIVE_TEMPERATURE_K;
use crate::units::{
    EarthFluxes, Kelvin, KilogramsPerSquareMetre, MetresPerSecondSquared, Pascals,
    WattsPerSquareMetre, Years,
};
use crate::{GENERATOR_VERSION, GeneratorVersion};

#[cfg(test)]
mod icy_lag_tests;
#[cfg(test)]
mod scattering_tests;
#[cfg(test)]
mod tests;

/// How many bins the condensed optics carry: P14.T49.i's twelve, contiguous over 0.3–5 µm
/// ([`SPECTRAL_BIN_EDGES_NM`], `CondensedOptics::spherical_albedo_bins`; decision-p14-t49c-split
/// §1.3), B, V and R the second, third and fourth.
pub(crate) const OPTICS_BANDS: usize = SPECTRAL_BINS;

/// How many weights a liquid's light takes: the light below its first bin, 380 nm, its
/// [`LIQUID_BINS`] bins over 380–760 nm ([`LIQUID_BIN_EDGES_NM`]), and the light above its last
/// bin, 760 nm ([`liquid_shares`]).
pub(crate) const LIQUID_WEIGHTS: usize = LIQUID_BINS + 2;

/// The second radiation constant c₂ = hc ÷ k, m K: 1.438 776 877 × 10⁻² (CODATA 2018, from the exact
/// h, c and k of the 2019 SI), the scale of a black body's spectrum in λT.
const SECOND_RADIATION_CONSTANT_M_K: f64 = 1.438_776_877e-2;

/// The share, 0 to 1, of the light of a black body at `temperature` that falls between the
/// wavelengths `shortest_nm` and `longest_nm` (nm, vacuum): (15 ÷ π⁴) ∫ x³ ÷ (eˣ − 1) dx over
/// x = c₂ ÷ λT between them, which is ∫ `B_λ` dλ over the band divided by σT⁴ ÷ π, by the sim's
/// 8-point Gauss–Legendre rule ([`GL8_NODES`], [`GL8_WEIGHTS`]; Abramowitz and Stegun 1964, Table
/// 25.4), exact for a polynomial of degree 15 in x, which a test holds within 10⁻¹² of a fine
/// Simpson sum on every band and bin from 2,000 to 40,000 K, summed in the table's order. A
/// `longest_nm` of +∞ gives the light beyond `shortest_nm`, x running from 0, where the integrand
/// goes smoothly to 0 as x² (the optics' last bin from 3,500 nm, [`optics_shares`]). Zero for a
/// temperature that is not positive and finite.
#[must_use]
pub(crate) fn planck_share(temperature: Kelvin, shortest_nm: f64, longest_nm: f64) -> f64 {
    let t = temperature.value();
    if !(t.is_finite() && t > 0.0) {
        return 0.0;
    }
    let x_at = |nm: f64| SECOND_RADIATION_CONSTANT_M_K / (nm * 1e-9 * t);
    let (low, high) = (x_at(longest_nm), x_at(shortest_nm));
    let (middle, half) = (f64::midpoint(low, high), (high - low) / 2.0);
    // A finite x whose eˣ − 1 overflows carries no light: x³ ÷ ∞ is 0.
    let planck = |x: f64| x * x * x / math::exp_m1(x);
    let sum = GL8_NODES
        .iter()
        .zip(GL8_WEIGHTS)
        .fold(0.0, |sum, (&node, weight)| {
            sum + weight * planck(middle + half * node)
        });
    let pi_squared = core::f64::consts::PI * core::f64::consts::PI;
    15.0 / (pi_squared * pi_squared) * half * sum
}

/// The shares of a black body's light at `temperature` in each band between consecutive `edges`
/// (nm, ascending), one fewer than the edges ([`planck_share`]).
#[must_use]
pub(crate) fn planck_shares<const N: usize>(temperature: Kelvin, edges: &[f64]) -> [f64; N] {
    debug_assert_eq!(edges.len(), N + 1, "N bands have N + 1 edges");
    let mut shares = [0.0; N];
    for (share, pair) in shares.iter_mut().zip(edges.windows(2)) {
        *share = planck_share(temperature, pair[0], pair[1]);
    }
    shares
}

/// The rest of a black body's light, 0 to 1, beyond the shares `shares` of it: 1 less their sum,
/// summed in their order, the whole spectrum's share being 1 ((15 ÷ π⁴) ∫₀^∞ x³ ÷ (eˣ − 1) dx = 1),
/// and at least 0, which rounding below a part in 10¹⁶ could otherwise cross.
#[must_use]
fn rest_of_the_light(shares: &[f64]) -> f64 {
    let rest = 1.0 - shares.iter().fold(0.0, |sum, &share| sum + share);
    if rest > 0.0 { rest } else { 0.0 }
}

/// The shares of a black body's light at `temperature` in each of the optics' bins
/// ([`SPECTRAL_BIN_EDGES_NM`]), of its whole spectrum (main's ruling, 2026-10-19): the first and
/// last bins stand for the light outside 0.3–5 µm (`substance::optics`'s module documentation), so
/// the last takes all the light above 3,500 nm ([`planck_share`] to +∞), the bins between their
/// edges' light ([`planck_share`]), and the first all the light below 390 nm, the rest
/// ([`rest_of_the_light`]), which the Gauss–Legendre rule over x from 390 nm to +∞ would not give
/// to the rule's accuracy. They sum to 1. All zero for a temperature that is not positive and
/// finite.
#[must_use]
pub(crate) fn optics_shares(temperature: Kelvin) -> [f64; OPTICS_BANDS] {
    let t = temperature.value();
    let mut shares = [0.0; OPTICS_BANDS];
    if !(t.is_finite() && t > 0.0) {
        return shares;
    }
    let last = OPTICS_BANDS - 1;
    for (bin, share) in shares.iter_mut().enumerate().skip(1) {
        let longest = if bin == last {
            f64::INFINITY
        } else {
            SPECTRAL_BIN_EDGES_NM[bin + 1]
        };
        *share = planck_share(temperature, SPECTRAL_BIN_EDGES_NM[bin], longest);
    }
    shares[0] = rest_of_the_light(&shares[1..]);
    shares
}

/// The shares of a black body's light at `temperature` that a liquid's albedo weighs
/// ([`LIQUID_WEIGHTS`]), of its whole spectrum, from the optics' shares `optics` at the same
/// temperature ([`optics_shares`]):
///
/// - the second to the sixteenth, the light in each of the liquids' bins ([`LIQUID_BIN_EDGES_NM`],
///   [`planck_shares`]);
/// - the last, all the light above 760 nm: its share up to the optics' first edge at or beyond it
///   (780 nm), and the optics' bins from there, the last of them to +∞;
/// - the first, all the light below 380 nm, the rest ([`rest_of_the_light`]).
///
/// They sum to 1. All zero for a temperature that is not positive and finite.
///
/// # Panics
///
/// Never: the optics' last edge, 5,000 nm, lies beyond the liquids' last, 760 nm.
#[must_use]
pub(crate) fn liquid_shares(
    temperature: Kelvin,
    optics: &[f64; OPTICS_BANDS],
) -> [f64; LIQUID_WEIGHTS] {
    let t = temperature.value();
    let mut shares = [0.0; LIQUID_WEIGHTS];
    if !(t.is_finite() && t > 0.0) {
        return shares;
    }
    let bins: [f64; LIQUID_BINS] = planck_shares(temperature, &LIQUID_BIN_EDGES_NM);
    shares[1..=LIQUID_BINS].copy_from_slice(&bins);
    let longest = LIQUID_BIN_EDGES_NM[LIQUID_BINS];
    let join = SPECTRAL_BIN_EDGES_NM
        .iter()
        .position(|&edge| edge >= longest)
        .expect("the optics' last edge, 5,000 nm, lies beyond the liquids' last, 760 nm");
    let gap = planck_share(temperature, longest, SPECTRAL_BIN_EDGES_NM[join]);
    shares[LIQUID_BINS + 1] = optics[join..].iter().fold(gap, |sum, &share| sum + share);
    shares[0] = rest_of_the_light(&shares[1..]);
    shares
}

/// Σ aᵢ wᵢ ÷ Σ wᵢ: the albedos `albedos` by bin weighted by the light `weights` in each bin,
/// normalised over the bins given, for any number of bins (the slices are of matching lengths).
/// Light of no weight in every bin weighs the bins equally.
#[must_use]
pub(crate) fn band_weighted(albedos: &[f64], weights: &[f64]) -> f64 {
    debug_assert_eq!(albedos.len(), weights.len(), "an albedo for every band");
    let (sum, total) = albedos
        .iter()
        .zip(weights)
        .fold((0.0, 0.0), |(sum, total), (&a, &w)| {
            (sum + a * w, total + w)
        });
    if total > 0.0 {
        sum / total
    } else {
        let (sum, count) = albedos
            .iter()
            .fold((0.0, 0.0), |(sum, count), &a| (sum + a, count + 1.0));
        if count > 0.0 { sum / count } else { 0.0 }
    }
}

/// How many classes Sudarsky, Burrows and Pinto's table gives: I to V.
const SUDARSKY_CLASSES: usize = 5;

/// The hosts of Sudarsky, Burrows and Pinto's (2000) Tables 1A–1B, by effective temperature, K:
/// M4V 3,200, K4V 4,620, G7V 5,530, G2V 5,770, F7V 6,240 and A8V 7,500 (Pecaut and Mamajek 2013,
/// ApJS 208, 9, Table 5, as the lane read it in the CDS catalogue, 2026-10-16).
const SUDARSKY_HOSTS_K: [f64; 6] = [3_200.0, 4_620.0, 5_530.0, 5_770.0, 6_240.0, 7_500.0];

/// The "Fiducial" Bond albedos of classes I to V about each of [`SUDARSKY_HOSTS_K`]: Sudarsky,
/// Burrows and Pinto 2000, ApJ 538, 885, Tables 1A–1B, p. 898, "Estimated Bond albedos of EGPs"
/// (the insolated profiles with full condensation), each the albedo weighted by the star's spectrum
/// over 0.3–2.5 µm (their eq. 5), read by the lane in the ApJ PDF and the arXiv source and checked
/// here against its text. The two "<0.01" (class IV about K4V and M4V; class III's 0.01 about M4V
/// is a value) are taken as 0.01, an upper bound, labelled.
const SUDARSKY_BOND: [[f64; SUDARSKY_CLASSES]; 6] = [
    [0.38, 0.56, 0.01, 0.01, 0.51],
    [0.48, 0.70, 0.05, 0.01, 0.53],
    [0.55, 0.79, 0.10, 0.02, 0.55],
    [0.57, 0.81, 0.12, 0.03, 0.55],
    [0.59, 0.84, 0.14, 0.03, 0.56],
    [0.63, 0.88, 0.17, 0.04, 0.57],
];

/// Class I's tholin-haze ratio, 0.48 ÷ 0.57 = 0.842: Sudarsky, Burrows and Pinto's (2000, §§6–7,
/// p. 901) class I about a G2V star under a photochemical haze of tholin (mixing ratio 10⁻⁸,
/// 0.05 µm grains), 0.48, over their haze-free Fiducial 0.57, applied to class I's column about
/// every host (M4V 0.32, G2V 0.48, A8V 0.53; decision-p14-t54a-ganymede §2).
///
/// Labelled: their G2V case stands for the hazes of every cold, `CH4`-bearing H/He giant under
/// ultraviolet. Both measured cold giants lie below the Fiducial: Jupiter at 0.503 ± 0.012 (Li et
/// al. 2018, Nat. Commun. 9, 3709), where Hanel et al. 1981's 0.343 ± 0.032 is superseded, and
/// Saturn at 0.41 ± 0.02 (Wang et al. 2024). P14.T24.h replaces it with an envelope's albedo from
/// its own decks and hazes.
pub(crate) const CLASS_I_HAZE_RATIO: f64 = 0.48 / 0.57;

/// An ice giant's class I as a share of class I's Fiducial column, 0.32 ÷ 0.57 = 0.561
/// (decision-p14-t54a-ganymede §3): 0.32 about a G2V star, the mean of the two measured ice giants'
/// Bond albedos, Uranus's 0.349 ± 0.016 as an orbital mean (Irwin et al. 2025) and Neptune's
/// 0.290 ± 0.067 (Pearl and Conrath 1991, JGR 96, 18921), scaled across hosts by the Fiducial class
/// I's host dependence (M4V 0.21, A8V 0.35), without [`CLASS_I_HAZE_RATIO`], since the measured
/// values hold their own hazes.
///
/// Labelled: an ice giant's darkness is its 2–4% of `CH4` absorbing in the red and the near
/// infrared, which none of Sudarsky, Burrows and Pinto's solar-composition classes holds, so the
/// class's only measured members stand for it, as the Moon's 0.11 stands for an airless rock. It
/// applies to a [`PlanetClass::IceGiant`] at and below class I's 130 K model planet, and is
/// interpolated to class II above it, as class I is. P14.T24.h replaces it.
pub(crate) const ICE_GIANT_CLASS_I_RATIO: f64 = 0.32 / 0.57;

/// The effective temperatures of the classes' model planets, K, the nodes a giant's albedo is
/// interpolated between: 130, 250, 600 and 1,200 K, their isolated profiles, and 1,700 K, the 1,200
/// K profile "scaled up" for class V (Sudarsky, Burrows and Pinto 2000, p. 887). The classes are
/// by effective temperature in the paper (I ≲ 150 K, II ∼ 250 K, III ≳ 350 K, IV 900–1,500 K and
/// V ≳ 1,500 K; pp. 885–886).
const SUDARSKY_PLANETS_K: [f64; SUDARSKY_CLASSES] = [130.0, 250.0, 600.0, 1_200.0, 1_700.0];

/// The value at `x` of the piecewise-linear curve through `values` at the ascending `nodes`, held at
/// its end values outside them.
#[must_use]
fn piecewise_linear(nodes: &[f64], values: &[f64], x: f64) -> f64 {
    debug_assert_eq!(nodes.len(), values.len());
    let (Some(&first), Some(&last)) = (nodes.first(), nodes.last()) else {
        return 0.0;
    };
    if x.is_nan() || x <= first {
        return values[0];
    }
    if x >= last {
        return values[values.len() - 1];
    }
    for (pair, value) in nodes.windows(2).zip(values.windows(2)) {
        if x <= pair[1] {
            let t = (x - pair[0]) / (pair[1] - pair[0]);
            return value[0] + t * (value[1] - value[0]);
        }
    }
    values[values.len() - 1]
}

/// Classes I to V's Fiducial Bond albedos about a host of effective temperature `host`: Sudarsky,
/// Burrows and Pinto's [`SUDARSKY_BOND`] interpolated linearly in the host's temperature between
/// its hosts, and held at M4V's below 3,200 K and A8V's above 7,500 K (a clamp, labelled: their
/// table spans no cooler or hotter host). A giant reads them through [`class_bonds`].
#[must_use]
pub(crate) fn sudarsky_bonds(host: Kelvin) -> [f64; SUDARSKY_CLASSES] {
    let mut bonds = [0.0; SUDARSKY_CLASSES];
    for (class, bond) in bonds.iter_mut().enumerate() {
        let column = SUDARSKY_BOND.map(|row| row[class]);
        *bond = piecewise_linear(&SUDARSKY_HOSTS_K, &column, host.value());
    }
    bonds
}

/// A gas envelope's Bond albedos about a host of effective temperature `host`, as a giant reads
/// them: classes I to V of [`sudarsky_bonds`] with class I's under its hazes
/// ([`CLASS_I_HAZE_RATIO`]), and an ice giant's class I ([`ICE_GIANT_CLASS_I_RATIO`]), each a
/// product of the Fiducial class I, so that [`class_i_bond`] and a one-host [`HostShade`] agree to
/// the bit.
#[must_use]
fn class_bonds(host: Kelvin) -> ([f64; SUDARSKY_CLASSES], f64) {
    let mut classes = sudarsky_bonds(host);
    let fiducial = classes[0];
    classes[0] = fiducial * CLASS_I_HAZE_RATIO;
    (classes, fiducial * ICE_GIANT_CLASS_I_RATIO)
}

/// The light a body's hosts give it, by bin, as the area mix reads it (P14.T51.b): each luminous
/// host's Planck shares in the optics' bins ([`optics_shares`]) and the liquids' weights
/// ([`liquid_shares`]), each of its whole spectrum, weighted by its flux at the body, its
/// gas-envelope albedos by class flux-weighted ([`class_bonds`]), the water cloud's albedo
/// calibrated on Earth ([`water_cloud_albedo`]) and the reference cloud column
/// ([`reference_cloud_column`]), summed once per body before its passes ([`HostShadeSum`]), its
/// own hosts' fluxes never shared across bodies. What those sums read alike, the two constants and
/// each luminous host's light at its effective temperature, a call chain's [`ShadeBook`] computes
/// once for all its bodies (lever (d)), where it holds one.
#[derive(Debug, Clone, Copy)]
pub(crate) struct HostShade {
    bands: [f64; OPTICS_BANDS],
    bins: [f64; LIQUID_WEIGHTS],
    giants: [f64; SUDARSKY_CLASSES],
    ice_giant: f64,
    water_cloud: f64,
    cloud_column: KilogramsPerSquareMetre,
}

impl HostShade {
    /// The light of one host of effective temperature `host`.
    #[must_use]
    pub(crate) fn of_host(host: Kelvin) -> Self {
        let mut sum = HostShadeSum::EMPTY;
        sum.add(EarthFluxes::new(1.0), host);
        sum.finish()
    }

    /// The light of a Sun, 5,772 K (IAU 2015 Resolution B3's nominal effective temperature), at
    /// which the photometry's references and the water cloud's calibration are taken, and which a
    /// body with no luminous host takes (its albedo then moves nothing).
    #[must_use]
    pub(crate) fn sun() -> Self {
        Self::of_host(Kelvin::new(SOLAR_EFFECTIVE_TEMPERATURE_K))
    }

    /// Its light in each of the optics' bins ([`optics_shares`]), unnormalised.
    #[must_use]
    pub(crate) const fn bands(&self) -> &[f64; OPTICS_BANDS] {
        &self.bands
    }

    /// Asserts it is `other`, every value to the bit (lever (d)'s check, and the tests').
    ///
    /// # Panics
    ///
    /// If any value differs in any bit.
    #[cfg(test)]
    pub(crate) fn assert_same_bits_as(&self, other: &Self) {
        use hyperion_testkit::float::assert_same_bits;
        let pairs = self
            .bands
            .iter()
            .zip(&other.bands)
            .chain(self.bins.iter().zip(&other.bins))
            .chain(self.giants.iter().zip(&other.giants));
        for (&a, &b) in pairs {
            assert_same_bits(a, b);
        }
        assert_same_bits(self.ice_giant, other.ice_giant);
        assert_same_bits(self.water_cloud, other.water_cloud);
        assert_same_bits(self.cloud_column.value(), other.cloud_column.value());
    }
}

/// One host's light as the area mix weighs it (lever (d), decision-p14-t54a-passes, "Cost
/// escalation (after Batch B)" §C.3): at its effective temperature, its shares in the optics' bins
/// ([`optics_shares`]) and the liquids' weights ([`liquid_shares`]) and its gas-envelope albedos by
/// class ([`class_bonds`]), each a function of the temperature alone, before any flux weighs them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct HostLightShares {
    temperature: Kelvin,
    bands: [f64; OPTICS_BANDS],
    bins: [f64; LIQUID_WEIGHTS],
    giants: [f64; SUDARSKY_CLASSES],
    ice_giant: f64,
}

impl HostLightShares {
    /// The light of a host of effective temperature `host`.
    #[must_use]
    pub(crate) fn of(host: Kelvin) -> Self {
        let bands = optics_shares(host);
        let bins = liquid_shares(host, &bands);
        let (giants, ice_giant) = class_bonds(host);
        Self {
            temperature: host,
            bands,
            bins,
            giants,
            ice_giant,
        }
    }

    /// Whether it is the light of a host of effective temperature `host`, bit for bit.
    #[must_use]
    fn is_at(&self, host: Kelvin) -> bool {
        self.temperature.value().total_cmp(&host.value()).is_eq()
    }
}

/// What the shades of one call chain's bodies share (lever (d), decision-p14-t54a-passes, "Cost
/// escalation (after Batch B)" §C.3): the host-independent water cloud's albedo
/// ([`water_cloud_albedo`]) and reference cloud column ([`reference_cloud_column`]), and each
/// distinct luminous host's light ([`HostLightShares`]), each computed once, in its constructor,
/// where every [`HostShadeSum`] would compute them for every body.
///
/// It is an immutable table of its inputs alone (the sim-determinism skill's "Allowed"), built by
/// the call chain that derives (`generate`, `snapshot_at` and the other queries that derive), never
/// held across chains. A shade read off it is the same bits as one computed afresh, since each
/// entry is the function a [`HostShadeSum`] would call, at the same temperature, summed in the same
/// order; a test holds every body's shade of the sample to it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ShadeBook {
    water_cloud: f64,
    cloud_column: KilogramsPerSquareMetre,
    hosts: Vec<HostLightShares>,
}

impl ShadeBook {
    /// The book of a call chain whose luminous hosts are of effective temperatures `hosts`: each
    /// distinct temperature's light once, a temperature that is not positive and finite skipped
    /// (a [`HostShadeSum`] adds no such host).
    #[must_use]
    pub(crate) fn new(hosts: impl IntoIterator<Item = Kelvin>) -> Self {
        let mut book = Self {
            water_cloud: water_cloud_albedo(),
            cloud_column: reference_cloud_column(),
            hosts: Vec::new(),
        };
        for host in hosts {
            book.add(host);
        }
        book
    }

    /// The book with the light of one more host of effective temperature `host`, a moon's
    /// planet's own glow, if it is not in it already (and is positive and finite). It takes the
    /// book, so that the caller decides where a copy is made.
    #[must_use]
    pub(crate) fn with_host(mut self, host: Kelvin) -> Self {
        self.add(host);
        self
    }

    /// Adds the light of a host of effective temperature `host` unless it holds it already or the
    /// temperature is not positive and finite.
    fn add(&mut self, host: Kelvin) {
        let t = host.value();
        if t.is_finite() && t > 0.0 && self.shares(host).is_none() {
            self.hosts.push(HostLightShares::of(host));
        }
    }

    /// The light of a host of effective temperature `host`, if it holds it.
    #[must_use]
    fn shares(&self, host: Kelvin) -> Option<&HostLightShares> {
        self.hosts.iter().find(|shares| shares.is_at(host))
    }
}

/// A [`HostShade`] being summed over a body's hosts, in their order: each luminous host's Planck
/// shares times its flux at the body, and its giant albedos by class ([`class_bonds`]) times that
/// flux.
///
/// Everything is linear in the hosts' fluxes, so it is summed once per body before the passes.
#[derive(Debug, Clone, Copy)]
pub(crate) struct HostShadeSum {
    bands: [f64; OPTICS_BANDS],
    bins: [f64; LIQUID_WEIGHTS],
    giants: [f64; SUDARSKY_CLASSES],
    ice_giant: f64,
    flux: f64,
}

impl HostShadeSum {
    /// No host yet.
    pub(crate) const EMPTY: Self = Self {
        bands: [0.0; OPTICS_BANDS],
        bins: [0.0; LIQUID_WEIGHTS],
        giants: [0.0; SUDARSKY_CLASSES],
        ice_giant: 0.0,
        flux: 0.0,
    };

    /// Adds a host of effective temperature `host` whose flux at the body is `flux`, its orbit-mean
    /// flux there. A host of no flux, or of a temperature that is not positive and finite, adds
    /// nothing.
    pub(crate) fn add(&mut self, flux: EarthFluxes, host: Kelvin) {
        self.add_from(flux, host, None);
    }

    /// [`add`](Self::add), with the host's light read off the call chain's book `book` where it
    /// holds it (lever (d)), and computed here otherwise.
    pub(crate) fn add_from(&mut self, flux: EarthFluxes, host: Kelvin, book: Option<&ShadeBook>) {
        let (flux, t) = (flux.value(), host.value());
        if !(flux.is_finite() && flux > 0.0 && t.is_finite() && t > 0.0) {
            return;
        }
        let computed;
        let light = if let Some(light) = book.and_then(|book| book.shares(host)) {
            light
        } else {
            computed = HostLightShares::of(host);
            &computed
        };
        for (sum, share) in self.bands.iter_mut().zip(light.bands) {
            *sum += flux * share;
        }
        for (sum, share) in self.bins.iter_mut().zip(light.bins) {
            *sum += flux * share;
        }
        for (sum, bond) in self.giants.iter_mut().zip(light.giants) {
            *sum += flux * bond;
        }
        self.ice_giant += flux * light.ice_giant;
        self.flux += flux;
    }

    /// The summed light, with the water cloud's calibration and the reference cloud column, each
    /// computed here once; a Sun's ([`HostShade::sun`], labelled) where no host added any.
    #[must_use]
    pub(crate) fn finish(self) -> HostShade {
        self.finish_from(None)
    }

    /// [`finish`](Self::finish), with the water cloud's calibration and the reference cloud column
    /// read off the call chain's book `book` if there is one (lever (d)). A body no host lights
    /// takes a Sun's computed afresh, as without a book: a rare body, whose albedo the light moves
    /// not at all.
    #[must_use]
    pub(crate) fn finish_from(self, book: Option<&ShadeBook>) -> HostShade {
        if self.flux.is_nan() || self.flux <= 0.0 {
            return HostShade::sun();
        }
        let (water_cloud, cloud_column) = book.map_or_else(
            || (water_cloud_albedo(), reference_cloud_column()),
            |book| (book.water_cloud, book.cloud_column),
        );
        HostShade {
            bands: self.bands,
            bins: self.bins,
            giants: self.giants.map(|sum| sum / self.flux),
            ice_giant: self.ice_giant / self.flux,
            water_cloud,
            cloud_column,
        }
    }
}

/// How a body's shade reads its call chain's [`ShadeBook`], a test-only switch of lever (d)
/// ([`with_shade_reads`]).
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShadeReads {
    /// Off the book where there is one, as the generator reads it.
    Book,
    /// Each body's own, as before the book: the unlevered path, the oracle.
    Unlevered,
    /// Both: each shade read off a book asserted the unlevered path's to the bit, and counted.
    Checked,
}

#[cfg(test)]
thread_local! {
    /// The switch on this thread, a test's own: tests run on threads of one process.
    static SHADE_READS: core::cell::Cell<ShadeReads> = const {
        core::cell::Cell::new(ShadeReads::Book)
    };
    /// The shades [`ShadeReads::Checked`] has checked on this thread.
    static CHECKED_SHADES: core::cell::Cell<u64> = const { core::cell::Cell::new(0) };
}

/// The book `book` a body's shade reads: itself, and in the tests none where they switch the book
/// off ([`ShadeReads::Unlevered`]).
#[must_use]
pub(crate) fn book_read(book: Option<&ShadeBook>) -> Option<&ShadeBook> {
    #[cfg(test)]
    if SHADE_READS.with(core::cell::Cell::get) == ShadeReads::Unlevered {
        return None;
    }
    book
}

/// Under [`ShadeReads::Checked`], asserts the shade `read`, read off the book `book`, is the
/// unlevered path's, `unlevered`, to the bit, and counts it; nothing where there is no book or
/// under any other setting.
#[cfg(test)]
pub(crate) fn check_shade(
    read: &HostShade,
    book: Option<&ShadeBook>,
    unlevered: impl FnOnce() -> HostShade,
) {
    if book.is_some() && SHADE_READS.with(core::cell::Cell::get) == ShadeReads::Checked {
        read.assert_same_bits_as(&unlevered());
        CHECKED_SHADES.with(|cell| cell.set(cell.get() + 1));
    }
}

/// `run`'s result with the bodies' shades reading their books as `reads` says, and how many
/// shades it checked against the unlevered path under [`ShadeReads::Checked`]. The switch is set
/// back after.
#[cfg(test)]
#[must_use]
pub(crate) fn with_shade_reads<R>(reads: ShadeReads, run: impl FnOnce() -> R) -> (R, u64) {
    let before = SHADE_READS.with(|cell| cell.replace(reads));
    CHECKED_SHADES.with(|cell| cell.set(0));
    let result = run();
    SHADE_READS.with(|cell| cell.set(before));
    (result, CHECKED_SHADES.with(core::cell::Cell::take))
}

/// A gas envelope's Bond albedo at its effective temperature `heated` (its equilibrium
/// temperature with its internal heat) about the hosts of `shade` (Sudarsky, Burrows and Pinto
/// 2000), for a body of class `class`: the hosts' flux-weighted albedos of classes I to V
/// ([`class_bonds`]), class I's under its hazes and, for a [`PlanetClass::IceGiant`], the ice
/// giants' class I, piecewise-linear between the classes' model planets at [`SUDARSKY_PLANETS_K`]
/// and held at class I's below 130 K and class V's above 1,700 K, so that it is continuous in the
/// temperature.
///
/// The classes are by effective temperature, as the paper has them, not by the equilibrium
/// temperature (P14.T51.b's plan text; a deviation, recorded). Class V's low-gravity case (g ≲ 10³
/// cm s⁻² is class V "even for `T_eff` < 1500 K", p. 897) is not modelled. A sub-Neptune takes
/// their classes (none is measured).
#[must_use]
pub(crate) fn giant_bond(shade: &HostShade, heated: Kelvin, class: PlanetClass) -> f64 {
    let mut nodes = shade.giants;
    match class {
        PlanetClass::IceGiant => nodes[0] = shade.ice_giant,
        PlanetClass::Rocky | PlanetClass::Icy | PlanetClass::SubNeptune | PlanetClass::GasGiant => {
        }
    }
    piecewise_linear(&SUDARSKY_PLANETS_K, &nodes, heated.value())
}

/// Class I's Bond albedo about a host of effective temperature `host` for a body of class `class`
/// ([`class_bonds`]): a giant's at any effective temperature up to class I's 130 K model planet
/// ([`giant_bond`]), the ice giants' stand-in for a [`PlanetClass::IceGiant`] and the hazy class I
/// otherwise.
#[must_use]
pub(crate) fn class_i_bond(host: Kelvin, class: PlanetClass) -> f64 {
    let (classes, ice_giant) = class_bonds(host);
    match class {
        PlanetClass::IceGiant => ice_giant,
        PlanetClass::Rocky | PlanetClass::Icy | PlanetClass::SubNeptune | PlanetClass::GasGiant => {
            classes[0]
        }
    }
}

/// A melt's Bond albedo, 0.10, dark lava (Essack, Seager and Pajusalu 2020, ApJ 898, 160), T13.c's
/// magma ocean's, read from its table: a melt has no P14.T49.h record yet (labelled).
#[must_use]
pub(crate) fn melt_bond() -> f64 {
    SurfaceState::MagmaOcean
        .albedo(SurfaceMaterial::Rock)
        .value()
}

/// The cosine-weighted (diffuse) Fresnel reflectance `r_d`(n) = 2 ∫₀¹ `R_F`(μ) μ dμ of a flat surface
/// of a dielectric of real index `n` > 1, for unpolarised light from outside: the classical closed
/// form
///
/// `r_d` = ½ + (n − 1)(3n + 1) ÷ 6(n + 1)² + n²(n² − 1)² ÷ (n² + 1)³ · ln((n − 1) ÷ (n + 1))
/// − 2n³(n² + 2n − 1) ÷ (n² + 1)(n⁴ − 1) + 8n⁴(n⁴ + 1) ÷ (n² + 1)(n⁴ − 1)² · ln n,
///
/// derived from the Fresnel equations (it is printed in the literature as Walsh's, 1926, from
/// memory, unread here), and held by a test within 10⁻¹⁰ of a 200,000-step midpoint integral of
/// the Fresnel reflectance (water's 0.0664 at 1.333, against 0.0200 at normal incidence;
/// decision-p14-t49c-split §2). Zero for an index that is not above 1.
#[must_use]
pub(crate) fn diffuse_fresnel_reflectance(n: f64) -> f64 {
    if !(n > 1.0 && n.is_finite()) {
        return 0.0;
    }
    let n2 = n * n;
    let n4 = n2 * n2;
    let quartic = n4 - 1.0;
    0.5 + (n - 1.0) * (3.0 * n + 1.0) / (6.0 * (n + 1.0) * (n + 1.0))
        + n2 * (n2 - 1.0) * (n2 - 1.0) / ((n2 + 1.0) * (n2 + 1.0) * (n2 + 1.0))
            * math::ln((n - 1.0) / (n + 1.0))
        - 2.0 * n2 * n * (n2 + 2.0 * n - 1.0) / ((n2 + 1.0) * quartic)
        + 8.0 * n4 * (n4 + 1.0) / ((n2 + 1.0) * quartic * quartic) * math::ln(n)
}

/// The internal diffuse reflectance `r_i` of the same surface from inside, for diffuse light: 1 −
/// (1 − `r_d`) ÷ n², with `r_d` its [`diffuse_fresnel_reflectance`] `outer` from outside. It follows
/// from reciprocity and the n² law of radiance (derived here; Lekner and Dorf 1988, Appl. Opt.
/// 27, 1278, unread), total internal reflection included: water's 0.4746 at 1.333, which a test
/// holds against the direct internal integral (decision-p14-t49c-split §2's 0.475).
#[must_use]
pub(crate) fn internal_diffuse_reflectance(n: f64, outer: f64) -> f64 {
    if !(n > 1.0 && n.is_finite()) {
        return 0.0;
    }
    1.0 - (1.0 - outer) / (n * n)
}

/// The deep subsurface reflectance's coefficient: R∞ = 0.33 `b_b` ÷ (a + `b_b`), as
/// decision-r11-palette-liquids §1.8 gives it, citing Morel and Prieur 1977 (Limnol. Oceanogr. 22,
/// 709), unread here. The `b_b` ÷ (a + `b_b`) form is the first-order term of Gordon, Brown and
/// Jacobs 1975's computed relation (Appl. Opt. 14, 417), as the science check found through IOCCG
/// Report 5, 2006, §1.5, which credits both papers with the subsurface reflectance's dependence on
/// `b_b` and a.
const SUBSURFACE_REFLECTANCE_COEFFICIENT: f64 = 0.33;

/// The light a liquid's body returns through its surface, in one bin ([`liquid_bin_albedo`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum BodyTerm {
    /// None: a record without its absorption or its scattering, a deep, flat, pure liquid
    /// (decision-p14-t49c-split §2), whose albedo is its surface's alone.
    Absent,
    /// The light a deep body of these coefficients returns.
    Coefficients {
        /// The absorption coefficient a, m⁻¹.
        absorption_per_m: f64,
        /// The scattering coefficient b, m⁻¹, of which the pure liquid's symmetric phase function
        /// sends half back.
        scattering_per_m: f64,
    },
}

impl BodyTerm {
    /// The body term of `record` in its bin `k`: its coefficients where it states both, else
    /// absent.
    #[must_use]
    fn of(record: &LiquidProperties, k: usize) -> Self {
        match (record.absorption_per_m, record.scattering_per_m) {
            (Some(a), Some(b)) => Self::Coefficients {
                absorption_per_m: a[k],
                scattering_per_m: b[k],
            },
            (None, _) | (_, None) => Self::Absent,
        }
    }
}

/// A flat, deep liquid's spherical albedo in one bin: its [`diffuse_fresnel_reflectance`] at the
/// index `n` plus the light its body `body` returns through the surface, (1 − `r_d`)(1 − `r_i`) R∞
/// ÷ (1 − `r_i` R∞), with R∞ = 0.33 `b_b` ÷ (a + `b_b`) of the absorption a and the backscattering
/// `b_b`, half the scattering b (the pure liquid's symmetric phase function). An absent body term,
/// or coefficients of no total or one not finite, leave the surface's alone: a deep, flat, pure
/// liquid (decision-p14-t49c-split §2).
#[must_use]
pub(crate) fn liquid_bin_albedo(n: f64, body: BodyTerm) -> f64 {
    let outer = diffuse_fresnel_reflectance(n);
    let BodyTerm::Coefficients {
        absorption_per_m: a,
        scattering_per_m: b,
    } = body
    else {
        return outer;
    };
    let backscattering = b / 2.0;
    let total = a + backscattering;
    if !(total > 0.0 && total.is_finite()) {
        return outer;
    }
    let deep = SUBSURFACE_REFLECTANCE_COEFFICIENT * backscattering / total;
    let inner = internal_diffuse_reflectance(n, outer);
    outer + (1.0 - outer) * (1.0 - inner) * deep / (1.0 - inner * deep)
}

/// A liquid's Bond albedo from its P14.T49.h record `record`, weighted by the light `bins` of its
/// host's whole spectrum ([`liquid_shares`]):
///
/// - in each of its bins, [`liquid_bin_albedo`] at the bin's own index and body term;
/// - below 380 nm, its first bin's albedo, which stands for the light below it as a solid's first
///   bin does for the light below its data (`substance::optics`'s module documentation; the
///   lane's reading, 2026-10-19), since the record gives no index there;
/// - above 760 nm, through P14.T49.i's infrared bins, `r_d` at its last bin's index, where the
///   body term vanishes, a growing with wavelength while n changes little
///   (decision-p14-t49c-split §2).
#[must_use]
pub(crate) fn liquid_bond(record: &LiquidProperties, bins: &[f64; LIQUID_WEIGHTS]) -> f64 {
    let mut albedos = [0.0; LIQUID_WEIGHTS];
    for (k, albedo) in albedos[1..=LIQUID_BINS].iter_mut().enumerate() {
        *albedo = liquid_bin_albedo(record.refractive_index[k], BodyTerm::of(record, k));
    }
    albedos[0] = albedos[1];
    albedos[LIQUID_BINS + 1] =
        diffuse_fresnel_reflectance(record.refractive_index[LIQUID_BINS - 1]);
    band_weighted(&albedos, bins)
}

/// A component's labelled stand-in (P14.T51.b; decision-p14-t49c-split §1.5 and §2): never a silent
/// default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum StandIn {
    /// A solid row without condensed optics takes water frost's, `H2O`'s row: the commonest frost,
    /// until its row has optics (P14.T49.i).
    FrostForAnUnmodelledSolid,
    /// A solution with no record of its own takes its solvent's, the largest species of its
    /// reference composition (Titan's `ch4_c2h6_n2`: `CH4`, 0.739 by mole), until P14.T49.h's
    /// solutions give a solution's index.
    SolventForASolution,
    /// A liquid row with no record (carbon dioxide's, until P14.T49.h's step 2b) takes `r_d` at liquid
    /// nitrogen's index at 550 nm, read from its record at its normal boiling point, for the light
    /// molecular liquids, whose visible indices lie at about 1.19–1.23 (from memory), with no body
    /// term.
    IndexForAnUnrecordedLiquid,
    /// A liquid at a temperature no bound of its liquid range holds, one that is not a number or
    /// lies above a range with no critical point, takes its own record at its first melt
    /// ([`liquid_record`]). A solution's solvent read so keeps the solvent's label
    /// ([`StandIn::SolventForASolution`]).
    RecordAtItsFirstMelt,
    /// An airless regolith of a known lid whose row is of the `Silicate` family takes at most the
    /// Moon's 0.11: space weathering's stand-in ([`airless_silicate_stand_in`], [`capped_lid`]),
    /// only where R10.T1.d's law cannot weather the lid (P14.T54.b.e's weathered area mix):
    /// [`CAPPED_ROWS`]' `basalt_reduced`, whose darkening is its carbon (decision-p14-t49k Q5), a
    /// row whose `FeO_T` is not modelled, and a pass with no maturity.
    SpaceWeathering,
    /// Under the weathered area mix (P14.T54.b.e), an airless lid whose row is of the `Silicate`
    /// family ([`capped_lid`]) on a surface younger than
    /// [`REGOLITH_FORMATION_TIME`] keeps T51.b's cap, the smaller of its row's Bond albedo and the
    /// Moon's 0.11 ([`airless_silicate_stand_in`]): it is a lava with no regolith yet, whose glassy
    /// or slab surface is not modelled, and its row is a fine powder's (decision-p14-t54b-e-findings
    /// §A.5).
    UnweatheredLava,
    /// An airless rocky body whose lid is not known (the passes' first pass, the passes' stand-in
    /// with no rotation law, a body with no crust) takes the Moon's 0.11 exactly (NASA's fact
    /// sheet, T13.c's present value; [`Regolith::bond`]).
    MoonForAnUnknownAirlessLid,
    /// Any other regolith whose lid is not known takes a row by its bulk's material, T49.c's
    /// `basalt` for rock and `H2O`'s fresh frost for ice ([`Regolith::stand_in_row`]).
    RowByMaterial,
}

impl StandIn {
    /// The label a reader states.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "P14.T54.a's bless note and census state the labels; the tests read them now"
        )
    )]
    #[must_use]
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::FrostForAnUnmodelledSolid => {
                "a solid without condensed optics takes water frost's, the commonest frost, until \
                 its row has optics (P14.T49.i)"
            }
            Self::SolventForASolution => {
                "a solution without a record takes its solvent's, the largest species of its \
                 reference composition, until P14.T49.h gives a solution's index"
            }
            Self::IndexForAnUnrecordedLiquid => {
                "a liquid without a record takes r_d at liquid nitrogen's 550 nm index, the light \
                 molecular liquids' (from memory, 1.19–1.23), until its row's record lands"
            }
            Self::RecordAtItsFirstMelt => {
                "a liquid at a temperature no bound of its range holds (not a number, or above a \
                 range with no critical point) takes its own record at its first melt"
            }
            Self::SpaceWeathering => {
                "an airless regolith whose lid's row is silicate takes at most the Moon's 0.11, \
                 space weathering's stand-in where the weathering law cannot weather it"
            }
            Self::UnweatheredLava => {
                "an airless silicate lid younger than a regolith: its lava's glassy surface is not \
                 modelled, and the Moon's 0.11 at most stands in"
            }
            Self::MoonForAnUnknownAirlessLid => {
                "an airless rocky body whose lid is not known (a first pass, a stand-in with no \
                 rotation law, a body with no crust) takes the Moon's 0.11"
            }
            Self::RowByMaterial => {
                "a regolith whose lid is not known (a first pass, a stand-in with no rotation \
                 law, a body with no crust) takes basalt for rock and water frost for ice"
            }
        }
    }
}

/// A component's Bond albedo, with the stand-in it took, if any.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Component {
    /// Its Bond albedo, 0 to 1.
    pub(crate) bond: f64,
    /// The stand-in it took, if any.
    pub(crate) stand_in: Option<StandIn>,
}

/// The row a solid without optics takes: water's, `H2O` ([`StandIn::FrostForAnUnmodelledSolid`]).
#[must_use]
fn water() -> SubstanceId {
    Gas::Water.substance()
}

/// The solid `id`'s Bond albedo under the light `bands` in the optics' bins: its spherical albedo
/// by bin (`CondensedOptics::spherical_albedo_bins`) weighted by the light ([`band_weighted`]),
/// never its normal albedo `A_N`. A row without condensed optics takes water frost's, labelled
/// ([`StandIn::FrostForAnUnmodelledSolid`]).
///
/// # Panics
///
/// Never: water's row has condensed optics (T49.c's table).
#[must_use]
pub(crate) fn solid_bond(id: SubstanceId, bands: &[f64; OPTICS_BANDS]) -> Component {
    if let Some(optics) = id.substance().condensed_optics() {
        return Component {
            bond: band_weighted(&optics.spherical_albedo_bins(), bands),
            stand_in: None,
        };
    }
    let frost = water()
        .substance()
        .condensed_optics()
        .expect("water's row has condensed optics (T49.c)");
    Component {
        bond: band_weighted(&frost.spherical_albedo_bins(), bands),
        stand_in: Some(StandIn::FrostForAnUnmodelledSolid),
    }
}

/// A liquid's P14.T49.h record at its reference composition as the area mix reads it
/// ([`liquid_record`]).
#[derive(Debug, Clone)]
enum LiquidRecord {
    /// Its record at its temperature, or held into its liquid range at the bound the temperature
    /// lies beyond (P14.T54.a's clamp, decision-r11-palette-liquids §1.7).
    InRange(LiquidProperties),
    /// Its record at its first melt, for a temperature no bound of its range holds: one that is not
    /// a number, or lies above a range with no critical point
    /// ([`StandIn::RecordAtItsFirstMelt`]).
    AtFirstMelt(LiquidProperties),
    /// No record: its row is not modelled as a liquid, a member its record needs is not modelled,
    /// or its composition is not.
    Unrecorded,
}

impl LiquidRecord {
    /// The record where it was read in its range, `None` otherwise.
    #[must_use]
    fn in_range(self) -> Option<LiquidProperties> {
        match self {
            Self::InRange(record) => Some(record),
            Self::AtFirstMelt(_) | Self::Unrecorded => None,
        }
    }
}

/// The record of the liquid `id` at `temperature` at its reference composition under `pressure`
/// ([`LiquidRecord`]), each case named so that a row with a record is never read as having none:
///
/// - in its liquid range, its record there;
/// - below its first melt, −∞ included, its record at its first melt;
/// - at or above its critical point, +∞ included, its record a part in a million below it;
/// - at a temperature no bound holds, not a number or above a range with no critical point, its
///   record at its first melt, labelled ([`StandIn::RecordAtItsFirstMelt`]);
/// - for a row not modelled as a liquid, a member its record needs not modelled (there, or at the
///   bound), or a composition not modelled, none.
///
/// Each record is the one the palette's liquid entry reads (`liquid_properties_at_pressure`): a
/// pure liquid's compressed to `pressure`, Pa, its index, scattering and viscosity at that density
/// (P14.T49.n's, from generator version 24; to 23 [`liquid_properties`]' saturated record), so that
/// the Bond albedo's liquid and the palette's read one `ρ_l`(T, p) at one state
/// (decision-p14-t54a-water-consistency's W4). A pressure at or under the liquid's vapour's, zero
/// included, gives the saturated record.
///
/// [`liquid_properties`]: crate::substance::liquid_properties
#[must_use]
fn liquid_record(id: SubstanceId, temperature: Kelvin, pressure: Pascals) -> LiquidRecord {
    let (lowest, below) =
        match liquid_properties_at_pressure(id, temperature, Composition::Reference, pressure) {
            Ok(record) => return LiquidRecord::InRange(record),
            Err(ResolveLiquidError::OutOfRange { lowest, below, .. }) => (lowest, below),
            // The row has no record at this temperature, and holding it elsewhere would not give
            // one. A component's errors cannot occur at the reference composition, which names
            // none.
            Err(
                ResolveLiquidError::NotALiquid { .. }
                | ResolveLiquidError::NotModelled { .. }
                | ResolveLiquidError::CompositionNotModelled { .. }
                | ResolveLiquidError::ComponentNotModelled { .. }
                | ResolveLiquidError::CompositionOutOfRange { .. },
            ) => return LiquidRecord::Unrecorded,
        };
    let t = temperature.value();
    let (held, in_range) = match below {
        _ if t < lowest.value() => (lowest, true),
        // A part in a million below the critical point, inside the range.
        Some(critical) if !t.is_nan() => (Kelvin::new(critical.value() * (1.0 - 1e-6)), true),
        // Not a number, or above a range with no critical point: no bound holds it.
        Some(_) | None => (lowest, false),
    };
    match liquid_properties_at_pressure(id, held, Composition::Reference, pressure) {
        Ok(record) if in_range => LiquidRecord::InRange(record),
        Ok(record) => LiquidRecord::AtFirstMelt(record),
        // `NotModelled`: a member the record needs (its density) is not modelled at the bound's
        // state, so the row has no record there. The rest cannot occur: the first call refused a
        // row that is not a liquid and a given composition before it read the temperature, the
        // reference composition names no component, and either bound lies inside the range.
        Err(
            ResolveLiquidError::NotModelled { .. }
            | ResolveLiquidError::NotALiquid { .. }
            | ResolveLiquidError::CompositionNotModelled { .. }
            | ResolveLiquidError::ComponentNotModelled { .. }
            | ResolveLiquidError::CompositionOutOfRange { .. }
            | ResolveLiquidError::OutOfRange { .. },
        ) => LiquidRecord::Unrecorded,
    }
}

/// One standard atmosphere, the pressure of a normal boiling point: 101,325 Pa, exact by
/// definition (the 10th CGPM, 1954, Resolution 4, from memory).
const STANDARD_ATMOSPHERE: Pascals = Pascals::new(101_325.0);

/// Liquid nitrogen's normal boiling point, at which [`StandIn::IndexForAnUnrecordedLiquid`] reads
/// its record: its row's saturation temperature under one [`STANDARD_ATMOSPHERE`] on the registry's
/// T49.b curve (NIST's WebBook gives 77.355 K, from memory).
///
/// # Panics
///
/// Never: nitrogen's row has a phase diagram whose liquid boils under one atmosphere (T49.b).
#[must_use]
fn nitrogen_normal_boiling_point() -> Kelvin {
    Gas::Nitrogen
        .substance()
        .substance()
        .phase_diagram()
        .and_then(|diagram| diagram.saturation_temperature(STANDARD_ATMOSPHERE))
        .expect(
            "nitrogen's row has a phase diagram whose liquid boils under one atmosphere (T49.b)",
        )
}

/// The wavelength at which [`StandIn::IndexForAnUnrecordedLiquid`] reads its index, nm: V's 550.
const STAND_IN_INDEX_WAVELENGTH_NM: f64 = 550.0;

/// The Bond albedo of a liquid with no record: `r_d` at liquid nitrogen's index at 550 nm, read from
/// its record ([`StandIn::IndexForAnUnrecordedLiquid`]), with no body term: its saturated record,
/// at its normal boiling point the liquid under its own vapour's one atmosphere, under either rule
/// of P14.T49.n's compression.
///
/// # Panics
///
/// Never: nitrogen's row has a liquid record at its normal boiling point (P14.T49.h).
#[must_use]
fn unrecorded_liquid_bond() -> f64 {
    let nitrogen = Gas::Nitrogen.substance();
    let record = liquid_record(nitrogen, nitrogen_normal_boiling_point(), Pascals::ZERO)
        .in_range()
        .expect("nitrogen's row has a liquid record at its normal boiling point (P14.T49.h)");
    diffuse_fresnel_reflectance(record.refractive_index_at(STAND_IN_INDEX_WAVELENGTH_NM))
}

/// The liquid entry `entry`'s Bond albedo under the light `bins` ([`liquid_shares`]), from its
/// P14.T49.h record at the temperature its condensate lies at in `partition`, under its surface
/// pressure ([`liquid_record`]: compressed to it from generator version 24, P14.T49.n): at a
/// temperature no bound of its range holds, its record at its first melt
/// ([`StandIn::RecordAtItsFirstMelt`]); a solution without a record takes its solvent's, under the
/// same pressure ([`StandIn::SolventForASolution`]), and a row without one `r_d` at liquid
/// nitrogen's index ([`StandIn::IndexForAnUnrecordedLiquid`]).
#[must_use]
pub(crate) fn liquid_entry_bond(
    entry: &CondensateEntry,
    partition: &Partition,
    bins: &[f64; LIQUID_WEIGHTS],
) -> Component {
    let liquid = entry.substance();
    let condensate = partition
        .condensates()
        .iter()
        .find(|c| c.liquid() == liquid);
    let temperature = condensate.map_or(partition.climate().mean(), Condensate::lies_at);
    let pressure = partition.surface_pressure();
    match liquid_record(liquid, temperature, pressure) {
        LiquidRecord::InRange(record) => {
            return Component {
                bond: liquid_bond(&record, bins),
                stand_in: None,
            };
        }
        LiquidRecord::AtFirstMelt(record) => {
            return Component {
                bond: liquid_bond(&record, bins),
                stand_in: Some(StandIn::RecordAtItsFirstMelt),
            };
        }
        LiquidRecord::Unrecorded => {}
    }
    let solvent = condensate
        .map(Condensate::substance)
        .filter(|&solvent| solvent != liquid);
    match solvent.map(|solvent| liquid_record(solvent, temperature, pressure)) {
        Some(LiquidRecord::InRange(record) | LiquidRecord::AtFirstMelt(record)) => Component {
            bond: liquid_bond(&record, bins),
            stand_in: Some(StandIn::SolventForASolution),
        },
        Some(LiquidRecord::Unrecorded) | None => Component {
            bond: unrecorded_liquid_bond(),
            stand_in: Some(StandIn::IndexForAnUnrecordedLiquid),
        },
    }
}

/// The Moon's Bond albedo, 0.11 (NASA's Moon fact sheet), T13.c's present airless-rock albedo,
/// read from its table: the cap of [`airless_silicate_stand_in`], and an airless rocky body's
/// whose lid is not known ([`StandIn::MoonForAnUnknownAirlessLid`]).
#[must_use]
fn moon_bond() -> f64 {
    SurfaceState::Airless.albedo(SurfaceMaterial::Rock).value()
}

/// The space-weathering stand-in (decision-p14-t49c-split §1.5; plan 14, P14.T51.b): an airless
/// body's regolith of a known lid whose row is of the `Silicate` family ([`capped_lid`]) takes the
/// smaller of its row's Bond albedo `row` and the Moon's 0.11
/// (NASA's fact sheet; T13.c's present value). A reference form is a fresh surface, and a fresh
/// lithology would make an airless rocky body far too bright; so it never brightens an airless
/// silicate body against today's value, and it lets an intrinsically dark row go darker. Labelled
/// ([`StandIn::SpaceWeathering`]). P14.T54.b.e weathers the area mix with R10.T1.d's law over
/// P14.T51.d's maturity instead ([`Regolith::weathered_lid`], from generator version 24;
/// decision-p14-t51d-weathering), and keeps this cap on
/// `basalt_reduced` alone ([`CAPPED_ROWS`]; decision-p14-t49k Q5: the law, npFe⁰ per `FeO_T`, does
/// not weather its Fe-poor, graphite-bearing row, which would leave a Mercury too bright), on a
/// known lid whose maturity is not modelled, and on a lava younger than a regolith
/// ([`REGOLITH_FORMATION_TIME`], [`StandIn::UnweatheredLava`]).
#[must_use]
pub(crate) fn airless_silicate_stand_in(row: f64) -> f64 {
    let cap = moon_bond();
    if row < cap { row } else { cap }
}

/// How the regolith's row is weathered (the lane's seam for P14.T51.d and P14.T54.b, 2026-10-16;
/// decision-p14-t51d-weathering).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum RegolithWeathering {
    /// The stand-in's arm: an airless regolith of a known lid whose row is silicate takes the
    /// space-weathering stand-in, the smaller of its row's Bond albedo and the Moon's 0.11
    /// ([`airless_silicate_stand_in`], labelled), and an airless rocky body's of a lid not known
    /// the Moon's 0.11 exactly; any other lid's row, or one under air, as it is
    /// ([`Regolith::bond`]). Under the weathered area mix, from generator version 24, it is a lid's
    /// that R10.T1.d's law cannot weather ([`Regolith::weathered_lid`]: `basalt_reduced`, a row
    /// whose `FeO_T` is not modelled, a pass with no maturity); to 23 it was every regolith's.
    StandIn,
    /// Not yet a regolith (decision-p14-t54b-e-findings §A.5): under the weathered area mix, an
    /// airless silicate lid on a surface younger than [`REGOLITH_FORMATION_TIME`] keeps the
    /// stand-in's arm, the smaller of its row's Bond albedo and the Moon's 0.11, under its own
    /// label ([`StandIn::UnweatheredLava`]), and is not weathered ([`Regolith::weathered_lid`]).
    UnweatheredLava,
    /// Weathered: its spherical albedos by bin, `spherical_bins`, which P14.T54.b.e computes from
    /// the lid's row by R10.T1.d's law at P14.T51.d's maturity ([`Regolith::weathered_lid`]), and
    /// which weigh by the host's bins as any solid's do ([`Regolith::weathered`]). Never capped.
    Weathered {
        /// The weathered spherical albedos in the optics' bins ([`OPTICS_BANDS`]).
        spherical_bins: [f64; OPTICS_BANDS],
    },
}

/// The regolith on the surface a body's condensates leave (P14.T51.b): "the crust's regolith".
///
/// Its row is an input of the area mix, a seam that P14.T54.a fills from T51.c's lid
/// ([`Regolith::with_lid`]), which each pass over a record takes at its own climate; where the lid
/// is not known (a first pass, the passes' stand-in, a body with no crust) the passes take a
/// stand-in by the material of the bulk their air reads ([`Regolith::of_material`],
/// [`Regolith::bond`]). Its fields are private and its constructors name each case, so that a
/// lid's row never sits unread under a weathered surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Regolith {
    /// Its lid's row, T51.c's from P14.T54.a; `None` where the lid is not known, and on a weathered
    /// regolith, whose band albedos carry its row.
    row: Option<SubstanceId>,
    /// The material of the body's bulk, which a lid not known reads: an airless rocky one takes
    /// the Moon's 0.11, and the rest a row by it.
    material: SurfaceMaterial,
    /// How it is weathered.
    weathering: RegolithWeathering,
    /// What its pass knows of the body's resurfacing, which an `H2O` row's lag reads under the
    /// icy regolith (P14.T49.m; [`with_resurfacing`](Self::with_resurfacing)): not known from
    /// every constructor.
    resurfacing: IcyResurfacing,
}

impl Regolith {
    /// The passes' regolith of a body of `material` whose lid is not known (a first pass, the
    /// passes' stand-in, a body with no crust), weathered by the stand-in.
    #[must_use]
    pub(crate) const fn of_material(material: SurfaceMaterial) -> Self {
        Self {
            row: None,
            material,
            weathering: RegolithWeathering::StandIn,
            resurfacing: IcyResurfacing::NOT_KNOWN,
        }
    }

    /// The regolith of a known lid's row `row` on a body of `material`, weathered by the stand-in:
    /// P14.T54.a's, from T51.c's lid, which each pass over a record takes at its own climate.
    #[must_use]
    pub(crate) const fn with_lid(row: SubstanceId, material: SurfaceMaterial) -> Self {
        Self {
            row: Some(row),
            material,
            weathering: RegolithWeathering::StandIn,
            resurfacing: IcyResurfacing::NOT_KNOWN,
        }
    }

    /// The same regolith on a body whose resurfacing its pass knows as `resurfacing` (P14.T49.m):
    /// an `H2O` row keeps its fresh frost where it is resurfaced, and takes its rock's lag
    /// otherwise, under the icy regolith ([`IcyRegolithRules::Mixture`]); every other row reads
    /// nothing of it.
    #[must_use]
    pub(crate) const fn with_resurfacing(self, resurfacing: IcyResurfacing) -> Self {
        Self {
            resurfacing,
            ..self
        }
    }

    /// A weathered regolith on a body of `material`, of spherical albedos by bin `spherical_bins`
    /// ([`RegolithWeathering::Weathered`]): P14.T54.b.e's, which computes them from the lid's row
    /// ([`weathered_lid`](Self::weathered_lid)), so that it holds no row of its own.
    #[must_use]
    pub(crate) const fn weathered(
        material: SurfaceMaterial,
        spherical_bins: [f64; OPTICS_BANDS],
    ) -> Self {
        Self {
            row: None,
            material,
            weathering: RegolithWeathering::Weathered { spherical_bins },
            resurfacing: IcyResurfacing::NOT_KNOWN,
        }
    }

    /// The regolith of a known lid's row `row` on a body of `material`, weathered as `at` gives
    /// ([`LidWeathering`]: P14.T51.d's maturity m, the pass's mean surface temperature, the surface
    /// age the maturity reads and the state of the pass's air), P14.T54.b.e's ([module](self)
    /// documentation, "The weathered area mix"):
    ///
    /// - [`CAPPED_ROWS`]' `basalt_reduced` keeps the stand-in's cap ([`with_lid`](Self::with_lid),
    ///   [`StandIn::SpaceWeathering`] on an airless body), one darkening, never weathered as well;
    /// - an airless lid whose row is silicate ([`capped_lid`] at `at`'s state) on a surface younger
    ///   than [`REGOLITH_FORMATION_TIME`] is a lava with no regolith yet: it keeps the stand-in's
    ///   arm, the smaller of its row's Bond albedo and the Moon's 0.11, under its own label
    ///   ([`StandIn::UnweatheredLava`]; decision-p14-t54b-e-findings §A.5), and is not weathered,
    ///   since its row is a fine powder's;
    /// - every other row with condensed optics is weathered ([`Regolith::weathered`]): each of its
    ///   spherical albedos by bin (`CondensedOptics::spherical_albedo_bins`), in bin order, by
    ///   [`weathered_spherical_albedo`] at the bin's midpoint ([`bin_wavelength_nm`]), under its
    ///   law (`CondensedColumns::phase_law`), at its solid's grain density at the temperature held
    ///   at its first melt ([`held_at_first_melt`], its palette entry's) and its palette `FeO_T`
    ///   ([`palette_feo_mass_fraction`]); never capped, so that `basalt_fe`'s row is darkened
    ///   once;
    /// - a row whose albedo is an observed, weathered value (`mars_dust`'s: its palette `FeO_T` is
    ///   `None` though its composition carries iron) takes its own bins to the bit, nothing
    ///   darkening twice (decision-p14-t51d-weathering §1.5), as does one of `FeO_T` 0 (an ice:
    ///   Europa's), and one at m = 0 (an air above the ion column: k and m +0);
    /// - a row whose `FeO_T` is not modelled (the chondrites, `phyllosilicate`, `cometary`), one
    ///   with no condensed optics, and one whose `FeO_T` is above 0 but whose solid has no grain
    ///   density at the temperature keep the stand-in (`with_lid`, capped where airless and
    ///   silicate): the law weathers only a row that carries its darkening agent
    ///   (decision-pre-23-review item 16), and there is nothing it can read;
    /// - under the icy regolith ([`IcyRegolithRules::Mixture`], P14.T49.m), an `H2O` lid keeps its
    ///   row (`with_lid`), which [`bond`](Self::bond) reads as its fresh frost or its rock's lag:
    ///   the law has no iron to darken there.
    ///
    /// # Panics
    ///
    /// If `at`'s maturity is not in [0, 1] (as [`weathered_spherical_albedo`]): P14.T51.d's
    /// maturity lies there.
    #[must_use]
    pub(crate) fn weathered_lid(
        row: SubstanceId,
        material: SurfaceMaterial,
        at: LidWeathering,
    ) -> Self {
        if row == water() && IcyRegolithRules::in_force() == IcyRegolithRules::Mixture {
            return Self::with_lid(row, material);
        }
        let substance = row.substance();
        let capped = CAPPED_ROWS.contains(&substance.key().as_str());
        let (Some(optics), Some(condensed), false) =
            (substance.condensed_optics(), substance.condensed(), capped)
        else {
            return Self::with_lid(row, material);
        };
        if at.surface_age.value() < REGOLITH_FORMATION_TIME.value()
            && capped_lid(row, at.state)
            && young_lava_capped()
        {
            return Self {
                row: Some(row),
                material,
                weathering: RegolithWeathering::UnweatheredLava,
                resurfacing: IcyResurfacing::NOT_KNOWN,
            };
        }
        let (maturity, temperature) = (at.maturity, at.temperature);
        let fresh = optics.spherical_albedo_bins();
        let feo = match (
            palette_feo_mass_fraction(row),
            substance.feo_mass_fraction(),
        ) {
            (Some(feo), _) if feo > 0.0 => feo,
            // Nothing to darken (an iron-free row), or darkened already (an observed, weathered
            // albedo, whose registry composition carries iron): the row's own bins, to the bit.
            (Some(_), _) | (None, Some(_)) => return Self::weathered(material, fresh),
            // No darkening agent the law can read: the stand-in's arm.
            (None, None) => return Self::with_lid(row, material),
        };
        let held = held_at_first_melt(row, temperature);
        let Some(density) = substance.grain_density(CondensedPhase::Solid, held) else {
            return Self::with_lid(row, material);
        };
        let law = condensed.phase_law();
        let mut spherical_bins = fresh;
        for (bin, albedo) in spherical_bins.iter_mut().enumerate() {
            *albedo = weathered_spherical_albedo(
                *albedo,
                bin_wavelength_nm(bin),
                law,
                density.value(),
                feo,
                maturity,
            );
        }
        Self::weathered(material, spherical_bins)
    }

    /// Whether it is weathered at a maturity ([`RegolithWeathering::Weathered`]), not on the
    /// stand-in's arm.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the weathered area mix's census reads it")
    )]
    #[must_use]
    pub(crate) const fn is_weathered(&self) -> bool {
        matches!(self.weathering, RegolithWeathering::Weathered { .. })
    }

    /// The row a regolith of `material` whose lid is not known takes on a body that is not an
    /// airless rocky one: T49.c's `basalt` for rock, the passes' land under air (Earth's cloud
    /// calibration takes it), and `H2O`'s fresh frost for ice (decision-p14-t49c-split §1.5, "an
    /// airless icy crust takes its fresh frost").
    ///
    /// # Panics
    ///
    /// Never: the registry holds `basalt`.
    #[must_use]
    pub(crate) fn stand_in_row(material: SurfaceMaterial) -> SubstanceId {
        match material {
            SurfaceMaterial::Rock => by_key("basalt").expect("the registry holds basalt"),
            SurfaceMaterial::Ice => water(),
        }
    }

    /// Its Bond albedo on a body in the state `state` under the light `bands`, by its weathering:
    ///
    /// - weathered, its albedos by bin weighted by the light;
    /// - under the stand-in, of a known lid: on an airless body whose lid's row is of the
    ///   `Silicate` family ([`capped_lid`]) the smaller of its row's albedo and the Moon's 0.11
    ///   ([`airless_silicate_stand_in`]), and otherwise its row as it is, so that an airless
    ///   water-ice lid (Europa's, on a rocky bulk) keeps its frost and an `Organic` one (a comet's)
    ///   its darkness, while Ceres's `phyllosilicate` and Mercury's `basalt_reduced` take the cap
    ///   (decision-p14-t54a-passes §3.4; decision-pre-23-review item 16);
    /// - under the stand-in, of a lid not known (the passes' first pass, the passes' stand-in, a
    ///   body with no crust): on an airless rocky body the Moon's 0.11 exactly, so that the
    ///   stand-in neither brightens nor darkens a body whose lid the passes do not know
    ///   (decision-p14-t49c-split §1.5), and on any other the row by its material
    ///   ([`Regolith::stand_in_row`]), each labelled;
    /// - not yet a regolith ([`RegolithWeathering::UnweatheredLava`]), as under the stand-in of a
    ///   known lid, its cap labelled [`StandIn::UnweatheredLava`];
    /// - under the icy regolith ([`IcyRegolithRules::Mixture`], P14.T49.m), an `H2O` row on an
    ///   airless or tenuous body that is not resurfaced ([`IcyResurfacing`]): its rock's lag
    ///   ([`icy_lag_bins`]) weighted by the light, a stand-in's row keeping its label.
    ///
    /// The one predicate the passes, the record and P14.T51.i's cold traps' bare regolith read.
    #[must_use]
    pub(crate) fn bond(&self, state: SurfaceState, bands: &[f64; OPTICS_BANDS]) -> Component {
        if let Some(lagged) = self.icy_lag(state, bands) {
            return lagged;
        }
        let cap = match self.weathering {
            RegolithWeathering::Weathered { spherical_bins } => {
                return Component {
                    bond: band_weighted(&spherical_bins, bands),
                    stand_in: None,
                };
            }
            RegolithWeathering::StandIn => StandIn::SpaceWeathering,
            RegolithWeathering::UnweatheredLava => StandIn::UnweatheredLava,
        };
        // A tenuous air's bare surface is weathered as an airless one's (P14.T53.c).
        let airless = matches!(state, SurfaceState::Airless | SurfaceState::Tenuous);
        match self.row {
            Some(row) if capped_lid(row, state) => Component {
                bond: airless_silicate_stand_in(solid_bond(row, bands).bond),
                stand_in: Some(cap),
            },
            Some(row) => solid_bond(row, bands),
            None if airless && self.material == SurfaceMaterial::Rock => Component {
                bond: moon_bond(),
                stand_in: Some(StandIn::MoonForAnUnknownAirlessLid),
            },
            None => Component {
                bond: solid_bond(Self::stand_in_row(self.material), bands).bond,
                stand_in: Some(StandIn::RowByMaterial),
            },
        }
    }

    /// Its Bond albedo under the light `bands` as P14.T49.m's lag, on a body in the state `state`:
    /// `Some` for an `H2O` row (a known lid's, or the stand-in's row for ice, labelled
    /// [`StandIn::RowByMaterial`]) on an airless or tenuous body that is not resurfaced, under
    /// [`IcyRegolithRules::Mixture`]; `None` for every other regolith, which [`bond`](Self::bond)
    /// takes as before.
    #[must_use]
    fn icy_lag(&self, state: SurfaceState, bands: &[f64; OPTICS_BANDS]) -> Option<Component> {
        let stand_in = match (self.row, self.weathering) {
            (Some(row), RegolithWeathering::StandIn) if row == water() => None,
            (None, RegolithWeathering::StandIn) if self.material == SurfaceMaterial::Ice => {
                Some(StandIn::RowByMaterial)
            }
            _ => return None,
        };
        // A tenuous air's bare surface is weathered as an airless one's (P14.T53.c).
        let airless = matches!(state, SurfaceState::Airless | SurfaceState::Tenuous);
        if !airless
            || self.resurfacing.resurfaced()
            || IcyRegolithRules::in_force() == IcyRegolithRules::Frost
        {
            return None;
        }
        Some(Component {
            bond: band_weighted(&icy_lag_bins(), bands),
            stand_in,
        })
    }
}

/// The generator version from which an airless body's `H2O` regolith is ice with its rock's lag
/// where it is not resurfaced (P14.T49.m; decision-p14-t54a-ganymede §1.5 and §1.8): 26, the
/// train after 25's.
///
/// It is this rule's one hold. Below it [`IcyRegolithRules::in_force`] is
/// [`IcyRegolithRules::Frost`]: every `H2O` regolith is T49.i's fresh frost, and every value on
/// the record is version 25's bit for bit. 26's bump deletes this hold, [`IcyRegolithRules`] with
/// its `Frost` arm, the guards that read it in [`Regolith::weathered_lid`] and
/// [`Regolith::bond`], the tests' switch and its tripwire
/// (`the_icy_regolith_is_held_only_until_the_27_bump`).
pub(crate) const ICY_REGOLITH_VERSION: GeneratorVersion = GeneratorVersion::new(27);

/// Whether an `H2O` regolith takes its rock's lag (its hold, [`ICY_REGOLITH_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum IcyRegolithRules {
    /// Version 25's and before: every `H2O` regolith is T49.i's fresh 100 µm frost.
    Frost,
    /// Version 26's: an airless or tenuous body's `H2O` regolith is ice with its rock's lag
    /// ([`icy_lag_bins`]), but where it is resurfaced ([`IcyResurfacing`]).
    Mixture,
}

impl IcyRegolithRules {
    /// The rules this generator version gives: the mixture from [`ICY_REGOLITH_VERSION`], the
    /// frost below it; in the tests, the rules a test's switch sets on its thread
    /// ([`with_icy_regolith`]).
    #[must_use]
    pub(crate) fn in_force() -> Self {
        #[cfg(test)]
        if let Some(rules) = ICY_REGOLITH_RULES.with(core::cell::Cell::get) {
            return rules;
        }
        if GENERATOR_VERSION >= ICY_REGOLITH_VERSION {
            Self::Mixture
        } else {
            Self::Frost
        }
    }
}

#[cfg(test)]
thread_local! {
    /// The icy regolith's rules a test sets on its thread, `None` for the version's.
    static ICY_REGOLITH_RULES: core::cell::Cell<Option<IcyRegolithRules>> =
        const { core::cell::Cell::new(None) };
}

/// `run`'s result with the icy regolith's rules `rules` on this thread, for the tests to reach
/// version 26's ahead of the bump; the version's rules are set back after, a panic in `run`
/// included. A record derives lazily, so a test reads it inside the switch. 26's bump deletes it
/// with the hold.
#[cfg(test)]
pub(crate) fn with_icy_regolith<R>(rules: IcyRegolithRules, run: impl FnOnce() -> R) -> R {
    /// Sets the thread's rules back as it drops.
    struct Restore(Option<IcyRegolithRules>);
    impl Drop for Restore {
        fn drop(&mut self) {
            ICY_REGOLITH_RULES.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(ICY_REGOLITH_RULES.with(|cell| cell.replace(Some(rules))));
    run()
}

/// The tidal heat flux from which an icy surface is resurfaced faster than its lag forms, and keeps
/// its fresh frost (P14.T49.m): 0.01 W m⁻² (the lane's lean, 2026-10-31).
///
/// - **What it stands for.** Enceladus, the least heated icy body seen resurfacing today, radiates
///   15.8 ± 3.1 GW from its south polar terrain (Howett et al. 2011, JGR 116, E03003; from
///   memory): 19.8 mW m⁻² over its 252.1 km sphere, 15.9 mW m⁻² at 1σ low. Europa's surface,
///   some 60 Myr old, is estimated at tens of mW m⁻² (from memory). 10 mW m⁻² lies under both.
/// - **What lies under it.** Ganymede's and Callisto's present tides, under 1 mW m⁻² (from
///   memory), under shells of 100–150 km whose surfaces are old (Nimmo and Pappalardo 2016, JGR
///   Planets 121, 1378, as [`ICE_SHELL_LIMIT`]'s documentation cites them): Ganymede's grooved
///   terrain was resurfaced in a past tidal epoch (from memory), which the generator's present
///   tides do not hold, and Callisto's never was.
/// - **What it is not.** A step, not a resurfacing rate set against a lag's growth; and the
///   generator's own tides decide it, so a body they under-heat (its Enceladus, at 5 µW m⁻²,
///   decision-p14-t56d-membrane §2.4) takes the lag.
///
/// [`ICE_SHELL_LIMIT`]: crate::planetary::hooks::figures::interior::ICE_SHELL_LIMIT
pub(crate) const RESURFACING_TIDAL_FLUX: WattsPerSquareMetre = WattsPerSquareMetre::new(0.01);

/// What a pass knows of its body's resurfacing, which an `H2O` regolith's lag reads (P14.T49.m;
/// [`Regolith::with_resurfacing`]): its tide and its volatile frost.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct IcyResurfacing {
    /// The tidal heat flux raised in the body (P14.T56.d), its figures' (`PassFigures`'s); `None`
    /// where it is not known: the passes' stand-in, a pass with no figures in its context (the
    /// first, C₀), and a body with no mass or radius.
    tidal: Option<WattsPerSquareMetre>,
    /// Whether its ice holds a volatile air (the pass's air's `ice_holds_a_volatile`).
    frost: VolatileFrost,
}

/// Whether a body's ice holds a volatile air, nitrogen, methane or argon (P14.T53.c), whose frost
/// cycles over its ground ([`IcyResurfacing`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum VolatileFrost {
    /// Its ice holds none.
    None,
    /// Its ice holds one.
    Held,
}

impl IcyResurfacing {
    /// Nothing known: no tide, no volatile frost. Its `H2O` regolith takes the lag, the old
    /// surface's.
    pub(crate) const NOT_KNOWN: Self = Self {
        tidal: None,
        frost: VolatileFrost::None,
    };

    /// A body whose tidal heat flux is `tidal`, `None` where it is not known, and whose ice holds
    /// a volatile air or none, as `frost` says.
    #[must_use]
    pub(crate) const fn new(tidal: Option<WattsPerSquareMetre>, frost: VolatileFrost) -> Self {
        Self { tidal, frost }
    }

    /// Whether the body is resurfaced faster than its lag forms, so that its `H2O` regolith keeps
    /// its fresh frost:
    ///
    /// - a tidal heat flux of at least [`RESURFACING_TIDAL_FLUX`], never where it is not known or
    ///   not a number: Europa's and Enceladus's class;
    /// - or a volatile air its ice holds (the lane's lean, 2026-10-31): its frost's seasonal cycle
    ///   refreshes the ground it does not cover, Triton's and Pluto's class, whose surfaces are
    ///   young or frost-covered (decision-p14-t51-batch E2's Triton 0.72–0.92). An icy body warm
    ///   enough to lose its nitrogen to Jeans's test, Ganymede's and Callisto's, holds none.
    #[must_use]
    pub(crate) fn resurfaced(self) -> bool {
        let frost = match self.frost {
            VolatileFrost::Held => true,
            VolatileFrost::None => false,
        };
        frost
            || self
                .tidal
                .is_some_and(|flux| flux >= RESURFACING_TIDAL_FLUX)
    }
}

/// The volume share of rock in an `H2O` regolith's lag ([`icy_lag_bins`]): 0.08, a calibration on
/// one body (labelled).
///
/// - **Its source.** It is the share at which the lag gives the measured Ganymede's bolometric
///   Bond albedo, 0.35 ± 0.03 (Squyres and Veverka 1982, Icarus 52; Buratti 1991, Icarus 92, 312;
///   decision-p14-t54a-ganymede §1.5 and §Sources), about a Sun: 0.3500 (`icy_lag_tests`'s
///   inversion). It is T13.c's airless ice, whose value the escape's calibration and T53.c's
///   110 K surfaces read; the window 0.32–0.38 is shares 0.066–0.097.
/// - **The bodies it misses.** Callisto, 0.11 ± 0.02 at a share of 0.47, whose generated twin has
///   Ganymede's bulk and tide, and whose darker lag is thermal segregation (Spencer 1987, Icarus 69,
///   297; Moore et al. 2004), a budget not built; the Uranian moons, 0.10–0.23 in the visible
///   (Karkoschka 2001), darker by radiolysis, not modelled; Saturn's mid-sized moons, 0.43–0.68
///   (Buratti et al. 2022), brighter under Enceladus's E-ring frost. Each is a deferred finding.
pub(crate) const ICY_LAG_NON_ICE_SHARE: f64 = 0.08;

/// The rock of an `H2O` regolith's lag ([`icy_lag_bins`]): `chondrite_ci`, the class the crust's
/// rule gives an icy body's rock altered in place by its ice at an ice-to-rock mass ratio of at
/// least 0.4 (`crust::CI_ICE_TO_ROCK`; Ganymede's and Callisto's are about 1), the dark,
/// carbonaceous-chondrite-like lag of Callisto's spectra (Spencer 1987, Icarus 70, 99, as
/// `figures::lag`'s documentation cites it). The body's own ratio is not read (the lane's lean,
/// 2026-10-31): `chondrite_cm`'s lag is 0.003 brighter at the same share about a Sun.
const ICY_LAG_ROCK: &str = "chondrite_ci";

/// The spherical albedos in the optics' bins of an `H2O` regolith's lag (P14.T49.m): T49.i's
/// two-size intimate mixture (decision-p14-t49i-rock-mixtures (b)) of the `H2O` row's fresh frost
/// and [`ICY_LAG_ROCK`] at [`ICY_LAG_NON_ICE_SHARE`] by volume ([`two_size_mixture`]). It reads
/// the registry's rows alone, so that it is the same for every body, and its Bond albedo follows
/// the host's light.
///
/// # Panics
///
/// Never: both rows have condensed optics (P14.T49.i).
#[must_use]
pub(crate) fn icy_lag_bins() -> [f64; OPTICS_BANDS] {
    let frost = water()
        .substance()
        .condensed_optics()
        .expect("water's row has condensed optics (T49.c)");
    let rock = by_key(ICY_LAG_ROCK)
        .and_then(|row| row.substance().condensed_optics())
        .expect("the registry holds chondrite_ci, with condensed optics (P14.T49.i)");
    two_size_mixture(
        (frost, 1.0 - ICY_LAG_NON_ICE_SHARE),
        (rock, ICY_LAG_NON_ICE_SHARE),
    )
}

/// The spherical albedos in the optics' bins of an intimate mixture of two rows' grains, each a
/// row's condensed optics and its volume fraction (P14.T49.m; decision-p14-t49i-rock-mixtures §2.1),
/// each row's grains at its reference form's diameter D and under its form's photometry:
///
/// 1. each row's single-scattering albedo w in the bin, its form's tabled spherical albedo
///    inverted (`hyperion-fit`'s `space_weathering` tables, `substance_albedos`' own solution);
/// 2. the mixture's, weighted by cross-section, volume fraction over diameter: w̄ = Σ (f ÷ D) w ÷ Σ
///    (f ÷ D) (Hapke 2012, its section not read, as the ruling states it), and its phase
///    asymmetry by the light each part scatters, ξ̄ = Σ (f ÷ D) w ξ ÷ Σ (f ÷ D) w, as Hapke's
///    mixture averages its phase function (from memory);
/// 3. its spherical albedo by van de Hulst's similarity relation (1974, A&A 35, 209), the one the
///    rows' own albedos are (`substance::optics`' module documentation): the isotropic medium of
///    albedo w* = w̄ (1 − ξ̄) ÷ (1 − w̄ ξ̄), read on the frost form's table at the w that its
///    asymmetry takes to w*.
///
/// A frost row alone gives its own bins back within rounding, and a regolith row alone its own
/// within the tables' interpolation, under 10⁻³ (`icy_lag_tests`).
#[must_use]
fn two_size_mixture(
    (first, first_share): (&CondensedOptics, f64),
    (second, second_share): (&CondensedOptics, f64),
) -> [f64; OPTICS_BANDS] {
    let part = |optics: &CondensedOptics, share: f64| {
        let form = optics.reference_form();
        let weight = share / form.grain_diameter().value();
        let asymmetry = form.photometry().phase_function().asymmetry();
        (
            optics.spherical_albedo_bins(),
            form_spherical(form),
            weight,
            asymmetry,
        )
    };
    let (a, a_table, a_weight, a_asymmetry) = part(first, first_share);
    let (b, b_table, b_weight, b_asymmetry) = part(second, second_share);
    let frost = ReferenceForm::Frost;
    let frost_asymmetry = frost.photometry().phase_function().asymmetry();
    core::array::from_fn(|bin| {
        let (a_w, b_w) = (
            grain_albedo_of(a_table, a[bin]),
            grain_albedo_of(b_table, b[bin]),
        );
        let scattered = a_weight * a_w + b_weight * b_w;
        if scattered <= 0.0 {
            return 0.0;
        }
        let w = scattered / (a_weight + b_weight);
        let xi = (a_weight * a_w * a_asymmetry + b_weight * b_w * b_asymmetry) / scattered;
        let isotropic = w * (1.0 - xi) / (1.0 - w * xi);
        let on_frost = isotropic / (1.0 - frost_asymmetry + isotropic * frost_asymmetry);
        spherical_of_grain(form_spherical(frost), on_frost)
    })
}

/// The spherical albedo `A_S` of the reference form `form` at each node of the tables' w
/// (`hyperion-fit`'s `space_weathering`, R10.T1.d's, by `substance_albedos`' own solution).
#[must_use]
const fn form_spherical(form: ReferenceForm) -> &'static [f64; form_tables::W.len()] {
    match form {
        ReferenceForm::Frost => &form_tables::FROST_SPHERICAL_ALBEDO,
        ReferenceForm::Regolith => &form_tables::REGOLITH_SPHERICAL_ALBEDO,
    }
}

/// The spherical albedo on the column `column` at single-scattering albedo `w`, linear in w between
/// the nodes and its end node's beyond them (as R10.T1.d's law reads it).
#[must_use]
fn spherical_of_grain(column: &[f64; form_tables::W.len()], w: f64) -> f64 {
    let nodes = &form_tables::W;
    let last = nodes.len() - 1;
    let above = nodes.partition_point(|&node| node <= w);
    if above == 0 {
        return column[0];
    }
    if above > last {
        return column[last];
    }
    let k = above - 1;
    column[k] + (w - nodes[k]) / (nodes[k + 1] - nodes[k]) * (column[k + 1] - column[k])
}

/// The single-scattering albedo whose spherical albedo on the ascending column `column` is
/// `albedo`: the column inverted linearly between its nodes; 1 above its top and 0 at or below 0
/// (as R10.T1.d's law inverts it).
#[must_use]
fn grain_albedo_of(column: &[f64; form_tables::W.len()], albedo: f64) -> f64 {
    let nodes = &form_tables::W;
    let last = nodes.len() - 1;
    if albedo <= 0.0 {
        return 0.0;
    }
    if albedo > column[last] {
        return 1.0;
    }
    let above = column.partition_point(|&value| value <= albedo);
    if above == 0 {
        return nodes[0];
    }
    if above > last {
        return nodes[last];
    }
    let k = above - 1;
    nodes[k] + (albedo - column[k]) / (column[k + 1] - column[k]) * (nodes[k + 1] - nodes[k])
}

/// Whether a known lid's row `row` on a body in the state `state` takes the space-weathering
/// stand-in's cap ([`airless_silicate_stand_in`]): an airless body whose lid's row is of the
/// `Silicate` mechanics family (decision-p14-t54a-passes §3.4, the six points' item 4). Keyed on
/// the lid's row, not the bulk's material, so that an airless water-ice lid on a rocky bulk
/// (Europa's) keeps its frost, and a `phyllosilicate` lid on an icy bulk (Ceres's) takes the cap.
/// A tenuous air's surface takes it too (P14.T53.c): its micrometeoroids still reach the ground
/// (P14.T51.d's dust share, under 0.3 kg m⁻² of column), and the maturity saturates over the age.
#[must_use]
pub(crate) fn capped_lid(row: SubstanceId, state: SurfaceState) -> bool {
    matches!(state, SurfaceState::Airless | SurfaceState::Tenuous)
        && row
            .substance()
            .condensed()
            .is_some_and(|condensed| condensed.mechanics() == MechanicsFamily::Silicate)
}

/// The lids whose rows keep the space-weathering stand-in's cap under the weathered area mix
/// ([`Regolith::weathered_lid`]): `basalt_reduced` alone, a closed list for the composition audit
/// (decision-p14-t49k Q5; P14.T54.b's plan text). R10.T1.d's law darkens by npFe⁰ per unit of
/// `FeO_T`, and Mercury's darkening agent is its carbon, which the law does not read (Shackelford et
/// al. 2025, PSJ 6, 254, a deferred finding), so that its graphite-bearing row, above the Moon's
/// 0.11 about a Sun, takes the cap and only the cap.
pub(crate) const CAPPED_ROWS: [&str; 1] = ["basalt_reduced"];

/// The surface age under which an airless silicate lid has no regolith yet, and keeps T51.b's cap
/// under the weathered area mix ([`Regolith::weathered_lid`], [`StandIn::UnweatheredLava`];
/// decision-p14-t54b-e-findings §A.5): 1 Myr.
///
/// - **What it stands for.** At micrometeoroid erosion of exposed lunar rock "on the order of
///   1 mm/10⁶ years for typical kilogram-sized lunar rocks" (Hörz et al. 1991, Lunar Sourcebook
///   ch. 4, p. 87, after Ashworth 1977; the same page gardens the regolith "to a depth of almost
///   1 cm" in 10⁶ years, and Fig. 4.20's caption puts contemporary regolith growth under 1 mm per
///   Myr; read by the batch's science check), 1 Myr comminutes about 1 mm, some 14 of the rows'
///   70 µm grains, an optically thick powder. Every rock row is such a powder (`substance::optics`'s "a fresh particulate
///   regolith of 70 µm grains"), never a flow. The top 2 cm of an existing regolith churn in about
///   81 kyr (Speyerer et al. 2016, Nature 538, 215; its abstract as the ruling read it), a lower
///   bound for reworking, not for forming a regolith on bedrock.
/// - **What it marks.** Under a silicate crust's resurfacing time τ(F) = 64.2 Myr × (F₀ ÷ F)^2.908
///   (`surface_age::resurfacing_time`, F₀ the generator's Earth's 92.55 mW m⁻²), a surface under
///   1 Myr means a heat flow above about 0.39 W m⁻², 4.2 times Earth's: global volcanic
///   resurfacing, Io's class. A surface just after its magma ocean's end, or a giant impact's,
///   falls under it too, and is quenched lava as well.
/// - **What it is not.** A step, not a fresh-to-gardened transition, and the same for every body,
///   not a clock scaled by the body's own micrometeoroid term: both are deferred, with the lava's
///   own slab or glass row, to the composition audit (decision-p14-t54b-e-findings §A.6).
///
/// It is part of the weathered area mix's behaviour (P14.T54.b.e).
pub(crate) const REGOLITH_FORMATION_TIME: Years = Years::new(1.0e6);

/// What a pass's weathered regolith reads beside its lid's row (P14.T54.b.e;
/// [`Regolith::weathered_lid`]): each the pass's own, the record's to the bit (rule (f)).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LidWeathering {
    /// P14.T51.d's maturity m, dimensionless in [0, 1].
    pub(crate) maturity: f64,
    /// The pass's mean surface temperature, at which the row's grain density is read.
    pub(crate) temperature: Kelvin,
    /// The surface age the maturity reads (`ModifierParts::surface_age`), yr: under
    /// [`REGOLITH_FORMATION_TIME`] an airless silicate lid has no regolith yet.
    pub(crate) surface_age: Years,
    /// The state of the pass's air, which says whether its lid is airless ([`capped_lid`]).
    pub(crate) state: SurfaceState,
}

/// Whether a lid younger than [`REGOLITH_FORMATION_TIME`] keeps the cap
/// ([`StandIn::UnweatheredLava`]): always, but in a test that reads P14.T54.b.e's area mix as
/// built before decision-p14-t54b-e-findings' must-fix ([`with_young_lava_weathered`]).
#[must_use]
fn young_lava_capped() -> bool {
    #[cfg(test)]
    if YOUNG_LAVA_WEATHERED.with(core::cell::Cell::get) {
        return false;
    }
    true
}

#[cfg(test)]
thread_local! {
    /// Whether a test reads young lava weathered as any other lid, on its thread.
    static YOUNG_LAVA_WEATHERED: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
}

/// `run`'s result with a lid younger than [`REGOLITH_FORMATION_TIME`] weathered as any other on
/// this thread: P14.T54.b.e's area mix as built (1ebcd3b6), which the must-fix's census sets
/// beside the fix (decision-p14-t54b-e-findings §A.5, item 1); set back after, a panic in `run`
/// included. A record derives lazily, so a test reads it inside the switch. A test's alone: it
/// goes with the census.
#[cfg(test)]
pub(crate) fn with_young_lava_weathered<R>(run: impl FnOnce() -> R) -> R {
    /// Sets the thread's switch back as it drops.
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            YOUNG_LAVA_WEATHERED.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(YOUNG_LAVA_WEATHERED.with(|cell| cell.replace(true)));
    run()
}

/// The wavelength, nm, at which the weathering law reads optics bin `bin`
/// ([`Regolith::weathered_lid`]): the midpoint of its edges ([`SPECTRAL_BIN_EDGES_NM`]), 345 nm to
/// 4,250 nm. The law's τ falls with λ, and one wavelength stands for each bin (labelled): the
/// first bin's 345 nm for all the light below 390 nm and the last's 4,250 nm for all the light
/// above 3,500 nm, as their albedos do (`substance::optics`'s module documentation), and the law's
/// own table of iron's z ends at 5,000 nm.
///
/// # Panics
///
/// If `bin` is not one of the [`OPTICS_BANDS`] bins.
#[must_use]
pub(crate) fn bin_wavelength_nm(bin: usize) -> f64 {
    f64::midpoint(SPECTRAL_BIN_EDGES_NM[bin], SPECTRAL_BIN_EDGES_NM[bin + 1])
}

/// The surface's Bond albedo, beneath any cloud: Σ over the published condensates `condensates`
/// of each entry's area × its Bond albedo (a solid's [`solid_bond`], a liquid's
/// [`liquid_entry_bond`] at its condensate's temperature in `partition`; a supercritical entry has
/// no interface and takes no area), then over the weathering deposits `deposits` (each a row and
/// its area, P14.T51.j's `ferric` from the weathered area mix, P14.T54.b.e) each at its row's
/// [`solid_bond`], plus the regolith's albedo `regolith` on the rest, max(0, 1 − Σ areas), under
/// the light of `shade`. Where the areas sum above 1 (ices that overlap, or an ice over a sea, as
/// R09's cover convention allows) they are scaled to 1, as T51.a's cover is. With no deposits it
/// is the condensates' and the regolith's alone, to the bit.
///
/// An Io's volcanic deposits (SO₂ frost and sulphur) are not among them, and the regolith takes
/// their area (recorded): both rows have condensed optics (P14.T49.i: `SO2`'s a stand-in on Nash,
/// Fanale and Nelson 1980's levels, `S8`'s measured), but the deposits are Io's measured shares
/// standing in, on every body of extreme volcanism, for the deposits P14.T24.g's outgassing will
/// give, and the plan's wiring (P14.T54.b, from P14.T51.j) puts `ferric`'s area alone in the mix.
#[must_use]
pub(crate) fn surface_bond(
    condensates: &Condensates,
    partition: &Partition,
    (regolith, deposits): (f64, &[(SubstanceId, f64)]),
    shade: &HostShade,
) -> f64 {
    surface_bond_with(
        condensates,
        partition,
        (regolith, deposits),
        shade,
        &|row| solid_bond(row, &shade.bands).bond,
    )
}

/// [`surface_bond`] with each solid entry's Bond albedo `solid` of its row: [`solid_bond`]'s under
/// the light of `shade` there, a sediment's cover over it under [`sedimented_surface_bond`].
#[must_use]
fn surface_bond_with(
    condensates: &Condensates,
    partition: &Partition,
    (regolith, deposits): (f64, &[(SubstanceId, f64)]),
    shade: &HostShade,
    solid: &dyn Fn(SubstanceId) -> f64,
) -> f64 {
    let entries = condensates.entries().iter().fold(0.0, |sum, entry| {
        sum + match entry.phase() {
            CondensatePhase::Solid | CondensatePhase::Liquid => entry.area_fraction(),
            CondensatePhase::Supercritical => 0.0,
        }
    });
    let areas = deposits.iter().fold(entries, |sum, &(_, area)| sum + area);
    let scale = if areas > 1.0 { 1.0 / areas } else { 1.0 };
    let covered = areas * scale;
    let rest = if covered < 1.0 { 1.0 - covered } else { 0.0 };
    let ground = condensates
        .entries()
        .iter()
        .fold(rest * regolith, |sum, entry| match entry.phase() {
            CondensatePhase::Solid => {
                sum + entry.area_fraction() * scale * solid(entry.substance())
            }
            CondensatePhase::Liquid => {
                let bond = liquid_entry_bond(entry, partition, &shade.bins).bond;
                sum + entry.area_fraction() * scale * bond
            }
            CondensatePhase::Supercritical => sum,
        });
    deposits.iter().fold(ground, |sum, &(row, area)| {
        sum + area * scale * solid_bond(row, &shade.bands).bond
    })
}

/// The generator version from which the organic sediment a hazy air's fallout lays covers its
/// solid ground (P14.T24.h.b's second part; decision-p14-t51-batch §9.4): 26, the train after 25's.
///
/// It is this rule's one hold. Below it [`OrganicSedimentRules::in_force`] is
/// [`OrganicSedimentRules::Bare`]: the ground is its rows' and its regolith's, as to 25, bit for
/// bit. 26's bump deletes this hold, [`OrganicSedimentRules`] with its `Bare` arm, the guard in
/// [`sediment_share`], the tests' switch and its tripwire
/// (`the_organic_sediment_is_held_only_until_the_27_bump`).
pub(crate) const ORGANIC_SEDIMENT_VERSION: GeneratorVersion = GeneratorVersion::new(27);

/// Whether a hazy air's fallout covers its solid ground (its hold, [`ORGANIC_SEDIMENT_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum OrganicSedimentRules {
    /// Version 25's and before: no sediment.
    Bare,
    /// Version 26's: the sediment covers its share of the solid ground ([`sediment_share`]).
    Sediment,
}

impl OrganicSedimentRules {
    /// The rules this generator version gives: the sediment from [`ORGANIC_SEDIMENT_VERSION`], none
    /// below it; in the tests, the rules a test's switch sets on its thread
    /// ([`with_organic_sediment`]).
    #[must_use]
    pub(crate) fn in_force() -> Self {
        #[cfg(test)]
        if let Some(rules) = ORGANIC_SEDIMENT_RULES.with(core::cell::Cell::get) {
            return rules;
        }
        if GENERATOR_VERSION >= ORGANIC_SEDIMENT_VERSION {
            Self::Sediment
        } else {
            Self::Bare
        }
    }
}

#[cfg(test)]
thread_local! {
    /// The sediment's rules a test sets on its thread, `None` for the version's.
    static ORGANIC_SEDIMENT_RULES: core::cell::Cell<Option<OrganicSedimentRules>> =
        const { core::cell::Cell::new(None) };
}

/// `run`'s result with the sediment's rules `rules` on this thread, for the tests to reach version
/// 26's ahead of the bump; the version's rules are set back after, a panic in `run` included. A
/// record derives lazily, so a test reads it inside the switch. 26's bump deletes it with the hold.
#[cfg(test)]
pub(crate) fn with_organic_sediment<R>(rules: OrganicSedimentRules, run: impl FnOnce() -> R) -> R {
    /// Sets the thread's rules back as it drops.
    struct Restore(Option<OrganicSedimentRules>);
    impl Drop for Restore {
        fn drop(&mut self) {
            ORGANIC_SEDIMENT_RULES.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(ORGANIC_SEDIMENT_RULES.with(|cell| cell.replace(Some(rules))));
    run()
}

/// Titan's surface pressure, Pa: 146,700, Huygens's 1,467 ± 1 hPa at the ground (Fulchignoni et al.
/// 2005, Nature 438, 785, from memory), at which T24.c's haze is calibrated ([`TITAN_GRAVITY`]).
pub(crate) const TITAN_SURFACE_PRESSURE: Pascals = Pascals::new(1.467e5);

/// Titan's surface gravity, m s⁻²: 1.3543, GM ÷ R² of its 8,978.137 km³ s⁻² and 2,574.76 km (JPL's
/// SAT441 and its physical parameters, as the moons' tests' `TITAN` fixture reads them).
pub(crate) const TITAN_GRAVITY: MetresPerSecondSquared = MetresPerSecondSquared::new(1.3543);

/// The methane haze's τ(550) on the snapshot path of an air of partial pressures `partials` under
/// the gravity `gravity` (P14.T24.h.b).
///
/// It is T24.c's at the air's index
/// ([`haze_optical_depth`](crate::planetary::hooks::aerosols::haze_optical_depth) of its CH₄ and
/// CO₂ mole fractions), which is Titan's column's, times the air's column `p_s` ÷ g over Titan's, at
/// most 1 (the lane's lean, 2026-10-31): a thin air holds a thin haze. It is a lower bound there:
/// Triton's and Pluto's 1 Pa airs take some 5 × 10⁻⁵ and 8 × 10⁻⁵, where their measured hazes are
/// of τ 10⁻³–10⁻² (from memory), since a haze's production is held by the ultraviolet's photons,
/// not the column (a deferred finding). None with no luminous host, whose ultraviolet makes it
/// (`hottest_host` 0), and none for an air of no pressure or a ground of no gravity.
#[must_use]
pub(crate) fn snapshot_haze_optical_depth(
    partials: &PartialPressures,
    gravity: MetresPerSecondSquared,
    hottest_host: Kelvin,
) -> f64 {
    use crate::planetary::hooks::aerosols::{haze_index, haze_optical_depth};
    let pressure = partials.total().value();
    let g = gravity.value();
    if !(pressure > 0.0 && pressure.is_finite() && g > 0.0 && hottest_host.value() > 0.0) {
        return 0.0;
    }
    let methane = partials.of(Gas::Methane).value() / pressure;
    let carbon_dioxide = partials.of(Gas::CarbonDioxide).value() / pressure;
    let titan = TITAN_SURFACE_PRESSURE.value() / TITAN_GRAVITY.value();
    let column = pressure / g / titan;
    let column = if column < 1.0 { column } else { 1.0 };
    haze_optical_depth(haze_index(methane, carbon_dioxide)) * column
}

/// The share of a body's solid ground the organic sediment covers under an air whose snapshot haze
/// is `haze` ([`snapshot_haze_optical_depth`]; P14.T24.h.b's second part).
///
/// It is the haze's τ over Titan's
/// ([`TITAN_HAZE_OPTICAL_DEPTH`](crate::planetary::hooks::aerosols::TITAN_HAZE_OPTICAL_DEPTH)), 0
/// to 1, under [`OrganicSedimentRules::Sediment`], and none below it.
///
/// - **What it stands for.** The haze's fallout settles on the ground: Titan's is dark organic
///   sediment, its plains and its dunes, not frost (Tomasko et al. 2005, Nature 438, 765, from
///   memory), over its whole solid surface at Titan's haze. A thinner haze's fallout covers its
///   share in proportion to its column (the lane's lean, 2026-10-31), continuous from none at the
///   haze's index of 1, so that the passes see no step.
/// - **What it covers.** The regolith and the `H2O` ice the condensates list, T49.m's icy row: the
///   fallout settles on the solid ground that is not the air's own seasonal frost. A volatile's
///   frost (N₂, CH₄, CO₂) condenses over it, and a liquid takes it in.
#[must_use]
pub(crate) fn sediment_share(haze: f64) -> f64 {
    use crate::planetary::hooks::aerosols::TITAN_HAZE_OPTICAL_DEPTH;
    if OrganicSedimentRules::in_force() == OrganicSedimentRules::Bare
        || haze.is_nan()
        || haze <= 0.0
    {
        return 0.0;
    }
    let share = haze / TITAN_HAZE_OPTICAL_DEPTH;
    if share < 1.0 { share } else { 1.0 }
}

/// The organic sediment's row: `tholin`, Khare et al. 1984's Titan tholin (the haze's own row,
/// P14.T24.c's), whose condensed optics stand for Titan's sediment, labelled: its sands and plains
/// are organic grains of its haze's fallout, whose own index is not measured (from memory).
const SEDIMENT_ROW: &str = "tholin";

/// The surface's Bond albedo beneath any cloud ([`surface_bond`]) under an air whose snapshot haze
/// is `haze`, its organic sediment over its share of its solid ground (P14.T24.h.b's second part).
///
/// The share s is [`sediment_share`]'s of the haze ([`snapshot_haze_optical_depth`]): the
/// regolith's albedo `regolith` and each `H2O` solid entry's are (1 − s) A + s `A_sed`, `A_sed`
/// the sediment row's ([`SEDIMENT_ROW`]) under the light of `shade`, and every other entry and
/// deposit as it is. At no sediment it is [`surface_bond`] to the bit.
///
/// # Panics
///
/// Never: the registry holds `tholin`.
#[must_use]
pub(crate) fn sedimented_surface_bond(
    condensates: &Condensates,
    partition: &Partition,
    (regolith, deposits): (f64, &[(SubstanceId, f64)]),
    shade: &HostShade,
    haze: f64,
) -> f64 {
    let sediment = sediment_share(haze);
    if sediment <= 0.0 {
        return surface_bond(condensates, partition, (regolith, deposits), shade);
    }
    let row = by_key(SEDIMENT_ROW).expect("the registry holds tholin");
    let covered = solid_bond(row, &shade.bands).bond;
    let under = |albedo: f64| (1.0 - sediment) * albedo + sediment * covered;
    let water = water();
    surface_bond_with(
        condensates,
        partition,
        (under(regolith), deposits),
        shade,
        &|id| {
            let own = solid_bond(id, &shade.bands).bond;
            if id == water { under(own) } else { own }
        },
    )
}

/// The share of the disc under cloud of a body in the state `state` before its condensable's
/// availability (the lane's stand-in, P14.T51.b, recorded): a runaway greenhouse 1, Venus's deck
/// (T13.c's table), and a steam envelope the same, stated (P14.T53.b part 3); a temperate world and
/// a snowball Earth's 0.67 (King et al. 2013, IEEE TGRS 51, 3826, as T13.c's table cites it); a gas
/// envelope 1, whose albedo is its class's, clouds included ([`giant_bond`]); airless and molten
/// surfaces none.
#[must_use]
pub(crate) fn cloud_share_of_state(state: SurfaceState) -> f64 {
    match state {
        SurfaceState::RunawayGreenhouse
        | SurfaceState::GasEnvelope
        | SurfaceState::SteamEnvelope => SurfaceState::RunawayGreenhouse.cloud_fraction(),
        SurfaceState::Temperate | SurfaceState::Snowball => {
            SurfaceState::Temperate.cloud_fraction()
        }
        SurfaceState::Airless | SurfaceState::MagmaOcean | SurfaceState::Tenuous => 0.0,
    }
}

/// Earth's mean surface gravity, at which the reference cloud column is weighed: 9.820 m s⁻²
/// (NASA's Earth fact sheet, as the partition's tests take it).
const EARTH_SURFACE_GRAVITY: MetresPerSecondSquared = MetresPerSecondSquared::new(9.82);

/// The temperature at which the reference cloud column holds water's vapour, K: 273.15, water's
/// freezing point at one atmosphere.
const REFERENCE_CLOUD_TEMPERATURE_K: f64 = 273.15;

/// The airborne column of a condensable that makes a full cloud cover (the lane's stand-in):
/// water's at P14.T24.c's surface humidity and its saturation at 273.15 K over Earth's gravity,
/// h `p_sat` ÷ g = 0.77 × 611 Pa ÷ 9.82 m s⁻² ≈ 47.9 kg m⁻², in the model's own measure (the
/// registry's T49.b curve), so that the generated Earth, whose column is some 130 kg m⁻², keeps a
/// full cover with margin and a Mars-like dry world falls near none. Computed once a call chain,
/// into its [`ShadeBook`] (lever (d)), or once a body where none is held, and carried into each
/// body's [`HostShade`].
///
/// # Panics
///
/// Never: water's row has a phase diagram.
#[must_use]
pub(crate) fn reference_cloud_column() -> KilogramsPerSquareMetre {
    let saturation =
        diagram::saturation_pressure(water(), Kelvin::new(REFERENCE_CLOUD_TEMPERATURE_K))
            .expect("water's row has a phase diagram");
    KilogramsPerSquareMetre::new(
        WATER_SURFACE_HUMIDITY * saturation.value() / EARTH_SURFACE_GRAVITY.value(),
    )
}

/// The availability, 0 to 1, of the condensable that makes a body's cloud, from its partition
/// `partition` under the light `shade` (the lane's stand-in, P14.T51.b, recorded): min(1, (1 − d) W
/// ÷ `W_ref`), with W the airborne column `x_s` p ÷ g of P14.T48.d's condensable of its air over its
/// climate and reservoirs ([`climate_condensable`]), d its dominance in the partition (T51.a's
/// continuous weight,
/// [`Partitioned::dominance`](crate::planetary::chem::partition::Partitioned::dominance)), and
/// `W_ref` the [`reference_cloud_column`], `shade`'s. A dominant condensable cold-traps at the surface
/// (Wordsworth 2015, as P14.T48.d cites it) and forms no Earth-like cover, so a Mars with carbon
/// dioxide caps falls near none, while Earth's minor water keeps its whole column (the lane's
/// rule, 2026-10-16). `None` where the air has no such condensable, or no pressure, whose
/// availability is none; the condensable with its availability otherwise.
#[must_use]
fn cloud_condensable(partition: &Partition, shade: &HostShade) -> Option<(SubstanceId, f64)> {
    let pressure = partition.surface_pressure();
    let p = pressure.value();
    let g = partition.ground().gravity().value();
    if !(p > 0.0 && p.is_finite() && g > 0.0 && g.is_finite()) {
        return None;
    }
    let gases: Vec<(SubstanceId, f64)> = partition
        .species()
        .iter()
        .map(|s| (s.substance(), s.partial_pressure().value() / p))
        .collect();
    let climate = partition.climate();
    let range = (climate.coldest(), climate.warmest());
    let condensable = climate_condensable(&gases, pressure, range, &partition.reservoirs())?;
    let (partial, dominance) = partition.of(condensable).map_or((0.0, 0.0), |s| {
        (s.partial_pressure().value(), s.dominance())
    });
    let available = (1.0 - dominance) * partial / g / shade.cloud_column.value();
    let available = if available.is_nan() {
        0.0
    } else {
        available.clamp(0.0, 1.0)
    };
    Some((condensable, available))
}

/// The generator version from which a cloud whose condensable is not water covers its own measured
/// share (P14.T24.h.b's first part; decision-p14-t51-batch §9.4): 26, the train after 25's.
///
/// It is this rule's one hold. Below it [`MinorCloudsRules::in_force`] is
/// [`MinorCloudsRules::WaterShare`]: every temperate or snowball world's cloud takes water's 0.67
/// before its availability, as to 25, bit for bit. 26's bump deletes this hold, [`MinorCloudsRules`]
/// with its `WaterShare` arm, the guard in [`condensable_cloud_share`], the tests' switch and its
/// tripwire (`the_minor_clouds_are_held_only_until_the_27_bump`).
pub(crate) const MINOR_CLOUDS_VERSION: GeneratorVersion = GeneratorVersion::new(27);

/// Whether a cloud whose condensable is not water covers its own share (its hold,
/// [`MINOR_CLOUDS_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum MinorCloudsRules {
    /// Version 25's and before: any condensable's cloud takes water's reference share.
    WaterShare,
    /// Version 26's: a condensable that is not water takes [`MINOR_CONDENSABLE_CLOUD_SHARE`].
    OwnShare,
}

impl MinorCloudsRules {
    /// The rules this generator version gives: a minor condensable's own share from
    /// [`MINOR_CLOUDS_VERSION`], water's below it; in the tests, the rules a test's switch sets on
    /// its thread ([`with_minor_clouds`]).
    #[must_use]
    pub(crate) fn in_force() -> Self {
        #[cfg(test)]
        if let Some(rules) = MINOR_CLOUDS_RULES.with(core::cell::Cell::get) {
            return rules;
        }
        if GENERATOR_VERSION >= MINOR_CLOUDS_VERSION {
            Self::OwnShare
        } else {
            Self::WaterShare
        }
    }
}

#[cfg(test)]
thread_local! {
    /// The minor clouds' rules a test sets on its thread, `None` for the version's.
    static MINOR_CLOUDS_RULES: core::cell::Cell<Option<MinorCloudsRules>> =
        const { core::cell::Cell::new(None) };
}

/// `run`'s result with the minor clouds' rules `rules` on this thread, for the tests to reach
/// version 26's ahead of the bump; the version's rules are set back after, a panic in `run`
/// included. A record derives lazily, so a test reads it inside the switch. 26's bump deletes it
/// with the hold.
#[cfg(test)]
pub(crate) fn with_minor_clouds<R>(rules: MinorCloudsRules, run: impl FnOnce() -> R) -> R {
    /// Sets the thread's rules back as it drops.
    struct Restore(Option<MinorCloudsRules>);
    impl Drop for Restore {
        fn drop(&mut self) {
            MINOR_CLOUDS_RULES.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(MINOR_CLOUDS_RULES.with(|cell| cell.replace(Some(rules))));
    run()
}

/// The share of the disc a cloud whose condensable is not water covers before its availability
/// (P14.T24.h.b): 0.004.
///
/// It is the middle of Titan's tropospheric methane clouds' 0.2–0.6% of its disc (Bouchez and
/// Brown 2005, ApJ 618, L53, from memory, as plan 14's task text cites it), its outbursts reaching
/// about 10% or less (Schaller et al. 2006, Icarus 182, 224, from memory), not held here. Titan's
/// is the one measured cloud of a minor condensable that is not water: it stands for every one,
/// labelled. Its cloud keeps the water cloud's albedo ([`cloud_albedo`]), which at this share moves
/// a Bond albedo by under 0.002.
pub(crate) const MINOR_CONDENSABLE_CLOUD_SHARE: f64 = 0.004;

/// The share of the disc a temperate or snowball world's cloud of the condensable `condensable`
/// covers before its availability (P14.T24.h.b's first part).
///
/// A condensable that is not water takes [`MINOR_CONDENSABLE_CLOUD_SHARE`] under
/// [`MinorCloudsRules::OwnShare`]; otherwise `state`'s ([`cloud_share_of_state`]), water's 0.67, as
/// to 25.
#[must_use]
fn condensable_cloud_share(state: SurfaceState, condensable: SubstanceId) -> f64 {
    if condensable != water() && MinorCloudsRules::in_force() == MinorCloudsRules::OwnShare {
        MINOR_CONDENSABLE_CLOUD_SHARE
    } else {
        cloud_share_of_state(state)
    }
}

/// The cloud fraction of a body in the state `state` whose runaway's deck is `deck`, whose
/// partition is `partition`, under the light `shade`: its state's share ([`cloud_share_of_state`])
/// times its condensable's availability ([`cloud_condensable`]), the whole for a runaway
/// greenhouse under its deck, which keeps it with or without sulphur, and a gas envelope (the plan:
/// "never from the Surface path's decks"). A runaway solved without its deck
/// ([`RunawayDeck::Deckless`], P14.T53.b part 1 (b); decision-absurd-states-24 R3.2) takes a
/// temperate world's share times its availability, the water cloud's, none on a dry world. A
/// steam envelope takes a runaway's, by its deck (P14.T53.b part 3).
#[must_use]
pub(crate) fn cloud_fraction(
    (state, deck): (SurfaceState, RunawayDeck),
    partition: &Partition,
    shade: &HostShade,
) -> f64 {
    match (state, deck) {
        (SurfaceState::RunawayGreenhouse | SurfaceState::SteamEnvelope, RunawayDeck::Kept)
        | (SurfaceState::GasEnvelope, _) => cloud_share_of_state(state),
        (SurfaceState::RunawayGreenhouse | SurfaceState::SteamEnvelope, RunawayDeck::Deckless) => {
            minor_cloud_fraction(SurfaceState::Temperate, partition, shade)
        }
        (SurfaceState::Temperate | SurfaceState::Snowball, _) => {
            minor_cloud_fraction(state, partition, shade)
        }
        (SurfaceState::Airless | SurfaceState::MagmaOcean | SurfaceState::Tenuous, _) => 0.0,
    }
}

/// The cloud fraction of a temperate or snowball world's cloud, its state `state`, of partition
/// `partition` under the light `shade`: its condensable's share ([`condensable_cloud_share`]) times
/// its availability ([`cloud_condensable`]), none where it has no condensable.
#[must_use]
fn minor_cloud_fraction(state: SurfaceState, partition: &Partition, shade: &HostShade) -> f64 {
    cloud_condensable(partition, shade).map_or(0.0, |(id, available)| {
        condensable_cloud_share(state, id) * available
    })
}

/// The Bond albedo of the cloud of a body in the state `state` whose runaway's deck is `deck`,
/// under the light of `shade`: a runaway greenhouse's under its deck Venus's 0.76 (Haus et al.
/// 2016, Icarus 272, 178, T13.c's table), so that Venus keeps it exactly at its full cover; a
/// temperate world's, a snowball's and a runaway's solved without its deck (P14.T53.b part 1 (b))
/// the water cloud calibrated on Earth ([`water_cloud_albedo`]), a steam envelope's a runaway's by
/// its deck (P14.T53.b part 3); none for the rest.
#[must_use]
pub(crate) fn cloud_albedo((state, deck): (SurfaceState, RunawayDeck), shade: &HostShade) -> f64 {
    match (state, deck) {
        (SurfaceState::RunawayGreenhouse | SurfaceState::SteamEnvelope, RunawayDeck::Kept) => {
            SurfaceState::RunawayGreenhouse
                .albedo(SurfaceMaterial::Rock)
                .value()
        }
        (SurfaceState::RunawayGreenhouse | SurfaceState::SteamEnvelope, RunawayDeck::Deckless)
        | (SurfaceState::Temperate | SurfaceState::Snowball, _) => shade.water_cloud,
        (
            SurfaceState::GasEnvelope
            | SurfaceState::Airless
            | SurfaceState::MagmaOcean
            | SurfaceState::Tenuous,
            _,
        ) => 0.0,
    }
}

/// The reference Earth's ocean share, 0.708 of its surface, at water's liquid albedo: a rounded
/// figure (science-p14-t24a-climate.md §1), where Eakins and Sharman 2010 give 0.7095 (361.9 × 10⁶
/// of 510.1 × 10⁶ km²) and IPCC AR5 WG1 Table 4.1's note a 0.711 (an ocean of 362.5 × 10⁶ km² and a
/// land of 147.6 × 10⁶ km²; Vaughan et al. 2013, read).
pub(crate) const REFERENCE_EARTH_OCEAN: f64 = 0.708;

/// The reference Earth's perennial ice share, 0.0289 of its surface: its grounded land ice, 10.0%
/// of the land (IPCC AR5 WG1, Vaughan et al. 2013, Table 4.1, read: the Antarctic ice sheet 8.3%,
/// Greenland's 1.2% and the glaciers 0.5% of a land of 147.6 × 10⁶ km², over a surface of 510.1 ×
/// 10⁶ km² with note a's ocean; decision-p14-t51a-part2 §3.1 and §8), at `H2O`'s frost. Its sea ice
/// lies over the ocean share and is left in it.
pub(crate) const REFERENCE_EARTH_ICE: f64 = 0.0289;

/// The reference Earth's mean surface temperature, K, at which its ocean's record is read: 288
/// (the generated Earth's, science-p14-t24a-climate.md §3).
const REFERENCE_EARTH_TEMPERATURE_K: f64 = 288.0;

/// The reference Earth's clear-sky Bond albedo `A_clear` about a Sun, computed from the registry
/// when it is called: its ocean ([`REFERENCE_EARTH_OCEAN`]) at water's liquid albedo at 288 K, its
/// perennial ice ([`REFERENCE_EARTH_ICE`]) at `H2O`'s frost, and the rest, its bare land, at T49.c's
/// `basalt` regolith (under air, as it is), all under a 5,772 K Planck curve.
///
/// Its ocean is the saturated liquid's under either rule of P14.T49.n's compression: a
/// calibration's reference, not a body. Compressed to one atmosphere it would be about
/// 4.6 × 10⁻⁵ denser (`κ_T` Δp), its index some 1.5 × 10⁻⁵ higher and `A_clear` a few parts in a
/// million brighter, below the calibration's figures' precision; a recorded refinement.
///
/// # Panics
///
/// Never: water's row has a liquid record at 288 K (P14.T49.h).
#[must_use]
pub(crate) fn reference_earth_clear_sky() -> f64 {
    let sun = sun_light();
    let ocean = liquid_record(
        water(),
        Kelvin::new(REFERENCE_EARTH_TEMPERATURE_K),
        Pascals::ZERO,
    )
    .in_range()
    .expect("water's row has a liquid record at 288 K (P14.T49.h)");
    let land = Regolith::of_material(SurfaceMaterial::Rock).bond(SurfaceState::Temperate, &sun.0);
    let rest = 1.0 - REFERENCE_EARTH_OCEAN - REFERENCE_EARTH_ICE;
    REFERENCE_EARTH_OCEAN * liquid_bond(&ocean, &sun.1)
        + REFERENCE_EARTH_ICE * solid_bond(water(), &sun.0).bond
        + rest * land.bond
}

/// A 5,772 K Planck curve's light in the optics' bins and the liquids' weights.
#[must_use]
fn sun_light() -> ([f64; OPTICS_BANDS], [f64; LIQUID_WEIGHTS]) {
    let bands = optics_shares(Kelvin::new(SOLAR_EFFECTIVE_TEMPERATURE_K));
    let bins = liquid_shares(Kelvin::new(SOLAR_EFFECTIVE_TEMPERATURE_K), &bands);
    (bands, bins)
}

/// A Sun's light in the optics' bins, at which the photometry's references are taken
/// ([`HostShade::sun`]).
#[must_use]
pub(crate) fn sun_bands() -> [f64; OPTICS_BANDS] {
    sun_light().0
}

/// The water cloud's Bond albedo (the lane's stand-in, P14.T51.b, recorded): the one albedo that
/// gives Earth's area mix NASA's 0.294 at its 0.67 of cloud, both T13.c's table's, `A_cloud` =
/// (0.294 − (1 − 0.67) `A_clear`) ÷ 0.67, with `A_clear` the reference Earth's clear sky computed
/// from the registry ([`reference_earth_clear_sky`]), so that it follows the registry's rows.
///
/// Under the adding rule ([`RayleighAddingRules::Added`], P14.T24.h.b) Earth's cloud is re-split:
/// its 0.294 is the cloud over 0.67 above its clear sky under its own Rayleigh layer, bin by bin
/// ([`reference_earth_clear_sky_added`]), the clear sky's 0.33 taking its Rayleigh share. The 0.294
/// is NASA's, Earth's 2000s, whose anthropogenic aerosol and land-use brightening (IPCC AR6 WGI
/// ch. 7's effective radiative forcings, from memory) the fit takes as the cloud's, stated.
#[must_use]
pub(crate) fn water_cloud_albedo() -> f64 {
    let earth = SurfaceState::Temperate
        .albedo(SurfaceMaterial::Rock)
        .value();
    let cloud = SurfaceState::Temperate.cloud_fraction();
    let clear = match RayleighAddingRules::in_force() {
        RayleighAddingRules::Max => reference_earth_clear_sky(),
        RayleighAddingRules::Added => reference_earth_clear_sky_added(),
    };
    (earth - (1.0 - cloud) * clear) / cloud
}

/// The reference Earth's clear sky under its own Rayleigh layer about a Sun (P14.T24.h.b): in each
/// of the optics' bins its layer ([`rayleigh_bins`] of [`reference_earth_air`] at
/// [`EARTH_SURFACE_GRAVITY`]) over its clear sky ([`reference_earth_clear_sky`]) by the adding rule
/// ([`rayleigh_added`]), weighted by a Sun's light.
#[must_use]
pub(crate) fn reference_earth_clear_sky_added() -> f64 {
    let clear = reference_earth_clear_sky();
    let layers =
        rayleigh_bins(&reference_earth_air(), EARTH_SURFACE_GRAVITY).unwrap_or([0.0; OPTICS_BANDS]);
    band_weighted(
        &layers.map(|layer| rayleigh_added(layer, clear)),
        &sun_bands(),
    )
}

/// The generator version from which a body's air's Rayleigh layer is laid over its clear sky by the
/// adding rule, not taken as its floor (P14.T24.h.b's fourth part; decision-p14-t51-batch §9.4;
/// decision-p14-t54a-passes §G3.5): 26, the train after 25's.
///
/// It is this rule's one hold. Below it [`RayleighAddingRules::in_force`] is
/// [`RayleighAddingRules::Max`]: the snapshot path's Bond albedo is the larger of the area mix and
/// the floor, and the water cloud's calibration Earth's 0.294 without its air, as to 25, bit for
/// bit. 26's bump deletes this hold, [`RayleighAddingRules`] with its `Max` arm, the guards in
/// [`water_cloud_albedo`] and `surface_mix::floored`, the tests' switch and its tripwire
/// (`the_rayleigh_adding_is_held_only_until_the_27_bump`).
pub(crate) const RAYLEIGH_ADDING_VERSION: GeneratorVersion = GeneratorVersion::new(27);

/// How the air's Rayleigh layer joins the area mix (its hold, [`RAYLEIGH_ADDING_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum RayleighAddingRules {
    /// Version 25's and before: the larger of the two, P14.T24.h.a's floor.
    Max,
    /// Version 26's: the layer over the clear sky by the adding rule ([`rayleigh_added`]), the cloud
    /// above it, Earth's cloud re-split ([`water_cloud_albedo`]).
    Added,
}

impl RayleighAddingRules {
    /// The rules this generator version gives: the adding rule from [`RAYLEIGH_ADDING_VERSION`],
    /// the max below it; in the tests, the rules a test's switch sets on its thread
    /// ([`with_rayleigh_adding`]).
    #[must_use]
    pub(crate) fn in_force() -> Self {
        #[cfg(test)]
        if let Some(rules) = RAYLEIGH_ADDING_RULES.with(core::cell::Cell::get) {
            return rules;
        }
        if GENERATOR_VERSION >= RAYLEIGH_ADDING_VERSION {
            Self::Added
        } else {
            Self::Max
        }
    }
}

#[cfg(test)]
thread_local! {
    /// The adding rule's rules a test sets on its thread, `None` for the version's.
    static RAYLEIGH_ADDING_RULES: core::cell::Cell<Option<RayleighAddingRules>> =
        const { core::cell::Cell::new(None) };
}

/// `run`'s result with the adding rule's rules `rules` on this thread, for the tests to reach
/// version 26's ahead of the bump; the version's rules are set back after, a panic in `run`
/// included. A record derives lazily, and a shade takes its water cloud when it is built, so a
/// test reads both inside the switch. 26's bump deletes it with the hold.
#[cfg(test)]
pub(crate) fn with_rayleigh_adding<R>(rules: RayleighAddingRules, run: impl FnOnce() -> R) -> R {
    /// Sets the thread's rules back as it drops.
    struct Restore(Option<RayleighAddingRules>);
    impl Drop for Restore {
        fn drop(&mut self) {
            RAYLEIGH_ADDING_RULES.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(RAYLEIGH_ADDING_RULES.with(|cell| cell.replace(Some(rules))));
    run()
}

/// The generator version from which a hazy air's haze is an absorbing layer over the rest of its
/// disc, and its absorbed sunlight its anti-greenhouse (P14.T24.h.b's third part;
/// decision-p14-t51-batch §9.4): 26, the train after 25's.
///
/// It is this rule's one hold. Below it [`HazeLayerRules::in_force`] is [`HazeLayerRules::Clear`]:
/// no haze in the Bond albedo and no anti-greenhouse, as to 25, bit for bit. 26's bump deletes this
/// hold, [`HazeLayerRules`] with its `Clear` arm, the guards in [`hazed_bins`],
/// [`anti_greenhouse_share`] and [`step_anti_greenhouse`] and the one in `escaping`'s solve that
/// builds the step's closure, the tests' switch, its line in `with_scattering_held` and its tripwire
/// (`the_haze_layer_is_held_only_until_the_27_bump`).
pub(crate) const HAZE_LAYER_VERSION: GeneratorVersion = GeneratorVersion::new(27);

/// Whether a hazy air's haze absorbs (its hold, [`HAZE_LAYER_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum HazeLayerRules {
    /// Version 25's and before: no haze layer.
    Clear,
    /// Version 26's: the haze a layer over the rest ([`hazed_bins`]), its absorption the
    /// anti-greenhouse ([`anti_greenhouse_share`]).
    Absorbing,
}

impl HazeLayerRules {
    /// The rules this generator version gives: the absorbing layer from [`HAZE_LAYER_VERSION`],
    /// none below it; in the tests, the rules a test's switch sets on its thread
    /// ([`with_haze_layer`]).
    #[must_use]
    pub(crate) fn in_force() -> Self {
        #[cfg(test)]
        if let Some(rules) = HAZE_LAYER_RULES.with(core::cell::Cell::get) {
            return rules;
        }
        if GENERATOR_VERSION >= HAZE_LAYER_VERSION {
            Self::Absorbing
        } else {
            Self::Clear
        }
    }
}

#[cfg(test)]
thread_local! {
    /// The haze layer's rules a test sets on its thread, `None` for the version's.
    static HAZE_LAYER_RULES: core::cell::Cell<Option<HazeLayerRules>> =
        const { core::cell::Cell::new(None) };
}

/// `run`'s result with the haze layer's rules `rules` on this thread, for the tests to reach version
/// 26's ahead of the bump; the version's rules are set back after, a panic in `run` included. A
/// record derives lazily, so a test reads it inside the switch. 26's bump deletes it with the hold.
#[cfg(test)]
pub(crate) fn with_haze_layer<R>(rules: HazeLayerRules, run: impl FnOnce() -> R) -> R {
    /// Sets the thread's rules back as it drops.
    struct Restore(Option<HazeLayerRules>);
    impl Drop for Restore {
        fn drop(&mut self) {
            HAZE_LAYER_RULES.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(HAZE_LAYER_RULES.with(|cell| cell.replace(Some(rules))));
    run()
}

/// The Titan haze aggregate's single-scattering albedo ω and asymmetry g at five wavelengths, nm.
///
/// They are optool 1.9.14's modified mean-field aggregate (Dominik, Min and Tazaki, ascl:2104.010) of
/// T24.c's monomers, 3,000 of 0.05 µm at a fractal dimension of 2 (Tomasko et al. 2008, as
/// [`TITAN_HAZE_MONOMER_RADIUS`](crate::planetary::hooks::aerosols::TITAN_HAZE_MONOMER_RADIUS)
/// takes them), at `tholin.json`'s index (Khare et al. 1984, Icarus 60, 127): ω its scattering
/// cross section over its extinction, as rendering plan R08's fixture
/// `view/atmosphere/fixtures/optool-titan.json` holds them (R08.T5.c), the client's own haze. Held
/// at the end nodes beyond them (labelled): below 380 nm the haze absorbs more, beyond 780 nm
/// Khare's tholin absorbs less, so the held values bound its absorption from below in the
/// ultraviolet and from above in the near infrared.
const TITAN_HAZE_AGGREGATE: [(f64, f64, f64); 5] = [
    (380.0, 16.3889 / 20.1344, 0.823_487),
    (440.0, 18.0518 / 20.1923, 0.818_964),
    (550.0, 15.8321 / 16.5393, 0.813_108),
    (680.0, 11.2738 / 11.4898, 0.807_764),
    (780.0, 8.41771 / 8.51578, 0.803_534),
];

/// The haze's τ above and below 30 km at `wavelength_nm`, each over its whole τ(550).
///
/// They are Bazzon, Schmid and Buenzli's (2014, A&A 572, A6, App. A) fit to Tomasko et al. 2008's
/// Titan haze, τ80 + τ30 + τ0 = 1.012 × 10⁷ λ^−2.339 + 2.029 × 10⁴ λ^−1.409 + 627.0 λ^−0.9706, λ in
/// nm (the fit T24.c's
/// [`TITAN_HAZE_OPTICAL_DEPTH`](crate::planetary::hooks::aerosols::TITAN_HAZE_OPTICAL_DEPTH) takes
/// at 550 nm), its τ80 and τ30 above 30 km and its τ0 below, extrapolated beyond its data
/// (labelled). The haze below 30 km lies in Titan's troposphere, which the thermal infrared does not
/// see through, so its absorption warms the grey air as the ground's does; only the haze above it
/// is the anti-greenhouse's ([`haze_absorbed`]). At 550 nm the upper haze is 0.83 of the whole.
#[must_use]
fn haze_spectral_shape(wavelength_nm: f64) -> (f64, f64) {
    let upper = |l: f64| 1.012e7 * math::powf(l, -2.339) + 2.029e4 * math::powf(l, -1.409);
    let lower = |l: f64| 627.0 * math::powf(l, -0.9706);
    let whole = upper(550.0) + lower(550.0);
    (upper(wavelength_nm) / whole, lower(wavelength_nm) / whole)
}

/// The haze aggregate's ω and g at `wavelength_nm` ([`TITAN_HAZE_AGGREGATE`]), linear in
/// wavelength between its nodes and held at its end nodes.
#[must_use]
fn haze_aggregate_at(wavelength_nm: f64) -> (f64, f64) {
    let nodes = &TITAN_HAZE_AGGREGATE;
    let (first, last) = (nodes[0], nodes[nodes.len() - 1]);
    if wavelength_nm <= first.0 {
        return (first.1, first.2);
    }
    if wavelength_nm >= last.0 {
        return (last.1, last.2);
    }
    let above = nodes.partition_point(|node| node.0 <= wavelength_nm);
    let (a, b) = (nodes[above - 1], nodes[above]);
    let t = (wavelength_nm - a.0) / (b.0 - a.0);
    (a.1 + t * (b.1 - a.1), a.2 + t * (b.2 - a.2))
}

/// The diffuse reflectance and transmission of a homogeneous layer of optical depth `tau`,
/// single-scattering albedo `omega` and asymmetry `g`, over black and lit diffusely.
///
/// It is the δ-Eddington two-stream solution, in closed form (Joseph, Wiscombe and Weinman 1976, J. Atmos.
/// Sci. 33, 2452, δ-scaling f = g², τ′ = (1 − ωf)τ, ω′ = (1 − f)ω ÷ (1 − ωf), g′ = g ÷ (1 + g);
/// Meador and Weaver 1980's Eddington γ₁ = [7 − ω′(4 + 3g′)] ÷ 4 and γ₂ = −[1 − ω′(4 − 3g′)] ÷ 4,
/// with k = √(γ₁² − γ₂²): R = γ₂ sinh kτ′ ÷ (k cosh kτ′ + γ₁ sinh kτ′), T = k ÷ (k cosh kτ′ + γ₁
/// sinh kτ′); from memory, derived). A conservative layer, k = 0, takes its limit, R = γ₁τ′ ÷ (1 +
/// γ₁τ′) and T = 1 − R; a dark, forward layer whose R would be negative, R = 0.
#[must_use]
fn two_stream_layer(tau: f64, omega: f64, g: f64) -> (f64, f64) {
    let f = g * g;
    let scaled_tau = (1.0 - omega * f) * tau;
    let scaled_omega = (1.0 - f) * omega / (1.0 - omega * f);
    let scaled_g = g / (1.0 + g);
    let gamma_1 = (7.0 - scaled_omega * (4.0 + 3.0 * scaled_g)) / 4.0;
    let gamma_2 = -(1.0 - scaled_omega * (4.0 - 3.0 * scaled_g)) / 4.0;
    let square = gamma_1 * gamma_1 - gamma_2 * gamma_2;
    if square <= 1e-24 {
        let reflected = gamma_1 * scaled_tau / (1.0 + gamma_1 * scaled_tau);
        return (reflected, 1.0 - reflected);
    }
    let k = square.sqrt();
    let (sinh, cosh) = (math::sinh(k * scaled_tau), math::cosh(k * scaled_tau));
    let denominator = k * cosh + gamma_1 * sinh;
    let reflected = gamma_2 * sinh / denominator;
    // The closure's γ₂ turns negative for a dark, forward layer, ω′(4 − 3g′) < 1: its reflectance
    // is held at 0 there (the haze's own ω, at least 0.81, keeps it positive).
    let reflected = if reflected > 0.0 { reflected } else { 0.0 };
    (reflected, k / denominator)
}

/// A haze's two layers in one of the optics' bins, each its diffuse reflectance and transmission
/// ([`two_stream_layer`]): the haze above 30 km, the anti-greenhouse's, and the haze below it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct HazeBin {
    /// The haze above 30 km.
    upper: (f64, f64),
    /// The haze below 30 km, in the troposphere.
    lower: (f64, f64),
}

/// The haze's two layers in each of the optics' bins at its τ(550) `haze`.
///
/// Each bin is read at its midpoint wavelength ([`bin_wavelength_nm`]), each layer's τ there by the
/// haze's spectral shape ([`haze_spectral_shape`]) and both at the aggregate's ω and g
/// ([`haze_aggregate_at`]), by [`two_stream_layer`].
#[must_use]
fn haze_layer_bins(haze: f64) -> [HazeBin; OPTICS_BANDS] {
    core::array::from_fn(|bin| {
        let wavelength = bin_wavelength_nm(bin);
        let (omega, g) = haze_aggregate_at(wavelength);
        let (upper, lower) = haze_spectral_shape(wavelength);
        HazeBin {
            upper: two_stream_layer(haze * upper, omega, g),
            lower: two_stream_layer(haze * lower, omega, g),
        }
    })
}

/// The adding rule of a layer of reflectance and transmission `(reflected, through)` over a ground
/// of albedo `below`: R + T² A ÷ (1 − A R).
#[must_use]
fn over(below: f64, (reflected, through): (f64, f64)) -> f64 {
    reflected + through * through * below / (1.0 - below * reflected)
}

/// Whether a haze of τ(550) `haze` lies over its disc as a layer: under [`HazeLayerRules::Absorbing`]
/// with a haze ([`hazed_bins`]).
#[must_use]
pub(crate) fn hazes(haze: f64) -> bool {
    HazeLayerRules::in_force() == HazeLayerRules::Absorbing && haze > 0.0
}

/// The Bond albedo of a disc whose albedos in the optics' bins are `below` under a haze of τ(550)
/// `haze` (P14.T24.h.b's third part), under the light `shade`.
///
/// The haze ([`snapshot_haze_optical_depth`]) is laid over the rest, its air's Rayleigh layer and
/// cloud and ground, in each bin by the adding rule, its lower layer first and its upper over it
/// ([`haze_layer_bins`]), weighted by the light. `below`'s own Bond albedo, weighted alike, under
/// [`HazeLayerRules::Clear`] or with no haze.
#[must_use]
pub(crate) fn hazed_bins(below: &[f64; OPTICS_BANDS], haze: f64, shade: &HostShade) -> f64 {
    if !hazes(haze) {
        return band_weighted(below, shade.bands());
    }
    let layers = haze_layer_bins(haze);
    let total: [f64; OPTICS_BANDS] = core::array::from_fn(|bin| {
        let HazeBin { upper, lower } = layers[bin];
        over(over(below[bin], lower), upper)
    });
    band_weighted(&total, shade.bands())
}

/// The Bond albedo of a disc of Bond albedo `below`, the same in every bin, under a haze of τ(550)
/// `haze` under the light `shade` ([`hazed_bins`]); `below` to the bit under
/// [`HazeLayerRules::Clear`] or with no haze.
#[must_use]
pub(crate) fn hazed(below: f64, haze: f64, shade: &HostShade) -> f64 {
    if !hazes(haze) {
        return below;
    }
    hazed_bins(&[below; OPTICS_BANDS], haze, shade)
}

/// The share of the light the haze of τ(550) `haze` absorbs above 30 km over black, under the light
/// `shade`.
///
/// It is 1 − R − T of the upper layer in each bin ([`haze_layer_bins`]), weighted by the light: a
/// lower bound on its share over a ground and an air that return light through it (labelled;
/// Titan's dark ground returns little).
#[must_use]
pub(crate) fn haze_absorbed(haze: f64, shade: &HostShade) -> f64 {
    if haze.is_nan() || haze <= 0.0 {
        return 0.0;
    }
    let absorbed = haze_layer_bins(haze).map(|bin| 1.0 - bin.upper.0 - bin.upper.1);
    band_weighted(&absorbed, shade.bands())
}

/// The anti-greenhouse's share γ of the body's absorbed sunlight its upper haze takes (P14.T24.h.b's
/// third part): `absorbed` ([`haze_absorbed`]) over the body's `absorbed_share`, 1 − A, held to [0,
/// 1]; 0 under [`HazeLayerRules::Clear`], with no haze or no absorbed light.
///
/// The grey surface beneath it is [`anti_greenhouse`]'s.
#[must_use]
pub(crate) fn anti_greenhouse_share(absorbed: f64, absorbed_share: f64) -> f64 {
    if HazeLayerRules::in_force() == HazeLayerRules::Clear
        || absorbed.is_nan()
        || absorbed <= 0.0
        || absorbed_share.is_nan()
        || absorbed_share <= 0.0
    {
        return 0.0;
    }
    let gamma = absorbed / absorbed_share;
    if gamma < 1.0 { gamma } else { 1.0 }
}

/// The surface of a grey air beneath a haze that takes the share `gamma` of the absorbed sunlight
/// (P14.T24.h.b's third part): `T_s`⁴ = (1 − γ) `T_grey`⁴ + (γ ÷ 2) `T_base`⁴, `grey` the air's grey
/// surface ([`GreenhouseCeiling`](crate::planetary::derive::atmosphere)'s) and `base` its base.
///
/// A layer above the grey air that absorbs sunlight and is transparent to the thermal infrared
/// radiates what it absorbs half up and half down. The grey air below then carries a net flux of
/// only (1 − γ) of the absorbed sunlight, and the half sent down arrives at its top, so that the
/// two-stream grey solution gives σ`T_s`⁴ = σ`T_eq`⁴ [(1 − γ ÷ 2) + ¾τ (1 − γ)] (derived; the
/// two-layer form of McKay, Lorenz and Lunine 1999, Icarus 137, 56, to be read: the pod reaches no
/// source), `T_eq`(1 + ¾τ)^¼ being the grey surface: at γ = 1 the classic anti-greenhouse limit,
/// `T_eq` ÷ 2^¼. McKay, Pollack and Courtin 1991 (Science 253, 1118, to be read) put Titan's
/// anti-greenhouse at some 9 K (from memory). `grey` to the bit at γ = 0.
#[must_use]
pub(crate) fn anti_greenhouse(grey: Kelvin, base: Kelvin, gamma: f64) -> Kelvin {
    if gamma.total_cmp(&0.0).is_le() {
        return grey;
    }
    let fourth = |t: Kelvin| {
        let square = t.value() * t.value();
        square * square
    };
    let surface = (1.0 - gamma) * fourth(grey) + gamma / 2.0 * fourth(base);
    Kelvin::new(surface.sqrt().sqrt())
}

/// The anti-greenhouse's share γ in a greenhouse step whose partial pressures are `partials`
/// (P14.T24.h.b's third part).
///
/// The body's surface gravity and hottest host are `lit`, its light is `shade` and it absorbs
/// `absorbed_share` of the light it receives: its upper haze's absorption over black
/// ([`haze_absorbed`]) at its snapshot τ ([`snapshot_haze_optical_depth`]) through
/// [`anti_greenhouse_share`]. 0, read at no cost, under [`HazeLayerRules::Clear`].
#[must_use]
pub(crate) fn step_anti_greenhouse(
    partials: &PartialPressures,
    (gravity, hottest_host): (MetresPerSecondSquared, Kelvin),
    shade: &HostShade,
    absorbed_share: f64,
) -> f64 {
    if HazeLayerRules::in_force() == HazeLayerRules::Clear {
        return 0.0;
    }
    let haze = snapshot_haze_optical_depth(partials, gravity, hottest_host);
    if haze <= 0.0 {
        return 0.0;
    }
    anti_greenhouse_share(haze_absorbed(haze, shade), absorbed_share)
}

/// `run`'s result with P14.T24.h.b's holds at 26 set back to their held arms on this thread.
///
/// The four are the minor clouds, the sediment, the adding rule and the haze layer, set back for a
/// test that pins another hold's figures inside `with_every_26_hold` as they stood before them;
/// each set back after, a panic in `run` included. 26's bump deletes it with them.
#[cfg(test)]
pub(crate) fn with_scattering_held<R>(run: impl FnOnce() -> R) -> R {
    with_minor_clouds(MinorCloudsRules::WaterShare, || {
        with_organic_sediment(OrganicSedimentRules::Bare, || {
            with_rayleigh_adding(RayleighAddingRules::Max, || {
                with_haze_layer(HazeLayerRules::Clear, run)
            })
        })
    })
}

/// The Bond albedo of a conservative layer of reflectance `layer` over black laid over a ground of
/// Bond albedo `ground` (P14.T24.h.b's fourth part).
///
/// It is the adding rule for two diffusely reflecting layers, R + T² A ÷ (1 − A R) with the conservative layer's transmission T = 1 − R, that is R +
/// (1 − R)² A ÷ (1 − A R) (van de Hulst 1980, *Multiple Light Scattering*, the adding method,
/// from memory; decision-p14-t51-batch §9.4's form): the light the ground returns, passed through
/// the layer both ways and reflected between them. It is R at A = 0, 1 at A = 1, and at least
/// either, so that the floor it replaces still holds.
#[must_use]
pub(crate) fn rayleigh_added(layer: f64, ground: f64) -> f64 {
    let through = 1.0 - layer;
    layer + through * through * ground / (1.0 - ground * layer)
}

/// The reference Earth's air: 101,325 Pa of dry air, N₂, O₂ and Ar at their dry mole fractions with
/// 360 ppm of CO₂ (Bodhaine et al. 1999, J. Atmos. Ocean. Tech. 16, 1854, as P14.T24.h.a's tests
/// take it).
#[must_use]
pub(crate) fn reference_earth_air() -> PartialPressures {
    const DRY: [(Gas, f64); 4] = [
        (Gas::Nitrogen, 0.780_84),
        (Gas::Oxygen, 0.209_46),
        (Gas::Argon, 0.009_34),
        (Gas::CarbonDioxide, 0.000_36),
    ];
    let sum = DRY.iter().fold(0.0, |sum, &(_, x)| sum + x);
    PartialPressures::from_fn(|gas| {
        let x = DRY
            .iter()
            .find(|&&(g, _)| g == gas)
            .map_or(0.0, |&(_, x)| x);
        Pascals::new(STANDARD_ATMOSPHERE.value() * x / sum)
    })
}

/// The reference Earth's Rayleigh layer about a Sun: [`rayleigh_floor`] of its air
/// ([`reference_earth_air`]) at [`EARTH_SURFACE_GRAVITY`], about 0.058, which the water cloud's
/// re-split separates ([`water_cloud_albedo`]).
#[cfg(test)]
#[must_use]
pub(crate) fn reference_earth_rayleigh() -> f64 {
    rayleigh_floor_in(&reference_earth_air(), EARTH_SURFACE_GRAVITY, &sun_bands())
}

/// The area mix A = `f_c` `A_cloud` + (1 − `f_c`) `A_surface` of a surface of Bond albedo `surface`
/// under a cloud fraction `cloud` of Bond albedo `cloud_albedo`.
#[must_use]
pub(crate) fn under_cloud(surface: f64, cloud: f64, cloud_albedo: f64) -> f64 {
    cloud * cloud_albedo + (1.0 - cloud) * surface
}

/// The exponent of a Rayleigh optical depth's fall with wavelength, τ ∝ λ^−4.08: the local slope
/// of Hansen and Travis's (1974, Space Sci. Rev. 16, 527, eq. 2.30, p. 544) fit to Earth's air at
/// 1,013.25 mb, τ = 0.008569 λ⁻⁴ (1 + 0.0113 λ⁻² + 0.00013 λ⁻⁴) with λ in µm (0.0973 at 0.55 µm),
/// at 550 nm, −d ln τ ÷ d ln λ = 4.077. The
/// sixteen measured gases' own slopes over 500–600 nm run from He's 4.03 to H₂S's 4.16 (the
/// registry's sources, [`rayleigh`](crate::substance::rayleigh)); one exponent serves every air,
/// as decision-p14-t54a-passes §G3.5 rules.
pub(crate) const RAYLEIGH_WAVELENGTH_EXPONENT: f64 = 4.08;

/// The Eddington closure's γ₁ (= γ₂) for conservative scattering with no asymmetry, g = 0, which
/// Rayleigh's phase function has: [7 − ω(4 + 3g)] ÷ 4 = 3/4 at ω = 1, from I = I₀ + μI₁ under the
/// phase function 1 + 3gμμ′ (the closure's table in Meador and Weaver 1980, J. Atmos. Sci. 37,
/// 630, and Toon et al. 1989, J. Geophys. Res. 94, 16287, Table 1, from memory: neither paper's
/// text was read; the science check derived both coefficients).
const EDDINGTON_GAMMA_1: f64 = 0.75;

/// The Eddington closure's γ₃, the share of the singly scattered direct beam sent back up:
/// (2 − 3gμ₀) ÷ 4 = 1/2 at g = 0, whatever μ₀ (derived as [`EDDINGTON_GAMMA_1`] is).
const EDDINGTON_GAMMA_3: f64 = 0.5;

/// The orders of the exponential integral the conservative layer's spherical albedo takes
/// ([`exponential_integral`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExponentialOrder {
    /// `E₃`.
    Three,
    /// `E₄`.
    Four,
}

impl ExponentialOrder {
    /// The order n.
    const fn n(self) -> u32 {
        match self {
            Self::Three => 3,
            Self::Four => 4,
        }
    }
}

/// The exponential integral `E_n`(x) = ∫₁^∞ e^(−xt) t^(−n) dt of order `order` at x ≥ 0 (Abramowitz
/// and Stegun 1964, §5.1): its series below x = 1 and Lentz's continued fraction above it, each to
/// the double's precision (Press, Teukolsky, Vetterling and Flannery 2007, _Numerical Recipes_,
/// 3rd ed., §6.3, `expint`), with the Euler–Mascheroni constant of the series' digamma term from
/// `core` (Abramowitz and Stegun 1964, 6.1.3). `E_n`(0) = 1 ÷ (n − 1); 0 at +∞, and at any x whose
/// e^(−x) underflows.
#[must_use]
fn exponential_integral(order: ExponentialOrder, x: f64) -> f64 {
    /// The most terms either form takes; both converge in under 60 for n ≤ 4.
    const MOST_TERMS: u32 = 200;
    let n = order.n();
    let order = f64::from(n);
    let below = order - 1.0;
    if x.is_nan() || x <= 0.0 {
        return 1.0 / below;
    }
    if x.is_infinite() {
        return 0.0;
    }
    if x > 1.0 {
        // Lentz's method for the continued fraction 1 ÷ (x + n −) 1·n ÷ (x + n + 2 −) 2(n + 1) ÷ …
        let mut denominator = x + order;
        let mut upper = 1.0 / f64::MIN_POSITIVE;
        let mut lower = 1.0 / denominator;
        let mut fraction = lower;
        for i in 1..=MOST_TERMS {
            let fi = f64::from(i);
            let numerator = -fi * (below + fi);
            denominator += 2.0;
            lower = 1.0 / (numerator * lower + denominator);
            upper = denominator + numerator / upper;
            let step = upper * lower;
            fraction *= step;
            if (step - 1.0).abs() <= f64::EPSILON {
                break;
            }
        }
        return fraction * math::exp(-x);
    }
    let mut sum = 1.0 / below;
    let mut factor = 1.0;
    for i in 1..=MOST_TERMS {
        let fi = f64::from(i);
        factor *= -x / fi;
        let term = if i == n - 1 {
            let digamma = (1..n).fold(-core::f64::consts::EULER_GAMMA, |psi, k| {
                psi + 1.0 / f64::from(k)
            });
            factor * (digamma - math::ln(x))
        } else {
            -factor / (fi - below)
        };
        sum += term;
        if term.abs() <= sum.abs() * f64::EPSILON {
            break;
        }
    }
    sum
}

/// The spherical (Bond) albedo, 0 to 1, of a conservatively scattering layer of optical depth `tau`
/// with no asymmetry over a black surface, in the Eddington closure: its reflectance of a beam at
/// μ₀, the solution of Meador and Weaver's (1980) two-stream equations at ω = 1, γ₁ = γ₂, with no
/// diffuse light down at the top, F↓(0) = 0, and none up from the black ground, F↑(τ) = 0,
///
/// R(μ₀) = [γ₁τ + (γ₃ − γ₁μ₀)(1 − e^(−τ ÷ μ₀))] ÷ (1 + γ₁τ),
///
/// weighted over the sphere, 2 ∫₀¹ R(μ₀) μ₀ dμ₀ = [γ₁τ + γ₃(1 − 2`E₃`(τ)) − γ₁(⅔ − 2`E₄`(τ))] ÷
/// (1 + γ₁τ), with γ₁ = ¾ and γ₃ = ½ ([`EDDINGTON_GAMMA_1`], [`EDDINGTON_GAMMA_3`]) and `E_n` the
/// exponential integrals ([`exponential_integral`]). It is 0 at τ = 0, τ (γ₃'s 2γ₃ = 1) in a thin
/// layer, which is single scattering's τ ÷ 2μ₀ over the sphere, and goes to 1 as the layer
/// deepens, 1 − 1 ÷ (γ₁τ) at depth. It rises with τ. Zero for a depth that is not a number or not
/// positive; one at +∞.
///
/// The closure runs low: it gives a conservative g = 0 layer's spherical albedo 0 to 0.008 below
/// the exact value, most near τ ≈ 1–2 (0.4397 against 0.4469 at τ = 1), under 0.001 above τ ≈ 10
/// and below τ ≈ 0.1 (the science check's doubling solution with Rayleigh's polarised phase
/// matrix; Eddington's extrapolation length ⅔ lies below the exact 0.7104), so it keeps the
/// floor a lower bound.
#[must_use]
pub(crate) fn conservative_spherical_albedo(tau: f64) -> f64 {
    if tau.is_nan() || tau <= 0.0 {
        return 0.0;
    }
    if tau.is_infinite() {
        return 1.0;
    }
    let (g1, g3) = (EDDINGTON_GAMMA_1, EDDINGTON_GAMMA_3);
    let numerator = g1 * tau
        + g3 * (1.0 - 2.0 * exponential_integral(ExponentialOrder::Three, tau))
        - g1 * (2.0 / 3.0 - 2.0 * exponential_integral(ExponentialOrder::Four, tau));
    (numerator / (1.0 + g1 * tau)).clamp(0.0, 1.0)
}

/// The Rayleigh optical depth at 550 nm of an air of partial pressures `partials` under the surface
/// gravity `gravity`: τ = Σᵢ `N_i` σᵢ over its gases, each gas's column `N_i` = `x_i` `p_s` ÷ (g m̄)
/// molecules per square metre, with m̄ = Σ `x_i` `m_i` the mean molecular mass, so τ = `p_s` Σ `p_i` σᵢ
/// ÷ (g Σ `p_i` `m_i`), summed in [`Gas::ALL`]'s order, and σᵢ each row's cross section
/// ([`gas_cross_section`]). Zero for an air of no pressure, or a gravity that is not positive and
/// finite.
#[must_use]
pub(crate) fn rayleigh_optical_depth(
    partials: &PartialPressures,
    gravity: MetresPerSecondSquared,
) -> f64 {
    let (mut pressure, mut mass, mut scattering) = (0.0, 0.0, 0.0);
    for gas in Gas::ALL {
        let p = partials.of(gas).value();
        let sigma = gas_cross_section(gas).value();
        pressure += p;
        mass += p * gas.molecular_mass().value();
        scattering += p * sigma;
    }
    let g = gravity.value();
    if !(pressure > 0.0 && mass > 0.0 && g.is_finite() && g > 0.0) {
        return 0.0;
    }
    pressure * scattering / (g * mass)
}

/// The air's own Bond albedo over a black surface (P14.T24.h.a; decision-p14-t54a-passes §G3.5),
/// the floor under the snapshot path's: an air of partial pressures `partials` under the surface
/// gravity `gravity`, under the light `shade`.
///
/// Its Rayleigh optical depth at 550 nm ([`rayleigh_optical_depth`]) is taken to each of the
/// optics' twelve bins at λ^−4.08 ([`RAYLEIGH_WAVELENGTH_EXPONENT`]), at the bin's longest
/// wavelength, where the bin's air is thinnest, and none in the last bin, which stands for all
/// the light beyond 3,500 nm. Each bin's albedo is that depth's conservative layer's
/// ([`conservative_spherical_albedo`]), and the bins are weighted by the hosts' light in them
/// ([`band_weighted`], [`HostShade::bands`]).
///
/// It is a lower bound on the clear air's own Bond albedo, as far as its gases scatter without
/// absorbing:
///
/// - each bin's depth is the least in the bin for air's dispersion (Hansen and Travis's eq. 2.30
///   lies at or above the λ^−4.08 law at every bin's long edge), and below the bin's mean for any
///   of the sixteen gases, whose slopes run 4.03–4.16 against a rise of 1.5–2.9 times across a bin;
/// - the reflectance rises with depth, and the closure runs low
///   ([`conservative_spherical_albedo`]);
/// - a gas the registry holds no cross section of scatters nothing.
///
/// A conservative layer over any surface reflects at least what it reflects over a black one, and
/// at least what the surface does: `R_tot` = R + (1 − R)² A ÷ (1 − AR) ≥ R, and 1 − `R_tot` =
/// (1 − R)(1 − A) ÷ (1 − AR) ≤ 1 − A. So the larger of it and the area mix is a lower bound on the
/// body's, continuous in the passes' map as a max of continuous functions.
///
/// It neglects the gases' own absorption (CH₄'s red and near-infrared bands, H₂O's and CO₂'s
/// near-infrared bands). Under an absorbing air the floor can exceed the air's own Bond albedo by
/// up to the share of the hosts' light it reflects beyond the absorbers' edge: about 0.13–0.21 for
/// `filler_c` e201's CH₄ column (decision-p14-t54a-passes §G3.2's bound is 0.25). Recorded for
/// P14.T24.h.b, whose adding rule and absorbers take it up.
#[must_use]
pub(crate) fn rayleigh_floor(
    partials: &PartialPressures,
    gravity: MetresPerSecondSquared,
    shade: &HostShade,
) -> f64 {
    rayleigh_floor_in(partials, gravity, shade.bands())
}

/// [`rayleigh_floor`] under the light `bands` in the optics' bins.
#[must_use]
fn rayleigh_floor_in(
    partials: &PartialPressures,
    gravity: MetresPerSecondSquared,
    bands: &[f64; OPTICS_BANDS],
) -> f64 {
    rayleigh_bins(partials, gravity).map_or(0.0, |albedos| band_weighted(&albedos, bands))
}

/// The air's Rayleigh layer's albedo over black in each of the optics' bins ([`rayleigh_floor`]'s,
/// before the light weighs them), of partial pressures `partials` under the surface gravity
/// `gravity`; `None` for an air of no Rayleigh depth, or one not a number.
#[must_use]
pub(crate) fn rayleigh_bins(
    partials: &PartialPressures,
    gravity: MetresPerSecondSquared,
) -> Option<[f64; OPTICS_BANDS]> {
    let tau = rayleigh_optical_depth(partials, gravity);
    if tau.is_nan() || tau <= 0.0 {
        return None;
    }
    let mut albedos = [0.0; OPTICS_BANDS];
    for (bin, albedo) in albedos.iter_mut().enumerate().take(OPTICS_BANDS - 1) {
        let longest = SPECTRAL_BIN_EDGES_NM[bin + 1];
        let thinned = math::powf(
            RAYLEIGH_REFERENCE_WAVELENGTH_NM / longest,
            RAYLEIGH_WAVELENGTH_EXPONENT,
        );
        *albedo = conservative_spherical_albedo(tau * thinned);
    }
    Some(albedos)
}
