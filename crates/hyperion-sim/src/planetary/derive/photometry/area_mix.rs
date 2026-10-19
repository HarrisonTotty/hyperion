//! The area-mix Bond albedo (plan 14, P14.T51.b; decision-p14-t49c-split §1.5 and §2): a body's
//! Bond albedo as the area-weighted sum of its surface's components, A = Σ aᵢAᵢ, with the cloud
//! over them, and a gas envelope's from its class.
//!
//! Every piece is a pure function of the registry's rows and of plain values. T13.c's passes call
//! it only under the `p14-snapshot` lane's hold (`SnapshotRules::Batch`, `SNAPSHOT_VERSION`), until
//! P14.T54.a, so that nothing on the record moves at version 22; T49.c's tripwire holds this file
//! for that reason. Every optical value is read through the registry when it is used
//! (`Substance::condensed_optics` and its spherical albedos, and
//! [`liquid_properties`]), with no copied value, so that P14.T49.i's rows and bands flow through
//! with no edit here.
//!
//! # The host's light (decision-p14-t49c-split §2)
//!
//! Each component's Bond albedo is its spherical albedo by band weighted by the light its hosts
//! send in each band's edges, normalised over the bands given ([`band_weighted`]): a Planck curve
//! at each luminous host's effective temperature, its share of the host's light in each band
//! integrated by an 8-node Gauss–Legendre rule ([`planck_share`]), weighted by the host's flux at
//! the body ([`HostShadeSum`]). The sim's stellar spectrum (`sky::colour::star_colour`) is not
//! used: a body's hosts reach it as P14.T12's `HostLight`, which carries no surface gravity.
//!
//! - **The solids' bands** are B, V and R, contiguous over 380–760 nm at the midpoints between
//!   their effective wavelengths ([`VISIBLE_BAND_EDGES_NM`]), until P14.T49.i adds the infrared.
//!   The weighting reads slices of matching lengths, so T49.i's bins are data, not code.
//! - **The liquids' bins** are P14.T49.h's fifteen, R08's bake bins over 380–760 nm
//!   ([`LIQUID_BIN_EDGES_NM`]).
//!
//! # The components
//!
//! - **A solid** ([`solid_bond`]): its row's spherical albedo by band
//!   (`CondensedOptics::spherical_albedo_bvr`), never its normal albedo `A_N`, a radiance
//!   factor, and never a normal-incidence Fresnel reflectance.
//! - **A liquid** ([`liquid_bond`]): per bin the cosine-weighted Fresnel reflectance of a flat
//!   surface `r_d`(n) ([`diffuse_fresnel_reflectance`]) plus the light its body returns,
//!   (1 − `r_d`)(1 − `r_i`) R∞ ÷ (1 − `r_i` R∞), with R∞ = 0.33 `b_b` ÷ (a + `b_b`) and `b_b` = b ÷ 2
//!   ([`SUBSURFACE_REFLECTANCE_COEFFICIENT`]'s sources) and `r_i` the surface's
//!   internal diffuse reflectance ([`internal_diffuse_reflectance`]), from P14.T49.h's record at
//!   the temperature its condensate lies at. A record without a or b has no body term: a deep,
//!   flat, pure liquid ([`BodyTerm`]).
//! - **The regolith** ([`Regolith`]) on the surface the condensates leave: its lid's row, or while
//!   the passes do not know the lid a stand-in, on an airless rocky body the Moon's 0.11 exactly
//!   and a known lid's row at most 0.11 ([`airless_silicate_stand_in`]).
//! - **The cloud's condensable** weighs by 1 − d, its dominance ([`cloud_availability`]).
//! - **The cloud** ([`cloud_fraction`], [`cloud_albedo`]): the state's cloud fraction times the
//!   condensable's availability, at the state's cloud albedo.
//!
//! # A gas envelope (Sudarsky, Burrows and Pinto 2000)
//!
//! [`giant_bond`]: the "Fiducial" Bond albedos of Sudarsky, Burrows and Pinto's (2000, ApJ 538,
//! 885) Tables 1A–1B by the host's effective temperature, flux-weighted over the hosts, and then by
//! the planet's effective temperature across the five classes' model planets, piecewise-linear so
//! that the albedo and the temperature converge together.
//!
//! # The stand-ins ([`StandIn`])
//!
//! Each is labelled, never a silent default: a solid without optics takes water frost's; a
//! solution its solvent's record; a liquid without a record `r_d` at liquid nitrogen's index; a
//! liquid at a temperature no bound of its range holds its record at its first melt; an airless
//! rocky body whose lid is not known the Moon's 0.11, a known lid's airless silicate
//! regolith at most 0.11, any other lid not known a row by its material; a body with no luminous
//! host a Sun's light.

