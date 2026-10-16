//! The condensate partition at the ground (plan 14, P14.T51.a; decision-composition §1.4): how much
//! of each condensable a body's air holds over its climate, how much lies on its surface, and in
//! which phase.
//!
//! The partition reads a body's exchangeable inventory after escape, by species, as column weights
//! M g ÷ 4πR² ([`ColumnInventory`]), and its climate's span, the coldest zone, the mean and the
//! warmest zone ([`ClimateSpan`]). For each species, in registry order:
//!
//! - **A species with no phase diagram** never condenses: all of it is airborne.
//! - **A condensable's vapour** is capped on its partial pressure in the air, x p ≤ h `p_sat`(T),
//!   with x its mole fraction and p the summed weights, never on its column weight (signoff-3.md,
//!   items 1.7 and F-g): for water in nitrogen the two differ by a factor of 1.6.
//!   - A **minor** condensable takes the mean temperature and its surface humidity h
//!     ([`surface_humidity`]), its vapour held locally by its background over a reservoir that
//!     lies across the surface, as Earth's water is.
//!   - A **dominant** one takes its saturation pressure at the coldest zone, h = 1, where its
//!     reservoir cold-traps: its pressure is set globally there, the collapse of Wordsworth (2015,
//!     ApJ 806, 180, as P14.T48.d cites it), as Mars's carbon dioxide and Pluto's and Triton's
//!     nitrogen are held.
//!   - Between them the change is continuous in the condensable's share of the air
//!     ([`dominance`]): its controlling temperature moves from the mean to the coldest zone, and its
//!     humidity from h to 1, as the weight d rises from 0 to 1.
//!   - Its airborne weight is the smaller of its inventory's and the weight that meets the cap.
//!     The species are solved in turn, in registry order, each against the others' weights, a
//!     fixed number of times from a deterministic start (decision-composition §1.3): each
//!     condensable starts from the least of its inventory and its two caps.
//! - **The condensed reservoir** is the rest, (M − `M_air`) ÷ 4πR², a column mass ([`Condensate`]).
//! - **Its phase where it lies**: the registry's phase diagram
//!   ([`PhaseDiagram::phase_at`](crate::substance::diagram::PhaseDiagram::phase_at)) at the
//!   temperature its reservoir lies at, the controlling temperature above, and the surface
//!   pressure: solid below the melting line, liquid inside its window, and **supercritical** where a
//!   species' own partial pressure and its temperature both exceed its critical point, which has no
//!   interface (Venus's carbon dioxide).
//! - **Seasonal** where part of it comes and goes over the year: where its range crosses its
//!   melting line, where the warmest zone takes its phase off the surface at the surface pressure
//!   (a liquid that boils), or, for a condensable held toward its cold trap, where the warmest zone
//!   rises above its frost point at its own partial pressure (Mars's carbon dioxide).
//!
//! # What this module is, and is not, yet
//!
//! It is P14.T51.a's first part, the partition's pressures, reservoirs and phases: a pure module
//! that nothing on the record calls. Its call into T13's passes is `atmosphere_under` in
//! [`atmosphere`](crate::planetary::derive::atmosphere), held at generator version 22 by the
//! `p14-snapshot` lane's hold (`SNAPSHOT_VERSION`). From that version its rules and constants are
//! output, and a change to any of them a generator-version change. Left to T51.a's second part and
//! to P14.T54.a:
//!
//! - the passes' climate, a closed form of T24.a's range from the body's spin (`derive_body` has
//!   none), which the passes take as a function of each step's column;
//! - the published `Condensates` (at most eight, one entry for each substance and phase), their
//!   area fractions by T24.b's cover, the split of a reservoir between its phases over the range,
//!   each reservoir's global equivalent layer over the registry's grain density, T24.a's
//!   liquid-water flag and P14.T48.d's condensable read from them;
//! - a solution's first melt and composition (Titan's seas: liquid down to the methane–ethane
//!   eutectic, 71.15 K, decision-p14-t49b-condensed §2.4), from the registry's condensed columns:
//!   every condensable is taken here as its pure substance.
//!
//! Every saturation pressure is the registry's phase diagram's
//! ([`diagram::saturation_pressure`](crate::substance::diagram::saturation_pressure), P14.T49.b's
//! fitted curves), not the held Clausius–Clapeyron sets that T13.c's greenhouse reads at 22. The
//! frost and boiling points are compared through the saturation pressure, which rises monotonically,
//! so the partition never inverts a curve (`saturation_temperature`'s 64 bisections are not on the
//! snapshot path).

use std::cmp::Ordering;
use std::error::Error;
use std::fmt;

use crate::math;
use crate::planetary::derive::atmosphere::{Gas, METHANE_RELATIVE_HUMIDITY};
use crate::substance::diagram::PhaseDiagram;
use crate::substance::{Phase, SubstanceId};
use crate::units::{Kelvin, KilogramsPerSquareMetre, MetresPerSecondSquared, Pascals};

