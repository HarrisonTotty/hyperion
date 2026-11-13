//! A gas envelope's own Bond albedo (plan 14, P14.T24.h.b part 5; decision-deep-decks-split §6.6):
//! the light its column returns from above its floor, the level where τ(550) from the top reaches
//! 10, in place of T51.b's class table ([`giant_bond`](super::area_mix::giant_bond)), on the
//! snapshot path ([`gas_envelope_light`]).
//!
//! With it come part 6, the cold giants' clouds the albedo reads, on the record and in the closed
//! form alike: the chromophore (at the disc's typical column to 30, from 31 mixed in its haze's
//! gas, `photochem::CHROMOPHORE_MIXING_RATIO`), the haze above a Jupiter-like ammonia cloud
//! (`photochem::ammonia_haze`), whose τ(550) is fitted on Jupiter's measured Bond albedo, and κ₁
//! refitted to the Galileo probe at that albedo (`GreyProfile::ADOPTED`). Part 7 draws an ice
//! giant's CH₄ deck as its methane layer, Irwin et al.'s (2022) Aerosol-2 of haze cores
//! (`envelope_atmosphere::methane_layer_mode`). Part 8 lowers a cold envelope's κ₁ with its
//! effective temperature below 120 K (`GreyProfile::at_effective_temperature`), so that the cold
//! giants meet their measured 1-bar temperatures.
//!
//! # The column
//!
//! The snapshot path cannot derive an envelope: P14.T52.d's chain costs 20–27 ms a giant, 13–22 ms
//! of it the condensation loop (pod, 16 vCPU, the sim at opt-level 2), where a pass costs some tens
//! of microseconds. So its passes read a closed form of the decks' columns ([`column`](mod@column)), at about
//! 0.1 ms a column and 0.1 ms its light: P14.T24.d's profile, P14.T50.b's abundances, each deck the
//! loop would draw above 100 bar at its base, Ackerman and Marley's column and eq. 13's sizes, and
//! P14.T52.b's hazes, each through the record's own rules where it reads one. The record's
//! envelope is not read: a snapshot's giant takes its closed form's column. The same light over the
//! record's own modes gives the table's giants and the hot fixture within 0.04 of it (the tests).
//!
//! # The light (the engine)
//!
//! In each of the optics' twelve bins ([`SPECTRAL_BIN_EDGES_NM`]) the column is a stack of
//! homogeneous layers over a black floor, each layer's diffuse reflectance and transmission by
//! a δ-scaled two-stream ([`layer_two_stream`]; the δ-PIFM closure, "The column read whole"
//! below) and the layers joined by the adding rule from the floor up, at
//! each of eight g-points of the bin's correlated-k gas absorption (the gases' k-distributions taken as perfectly correlated, a stated approximation); the bin's spherical
//! albedo is their weighted sum, and the Bond albedo the bins weighted by the hosts' light
//! ([`band_weighted`]). Each layer holds:
//!
//! - **Rayleigh** scattering by its gas at 550 nm ([`gas_cross_section`]), taken to the bin as
//!   the bin's mean of λ^−4.08 (T24.h.a's local slope; [`rayleigh_bin_factor`]);
//! - **the lines** of CH₄, H₂O and CO and the pairs' absorption (H₂–H₂, and the alkali doublets'
//!   far wings with H₂ and He), [`opacity`]'s tables:
//!   - CH₄ over 380–800 nm from Karkoschka and Tomasko 2010 (Icarus 205, 674) as the client's
//!     `crossSections.json` holds it, its Eq. 8 in temperature over 50–300 K and held beyond;
//!   - CH₄ from 851 nm, and H₂O, CO and H₂–H₂ everywhere, from H. Nicholls and L. Soucasse's
//!     SOCRATES spectral files `Dayspring` (Zenodo 15721749 and 15799474, CC BY 4.0, 2025),
//!     their k-terms fitted to the DACE opacity database (Grimm et al. 2021, ApJS 253, 30, from
//!     memory): the 256-band file for CH₄ over 851–1,024 nm, the 48-band file beyond and for the
//!     rest, each band's terms shared out by its overlap with the bin;
//!   - the Na and K resonance doublets' far wings beyond 25 cm⁻¹ (R08.T4.e), the client's
//!     `alkaliPairs.json`: Allard et al. 2019's Na–H₂, and the ruled impact estimates for Na–He,
//!     K–H₂ and K–He, over 380–800 nm;
//!   - nothing of CH₄ over 800–851 nm (no source in the repository; a window there, so a lower
//!     bound on its absorption), of H₂–He's pairs, NH₃, PH₃ or the alkali cores (each labelled);
//! - **the particles** of each mode by their τ(550) between the layer's levels, their profile's
//!   (p ÷ `p_b`)^f, each material's extinction relative to 550 nm, ω and g in the bin from
//!   [`opacity::PARTICLES`]: Mie spheres (Bohren and Huffman's `BHMIE`) of a `σ_g` 2 log-normal at
//!   the client's index files over 380–780 nm and `hyperion-fit`'s infrared sets beyond (held at
//!   780 nm's where a material has none), the geometric limit past a size parameter of 150, and
//!   the volatile ices' crystals at g 0.75 (Yang et al. 2013's roughened aggregate, J. Atmos. Sci.
//!   70, 330, as the client's crystal table gives it) past a size parameter of 20.
//!
//! # What is not modelled
//!
//! - **Below the floor** nothing is read: a conservative deck read whole would give the albedo of
//!   a semi-infinite cloud (decision-deep-decks-split §6.6), so the floor is black (in a column
//!   with the bright haze, each bin's own floor; "The column read whole").
//! - **Thermal emission** in the last bins, and the alkali lines' cores (within 25 cm⁻¹).
//! - **The host's \[Fe/H\] and C/O on the snapshot path**: the closed form takes a protosolar host
//!   ([`column`](mod@column)).
//!
//! # The column read whole
//!
//! The column starts at its highest mode's top where that lies above [`COLUMN_TOP_PA`], so that a
//! haze above it (Gao et al.'s soot, which forms at 0.1 Pa) is read, and every layer takes the
//! δ-PIFM closure (Zdunkowski, Welch and Korb 1980), whose γ₂ is never negative and which is the
//! δ-Eddington's at ω = 1. Against an exact doubling of a Henyey–Greenstein layer lit diffusely
//! (the lane's check, 48 streams a hemisphere, re-derived by its science check) it runs 0.5–9% low
//! over ω 0.1–1 and τ 0.3–17, where the δ-Eddington runs 0.005–0.06 low and below 0 for ω under
//! about 0.3, so that a dark layer, ω′(4 − 3g′) < 1, would reflect nothing under it (its γ₂ < 0,
//! held at R = 0). A gamma soot mode, as the record draws Gao et al.'s, reads its own narrow Mie table
//! ([`narrow_table_for`]). A gas envelope above
//! `HOT_GIANT_TEMPERATURE`, which has no analogue, then takes its p from its column's own albedo
//! by band (`photometry`'s module documentation).
//!
//! By research-leans-50's ruling (the lane `fix-dark-sub-neptune-2`) the soot's column is shared by the zero-albedo `T_eq` with a bright organic haze, Khare et al.'s tholin
//! (`envelope_atmosphere::haze_shares`, `photochem::bright_haze`), on the record and in the closed
//! form alike; the tholin reads a narrow table of its own and has no wide one; and the closed form
//! gives the bright haze's share of its hazes' light by bin
//! ([`column::ClosedLight::bright_share`]), by which the photometry weighs a provisional
//! envelope's q (`photometry::BRIGHT_HAZE_PHASE_INTEGRAL`).
//!
//! By research-leans-52's ruling (2026-11-13, the lane `fix-dark-sub-neptune-3`) the bright haze
//! has a coagulated mode beneath its slab (`photochem::coagulated_haze`), and a column
//! that holds the bright haze is read in each bin down to where that bin's own τ, its Rayleigh and
//! its particles', reaches 10 ([`ColumnFloor::EachBin`]), not τ(550): its infrared, where the
//! 0.1 µm slab is thin, is read in the coagulated mode below. The crossing layer is cut there, a
//! homogeneous layer's share. Every other column keeps the floor at τ(550) 10, so the giants'
//! columns stay as they were (a lean, landed as ruled: the per-bin floor would move Saturn's B/V
//! and R/V by 0.02 and Uranus's Bond albedo by 0.004).
//!
//! # Determinism
//!
//! Every loop's count is fixed, every sum runs in a fixed order and every transcendental is
//! [`math`]'s.