use hyperion_surface::field::{LIQUID_BINS, LiquidProperties};

use crate::math;
use crate::planetary::chem::partition::{
    Condensate, CondensateEntry, CondensatePhase, Condensates, Partition, WATER_SURFACE_HUMIDITY,
};
use crate::planetary::derive::atmosphere::{Gas, SurfaceMaterial, SurfaceState};
use crate::planetary::hooks::surface::regime::climate_condensable;
use crate::substance::optics::BAND_WAVELENGTHS_NM;
use crate::substance::{
    Composition, LIQUID_BIN_EDGES_NM, ResolveLiquidError, SubstanceId, by_key, diagram,
    liquid_properties,
};
use crate::tables::gauss_legendre::{GL8_NODES, GL8_WEIGHTS};
use crate::units::consts::SOLAR_EFFECTIVE_TEMPERATURE_K;
use crate::units::{EarthFluxes, Kelvin, KilogramsPerSquareMetre, MetresPerSecondSquared, Pascals};

#[cfg(test)]
mod tests;

/// How many bands the condensed optics carry: B, V and R ([`BAND_WAVELENGTHS_NM`]) until
/// P14.T49.i adds the infrared.
pub(crate) const OPTICS_BANDS: usize = BAND_WAVELENGTHS_NM.len();

/// The edges of the optics' bands, nm: 380, 495, 595 and 760, contiguous, for T49.c's B, V and R at
/// 440, 550 and 640 nm. The inner edges are the midpoints between consecutive effective
/// wavelengths, (440 + 550) ÷ 2 and (550 + 640) ÷ 2, and the outer ones the ends of R08's bake range,
/// the liquids' bins ([`LIQUID_BIN_EDGES_NM`]). T51.b's own until P14.T49.i states its contiguous
/// bins (decision-p14-t49c-split §1.3), whose edges replace these as data.
pub(crate) const VISIBLE_BAND_EDGES_NM: [f64; OPTICS_BANDS + 1] = [380.0, 495.0, 595.0, 760.0];

/// The second radiation constant c₂ = hc ÷ k, m K: 1.438 776 877 × 10⁻² (CODATA 2018, from the exact
/// h, c and k of the 2019 SI), the scale of a black body's spectrum in λT.
const SECOND_RADIATION_CONSTANT_M_K: f64 = 1.438_776_877e-2;

/// The share, 0 to 1, of the light of a black body at `temperature` that falls between the
/// wavelengths `shortest_nm` and `longest_nm` (nm, vacuum): (15 ÷ π⁴) ∫ x³ ÷ (eˣ − 1) dx over
/// x = c₂ ÷ λT between them, which is ∫ `B_λ` dλ over the band divided by σT⁴ ÷ π, by the sim's
/// 8-point Gauss–Legendre rule ([`GL8_NODES`], [`GL8_WEIGHTS`]; Abramowitz and Stegun 1964, Table
/// 25.4), exact for a polynomial of degree 15 in x, which a test holds within 10⁻¹² of a fine
/// Simpson sum on every band and bin from 2,000 to 40,000 K, summed in the table's order. Zero for a
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

/// Σ aᵢ wᵢ ÷ Σ wᵢ: the albedos `albedos` by band weighted by the light `weights` in each band,
/// normalised over the bands given, for any number of bands (the slices are of matching lengths).
/// Light of no weight in every band weighs the bands equally.
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