/// The relative humidity of water vapour at the surface over a reservoir that lies across it: 0.77.
///
/// It is Manabe and Wetherald's (1967, J. Atmos. Sci. 24, 241, read; p. 243, below their eq. 2:
/// "h* is the relative humidity at the earth's surface, equal to 0.77"), the surface value of their
/// fixed relative-humidity profile fitted to Telegadas and London's (1954) hemispheric means. Their
/// humidity is on the partial pressure, the vapour's h `e_s`(T) (their Fig. 2's mixing ratio
/// 0.622 h `e_s` ÷ (p − h `e_s`)), as the partition's cap is (P14.T24.c's surface humidity for
/// water; plan 14, the constant T51.a's partition reads).
pub const WATER_SURFACE_HUMIDITY: f64 = 0.77;

/// The share of the air below which a condensable is wholly minor, its dominance 0: 0.1 (the
/// lane's, provisional).
///
/// A condensable at under a tenth of the air is dilute: its condensation moves under a tenth of the
/// air's mass, and its background carries its vapour locally. Earth's water, at about 1.3% of its
/// surface air, and Titan's methane, at 5.65 ± 0.18% (Niemann et al. 2010, JGR 115, E12006,
/// abstract, as [`METHANE_RELATIVE_HUMIDITY`] cites it), lie below it, so Titan stays on the minor
/// branch, as plan 14's test requires.
pub const DOMINANCE_ONSET: f64 = 0.1;

/// The share of the air above which a condensable is wholly dominant, its dominance 1: 0.9 (the
/// lane's, provisional).
///
/// Mars's carbon dioxide, 95.1% of its air by volume (NASA's Mars fact sheet), lies above it, and is
/// held at its cold trap's saturation pressure as its seasonal caps hold it; so is the nitrogen of
/// Triton, with a few parts in ten thousand of methane and carbon monoxide (Lellouch et al. 2010,
/// A&A 512, L8, read), and of Pluto, with 0.5 ± 0.1% of methane in 2008 (Lellouch et al. 2015,
/// Icarus 246, 268, read).
pub const DOMINANCE_FULL: f64 = 0.9;

/// How many times the condensables are solved in turn, each against the others' weights: 8, a
/// fixed count from a deterministic start (decision-composition §1.3's rule for every solver).
///
/// From each condensable's least cap, one condensable over a background that does not condense is
/// solved exactly in the first sweep, and a minor one in a condensing background meets its cap to
/// 10⁻⁹ within a few. Two condensables that both near dominance in one air (nitrogen with krypton
/// at 60–95 K, the slowest the tests meet) gain a factor of about 3 a sweep, and leave every
/// species of that air, a minor one's too, within 10⁻⁴ of its cap (6.7 × 10⁻⁵ at worst): far
/// inside the model's physics, and stated in the tests.
pub(crate) const PARTITION_SWEEPS: u32 = 8;

/// The span of a body's climate the partition reads: its coldest zone, its mean surface
/// temperature and its warmest zone, each orbit-averaged.
///
/// They are P14.T24.a's [`climate_range`](crate::planetary::hooks::surface::SurfaceConditions::climate_range),
/// a half-day mean at each end, with its mean between.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct ClimateSpan {
    coldest: Kelvin,
    mean: Kelvin,
    warmest: Kelvin,
}

/// Why a [`ClimateSpan`] was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BuildClimateSpanError {
    /// A temperature is negative, infinite or not a number.
    NotATemperature,
    /// The coldest zone is warmer than the mean, or the mean than the warmest zone.
    OutOfOrder,
}

impl fmt::Display for BuildClimateSpanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NotATemperature => {
                "a climate's temperature is negative, infinite or not a number"
            }
            Self::OutOfOrder => "a climate's coldest zone, mean and warmest zone are out of order",
        })
    }
}

impl Error for BuildClimateSpanError {}

impl ClimateSpan {
    /// The span from `coldest` through `mean` to `warmest`, each finite and not negative, in K; a
    /// −0 is written +0.
    ///
    /// # Errors
    ///
    /// - [`BuildClimateSpanError::NotATemperature`] for a temperature that is negative, infinite
    ///   or not a number.
    /// - [`BuildClimateSpanError::OutOfOrder`] unless coldest ≤ mean ≤ warmest.
    pub fn new(
        coldest: Kelvin,
        mean: Kelvin,
        warmest: Kelvin,
    ) -> Result<Self, BuildClimateSpanError> {
        let temperature = |t: Kelvin| t.value().is_finite() && t.value() >= 0.0;
        if !(temperature(coldest) && temperature(mean) && temperature(warmest)) {
            return Err(BuildClimateSpanError::NotATemperature);
        }
        if coldest > mean || mean > warmest {
            return Err(BuildClimateSpanError::OutOfOrder);
        }
        let positive = |t: Kelvin| Kelvin::new(t.value() + 0.0);
        Ok(Self {
            coldest: positive(coldest),
            mean: positive(mean),
            warmest: positive(warmest),
        })
    }