use crate::math;
use crate::planetary::chem::photochem::HazeRule;
use crate::planetary::derive::atmosphere::Gas;
use crate::planetary::derive::photometry::area_mix::{HostShade, OPTICS_BANDS, band_weighted};
use crate::planetary::derive::photometry::{Bands, BrightShare, DiscColour};
use crate::substance::optics::SPECTRAL_BIN_EDGES_NM;
use crate::substance::rayleigh::gas_cross_section;
use crate::substance::{CondensedPhase, Element, MOLAR_GAS_CONSTANT};
use crate::units::consts::BOLTZMANN_CONSTANT;

pub(crate) mod column;
pub(crate) mod narrow;
pub(crate) mod opacity;
#[cfg(test)]
mod tests;

/// The Rayleigh law's index in wavelength over the optics' bins, 4.08: T24.h.a's local slope at
/// 550 nm of Hansen and Travis 1974's eq. 2.30 (`area_mix`'s floor takes the same).
const RAYLEIGH_INDEX: f64 = 4.08;

/// The reference wavelength of every τ(550), nm.
const REFERENCE_NM: f64 = 550.0;

/// The absorption optical depth from the top below which a g-point's deeper layers are not added:
/// 30, two ways e^−60 of the light, under any float's resolution of an albedo.
const OPAQUE_ABSORPTION: f64 = 30.0;

/// The gases a layer's absorption reads, by mole fraction.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Absorbers {
    /// H₂.
    pub(crate) hydrogen: f64,
    /// He.
    pub(crate) helium: f64,
    /// CH₄.
    pub(crate) methane: f64,
    /// H₂O.
    pub(crate) water: f64,
    /// CO.
    pub(crate) carbon_monoxide: f64,
    /// Na, the atom.
    pub(crate) sodium: f64,
    /// K, the atom.
    pub(crate) potassium: f64,
}

/// One homogeneous layer of an envelope's column above its floor, as the engine reads it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ColumnLayer {
    /// Its top, Pa.
    pub(crate) top: f64,
    /// Its bottom, Pa.
    pub(crate) bottom: f64,
    /// Its temperature, K, at its geometric mean pressure.
    pub(crate) temperature: f64,
    /// Its gas's absorbers.
    pub(crate) gas: Absorbers,
    /// Its particles in each bin: extinction optical depth, scattering optical depth, and that
    /// times g.
    pub(crate) particles: [[f64; 3]; OPTICS_BANDS],
}

/// Where a column's floor lies ([`assemble`], [`bin_albedos`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum ColumnFloor {
    /// R08's: where τ(550) from the top, the gas's Rayleigh and every mode's, reaches
    /// [`FLOOR_OPTICAL_DEPTH`], in every bin alike.
    Visual,
    /// In each bin where that bin's own τ from the top, its Rayleigh and its particles'
    /// extinction, reaches [`FLOOR_OPTICAL_DEPTH`], the layer it crosses cut there (a
    /// homogeneous layer's share, exact for one): a column that holds the bright haze's
    /// (research-leans-52's ruling, 2026-11-13; the [module documentation](self), "The column read
    /// whole").
    EachBin,
}

