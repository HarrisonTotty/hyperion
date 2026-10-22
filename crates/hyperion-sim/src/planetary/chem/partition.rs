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
//!     humidity from h to 1, as the weight d rises from 0 to 1. Over its own sea at the mean the
//!     pull is weakened by that sea's ocean weight c, `T_mean` − d (1 − c)(`T_mean` − `T_cold`)
//!     ([`Partitioned::own_ocean_weight`]; decision-p14-t51a-partition §4).
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
//!   interface (Venus's carbon dioxide). A liquid that lies as a solution of its air
//!   (`AIR_SOLUTIONS`: Titan's seas) is liquid down to that solution's first melt.
//! - **Seasonal** where part of it comes and goes over the year: where its range crosses its
//!   melting line, where the warmest zone takes its phase off the surface at the surface pressure
//!   (a liquid that boils), or, for a condensable held toward its cold trap, where the warmest zone
//!   rises above its frost point at its own partial pressure (Mars's carbon dioxide); and by phase,
//!   where the warmest zone takes that phase off ([`Condensate::is_seasonal_as`]).
//!
//! [`Partition::publish`] lists a body's condensates as its record will carry them, at most eight
//! entries, one for each substance and phase, each with its reservoir, its global equivalent layer,
//! its area by P14.T24.b's cover, generalised by substance ([`Cover`]), and its seasonal flag.
//!
//! # What this module is, and is not, yet
//!
//! It is P14.T51.a. Its call into T13's passes is `atmosphere_under` in
//! [`atmosphere`](crate::planetary::derive::atmosphere), and its call on the record
//! `GlobalFigures::condensates_and_ice_over`, both on the record since P14.T54.a deleted the
//! `p14-snapshot` lane's hold, so its rules and constants are output, and a change to any of them a
//! generator-version change. The record's surface section holds the published [`Condensates`]
//! (P14.T54.a), and the wire sends them. Left to P14.T54.a's later steps: the passes' climate
//! (P14.T24.a's range, `step_climate`, from the body's spin derived before them).
//!
//! Every saturation pressure is the registry's phase diagram's
//! ([`diagram::saturation_pressure`](crate::substance::diagram::saturation_pressure), P14.T49.b's
//! fitted curves), since P14.T54.a retired the held Clausius–Clapeyron sets. The
//! frost and boiling points are compared through the saturation pressure, which rises monotonically,
//! so the partition never inverts a curve (`saturation_temperature`'s 64 bisections are not on the
//! snapshot path).

use std::cmp::Ordering;
use std::error::Error;
use std::fmt;

use crate::math;
use crate::planetary::derive::atmosphere::{
    Gas, METHANE_RELATIVE_HUMIDITY, MOLAR_GAS_CONSTANT, SurfaceMaterial, SurfaceState,
};
use crate::planetary::hooks::crust::CrustLithology;
#[cfg(doc)]
use crate::planetary::hooks::figures::cover::{
    self, AIRLESS_ICE_TEMPERATURE, DRY_SURFACE_WATER_SHARE,
};
use crate::planetary::hooks::figures::cover::{
    ICE_SHEET_THICKNESS, PERENNIAL_ICE_TEMPERATURE, melting_point,
};
use crate::planetary::hooks::figures::relief::Hypsometry;
use crate::planetary::hooks::surface::{FULL_OCEAN_FRACTION, OceanWeight};
use crate::planetary::surface::MAX_CONDENSATES;
use crate::substance::diagram::PhaseDiagram;
use crate::substance::{CondensedPhase, Phase, Substance, SubstanceId, by_key};
use crate::units::consts::SECONDS_PER_JULIAN_YEAR;
use crate::units::{
    Kelvin, KilogramsPerCubicMetre, KilogramsPerSquareMetre, Metres, MetresPerSecondSquared,
    Pascals,
};

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

/// How many times, at most, the condensables are solved in turn, each against the others' weights:
/// 8, a fixed cap from a deterministic start (decision-composition §1.3's rule for every solver).
/// The sweeps stop early after one that moves no weight, bit for bit, which every later sweep
/// would repeat (lever (p), [`AirborneWeights::solve`]), so a solve is the full count's to the bit.
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

    /// The span of a climate whose range is `range`, coldest then warmest (P14.T24.a's
    /// [`climate_range`](crate::planetary::hooks::surface::SurfaceConditions::climate_range)),
    /// about the mean `mean`: the range's ends, widened to the mean where the range does not hold
    /// it, a climate whose zonal ends both lie on one side of its mean.
    ///
    /// A mean that [`isothermal`](Self::isothermal) would not keep, or an end that is not a
    /// temperature, gives the isothermal span at the mean.
    #[must_use]
    pub fn about(range: (Kelvin, Kelvin), mean: Kelvin) -> Self {
        let (cold, warm) = range;
        let coldest = if cold < mean { cold } else { mean };
        let warmest = if warm > mean { warm } else { mean };
        Self::new(coldest, mean, warmest).unwrap_or_else(|_| Self::isothermal(mean))
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

    /// The temperature that controls a condensable pulled toward its cold trap by `pull`, in
    /// [0, 1]: `T_mean` − p (`T_mean` − `T_cold`), with p = d (1 − c) for its dominance d and its
    /// own sea's ocean weight c ([`Partitioned::own_ocean_weight`]). It is the mean for a minor
    /// condensable, and for a dominant one over its own full ocean; the coldest zone for a dominant
    /// one with no sea of its own.
    #[must_use]
    fn controlling(&self, pull: f64) -> Kelvin {
        let (cold, mean) = (self.coldest.value(), self.mean.value());
        if pull <= 0.0 {
            self.mean
        } else if pull >= 1.0 {
            self.coldest
        } else {
            Kelvin::new(mean - pull * (mean - cold))
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

/// The basins a body's liquids fill, as the partition reads them: P14.T24.b's capacity, the
/// hypsometry of P14.T48.a ([`Hypsometry::fill`]), or none known.
///
/// A liquid reservoir of a global layer h fills the share f of the surface that h fills of the
/// hypsometry, and weighs c = min(1, f ÷ [`FULL_OCEAN_FRACTION`]) in its climate, as P14.T24.a's θ
/// weighs water's potential ocean ([`OceanWeight::of_potential_ocean`]; decision-p14-t51a-partition
/// §4). The layer that fills that fraction is found once, so that a deeper one weighs fully with no
/// search of its own.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Basins {
    of: Option<Filled>,
}

/// A known hypsometry, with the layer that fills [`FULL_OCEAN_FRACTION`] of it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Filled {
    hypsometry: Hypsometry,
    full_ocean: Metres,
}

impl Basins {
    /// No basins known: no liquid fills any share of the surface, and none weighs in its climate,
    /// so that a condensable's own sea does not hold it at the mean (part 1's rule, c = 0).
    pub const UNKNOWN: Self = Self { of: None };

    /// The basins of the hypsometry `hypsometry`.
    #[must_use]
    pub fn of(hypsometry: &Hypsometry) -> Self {
        Self {
            of: Some(Filled {
                hypsometry: *hypsometry,
                full_ocean: hypsometry.layer_filling(FULL_OCEAN_FRACTION),
            }),
        }
    }

    /// The hypsometry and the layer that fills [`FULL_OCEAN_FRACTION`] of it, where the basins are
    /// known: what a pass's context compares of them (P14.T54.a).
    #[must_use]
    pub(crate) fn known(&self) -> Option<(&Hypsometry, Metres)> {
        self.of
            .as_ref()
            .map(|filled| (&filled.hypsometry, filled.full_ocean))
    }

    /// The share of the surface, 0 to 1, that a liquid of global layer `depth` fills
    /// ([`Hypsometry::fill`]); none where no basins are known.
    #[must_use]
    pub fn area_filled(&self, depth: Metres) -> f64 {
        self.of
            .map_or(0.0, |filled| filled.hypsometry.fill(depth).1)
    }

    /// The weight c, 0 to 1, of a liquid of global layer `depth` in its climate: min(1,
    /// f ÷ [`FULL_OCEAN_FRACTION`]) of the share f it fills ([`area_filled`](Self::area_filled)),
    /// and 0 where no basins are known.
    ///
    /// From the layer that fills that fraction upward it is 1 by this rule, found once, with no
    /// search of its own: within a part in 10⁹ of that layer the direct share may lie a few bits
    /// under 0.708.
    #[must_use]
    pub fn ocean_weight(&self, depth: Metres) -> f64 {
        let Some(filled) = self.of else {
            return 0.0;
        };
        if depth >= filled.full_ocean && depth.value() > 0.0 {
            return 1.0;
        }
        OceanWeight::of_potential_ocean(filled.hypsometry.fill(depth).1).get()
    }
}

/// The ground the partition lies on (P14.T51.a): the surface gravity that weighs its reservoirs,
/// the basins its liquids fill and the share of its condensed water that lies at the surface.
///
/// A gravity that is not positive and finite gives no reservoir a mass, and [`Basins::UNKNOWN`]
/// weighs no sea.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ground {
    gravity: MetresPerSecondSquared,
    basins: Basins,
    water_at_surface: f64,
}

impl Ground {
    /// The ground of surface gravity `gravity`, m s⁻², over the basins `basins`, with the share
    /// `water_at_surface` of its condensed water at the surface, held to [0, 1] (none for a share
    /// that is not a number).
    ///
    /// The share is P14.T24.b's, all of it while the volcanic greenhouse runs and
    /// [`DRY_SURFACE_WATER_SHARE`] after ([`cover::water_surface_share`]), the rest in the crust and
    /// the ground ice: its own sea's weight reads that share of its water
    /// (decision-p14-t51a-part2 §4.2).
    #[must_use]
    pub fn new(gravity: MetresPerSecondSquared, basins: Basins, water_at_surface: f64) -> Self {
        let share = if water_at_surface.is_nan() {
            0.0
        } else {
            water_at_surface.clamp(0.0, 1.0)
        };
        Self {
            gravity,
            basins,
            water_at_surface: share,
        }
    }

    /// A ground of surface gravity `gravity` whose basins are not known, all of its water at the
    /// surface: part 1's partition, with no own sea weighed.
    #[must_use]
    pub const fn without_basins(gravity: MetresPerSecondSquared) -> Self {
        Self {
            gravity,
            basins: Basins::UNKNOWN,
            water_at_surface: 1.0,
        }
    }