/// Classes I to V's Bond albedos about a host of effective temperature `host`: Sudarsky, Burrows
/// and Pinto's [`SUDARSKY_BOND`] interpolated linearly in the host's temperature between its hosts,
/// and held at M4V's below 3,200 K and A8V's above 7,500 K (a clamp, labelled: their table spans no
/// cooler or hotter host).
#[must_use]
pub(crate) fn sudarsky_bonds(host: Kelvin) -> [f64; SUDARSKY_CLASSES] {
    let mut bonds = [0.0; SUDARSKY_CLASSES];
    for (class, bond) in bonds.iter_mut().enumerate() {
        let column = SUDARSKY_BOND.map(|row| row[class]);
        *bond = piecewise_linear(&SUDARSKY_HOSTS_K, &column, host.value());
    }
    bonds
}

/// The light a body's hosts give it, by band, as the area mix reads it (P14.T51.b): each luminous
/// host's Planck shares in the optics' bands and the liquids' bins, weighted by its flux at the
/// body, its gas-envelope albedos by class flux-weighted, the water cloud's albedo calibrated on
/// Earth ([`water_cloud_albedo`]) and the reference cloud column ([`reference_cloud_column`]),
/// built once per body before its passes ([`HostShadeSum`]) and never across bodies.
#[derive(Debug, Clone, Copy)]
pub(crate) struct HostShade {
    bands: [f64; OPTICS_BANDS],
    bins: [f64; LIQUID_BINS],
    giants: [f64; SUDARSKY_CLASSES],
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

    /// Its light in each of the optics' bands ([`VISIBLE_BAND_EDGES_NM`]), unnormalised.
    #[must_use]
    pub(crate) const fn bands(&self) -> &[f64; OPTICS_BANDS] {
        &self.bands
    }
}

/// A [`HostShade`] being summed over a body's hosts, in their order: each luminous host's Planck
/// shares times its flux at the body, and its giant albedos by class times that flux.
///
/// Everything is linear in the hosts' fluxes, so it is summed once per body before the passes.
#[derive(Debug, Clone, Copy)]
pub(crate) struct HostShadeSum {
    bands: [f64; OPTICS_BANDS],
    bins: [f64; LIQUID_BINS],
    giants: [f64; SUDARSKY_CLASSES],
    flux: f64,
}

impl HostShadeSum {
    /// No host yet.
    pub(crate) const EMPTY: Self = Self {
        bands: [0.0; OPTICS_BANDS],
        bins: [0.0; LIQUID_BINS],
        giants: [0.0; SUDARSKY_CLASSES],
        flux: 0.0,
    };

    /// Adds a host of effective temperature `host` whose flux at the body is `flux`, its orbit-mean
    /// flux there. A host of no flux, or of a temperature that is not positive and finite, adds
    /// nothing.
    pub(crate) fn add(&mut self, flux: EarthFluxes, host: Kelvin) {
        let (flux, t) = (flux.value(), host.value());
        if !(flux.is_finite() && flux > 0.0 && t.is_finite() && t > 0.0) {
            return;
        }
        let bands: [f64; OPTICS_BANDS] = planck_shares(host, &VISIBLE_BAND_EDGES_NM);
        let bins: [f64; LIQUID_BINS] = planck_shares(host, &LIQUID_BIN_EDGES_NM);
        for (sum, share) in self.bands.iter_mut().zip(bands) {
            *sum += flux * share;
        }
        for (sum, share) in self.bins.iter_mut().zip(bins) {
            *sum += flux * share;
        }
        for (sum, bond) in self.giants.iter_mut().zip(sudarsky_bonds(host)) {
            *sum += flux * bond;
        }
        self.flux += flux;
    }

    /// The summed light, with the water cloud's calibration and the reference cloud column, each
    /// computed here once; a Sun's ([`HostShade::sun`], labelled) where no host added any.
    #[must_use]
    pub(crate) fn finish(self) -> HostShade {
        if self.flux.is_nan() || self.flux <= 0.0 {
            return HostShade::sun();
        }
        HostShade {
            bands: self.bands,
            bins: self.bins,
            giants: self.giants.map(|sum| sum / self.flux),
            water_cloud: water_cloud_albedo(),
            cloud_column: reference_cloud_column(),
        }
    }
}