impl ColumnFloor {
    /// The floor of a column whose photochemical hazes are those of the rules `hazes`:
    /// [`ColumnFloor::EachBin`] with the bright haze ([`HazeRule::Bright`]),
    /// [`ColumnFloor::Visual`] otherwise, so that the giants' columns stay as they were (a lean,
    /// 2026-11-13, landed as ruled).
    #[must_use]
    pub(crate) fn of(hazes: impl IntoIterator<Item = HazeRule>) -> Self {
        if hazes.into_iter().any(|rule| rule == HazeRule::Bright) {
            Self::EachBin
        } else {
            Self::Visual
        }
    }
}

/// An envelope's column above its floor: its gravity, its gas's mean molar mass and its Rayleigh
/// cross section at 550 nm (per molecule of the mixture), its layers from the top down, and where
/// its floor lies in each bin.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct EnvelopeColumn {
    /// The gravity, m s⁻².
    pub(crate) gravity: f64,
    /// The gas's mean molar mass, g mol⁻¹.
    pub(crate) molar_mass_g_per_mol: f64,
    /// The gas's Rayleigh cross section at 550 nm, m² a molecule.
    pub(crate) rayleigh_550_m2: f64,
    /// The layers, top first.
    pub(crate) layers: Vec<ColumnLayer>,
    /// Its floor: at its last layer's bottom in every bin, or cut in each bin.
    pub(crate) floor: ColumnFloor,
}

/// The mean over bin `bin` of (550 ÷ λ)^4.08, uniform in wavelength: the bin's Rayleigh optical
/// depth over its 550 nm one.
#[must_use]
pub(crate) fn rayleigh_bin_factor(bin: usize) -> f64 {
    let (low, high) = (SPECTRAL_BIN_EDGES_NM[bin], SPECTRAL_BIN_EDGES_NM[bin + 1]);
    let index = RAYLEIGH_INDEX - 1.0;
    math::powf(REFERENCE_NM, RAYLEIGH_INDEX) * (math::powf(low, -index) - math::powf(high, -index))
        / (index * (high - low))
}

/// The Rayleigh cross section at 550 nm of a gas `gas`, m² a molecule: each of its gases' rows by
/// its mole fraction (H₂, He, CH₄ and H₂O; none for CO, Na and K, whose share is small).
#[must_use]
pub(crate) fn rayleigh_of(gas: &Absorbers) -> f64 {
    let of = |g: Gas| gas_cross_section(g).value();
    gas.hydrogen * of(Gas::Hydrogen)
        + gas.helium * of(Gas::Helium)
        + gas.methane * of(Gas::Methane)
        + gas.water * of(Gas::Water)
}

/// The molar masses of H₂, CH₄, H₂O and CO, g mol⁻¹: the registry's rows', CO's the sum of its
/// atoms' standard weights.
fn absorber_masses() -> [f64; 4] {
    let of = |g: Gas| {
        g.substance()
            .substance()
            .molar_mass_g_per_mol()
            .unwrap_or(0.0)
    };
    [
        of(Gas::Hydrogen),
        of(Gas::Methane),
        of(Gas::Water),
        Element::C.standard_atomic_weight_g_per_mol()
            + Element::O.standard_atomic_weight_g_per_mol(),
    ]
}

/// The index and weight of the node below `x` in the ascending `nodes`, held at the ends: (i, f)
/// with x = (1 − f) nodes\[i] + f nodes\[i + 1].
#[must_use]
fn bracket(nodes: &[f64], x: f64) -> (usize, f64) {
    let last = nodes.len() - 1;
    if x.is_nan() || x <= nodes[0] {
        return (0, 0.0);
    }
    if x >= nodes[last] {
        return (last - 1, 1.0);
    }
    let above = nodes.partition_point(|&node| node <= x).clamp(1, last);
    let (a, b) = (nodes[above - 1], nodes[above]);
    (above - 1, (x - a) / (b - a))
}

/// [`bracket`] in ln x over the ascending positive `nodes`: the weight is ln(x ÷ nodes\[i]) ÷
/// ln(nodes\[i + 1] ÷ nodes\[i]).
#[must_use]
fn log_bracket(nodes: &[f64], x: f64) -> (usize, f64) {
    let (i, f) = bracket(nodes, x);
    if f <= 0.0 || f >= 1.0 {
        return (i, f);
    }
    (
        i,
        math::ln(x / nodes[i]) / math::ln(nodes[i + 1] / nodes[i]),
    )
}

/// A line table's value at ln p and T (each a [`log_bracket`] or [`bracket`]): ln k bilinear
/// (the tables hold ln k, −700 for none).
#[must_use]
fn bilinear(table: &[[f64; 6]; 6], (i, fx): (usize, f64), (j, fy): (usize, f64)) -> f64 {
    let ln = (1.0 - fx) * ((1.0 - fy) * table[i][j] + fy * table[i][j + 1])
        + fx * ((1.0 - fy) * table[i + 1][j] + fy * table[i + 1][j + 1]);
    math::exp(ln)
}

/// A pair table's value at T (a [`bracket`]), linear.
#[must_use]
fn linear(row: &[f64; 6], (j, fy): (usize, f64)) -> f64 {
    row[j] * (1.0 - fy) + row[j + 1] * fy
}