    /// The mean surface gravity.
    #[must_use]
    pub const fn gravity(&self) -> MetresPerSecondSquared {
        self.gravity
    }

    /// The basins its liquids fill.
    #[must_use]
    pub const fn basins(&self) -> &Basins {
        &self.basins
    }

    /// The share of its condensed water at the surface, 0 to 1.
    #[must_use]
    pub const fn water_at_surface(&self) -> f64 {
        self.water_at_surface
    }
}

/// A solution a condensable's liquid lies as under an air whose largest gas besides it is a given
/// background (decision-p14-t51a-part2 §2.2): one row of [`AIR_SOLUTIONS`], each a registry key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AirSolution {
    /// The condensable whose liquid it is, the solvent.
    pub(crate) solvent: &'static str,
    /// The air's largest gas besides the solvent, under which its liquid is the solution.
    pub(crate) background: &'static str,
    /// The solution's row.
    pub(crate) solution: &'static str,
}

/// The solutions a condensable's liquid lies as, by its air (decision-p14-t51a-part2 §2.2): one row,
/// methane's liquid under an air whose largest gas besides it is nitrogen, Titan's seas'
/// `ch4_c2h6_n2`. Its nitrogen is that air's, dissolved, and its ethane the photolysis of its methane,
/// which survives where the hydrogen escapes, as on Titan ("The CH4 in Titan's atmosphere is
/// irreversibly destroyed by photolysis because one of the most abundant photolysis products, H2,
/// escapes": Hörst 2017, J. Geophys. Res. Planets 122, 432, doi:10.1002/2016JE005240, read). Under
/// an air of hydrogen methane's products recombine to methane, and its liquid is pure (from memory;
/// decision-p14-t51a-part2 §2.1).
///
/// It is a closed set, for the composition audit. The registry's solution rows carry no components
/// and no Henry's constants yet (P14.T49.h's mixing rules); water's seas stay pure, since the
/// brines' rows are their eutectics, far saltier than a sea, and their salts the crust's (P14.T51.c
/// with T51.g); ammonia–water and the acids need a coupled partition of two condensables (P14.T52.b–c).
/// The vapour's cap is the pure substance's, at its humidity: no Raoult factor, since T24.f's 0.5
/// already stands for Titan's measured methane.
pub(crate) const AIR_SOLUTIONS: [AirSolution; 1] = [AirSolution {
    solvent: "CH4",
    background: "N2",
    solution: "ch4_c2h6_n2",
}];

/// The solution whose liquid the condensable `substance` lies as under the air `air`, its species'
/// airborne weights over their molar masses in registry order, if any ([`AIR_SOLUTIONS`]): where the
/// air's largest gas besides it, ties by registry order, is a row's background.
#[must_use]
fn solution_of(substance: SubstanceId, air: &[(SubstanceId, f64)]) -> Option<SubstanceId> {
    AIR_SOLUTIONS.iter().find_map(|row| {
        if by_key(row.solvent) != Some(substance) {
            return None;
        }
        let mut largest: Option<(SubstanceId, f64)> = None;
        for &(id, moles) in air {
            if id != substance && moles > 0.0 && largest.is_none_or(|(_, most)| moles > most) {
                largest = Some((id, moles));
            }
        }
        let background = by_key(row.background)?;
        (largest.map(|(id, _)| id) == Some(background))
            .then(|| by_key(row.solution))
            .flatten()
    })
}

/// The first melt of a condensable's frozen assemblage: its liquid's solution's transition where it
/// lies as one ([`solution_of`]), and otherwise its own row's, its triple point; read as
/// [`Transition::temperature`](crate::substance::Transition::temperature) for every kind
/// (decision-p14-t49b-condensed §2). A row with no condensed columns takes its diagram's triple
/// point.
#[must_use]
fn first_melt_of(row: &Substance, solution: Option<&Substance>, diagram: &PhaseDiagram) -> Kelvin {
    solution
        .or(Some(row))
        .and_then(Substance::condensed)
        .map_or(diagram.triple_temperature(), |c| {
            c.transition().temperature()
        })
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

    /// Whether any of its species can condense: one with a phase diagram and a positive weight,
    /// the only species whose caps read the climate ([`AirborneWeights::solve`]). A column with
    /// none is partitioned the same over every climate.
    #[must_use]
    pub(crate) fn condenses(&self) -> bool {
        self.species
            .iter()
            .any(|species| species.condensable().is_some())
    }
}

impl Species {
    /// The species' phase diagram where it can condense, with a positive weight: the only
    /// species whose caps read the climate ([`AirborneWeights::solve`]).
    #[must_use]
    fn condensable(&self) -> Option<&'static PhaseDiagram> {
        // By comparison, so that a NaN weight condenses nothing.
        self.diagram.filter(|_| self.weight.value() > 0.0)
    }
}

/// What the sweeps keep of one condensable over one partition: its saturation pressures at the
/// climate's two ends, computed once, and its dominance and its own sea's ocean weight as last
/// solved.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Caps {
    /// h `p_sat`(`T_mean`), the minor branch's cap.
    minor_cap: Pascals,
    /// `p_sat`(`T_cold`), the dominant branch's.
    cold_cap: Pascals,
    dominance: f64,
    /// c, the ocean weight of its own liquid reservoir at the mean ([`own_ocean_weight`]).
    own_ocean: f64,
}

/// The ocean weight c, 0 to 1, of the own liquid reservoir of the condensable `species`, of
/// phase diagram `diagram`, at the mean of the climate `climate`, on the ground `ground`, where on
/// the minor branch the others weigh `others` and it weighs `minor` (decision-p14-t51a-partition
/// §4).
///
/// It is its reservoir at the mean, its inventory less its minor branch's weight (water's at its
/// ground's surface share, [`Ground::water_at_surface`]), where its pure substance's phase at the
/// mean and the surface pressure that branch gives is liquid ([`condensed_phase_at`]): that
/// liquid's global layer, over its grain density at the mean, weighed by the basins
/// ([`Basins::ocean_weight`]). A closed form of the inventory and the climate, as the dominance's
/// share is, so that it needs no iteration of its own. A solution's first melt labels the phases
/// after the solve and never moves the cap (decision-p14-t51a-part2 §2.2). Zero where the
/// condensable would boil or freeze at the mean, where all of it is airborne there, and where the
/// gravity or the liquid's density is not known.
#[must_use]
fn own_ocean_weight(
    species: &Species,
    diagram: &PhaseDiagram,
    climate: &ClimateSpan,
    ground: &Ground,
    others: Pascals,
    minor: Pascals,
) -> f64 {
    let share = if species.substance == Gas::Water.substance() {
        ground.water_at_surface
    } else {
        1.0
    };
    let reservoir = (species.weight - minor).value() * share;
    let g = ground.gravity.value();
    // By comparison, so that a NaN holds no sea.
    if !(reservoir > 0.0 && g > 0.0 && g.is_finite()) {
        return 0.0;
    }
    let pressure = others + minor;
    let pure = diagram.triple_temperature();
    let phase = condensed_phase_at(diagram, pure, climate.mean, pressure, minor);
    if phase != Some(CondensatePhase::Liquid) {
        return 0.0;
    }
    let Some(density) = species
        .substance
        .substance()
        .grain_density(CondensedPhase::Liquid, climate.mean)
    else {
        return 0.0;
    };
    let depth = reservoir / g / density.value();
    ground.basins.ocean_weight(Metres::new(depth))
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
    /// Each condensable starts at the least of its inventory and its two caps, and the sweeps run
    /// in registry order, at most [`PARTITION_SWEEPS`] of them. They stop after a sweep that leaves
    /// every weight as it was, bit for bit (lever (p), decision-p14-t54a-passes, "Cost escalation
    /// (after Batch B)"): a species' update reads only the others' weights and its own fixed caps,
    /// so every later sweep would repeat that one, its dominance and own ocean included, and the
    /// solve is the full count's to the bit.
    pub(crate) fn solve(
        &mut self,
        climate: &ClimateSpan,
        ground: &Ground,
        inventory: &ColumnInventory,
    ) {
        #[cfg(test)]
        if let Some(checked) = checked_exits(
            ExitsChecked::Solve,
            || {
                let mut solved = Self::new();
                solved.solve_sweeping(climate, ground, inventory, PARTITION_SWEEPS);
                solved
            },
            same_solve,
        ) {
            *self = checked;
            return;
        }
        self.solve_sweeping(climate, ground, inventory, PARTITION_SWEEPS);
    }

    /// [`solve`](Self::solve) with at most `sweeps` sweeps, for the tests that compare the count
    /// with more.
    fn solve_sweeping(
        &mut self,
        climate: &ClimateSpan,
        ground: &Ground,
        inventory: &ColumnInventory,
        sweeps: u32,
    ) {
        self.start(climate, inventory);
        #[cfg(test)]
        count_exits(|counts| counts.solves += 1);
        for _ in 0..sweeps {
            #[cfg(test)]
            count_exits(|counts| counts.sweeps += 1);
            if !self.sweep(climate, ground, inventory) && exits_taken() {
                break;
            }
        }
    }

    /// Each species' weight before the sweeps, and each condensable's caps: its saturation
    /// pressures at the climate `climate`'s two ends, with no dominance or own ocean yet.
    fn start(&mut self, climate: &ClimateSpan, inventory: &ColumnInventory) {
        let Self { weights, caps } = self;
        weights.clear();
        caps.clear();
        for species in &inventory.species {
            let held = species.condensable().map(|diagram| Caps {
                minor_cap: Pascals::new(
                    species.humidity * diagram.saturation_pressure(climate.mean).value(),
                ),
                cold_cap: diagram.saturation_pressure(climate.coldest),
                dominance: 0.0,
                own_ocean: 0.0,
            });
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
    }

    /// One sweep of the condensables in registry order over the climate `climate` on the ground
    /// `ground`; whether it moved any weight, bit for bit.
    fn sweep(
        &mut self,
        climate: &ClimateSpan,
        ground: &Ground,
        inventory: &ColumnInventory,
    ) -> bool {
        let Self { weights, caps } = self;
        let mut moved = false;
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
            // Its own sea, where it has one at the mean, holds it there (§4); a minor
            // condensable is held there already.
            let c = if d > 0.0 {
                own_ocean_weight(species, diagram, climate, ground, total, minor)
            } else {
                0.0
            };
            let pull = d * (1.0 - c);
            let cap = if d <= 0.0 {
                held.minor_cap
            } else if pull >= 1.0 {
                held.cold_cap
            } else {
                let humidity = math::powf(species.humidity, 1.0 - d);
                let at = diagram.saturation_pressure(climate.controlling(pull));
                Pascals::new(humidity * at.value())
            };
            let weight = if d <= 0.0 { minor } else { at_cap(cap) };
            moved |= !weight.value().total_cmp(&weights[k].value()).is_eq();
            weights[k] = weight;
            caps[k] = Some(Caps {
                dominance: d,
                own_ocean: c,
                ..held
            });
        }
        moved
    }
}