    /// The span of a climate with no contrast, all at `temperature`: a body whose climate's range is
    /// not known, on which a dominant condensable is held at the mean.
    ///
    /// A temperature that is negative or not a number is taken as 0 K, at which no condensable's
    /// vapour is airborne, and an infinite one as the largest finite temperature, above every
    /// critical point, at which every condensable is: the span is always one
    /// [`new`](Self::new) accepts.
    #[must_use]
    pub fn isothermal(temperature: Kelvin) -> Self {
        let t = temperature.value();
        let t = if t.is_nan() || t <= 0.0 {
            0.0
        } else if t.is_infinite() {
            f64::MAX
        } else {
            t
        };
        let at = Kelvin::new(t);
        Self {
            coldest: at,
            mean: at,
            warmest: at,
        }
    }

    /// The coldest zone, where a dominant condensable's reservoir cold-traps.
    #[must_use]
    pub const fn coldest(&self) -> Kelvin {
        self.coldest
    }

    /// The mean surface temperature, at which a minor condensable's vapour is held.
    #[must_use]
    pub const fn mean(&self) -> Kelvin {
        self.mean
    }

    /// The warmest zone, which sets whether a condensate is seasonal.
    #[must_use]
    pub const fn warmest(&self) -> Kelvin {
        self.warmest
    }

    /// The temperature that controls a condensable of dominance `dominance`, d in [0, 1]:
    /// `T_mean` − d (`T_mean` − `T_cold`), the mean for a minor condensable and the coldest zone for a
    /// dominant one.
    #[must_use]
    fn controlling(&self, dominance: f64) -> Kelvin {
        let (cold, mean) = (self.coldest.value(), self.mean.value());
        if dominance <= 0.0 {
            self.mean
        } else if dominance >= 1.0 {
            self.coldest
        } else {
            Kelvin::new(mean - dominance * (mean - cold))
        }
    }
}

/// The relative humidity at which a minor condensable's vapour is held at the surface, over its
/// saturation pressure at the mean temperature.
///
/// It is P14.T24.c's surface humidity, which the partition reads (plan 14):
///
/// - water, [`WATER_SURFACE_HUMIDITY`], 0.77 (Manabe and Wetherald 1967);
/// - methane, [`METHANE_RELATIVE_HUMIDITY`], 0.5 (Niemann et al. 2010, over Titan's liquid, and
///   taken over the solid's curve below its triple point, science-p14-t24f §1);
/// - every other condensable 1, saturation: an upper bound, the humidity the vapour reaches at
///   its lifting condensation level, and P14.T13.c's convention for every condensable it holds.
///   Both measured cases lie below it, since subsiding air dries any condensable's boundary layer,
///   so it overstates the vapour of every other minor condensable; none other has been measured
///   (the lane's, provisional, a closed set for the composition audit).
#[must_use]
pub fn surface_humidity(substance: SubstanceId) -> f64 {
    match substance.gas() {
        Some(Gas::Water) => WATER_SURFACE_HUMIDITY,
        Some(Gas::Methane) => METHANE_RELATIVE_HUMIDITY,
        Some(
            Gas::Hydrogen
            | Gas::Helium
            | Gas::Ammonia
            | Gas::Nitrogen
            | Gas::Oxygen
            | Gas::CarbonDioxide
            | Gas::Argon,
        )
        | None => 1.0,
    }
}

/// The dominance d of a condensable whose share of the air, its mole fraction on the minor branch,
/// is `share`: 0 up to [`DOMINANCE_ONSET`], 1 from [`DOMINANCE_FULL`], and a smooth step between.
///
/// The step is 3s² − 2s³ in s = (x − 0.1) ÷ 0.8, continuous with its slope at both ends (the
/// lane's, provisional; plan 14's "a weight the builder states and cites, not a switch").
///
/// It stands for the change from a dilute condensable, whose vapour its background holds locally,
/// to a non-dilute one, which sets its own air's pressure at its cold trap (Pierrehumbert and Ding
/// 2016, Proc. R. Soc. A 472, 20160107, arXiv:1608.03294, abstract read: non-dilute atmospheres
/// "have weak horizontal temperature gradients", and "the relative humidity of the condensible
/// component tends towards 100%" as they become non-dilute; Ding and Pierrehumbert 2016, ApJ 822,
/// 24, abstract read).
///
/// The share is the condensable's mole fraction where its vapour is held on the minor branch, a
/// closed form of the inventory and the climate, so that the weight needs no iteration of its own.
/// It is the share P14.T24.a's greenhouse gain already weights a condensable's own feedback by,
/// 1 − x, there to take a dominant condensable's cold-trapped pressure out of its local feedback
/// (science-p14-t24a-climate.md §1). The ramp's ends are set by the bodies it must hold apart:
/// Titan's 5.65% of methane wholly minor and Mars's 95.1% of carbon dioxide wholly dominant
/// ([`DOMINANCE_ONSET`], [`DOMINANCE_FULL`]).
///
/// A share that is not a number is taken as 0.
///
/// # Examples
///
/// ```
/// use hyperion_sim::planetary::chem::partition::dominance;
///
/// assert_eq!(dominance(0.0565), 0.0);
/// assert_eq!(dominance(0.951), 1.0);
/// assert!((dominance(0.5) - 0.5).abs() < 1e-12);
/// ```
#[must_use]
pub fn dominance(share: f64) -> f64 {
    let s = (share - DOMINANCE_ONSET) / (DOMINANCE_FULL - DOMINANCE_ONSET);
    if s.is_nan() || s <= 0.0 {
        0.0
    } else if s >= 1.0 {
        1.0
    } else {
        s * s * (3.0 - 2.0 * s)
    }
}