/// What a layer's absorption reads in every bin, found once a layer: its brackets in the line
/// tables' pressure and temperature, each gas's mass column (kg m⁻²), H₂–H₂'s factor (q² ∫ ρ dp ÷
/// g) and the alkali pairs' (`x_A` `x_B` ∫ n dp ÷ (m̄ g)).
#[derive(Debug, Clone, Copy)]
struct LayerGas {
    p_at: (usize, f64),
    t_at: (usize, f64),
    lines: [f64; 3],
    hydrogen_pairs: f64,
    alkali_pairs: [f64; 4],
}

impl LayerGas {
    /// The layer `layer` of a column under `gravity` (m s⁻²) of mean molar mass `molar_mass` (g
    /// mol⁻¹), the absorbers' molar masses `masses` ([`absorber_masses`]).
    fn of(layer: &ColumnLayer, (gravity, molar_mass): (f64, f64), masses: &[f64; 4]) -> Self {
        let (top, bottom, t) = (layer.top, layer.bottom, layer.temperature);
        let middle = (top * bottom).sqrt();
        let mass_column = (bottom - top) / gravity;
        let gas = &layer.gas;
        let [m_h2, m_ch4, m_h2o, m_co] = *masses;
        // ∫ ρ dp over the layer, kg Pa m⁻³ (μ p ÷ R T at the layer's temperature), for the pairs
        // by mass density, and ∫ n dp ÷ (m̄ g) for the pairs by number density.
        let squares = bottom * bottom - top * top;
        let rho_dp = (molar_mass / 1_000.0) * squares / (2.0 * MOLAR_GAS_CONSTANT * t);
        let hydrogen_share = gas.hydrogen * m_h2 / molar_mass;
        let molecule = molar_mass / 1_000.0 * BOLTZMANN_CONSTANT / MOLAR_GAS_CONSTANT;
        let number_dp = squares / (2.0 * BOLTZMANN_CONSTANT * t * molecule * gravity);
        Self {
            p_at: log_bracket(&opacity::PRESSURES_PA, middle),
            t_at: bracket(&opacity::TEMPERATURES_K, t),
            lines: [
                gas.methane * m_ch4 / molar_mass * mass_column,
                gas.water * m_h2o / molar_mass * mass_column,
                gas.carbon_monoxide * m_co / molar_mass * mass_column,
            ],
            hydrogen_pairs: hydrogen_share * hydrogen_share * rho_dp / gravity,
            alkali_pairs: [
                gas.sodium * gas.hydrogen * number_dp,
                gas.sodium * gas.helium * number_dp,
                gas.potassium * gas.hydrogen * number_dp,
                gas.potassium * gas.helium * number_dp,
            ],
        }
    }

    /// The g-points' absorption optical depths in bin `bin`.
    fn absorption(&self, bin: usize) -> [f64; 8] {
        let lines = [
            &opacity::METHANE[bin],
            &opacity::WATER[bin],
            &opacity::CARBON_MONOXIDE[bin],
        ];
        let pairs = [
            &opacity::SODIUM_HYDROGEN[bin],
            &opacity::SODIUM_HELIUM[bin],
            &opacity::POTASSIUM_HYDROGEN[bin],
            &opacity::POTASSIUM_HELIUM[bin],
        ];
        core::array::from_fn(|g| {
            let mut tau = 0.0;
            for (table, column) in lines.iter().zip(self.lines) {
                if column > 0.0 {
                    tau += bilinear(&table[g], self.p_at, self.t_at) * column;
                }
            }
            if self.hydrogen_pairs > 0.0 {
                tau += linear(&opacity::HYDROGEN_PAIRS[bin][g], self.t_at) * self.hydrogen_pairs;
            }
            for (table, factor) in pairs.iter().zip(self.alkali_pairs) {
                if factor > 0.0 {
                    tau += linear(&table[g], self.t_at) * factor;
                }
            }
            tau
        })
    }
}

/// The diffuse reflectance and transmission of a homogeneous layer of optical depth `tau`,
/// single-scattering albedo `omega` and asymmetry `g`, over black and lit diffusely, by the δ-PIFM
/// closure, δ-scaled (Joseph, Wiscombe and Weinman 1976), its deep limit R = γ₂ ÷ (k + γ₁), T = 0
/// where k τ′ passes 300 (e^−600 of the light), which the hyperbolic functions would overflow.
///
/// The δ-PIFM is the practical improved flux method of Zdunkowski, Welch and Korb 1980 (Beitr.
/// Phys. Atmos. 53, 147–166), γ₁ = (8 − ω′(5 + 3g′)) ÷ 4, γ₂ = 3ω′(1 − g′) ÷ 4, as ECMWF's ecRad
/// writes them (`radiation_two_stream.F90`, citing that paper; Hogan and Bozzo 2018, J. Adv.
/// Model. Earth Syst. 10, 1990), read by the lane's science check (Räisänen 2002, Q. J. R.
/// Meteorol. Soc. 128, 2397, Table 1, recalled): never negative, the δ-Eddington's at ω′ = 1, and
/// within 10% of an exact diffuse reflectance where the δ-Eddington of `area_mix`'s haze layer
/// (Meador and Weaver 1980, Table 1) is not (the [module documentation](self), "The column read
/// whole").
#[must_use]
pub(crate) fn layer_two_stream(tau: f64, omega: f64, g: f64) -> (f64, f64) {
    let f = g * g;
    let scaled_tau = (1.0 - omega * f) * tau;
    let scaled_omega = (1.0 - f) * omega / (1.0 - omega * f);
    let scaled_g = g / (1.0 + g);
    let gamma_1 = (8.0 - scaled_omega * (5.0 + 3.0 * scaled_g)) / 4.0;
    let gamma_2 = 3.0 * scaled_omega * (1.0 - scaled_g) / 4.0;
    let square = gamma_1 * gamma_1 - gamma_2 * gamma_2;
    if square <= 1e-24 {
        let reflected = gamma_1 * scaled_tau / (1.0 + gamma_1 * scaled_tau);
        return (reflected, 1.0 - reflected);
    }
    let k = square.sqrt();
    let depth = k * scaled_tau;
    let (reflected, through) = if depth > 300.0 {
        (gamma_2 / (k + gamma_1), 0.0)
    } else {
        // sinh and cosh of one exp_m1, e^d − 1, which keeps sinh's precision as d → 0.
        let m = math::exp_m1(depth);
        let sinh = f64::midpoint(m, m / (1.0 + m));
        let cosh = sinh + 1.0 / (1.0 + m);
        let denominator = k * cosh + gamma_1 * sinh;
        (gamma_2 * sinh / denominator, k / denominator)
    };
    (if reflected > 0.0 { reflected } else { 0.0 }, through)
}