/// Whether a solve of the partition's weights ([`AirborneWeights::solve`]) and a step of T13.c's
/// partitioned greenhouse stop at their bit-exact fixed points (lever (p), decision-p14-t54a-passes,
/// "Cost escalation (after Batch B)"): always in the generator, and in the tests unless they switch
/// the exits off (`with_exits`).
#[must_use]
pub(crate) fn exits_taken() -> bool {
    #[cfg(test)]
    if EXITS.with(core::cell::Cell::get) == Exits::Unlevered {
        return false;
    }
    true
}

/// How the partition's solves and T13.c's partitioned greenhouses stop, a test-only switch of
/// lever (p) ([`with_exits`]).
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Exits {
    /// At their bit-exact fixed points, as the generator's do ([`exits_taken`]).
    Taken,
    /// After every sweep and every step, each step reading its climate, as before the lever: the
    /// unlevered path, the oracle.
    Unlevered,
    /// Both: each greenhouse, and each solve outside one, run with the exits and without and
    /// asserted the same to the bit, and counted.
    Checked,
}

/// What [`checked_exits`] checks.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExitsChecked {
    /// A solve of the partition's weights outside a greenhouse.
    Solve,
    /// A partitioned greenhouse, its solves inside it.
    Greenhouse,
}

/// What [`with_exits`] counts on its thread while the exits are taken: the solves and their
/// sweeps, the greenhouses and their evaluations (each step's and the final one) and the step
/// climates they read; and under [`Exits::Checked`] the solves and greenhouses checked.
#[cfg(test)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ExitCounts {
    pub(crate) solves: u64,
    pub(crate) sweeps: u64,
    pub(crate) greenhouses: u64,
    pub(crate) evaluations: u64,
    pub(crate) climates: u64,
    pub(crate) checked_solves: u64,
    pub(crate) checked_greenhouses: u64,
}

#[cfg(test)]
impl ExitCounts {
    /// None counted.
    const ZERO: Self = Self {
        solves: 0,
        sweeps: 0,
        greenhouses: 0,
        evaluations: 0,
        climates: 0,
        checked_solves: 0,
        checked_greenhouses: 0,
    };
}

#[cfg(test)]
thread_local! {
    /// The switch on this thread, a test's own: tests run on threads of one process.
    static EXITS: core::cell::Cell<Exits> = const { core::cell::Cell::new(Exits::Taken) };
    /// What the exits did on this thread.
    static EXIT_COUNTS: core::cell::Cell<ExitCounts> = const {
        core::cell::Cell::new(ExitCounts::ZERO)
    };
}

/// `run`'s result with the partition's solves and the partitioned greenhouses stopping as `exits`
/// says, and what they counted. The switch is set back after.
#[cfg(test)]
#[must_use]
pub(crate) fn with_exits<R>(exits: Exits, run: impl FnOnce() -> R) -> (R, ExitCounts) {
    let before = EXITS.with(|cell| cell.replace(exits));
    EXIT_COUNTS.with(|cell| cell.set(ExitCounts::ZERO));
    let result = run();
    EXITS.with(|cell| cell.set(before));
    (
        result,
        EXIT_COUNTS.with(|cell| cell.replace(ExitCounts::ZERO)),
    )
}

/// Counts with `count` where the exits are taken on this thread: the generator's path, which
/// [`Exits::Checked`] runs as well as the oracle's.
#[cfg(test)]
pub(crate) fn count_exits(count: impl FnOnce(&mut ExitCounts)) {
    if EXITS.with(core::cell::Cell::get) == Exits::Taken {
        EXIT_COUNTS.with(|cell| {
            let mut counts = cell.get();
            count(&mut counts);
            cell.set(counts);
        });
    }
}

/// Under [`Exits::Checked`], `run`'s result with the exits taken, which `same` asserts is its
/// result with them off, counted as `what`; `None` under any other setting, and inside a check
/// already made.
#[cfg(test)]
pub(crate) fn checked_exits<T>(
    what: ExitsChecked,
    run: impl Fn() -> T,
    same: impl FnOnce(&T, &T),
) -> Option<T> {
    if EXITS.with(core::cell::Cell::get) != Exits::Checked {
        return None;
    }
    let set = |exits: Exits| EXITS.with(|cell| cell.set(exits));
    set(Exits::Unlevered);
    let oracle = run();
    set(Exits::Taken);
    let taken = run();
    match what {
        ExitsChecked::Solve => count_exits(|counts| counts.checked_solves += 1),
        ExitsChecked::Greenhouse => count_exits(|counts| counts.checked_greenhouses += 1),
    }
    set(Exits::Checked);
    same(&taken, &oracle);
    Some(taken)
}

/// Asserts that the solves `taken` and `oracle` hold the same weights and caps, bit for bit.
#[cfg(test)]
fn same_solve(taken: &AirborneWeights, oracle: &AirborneWeights) {
    use hyperion_testkit::float::assert_same_bits;
    assert_eq!(taken.weights.len(), oracle.weights.len());
    for (a, b) in taken.weights.iter().zip(&oracle.weights) {
        assert_same_bits(a.value(), b.value());
    }
    assert_eq!(taken.caps.len(), oracle.caps.len());
    for (a, b) in taken.caps.iter().zip(&oracle.caps) {
        match (a, b) {
            (Some(a), Some(b)) => {
                assert_same_bits(a.minor_cap.value(), b.minor_cap.value());
                assert_same_bits(a.cold_cap.value(), b.cold_cap.value());
                assert_same_bits(a.dominance, b.dominance);
                assert_same_bits(a.own_ocean, b.own_ocean);
            }
            (None, None) => {}
            _ => panic!("the caps differ: {a:?} against {b:?}"),
        }
    }
}

/// The phase a condensate lies in (P14.T51.a): never the vapour.
///
/// It is distinct from the registry's [`CondensedPhase`],
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
    own_ocean_weight: f64,
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
    /// diagram, 1 for one held toward its cold trap.
    #[must_use]
    pub const fn dominance(&self) -> f64 {
        self.dominance
    }

    /// The ocean weight c of its own liquid reservoir at the mean, 0 to 1
    /// (decision-p14-t51a-partition §4): min(1, f ÷ 0.708) of the share f of the surface that
    /// reservoir fills of its ground's basins ([`Basins::ocean_weight`]), where its phase at the
    /// mean is liquid with more of it than its air holds there; 0 otherwise, and for a minor
    /// condensable, which its own sea already holds at the mean.
    ///
    /// It weakens the pull toward the cold trap, `T_mean` − d (1 − c)(`T_mean` − `T_cold`): a
    /// non-dilute condensable over its own ocean holds its vapour where the ocean lies, its weak
    /// horizontal gradients and its latent heat keeping no colder zone in control (Pierrehumbert and
    /// Ding 2016, Proc. R. Soc. A 472, 20160107; Ding and Pierrehumbert 2016, ApJ 822, 24; both
    /// abstracts read).
    #[must_use]
    pub const fn own_ocean_weight(&self) -> f64 {
        self.own_ocean_weight
    }

    /// The temperature that controls its vapour and at which its reservoir lies,
    /// `T_mean` − d (1 − c)(`T_mean` − `T_cold`): the mean for a minor species and for a dominant
    /// one over its own full ocean, the coldest zone for a dominant one with no sea of its own.
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
    seasonal_phases: CondensatePhases,
    liquid: SubstanceId,
    first_melt: Kelvin,
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
    /// A supercritical fluid's is none, since the fluid is the air at the surface. Its published
    /// entries divide it between its phases, each with its global equivalent layer over its grain
    /// density ([`Partition::publish`]).
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

    /// Whether its reservoir in the phase `phase` is seasonal (decision-p14-t51a-partition §3).
    ///
    /// It is where the warmest zone takes that phase off the surface: a solid that melts there, or,
    /// held toward its cold trap, sublimes there at its own partial pressure; a liquid that boils
    /// there at the surface pressure. Earth's ocean is perennial and its ice seasonal; Mars's carbon
    /// dioxide ice is seasonal and its water ice, a minor condensable's, perennial. False for a
    /// phase its reservoir does not take, and for a supercritical fluid, which has no reservoir.
    #[must_use]
    pub const fn is_seasonal_as(&self, phase: CondensatePhase) -> bool {
        self.seasonal_phases.contains(phase)
    }

    /// The phases its reservoir takes over the climate's range, at the surface pressure.
    ///
    /// It holds its own phase, with the solid where the coldest zone freezes a liquid (Earth's ice),
    /// or the liquid where the warmest zone melts a solid. [`Partition::publish`] divides the
    /// reservoir between them.
    #[must_use]
    pub const fn phases(&self) -> CondensatePhases {
        self.phases
    }

    /// The row its liquid lies as: its own, or the solution it is the solvent of (Titan's seas'
    /// `ch4_c2h6_n2` for methane under nitrogen).
    #[must_use]
    pub const fn liquid(&self) -> SubstanceId {
        self.liquid
    }

    /// Its frozen assemblage's first melt, below which its reservoir is solid: its row's
    /// transition, or its liquid's solution's (decision-p14-t49b-condensed §2).
    #[must_use]
    pub const fn first_melt(&self) -> Kelvin {
        self.first_melt
    }
}