/// The column weight w, Pa, of a species of molar mass `molar_mass_g_per_mol` whose partial
/// pressure in the air meets `cap`, among other species weighing `others` in all, whose weights
/// over molar masses sum to `others_pa_mol_per_g` (Pa mol g⁻¹).
///
/// It solves x (w + W) = cap with x = (w ÷ μ) ÷ (w ÷ μ + Σ), so w² + b w − c = 0 with b = W − cap
/// and c = cap μ Σ (signoff-3.md, item 1.7; P14.T24.f's form for methane, for every condensable).
/// The positive root is taken as 2c ÷ (b + √(b² + 4c)) for b ≥ 0 and (√(b² + 4c) − b) ÷ 2 below, so
/// that neither cancels. An infinite cap, above the critical temperature, gives an infinite weight,
/// and one that is not positive none. The molar mass is positive and the others' sums are not
/// negative.
#[must_use]
pub(crate) fn weight_meeting_cap(
    cap: Pascals,
    others: Pascals,
    others_pa_mol_per_g: f64,
    molar_mass_g_per_mol: f64,
) -> Pascals {
    let cap = cap.value();
    if cap.is_nan() || cap <= 0.0 {
        return Pascals::ZERO;
    }
    if cap.is_infinite() {
        return Pascals::new(f64::INFINITY);
    }
    let b = others.value() - cap;
    let c = cap * molar_mass_g_per_mol * others_pa_mol_per_g;
    let root = if b >= 0.0 {
        if c > 0.0 {
            2.0 * c / (b + (b * b + 4.0 * c).sqrt())
        } else {
            0.0
        }
    } else {
        ((b * b + 4.0 * c).sqrt() - b) / 2.0
    };
    Pascals::new(root)
}

/// A body's exchangeable inventory after escape, by species, as the partition reads it: each
/// species' column weight M g ÷ 4πR², in registry order.
///
/// Every species is a gas row of the registry, one with gas columns and a molar mass, named once,
/// of a finite weight that is not negative.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ColumnInventory {
    species: Vec<Species>,
}

/// One species of a [`ColumnInventory`], with what the partition reads of its row once.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Species {
    substance: SubstanceId,
    weight: Pascals,
    molar_mass: f64,
    diagram: Option<&'static PhaseDiagram>,
    humidity: f64,
}

/// Why a [`ColumnInventory`] was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BuildColumnInventoryError {
    /// The row is no gas: it has no gas columns, or no molar mass.
    NotAGas(SubstanceId),
    /// The row is named twice.
    Repeated(SubstanceId),
    /// Its weight is negative, infinite or not a number.
    NotAWeight(SubstanceId),
}

impl fmt::Display for BuildColumnInventoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (what, id) = match self {
            Self::NotAGas(id) => ("is no gas, with no gas columns or no molar mass", id),
            Self::Repeated(id) => ("is named twice", id),
            Self::NotAWeight(id) => ("weighs a negative, infinite or not-a-number amount", id),
        };
        write!(f, "an inventory's {} {what}", id.substance().key().as_str())
    }
}

impl Error for BuildColumnInventoryError {}

impl ColumnInventory {
    /// The inventory of the species and column weights `entries`, taken in registry order whatever
    /// their order here.
    ///
    /// # Errors
    ///
    /// - [`BuildColumnInventoryError::NotAGas`] for a row with no gas columns or no molar mass.
    /// - [`BuildColumnInventoryError::Repeated`] for a row named twice.
    /// - [`BuildColumnInventoryError::NotAWeight`] for a weight that is negative, infinite or not
    ///   a number.
    pub fn new(
        entries: impl IntoIterator<Item = (SubstanceId, Pascals)>,
    ) -> Result<Self, BuildColumnInventoryError> {
        let mut species = Vec::new();
        for (substance, weight) in entries {
            let row = substance.substance();
            let molar_mass = row
                .molar_mass_g_per_mol()
                .filter(|_| row.gas().is_some())
                .ok_or(BuildColumnInventoryError::NotAGas(substance))?;
            let w = weight.value();
            if !(w.is_finite() && w >= 0.0) {
                return Err(BuildColumnInventoryError::NotAWeight(substance));
            }
            species.push(Species {
                substance,
                // A −0 weighs nothing, and is written so.
                weight: Pascals::new(w + 0.0),
                molar_mass,
                diagram: row.phase_diagram(),
                humidity: surface_humidity(substance),
            });
        }
        species.sort_by_key(|s| s.substance);
        if let Some(pair) = species
            .windows(2)
            .find(|pair| pair[0].substance == pair[1].substance)
        {
            return Err(BuildColumnInventoryError::Repeated(pair[0].substance));
        }
        Ok(Self { species })
    }