/// The spherical albedo of `column` over a black floor in each of the optics' bins (the
/// [module](self)'s engine), each layer by the δ-PIFM ([`layer_two_stream`]).
#[must_use]
pub(crate) fn bin_albedos(column: &EnvelopeColumn) -> [f64; OPTICS_BANDS] {
    let masses = absorber_masses();
    let air = (column.gravity, column.molar_mass_g_per_mol);
    let molecule = column.molar_mass_g_per_mol / 1_000.0 * BOLTZMANN_CONSTANT / MOLAR_GAS_CONSTANT;
    let rayleigh_per_pa = column.rayleigh_550_m2 / (molecule * column.gravity);
    let count = column.layers.len();
    let gases: Vec<LayerGas> = column
        .layers
        .iter()
        .map(|layer| LayerGas::of(layer, air, &masses))
        .collect();
    let mut absorption = vec![[0.0; 8]; count];
    let mut scattering = vec![[0.0; 3]; count];
    core::array::from_fn(|bin| {
        let factor = rayleigh_bin_factor(bin);
        for (i, layer) in column.layers.iter().enumerate() {
            absorption[i] = gases[i].absorption(bin);
            let rayleigh = rayleigh_per_pa * factor * (layer.bottom - layer.top);
            let [ext, sca, g_sca] = layer.particles[bin];
            scattering[i] = [rayleigh + ext, rayleigh + sca, g_sca];
        }
        // The bin's own floor (ColumnFloor::EachBin): the layer in which its Rayleigh and
        // particles' τ from the top reaches FLOOR_OPTICAL_DEPTH, and the share of that layer above
        // it; the whole column under ColumnFloor::Visual or where the bin's τ never reaches it.
        let (floor_layer, floor_share) = match column.floor {
            ColumnFloor::Visual => (count, 1.0),
            ColumnFloor::EachBin => bin_floor(&scattering),
        };
        let mut albedo = 0.0;
        for (g, weight) in opacity::G_WEIGHTS.iter().enumerate() {
            // The deepest layer the light returns from: where the absorption from the top passes
            // OPAQUE_ABSORPTION.
            let mut deepest = count;
            let mut absorbed = 0.0;
            for (i, (gas, [ext, sca, _])) in absorption.iter().zip(&scattering).enumerate() {
                absorbed += gas[g] + (ext - sca);
                if absorbed > OPAQUE_ABSORPTION {
                    deepest = i + 1;
                    break;
                }
            }
            // The bin's floor where it lies at or above that layer, its crossing layer cut.
            let share = if floor_layer < deepest {
                deepest = floor_layer + 1;
                floor_share
            } else {
                1.0
            };
            let mut below = 0.0;
            for i in (0..deepest).rev() {
                let [ext, sca, g_sca] = scattering[i];
                let gas = absorption[i][g];
                let (ext, sca, g_sca, gas) = if i == floor_layer {
                    (ext * share, sca * share, g_sca * share, gas * share)
                } else {
                    (ext, sca, g_sca, gas)
                };
                let tau = ext + gas;
                if tau <= 0.0 {
                    continue;
                }
                let omega = sca / tau;
                let asymmetry = if sca > 0.0 { g_sca / sca } else { 0.0 };
                let (reflected, through) = layer_two_stream(tau, omega, asymmetry);
                below = reflected + through * through * below / (1.0 - reflected * below);
            }
            albedo += weight * below;
        }
        albedo
    })
}

/// The layer of a column whose scattering by layer is `scattering` (its extinction first, the
/// bin's Rayleigh and particles') in which the extinction from the top reaches
/// [`FLOOR_OPTICAL_DEPTH`], and the share of that layer above the level where it does: its
/// layers' count and 1 where it never does.
#[must_use]
fn bin_floor(scattering: &[[f64; 3]]) -> (usize, f64) {
    let mut above = 0.0;
    for (i, [ext, _, _]) in scattering.iter().enumerate() {
        if above + ext >= FLOOR_OPTICAL_DEPTH {
            return (i, (FLOOR_OPTICAL_DEPTH - above) / ext);
        }
        above += ext;
    }
    (scattering.len(), 1.0)
}

/// The Bond albedo of `column` under the light `shade`: [`bin_albedos`] weighted by it (the tests'
/// reading of a column; the passes weigh [`column::snapshot_light`]'s bins).
#[cfg(test)]
#[must_use]
pub(crate) fn column_bond(column: &EnvelopeColumn, shade: &HostShade) -> f64 {
    band_weighted(&bin_albedos(column), shade.bands())
}