/// A gas envelope's Bond albedo at its effective temperature `heated` (its equilibrium
/// temperature with its internal heat) about the hosts of `shade` (Sudarsky, Burrows and Pinto
/// 2000): the hosts' flux-weighted albedos of classes I to V, piecewise-linear between the classes'
/// model planets at [`SUDARSKY_PLANETS_K`] and held at class I's below 130 K and class V's above
/// 1,700 K, so that it is continuous in the temperature.
///
/// The classes are by effective temperature, as the paper has them, not by the equilibrium
/// temperature (P14.T51.b's plan text; a deviation, recorded). Class V's low-gravity case (g ≲ 10³
/// cm s⁻² is class V "even for `T_eff` < 1500 K", p. 897) is not modelled.
#[must_use]
pub(crate) fn giant_bond(shade: &HostShade, heated: Kelvin) -> f64 {
    piecewise_linear(&SUDARSKY_PLANETS_K, &shade.giants, heated.value())
}

/// Class I's Bond albedo about a host of effective temperature `host` ([`sudarsky_bonds`]): a
/// giant's at any effective temperature up to class I's 130 K model planet ([`giant_bond`]).
#[must_use]
pub(crate) fn class_i_bond(host: Kelvin) -> f64 {
    sudarsky_bonds(host)[0]
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

/// A liquid's Bond albedo from its P14.T49.h record `record`, weighted by the light `bins` in each
/// of its bins: [`liquid_bin_albedo`] bin by bin. P14.T49.i's infrared bins take `r_d` at the last
/// bin's index, where the body term vanishes (decision-p14-t49c-split §2), by appending their bins'
/// albedos and light here.
#[must_use]
pub(crate) fn liquid_bond(record: &LiquidProperties, bins: &[f64; LIQUID_BINS]) -> f64 {
    let mut albedos = [0.0; LIQUID_BINS];
    for (k, albedo) in albedos.iter_mut().enumerate() {
        *albedo = liquid_bin_albedo(record.refractive_index[k], BodyTerm::of(record, k));
    }
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
    /// An airless silicate regolith of a known lid takes at most the Moon's 0.11: space
    /// weathering's stand-in until P14.T54.b weathers the area mix ([`airless_silicate_stand_in`]).
    SpaceWeathering,
    /// An airless rocky body whose lid is not known takes the Moon's 0.11 exactly (NASA's fact
    /// sheet, T13.c's present value), until P14.T54.a passes T51.c's lid ([`Regolith::bond`]).
    MoonForAnUnknownAirlessLid,
    /// Any other regolith whose lid is not known takes a row by its bulk's material, T49.c's
    /// `basalt` for rock and `H2O`'s fresh frost for ice, until P14.T54.a passes T51.c's lid
    /// ([`Regolith::stand_in_row`]).
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
                "an airless silicate regolith takes at most the Moon's 0.11, space weathering's \
                 stand-in until P14.T54.b weathers the area mix"
            }
            Self::MoonForAnUnknownAirlessLid => {
                "an airless rocky body whose lid is not known takes the Moon's 0.11, until \
                 P14.T54.a passes T51.c's lid"
            }
            Self::RowByMaterial => {
                "a regolith whose lid is not known takes basalt for rock and water frost for ice, \
                 until P14.T54.a passes T51.c's lid"
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

/// The solid `id`'s Bond albedo under the light `bands` in the optics' bands: its spherical albedo
/// by band (`CondensedOptics::spherical_albedo_bvr`) weighted by the light ([`band_weighted`]),
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
            bond: band_weighted(&optics.spherical_albedo_bvr(), bands),
            stand_in: None,
        };
    }
    let frost = water()
        .substance()
        .condensed_optics()
        .expect("water's row has condensed optics (T49.c)");
    Component {
        bond: band_weighted(&frost.spherical_albedo_bvr(), bands),
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

/// The record of the liquid `id` at `temperature` at its reference composition
/// ([`LiquidRecord`]), each case named so that a row with a record is never read as having none:
///
/// - in its liquid range, its record there;
/// - below its first melt, −∞ included, its record at its first melt;
/// - at or above its critical point, +∞ included, its record a part in a million below it;
/// - at a temperature no bound holds, not a number or above a range with no critical point, its
///   record at its first melt, labelled ([`StandIn::RecordAtItsFirstMelt`]);
/// - for a row not modelled as a liquid, a member its record needs not modelled (there, or at the
///   bound), or a composition not modelled, none.
#[must_use]
fn liquid_record(id: SubstanceId, temperature: Kelvin) -> LiquidRecord {
    let (lowest, below) = match liquid_properties(id, temperature, Composition::Reference) {
        Ok(record) => return LiquidRecord::InRange(record),
        Err(ResolveLiquidError::OutOfRange { lowest, below, .. }) => (lowest, below),
        // The row has no record at this temperature, and holding it elsewhere would not give one.
        // A component's errors cannot occur at the reference composition, which names none.
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
    match liquid_properties(id, held, Composition::Reference) {
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
/// its record ([`StandIn::IndexForAnUnrecordedLiquid`]), with no body term.
///
/// # Panics
///
/// Never: nitrogen's row has a liquid record at its normal boiling point (P14.T49.h).
#[must_use]
fn unrecorded_liquid_bond() -> f64 {
    let nitrogen = Gas::Nitrogen.substance();
    let record = liquid_record(nitrogen, nitrogen_normal_boiling_point())
        .in_range()
        .expect("nitrogen's row has a liquid record at its normal boiling point (P14.T49.h)");
    diffuse_fresnel_reflectance(record.refractive_index_at(STAND_IN_INDEX_WAVELENGTH_NM))
}

/// The liquid entry `entry`'s Bond albedo under the light `bins`, from its P14.T49.h record at the
/// temperature its condensate lies at in `partition` ([`liquid_record`]): at a temperature no bound
/// of its range holds, its record at its first melt ([`StandIn::RecordAtItsFirstMelt`]); a
/// solution without a record takes its solvent's ([`StandIn::SolventForASolution`]), and a row
/// without one `r_d` at liquid nitrogen's index ([`StandIn::IndexForAnUnrecordedLiquid`]).
#[must_use]
pub(crate) fn liquid_entry_bond(
    entry: &CondensateEntry,
    partition: &Partition,
    bins: &[f64; LIQUID_BINS],
) -> Component {
    let liquid = entry.substance();
    let condensate = partition
        .condensates()
        .iter()
        .find(|c| c.liquid() == liquid);
    let temperature = condensate.map_or(partition.climate().mean(), Condensate::lies_at);
    match liquid_record(liquid, temperature) {
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
    match solvent.map(|solvent| liquid_record(solvent, temperature)) {
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
/// body's silicate regolith of a known lid takes the smaller of its row's Bond albedo `row` and the
/// Moon's 0.11
/// (NASA's fact sheet; T13.c's present value). A reference form is a fresh surface, and a fresh
/// lithology would make an airless rocky body far too bright; so it never brightens an airless
/// silicate body against today's value, and it lets an intrinsically dark row go darker. Labelled
/// ([`StandIn::SpaceWeathering`]); P14.T54.b deletes it and weathers the area mix with R10.T1.d's
/// law over P14.T51.d's maturity and rate, through [`Regolith::weathered`]
/// (decision-p14-t51d-weathering).
#[must_use]
pub(crate) fn airless_silicate_stand_in(row: f64) -> f64 {
    let cap = moon_bond();
    if row < cap { row } else { cap }
}

/// How the regolith's row is weathered (the lane's seam for P14.T51.d and P14.T54.b, 2026-10-16;
/// decision-p14-t51d-weathering).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum RegolithWeathering {
    /// As the passes take it until P14.T54.b: an airless silicate regolith of a known lid takes the
    /// space-weathering stand-in, the smaller of its row's Bond albedo and the Moon's 0.11
    /// ([`airless_silicate_stand_in`], labelled), and of a lid not known the Moon's 0.11 exactly; an
    /// icy regolith, or one under air, its row as it is ([`Regolith::bond`]). P14.T54.b deletes this
    /// arm when it weathers the area mix with R10.T1.d's law.
    StandIn,
    /// Weathered: its spherical albedos by band, `spherical_bvr`, which P14.T54.b computes from the
    /// row by R10.T1.d's law at P14.T51.d's maturity and rate, and which weigh by the host's bands
    /// as any solid's do ([`Regolith::weathered`]). T51.d's tests reach the area mix through it
    /// with no wiring.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "P14.T54.b's weathering constructs it; P14.T51.d's tests and these do now"
        )
    )]
    Weathered {
        /// The weathered spherical albedos in the optics' bands.
        spherical_bvr: [f64; OPTICS_BANDS],
    },
}

/// The regolith on the surface a body's condensates leave (P14.T51.b): "the crust's regolith".
///
/// Its row is an input of the area mix, a seam that P14.T54.a fills from T51.c's lid; until then
/// the lid is not known and the passes take a stand-in by the material of the bulk their air reads
/// ([`Regolith::of_material`], [`Regolith::bond`]). Its fields are private and its constructors
/// name each case, so that a lid's row never sits unread under a weathered surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Regolith {
    /// Its lid's row, T51.c's from P14.T54.a; `None` where the lid is not known, and on a weathered
    /// regolith, whose band albedos carry its row.
    row: Option<SubstanceId>,
    /// The material of the body's bulk: rock is silicate, which the space-weathering stand-in
    /// takes on an airless body.
    material: SurfaceMaterial,
    /// How it is weathered.
    weathering: RegolithWeathering,
}

impl Regolith {
    /// The passes' regolith of a body of `material` whose lid is not known, until P14.T54.a gives
    /// them T51.c's, weathered by the stand-in.
    #[must_use]
    pub(crate) const fn of_material(material: SurfaceMaterial) -> Self {
        Self {
            row: None,
            material,
            weathering: RegolithWeathering::StandIn,
        }
    }

    /// The regolith of a known lid's row `row` on a body of `material`, weathered by the stand-in:
    /// P14.T54.a's, from T51.c's lid.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "P14.T54.a passes T51.c's lid through it; the tests reach it now"
        )
    )]
    #[must_use]
    pub(crate) const fn with_lid(row: SubstanceId, material: SurfaceMaterial) -> Self {
        Self {
            row: Some(row),
            material,
            weathering: RegolithWeathering::StandIn,
        }
    }

    /// A weathered regolith on a body of `material`, of spherical albedos by band `spherical_bvr`
    /// ([`RegolithWeathering::Weathered`]): P14.T54.b's, which computes them from the lid's row,
    /// so that it holds no row of its own.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "P14.T54.b's weathering builds it; P14.T51.d's tests and these reach it now"
        )
    )]
    #[must_use]
    pub(crate) const fn weathered(
        material: SurfaceMaterial,
        spherical_bvr: [f64; OPTICS_BANDS],
    ) -> Self {
        Self {
            row: None,
            material,
            weathering: RegolithWeathering::Weathered { spherical_bvr },
        }
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
    /// - weathered, its band albedos weighted by the light;
    /// - under the stand-in, on an airless rocky body the smaller of its known lid's row's albedo
    ///   and the Moon's 0.11 ([`airless_silicate_stand_in`]), or the Moon's 0.11 exactly where its
    ///   lid is not known, so that the stand-in neither brightens nor darkens a body whose lid the
    ///   passes do not know (decision-p14-t49c-split §1.5); on any other body its lid's row as it
    ///   is, or where the lid is not known the row by its material ([`Regolith::stand_in_row`]),
    ///   each labelled.
    #[must_use]
    pub(crate) fn bond(&self, state: SurfaceState, bands: &[f64; OPTICS_BANDS]) -> Component {
        if let RegolithWeathering::Weathered { spherical_bvr } = self.weathering {
            return Component {
                bond: band_weighted(&spherical_bvr, bands),
                stand_in: None,
            };
        }
        let airless_rock = state == SurfaceState::Airless && self.material == SurfaceMaterial::Rock;
        match (self.row, airless_rock) {
            (Some(row), true) => Component {
                bond: airless_silicate_stand_in(solid_bond(row, bands).bond),
                stand_in: Some(StandIn::SpaceWeathering),
            },
            (Some(row), false) => solid_bond(row, bands),
            (None, true) => Component {
                bond: moon_bond(),
                stand_in: Some(StandIn::MoonForAnUnknownAirlessLid),
            },
            (None, false) => Component {
                bond: solid_bond(Self::stand_in_row(self.material), bands).bond,
                stand_in: Some(StandIn::RowByMaterial),
            },
        }
    }
}