    /// The species and their column weights, in registry order.
    #[must_use]
    pub fn species(&self) -> impl ExactSizeIterator<Item = (SubstanceId, Pascals)> + '_ {
        self.species.iter().map(|s| (s.substance, s.weight))
    }

    /// How many species it names.
    #[must_use]
    pub fn len(&self) -> usize {
        self.species.len()
    }

    /// Whether it names none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.species.is_empty()
    }
}

/// What the sweeps keep of one condensable over one partition: its saturation pressures at the
/// climate's two ends, computed once, and its dominance as last solved.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Caps {
    /// h `p_sat`(`T_mean`), the minor branch's cap.
    minor_cap: Pascals,
    /// `p_sat`(`T_cold`), the dominant branch's.
    cold_cap: Pascals,
    dominance: f64,
}

/// The weights and weights over molar mass of every species but `skip`, summed in registry order.
#[must_use]
fn others(inventory: &ColumnInventory, weights: &[Pascals], skip: usize) -> (Pascals, f64) {
    let (mut total, mut moles) = (0.0, 0.0);
    for (k, (species, w)) in inventory.species.iter().zip(weights).enumerate() {
        if k != skip {
            total += w.value();
            moles += w.value() / species.molar_mass;
        }
    }
    (Pascals::new(total), moles)
}

/// The mole fraction of a species of weight `weight` and molar mass `molar_mass` among others whose
/// weights over molar mass sum to `others_moles`; 0 where the air holds nothing.
#[must_use]
fn share(weight: Pascals, molar_mass: f64, others_moles: f64) -> f64 {
    let own = weight.value() / molar_mass;
    let all = own + others_moles;
    if all > 0.0 { own / all } else { 0.0 }
}

/// The partition's airborne weights over one inventory, with what each condensable's sweeps keep.
///
/// A caller that solves the same inventory at many temperatures (T13.c's greenhouse steps) keeps
/// one, so that no step allocates ([`AirborneWeights::solve`]). Each solve starts afresh from its
/// inputs alone: nothing of an earlier solve is read.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct AirborneWeights {
    weights: Vec<Pascals>,
    caps: Vec<Option<Caps>>,
}

impl AirborneWeights {
    /// Empty weights, to be solved.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// The airborne weight of each species, in the inventory's order (registry order), as last
    /// solved; empty before the first.
    #[must_use]
    pub(crate) fn weights(&self) -> &[Pascals] {
        &self.weights
    }

    /// Solves the airborne weight of each species of `inventory` over the climate `climate`, the
    /// partition's pressures alone ([the module](self)'s rules), for T13.c's passes, which read
    /// them at every step of their greenhouse ([`partition`] gives the reservoirs and phases).
    ///
    /// Each condensable starts at the least of its inventory and its two caps, and
    /// [`PARTITION_SWEEPS`] sweeps run in registry order.
    pub(crate) fn solve(&mut self, climate: &ClimateSpan, inventory: &ColumnInventory) {
        self.solve_sweeping(climate, inventory, PARTITION_SWEEPS);
    }

    /// [`solve`](Self::solve) with `sweeps` sweeps, for the tests that compare the count with
    /// more.
    fn solve_sweeping(&mut self, climate: &ClimateSpan, inventory: &ColumnInventory, sweeps: u32) {
        let Self { weights, caps } = self;
        weights.clear();
        caps.clear();
        for species in &inventory.species {
            let held = match species.diagram {
                Some(diagram) if species.weight.value() > 0.0 => Some(Caps {
                    minor_cap: Pascals::new(
                        species.humidity * diagram.saturation_pressure(climate.mean).value(),
                    ),
                    cold_cap: diagram.saturation_pressure(climate.coldest),
                    dominance: 0.0,
                }),
                Some(_) | None => None,
            };
            // Each condensable starts at the least its caps allow, near where a dominant one ends,
            // so that one which collapses by orders of magnitude from its inventory misleads no
            // sweep.
            let start = held.map_or(species.weight, |held| {
                let least = if held.cold_cap < held.minor_cap {
                    held.cold_cap
                } else {
                    held.minor_cap
                };
                if least < species.weight {
                    least
                } else {
                    species.weight
                }
            });
            weights.push(start);
            caps.push(held);
        }
        for _ in 0..sweeps {
            for (k, species) in inventory.species.iter().enumerate() {
                let (Some(held), Some(diagram)) = (caps[k], species.diagram) else {
                    continue;
                };
                let (total, moles) = others(inventory, weights, k);
                let at_cap = |cap: Pascals| {
                    let w = weight_meeting_cap(cap, total, moles, species.molar_mass);
                    if w < species.weight {
                        w
                    } else {
                        species.weight
                    }
                };
                let minor = at_cap(held.minor_cap);
                let d = dominance(share(minor, species.molar_mass, moles));
                let cap = if d <= 0.0 {
                    held.minor_cap
                } else if d >= 1.0 {
                    held.cold_cap
                } else {
                    let humidity = math::powf(species.humidity, 1.0 - d);
                    let at = diagram.saturation_pressure(climate.controlling(d));
                    Pascals::new(humidity * at.value())
                };
                weights[k] = if d <= 0.0 { minor } else { at_cap(cap) };
                caps[k] = Some(Caps {
                    dominance: d,
                    ..held
                });
            }
        }
    }
}