/// A particle material of [`opacity::PARTICLES`], by its row's key and phase: water's solid is its
/// own table (`H2O_ice`), every other row's one table serves both phases (the client's rule: a
/// liquid with no file of its own reads its solid's as spheres). `None` for a row with no table.
#[must_use]
pub(crate) fn particle_table(key: &str, phase: CondensedPhase) -> Option<usize> {
    let key = match (key, phase) {
        ("H2O", CondensedPhase::Solid) => "H2O_ice",
        (key, CondensedPhase::Solid | CondensedPhase::Liquid) => key,
    };
    opacity::PARTICLE_KEYS.iter().position(|k| *k == key)
}

/// A particle table the column reads (its Mie spheres' extinction over its 550 nm value, ω and g
/// by effective radius and bin): one of [`opacity::PARTICLES`]', over the decks' `σ_g` 2
/// log-normal, or one of [`narrow::NARROW_PARTICLES`]', over a gamma of `v_eff` 0.05
/// ([`table_for`]); each its index there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParticleTable {
    /// One of [`opacity::PARTICLES`]' ([`particle_table`]).
    Wide(usize),
    /// One of [`narrow::NARROW_PARTICLES`]'.
    Narrow(usize),
}

impl ParticleTable {
    /// Its effective radii, m: [`opacity::RADII_M`] or [`narrow::NARROW_RADII_M`].
    #[must_use]
    fn radii(self) -> &'static [f64] {
        match self {
            Self::Wide(_) => &opacity::RADII_M,
            Self::Narrow(_) => &narrow::NARROW_RADII_M,
        }
    }

    /// Its rows, by effective radius and bin.
    #[must_use]
    fn rows(self) -> &'static [[[f64; 3]; OPTICS_BANDS]] {
        match self {
            Self::Wide(table) => &opacity::PARTICLES[table],
            Self::Narrow(table) => &narrow::NARROW_PARTICLES[table],
        }
    }
}

/// The table a mode of material `table` ([`particle_table`]) whose sizes are `distribution` reads:
/// for a gamma distribution of `v_eff` [`narrow::NARROW_EFFECTIVE_VARIANCE`] of a material with a
/// narrow table ([`narrow::NARROW_KEYS`]) that one, else the table itself, at the decks' `σ_g` 2
/// log-normal.
///
/// The narrow tables are soot's and the bright haze's tholin (`reduce_narrow.py`; the lanes'
/// leans, 2026-11-10): the record draws Gao et al.'s soot as a gamma of `v_eff` 0.05, whose ω at
/// 50 nm is 0.16, 0.10 and 0.07 in B, V and R against the wide table's 0.22, 0.18 and 0.14, and
/// the tholin has no wide table. The cold giants' hazes, gammas too, keep the wide table, with
/// which their optical depths were fitted on the giants' measured albedos and colours (a finding).
#[must_use]
pub(crate) fn table_for(
    table: usize,
    distribution: &crate::substance::SizeDistribution,
) -> ParticleTable {
    let key = opacity::PARTICLE_KEYS.get(table).copied().unwrap_or("");
    narrow_table_for(key, distribution).unwrap_or(ParticleTable::Wide(table))
}

/// The narrow table a mode of the material keyed `key` whose sizes are `distribution` reads: for a
/// gamma distribution of `v_eff` [`narrow::NARROW_EFFECTIVE_VARIANCE`] of a material with one
/// ([`narrow::NARROW_KEYS`]); `None` otherwise. The bright haze's tholin ([`bright_haze`]) has a
/// narrow table and no wide one.
///
/// [`bright_haze`]: crate::planetary::chem::photochem::bright_haze
#[must_use]
pub(crate) fn narrow_table_for(
    key: &str,
    distribution: &crate::substance::SizeDistribution,
) -> Option<ParticleTable> {
    use crate::substance::SizeDistributionKind;
    match distribution.kind() {
        SizeDistributionKind::Gamma => {
            let width = distribution.effective_variance() - narrow::NARROW_EFFECTIVE_VARIANCE;
            narrow::NARROW_KEYS
                .iter()
                .position(|k| *k == key)
                .filter(|_| width.abs() < 1e-9)
                .map(ParticleTable::Narrow)
        }
        SizeDistributionKind::LogNormal => None,
    }
}

/// The extinction over its 550 nm value, ω and g of the particle table `table` ([`table_for`]) at
/// an effective radius bracketed in its radii ([`log_bracket`]) as `(i, f)` in bin `bin`: linear
/// in ln r between them, held at the ends.
#[must_use]
fn particle_optics_at(table: ParticleTable, (i, f): (usize, f64), bin: usize) -> [f64; 3] {
    let rows = table.rows();
    let (a, b) = (rows[i][bin], rows[i + 1][bin]);
    core::array::from_fn(|k| a[k] * (1.0 - f) + b[k] * f)
}

/// The split of R08's floor, τ(550) 10 from the top (decision-deep-decks-split §3.3): the column
/// is read above it alone.
pub(crate) const FLOOR_OPTICAL_DEPTH: f64 = 10.0;

/// The column's top, 1 Pa: P14.T52.d's deck column's ([`DECK_COLUMN_TOP`]).
///
/// [`DECK_COLUMN_TOP`]: crate::planetary::hooks::envelope_atmosphere::DECK_COLUMN_TOP
pub(crate) const COLUMN_TOP_PA: f64 = 1.0;

/// The deepest level the floor is sought at, 10⁹ Pa, the deck column's bottom.
const FLOOR_SEARCH_BOTTOM_PA: f64 = 1.0e9;

/// The bisections of the floor in ln p, 40: to 2 × 10⁻¹¹ in ln p.
const FLOOR_BISECTIONS: u32 = 40;

/// The widest layer, in ln p: two e-folds of pressure (within 2 × 10⁻⁴ of one e-fold's Bond albedo
/// on the table's giants and the hot fixture, the lane's check).
const LAYER_STEP_LN_P: f64 = 2.0;