/// A body's condensate partition (P14.T51.a; [`partition`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Partition {
    species: Vec<Partitioned>,
    condensates: Vec<Condensate>,
    surface_pressure: Pascals,
    climate: ClimateSpan,
    ground: Ground,
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
    /// one for each substance, which [`publish`](Self::publish) lists as at most
    /// [`MAX_CONDENSATES`] entries, one for each substance and phase.
    #[must_use]
    pub fn condensates(&self) -> &[Condensate] {
        &self.condensates
    }

    /// The condensate of `substance`, if it has one.
    #[must_use]
    pub fn condensate(&self, substance: SubstanceId) -> Option<&Condensate> {
        self.condensates.iter().find(|c| c.substance == substance)
    }

    /// The condensables its surface holds a reservoir of, in registry order.
    ///
    /// They are those with a condensed reservoir, which P14.T48.d's climate regime reads in place
    /// of [`surface_reservoirs`](crate::planetary::hooks::surface::regime::surface_reservoirs)
    /// from 22 → 23. A supercritical fluid is none.
    #[must_use]
    pub fn reservoirs(&self) -> Vec<SubstanceId> {
        let mut held: Vec<SubstanceId> = self
            .condensates
            .iter()
            .filter(|c| c.reservoir.value() > 0.0)
            .map(|c| c.substance)
            .collect();
        held.sort_unstable();
        held
    }

    /// The climate it was solved over.
    #[must_use]
    pub const fn climate(&self) -> &ClimateSpan {
        &self.climate
    }

    /// The ground it lies on.
    #[must_use]
    pub const fn ground(&self) -> &Ground {
        &self.ground
    }
}

/// Whether a condensable of `diagram`, whose frozen assemblage first melts at `first_melt`, is a
/// solution's liquid at `temperature` where its pure substance would be solid: between its first
/// melt and its pure triple point (decision-p14-t49b-condensed §2.4: Titan's seas are liquid down to
/// the methane–ethane eutectic). A pure substance, whose first melt is its triple point, never is.
#[must_use]
fn solution_liquid_at(diagram: &PhaseDiagram, first_melt: Kelvin, temperature: Kelvin) -> bool {
    first_melt < diagram.triple_temperature()
        && temperature >= first_melt
        && temperature < diagram.triple_temperature()
}

/// The condensate phase of a reservoir of the substance of `diagram`, whose frozen assemblage first
/// melts at `first_melt` and whose partial pressure in the air is `partial`, at `temperature`
/// under the surface pressure `pressure`, or `None` where it cannot lie condensed there.
///
/// At or above its critical temperature it is supercritical where its own partial pressure is at
/// least its critical pressure, and otherwise vapour, as [`condensate_of`]'s label is judged.
/// Below it, it is the diagram's phase at the surface pressure, and none where its vapour pressure
/// exceeds the surface pressure, so that it boils; a solution's liquid where its pure substance
/// would be solid above its first melt ([`solution_liquid_at`]).
#[must_use]
fn condensed_phase_at(
    diagram: &PhaseDiagram,
    first_melt: Kelvin,
    temperature: Kelvin,
    pressure: Pascals,
    partial: Pascals,
) -> Option<CondensatePhase> {
    if temperature >= diagram.critical_temperature() {
        return (partial >= diagram.critical_pressure()).then_some(CondensatePhase::Supercritical);
    }
    match diagram
        .phase_at(temperature, pressure)
        .and_then(CondensatePhase::of)
    {
        Some(CondensatePhase::Solid) if solution_liquid_at(diagram, first_melt, temperature) => {
            Some(CondensatePhase::Liquid)
        }
        phase => phase,
    }
}

/// Whether the warmest zone, at `warmest`, takes the reservoir of the substance of `diagram` in the
/// phase `phase` off the surface (decision-p14-t51a-partition §3): a solid that is not solid there
/// at the surface pressure `pressure`, so that it melts, or, held toward its cold trap (its
/// dominance `dominance` above 0), whose vapour pressure there exceeds its own partial pressure
/// `partial`, so that it sublimes; a liquid that is neither liquid nor solid there, so that it
/// boils. A supercritical fluid has no reservoir to take.
///
/// A minor solid is held below saturation at the mean, so its frost point always lies below the
/// warmest zone: the sublimation test is a dominant one's only, as the species' own flag reads it
/// (the part 1 ruling's §3), or Titan's crust and Mars's water ice would read seasonal.
#[must_use]
fn taken_off(
    (diagram, first_melt): (&PhaseDiagram, Kelvin),
    phase: CondensatePhase,
    warmest: Kelvin,
    (pressure, partial): (Pascals, Pascals),
    dominance: f64,
) -> bool {
    let there = condensed_phase_at(diagram, first_melt, warmest, pressure, partial);
    match phase {
        CondensatePhase::Solid => {
            there != Some(CondensatePhase::Solid)
                || (dominance > 0.0 && diagram.saturation_pressure(warmest) > partial)
        }
        CondensatePhase::Liquid => !matches!(
            there,
            Some(CondensatePhase::Liquid | CondensatePhase::Solid)
        ),
        CondensatePhase::Supercritical => false,
    }
}

/// The condensate of the species `species` of a body's air, its row's `diagram`, the solution its
/// liquid lies as, if any ([`solution_of`]), and its partition `partitioned`, over `climate` under
/// the surface pressure `pressure` at surface gravity `gravity`; `None` for a species with nothing
/// condensed that is not supercritical either.
#[must_use]
fn condensate_of(
    species: &Species,
    (diagram, solution): (&PhaseDiagram, Option<SubstanceId>),
    partitioned: &Partitioned,
    climate: &ClimateSpan,
    pressure: Pascals,
    gravity: MetresPerSecondSquared,
) -> Option<Condensate> {
    let lies_at = partitioned.lies_at;
    let condensed = partitioned.condensed().value();
    let first_melt = first_melt_of(
        species.substance.substance(),
        solution.map(SubstanceId::substance),
        diagram,
    );
    let liquid = solution.unwrap_or(species.substance);
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
                seasonal_phases: CondensatePhases::default(),
                liquid,
                first_melt,
            }
        });
    }
    // Its reservoir lies where its vapour is controlled; where its vapour pressure there exceeds
    // the surface pressure, at the coldest zone; and where it cannot lie condensed even there, by
    // its first melt.
    let partial = partitioned.partial_pressure;
    let at = |t: Kelvin| condensed_phase_at(diagram, first_melt, t, pressure, partial);
    let phase =
        at(lies_at)
            .or_else(|| at(climate.coldest))
            .unwrap_or(if climate.coldest < first_melt {
                CondensatePhase::Solid
            } else {
                CondensatePhase::Liquid
            });
    let mut phases = CondensatePhases::default().with(phase);
    let inside =
        (climate.coldest < first_melt && first_melt < climate.warmest).then_some(first_melt);
    for t in [Some(climate.coldest), Some(climate.warmest), inside]
        .into_iter()
        .flatten()
    {
        if let Some(found) = at(t) {
            phases = phases.with(found);
        }
    }
    // Part of it comes and goes where its range crosses its melting line, where the warmest zone
    // takes its phase off the surface, or, held toward its cold trap, where the warmest zone rises
    // above its frost point at its own partial pressure: sublimation goes as its vapour pressure
    // over its own partial pressure, not over the surface pressure, which sets only boiling.
    let melts = phases.contains(CondensatePhase::Solid) && phases.contains(CondensatePhase::Liquid);
    let leaves = at(climate.warmest) != Some(phase);
    let sublimes =
        partitioned.dominance > 0.0 && diagram.saturation_pressure(climate.warmest) > partial;
    let seasonal = melts || leaves || sublimes;
    let mut seasonal_phases = CondensatePhases::default();
    for each in [CondensatePhase::Solid, CondensatePhase::Liquid] {
        if phases.contains(each)
            && taken_off(
                (diagram, first_melt),
                each,
                climate.warmest,
                (pressure, partial),
                partitioned.dominance,
            )
        {
            seasonal_phases = seasonal_phases.with(each);
        }
    }
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
        seasonal_phases,
        liquid,
        first_melt,
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

/// The condensate partition of a body on the ground `ground`, whose exchangeable inventory after
/// escape is `inventory`, over the climate `climate` ([the module](self)'s rules): each species'
/// airborne and condensed weights and partial pressure, and its condensate, if any.
///
/// For every species the airborne and condensed weights sum to its inventory's, and the surface
/// pressure is the airborne weights summed in registry order. A gravity that is not positive and
/// finite leaves every reservoir zero, and weighs no condensable's own sea.
///
/// # Examples
///
/// A Mars given its measured air by hand, 636 Pa of 95.1% carbon dioxide, 2.59% nitrogen and 1.94%
/// argon by volume, with more carbon dioxide on its surface, holds its carbon dioxide at the frost
/// point of its cold trap, and the rest as a perennial, seasonal ice:
///
/// ```
/// use hyperion_sim::planetary::chem::partition::{
///     ClimateSpan, ColumnInventory, CondensatePhase, Ground, partition,
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
/// let ground = Ground::without_basins(MetresPerSecondSquared::new(3.72));
/// let mars = partition(&climate, &ground, &inventory);
/// let cap = diagram::saturation_pressure(co2, climate.coldest()).expect("CO2 has a diagram");
/// let held = mars.of(co2).expect("named").partial_pressure();
/// assert!((held.value() / cap.value() - 1.0).abs() < 1e-9);
/// let ice = mars.condensate(co2).expect("its cap");
/// assert_eq!(ice.phase(), CondensatePhase::Solid);
/// assert!(ice.reservoir().value() > 0.0 && ice.is_seasonal());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn partition(climate: &ClimateSpan, ground: &Ground, inventory: &ColumnInventory) -> Partition {
    let mut solved = AirborneWeights::new();
    solved.solve(climate, ground, inventory);
    let AirborneWeights {
        weights,
        caps: held,
    } = solved;
    let (mut pressure, mut moles) = (0.0, 0.0);
    for (species, w) in inventory.species.iter().zip(&weights) {
        pressure += w.value();
        moles += w.value() / species.molar_mass;
    }
    // What each liquid lies as reads the solved air (decision-p14-t51a-part2 §2.2).
    let air: Vec<(SubstanceId, f64)> = inventory
        .species
        .iter()
        .zip(&weights)
        .map(|(s, w)| (s.substance, w.value() / s.molar_mass))
        .collect();
    let mut species = Vec::with_capacity(inventory.len());
    let mut condensates = Vec::new();
    for ((s, &airborne), state) in inventory.species.iter().zip(&weights).zip(&held) {
        let x = if moles > 0.0 {
            airborne.value() / s.molar_mass / moles
        } else {
            0.0
        };
        let (d, c) = state.map_or((0.0, 0.0), |state| (state.dominance, state.own_ocean));
        let partitioned = Partitioned {
            substance: s.substance,
            inventory: s.weight,
            airborne,
            partial_pressure: Pascals::new(x * pressure),
            dominance: d,
            own_ocean_weight: c,
            lies_at: climate.controlling(d * (1.0 - c)),
        };
        if let Some(diagram) = s.diagram
            && let Some(condensate) = condensate_of(
                s,
                (diagram, solution_of(s.substance, &air)),
                &partitioned,
                climate,
                Pascals::new(pressure),
                ground.gravity,
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
        climate: *climate,
        ground: *ground,
    }
}