/// The phase a condensate lies in (P14.T51.a): never the vapour.
///
/// It is distinct from the registry's [`CondensedPhase`](crate::substance::condensed::CondensedPhase),
/// what a grain density is the density of, by its supercritical fluid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CondensatePhase {
    /// Solid: below its melting line, an ice or a frost.
    Solid,
    /// Liquid, inside its window at the surface pressure.
    Liquid,
    /// Supercritical: its own partial pressure and its temperature above its critical point, with
    /// no interface.
    Supercritical,
}

impl From<CondensatePhase> for Phase {
    fn from(phase: CondensatePhase) -> Self {
        match phase {
            CondensatePhase::Solid => Self::Solid,
            CondensatePhase::Liquid => Self::Liquid,
            CondensatePhase::Supercritical => Self::Supercritical,
        }
    }
}

impl CondensatePhase {
    /// The condensate phase `phase` names, or `None` for the vapour.
    #[must_use]
    pub const fn of(phase: Phase) -> Option<Self> {
        match phase {
            Phase::Solid => Some(Self::Solid),
            Phase::Liquid => Some(Self::Liquid),
            Phase::Supercritical => Some(Self::Supercritical),
            Phase::Vapour => None,
        }
    }

    /// The bit of a [`CondensatePhases`] it takes.
    #[must_use]
    const fn bit(self) -> u8 {
        match self {
            Self::Solid => 1,
            Self::Liquid => 2,
            Self::Supercritical => 4,
        }
    }
}

/// A set of [`CondensatePhase`]s: those a condensate's reservoir takes over its climate's range.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CondensatePhases(u8);

impl CondensatePhases {
    /// Whether the set holds `phase`.
    #[must_use]
    pub const fn contains(self, phase: CondensatePhase) -> bool {
        self.0 & phase.bit() != 0
    }

    /// The set with `phase` added.
    #[must_use]
    const fn with(self, phase: CondensatePhase) -> Self {
        Self(self.0 | phase.bit())
    }
}

/// One species of a body's air as the partition leaves it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Partitioned {
    substance: SubstanceId,
    inventory: Pascals,
    airborne: Pascals,
    partial_pressure: Pascals,
    dominance: f64,
    lies_at: Kelvin,
}

impl Partitioned {
    /// The species.
    #[must_use]
    pub const fn substance(&self) -> SubstanceId {
        self.substance
    }

    /// Its inventory's column weight, M g ÷ 4πR².
    #[must_use]
    pub const fn inventory(&self) -> Pascals {
        self.inventory
    }

    /// Its airborne column weight, `M_air` g ÷ 4πR²: its share of the surface pressure as P14.T13.c
    /// sums it.
    #[must_use]
    pub const fn airborne(&self) -> Pascals {
        self.airborne
    }

    /// The weight of its condensed reservoir, (M − `M_air`) g ÷ 4πR².
    #[must_use]
    pub fn condensed(&self) -> Pascals {
        self.inventory - self.airborne
    }

    /// Its partial pressure in the air, x p, with x its mole fraction and p the surface pressure.
    #[must_use]
    pub const fn partial_pressure(&self) -> Pascals {
        self.partial_pressure
    }

    /// Its dominance d ([`dominance`]): 0 for a minor condensable and for a species with no phase
    /// diagram, 1 for one held at its cold trap.
    #[must_use]
    pub const fn dominance(&self) -> f64 {
        self.dominance
    }

    /// The temperature that controls its vapour and at which its reservoir lies: the mean for a
    /// minor species, the coldest zone for a dominant one.
    #[must_use]
    pub const fn lies_at(&self) -> Kelvin {
        self.lies_at
    }
}

/// One condensate at a body's surface (P14.T51.a): a condensed reservoir, or a supercritical fluid
/// with none.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Condensate {
    substance: SubstanceId,
    phase: CondensatePhase,
    reservoir: KilogramsPerSquareMetre,
    lies_at: Kelvin,
    seasonal: bool,
    phases: CondensatePhases,
}

impl Condensate {
    /// The substance.
    #[must_use]
    pub const fn substance(&self) -> SubstanceId {
        self.substance
    }

    /// Its phase where it lies.
    #[must_use]
    pub const fn phase(&self) -> CondensatePhase {
        self.phase
    }

    /// Its condensed reservoir, M − `M_air` over the body's whole surface: a column mass, finite and
    /// not negative.
    ///
    /// A supercritical fluid's is none, since the fluid is the air at the surface. Its global
    /// equivalent layer is this over the condensate's grain density, the registry's
    /// [`Substance::grain_density`](crate::substance::Substance::grain_density) (T51.a's second
    /// part).
    #[must_use]
    pub const fn reservoir(&self) -> KilogramsPerSquareMetre {
        self.reservoir
    }

    /// The temperature it lies at, as [`Partitioned::lies_at`].
    #[must_use]
    pub const fn lies_at(&self) -> Kelvin {
        self.lies_at
    }

    /// Whether it is seasonal, part of it coming and going over the year ([the module](self)'s
    /// rule): its range crosses its melting line, its warmest zone takes its phase off the surface,
    /// or, held toward its cold trap, its warmest zone rises above its frost point.
    #[must_use]
    pub const fn is_seasonal(&self) -> bool {
        self.seasonal
    }