/// The top of a column of `modes`, Pa ([`assemble`]): the highest top of a mode with a τ where that
/// lies above [`COLUMN_TOP_PA`], else [`COLUMN_TOP_PA`].
#[must_use]
fn column_top(modes: &[ColumnMode]) -> f64 {
    // Positive tops alone, so that no signed zero or NaN reaches the fold's `min` (portable across
    // targets) or the floor search's ln.
    modes
        .iter()
        .filter(|mode| mode.coverage > 0.0 && mode.tau > 0.0 && mode.top > 0.0)
        .map(|mode| mode.top)
        .fold(COLUMN_TOP_PA, f64::min)
}

/// One mode of a column as its assembly reads it: its τ(550) laid by its profile, (p ÷
/// `p_b`)^f between its base and top, at its coverage, and its particle table and effective radius
/// (`None`: counted in the floor's τ, but no optics).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ColumnMode {
    /// Its τ(550).
    pub(crate) tau: f64,
    /// Its base, Pa.
    pub(crate) base: f64,
    /// Its top, Pa.
    pub(crate) top: f64,
    /// Its profile's exponent f.
    pub(crate) exponent: f64,
    /// The share of the sky it covers.
    pub(crate) coverage: f64,
    /// Its particle table ([`table_for`]) and effective radius, m.
    pub(crate) optics: Option<(ParticleTable, f64)>,
}

impl ColumnMode {
    /// Its τ(550) above the level `p` (Pa): τ ((p ÷ `p_b`)^(f+1) − (`p_t` ÷ `p_b`)^(f+1)) ÷ (1 −
    /// (`p_t` ÷ `p_b`)^(f+1)), 0 above its top and all of it below its base, times its coverage.
    #[must_use]
    pub(crate) fn tau_above(&self, p: f64) -> f64 {
        let above = if p <= self.top {
            0.0
        } else if p >= self.base {
            self.tau
        } else {
            let edge = math::powf(self.top / self.base, self.exponent + 1.0);
            self.tau * (math::powf(p / self.base, self.exponent + 1.0) - edge) / (1.0 - edge)
        };
        above * self.coverage
    }
}

/// The level of a column's floor `floor` from its top `top` (Pa), its Rayleigh τ(550) per Pa
/// `rayleigh_per_pa` and its modes `modes` with their tables' brackets `brackets` ([`assemble`]):
/// by [`FLOOR_BISECTIONS`] bisections in ln p from the top to [`FLOOR_SEARCH_BOTTOM_PA`], the last
/// level where τ(550) from p = 0 ([`ColumnFloor::Visual`]) or the least of the bins' τ from the top
/// ([`ColumnFloor::EachBin`]) lies under [`FLOOR_OPTICAL_DEPTH`].
#[must_use]
fn floor_level(
    floor: ColumnFloor,
    (top, rayleigh_per_pa): (f64, f64),
    modes: &[ColumnMode],
    brackets: &[Option<(ParticleTable, (usize, f64))>],
) -> f64 {
    let total = |p: f64| {
        let mut tau = rayleigh_per_pa * p;
        for mode in modes {
            tau += mode.tau_above(p);
        }
        tau
    };
    // Under ColumnFloor::EachBin each mode's extinction in each bin over its 550 nm one (0 for a
    // mode with no optics, which the engine does not read) and its τ above the top, for the least
    // of the bins' τ above p as the engine reads them from the top; nothing under Visual.
    let by_bin: Vec<([f64; OPTICS_BANDS], f64)> = match floor {
        ColumnFloor::Visual => Vec::new(),
        ColumnFloor::EachBin => brackets
            .iter()
            .zip(modes)
            .map(|(bracket, mode)| {
                let extinction = core::array::from_fn(|bin| {
                    bracket.map_or(0.0, |(table, at)| particle_optics_at(table, at, bin)[0])
                });
                (extinction, mode.tau_above(top))
            })
            .collect(),
    };
    let least = |p: f64| {
        let below: Vec<f64> = modes
            .iter()
            .zip(&by_bin)
            .map(|(mode, (_, at_top))| mode.tau_above(p) - at_top)
            .collect();
        let mut least = f64::INFINITY;
        for bin in 0..OPTICS_BANDS {
            let mut tau = rayleigh_per_pa * rayleigh_bin_factor(bin) * (p - top);
            for (delta, (extinction, _)) in below.iter().zip(&by_bin) {
                tau += delta * extinction[bin];
            }
            if tau < least {
                least = tau;
            }
        }
        least
    };
    let (mut low, mut high) = (math::ln(top), math::ln(FLOOR_SEARCH_BOTTOM_PA));
    for _ in 0..FLOOR_BISECTIONS {
        let middle = f64::midpoint(low, high);
        let tau = match floor {
            ColumnFloor::Visual => total(math::exp(middle)),
            ColumnFloor::EachBin => least(math::exp(middle)),
        };
        if tau < FLOOR_OPTICAL_DEPTH {
            low = middle;
        } else {
            high = middle;
        }
    }
    math::exp(low)
}