/// What the published condensates read of a body's surface beyond its partition (P14.T51.a's
/// second part): P14.T24.b's cover, generalised by substance ([`Partition::publish`]).
///
/// Plain data with public fields.
#[derive(Clone, Copy)]
pub struct Cover<'a> {
    /// The share of the surface, 0 to 1, whose annual-mean climate lies below a temperature:
    /// P14.T24.a's [`cold_area`](crate::planetary::hooks::surface::SurfaceConditions::cold_area),
    /// read off one profile of the body for every solid (decision-p14-t51a-part2 §3.5).
    pub cold_area: &'a dyn Fn(Kelvin) -> f64,
    /// The surface state (P14.T13.c), which sets where a solid lasts the year: P14.T24.b's edges
    /// for water, and for any other solid its first melt's share under air, blended with its
    /// sublimation edge where airless.
    pub state: SurfaceState,
    /// Water's own cover, P14.T24.b's, which the water entries take.
    pub water: WaterCover,
}

impl fmt::Debug for Cover<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cover")
            .field("state", &self.state)
            .field("water", &self.water)
            .finish_non_exhaustive()
    }
}

/// Water's cover at a body's surface, P14.T24.b's at its surface water (decision-p14-t51a-part2
/// §4.2): what the published water entries take, so that they are T24.b's to the bit.
///
/// Plain data with public fields.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterCover {
    /// The surface water `σ_s` (`cover::surface_water`): the share of the partition's
    /// condensed water that lies at the surface.
    pub surface_water: KilogramsPerSquareMetre,
    /// The potential ocean `f_pot` the whole of it fills ([`cover::potential_ocean`]), which the
    /// liquid entry's area no longer takes: that is its own layer's fill
    /// (decision-p14-t54a-water-consistency §1.4).
    pub potential_ocean: f64,
    /// Its perennial ice's share of the surface ([`cover::ice_fraction`]), the solid entry's area.
    pub ice_fraction: f64,
    /// The density of its ice, ice Ih's at the mean surface temperature
    /// ([`ice_ih_density`](crate::substance::ice_ih_density)), at which T24.b caps the ice.
    pub ice_density: KilogramsPerCubicMetre,
    /// Whether the solid entry holds the crust's frozen water where the surface water holds none
    /// ([`CrustWater`]).
    pub crust_water: CrustWater,
}

/// Whether water's solid entry lists an ice bulk's crust where the surface water holds none
/// ([`WaterCover::crust_water`]; decision-p14-t54a-ganymede, C1 follow-up §C1.4, as §C3.1
/// amended it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CrustWater {
    /// Listed: the record's, on a body whose bulk is ice ([`SurfaceMaterial::Ice`]) whose lid is
    /// `H2O` (or not known), whose crust is its water. Its entry is then the partition's whole
    /// frozen water, the crust's ice-held water, shell and deeper ice, at
    /// [`WaterCover::ice_fraction`], until P14.T53.c divides it; the surface water, the water free
    /// to pool, stays P14.T13.b's, on water's Jeans verdict.
    ///
    /// [`SurfaceMaterial::Ice`]: crate::planetary::derive::atmosphere::SurfaceMaterial::Ice
    Listed,
    /// Not listed: off an ice bulk; on an ice bulk under any other lid (`cometary`, a chondrite,
    /// `phyllosilicate`, a silicate), whose crust's water stays in the bulk, unlisted, until
    /// P14.T53.c divides it (decision-p14-t54a-ganymede §C3.1): the lid's row is the surface the
    /// albedo reads, and the cover's ice area under it is the ice table beneath the lag, which the
    /// record does not publish; and in T13.c's passes, whose area mix reads an ice bulk's crust as
    /// its regolith.
    Unlisted,
}

impl CrustWater {
    /// The record's, on a body whose bulk is of `material` under the lid `lid`, its published
    /// pass's own, whose row its regolith read (decision-p14-t54a-ganymede §C3.1):
    /// [`Listed`](Self::Listed) on an ice bulk ([`SurfaceMaterial::Ice`]) whose lid is
    /// [`CrustLithology::Water`] or not known (`None`: the passes' stand-in, or a basis refused,
    /// whose regolith is the material's stand-in row, `H2O` on an ice bulk), so that the entry is
    /// listed exactly where the row the albedo read is `H2O`; [`Unlisted`](Self::Unlisted) on rock
    /// and under any other lid.
    #[must_use]
    pub(crate) const fn of_record(material: SurfaceMaterial, lid: Option<CrustLithology>) -> Self {
        match (material, lid) {
            (SurfaceMaterial::Ice, None | Some(CrustLithology::Water)) => Self::Listed,
            (
                SurfaceMaterial::Ice,
                Some(
                    CrustLithology::Graphite
                    | CrustLithology::Feldspathic
                    | CrustLithology::Basalt
                    | CrustLithology::IronRichBasalt
                    | CrustLithology::Komatiite
                    | CrustLithology::ReducedBasalt
                    | CrustLithology::Granite
                    | CrustLithology::Ultramafic
                    | CrustLithology::Phyllosilicate
                    | CrustLithology::ChondriteCi
                    | CrustLithology::ChondriteCm
                    | CrustLithology::ChondriteCv
                    | CrustLithology::ChondriteH
                    | CrustLithology::ChondriteL
                    | CrustLithology::ChondriteLl
                    | CrustLithology::ChondriteE
                    | CrustLithology::Cometary,
                ),
            )
            | (SurfaceMaterial::Rock, _) => Self::Unlisted,
        }
    }
}

/// The net sublimation rate at which an airless surface keeps a solid over the Solar System's age:
/// 144 kg m⁻² Gyr⁻¹, water ice's Hertz–Knudsen loss at 110 K (Schorghofer and Williams 2020, PSJ 1,
/// 54, doi:10.3847/PSJ/abb6ff, eq. 8 and Table 1, read, with a sublimation coefficient α of 1, as
/// P14.T24.b's [`AIRLESS_ICE_TEMPERATURE`] and plan 14's P14.T51.f state it;
/// decision-p14-t51a-part2 §3.3, whose arithmetic from Murphy and Koop's 2005 curve gives 144.0 at
/// 110.0 K). The ruling's `AIRLESS_ICE_LOSS_RATE`, its unit in its name.
pub const AIRLESS_ICE_LOSS_KG_PER_M2_GYR: f64 = 144.0;

/// The temperature, below its first melt `first_melt`, at which a solid of `diagram` and molar mass
/// `molar_mass_g_per_mol` loses `loss_kg_per_m2_gyr` (kg m⁻² Gyr⁻¹; T51.a's is
/// [`AIRLESS_ICE_LOSS_KG_PER_M2_GYR`]) by net Hertz–Knudsen sublimation against its own partial
/// pressure `partial`: α (`p_sat`(T) − p) √(M ÷ 2πRT) = J* with α = 1 (Schorghofer and Williams
/// 2020, eq. 8; decision-p14-t51a-part2 §3.3), by [`EDGE_HALVINGS`] halvings over [0,
/// `first_melt`]. Its first melt where it loses less even there.
///
/// At `partial` = 0 it is P14.T51.f's cold-trap threshold ([`exosphere_threshold`]). The loss is a
/// parameter so that T51.f's carbon dioxide test can take Schorghofer et al. 2021's 10 kg m⁻²
/// Gyr⁻¹; at 144 it gives the bits it gave as a constant.
///
/// Crate-visible for rendering plan R09.T13.d, whose ices on an airless body are ranked against
/// this edge at no partial pressure (decision-r09-t13d-normalisation §1.2).
#[must_use]
pub(crate) fn sublimation_edge(
    diagram: &PhaseDiagram,
    molar_mass_g_per_mol: f64,
    first_melt: Kelvin,
    partial: Pascals,
    loss_kg_per_m2_gyr: f64,
) -> Kelvin {
    let rate = loss_kg_per_m2_gyr / (1e9 * SECONDS_PER_JULIAN_YEAR);
    let kg_per_mol = molar_mass_g_per_mol * 1e-3;
    let loss = |t: f64| {
        let vapour = diagram.saturation_pressure(Kelvin::new(t)).value();
        let speed = (kg_per_mol / (2.0 * core::f64::consts::PI * MOLAR_GAS_CONSTANT * t)).sqrt();
        (vapour - partial.value()) * speed
    };
    let top = first_melt.value();
    // A NaN loss keeps the first melt.
    let at_top = loss(top);
    if at_top.is_nan() || at_top <= rate {
        return first_melt;
    }
    let (mut low, mut high) = (0.0, top);
    for _ in 0..EDGE_HALVINGS {
        let middle = f64::midpoint(low, high);
        if loss(middle) > rate {
            high = middle;
        } else {
            low = middle;
        }
    }
    Kelvin::new(f64::midpoint(low, high))
}

/// How many halvings locate an airless solid's sublimation edge: 64, always that many, past the
/// last bit of its first melt ([`sublimation_edge`]).
const EDGE_HALVINGS: u32 = 64;

/// The cold-trap threshold of the condensable `substance` at the loss `loss_kg_per_m2_gyr`, kg m⁻²
/// Gyr⁻¹ (P14.T51.f; science-r09-drainage.md §4.3 item 4): its [`sublimation_edge`] into vacuum,
/// at no partial pressure of its own, below its pure row's first melt (no air holds a solution in
/// an exosphere). One rule for every species: within 1 K of 110 K for water and 58.18 K for carbon
/// dioxide at 144 kg m⁻² Gyr⁻¹ on T49.b's curves (the partition's and the cold traps' tests), and
/// 55.53 K for carbon dioxide at 10. `None` for a row with no phase diagram or no molar mass, which
/// never condenses.
#[must_use]
pub(crate) fn exosphere_threshold(
    substance: SubstanceId,
    loss_kg_per_m2_gyr: f64,
) -> Option<Kelvin> {
    let row = substance.substance();
    let diagram = row.phase_diagram()?;
    let molar_mass = row.molar_mass_g_per_mol()?;
    Some(vacuum_edge(row, (diagram, molar_mass), loss_kg_per_m2_gyr))
}