    /// The phases its reservoir takes over the climate's range, at the surface pressure.
    ///
    /// It holds its own phase, with the solid where the coldest zone freezes a liquid (Earth's ice),
    /// or the liquid where the warmest zone melts a solid. A reservoir is not yet divided between
    /// them (T51.a's second part, by T24.b's cover).
    #[must_use]
    pub const fn phases(&self) -> CondensatePhases {
        self.phases
    }
}

/// A body's condensate partition (P14.T51.a; [`partition`]).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Partition {
    species: Vec<Partitioned>,
    condensates: Vec<Condensate>,
    surface_pressure: Pascals,
}

impl Partition {
    /// Every species of the inventory, in registry order.
    #[must_use]
    pub fn species(&self) -> &[Partitioned] {
        &self.species
    }

    /// The species `substance`, if the inventory names it.
    #[must_use]
    pub fn of(&self, substance: SubstanceId) -> Option<&Partitioned> {
        self.species.iter().find(|s| s.substance == substance)
    }

    /// The surface pressure, the airborne weights summed in registry order.
    #[must_use]
    pub const fn surface_pressure(&self) -> Pascals {
        self.surface_pressure
    }

    /// The condensates, by reservoir mass, largest first, with ties by registry order: every one,
    /// one for each substance, which T51.a's second part publishes as at most eight entries, one
    /// for each substance and phase.
    #[must_use]
    pub fn condensates(&self) -> &[Condensate] {
        &self.condensates
    }

    /// The condensate of `substance`, if it has one.
    #[must_use]
    pub fn condensate(&self, substance: SubstanceId) -> Option<&Condensate> {
        self.condensates.iter().find(|c| c.substance == substance)
    }
}

/// The condensate phase of a reservoir of the substance of `diagram`, whose partial pressure in the
/// air is `partial`, at `temperature` under the surface pressure `pressure`, or `None` where it
/// cannot lie condensed there.
///
/// At or above its critical temperature it is supercritical where its own partial pressure is at
/// least its critical pressure, and otherwise vapour, as [`condensate_of`]'s label is judged.
/// Below it, it is the diagram's phase at the surface pressure, and none where its vapour pressure
/// exceeds the surface pressure, so that it boils.
#[must_use]
fn condensed_phase_at(
    diagram: &PhaseDiagram,
    temperature: Kelvin,
    pressure: Pascals,
    partial: Pascals,
) -> Option<CondensatePhase> {
    if temperature >= diagram.critical_temperature() {
        return (partial >= diagram.critical_pressure()).then_some(CondensatePhase::Supercritical);
    }
    diagram
        .phase_at(temperature, pressure)
        .and_then(CondensatePhase::of)
}

/// The condensate of the species `species` of a body's air, its row's `diagram` and its partition
/// `partitioned`, over `climate` under the surface pressure `pressure` at surface gravity `gravity`;
/// `None` for a species with nothing condensed that is not supercritical either.
#[must_use]
fn condensate_of(
    species: &Species,
    diagram: &PhaseDiagram,
    partitioned: &Partitioned,
    climate: &ClimateSpan,
    pressure: Pascals,
    gravity: MetresPerSecondSquared,
) -> Option<Condensate> {
    let lies_at = partitioned.lies_at;
    let condensed = partitioned.condensed().value();
    if condensed.is_nan() || condensed <= 0.0 {
        // Nothing condensed: a supercritical fluid at the surface, where the species makes one of
        // its own, or nothing.
        let supercritical = lies_at >= diagram.critical_temperature()
            && partitioned.partial_pressure >= diagram.critical_pressure();
        return supercritical.then(|| {
            let phase = CondensatePhase::Supercritical;
            Condensate {
                substance: species.substance,
                phase,
                reservoir: KilogramsPerSquareMetre::ZERO,
                lies_at,
                seasonal: false,
                phases: CondensatePhases::default().with(phase),
            }
        });
    }
    // Its reservoir lies where its vapour is controlled; where its vapour pressure there exceeds
    // the surface pressure, at the coldest zone; and where it cannot lie condensed even there, by
    // its triple point.
    let partial = partitioned.partial_pressure;
    let phase = condensed_phase_at(diagram, lies_at, pressure, partial)
        .or_else(|| condensed_phase_at(diagram, climate.coldest, pressure, partial))
        .unwrap_or(if climate.coldest < diagram.triple_temperature() {
            CondensatePhase::Solid
        } else {
            CondensatePhase::Liquid
        });
    let mut phases = CondensatePhases::default().with(phase);
    let triple = diagram.triple_temperature();
    let inside = (climate.coldest < triple && triple < climate.warmest).then_some(triple);
    for t in [Some(climate.coldest), Some(climate.warmest), inside]
        .into_iter()
        .flatten()
    {
        if let Some(found) = condensed_phase_at(diagram, t, pressure, partial) {
            phases = phases.with(found);
        }
    }
    // Part of it comes and goes where its range crosses its melting line, where the warmest zone
    // takes its phase off the surface, or, held toward its cold trap, where the warmest zone rises
    // above its frost point at its own partial pressure: sublimation goes as its vapour pressure
    // over its own partial pressure, not over the surface pressure, which sets only boiling.
    let melts = phases.contains(CondensatePhase::Solid) && phases.contains(CondensatePhase::Liquid);
    let leaves = condensed_phase_at(diagram, climate.warmest, pressure, partial) != Some(phase);
    let sublimes =
        partitioned.dominance > 0.0 && diagram.saturation_pressure(climate.warmest) > partial;
    let seasonal = melts || leaves || sublimes;
    // A gravity that is not positive and finite gives no column mass a weight can be read as.
    let kg = condensed / gravity.value();
    let reservoir = if kg.is_finite() && kg > 0.0 { kg } else { 0.0 };
    Some(Condensate {
        substance: species.substance,
        phase,
        reservoir: KilogramsPerSquareMetre::new(reservoir),
        lies_at,
        seasonal,
        phases,
    })
}