/// The column of `modes` in a gas of mean molar mass `molar_mass` (g mol⁻¹) whose Rayleigh
/// scattering is that of `scattering` ([`rayleigh_of`]) under `gravity` (m s⁻²), its temperature
/// by pressure `temperature_at` and its absorbers by pressure and temperature `gas_at`, to the
/// floor `floor`, and the layers from the top down to it split at each mode's base, at e^−1 and
/// e^−3 of its column above it and at its top, none wider than [`LAYER_STEP_LN_P`]. The top is the
/// highest top of a mode with a τ where that lies above [`COLUMN_TOP_PA`], so that a haze there is
/// read, else [`COLUMN_TOP_PA`].
///
/// Under [`ColumnFloor::Visual`] the floor lies where τ(550) from the top, the gas's Rayleigh and
/// each mode's, reaches [`FLOOR_OPTICAL_DEPTH`] (by [`FLOOR_BISECTIONS`] bisections in ln p from
/// the top to 10⁹ Pa); under [`ColumnFloor::EachBin`] the column reaches down to where every bin's
/// own τ from the top, its Rayleigh and its particles' extinction, reaches [`FLOOR_OPTICAL_DEPTH`]
/// (by the same bisection, to 10⁹ Pa at most), and [`bin_albedos`] cuts each bin at its own.
#[must_use]
pub(crate) fn assemble(
    floor: ColumnFloor,
    (gravity, molar_mass): (f64, f64),
    scattering: &Absorbers,
    modes: &[ColumnMode],
    temperature_at: impl Fn(f64) -> f64,
    gas_at: impl Fn(f64, f64) -> Absorbers,
) -> EnvelopeColumn {
    let rayleigh = rayleigh_of(scattering);
    let molecule = molar_mass / 1_000.0 * BOLTZMANN_CONSTANT / MOLAR_GAS_CONSTANT;
    let rayleigh_per_pa = rayleigh / (molecule * gravity);
    let top = column_top(modes);
    let brackets: Vec<Option<(ParticleTable, (usize, f64))>> = modes
        .iter()
        .map(|mode| {
            mode.optics
                .map(|(table, radius)| (table, log_bracket(table.radii(), radius)))
        })
        .collect();
    let bottom = floor_level(floor, (top, rayleigh_per_pa), modes, &brackets);
    let mut points = vec![top, bottom];
    for mode in modes.iter().filter(|m| m.coverage > 0.0) {
        let f = mode.exponent + 1.0;
        for p in [
            mode.base,
            mode.base * math::exp(-1.0 / f),
            mode.base * math::exp(-3.0 / f),
            mode.top,
        ] {
            if top < p && p < bottom {
                points.push(p);
            }
        }
    }
    points.sort_by(f64::total_cmp);
    points.dedup();
    let mut levels = vec![points[0]];
    for &p in &points[1..] {
        let a = *levels.last().expect("the top is a level");
        let steps = (math::ln(p / a) / LAYER_STEP_LN_P).ceil();
        let mut k = 1.0;
        while k < steps {
            levels.push(a * math::powf(p / a, k / steps));
            k += 1.0;
        }
        levels.push(p);
    }
    let mut above: Vec<f64> = modes.iter().map(|mode| mode.tau_above(levels[0])).collect();
    let layers = levels
        .windows(2)
        .map(|pair| {
            let (top, bottom) = (pair[0], pair[1]);
            let temperature = temperature_at((top * bottom).sqrt());
            let mut taus = Vec::with_capacity(modes.len());
            for (mode, upper) in modes.iter().zip(above.iter_mut()) {
                let lower = mode.tau_above(bottom);
                taus.push(lower - *upper);
                *upper = lower;
            }
            let particles = core::array::from_fn(|bin| {
                let mut sums = [0.0; 3];
                for (tau, bracket) in taus.iter().zip(&brackets) {
                    let Some((table, at)) = bracket else {
                        continue;
                    };
                    if *tau > 0.0 {
                        let [q, w, g] = particle_optics_at(*table, *at, bin);
                        sums[0] += tau * q;
                        sums[1] += tau * q * w;
                        sums[2] += tau * q * w * g;
                    }
                }
                sums
            });
            ColumnLayer {
                top,
                bottom,
                temperature,
                gas: gas_at((top * bottom).sqrt(), temperature),
                particles,
            }
        })
        .collect();
    EnvelopeColumn {
        gravity,
        molar_mass_g_per_mol: molar_mass,
        rayleigh_550_m2: rayleigh,
        layers,
        floor,
    }
}

/// A gas envelope's Bond albedo under the light `shade` in the pass `inputs` describe, its colour
/// and its bright haze's share of its hazes' light: its own column's
/// ([`column::snapshot_light_under`]), T51.b's class table's ([`giant_bond`]) where the closed form
/// gives no column (no internal heat to join an adiabat, an envelope not hydrogen-dominated,
/// abundances refused) or its column no light (labelled).
///
/// The colour is its column's spherical albedo in the B, V and R bins
/// ([`ColourBand`](super::area_mix::ColourBand)); `None` for the class table, and where those
/// albedos are not a colour ([`DiscColour::new`]). The share is the bright haze's of its
/// photochemical hazes' scattering in the same bins where its column holds a bright haze, `None`
/// otherwise; the photometry weighs its phase integral by it.
///
/// [`giant_bond`]: super::area_mix::giant_bond
#[must_use]
pub(crate) fn gas_envelope_light(
    shade: &HostShade,
    inputs: &crate::planetary::derive::atmosphere::AtmosphereInputs,
) -> (f64, Option<DiscColour>, Option<BrightShare>) {
    use super::area_mix::ColourBand;
    match column::snapshot_light_under(column::ClosedReading::in_force(), inputs, shade) {
        Some(light) => {
            let in_bands = |bins: &[f64; OPTICS_BANDS]| Bands {
                b: bins[ColourBand::B.bin()],
                v: bins[ColourBand::V.bin()],
                r: bins[ColourBand::R.bin()],
            };
            (
                light.bond,
                DiscColour::new(in_bands(&light.bins)),
                BrightShare::new(in_bands(&light.bright_share)),
            )
        }
        None => (
            super::area_mix::giant_bond(shade, inputs.heated, inputs.class),
            None,
            None,
        ),
    }
}