/// The [`sublimation_edge`] of the row `row`, of `diagram` and molar mass `molar_mass_g_per_mol`,
/// at no partial pressure, below its pure first melt ([`exosphere_threshold`]).
#[must_use]
fn vacuum_edge(
    row: &Substance,
    (diagram, molar_mass_g_per_mol): (&PhaseDiagram, f64),
    loss_kg_per_m2_gyr: f64,
) -> Kelvin {
    sublimation_edge(
        diagram,
        molar_mass_g_per_mol,
        first_melt_of(row, None, diagram),
        Pascals::ZERO,
        loss_kg_per_m2_gyr,
    )
}

/// Which edges a body's solids take where it publishes them ([`Partition::publish_with`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum SolidEdges {
    /// P14.T51.a's: each solid's [`ice_line`] by the state, against the cover's cold area, the
    /// annual mean's, its mass its whole reservoir where it lies as a solid; and water T24.b's
    /// ([`WaterCover`]).
    Partition,
    /// P14.T51.f's on a body with no collisional atmosphere (from generator version 24, its hold):
    /// each solid's edge is its cold-trap threshold, its sublimation edge into vacuum
    /// ([`exosphere_threshold`] at [`AIRLESS_ICE_LOSS_KG_PER_M2_GYR`]), against the cover's cold
    /// area, which the caller gives as the zonal annual maximum's; and its mass is what that area
    /// holds at [`ICE_SHEET_THICKNESS`], at most its reservoir, so that the rest is left for its
    /// cold traps (science-r09-drainage.md §4.3 item 6).
    Exosphere {
        /// Whether water keeps T24.b's cover ([`WaterCover`]) rather than this rule: an ice-rich
        /// crust, which is its own ice.
        water_keeps_cover: bool,
    },
}

impl SolidEdges {
    /// Whether water's entries are T24.b's ([`WaterCover`]).
    #[must_use]
    const fn water_keeps_cover(self) -> bool {
        match self {
            Self::Partition => true,
            Self::Exosphere { water_keeps_cover } => water_keeps_cover,
        }
    }
}

/// The ratio of a solid's annual-mean edge under air to its first melt: P14.T24.b's perennial
/// edge over water's melting point, [`PERENNIAL_ICE_TEMPERATURE`] ÷ [`melting_point`] = 263.15 ÷
/// 273.16 = 0.963 355 (decision-p14-t51a-part2 §3.2). The seasonal excess a perennial ice must lie
/// below its melt goes as the temperature, (T ÷ 4)(δF ÷ F) on a radiatively controlled surface, so
/// the margin is a ratio, not water's 10 K.
#[must_use]
fn perennial_ratio() -> f64 {
    PERENNIAL_ICE_TEMPERATURE.value() / melting_point().value()
}

/// The annual-mean temperature below which a solid condensate of `diagram`, molar mass
/// `molar_mass_g_per_mol` and first melt `first_melt`, of dominance `dominance` and partial pressure
/// `partial` in its air, lasts the year at the surface of a body in the state `state`
/// (decision-p14-t51a-part2 §3.2–3.3), for every solid but water, which keeps P14.T24.b's edges:
///
/// - under air, a `Snowball` included, its first melt times [`perennial_ratio`];
/// - airless, that edge weighted by its dominance d, d r `T_fm` + (1 − d) `T_HK`, with `T_HK` its
///   sublimation edge at its own partial pressure ([`sublimation_edge`]): a dominant ice sets its
///   own air and buffers its own temperature (Pluto's and Triton's nitrogen), a minor one is lost
///   where it is warmer than its own air can hold. It is continuous in d and in the pressure.
#[must_use]
fn ice_line(
    (diagram, molar_mass_g_per_mol): (&PhaseDiagram, f64),
    first_melt: Kelvin,
    (dominance, partial): (f64, Pascals),
    state: SurfaceState,
) -> Kelvin {
    let under_air = perennial_ratio() * first_melt.value();
    match state {
        SurfaceState::Airless => {
            let edge = sublimation_edge(
                diagram,
                molar_mass_g_per_mol,
                first_melt,
                partial,
                AIRLESS_ICE_LOSS_KG_PER_M2_GYR,
            );
            let d = dominance.clamp(0.0, 1.0);
            Kelvin::new(d * under_air + (1.0 - d) * edge.value())
        }
        SurfaceState::GasEnvelope
        | SurfaceState::MagmaOcean
        | SurfaceState::RunawayGreenhouse
        | SurfaceState::Temperate
        | SurfaceState::Snowball => Kelvin::new(under_air),
    }
}

/// One published condensate (P14.T51.a): a substance in one phase at a body's surface, with its
/// share of the substance's reservoir, its global equivalent layer, the area it covers and whether
/// it is seasonal ([`Partition::publish`]).
///
/// On an ice-bulk body whose lid is `H2O` (or not known) the `H2O` solid entry is the crust's
/// ice-held frozen water, all of it (shell and deeper ice), at the cover's ice area, until
/// P14.T53.c divides it; under any other lid (`cometary`, a chondrite, `phyllosilicate`, a
/// silicate) the crust's water stays in the bulk, unlisted, until P14.T53.c divides it. T24.b's
/// surface water, the water free to pool, is T13.b's (decision-p14-t54a-ganymede, C1 follow-up
/// §C1.4, as §C3.1 amended it; [`WaterCover::crust_water`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CondensateEntry {
    substance: SubstanceId,
    phase: CondensatePhase,
    reservoir: KilogramsPerSquareMetre,
    reservoir_gel: Option<Metres>,
    area_fraction: f64,
    seasonal: bool,
    trapped_area_fraction: f64,
}

impl CondensateEntry {
    /// The part of [`area_fraction`](Self::area_fraction) in cold traps (P14.T51.f, P14.T51.i;
    /// decision-r09-t13f-cold-traps §1.4), 0 to `area_fraction`: the trap's exposed area as
    /// `Condensates::with_cold_traps` merged it, held with the entry to 1 (the zonal part kept
    /// whole, the trap given what is left), and 0 for an entry with no cold-trapped part. R09's
    /// field places the rest, its zonal part, by its own rule. Dimensionless.
    #[must_use]
    pub const fn trapped_area_fraction(&self) -> f64 {
        self.trapped_area_fraction
    }

    /// The substance: the condensable's row, or for a liquid the solution it lies as
    /// ([`Condensate::liquid`]).
    #[must_use]
    pub const fn substance(&self) -> SubstanceId {
        self.substance
    }

    /// Its phase.
    #[must_use]
    pub const fn phase(&self) -> CondensatePhase {
        self.phase
    }

    /// Its reservoir, its mass over the body's whole surface, kg m⁻²: its share of the substance's
    /// reservoir at the surface; none for a supercritical fluid.
    #[must_use]
    pub const fn reservoir(&self) -> KilogramsPerSquareMetre {
        self.reservoir
    }

    /// Its global equivalent layer, plan 14's `reservoir_gel_m`.
    ///
    /// It is its reservoir over its grain density in its phase at the temperature it lies at
    /// ([`Substance::grain_density`]); `None` where the registry has not read that density, which
    /// the wire sends as absent (P14.T54.a). Zero for a supercritical fluid. On a body whose water
    /// froze, the whole frozen water the partition holds, shell and deeper ice included, until
    /// P14.T53.c divides it between the condensate, the crust and the shell
    /// (decision-pre-23-review item 12).
    #[must_use]
    pub const fn reservoir_gel(&self) -> Option<Metres> {
        self.reservoir_gel
    }

    /// The share of the surface it covers, 0 to 1 (decision-p14-t54a-water-consistency §4, W4–W5):
    ///
    /// - a liquid's: the share of the body's basins its layer fills (water's alone, the other
    ///   liquids' together, divided by volume), so none where its reservoir lies whole as ice;
    /// - a solid's: its climate's cold area below its ice line, at most its mass spread at
    ///   [`ICE_SHEET_THICKNESS`] of its grain density, but water on an ice-rich crust, which is the
    ///   crust's own ice;
    /// - none for a supercritical fluid, which is the air at the surface and the only entry with no
    ///   mass.
    ///
    /// Entries may overlap: an ice over a sea, and ices in the same cold zones.
    #[must_use]
    pub const fn area_fraction(&self) -> f64 {
        self.area_fraction
    }

    /// Whether it is seasonal: the warmest zone takes this phase off the surface
    /// ([`Condensate::is_seasonal_as`]).
    #[must_use]
    pub const fn is_seasonal(&self) -> bool {
        self.seasonal
    }
}

/// A body's published condensates (P14.T51.a; [`Partition::publish`]).
///
/// At most [`MAX_CONDENSATES`] entries, one for each substance and phase, sorted by reservoir mass,
/// largest first, with ties by `SubstanceId`, then phase. A record's surface section holds them
/// ([`Surface::condensates`](crate::planetary::record::Surface::condensates), from P14.T54.a).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Condensates {
    entries: Vec<CondensateEntry>,
}

impl Condensates {
    /// The entries, in their order.
    #[must_use]
    pub fn entries(&self) -> &[CondensateEntry] {
        &self.entries
    }

    /// The entry of `substance` in `phase`, if there is one.
    #[must_use]
    pub fn entry(
        &self,
        substance: SubstanceId,
        phase: CondensatePhase,
    ) -> Option<&CondensateEntry> {
        self.entries
            .iter()
            .find(|e| e.substance == substance && e.phase == phase)
    }

    /// The liquids as `(SubstanceId, area fraction)`, in the entries' order: P14.T24.b's
    /// `surface_liquids`, whose sum is the ocean fraction.
    #[must_use]
    pub fn surface_liquids(&self) -> Vec<(SubstanceId, f64)> {
        self.areas(CondensatePhase::Liquid)
    }

    /// The solids as `(SubstanceId, area fraction)`, in the entries' order: P14.T24.b's
    /// `surface_ices`.
    #[must_use]
    pub fn surface_ices(&self) -> Vec<(SubstanceId, f64)> {
        self.areas(CondensatePhase::Solid)
    }