/// The order of the published condensates: by reservoir mass, largest first, with ties by registry
/// order (P14.T51.a).
#[must_use]
fn reservoir_order(a: &Condensate, b: &Condensate) -> Ordering {
    b.reservoir
        .total_cmp(&a.reservoir)
        .then(a.substance.cmp(&b.substance))
}

/// The condensate partition of a body of surface gravity `gravity`, m s⁻², positive and finite,
/// whose exchangeable inventory after escape is `inventory`, over the climate `climate` ([the
/// module](self)'s rules): each species' airborne and condensed weights and partial pressure, and
/// its condensate, if any.
///
/// For every species the airborne and condensed weights sum to its inventory's, and the surface
/// pressure is the airborne weights summed in registry order. A gravity that is not positive and
/// finite leaves every reservoir zero.
///
/// # Examples
///
/// A Mars given its measured air by hand, 636 Pa of 95.1% carbon dioxide, 2.59% nitrogen and 1.94%
/// argon by volume, with more carbon dioxide on its surface, holds its carbon dioxide at the frost
/// point of its cold trap, and the rest as a perennial, seasonal ice:
///
/// ```
/// use hyperion_sim::planetary::chem::partition::{
///     ClimateSpan, ColumnInventory, CondensatePhase, partition,
/// };
/// use hyperion_sim::planetary::derive::atmosphere::Gas;
/// use hyperion_sim::substance::diagram;
/// use hyperion_sim::units::{Kelvin, MetresPerSecondSquared, Pascals};
///
/// let co2 = Gas::CarbonDioxide.substance();
/// let climate = ClimateSpan::new(Kelvin::new(147.0), Kelvin::new(214.0), Kelvin::new(242.0))?;
/// let inventory = ColumnInventory::new([
///     (co2, Pascals::new(1_200.0)),
///     (Gas::Nitrogen.substance(), Pascals::new(11.0)),
///     (Gas::Argon.substance(), Pascals::new(12.0)),
/// ])?;
/// let mars = partition(&climate, MetresPerSecondSquared::new(3.72), &inventory);
/// let cap = diagram::saturation_pressure(co2, climate.coldest()).expect("CO2 has a diagram");
/// let held = mars.of(co2).expect("named").partial_pressure();
/// assert!((held.value() / cap.value() - 1.0).abs() < 1e-9);
/// let ice = mars.condensate(co2).expect("its cap");
/// assert_eq!(ice.phase(), CondensatePhase::Solid);
/// assert!(ice.reservoir().value() > 0.0 && ice.is_seasonal());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn partition(
    climate: &ClimateSpan,
    gravity: MetresPerSecondSquared,
    inventory: &ColumnInventory,
) -> Partition {
    let mut solved = AirborneWeights::new();
    solved.solve(climate, inventory);
    let AirborneWeights {
        weights,
        caps: held,
    } = solved;
    let (mut pressure, mut moles) = (0.0, 0.0);
    for (species, w) in inventory.species.iter().zip(&weights) {
        pressure += w.value();
        moles += w.value() / species.molar_mass;
    }
    let mut species = Vec::with_capacity(inventory.len());
    let mut condensates = Vec::new();
    for ((s, &airborne), state) in inventory.species.iter().zip(&weights).zip(&held) {
        let x = if moles > 0.0 {
            airborne.value() / s.molar_mass / moles
        } else {
            0.0
        };
        let d = state.map_or(0.0, |state| state.dominance);
        let partitioned = Partitioned {
            substance: s.substance,
            inventory: s.weight,
            airborne,
            partial_pressure: Pascals::new(x * pressure),
            dominance: d,
            lies_at: climate.controlling(d),
        };
        if let Some(diagram) = s.diagram
            && let Some(condensate) = condensate_of(
                s,
                diagram,
                &partitioned,
                climate,
                Pascals::new(pressure),
                gravity,
            )
        {
            condensates.push(condensate);
        }
        species.push(partitioned);
    }
    condensates.sort_by(reservoir_order);
    Partition {
        species,
        condensates,
        surface_pressure: Pascals::new(pressure),
    }
}

#[cfg(test)]
mod tests;