/// The surface's Bond albedo, beneath any cloud: Σ over the published condensates `condensates`
/// of each entry's area × its Bond albedo (a solid's [`solid_bond`], a liquid's
/// [`liquid_entry_bond`] at its condensate's temperature in `partition`; a supercritical entry has
/// no interface and takes no area), plus the regolith's albedo `regolith` on the rest, max(0, 1 −
/// Σ areas), under the light of `shade`. Where the areas sum above 1 (ices that overlap, or an ice
/// over a sea, as R09's cover convention allows) they are scaled to 1, as T51.a's cover is.
#[must_use]
pub(crate) fn surface_bond(
    condensates: &Condensates,
    partition: &Partition,
    regolith: f64,
    shade: &HostShade,
) -> f64 {
    let areas = condensates.entries().iter().fold(0.0, |sum, entry| {
        sum + match entry.phase() {
            CondensatePhase::Solid | CondensatePhase::Liquid => entry.area_fraction(),
            CondensatePhase::Supercritical => 0.0,
        }
    });
    let scale = if areas > 1.0 { 1.0 / areas } else { 1.0 };
    let covered = areas * scale;
    let rest = if covered < 1.0 { 1.0 - covered } else { 0.0 };
    condensates
        .entries()
        .iter()
        .fold(rest * regolith, |sum, entry| match entry.phase() {
            CondensatePhase::Solid => {
                sum + entry.area_fraction()
                    * scale
                    * solid_bond(entry.substance(), &shade.bands).bond
            }
            CondensatePhase::Liquid => {
                let bond = liquid_entry_bond(entry, partition, &shade.bins).bond;
                sum + entry.area_fraction() * scale * bond
            }
            CondensatePhase::Supercritical => sum,
        })
}