    /// The entries in `phase` as `(SubstanceId, area fraction)`.
    #[must_use]
    fn areas(&self, phase: CondensatePhase) -> Vec<(SubstanceId, f64)> {
        self.entries
            .iter()
            .filter(|e| e.phase == phase)
            .map(|e| (e.substance, e.area_fraction))
            .collect()
    }

    /// These condensates with the cold-trapped ices `traps` merged in (P14.T51.f): each trap that
    /// holds an area and a reservoir adds both to its substance's solid entry, or is inserted as
    /// one, `{ substance, Solid, reservoir, area_fraction, seasonal: false }`; then the entries are
    /// re-sorted in [`Partition::publish`]'s order and held to [`MAX_CONDENSATES`].
    ///
    /// A merged entry keeps its seasonal flag, its zonal part's, and its area is held to 1, though
    /// the traps lie only where the zonal part does not. Its layer is the sum of its parts' layers:
    /// its zonal part's, and the trap's reservoir over the trap's grain density at its threshold,
    /// the coldest it lies at ([`TrappedIce::threshold`]); it has none where either part has none.
    #[must_use]
    pub(crate) fn with_cold_traps(mut self, traps: &[TrappedIce]) -> Self {
        for trap in traps {
            let (mass, area) = (trap.reservoir.value(), trap.area_fraction);
            // A NaN holds none.
            if !(mass > 0.0 && area > 0.0) {
                continue;
            }
            let layer = trap
                .substance
                .substance()
                .grain_density(CondensedPhase::Solid, trap.threshold)
                .map(|rho| mass / rho.value());
            let found = self
                .entries
                .iter_mut()
                .find(|e| e.substance == trap.substance && e.phase == CondensatePhase::Solid);
            if let Some(entry) = found {
                let total = entry.area_fraction + area;
                // Held to 1, the zonal part whole and the trap the rest.
                let trapped = if total > 1.0 {
                    1.0 - entry.area_fraction
                } else {
                    area
                };
                entry.area_fraction = if total > 1.0 { 1.0 } else { total };
                entry.trapped_area_fraction += trapped;
                entry.reservoir = KilogramsPerSquareMetre::new(entry.reservoir.value() + mass);
                entry.reservoir_gel = match (entry.reservoir_gel, layer) {
                    (Some(zonal), Some(trapped)) => Some(Metres::new(zonal.value() + trapped)),
                    (None, _) | (_, None) => None,
                };
            } else {
                let held = if area > 1.0 { 1.0 } else { area };
                self.entries.push(CondensateEntry {
                    substance: trap.substance,
                    phase: CondensatePhase::Solid,
                    reservoir: trap.reservoir,
                    reservoir_gel: layer.map(Metres::new),
                    area_fraction: held,
                    seasonal: false,
                    trapped_area_fraction: held,
                });
            }
        }
        self.entries.sort_by(entry_order);
        self.entries.truncate(MAX_CONDENSATES);
        self
    }
}

/// One condensable's ice in a body's cold traps (P14.T51.f), as
/// [`Condensates::with_cold_traps`] merges it.
///
/// Plain data with crate-visible fields.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TrappedIce {
    /// The substance, its own row.
    pub(crate) substance: SubstanceId,
    /// Its reservoir over the body's whole surface, kg m⁻², finite and not negative.
    pub(crate) reservoir: KilogramsPerSquareMetre,
    /// The share of the surface its exposed ice covers, 0 to 1: the cold-trap fraction times the
    /// exposed fraction.
    pub(crate) area_fraction: f64,
    /// Its cold-trap threshold, the temperature its grain density is read at.
    pub(crate) threshold: Kelvin,
}

/// The order of the published entries: by reservoir mass, largest first, with ties by
/// `SubstanceId`, then phase.
#[must_use]
fn entry_order(a: &CondensateEntry, b: &CondensateEntry) -> Ordering {
    b.reservoir
        .total_cmp(&a.reservoir)
        .then(a.substance.cmp(&b.substance))
        .then(a.phase.cmp(&b.phase))
}

/// A condensed entry's mass and grain density, before the liquids share their basins.
#[derive(Debug, Clone, Copy)]
struct Part {
    substance: SubstanceId,
    phase: CondensatePhase,
    mass_kg_per_m2: f64,
    density_kg_per_m3: Option<f64>,
    area: f64,
    seasonal: bool,
}

impl Part {
    /// The entry it publishes, at the area `area`.
    #[must_use]
    fn entry(&self, area: f64) -> CondensateEntry {
        CondensateEntry {
            substance: self.substance,
            phase: self.phase,
            reservoir: KilogramsPerSquareMetre::new(self.mass_kg_per_m2),
            reservoir_gel: self
                .density_kg_per_m3
                .map(|rho| Metres::new(self.mass_kg_per_m2 / rho)),
            area_fraction: area,
            seasonal: self.seasonal,
            trapped_area_fraction: 0.0,
        }
    }
}

impl Partition {
    /// The published condensates of the partition over the cover `cover` (P14.T51.a's second
    /// part; decision-p14-t51a-part2 §3–4): each condensate's reservoir divided between its
    /// phases, each part with its global equivalent layer, its area and its own seasonal flag; at
    /// most [`MAX_CONDENSATES`] entries, the smallest reservoirs dropped beyond them.
    ///
    /// - **Water** is P14.T24.b's to the bit ([`WaterCover`]): its reservoir the surface water
    ///   `σ_s`, its ice T24.b's perennial ice at its sheet of [`ICE_SHEET_THICKNESS`], the whole
    ///   where T24.b's cap binds, and its liquid, where it lies as one, the rest, over the share of
    ///   the basins its own layer fills, none where the ice holds the whole
    ///   (decision-p14-t54a-water-consistency §1.4). Where `σ_s` is none, a body whose bulk is ice
    ///   ([`WaterCover::crust_water`]) lists its crust instead, as a solid alone: the partition's
    ///   whole frozen water at T24.b's ice area (decision-p14-t54a-ganymede, C1 follow-up §C1.4).
    /// - **Any other solid** covers its climate's cold area below its edge (r × its first melt under
    ///   air, r = 263.15 ÷ 273.16; airless, blended by its dominance with its net Hertz–Knudsen edge
    ///   at its own partial pressure; decision-p14-t51a-part2 §3.2–3.3), at most the area its
    ///   reservoir covers at [`ICE_SHEET_THICKNESS`] of its grain density at the temperature it
    ///   lies at, within its solid range. A solid whose density the registry has not read (argon,
    ///   and oxygen's β phase) is capped at its liquid's density at its triple point, 13–20% below
    ///   argon's solid (from memory) and some 7% below oxygen's, and has no layer.
    /// - **The split.** A reservoir that lies as a liquid and freezes in its coldest zones gives its
    ///   solid the mass of its cover, at most the whole, and its liquid the rest; one that lies as a
    ///   solid is solid whole, its area still capped, so that a frozen body's shell is its whole
    ///   reservoir (decision-p14-t51a-part2 §3.4 leaves no phase for the rest).
    /// - **Any other liquid** fills the basins ([`Basins::area_filled`]): those liquids' layers
    ///   together fill their share of the surface, divided between them by volume. A liquid's layer
    ///   is its mass over its liquid row's grain density ([`Condensate::liquid`]) at the
    ///   temperature it lies at, within its liquid range.
    /// - **A supercritical fluid** is listed with no reservoir and no area.
    ///
    /// A part with no mass is not listed.
    ///
    /// # Examples
    ///
    /// A Mars given its measured air by hand publishes its carbon dioxide's caps, their area the
    /// layer of their reservoir spread at 1.93 km of ice, and seasonal:
    ///
    /// ```
    /// use hyperion_sim::planetary::chem::partition::{
    ///     ClimateSpan, ColumnInventory, CondensatePhase, Cover, CrustWater, Ground, WaterCover,
    ///     partition,
    /// };
    /// use hyperion_sim::planetary::derive::atmosphere::{Gas, SurfaceState};
    /// use hyperion_sim::units::{
    ///     Kelvin, KilogramsPerCubicMetre, KilogramsPerSquareMetre, MetresPerSecondSquared, Pascals,
    /// };
    ///
    /// let co2 = Gas::CarbonDioxide.substance();
    /// let climate = ClimateSpan::new(Kelvin::new(147.0), Kelvin::new(214.0), Kelvin::new(242.0))?;
    /// let inventory = ColumnInventory::new([
    ///     (co2, Pascals::new(1_200.0)),
    ///     (Gas::Nitrogen.substance(), Pascals::new(11.0)),
    ///     (Gas::Argon.substance(), Pascals::new(12.0)),
    /// ])?;
    /// let ground = Ground::without_basins(MetresPerSecondSquared::new(3.72));
    /// let mars = partition(&climate, &ground, &inventory);
    /// // Its annual mean runs from 150 K at its poles to 225 K, by hand, and it holds no water.
    /// let cold_area = |t: Kelvin| ((t.value() - 150.0) / 75.0).clamp(0.0, 1.0);
    /// let water = WaterCover {
    ///     surface_water: KilogramsPerSquareMetre::ZERO,
    ///     potential_ocean: 0.0,
    ///     ice_fraction: 0.0,
    ///     ice_density: KilogramsPerCubicMetre::new(917.0),
    ///     crust_water: CrustWater::Unlisted,
    /// };
    /// let published = mars.publish(&Cover { cold_area: &cold_area, state: SurfaceState::Temperate, water });
    /// let caps = published.entry(co2, CondensatePhase::Solid).expect("its caps");
    /// let layer = caps.reservoir_gel().expect("solid CO2's density is read").value();
    /// assert!((caps.area_fraction() - layer / 1_930.0).abs() < 1e-12);
    /// assert!(caps.is_seasonal());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn publish(&self, cover: &Cover<'_>) -> Condensates {
        self.publish_with(cover, SolidEdges::Partition)
    }

    /// [`publish`](Self::publish) with the solids' edges `edges`: under
    /// [`SolidEdges::Partition`] it is `publish` to the bit, and under [`SolidEdges::Exosphere`]
    /// P14.T51.f's airless branch, which its hold at 24 keeps off the record until then.
    #[must_use]
    pub(crate) fn publish_with(&self, cover: &Cover<'_>, edges: SolidEdges) -> Condensates {
        let mut entries: Vec<CondensateEntry> = Vec::new();
        let mut liquids: Vec<Part> = Vec::new();
        for c in &self.condensates {
            let Some(diagram) = c.substance.substance().phase_diagram() else {
                continue;
            };
            if c.phase == CondensatePhase::Supercritical {
                entries.push(CondensateEntry {
                    substance: c.substance,
                    phase: CondensatePhase::Supercritical,
                    reservoir: KilogramsPerSquareMetre::ZERO,
                    reservoir_gel: Some(Metres::ZERO),
                    area_fraction: 0.0,
                    seasonal: false,
                    trapped_area_fraction: 0.0,
                });
                continue;
            }
            let [solid, liquid] =
                if c.substance == Gas::Water.substance() && edges.water_keeps_cover() {
                    water_parts(c, diagram, &cover.water, &self.ground.basins)
                } else {
                    let held = self
                        .of(c.substance)
                        .map_or((0.0, Pascals::ZERO), |s| (s.dominance, s.partial_pressure));
                    parts_of(c, diagram, (held, edges), cover)
                };
            if let Some(solid) = solid {
                entries.push(solid.entry(solid.area));
            }
            match liquid {
                Some(liquid) if c.substance == Gas::Water.substance() => {
                    entries.push(liquid.entry(liquid.area));
                }
                Some(liquid) => liquids.push(liquid),
                None => {}
            }
        }
        // Every liquid but water's shares the basins by volume.
        let depth = |p: &Part| {
            p.density_kg_per_m3
                .map_or(0.0, |rho| p.mass_kg_per_m2 / rho)
        };
        let volume = liquids.iter().fold(0.0, |sum, p| sum + depth(p));
        let filled = if volume > 0.0 {
            self.ground.basins.area_filled(Metres::new(volume))
        } else {
            0.0
        };
        for part in &liquids {
            let area = if volume > 0.0 {
                filled * depth(part) / volume
            } else {
                0.0
            };
            entries.push(part.entry(area));
        }
        entries.sort_by(entry_order);
        entries.truncate(MAX_CONDENSATES);
        Condensates { entries }
    }
}

/// The ice and the sea of water's condensate `c`, of `diagram`, over its cover `water` on the
/// basins `basins` (decision-p14-t51a-part2 §4.2): P14.T24.b's surface water and ice, the sea
/// the rest.
///
/// The ice takes its sheet first, and the whole surface water where T24.b's cap binds, so that
/// there is exactly no sea where the ice holds all the water (decision-p14-t54a-water-consistency
/// §1.4): the test is the expression `cover::ice_fraction_over` and the passes' water cover cap
/// with, `σ_s` ÷ (`ρ_ice` × [`ICE_SHEET_THICKNESS`]) at the cover's own density, so that rounding
/// leaves no sea of an ulp on either path. The sea's area is the share of the basins its own
/// layer fills ([`Basins::area_filled`]), as every other liquid's is, with no minimum depth: the
/// fill bounds it by its volume, continuously.
///
/// Where the surface water holds none, an ice bulk's crust gives the solid alone
/// ([`crust_ice`]).
#[must_use]
fn water_parts(
    c: &Condensate,
    diagram: &PhaseDiagram,
    water: &WaterCover,
    basins: &Basins,
) -> [Option<Part>; 2] {
    let reservoir = water.surface_water.value();
    // A NaN holds none.
    if reservoir.is_nan() || reservoir <= 0.0 {
        return [crust_ice(c, water), None];
    }
    let lies_as_liquid = c.phase == CondensatePhase::Liquid;
    let rho = water.ice_density.value();
    let solid_mass = if !c.phases.contains(CondensatePhase::Solid) {
        0.0
    } else if lies_as_liquid {
        let capped = water.ice_fraction >= reservoir / (rho * ICE_SHEET_THICKNESS.value());
        let sheet = water.ice_fraction * rho * ICE_SHEET_THICKNESS.value();
        if capped || sheet >= reservoir {
            reservoir
        } else {
            sheet
        }
    } else {
        reservoir
    };
    let solid = (solid_mass > 0.0).then_some(Part {
        substance: c.substance,
        phase: CondensatePhase::Solid,
        mass_kg_per_m2: solid_mass,
        density_kg_per_m3: (rho > 0.0).then_some(rho),
        area: water.ice_fraction,
        seasonal: c.is_seasonal_as(CondensatePhase::Solid),
    });
    let liquid_mass = reservoir - solid_mass;
    let liquid = (lies_as_liquid && liquid_mass > 0.0).then(|| {
        let density = liquid_density_kg_per_m3(c, diagram);
        Part {
            substance: c.liquid,
            phase: CondensatePhase::Liquid,
            mass_kg_per_m2: liquid_mass,
            density_kg_per_m3: density,
            area: density.map_or(0.0, |rho_l| {
                basins.area_filled(Metres::new(liquid_mass / rho_l))
            }),
            seasonal: c.is_seasonal_as(CondensatePhase::Liquid),
        }
    });
    [solid, liquid]
}

/// The solid of an ice bulk's crust, water's condensate `c` under its cover `water` where the
/// surface water holds none ([`WaterCover::crust_water`]; decision-p14-t54a-ganymede, C1 follow-up
/// §C1.4): on an ice bulk whose lid is `H2O` (or not known), the partition's whole frozen water,
/// the crust's ice-held water, at T24.b's ice area, its layer at the cover's ice density. None off
/// an ice bulk, and none under any other lid, where the crust's water stays in the bulk, unlisted,
/// until P14.T53.c divides it (§C3.1); none for water that does not lie as ice: a sea is T24.b's
/// surface water, on water's Jeans verdict. Its seasonal flag is its solid's.
#[must_use]
fn crust_ice(c: &Condensate, water: &WaterCover) -> Option<Part> {
    let mass = c.reservoir.value();
    // By comparison, so that a NaN reservoir holds none.
    let listed = water.crust_water == CrustWater::Listed;
    if !(listed && c.phase == CondensatePhase::Solid && mass > 0.0) {
        return None;
    }
    let rho = water.ice_density.value();
    Some(Part {
        substance: c.substance,
        phase: CondensatePhase::Solid,
        mass_kg_per_m2: mass,
        density_kg_per_m3: (rho > 0.0).then_some(rho),
        area: water.ice_fraction,
        seasonal: c.is_seasonal_as(CondensatePhase::Solid),
    })
}

/// The grain density, kg m⁻³, of the liquid row of the condensate `c` of `diagram` at the
/// temperature it lies at, held within its liquid range, from its first melt to its critical point.
#[must_use]
fn liquid_density_kg_per_m3(c: &Condensate, diagram: &PhaseDiagram) -> Option<f64> {
    let critical = diagram.critical_temperature();
    let at = if c.lies_at < c.first_melt {
        c.first_melt
    } else if c.lies_at > critical {
        critical
    } else {
        c.lies_at
    };
    c.liquid
        .substance()
        .grain_density(CondensedPhase::Liquid, at)
        .map(KilogramsPerCubicMetre::value)
}

/// The solid and liquid parts of the condensed reservoir `c` of `diagram`, held at the dominance
/// and the partial pressure `held`, with the solids' edges `edges`, over the cover `cover`, for
/// every condensable but water, and for water too under [`SolidEdges::Exosphere`] on a crust that
/// is not its own ice ([`Partition::publish_with`]'s split).
#[must_use]
fn parts_of(
    c: &Condensate,
    diagram: &PhaseDiagram,
    (held, edges): ((f64, Pascals), SolidEdges),
    cover: &Cover<'_>,
) -> [Option<Part>; 2] {
    let reservoir = c.reservoir.value();
    let lies_as_liquid = c.phase == CondensatePhase::Liquid;
    let row = c.substance.substance();
    let solid_at = if c.lies_at < c.first_melt {
        c.lies_at
    } else {
        c.first_melt
    };
    let solid_kg_per_m3 = row
        .grain_density(CondensedPhase::Solid, solid_at)
        .map(KilogramsPerCubicMetre::value);
    // A solid whose density was not read is capped at its liquid's at the triple point.
    let cap_kg_per_m3 = solid_kg_per_m3.or_else(|| {
        row.grain_density(CondensedPhase::Liquid, diagram.triple_temperature())
            .map(KilogramsPerCubicMetre::value)
    });
    let mut solid_mass = 0.0;
    let mut solid = None;
    if c.phases.contains(CondensatePhase::Solid) {
        let molar_mass = row.molar_mass_g_per_mol().expect(
            "a partition's condensable is a gas row with a molar mass, as its inventory holds",
        );
        let (line, sheet_holds_it) = match edges {
            SolidEdges::Partition => (
                ice_line((diagram, molar_mass), c.first_melt, held, cover.state),
                lies_as_liquid,
            ),
            SolidEdges::Exosphere { .. } => (
                vacuum_edge(row, (diagram, molar_mass), AIRLESS_ICE_LOSS_KG_PER_M2_GYR),
                true,
            ),
        };
        let cold = (cover.cold_area)(line);
        let cold = if cold.is_nan() {
            0.0
        } else {
            cold.clamp(0.0, 1.0)
        };
        let (mass, area) = match cap_kg_per_m3 {
            Some(rho) if rho > 0.0 => {
                let sheet = rho * ICE_SHEET_THICKNESS.value();
                let mass = if sheet_holds_it && cold * sheet < reservoir {
                    cold * sheet
                } else {
                    reservoir
                };
                let covers = mass / sheet;
                (mass, if cold < covers { cold } else { covers })
            }
            Some(_) | None => (if lies_as_liquid { 0.0 } else { reservoir }, cold),
        };
        solid_mass = mass;
        solid = (mass > 0.0).then_some(Part {
            substance: c.substance,
            phase: CondensatePhase::Solid,
            mass_kg_per_m2: mass,
            density_kg_per_m3: solid_kg_per_m3,
            area,
            seasonal: c.is_seasonal_as(CondensatePhase::Solid),
        });
    }
    let liquid_mass = reservoir - solid_mass;
    let liquid = (lies_as_liquid && liquid_mass > 0.0).then(|| Part {
        substance: c.liquid,
        phase: CondensatePhase::Liquid,
        mass_kg_per_m2: liquid_mass,
        density_kg_per_m3: liquid_density_kg_per_m3(c, diagram),
        area: 0.0,
        seasonal: c.is_seasonal_as(CondensatePhase::Liquid),
    });
    [solid, liquid]
}

#[cfg(test)]
mod tests;