/// The share of the disc under cloud of a body in the state `state` before its condensable's
/// availability (the lane's stand-in, P14.T51.b, recorded): a runaway greenhouse 1, Venus's deck
/// (T13.c's table); a temperate world and a snowball Earth's 0.67 (King et al. 2013, IEEE TGRS 51,
/// 3826, as T13.c's table cites it); a gas envelope 1, whose albedo is its class's, clouds
/// included ([`giant_bond`]); airless and molten surfaces none.
#[must_use]
pub(crate) fn cloud_share_of_state(state: SurfaceState) -> f64 {
    match state {
        SurfaceState::RunawayGreenhouse | SurfaceState::GasEnvelope => {
            SurfaceState::RunawayGreenhouse.cloud_fraction()
        }
        SurfaceState::Temperate | SurfaceState::Snowball => {
            SurfaceState::Temperate.cloud_fraction()
        }
        SurfaceState::Airless | SurfaceState::MagmaOcean => 0.0,
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
/// full cover with margin and a Mars-like dry world falls near none. Computed once a body, into
/// its [`HostShade`].
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
/// rule, 2026-10-16). None where the air has no such condensable, or no pressure.
#[must_use]
pub(crate) fn cloud_availability(partition: &Partition, shade: &HostShade) -> f64 {
    let pressure = partition.surface_pressure();
    let p = pressure.value();
    let g = partition.ground().gravity().value();
    if !(p > 0.0 && p.is_finite() && g > 0.0 && g.is_finite()) {
        return 0.0;
    }
    let gases: Vec<(SubstanceId, f64)> = partition
        .species()
        .iter()
        .map(|s| (s.substance(), s.partial_pressure().value() / p))
        .collect();
    let climate = partition.climate();
    let range = (climate.coldest(), climate.warmest());
    let Some(condensable) = climate_condensable(&gases, pressure, range, &partition.reservoirs())
    else {
        return 0.0;
    };
    let (partial, dominance) = partition.of(condensable).map_or((0.0, 0.0), |s| {
        (s.partial_pressure().value(), s.dominance())
    });
    let available = (1.0 - dominance) * partial / g / shade.cloud_column.value();
    if available.is_nan() {
        0.0
    } else {
        available.clamp(0.0, 1.0)
    }
}

/// The cloud fraction of a body in the state `state` whose partition is `partition`, under the
/// light `shade`: its state's share ([`cloud_share_of_state`]) times its condensable's
/// availability ([`cloud_availability`]), the whole for a runaway greenhouse, which keeps its deck
/// with or without sulphur, and a gas envelope (the plan: "never from the Surface path's decks").
#[must_use]
pub(crate) fn cloud_fraction(state: SurfaceState, partition: &Partition, shade: &HostShade) -> f64 {
    let of_state = cloud_share_of_state(state);
    match state {
        SurfaceState::RunawayGreenhouse | SurfaceState::GasEnvelope => of_state,
        SurfaceState::Temperate | SurfaceState::Snowball => {
            of_state * cloud_availability(partition, shade)
        }
        SurfaceState::Airless | SurfaceState::MagmaOcean => 0.0,
    }
}

/// The Bond albedo of the cloud of a body in the state `state` under the light of `shade`: a
/// runaway greenhouse's Venus's 0.76 (Haus et al. 2016, Icarus 272, 178, T13.c's table), so that
/// Venus keeps it exactly at its full cover; a temperate world's and a snowball's the water cloud
/// calibrated on Earth ([`water_cloud_albedo`]); none for the rest.
#[must_use]
pub(crate) fn cloud_albedo(state: SurfaceState, shade: &HostShade) -> f64 {
    match state {
        SurfaceState::RunawayGreenhouse => SurfaceState::RunawayGreenhouse
            .albedo(SurfaceMaterial::Rock)
            .value(),
        SurfaceState::Temperate | SurfaceState::Snowball => shade.water_cloud,
        SurfaceState::GasEnvelope | SurfaceState::Airless | SurfaceState::MagmaOcean => 0.0,
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
/// # Panics
///
/// Never: water's row has a liquid record at 288 K (P14.T49.h).
#[must_use]
pub(crate) fn reference_earth_clear_sky() -> f64 {
    let sun = sun_light();
    let ocean = liquid_record(water(), Kelvin::new(REFERENCE_EARTH_TEMPERATURE_K))
        .in_range()
        .expect("water's row has a liquid record at 288 K (P14.T49.h)");
    let land = Regolith::of_material(SurfaceMaterial::Rock).bond(SurfaceState::Temperate, &sun.0);
    let rest = 1.0 - REFERENCE_EARTH_OCEAN - REFERENCE_EARTH_ICE;
    REFERENCE_EARTH_OCEAN * liquid_bond(&ocean, &sun.1)
        + REFERENCE_EARTH_ICE * solid_bond(water(), &sun.0).bond
        + rest * land.bond
}

/// A 5,772 K Planck curve's light in the optics' bands and the liquids' bins.
#[must_use]
fn sun_light() -> ([f64; OPTICS_BANDS], [f64; LIQUID_BINS]) {
    let sun = Kelvin::new(SOLAR_EFFECTIVE_TEMPERATURE_K);
    (
        planck_shares(sun, &VISIBLE_BAND_EDGES_NM),
        planck_shares(sun, &LIQUID_BIN_EDGES_NM),
    )
}

/// A Sun's light in the optics' bands, at which the photometry's references are taken
/// ([`HostShade::sun`]).
#[must_use]
pub(crate) fn sun_bands() -> [f64; OPTICS_BANDS] {
    sun_light().0
}

/// The water cloud's Bond albedo (the lane's stand-in, P14.T51.b, recorded): the one albedo that
/// gives Earth's area mix NASA's 0.294 at its 0.67 of cloud, both T13.c's table's, `A_cloud` =
/// (0.294 − (1 − 0.67) `A_clear`) ÷ 0.67, with `A_clear` the reference Earth's clear sky computed
/// from the registry ([`reference_earth_clear_sky`]), so that it follows the registry's rows.
#[must_use]
pub(crate) fn water_cloud_albedo() -> f64 {
    let earth = SurfaceState::Temperate
        .albedo(SurfaceMaterial::Rock)
        .value();
    let cloud = SurfaceState::Temperate.cloud_fraction();
    (earth - (1.0 - cloud) * reference_earth_clear_sky()) / cloud
}

/// The area mix A = `f_c` `A_cloud` + (1 − `f_c`) `A_surface` of a surface of Bond albedo `surface`
/// under a cloud fraction `cloud` of Bond albedo `cloud_albedo`.
#[must_use]
pub(crate) fn under_cloud(surface: f64, cloud: f64, cloud_albedo: f64) -> f64 {
    cloud * cloud_albedo + (1.0 - cloud) * surface
}
