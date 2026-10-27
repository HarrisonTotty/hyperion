//! Escape by regime and fractionation (plan 14, P14.T53.a): its hold, and its pure laws, which
//! the atmosphere's derivation reads under the hold's batch rules alone.
//!
//! P14.T53.a's laws join the snapshot path at the 23 → 24 batch's bump (P14.T54.b). All of
//! P14.T53.a is held at generator version 24 (`ESCAPE_VERSION`; decision-bump-plan-after-22 §1.3,
//! and §5 "For 24": "chem/escape.rs ... stay pure, with the greenhouse refits behind a hold at
//! 24"), so that nothing on the record moves before that bump. Behind it:
//!
//! - the laws below, wired into
//!   [`derive::atmosphere`](crate::planetary::derive::atmosphere)'s steps (the shoreline, the thin
//!   envelope's air, the timed runaway oxygen, the weathering thermostat, the cold trap's failure,
//!   methane's outcome, the desiccated carbon dioxide and each element's outcome, on
//!   [`Atmosphere::escape`](crate::planetary::derive::atmosphere::Atmosphere::escape));
//! - the greenhouse's broadened carbon dioxide term and its refits, in the chain's order
//!   (`GreenhouseConstants::BATCH` in [`derive::atmosphere`](crate::planetary::derive::atmosphere):
//!   [`BROADENED_CARBON_DIOXIDE_BAND`], [`BROADENED_CARBON_DIOXIDE_EXPONENT`] and
//!   [`BROADENED_CARBON_DIOXIDE_COLLISIONS`], water split into its vapour and its cloud,
//!   [`BROADENED_GREENHOUSE_WATER_VAPOUR`] and [`BROADENED_GREENHOUSE_WATER_CLOUD`], and hydrogen's
//!   collision-induced term, [`BROADENED_GREENHOUSE_HYDROGEN`]; then Earth's meridional
//!   conductance, [`BROADENED_EARTH_MERIDIONAL_CONDUCTANCE_W_PER_M2_K`]);
//! - the greenhouse gain's weight of a held gas, (1 − x)(1 − d) with d the partition's dominance
//!   (decision-pre-23-review item 21), and each term's own slope ∂τᵢ ÷ ∂ln pᵢ in place of ½τᵢ, of
//!   the water vapour alone, not its cloud.
//!
//! # The laws
//!
//! Each law is a pure function of plain quantities, continuous in its inputs and, where it takes an
//! age or a fluence, monotone in it as its documentation states; an input that is not finite or
//! not positive gives the law's documented "nothing", never a number that is not one.
//! [`derive::atmosphere`](crate::planetary::derive::atmosphere) reads them behind the hold.
//!
//! - **The cosmic shoreline** (Zahnle and Catling 2017, ApJ 843, 122): the share of a body's bulk
//!   air lost to the non-thermal escape its X-ray and ultraviolet fluence drives
//!   ([`xuv_shoreline_share`]) and to impact erosion ([`impact_shoreline_share`]), timed by the
//!   lunar chronology ([`impact_chronology`]), together a [`Shoreline`].
//! - **Diffusion and fractionation**: a light gas's diffusion-limited flux through a heavier one
//!   ([`diffusion_limited_flux_per_m2_s`], Hunten 1973), the crossover mass and fractionation factor of
//!   escaping hydrogen ([`crossover_mass`], [`fractionation_factor`]; Hunten, Pepin and Walker
//!   1987), and the helium an envelope keeps as it loses its hydrogen ([`envelope_helium`]; Hu,
//!   Seager and Yung 2015; Malsky et al. 2023).
//! - **Oxygen**: a runaway world's water lost over the X-ray and ultraviolet history, with the
//!   oxygen its escaping hydrogen drags away, its melt takes and its surface sinks
//!   ([`runaway_oxygen_timed`]; Luger and Barnes 2015; Schaefer et al. 2016), and the oxygen of a
//!   temperate world whose cold trap fails for want of a non-condensing gas
//!   ([`cold_trap_failure_oxygen`]; Wordsworth and Pierrehumbert 2014).
//! - **Carbon**: methane's escape through nitrogen against its photolysis to hydrocarbons
//!   ([`methane_outcome`]), and a desiccated carbon dioxide air's photolysis under an M dwarf
//!   ([`desiccated_carbon_dioxide`]; Gao et al. 2015).
//! - **The thin envelope's air** ([`thin_envelope_air`]) and each element's outcome
//!   ([`ElementOutcome`], [`EscapeOutcome`]).
//!
//! The laws that integrate over time do so on one fixed grid of absolute times, the same for every
//! input: a first step from formation to 10⁶ yr, then steps between the nodes 10^(k/16) yr, sixteen
//! to the decade, the last cut at the age (to at most 10¹³ yr). Each step's result is continuous
//! in its length, so the outcome is continuous in the age.
//!
//! [`BROADENED_CARBON_DIOXIDE_BAND`]: crate::planetary::derive::atmosphere::BROADENED_CARBON_DIOXIDE_BAND
//! [`BROADENED_CARBON_DIOXIDE_EXPONENT`]: crate::planetary::derive::atmosphere::BROADENED_CARBON_DIOXIDE_EXPONENT
//! [`BROADENED_CARBON_DIOXIDE_COLLISIONS`]: crate::planetary::derive::atmosphere::BROADENED_CARBON_DIOXIDE_COLLISIONS
//! [`BROADENED_GREENHOUSE_WATER_VAPOUR`]: crate::planetary::derive::atmosphere::BROADENED_GREENHOUSE_WATER_VAPOUR
//! [`BROADENED_GREENHOUSE_WATER_CLOUD`]: crate::planetary::derive::atmosphere::BROADENED_GREENHOUSE_WATER_CLOUD
//! [`BROADENED_GREENHOUSE_HYDROGEN`]: crate::planetary::derive::atmosphere::BROADENED_GREENHOUSE_HYDROGEN
//! [`BROADENED_EARTH_MERIDIONAL_CONDUCTANCE_W_PER_M2_K`]: crate::planetary::hooks::surface::BROADENED_EARTH_MERIDIONAL_CONDUCTANCE_W_PER_M2_K

use core::f64::consts::PI;

use hyperion_surface::craters::{SOLAR_SYSTEM_AGE, lunar_chronology};

use crate::math;
use crate::planetary::context::{XUV_DECAY_INDEX, XuvHistory};
use crate::planetary::derive::atmosphere::{
    ATOMIC_MASS_CONSTANT_KG, EARTH_OXYGEN_SINK_MOL_PER_YEAR, ESCAPING_FLOW_TEMPERATURE, Gas,
    OXYGEN_IN_HYDROGEN_DIFFUSION_PER_METRE_SECOND, WATER_LOSS_ESCAPE_EFFICIENCY,
};
use crate::planetary::params::THIN_ENVELOPE_FRACTION;
use crate::stellar::Composition;
use crate::stellar::composition::Z_SOLAR;
use crate::substance::Element;
use crate::units::consts::{
    BOLTZMANN_CONSTANT, EARTH_RADIUS_M, GRAVITATIONAL_CONSTANT, SECONDS_PER_JULIAN_YEAR,
    SOLAR_CONSTANT_W_PER_M2,
};
use crate::units::{
    Gigayears, JoulesPerSquareMetre, Kelvin, Kilograms, Metres, MetresPerSecond,
    MetresPerSecondSquared, Pascals, WattsPerSquareMetre, Years,
};
use crate::{GENERATOR_VERSION, GeneratorVersion};

/// The generator version from which P14.T53.a is on the record: 24, the 23 → 24 batch's bump,
/// P14.T54.b (decision-bump-plan-after-22 §1.3 and §5, "For 24").
///
/// It is P14.T53.a's one hold. Below it [`EscapeRules::in_force`] is [`EscapeRules::Held`], so
/// that every figure on the record is version 23's bit for bit. P14.T54.b deletes this hold,
/// [`EscapeRules`] and its `Held` arm, and its tripwire (`the_escape_is_held_only_until_the_24_bump`).
pub(crate) const ESCAPE_VERSION: GeneratorVersion = GeneratorVersion::new(24);

/// Which rules the greenhouse and P14.T53.a's escape follow (the escape's hold,
/// [`ESCAPE_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum EscapeRules {
    /// Versions 22's and 23's: P14.T13.c's √p terms at P14.T57.a's constants, the greenhouse
    /// gain's (1 − x) ½τᵢ, and P14.T24.a's D⊕.
    Held,
    /// The 23 → 24 batch's: P14.T53.a's broadened carbon dioxide term with its refits, and the
    /// gain's (1 − x)(1 − d) ∂τᵢ ÷ ∂ln pᵢ.
    Batch,
}

impl EscapeRules {
    /// The rules this generator version gives: the batch's from [`ESCAPE_VERSION`], and versions
    /// 22's and 23's below it. A test may set them on its thread ([`with_rules`]).
    #[must_use]
    pub(crate) fn in_force() -> Self {
        #[cfg(test)]
        if let Some(rules) = RULES.with(core::cell::Cell::get) {
            return rules;
        }
        if GENERATOR_VERSION >= ESCAPE_VERSION {
            Self::Batch
        } else {
            Self::Held
        }
    }
}

#[cfg(test)]
thread_local! {
    /// The rules a test set on this thread ([`with_rules`]), consulted before the version: tests
    /// run on threads of one process.
    static RULES: core::cell::Cell<Option<EscapeRules>> = const { core::cell::Cell::new(None) };
}

/// `run`'s result under the rules `rules` on this thread ([`EscapeRules::in_force`]), with the
/// Rayleigh floor's hold (P14.T24.h.a, `RAYLEIGH_FLOOR_VERSION`) switched with them, since version
/// 24 lifts both: [`EscapeRules::Batch`] floors the snapshot path's Bond albedo
/// (`RayleighFloorRules::Floored`) and [`EscapeRules::Held`] does not (`RayleighFloorRules::Held`),
/// so that a test under either reads that version's whole state (decision-bump-plan-after-22 V3.2).
/// The rules before are set back after, a panic in `run` included, as the partition's test-only
/// switch of its exits is (`partition::with_exits`). The floor's own tests switch it alone
/// (`area_mix::with_rayleigh_floor`).
#[cfg(test)]
pub(crate) fn with_rules<R>(rules: EscapeRules, run: impl FnOnce() -> R) -> R {
    use crate::planetary::derive::photometry::area_mix::{RayleighFloorRules, with_rayleigh_floor};
    /// Sets the thread's escape rules back as it drops.
    struct Restore(Option<EscapeRules>);
    impl Drop for Restore {
        fn drop(&mut self) {
            RULES.with(|cell| cell.set(self.0));
        }
    }
    let floor = match rules {
        EscapeRules::Held => RayleighFloorRules::Held,
        EscapeRules::Batch => RayleighFloorRules::Floored,
    };
    let _restore = Restore(RULES.with(|cell| cell.replace(Some(rules))));
    with_rayleigh_floor(floor, run)
}

/// The generator version from which every envelope's helium starts at its host star's own
/// primordial He/H ([`primordial_helium_per_hydrogen`]), not the protosolar value (P14.T54.b.i;
/// decision-p14-t50b-envelope §5): 24.
///
/// It moves the thin envelopes' helium under the escape's batch, the fractionation's start
/// (`helium_before_passes`), and the He/H P14.T50.b's envelope reads, so it has its own arm, for
/// P14.T54.b.k's attribution, rather than an arm of [`EscapeRules`]. Below it
/// [`PrimordialHeliumRules::in_force`] is [`PrimordialHeliumRules::Protosolar`]; under the escape's
/// hold no start is computed at all, so version 23's figures keep their bits. P14.T54.b.l deletes
/// it, [`PrimordialHeliumRules`] with its `Protosolar` arm, `with_primordial_helium`, its line in
/// `with_every_24_hold` and its tripwire
/// (`the_host_s_primordial_helium_is_held_only_until_the_24_bump`).
pub(crate) const PRIMORDIAL_HELIUM_VERSION: GeneratorVersion = GeneratorVersion::new(24);

/// Which helium an envelope forms with (the hold, [`PRIMORDIAL_HELIUM_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum PrimordialHeliumRules {
    /// Versions 23's and before: the protosolar He/H, 0.0968, about every host
    /// (`protosolar_helium_mass_fraction`).
    Protosolar,
    /// The 23 → 24 batch's: the host star's own ([`primordial_helium_per_hydrogen`]).
    Host,
}

impl PrimordialHeliumRules {
    /// The rules this generator version gives: the host's helium from
    /// [`PRIMORDIAL_HELIUM_VERSION`], the protosolar below it; in the tests, the rules a test's
    /// switch sets on its thread (`with_primordial_helium`).
    #[must_use]
    pub(crate) fn in_force() -> Self {
        #[cfg(test)]
        if let Some(rules) = PRIMORDIAL_HELIUM_RULES.with(core::cell::Cell::get) {
            return rules;
        }
        if GENERATOR_VERSION >= PRIMORDIAL_HELIUM_VERSION {
            Self::Host
        } else {
            Self::Protosolar
        }
    }
}

#[cfg(test)]
thread_local! {
    /// The primordial helium's rules a test sets on its thread, `None` for the version's.
    static PRIMORDIAL_HELIUM_RULES: core::cell::Cell<Option<PrimordialHeliumRules>> =
        const { core::cell::Cell::new(None) };
}

/// `run`'s result with the primordial helium's rules `rules` on this thread
/// ([`PrimordialHeliumRules::in_force`]), for the tests to reach 24's start ahead of the bump; the
/// rules before are set back after, a panic in `run` included. A system derives lazily, so a test
/// generates and reads it inside the switch. P14.T54.b.l deletes it with the hold.
#[cfg(test)]
pub(crate) fn with_primordial_helium<R>(
    rules: PrimordialHeliumRules,
    run: impl FnOnce() -> R,
) -> R {
    /// Sets the thread's rules back as it drops.
    struct Restore(Option<PrimordialHeliumRules>);
    impl Drop for Restore {
        fn drop(&mut self) {
            PRIMORDIAL_HELIUM_RULES.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(PRIMORDIAL_HELIUM_RULES.with(|cell| cell.replace(Some(rules))));
    run()
}

/// The smooth step 3u² − 2u³ of `u` held to [0, 1], continuous with its slope at both ends: 0 at
/// and below 0, 1 at and above 1, and 0 for a `u` that is not a number. Every ramp of the laws is
/// this one, as P14.T51.a's [`dominance`](super::partition::dominance) is (the lane's).
#[must_use]
fn smoothstep(u: f64) -> f64 {
    if u.is_nan() || u <= 0.0 {
        0.0
    } else if u >= 1.0 {
        1.0
    } else {
        u * u * (3.0 - 2.0 * u)
    }
}

/// The exponent k of the time grid's first node, 10^(k/16) yr: 96, so 10⁶ yr.
const GRID_FIRST_NODE: u32 = 96;

/// The exponent k of the time grid's last node: 208, so 10¹³ yr, beyond any system's age.
const GRID_LAST_NODE: u32 = 208;

/// The time grid's nodes to the decade: 16.
const GRID_NODES_PER_DECADE: f64 = 16.0;

/// The steps (start, end) of the laws' fixed time grid up to `age`, in years: [0, 10⁶], then
/// [10^((k − 1)/16), 10^(k/16)], the last cut at `age` and none beyond 10¹³ yr ([the module
/// documentation](self)).
fn grid_steps(age: f64) -> impl Iterator<Item = (f64, f64)> {
    let node = |k: u32| math::exp10(f64::from(k) / GRID_NODES_PER_DECADE);
    (GRID_FIRST_NODE..=GRID_LAST_NODE)
        .map(move |k| {
            let start = if k == GRID_FIRST_NODE {
                0.0
            } else {
                node(k - 1)
            };
            (start, node(k).min(age))
        })
        .take_while(move |&(start, _)| start < age)
}

/// The mass of a hydrogen atom, kg: half the registry's H₂ molecule, as
/// [`oxygen_escape_parameter`](crate::planetary::derive::atmosphere::oxygen_escape_parameter)
/// counts it.
#[must_use]
fn hydrogen_atom_mass() -> f64 {
    0.5 * Gas::Hydrogen.molecular_mass().value()
}

/// `value` where it is finite and positive.
#[must_use]
fn positive(value: f64) -> Option<f64> {
    (value.is_finite() && value > 0.0).then_some(value)
}

/// A body's mass and radius, kg and m, where both are finite and positive.
#[must_use]
fn bulk(mass: Kilograms, radius: Metres) -> Option<(f64, f64)> {
    Some((positive(mass.value())?, positive(radius.value())?))
}

/// The oxygen's mass in a mass of water, M(O₂) ÷ 2 M(H₂O) with the registry's molar masses, as
/// [`runaway_oxygen`](crate::planetary::derive::atmosphere::runaway_oxygen) counts it: about 0.888,
/// Luger and Barnes's 8 ÷ 9.
#[must_use]
fn oxygen_per_water() -> f64 {
    Gas::Oxygen.molar_mass_g_per_mol() / (2.0 * Gas::Water.molar_mass_g_per_mol())
}

/// A sink of `mol_per_m2_year` mol of O₂ per square metre per year over the surface of a body of
/// radius `radius` m, in kg yr⁻¹; none for a rate that is not a number or not positive.
#[must_use]
fn oxygen_sink_kg_per_year(mol_per_m2_year: f64, radius: f64) -> f64 {
    if mol_per_m2_year.is_nan() || mol_per_m2_year <= 0.0 {
        return 0.0;
    }
    mol_per_m2_year * 4.0 * PI * radius * radius * Gas::Oxygen.molar_mass_g_per_mol() * 1e-3
}

/// A body's X-ray and ultraviolet exposure over its history: the orbit-averaged flux `F_s` it
/// receives while its hosts are saturated, for their saturation time `t_sat`, then
/// `F_s` (t ÷ `t_sat`)^−1.5 ([`XUV_DECAY_INDEX`]): an [`XuvHistory`] seen from a fixed orbit.
///
/// Its fluence is the history's closed form, `F_s` t while saturated and `F_s` `t_sat` (3 − 2
/// √(`t_sat` ÷ t)) after ([`XuvHistory::fluence`]), so that a law reading it over a span of time
/// takes the energy delivered in that span exactly.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct XuvExposure {
    saturated: WattsPerSquareMetre,
    saturation_time: Years,
}

impl XuvExposure {
    /// An exposure to nothing, a dark host's: no flux, ever.
    pub const NONE: Self = Self {
        saturated: WattsPerSquareMetre::ZERO,
        saturation_time: Years::new(1.0e8),
    };

    /// The exposure to the saturated flux `saturated` for `saturation_time`, then its decline. A
    /// flux or a saturation time that is not finite and positive gives [`NONE`](Self::NONE).
    #[must_use]
    pub fn new(saturated: WattsPerSquareMetre, saturation_time: Years) -> Self {
        match (
            positive(saturated.value()),
            positive(saturation_time.value()),
        ) {
            (Some(_), Some(_)) => Self {
                saturated,
                saturation_time,
            },
            _ => Self::NONE,
        }
    }

    /// The exposure of a body on an orbit of semi-major axis `semi_major_axis` and eccentricity
    /// `eccentricity` about the star of `history`: its saturated flux there
    /// ([`XuvHistory::saturated_flux`]) for the star's saturation time.
    #[must_use]
    pub fn of(history: &XuvHistory, semi_major_axis: Metres, eccentricity: f64) -> Self {
        Self::new(
            history.saturated_flux(semi_major_axis, eccentricity),
            history.saturation_time(),
        )
    }

    /// The flux while the hosts are saturated.
    #[must_use]
    pub const fn saturated(&self) -> WattsPerSquareMetre {
        self.saturated
    }

    /// How long the hosts stay saturated.
    #[must_use]
    pub const fn saturation_time(&self) -> Years {
        self.saturation_time
    }

    /// The flux at `age`: `F_s` until the saturation time, then `F_s` (t ÷ `t_sat`)^−1.5; zero
    /// before birth and at an age that is not a number. Non-increasing in `age`.
    #[must_use]
    pub fn flux_at(&self, age: Years) -> WattsPerSquareMetre {
        let (t, t_sat) = (age.value(), self.saturation_time.value());
        if t.is_nan() || t <= 0.0 {
            WattsPerSquareMetre::ZERO
        } else if t <= t_sat {
            self.saturated
        } else {
            WattsPerSquareMetre::new(
                self.saturated.value() * math::powf(t / t_sat, -XUV_DECAY_INDEX),
            )
        }
    }

    /// The energy per unit area received from `from` to `to`: zero for a span that is empty or
    /// reversed, and the whole history's, 3 `F_s` `t_sat`, from formation to an infinite age.
    /// Continuous and non-decreasing in `to`.
    #[must_use]
    pub fn fluence_between(&self, from: Years, to: Years) -> JoulesPerSquareMetre {
        let span = self.saturated_years(to.value()) - self.saturated_years(from.value());
        if span > 0.0 {
            JoulesPerSquareMetre::new(self.saturated.value() * span * SECONDS_PER_JULIAN_YEAR)
        } else {
            JoulesPerSquareMetre::ZERO
        }
    }

    /// The flux's integral from formation to `years`, in years at the saturated flux: t while
    /// saturated, then `t_sat` (3 − 2 √(`t_sat` ÷ t)); zero before birth.
    #[must_use]
    fn saturated_years(&self, years: f64) -> f64 {
        let t_sat = self.saturation_time.value();
        if years.is_nan() || years <= 0.0 {
            0.0
        } else if years <= t_sat {
            years
        } else {
            t_sat * (3.0 - 2.0 * (t_sat / years).sqrt())
        }
    }
}

/// The cosmic shoreline's X-ray and ultraviolet fluence, over Earth's, at the reference escape
/// speed [`SHORELINE_REFERENCE_ESCAPE_SPEED`]: 0.66.
///
/// Zahnle and Catling (2017, ApJ 843, 122, arXiv:1702.03386, read) find the Solar System's bodies
/// with air and those without divided by a line I ∝ `v_esc`⁴ ("The dividing line goes as I ∝
/// `v_esc`⁴", abstract; §2 and Fig. 1, "drawn by eye"), and "the shoreline also appears in XUV with
/// the same power law" (§1), against the cumulative X-ray and ultraviolet fluence normalised to the
/// Sun's whole history (their Fig. 2 and eq. 27), the axis this term reads. The line is read off
/// their Fig. 1 as I ≈ 0.66 (v ÷ 5 km s⁻¹)⁴, decision-p14-t54a-ganymede's science check, which
/// fitted 0.66 (v ÷ 5)^4.07 to it; the index taken is the paper's, [`SHORELINE_INDEX`]. On it Titan
/// lies some 0.67 dex and Ganymede 0.21 dex on the air side, and Callisto on the line.
pub const SHORELINE_FLUENCE_AT_REFERENCE: f64 = 0.66;

/// The escape speed at which the shoreline's fluence is [`SHORELINE_FLUENCE_AT_REFERENCE`]:
/// 5 km s⁻¹ (Zahnle and Catling 2017, Fig. 1, as that constant reads it).
pub const SHORELINE_REFERENCE_ESCAPE_SPEED: MetresPerSecond = MetresPerSecond::new(5.0e3);

/// The cosmic shoreline's index in the escape speed: I ∝ `v_esc`⁴ (Zahnle and Catling 2017,
/// abstract and §2).
pub const SHORELINE_INDEX: f64 = 4.0;

/// The shoreline's half-width, dex: 0.2, the line's uncertainty of ±0.1–0.2 dex read from Zahnle
/// and Catling's (2017) Fig. 1 (decision-p14-t54a-ganymede, C0 follow-up, item 1), across which a
/// body's air goes from kept to lost ([`xuv_shoreline_share`]).
pub const SHORELINE_HALF_WIDTH_DEX: f64 = 0.2;

/// How far a body lies from the cosmic shoreline, dex: log₁₀(F ÷ 0.66 (v ÷ 5 km s⁻¹)⁴), with F
/// `fluence_ratio`, its X-ray and ultraviolet fluence over Earth's
/// ([`earth_xuv_fluence`](crate::planetary::derive::atmosphere::earth_xuv_fluence)), and v its
/// `escape_speed` ([`SHORELINE_FLUENCE_AT_REFERENCE`]). Positive on the airless side.
///
/// −∞ for a fluence that is not positive, an infinite escape speed, or an input that is not a
/// number (no loss); +∞ for an escape speed that is not positive, or an infinite fluence.
#[must_use]
pub fn shoreline_distance_dex(fluence_ratio: f64, escape_speed: MetresPerSecond) -> f64 {
    let speed = escape_speed.value();
    if fluence_ratio.is_nan() || speed.is_nan() || fluence_ratio <= 0.0 {
        return f64::NEG_INFINITY;
    }
    if speed > 0.0 && speed.is_infinite() {
        return f64::NEG_INFINITY;
    }
    if speed <= 0.0 || fluence_ratio.is_infinite() {
        return f64::INFINITY;
    }
    let line = SHORELINE_FLUENCE_AT_REFERENCE
        * math::powf(
            speed / SHORELINE_REFERENCE_ESCAPE_SPEED.value(),
            SHORELINE_INDEX,
        );
    math::log10(fluence_ratio / line)
}

/// The share of a body's bulk air lost to the non-thermal escape its X-ray and ultraviolet fluence
/// drives, a body lying `distance_dex` from the cosmic shoreline ([`shoreline_distance_dex`]): the
/// smooth step 3u² − 2u³ of u = (s + w) ÷ 2w held to [0, 1], w [`SHORELINE_HALF_WIDTH_DEX`], so 0
/// below −w, ½ on the line and 1 above +w (the lane's ramp across the line's uncertainty). A
/// distance that is not a number loses nothing.
///
/// # Examples
///
/// Callisto, at about 2.44 km s⁻¹ and Jupiter's 0.037 of Earth's fluence, lies on the line, and
/// Titan, at about 2.64 km s⁻¹ and Saturn's 0.011, well on the air side:
///
/// ```
/// use hyperion_sim::planetary::chem::escape::{shoreline_distance_dex, xuv_shoreline_share};
/// use hyperion_sim::units::MetresPerSecond;
///
/// let share = |fluence: f64, km_s: f64| {
///     xuv_shoreline_share(shoreline_distance_dex(fluence, MetresPerSecond::new(km_s * 1e3)))
/// };
/// assert!((share(0.037, 2.44) - 0.5).abs() < 0.05);
/// assert!(share(0.0109, 2.64) <= 0.0);
/// ```
#[must_use]
pub fn xuv_shoreline_share(distance_dex: f64) -> f64 {
    smoothstep((distance_dex + SHORELINE_HALF_WIDTH_DEX) / (2.0 * SHORELINE_HALF_WIDTH_DEX))
}

/// The ratios of impact speed to escape speed across which impact erosion goes from keeping a
/// body's air to annihilating it: 4 and 5. Zahnle and Catling (2017, §5): impact erosion "fits to a
/// `v_imp` ≈ 4 − 5 `v_esc` shoreline" (abstract), and, "when it gets the upper hand, will
/// annihilate the atmosphere".
pub const IMPACT_SHORELINE_RATIOS: (f64, f64) = (4.0, 5.0);

/// The mean speed at which a planet is struck, √(`v_orb`² + `v_esc`²), from its orbital speed
/// `orbital_speed` about its star and its escape speed `escape_speed`.
///
/// It is Zahnle and Catling's (2017) eq. 41, `v_imp`² = `v_enc`² + `v_esc`², with the encounter
/// speed `v_enc` = `v_orb`, their convention for every exoplanet of their Fig. 4. In the inner
/// Solar System encounter speeds are 0.5–1.0 `v_orb`, the higher for comets (Bottke et al. 1995, as
/// their §5 quotes them). A speed that is not positive, or not a number, counts as none.
#[must_use]
pub fn planet_impact_speed(
    orbital_speed: MetresPerSecond,
    escape_speed: MetresPerSecond,
) -> MetresPerSecond {
    let (orbit, escape) = (
        orbital_speed.value().max(0.0),
        escape_speed.value().max(0.0),
    );
    MetresPerSecond::new(math::hypot(orbit, escape))
}

/// The mean encounter speed of an ecliptic comet with a planet, as a share of the planet's orbital
/// speed: 0.34 (Zahnle, Schenk, Levison and Dones 2003, Icarus 163, 263, read, §3: "The average of
/// these is ⟨v∞⟩ ≈ 0.34 `V_orb`", a fit to Levison and Duncan's 1997 encounter speeds with
/// Jupiter).
pub const MOON_IMPACTOR_SPEED_SHARE: f64 = 0.34;

/// The mean speed at which a moon is struck, √(3 `v_orb`² + v∞² + `v_esc`²), from its orbital speed
/// `orbital_speed_about_parent` about its planet, the planet's orbital speed
/// `parent_orbital_speed_about_star` `V_p` about its star and the moon's escape speed
/// `escape_speed`, with v∞ = [`MOON_IMPACTOR_SPEED_SHARE`] `V_p`.
///
/// It is Zahnle et al.'s (2003) eq. 1, after Lissauer et al. (1988): "In general `v_orb` is by far
/// the largest term", and "The more complete Monte Carlo estimate (ZDL98) of ⟨`v_i`⟩ is about 5%
/// higher". Their Table 1 gives Ganymede 20, Callisto 15 and Titan 10.5 km s⁻¹, which this gives
/// within 3% (tested). A speed that is not positive, or not a number, counts as none.
#[must_use]
pub fn moon_impact_speed(
    orbital_speed_about_parent: MetresPerSecond,
    parent_orbital_speed_about_star: MetresPerSecond,
    escape_speed: MetresPerSecond,
) -> MetresPerSecond {
    let orbit = orbital_speed_about_parent.value().max(0.0);
    let infinity = MOON_IMPACTOR_SPEED_SHARE * parent_orbital_speed_about_star.value().max(0.0);
    let escape = escape_speed.value().max(0.0);
    MetresPerSecond::new((3.0 * orbit * orbit + infinity * infinity + escape * escape).sqrt())
}

/// The share of a body's bulk air that impact erosion takes once it has received all its impacts,
/// at the ratio `ratio` of its impact speed to its escape speed: the smooth step 3u² − 2u³ of u =
/// (ln r − ln 4) ÷ (ln 5 − ln 4) held to [0, 1], 0 up to 4 and 1 from 5
/// ([`IMPACT_SHORELINE_RATIOS`]; the ramp is the lane's). A ratio that is not positive, or not a
/// number, takes nothing.
#[must_use]
pub fn impact_shoreline_share(ratio: f64) -> f64 {
    if ratio.is_nan() || ratio <= 0.0 {
        return 0.0;
    }
    let (low, high) = IMPACT_SHORELINE_RATIOS;
    let (low, high) = (math::ln(low), math::ln(high));
    smoothstep((math::ln(ratio) - low) / (high - low))
}

/// The span over which a body receives its impacts, from its formation: 4.5 Gyr, the lunar
/// chronology's from 4.5 Ga ([`impact_chronology`]).
pub const IMPACT_CHRONOLOGY_SPAN: Gigayears = Gigayears::new(4.5);

/// The share of its impacts a body has received by `age` from its formation, on the lunar
/// chronology: c(t) = \[N(4.5) − N(4.5 − t)\] ÷ N(4.5) up to [`IMPACT_CHRONOLOGY_SPAN`], and 1
/// after.
///
/// N(T) = 5.44 × 10⁻¹⁴ (e^(6.93 T) − 1) + 8.38 × 10⁻⁴ T km⁻², T in Gyr before the present, is
/// Neukum, Ivanov and Hartmann's (2001, Space Sci. Rev. 96, 55, eq. 5) cumulative density of
/// craters over 1 km, as the surface's [`lunar_chronology`] counts it (Iqbal et al. 2019 quote it
/// so), with N(4.5) = 1.905 km⁻². The curve is calibrated by dated lunar samples only to about
/// 4.1 Ga, so its first 0.4 Gyr are an extrapolation, in which N(4.5) is some 30 times N(4.0).
/// The chronology's own tail, linear in T, would carry the share past 1 after 4.5 Gyr; it is held
/// at 1. It gives 0.499, 0.967, 0.9975, 0.9989 and 1 at 0.1, 0.5, 1, 2 and 4.5 Gyr (tested):
/// nearly all of a body's impacts come in its first few hundred million years.
///
/// Zero for an age that is not positive or not a number. Continuous and non-decreasing in `age`.
#[must_use]
pub fn impact_chronology(age: Years) -> f64 {
    let t = Gigayears::from(age).value();
    let span = IMPACT_CHRONOLOGY_SPAN.value();
    if t.is_nan() || t <= 0.0 {
        return 0.0;
    }
    if t >= span {
        return 1.0;
    }
    let density = |before: f64| lunar_chronology(Gigayears::new(before), SOLAR_SYSTEM_AGE).value();
    let whole = density(span);
    ((whole - density(span - t)) / whole).clamp(0.0, 1.0)
}

/// The cosmic shoreline's verdict on a body's bulk air, every gas but an ice-held one: the share
/// its X-ray and ultraviolet fluence takes ([`xuv_shoreline_share`]), the share impact erosion
/// takes once every impact has come ([`impact_shoreline_share`]), and the share of its impacts it
/// has received ([`impact_chronology`]).
///
/// The ice-held volatiles a body's ice keeps (decision-p14-t54a-ganymede, C0 follow-up, rule (b))
/// are not air, and keep their own rule.
#[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd)]
pub struct Shoreline {
    xuv: f64,
    impact: f64,
    chronology: f64,
}

impl Shoreline {
    /// The verdict that keeps all: nothing lost, the default.
    pub const NONE: Self = Self {
        xuv: 0.0,
        impact: 0.0,
        chronology: 0.0,
    };

    /// The verdict of the three shares, each held to [0, 1], one that is not a number taken as 0.
    #[must_use]
    pub fn new(xuv: f64, impact: f64, chronology: f64) -> Self {
        let share = |x: f64| if x.is_nan() { 0.0 } else { x.clamp(0.0, 1.0) };
        Self {
            xuv: share(xuv),
            impact: share(impact),
            chronology: share(chronology),
        }
    }

    /// The verdict on a body at the X-ray and ultraviolet fluence `fluence_ratio` over Earth's,
    /// escape speed `escape_speed`, mean impact speed `impact_speed`
    /// ([`planet_impact_speed`], [`moon_impact_speed`]) and age `age`. An escape speed that is not
    /// positive gives impact erosion the whole air.
    ///
    /// # Examples
    ///
    /// Earth keeps its air; Mercury, at 6.7 times Earth's fluence and 4.25 km s⁻¹, keeps none:
    ///
    /// ```
    /// use hyperion_sim::planetary::chem::escape::{Shoreline, planet_impact_speed};
    /// use hyperion_sim::units::{MetresPerSecond, Years};
    ///
    /// let km_s = |v: f64| MetresPerSecond::new(v * 1e3);
    /// let age = Years::new(4.57e9);
    /// let earth = Shoreline::of(1.0, km_s(11.19), planet_impact_speed(km_s(29.78), km_s(11.19)), age);
    /// assert!(earth.kept() > 1.0 - 1e-9);
    /// let mercury = Shoreline::of(6.68, km_s(4.25), planet_impact_speed(km_s(47.36), km_s(4.25)), age);
    /// assert!(mercury.kept() < 1e-9);
    /// ```
    #[must_use]
    pub fn of(
        fluence_ratio: f64,
        escape_speed: MetresPerSecond,
        impact_speed: MetresPerSecond,
        age: Years,
    ) -> Self {
        let ratio = if escape_speed.value() > 0.0 {
            impact_speed.value() / escape_speed.value()
        } else {
            f64::INFINITY
        };
        Self::new(
            xuv_shoreline_share(shoreline_distance_dex(fluence_ratio, escape_speed)),
            impact_shoreline_share(ratio),
            impact_chronology(age),
        )
    }

    /// The share its X-ray and ultraviolet fluence takes.
    #[must_use]
    pub const fn xuv(&self) -> f64 {
        self.xuv
    }

    /// The share impact erosion takes once every impact has come.
    #[must_use]
    pub const fn impact(&self) -> f64 {
        self.impact
    }

    /// The share of its impacts the body has received.
    #[must_use]
    pub const fn chronology(&self) -> f64 {
        self.chronology
    }

    /// The share of the bulk air that survives: (1 − xuv)(1 − impact × chronology).
    #[must_use]
    pub fn kept(&self) -> f64 {
        (1.0 - self.xuv) * (1.0 - self.impact * self.chronology)
    }
}

/// A binary diffusion coefficient b = n D, m⁻¹ s⁻¹, as b₀ (T ÷ `T₀`)^s: the product of the number
/// density and the diffusion coefficient, which depends on the temperature alone.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct BinaryDiffusion {
    per_metre_second: f64,
    reference: Kelvin,
    index: f64,
}

impl BinaryDiffusion {
    /// The coefficient `per_metre_second` m⁻¹ s⁻¹ at the temperature `reference`, scaling as
    /// T^`index`. One m⁻¹ s⁻¹ is 10⁻² cm⁻¹ s⁻¹, the unit the sources print.
    ///
    /// # Panics
    ///
    /// If `reference` is not finite and positive: every coefficient is the module's own constant,
    /// so a bad reference is a bug (in a constant's evaluation, a compile error).
    #[must_use]
    pub(crate) const fn new(per_metre_second: f64, reference: Kelvin, index: f64) -> Self {
        assert!(
            reference.value().is_finite() && reference.value() > 0.0,
            "a binary diffusion coefficient's reference temperature is finite and positive"
        );
        Self {
            per_metre_second,
            reference,
            index,
        }
    }

    /// The coefficient at `temperature`, m⁻¹ s⁻¹; zero at a temperature that is not positive, or
    /// not a number.
    #[must_use]
    pub fn per_metre_second_at(&self, temperature: Kelvin) -> f64 {
        let t = temperature.value();
        if t.is_nan() || t <= 0.0 {
            return 0.0;
        }
        self.per_metre_second * math::powf(t / self.reference.value(), self.index)
    }
}

/// Atomic hydrogen in helium: b = 1.04 × 10¹⁸ T^0.732 cm⁻¹ s⁻¹ (Mason and Marrero 1970, Adv. At.
/// Mol. Phys. 6, 155, as Hu, Seager and Yung 2015, ApJ 807, 8, §2.1, and Malsky et al. 2023, Nat.
/// Astron. 7, 57, eq. A3, both read, give it), so 1.04 × 10²⁰ T^0.732 m⁻¹ s⁻¹.
pub const HYDROGEN_HELIUM_DIFFUSION: BinaryDiffusion =
    BinaryDiffusion::new(1.04e20, Kelvin::new(1.0), 0.732);

/// Molecular hydrogen in nitrogen: b ≈ 1.5 × 10¹⁹ (T ÷ 300 K)^0.75 cm⁻¹ s⁻¹ (Zahnle and Catling
/// 2017, before their eq. 38, read). It gives D = b ÷ n = 0.61 cm² s⁻¹ at 300 K and one atmosphere,
/// some 20% below the 0.78 cm² s⁻¹ measured (from memory, recorded).
pub const HYDROGEN_NITROGEN_DIFFUSION: BinaryDiffusion =
    BinaryDiffusion::new(1.5e21, Kelvin::new(300.0), 0.75);

/// Methane in nitrogen: b = 7.34 × 10¹⁶ T^0.75 cm⁻¹ s⁻¹ (Marrero and Mason 1972, J. Phys. Chem.
/// Ref. Data 1, 3, Table 13, as `derive::atmosphere` cites it for methane kept where nitrogen is;
/// it gives D = 0.214 cm² s⁻¹ at 298 K and one atmosphere, the fit's lowest temperature, and is
/// extrapolated below it), so 7.34 × 10¹⁸ T^0.75 m⁻¹ s⁻¹.
pub const METHANE_NITROGEN_DIFFUSION: BinaryDiffusion =
    BinaryDiffusion::new(7.34e18, Kelvin::new(1.0), 0.75);

/// Atomic oxygen in atomic hydrogen: b = 4.8 × 10¹⁷ T^0.75 cm⁻¹ s⁻¹ (Zahnle and Kasting 1986,
/// Icarus 68, 462, as Luger and Barnes 2015 take it;
/// [`OXYGEN_IN_HYDROGEN_DIFFUSION_PER_METRE_SECOND`]).
pub const OXYGEN_HYDROGEN_DIFFUSION: BinaryDiffusion = BinaryDiffusion::new(
    OXYGEN_IN_HYDROGEN_DIFFUSION_PER_METRE_SECOND,
    Kelvin::new(1.0),
    0.75,
);

/// Water vapour in a non-condensing air of oxygen or nitrogen, at Wordsworth and Pierrehumbert's
/// homopause temperature [`COLD_TRAP_HOMOPAUSE_TEMPERATURE`]: b ≈ 6.1 × 10¹⁸ cm⁻¹ s⁻¹, so
/// 6.1 × 10²⁰ m⁻¹ s⁻¹, scaling as T^0.75 ("Typically b ∝ T^0.75", Zahnle and Catling 2017,
/// §4.2), though the route reads it only at 300 K.
///
/// The value is the one Wordsworth and Pierrehumbert's (2014, ApJ Lett. 785, L20, read) figures
/// imply (computed by the lane): an O₂ sink of 5 × 10⁹ molecules cm⁻² s⁻¹, balanced by the
/// diffusion-limited loss of 10¹⁰ water molecules cm⁻² s⁻¹, each leaving one oxygen atom, through
/// O₂ at Earth's gravity needs their "3 × 10⁻³ mol/mol" at the cold trap. They take b(H₂O–O₂) from
/// Marrero and Mason (1972), whose water–oxygen value at 300 K and one atmosphere, D ≈
/// 0.25 cm² s⁻¹, gives the same b = n D (from memory, recorded).
pub const WATER_AIR_DIFFUSION: BinaryDiffusion =
    BinaryDiffusion::new(6.1e20, Kelvin::new(300.0), 0.75);

/// The flux, molecules m⁻² s⁻¹, at which a light gas of molecular mass `light_mass` and mixing
/// ratio `light_mixing_ratio` diffuses through a heavier one of molecular mass `heavy_mass`, with
/// binary diffusion coefficient `diffusion_b_per_metre_second`, out of a static isothermal
/// atmosphere at `temperature` on a body of mass `mass` and radius `radius`:
/// φ = f G M (`m_j` − `m_i`) b ÷ (R² k T).
///
/// It is the diffusion limit (Hunten 1973, J. Atmos. Sci. 30, 1481; Zahnle and Catling 2017,
/// eq. 35, read), the upper bound on how fast the light gas can escape by diffusing through a
/// heavier one that does not escape. A mixing ratio above 1 is taken as 1. Zero where the light gas
/// is not lighter, and for an input, a mass included, that is not finite and positive.
///
/// The three diffusion laws pair: this flux bounds the light gas's escape; hydrogen escaping at a
/// flux sets its [`crossover_mass`], the heaviest molecule its flow drags; and a heavier species'
/// [`fractionation_factor`] at that crossover is the share of its own mixing ratio at which it
/// escapes with the hydrogen.
///
/// # Examples
///
/// Titan's methane through its nitrogen at 1.48% and 150 K is limited to some 6 × 10¹³
/// molecules m⁻² s⁻¹, 6 × 10⁹ cm⁻² s⁻¹; nitrogen through methane, heavier through lighter, does not
/// diffuse out:
///
/// ```
/// use hyperion_sim::planetary::chem::escape::{
///     METHANE_NITROGEN_DIFFUSION, diffusion_limited_flux_per_m2_s,
/// };
/// use hyperion_sim::planetary::derive::atmosphere::Gas;
/// use hyperion_sim::units::{Kelvin, Kilograms, Metres};
///
/// let (titan, radius, t) = (Kilograms::new(1.345e23), Metres::new(2.575e6), Kelvin::new(150.0));
/// let (methane, nitrogen) = (Gas::Methane.molecular_mass(), Gas::Nitrogen.molecular_mass());
/// let b = METHANE_NITROGEN_DIFFUSION.per_metre_second_at(t);
/// let flux = diffusion_limited_flux_per_m2_s(0.0148, methane, nitrogen, b, titan, radius, t);
/// assert!((flux / 6e13 - 1.0).abs() < 0.1);
/// assert!(diffusion_limited_flux_per_m2_s(0.98, nitrogen, methane, b, titan, radius, t) <= 0.0);
/// ```
#[must_use]
pub fn diffusion_limited_flux_per_m2_s(
    light_mixing_ratio: f64,
    light_mass: Kilograms,
    heavy_mass: Kilograms,
    diffusion_b_per_metre_second: f64,
    mass: Kilograms,
    radius: Metres,
    temperature: Kelvin,
) -> f64 {
    let (Some((m, r)), Some(share), Some(coefficient), Some(t), Some(light), Some(heavy)) = (
        bulk(mass, radius),
        positive(light_mixing_ratio),
        positive(diffusion_b_per_metre_second),
        positive(temperature.value()),
        positive(light_mass.value()),
        positive(heavy_mass.value()),
    ) else {
        return 0.0;
    };
    let separation = heavy - light;
    if separation <= 0.0 {
        return 0.0;
    }
    share.min(1.0) * GRAVITATIONAL_CONSTANT * m * separation * coefficient
        / (r * r * BOLTZMANN_CONSTANT * t)
}

/// The crossover mass of hydrogen escaping at the flux `hydrogen_flux_per_m2_s` (atoms m⁻² s⁻¹)
/// through an air whose hydrogen mole fraction is `hydrogen_mole_fraction`, at gravity `gravity`
/// and temperature `temperature`, with b `diffusion_b_per_metre_second` the hydrogen's binary
/// diffusion coefficient in the heavier gas: `m_c` = `m_H` + k T `F_H` ÷ (b g `X_H`), the heaviest
/// molecule the flow can drag along.
///
/// It is Hunten, Pepin and Walker's (1987, Icarus 69, 532) crossover mass, as Schaefer et al.
/// (2016, ApJ 829, 63, eqs. 4 and 5) and Luger and Barnes (2015, Astrobiology 15, 119, eqs. 3 and
/// 4) print it, with `m_H` the registry's hydrogen atom. The hydrogen atom's own mass where the
/// flux is not positive, or not a number; infinite where b g X is not positive, which no diffusion
/// separates. Its [`fractionation_factor`] for a heavier species is the share of that species'
/// mixing ratio the flow carries off ([`diffusion_limited_flux_per_m2_s`] gives how the laws pair).
///
/// # Examples
///
/// Hydrogen escaping a sub-Neptune's envelope, 90% hydrogen, through its helium at 3,000 K and
/// 17 m s⁻² drags the helium along at almost its whole mixing ratio at 10²⁰ atoms m⁻² s⁻¹, at a
/// third of it at 10¹⁷, and not at all at 10¹⁶:
///
/// ```
/// use hyperion_sim::planetary::chem::escape::{
///     HOMOPAUSE_TEMPERATURE, HYDROGEN_HELIUM_DIFFUSION, crossover_mass, fractionation_factor,
/// };
/// use hyperion_sim::planetary::derive::atmosphere::Gas;
/// use hyperion_sim::units::{Kilograms, MetresPerSecondSquared};
///
/// let b = HYDROGEN_HELIUM_DIFFUSION.per_metre_second_at(HOMOPAUSE_TEMPERATURE);
/// let gravity = MetresPerSecondSquared::new(17.0);
/// let helium = Gas::Helium.molecular_mass();
/// let hydrogen = Kilograms::new(0.5 * Gas::Hydrogen.molecular_mass().value());
/// let factor = |flux: f64| {
///     let crossover = crossover_mass(flux, b, gravity, HOMOPAUSE_TEMPERATURE, 0.9);
///     fractionation_factor(crossover, helium, hydrogen)
/// };
/// assert!(factor(1e20) > 0.99);
/// assert!((factor(1e17) - 0.33).abs() < 0.01);
/// assert!(factor(1e16) <= 0.0);
/// ```
#[must_use]
pub fn crossover_mass(
    hydrogen_flux_per_m2_s: f64,
    diffusion_b_per_metre_second: f64,
    gravity: MetresPerSecondSquared,
    temperature: Kelvin,
    hydrogen_mole_fraction: f64,
) -> Kilograms {
    let hydrogen = hydrogen_atom_mass();
    let (Some(flux), Some(t)) = (
        positive(hydrogen_flux_per_m2_s),
        positive(temperature.value()),
    ) else {
        return Kilograms::new(hydrogen);
    };
    let resistance = diffusion_b_per_metre_second * gravity.value() * hydrogen_mole_fraction;
    if resistance.is_nan() || resistance <= 0.0 {
        return Kilograms::new(f64::INFINITY);
    }
    Kilograms::new(hydrogen + BOLTZMANN_CONSTANT * t * flux / resistance)
}

/// The fractionation factor of a heavy species of molecular mass `heavy_mass` escaping with
/// hydrogen, of molecular mass `hydrogen_mass`, at the crossover mass `crossover`
/// ([`crossover_mass`]): x = (`m_c` − `m_j`) ÷ (`m_c` − `m_H`), held to [0, 1], the share of its
/// own mixing ratio at which it escapes with the hydrogen (Hunten, Pepin and Walker 1987; Hu,
/// Seager and Yung 2015, eq. 16's x₂). Zero at a crossover not above the hydrogen's or one that is
/// not a number, and for a mass that is not finite and positive; 1 at an infinite crossover.
/// [`crossover_mass`]'s example pairs the two.
#[must_use]
pub fn fractionation_factor(
    crossover: Kilograms,
    heavy_mass: Kilograms,
    hydrogen_mass: Kilograms,
) -> f64 {
    let (Some(heavy), Some(light)) = (
        positive(heavy_mass.value()),
        positive(hydrogen_mass.value()),
    ) else {
        return 0.0;
    };
    let c = crossover.value();
    if c.is_nan() || c <= light {
        return 0.0;
    }
    if c.is_infinite() {
        return 1.0;
    }
    ((c - heavy) / (c - light)).clamp(0.0, 1.0)
}

/// The temperature at the homopause of a fractionating envelope, K: 3,000, Malsky et al.'s (2023,
/// Nat. Astron. 7, 57, §A.4 and §B.2, read) cooler case, the lower bound earlier sub-Neptune
/// studies took (their ref. 59, §B.2); they also run 10,000 K, at which hydrogen drags its helium more strongly and
/// the helium rises more slowly.
pub const HOMOPAUSE_TEMPERATURE: Kelvin = Kelvin::new(3_000.0);

/// What a hydrogen and helium envelope keeps of each after escape ([`envelope_helium`]).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct EnvelopeHelium {
    helium_to_hydrogen: f64,
    helium_mass_fraction: f64,
    hydrogen_kept_share: f64,
    helium_kept_share: f64,
    initial_helium_mass_fraction: f64,
}

impl EnvelopeHelium {
    /// An envelope of helium mass fraction `helium` that has lost nothing.
    #[must_use]
    fn untouched(helium: f64) -> Self {
        Self {
            helium_to_hydrogen: helium_to_hydrogen(helium),
            helium_mass_fraction: helium,
            hydrogen_kept_share: 1.0,
            helium_kept_share: 1.0,
            initial_helium_mass_fraction: helium,
        }
    }

    /// Helium atoms per hydrogen atom: infinite where no hydrogen is left.
    #[must_use]
    pub const fn helium_to_hydrogen(&self) -> f64 {
        self.helium_to_hydrogen
    }

    /// Helium's share of the envelope's hydrogen and helium by mass, Y: 1 where no hydrogen is
    /// left, an envelope lost whole included.
    #[must_use]
    pub const fn helium_mass_fraction(&self) -> f64 {
        self.helium_mass_fraction
    }

    /// The share of its hydrogen the envelope keeps.
    #[must_use]
    pub const fn hydrogen_kept_share(&self) -> f64 {
        self.hydrogen_kept_share
    }

    /// The share of its helium the envelope keeps.
    #[must_use]
    pub const fn helium_kept_share(&self) -> f64 {
        self.helium_kept_share
    }

    /// The helium mass fraction the envelope formed with, Y₀, the fractionation's start (held to
    /// [0, 1], a NaN as 0): the start the escape's ledger books its formed helium at, so that the
    /// two agree (P14.T54.b.i).
    #[must_use]
    pub const fn initial_helium_mass_fraction(&self) -> f64 {
        self.initial_helium_mass_fraction
    }
}

/// Helium atoms per hydrogen atom in a gas of hydrogen and helium of helium mass fraction `helium`.
#[must_use]
fn helium_to_hydrogen(helium: f64) -> f64 {
    let hydrogen_share = 1.0 - helium;
    if hydrogen_share <= 0.0 {
        return f64::INFINITY;
    }
    helium * hydrogen_atom_mass() / (hydrogen_share * Gas::Helium.molecular_mass().value())
}

/// A star's helium-to-hydrogen mass ratio Y ÷ X at metal fraction `z` and helium excess
/// `helium_excess`, X = 0.76 − 3Z − ΔY and Y = 1 − X − Z: the hydrogen fraction of the stellar
/// formulae's models, X = 0.76 − 3Z (Pols et al. 1998, MNRAS 298, 525, as `sse::ms::hydrogen` and
/// every other X of the crate take it), with a helium excess taking the place of hydrogen, so that
/// X + Y + Z = 1 (main's ruling, 2026-10-27: the plan's Y = 1 − X − Z + ΔY summed to 1 + ΔY).
#[must_use]
fn helium_over_hydrogen_by_mass(z: f64, helium_excess: f64) -> f64 {
    let hydrogen = 0.76 - 3.0 * z - helium_excess;
    let helium = 1.0 - hydrogen - z;
    helium / hydrogen
}

/// The primordial He/H by number of a gas envelope formed about a star of composition
/// `composition` (P14.T54.b.i; decision-p14-t50b-envelope §5): the protosolar He/H n☉ times the
/// star's Y ÷ X over the Sun's, n☉ × (Y ÷ X)(Z, ΔY) ÷ (Y ÷ X)(Z☉, 0).
///
/// - n☉ is the element registry's protosolar He/H, 10^(10.986 − 12) = 0.0968 (Lodders 2010,
///   Table 6, as [`Element::protosolar_abundance_per_hydrogen`] holds it), the value
///   `protosolar_helium_mass_fraction` reads.
/// - Z is the star's metal fraction as the stellar formulae read it, [`Composition::z_fit`]: Z☉ ×
///   10^\[Fe/H\] held to 0.0001–0.03, as every other X = 0.76 − 3Z of the crate takes it (the
///   wind's Eddington factor, the accretion's Eddington rate, the main sequence's degenerate
///   radius); Z☉ is [`Z_SOLAR`], 0.02, the formulae's solar metal fraction.
/// - ΔY is the star's [`Composition::helium_excess`], 0 for every star the grid places and at most
///   0.18 in a massive cluster's second population, so that X stays at least 0.49.
///
/// A solar host (Z = Z☉, ΔY = 0) gives n☉ to the bit, the two Y ÷ X being one expression of the
/// same values. \[Fe/H\] −1 gives 0.809 n☉ (0.0783), and any host from +0.176 dex, where Z reaches
/// 0.03, 1.119 n☉ (0.1084); a helium excess of 0.05 at solar Z gives 1.269 n☉ (0.1229).
#[must_use]
pub(crate) fn primordial_helium_per_hydrogen(composition: &Composition) -> f64 {
    let ratio = helium_over_hydrogen_by_mass(
        composition.z_fit().value(),
        composition.helium_excess().value(),
    ) / helium_over_hydrogen_by_mass(Z_SOLAR.value(), 0.0);
    Element::He.protosolar_abundance_per_hydrogen() * ratio
}

/// The helium mass fraction Y = `m_He` n ÷ (`m_H` + `m_He` n) of hydrogen and helium of He/H `n`
/// by number, at the registry's standard atomic weights (`protosolar_helium_mass_fraction` is it
/// at the protosolar He/H, 0.278).
#[must_use]
pub(crate) fn helium_mass_fraction_of(per_hydrogen: f64) -> f64 {
    let helium = per_hydrogen * Element::He.standard_atomic_weight_g_per_mol();
    helium / (Element::H.standard_atomic_weight_g_per_mol() + helium)
}

/// The helium mass fraction an envelope of a host of composition `composition` starts its
/// fractionation from under the rules in force ([`PrimordialHeliumRules`]): the protosolar
/// (`protosolar_helium_mass_fraction`) below [`PRIMORDIAL_HELIUM_VERSION`], and the host's own
/// from it, [`helium_mass_fraction_of`] its [`primordial_helium_per_hydrogen`]. Only the escape's
/// batch reads it (`helium_before_passes`), so that under the escape's hold none is computed.
#[must_use]
pub(crate) fn fractionation_start(composition: &Composition) -> f64 {
    match PrimordialHeliumRules::in_force() {
        PrimordialHeliumRules::Protosolar => {
            crate::planetary::derive::atmosphere::protosolar_helium_mass_fraction()
        }
        PrimordialHeliumRules::Host => {
            helium_mass_fraction_of(primordial_helium_per_hydrogen(composition))
        }
    }
}

/// The He/H by number an envelope of a host of composition `composition` forms with under the
/// rules in force ([`PrimordialHeliumRules`]), before any escape: the protosolar He/H below
/// [`PRIMORDIAL_HELIUM_VERSION`] and the host's own, [`primordial_helium_per_hydrogen`], from it.
/// The record's envelope reads it where the escape is held, which fractionates nothing and carries
/// no helium (P14.T54.b.i).
#[must_use]
pub(crate) fn host_helium_per_hydrogen(composition: &Composition) -> f64 {
    match PrimordialHeliumRules::in_force() {
        PrimordialHeliumRules::Protosolar => Element::He.protosolar_abundance_per_hydrogen(),
        PrimordialHeliumRules::Host => primordial_helium_per_hydrogen(composition),
    }
}

/// The helium a hydrogen and helium envelope keeps as escape takes its hydrogen preferentially
/// (P14.T53.a; Hu, Seager and Yung 2015, ApJ 807, 8, read; Malsky et al. 2023, Nat. Astron. 7, 57,
/// read): the envelope of `initial_envelope`, helium mass fraction `helium_mass_fraction_initial`
/// (helium's share of its hydrogen and helium, which the caller gives: from generator version 24
/// the host star's own, behind `PRIMORDIAL_HELIUM_VERSION`, decision-p14-t50b-envelope §5), on a
/// body of mass `mass` and radius `radius` under the exposure `exposure`, that loses `lost` by
/// `age`.
///
/// - **The loss's timing.** `lost` is the caller's energy-limited loss
///   ([`energy_limited_loss`](crate::planetary::derive::atmosphere::energy_limited_loss), as
///   `derive`'s envelope escape takes it), whose rate follows the X-ray and ultraviolet
///   flux: Ṁ(t) = `lost` × F(t) ÷ ∫₀^age F dt.
/// - **Its partition.** At each step of the module's time grid the mass flux per unit area, Φ ÷
///   4πR², is split between hydrogen and helium by Hu et al.'s eqs. 13 to 15: `φ_He` ÷ `X_He` =
///   `φ_H` ÷ `X_H` − `φ_DL`, with `φ_DL` = G M (`m_He` − `m_H`) b ÷ (R² k T) (their eq. 12, b
///   [`HYDROGEN_HELIUM_DIFFUSION`] for neutral hydrogen at [`HOMOPAUSE_TEMPERATURE`]) and X the
///   envelope's mole fractions then. The fractionation factor x₂ = (`φ_He` ÷ `φ_H`) ÷ (`X_He` ÷
///   `X_H`) (their eq. 16) is [`fractionation_factor`] at the [`crossover_mass`] of that hydrogen
///   flux, the same expression, and helium escapes as d ln `N_He` = x₂ d ln `N_H`, so that over the
///   step `N_He` ÷ `N_He`,₀ = (`N_H` ÷ `N_H`,₀)^x₂. The step's hydrogen share s solves `m_H` `N_H`
///   (1 − s) + `m_He` `N_He` (1 − s^x₂) = ΔM by a fixed 64 bisections. Where the hydrogen runs out
///   with x₂ = 0 the helium stays, and the step's loss beyond the hydrogen is not made; where no
///   hydrogen is left the helium is lost alone.
///
/// The hydrogen is atomic, as Malsky et al. count it, and the envelope's metals are left out:
/// T50.b carries them relative to its hydrogen. Malsky et al. compute the hydrogen's ionisation
/// and its ions' drag on the helium (their eq. A3), which this leaves out, so it fractionates
/// more than theirs at a given homopause temperature.
///
/// The step's flux is its mean, so the result is continuous in `age`, and with a `lost` that grows
/// as the fluence does it never falls in `age` (tested). An envelope that loses nothing keeps its
/// helium bit for bit, as does one under no exposure. An envelope or a loss that is not finite and
/// positive, or a body or an age that is not, loses nothing.
///
/// # Examples
///
/// A 2 R⊕ planet of 7 M⊕ that loses almost all of a 0.7% envelope at 0.047 au of a Sun-like star
/// ends richer in helium than it began:
///
/// ```
/// use hyperion_sim::planetary::chem::escape::{XuvExposure, envelope_helium};
/// use hyperion_sim::planetary::context::XuvHistory;
/// use hyperion_sim::units::consts::{EARTH_MASS_KG, EARTH_RADIUS_M, METRES_PER_AU};
/// use hyperion_sim::units::{Kilograms, Metres, SolarLuminosities, SolarMasses, Years};
///
/// let sun = XuvHistory::new(SolarMasses::new(1.0), SolarLuminosities::new(0.7));
/// let exposure = XuvExposure::of(&sun, Metres::new(0.047 * METRES_PER_AU), 0.0);
/// let (mass, radius) = (Kilograms::new(7.0 * EARTH_MASS_KG), Metres::new(2.0 * EARTH_RADIUS_M));
/// let envelope = Kilograms::new(0.007 * mass.value());
/// let lost = Kilograms::new(0.9 * envelope.value());
/// let helium = envelope_helium(envelope, lost, mass, radius, &exposure, Years::new(5e9), 0.27);
/// assert!(helium.helium_mass_fraction() > 0.27);
/// assert!(helium.hydrogen_kept_share() < 0.1);
/// ```
#[must_use]
pub fn envelope_helium(
    initial_envelope: Kilograms,
    lost: Kilograms,
    mass: Kilograms,
    radius: Metres,
    exposure: &XuvExposure,
    age: Years,
    helium_mass_fraction_initial: f64,
) -> EnvelopeHelium {
    let initial_share = if helium_mass_fraction_initial.is_nan() {
        0.0
    } else {
        helium_mass_fraction_initial.clamp(0.0, 1.0)
    };
    let untouched = EnvelopeHelium::untouched(initial_share);
    let (Some(envelope), Some(lost_mass), Some((m, r)), Some(t)) = (
        positive(initial_envelope.value()),
        positive(lost.value()),
        bulk(mass, radius),
        positive(age.value()),
    ) else {
        return untouched;
    };
    let Some(whole) = positive(exposure.fluence_between(Years::ZERO, age).value()) else {
        return untouched;
    };
    let step = Fractionation::new(m, r);
    let (hydrogen_mass, helium_mass) = (step.hydrogen_mass, step.helium_mass);
    let (hydrogen_initial, helium_initial) = (
        (1.0 - initial_share) * envelope / hydrogen_mass,
        initial_share * envelope / helium_mass,
    );
    let (mut hydrogen, mut helium) = (hydrogen_initial, helium_initial);
    for (start, end) in grid_steps(t) {
        if hydrogen + helium <= 0.0 {
            break;
        }
        let step_mass = lost_mass
            * exposure
                .fluence_between(Years::new(start), Years::new(end))
                .value()
            / whole;
        let seconds = (end - start) * SECONDS_PER_JULIAN_YEAR;
        if step_mass > 0.0 && seconds > 0.0 {
            (hydrogen, helium) = step.lose(hydrogen, helium, step_mass, seconds);
        }
    }
    let (hydrogen_left, helium_left) = (hydrogen * hydrogen_mass, helium * helium_mass);
    let (helium_mass_fraction, ratio) = if hydrogen > 0.0 {
        (
            helium_left / (hydrogen_left + helium_left),
            helium / hydrogen,
        )
    } else {
        (1.0, f64::INFINITY)
    };
    EnvelopeHelium {
        helium_to_hydrogen: ratio,
        helium_mass_fraction,
        hydrogen_kept_share: if hydrogen_initial > 0.0 {
            hydrogen / hydrogen_initial
        } else {
            1.0
        },
        helium_kept_share: if helium_initial > 0.0 {
            helium / helium_initial
        } else {
            1.0
        },
        initial_helium_mass_fraction: initial_share,
    }
}

/// One step of an envelope's fractionating loss ([`envelope_helium`]) on a body of mass `m` and
/// radius `r`.
#[derive(Debug, Clone, Copy)]
struct Fractionation {
    hydrogen_mass: f64,
    helium_mass: f64,
    gravity: f64,
    area: f64,
    coefficient: f64,
    /// Hu et al.'s eq. 12: the diffusion-limited flux per unit of hydrogen's mixing ratio.
    diffusion_limit: f64,
}

impl Fractionation {
    #[must_use]
    fn new(m: f64, r: f64) -> Self {
        let hydrogen_mass = hydrogen_atom_mass();
        let helium_mass = Gas::Helium.molecular_mass().value();
        let gravity = GRAVITATIONAL_CONSTANT * m / (r * r);
        let coefficient = HYDROGEN_HELIUM_DIFFUSION.per_metre_second_at(HOMOPAUSE_TEMPERATURE);
        Self {
            hydrogen_mass,
            helium_mass,
            gravity,
            area: 4.0 * PI * r * r,
            coefficient,
            diffusion_limit: gravity * (helium_mass - hydrogen_mass) * coefficient
                / (BOLTZMANN_CONSTANT * HOMOPAUSE_TEMPERATURE.value()),
        }
    }

    /// The hydrogen and helium atoms left of `hydrogen` and `helium` after a step of `seconds`
    /// that loses `step_mass`, kg.
    #[must_use]
    fn lose(&self, hydrogen: f64, helium: f64, step_mass: f64, seconds: f64) -> (f64, f64) {
        let (hydrogen_mass, helium_mass) = (self.hydrogen_mass, self.helium_mass);
        if hydrogen <= 0.0 {
            return (hydrogen, (helium - step_mass / helium_mass).max(0.0));
        }
        let atoms = hydrogen + helium;
        let (hydrogen_fraction, helium_fraction) = (hydrogen / atoms, helium / atoms);
        // Hu et al.'s eqs. 13 and 15: the hydrogen flux over its mixing ratio, φ_H ÷ X_H, given the
        // step's mass flux per unit area.
        let per_mixing_ratio = (step_mass / (seconds * self.area)
            + helium_mass * helium_fraction * self.diffusion_limit)
            / (hydrogen_mass * hydrogen_fraction + helium_mass * helium_fraction);
        let crossover = crossover_mass(
            hydrogen_fraction * per_mixing_ratio,
            self.coefficient,
            MetresPerSecondSquared::new(self.gravity),
            HOMOPAUSE_TEMPERATURE,
            hydrogen_fraction,
        );
        let x2 = fractionation_factor(
            crossover,
            Kilograms::new(helium_mass),
            Kilograms::new(hydrogen_mass),
        );
        let removed = |share: f64| {
            hydrogen_mass * hydrogen * (1.0 - share)
                + helium_mass * helium * (1.0 - math::powf(share, x2))
        };
        let share = if removed(0.0) <= step_mass {
            0.0
        } else {
            let (mut low, mut high) = (0.0, 1.0);
            for _ in 0..64 {
                let mid = f64::midpoint(low, high);
                if removed(mid) > step_mass {
                    low = mid;
                } else {
                    high = mid;
                }
            }
            f64::midpoint(low, high)
        };
        (hydrogen * share, helium * math::powf(share, x2))
    }
}

/// Earth's removal of atmospheric oxygen by its surface per unit area, mol of O₂ m⁻² yr⁻¹:
/// [`EARTH_OXYGEN_SINK_MOL_PER_YEAR`] over 4π R⊕², about 0.0433, the rate `derive`'s runaway oxygen
/// takes for every world (provisional, as there).
pub const EARTH_OXYGEN_SINK_MOL_PER_M2_YEAR: f64 =
    EARTH_OXYGEN_SINK_MOL_PER_YEAR / (4.0 * PI * EARTH_RADIUS_M * EARTH_RADIUS_M);

/// The X-ray and ultraviolet flux above which a runaway world's escaping hydrogen drags oxygen with
/// it, for a body of mass `mass` and radius `radius`: `F_c` = 40 G² M² `m_H`² b ÷ (k T ε R³), at ε
/// [`WATER_LOSS_ESCAPE_EFFICIENCY`], T [`ESCAPING_FLOW_TEMPERATURE`] and b
/// [`OXYGEN_HYDROGEN_DIFFUSION`] at T.
///
/// It is the flux at which
/// [`oxygen_escape_parameter`](crate::planetary::derive::atmosphere::oxygen_escape_parameter)'s x
/// is 1 (Luger and Barnes 2015, Astrobiology 15, 119, eq. 9 and eq. B1 at η = 0), 180 (M ÷ M⊕)²
/// (R ÷ R⊕)⁻³ erg cm⁻² s⁻¹, some 0.18 W m⁻² for Earth. Infinite for a mass or radius that is not
/// finite and positive.
#[must_use]
pub fn critical_xuv_flux(mass: Kilograms, radius: Metres) -> WattsPerSquareMetre {
    let Some((m, r)) = bulk(mass, radius) else {
        return WattsPerSquareMetre::new(f64::INFINITY);
    };
    let hydrogen = hydrogen_atom_mass();
    let t = ESCAPING_FLOW_TEMPERATURE;
    let g_m = GRAVITATIONAL_CONSTANT * m;
    WattsPerSquareMetre::new(
        40.0 * g_m * g_m * hydrogen * hydrogen * OXYGEN_HYDROGEN_DIFFUSION.per_metre_second_at(t)
            / (BOLTZMANN_CONSTANT * t.value() * WATER_LOSS_ESCAPE_EFFICIENCY * r * r * r),
    )
}

/// A runaway world's water loss over its X-ray and ultraviolet history, in closed form (Luger and
/// Barnes 2015, eqs. A8, A9 and B3), shared by the oxygen routes.
#[derive(Debug, Clone, Copy)]
struct WaterLoss {
    /// k = ε π R³ ÷ (G M), so that k F is the energy-limited loss rate of hydrogen, kg s⁻¹.
    k: f64,
    /// The critical flux `F_c`, W m⁻² ([`critical_xuv_flux`]).
    critical: f64,
    /// The time the flux falls to `F_c`, yr: 0 for a saturated flux not above it.
    critical_time: f64,
    exposure: XuvExposure,
}

impl WaterLoss {
    /// The loss of a body of mass `mass` and radius `radius` under `exposure`, where the body is
    /// finite and positive.
    #[must_use]
    fn new(mass: Kilograms, radius: Metres, exposure: &XuvExposure) -> Option<Self> {
        let (m, r) = bulk(mass, radius)?;
        let critical = critical_xuv_flux(mass, radius).value();
        let saturated = exposure.saturated().value();
        let critical_time = if saturated > critical {
            exposure.saturation_time().value()
                * math::powf(saturated / critical, 1.0 / XUV_DECAY_INDEX)
        } else {
            0.0
        };
        Some(Self {
            k: WATER_LOSS_ESCAPE_EFFICIENCY * PI * r * r * r / (GRAVITATIONAL_CONSTANT * m),
            critical,
            critical_time,
            exposure: *exposure,
        })
    }

    /// ∫₀^t F dt, J m⁻².
    #[must_use]
    fn fluence(&self, years: f64) -> f64 {
        self.exposure
            .fluence_between(Years::ZERO, Years::new(years))
            .value()
    }

    /// ∫₀^t min(F, `F_c`) dt, J m⁻²: `F_c` t until the flux falls to `F_c`, then the flux's own.
    #[must_use]
    fn capped_fluence(&self, years: f64) -> f64 {
        let capped = years.min(self.critical_time).max(0.0);
        self.critical * capped * SECONDS_PER_JULIAN_YEAR + self.fluence(years)
            - self.fluence(capped)
    }

    /// The water lost by `years`, kg: k (Φ + 8 `Φ_c`), `Φ_c` the capped fluence. Above `F_c` the
    /// loss rate 9 k F ÷ (1 + 8η) of Luger and Barnes's eq. A9 is k (F + 8 `F_c`), since
    /// η = (x − 1) ÷ (x + 8) makes 1 + 8η = 9x ÷ (x + 8); below it η is 0 and the rate 9 k F.
    #[must_use]
    fn water_lost_by(&self, years: f64) -> f64 {
        self.k * (self.fluence(years) + 8.0 * self.capped_fluence(years))
    }

    /// The oxygen left behind by `years`, kg of O₂: 9 r k `Φ_c`, r the oxygen per water. Per water
    /// lost the oxygen kept is r (1 − η), and (1 − η) ÷ (1 + 8η) = 1 ÷ x, so above `F_c` the oxygen
    /// is left at 9 r k `F_c`, whatever the flux (Luger and Barnes's eq. B3), and below it at 9 r
    /// k F.
    #[must_use]
    fn oxygen_left_by(&self, years: f64) -> f64 {
        9.0 * oxygen_per_water() * self.k * self.capped_fluence(years)
    }

    /// The rate, kg s⁻¹ of water, above which the escaping hydrogen drags oxygen: 9 k `F_c`.
    #[must_use]
    fn drag_free_rate(&self) -> f64 {
        9.0 * self.k * self.critical
    }

    /// When `water` is gone, by a fixed 64 bisections of [0, `age`], yr: `age` if it lasts.
    #[must_use]
    fn drying_time(&self, water: f64, age: f64) -> f64 {
        if self.water_lost_by(age) <= water {
            return age;
        }
        let (mut low, mut high) = (0.0, age);
        for _ in 0..64 {
            let mid = f64::midpoint(low, high);
            if self.water_lost_by(mid) < water {
                low = mid;
            } else {
                high = mid;
            }
        }
        high
    }
}

/// A body's young magma ocean, as the oxygen routes read it: how long it lasts from formation, and
/// how much oxygen its iron can take.
///
/// The fields are plain inputs with no invariant between them, so they are public, as
/// [`AbundanceDraws`](super::AbundanceDraws)'s are.
#[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd)]
pub struct MeltWindow {
    /// How long the melt lasts from formation; it takes no oxygen after.
    pub until: Years,
    /// The most oxygen, as O₂, the melt can take: the caller's, from the mantle's `FeO`, a quarter
    /// mole of O₂ per mole oxidised to `FeO₁.₅` (an Earth mantle of 4.01 × 10²⁴ kg at 8.05 wt%
    /// `FeO` takes some 3.6 × 10²² kg, 29 oceans' worth; science-p14-t24f §3).
    pub capacity: Kilograms,
}

impl MeltWindow {
    /// No melt: it never takes any oxygen, the default.
    pub const NONE: Self = Self {
        until: Years::ZERO,
        capacity: Kilograms::ZERO,
    };
}

/// Where the oxygen of a world's lost water went (each as O₂): what escaped with the hydrogen, what
/// its melt took, what its surface took and what its air keeps.
///
/// The parts sum to the oxygen freed: `freed` = `dragged` + `sequestered` + `sunk` + `kept`.
#[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd)]
pub struct OxygenOutcome {
    water_lost: Kilograms,
    freed: Kilograms,
    dragged: Kilograms,
    sequestered: Kilograms,
    sunk: Kilograms,
    kept: Kilograms,
}

impl OxygenOutcome {
    /// No water lost: no oxygen anywhere, the default.
    pub const NONE: Self = Self {
        water_lost: Kilograms::ZERO,
        freed: Kilograms::ZERO,
        dragged: Kilograms::ZERO,
        sequestered: Kilograms::ZERO,
        sunk: Kilograms::ZERO,
        kept: Kilograms::ZERO,
    };

    /// The water lost to space.
    #[must_use]
    pub const fn water_lost(&self) -> Kilograms {
        self.water_lost
    }

    /// The oxygen of the water lost, freed by its photolysis.
    #[must_use]
    pub const fn freed(&self) -> Kilograms {
        self.freed
    }

    /// The oxygen the escaping hydrogen dragged away with it.
    #[must_use]
    pub const fn dragged(&self) -> Kilograms {
        self.dragged
    }

    /// The oxygen the young melt took into its iron, as `FeO₁.₅`.
    #[must_use]
    pub const fn sequestered(&self) -> Kilograms {
        self.sequestered
    }

    /// The oxygen the surface took up after the melt froze.
    #[must_use]
    pub const fn sunk(&self) -> Kilograms {
        self.sunk
    }

    /// The oxygen the air keeps.
    #[must_use]
    pub const fn kept(&self) -> Kilograms {
        self.kept
    }
}

/// The oxygen a runaway world keeps of the water it loses over its X-ray and ultraviolet history
/// (P14.T53.a, in place of P14.T24.f's single route,
/// [`runaway_oxygen`](crate::planetary::derive::atmosphere::runaway_oxygen)): a body of mass `mass`
/// and radius `radius` holding `water` under the exposure `exposure`, whose young melt is `melt`,
/// whose surface takes oxygen at `sink_mol_per_m2_year` mol of O₂ m⁻² yr⁻¹ after its melt has
/// frozen, at age `age`.
///
/// - **The water** is lost from formation at Luger and Barnes's (2015, Astrobiology 15, 119, read)
///   rate, 9 k F ÷ (1 + 8η) with k = ε π R³ ÷ (G M) (their eq. A9; ε
///   [`WATER_LOSS_ESCAPE_EFFICIENCY`]) and η their oxygen escape parameter
///   ([`oxygen_escape_parameter`](crate::planetary::derive::atmosphere::oxygen_escape_parameter)),
///   until it is gone or until `age`. With x = F ÷ `F_c` ([`critical_xuv_flux`]) and η = (x − 1) ÷
///   (x + 8) above 1, 1 + 8η = 9x ÷ (x + 8), so the rate is k (F + 8 `F_c`) above the critical flux
///   and 9 k F below it, and the water lost by t is k (Φ + 8 `Φ_c`), with Φ = ∫ F dt and `Φ_c` = ∫
///   min(F, `F_c`) dt, closed forms of the exposure. The time it is gone is found by a fixed 64
///   bisections of that closed form.
/// - **The oxygen left behind**, per water lost, is r (1 − η), r = M(O₂) ÷ 2 M(H₂O) (their eq. A8's
///   (8 − 8η) ÷ 9 with the registry's molar masses), and (1 − η) ÷ (1 + 8η) = 1 ÷ x. So it accrues
///   at 9 r k min(F, `F_c`): above the critical flux at their eq. B3's constant rate, whatever the
///   flux, which is the diffusion limit's (Zahnle and Catling 2017, §6.4: "the diffusion limit
///   becomes the rate that oxygen is left behind"). This is exact for the code's η.
/// - **The melt** takes the oxygen left while it lasts, until `melt.until`, as `FeO` oxidised to
///   `FeO₁.₅` (Schaefer et al. 2016, ApJ 829, 63, read, §4.2, the equilibrium of Kress and
///   Carmichael 1991), up to its capacity, beyond which the rest stays in the air: all of it to
///   the melt's stoichiometric capacity, the lane's upper bound, where Schaefer et al.'s uptake
///   follows Kress and Carmichael's equilibrium and the solidification front (their eq. 24). Schaefer et al.
///   find the melt takes "at most ∼ 10%" of the O₂ produced only because 90–99% of it escapes,
///   which the drag here already removes. After the melt has frozen it takes none ("Following
///   magma ocean solidification, direct exchange of oxygen between the mantle and the atmosphere
///   halts", Schaefer et al., §4.2).
/// - **The surface** takes oxygen at the sink's rate over the body's area from `melt.until` to
///   `age`, while the air holds any. The oxygen accrues at a rate that never rises after the melt
///   has frozen, so the air's oxygen is the oxygen accrued less the sink's whole take, held at
///   zero.
///
/// None is left below zero. With no melt, a saturation that outlasts the loss and the sink over
/// the whole age it is `runaway_oxygen`'s figure, that route being this one at the saturated flux
/// (tested to 10⁻⁹). The oxygen kept is continuous in every input; it rises and then falls with
/// `age` as the sink takes over, and falls with a later freeze while the oxygen accrues faster
/// than the sink takes it and the melt's capacity is not reached (tested).
///
/// A water, body or age that is not finite and positive gives [`OxygenOutcome::NONE`]; a sink that
/// is not positive or not a number takes nothing, and a melt time or capacity that is not positive
/// none.
///
/// # Examples
///
/// An Earth under a saturated flux of 1 W m⁻² for a gigayear loses five oceans in some 150 Myr;
/// had its melt lasted the first 50 Myr, its air keeps only the oxygen left after:
///
/// ```
/// use hyperion_sim::planetary::chem::escape::{
///     EARTH_OXYGEN_SINK_MOL_PER_M2_YEAR, MeltWindow, XuvExposure, runaway_oxygen_timed,
/// };
/// use hyperion_sim::units::consts::{EARTH_MASS_KG, EARTH_RADIUS_M};
/// use hyperion_sim::units::{Kilograms, Metres, WattsPerSquareMetre, Years};
///
/// let (mass, radius) = (Kilograms::new(EARTH_MASS_KG), Metres::new(EARTH_RADIUS_M));
/// let exposure = XuvExposure::new(WattsPerSquareMetre::new(1.0), Years::new(1e9));
/// let water = Kilograms::new(5.0 * 1.4e21);
/// let melt = MeltWindow { until: Years::new(5e7), capacity: Kilograms::new(3.6e22) };
/// let age = Years::new(2e8);
/// let sink = EARTH_OXYGEN_SINK_MOL_PER_M2_YEAR;
/// let timed = runaway_oxygen_timed(water, mass, radius, &exposure, melt, sink, age);
/// let unmelted = runaway_oxygen_timed(water, mass, radius, &exposure, MeltWindow::NONE, sink, age);
/// assert!((timed.water_lost().value() - water.value()).abs() < 1e-9 * water.value());
/// assert!(timed.sequestered().value() > 0.0);
/// assert!(timed.kept().value() < unmelted.kept().value());
/// ```
#[must_use]
pub fn runaway_oxygen_timed(
    water: Kilograms,
    mass: Kilograms,
    radius: Metres,
    exposure: &XuvExposure,
    melt: MeltWindow,
    sink_mol_per_m2_year: f64,
    age: Years,
) -> OxygenOutcome {
    let (Some(inventory), Some(t), Some(loss), Some((_, r))) = (
        positive(water.value()),
        positive(age.value()),
        WaterLoss::new(mass, radius, exposure),
        bulk(mass, radius),
    ) else {
        return OxygenOutcome::NONE;
    };
    let dry = loss.drying_time(inventory, t);
    let water_lost = loss.water_lost_by(dry).min(inventory);
    let freed = oxygen_per_water() * water_lost;
    let left = loss.oxygen_left_by(dry).min(freed);
    let melt_end = if melt.until.value().is_nan() {
        0.0
    } else {
        melt.until.value().max(0.0)
    };
    let capacity = if melt.capacity.value().is_nan() {
        0.0
    } else {
        melt.capacity.value().max(0.0)
    };
    let at_freeze = loss.oxygen_left_by(melt_end.min(dry)).min(left);
    let sequestered = at_freeze.min(capacity);
    let air = left - sequestered;
    let (sunk, kept) = if melt_end >= t {
        (0.0, air)
    } else {
        let taken = oxygen_sink_kg_per_year(sink_mol_per_m2_year, r) * (t - melt_end);
        let kept = if air > taken { air - taken } else { 0.0 };
        (air - kept, kept)
    };
    OxygenOutcome {
        water_lost: Kilograms::new(water_lost),
        freed: Kilograms::new(freed),
        dragged: Kilograms::new(freed - left),
        sequestered: Kilograms::new(sequestered),
        sunk: Kilograms::new(sunk),
        kept: Kilograms::new(kept),
    }
}

/// The surface state of a temperate world's air, as Wordsworth and Pierrehumbert's (2014) moist
/// upper atmosphere reads it: its water vapour, over a non-condensing background.
///
/// The fields are plain inputs with no invariant between them, so they are public, as
/// [`AbundanceDraws`](super::AbundanceDraws)'s are.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct MoistAir {
    /// The water vapour's partial pressure at the surface.
    pub vapour_pressure: Pascals,
    /// The non-condensing gases' partial pressure at the surface; oxygen, argon and carbon dioxide
    /// count as nitrogen does (Wordsworth and Pierrehumbert 2014, Fig. 1: "Results using O₂, Ar or
    /// CO₂ as the non-condensing gas are similar").
    pub background_pressure: Pascals,
    /// The non-condensing gases' mean molar mass, g mol⁻¹.
    pub background_molar_mass_g_per_mol: f64,
    /// The non-condensing gases' specific heat at constant pressure, J kg⁻¹ K⁻¹.
    pub background_heat_capacity_j_per_kg_k: f64,
    /// Water's specific latent heat at the surface, J kg⁻¹.
    pub latent_heat_j_per_kg: f64,
    /// The surface temperature.
    pub surface_temperature: Kelvin,
}

/// The ramp in Wordsworth and Pierrehumbert's moisture number over which a temperate world's upper
/// atmosphere goes from dry to moist, and its water from cold-trapped to lost: M from 1 to 2, a
/// smooth step (the lane's; their criterion is M > 1 alone).
pub const MOIST_RAMP: (f64, f64) = (1.0, 2.0);

/// The homopause temperature at which a temperate world's water diffuses out through its air, K:
/// 300, Wordsworth and Pierrehumbert's (2014, footnote 5: "For simplicity, `T_h` = 300 K is used
/// here").
pub const COLD_TRAP_HOMOPAUSE_TEMPERATURE: Kelvin = Kelvin::new(300.0);

/// Wordsworth and Pierrehumbert's moisture number of `air` with `added` Pa more of background.
#[must_use]
fn moisture(air: &MoistAir, added: f64) -> f64 {
    let (vapour, latent, heat, t) = (
        air.vapour_pressure.value(),
        air.latent_heat_j_per_kg,
        air.background_heat_capacity_j_per_kg_k,
        air.surface_temperature.value(),
    );
    let (Some(vapour), Some(latent), Some(heat), Some(t), Some(molar)) = (
        positive(vapour),
        positive(latent),
        positive(heat),
        positive(t),
        positive(air.background_molar_mass_g_per_mol),
    ) else {
        return 0.0;
    };
    let background = air.background_pressure.value().max(0.0) + added.max(0.0);
    if background.is_nan() {
        return 0.0;
    }
    let epsilon = Gas::Water.molar_mass_g_per_mol() / molar;
    // A zero background gives an infinite number: a pure steam air cannot be cold-trapped.
    epsilon * vapour * latent / (background * heat * t)
}

/// Wordsworth and Pierrehumbert's (2014, ApJ Lett. 785, L20, read, §2) moisture number of `air`,
/// M = ε `p_v` L ÷ (`p_n` `c_p` `T_s`) with ε = `m_v` ÷ `m_n`, the ratio of the water's latent
/// heat to the background's sensible heat. Its upper atmosphere is moist where M > 1 ("Values of
/// M > 1 (M < 1) correspond in general to situations where the upper atmosphere is moist (dry)"):
/// their Fig. 1 puts the line at 340 K over 1 bar of nitrogen, 295 K over 0.1 bar and 255 K over
/// 0.01 bar.
///
/// Zero for a vapour, latent heat, heat capacity, temperature or molar mass that is not finite and
/// positive; infinite over no background.
#[must_use]
pub fn moist_upper_atmosphere(air: &MoistAir) -> f64 {
    moisture(air, 0.0)
}

/// The oxygen a temperate world builds when its cold trap fails for want of a non-condensing gas
/// (P14.T53.a; Wordsworth and Pierrehumbert 2014, ApJ Lett. 785, L20, read): a body of mass `mass`
/// and radius `radius` with `water` at the surface under the air `air`, exposed to `exposure`,
/// whose surface takes oxygen at `sink_mol_per_m2_year` mol of O₂ m⁻² yr⁻¹, at age `age`.
///
/// - **The loss.** Where the upper atmosphere is moist, M > 1 ([`moist_upper_atmosphere`]), water
///   reaches the homopause unhindered and is lost at the diffusion limit through the background,
///   Φ = b f g (`m_n` − `m_H₂O`) ÷ k T (their b f (`H_n`⁻¹ − `H_H₂O`⁻¹), the scale heights
///   H = k T ÷ m g), with f the vapour's surface mixing ratio `p_v` ÷ `p_s`, since no cold trap
///   holds it back, T [`COLD_TRAP_HOMOPAUSE_TEMPERATURE`] and b [`WATER_AIR_DIFFUSION`]. It is
///   held to the energy limit of the runaway route ([`runaway_oxygen_timed`]) and to the water
///   there is. The loss rises over [`MOIST_RAMP`] by a smooth step in M, so that the route is
///   continuous.
/// - **The oxygen.** Each water molecule lost leaves its oxygen atom, so O₂ builds at half the
///   loss in molecules, less what the surface takes. The diffusion limit through any background
///   lighter than carbon dioxide lies below the rate at which the escaping hydrogen would drag
///   oxygen with it (both scale as the body's mass: at f = 1 through nitrogen at 0.37 of it), so
///   none escapes; a heavier background's is held to that rate.
/// - **Its feedback.** The oxygen built joins the background, as their Fig. 1's caption counts it,
///   lowering M and the vapour's mixing ratio, until the cold trap closes or the loss balances the
///   sink: "atmospheric O₂ will build up on such a planet until `p_n` is high enough to cold-trap
///   H₂O and reduce loss rates to negligible values" (§3), or, with a surface sink, "until the loss
///   of hydrogen to space is balanced by the surface removal rate" (§3), which over an Earth they
///   find at some 0.15 bar. The O₂'s pressure is its weight over the surface.
///
/// The route is integrated over the module's time grid by implicit (backward) Euler steps, each
/// solved for the air's oxygen by a fixed 64 bisections, so that a feedback faster than the step,
/// as it is on an Earth (some 10⁴ yr), settles at its balance. The background's molar mass and
/// heat capacity are the caller's, the oxygen added counting at them.
///
/// The melt takes nothing here ([`OxygenOutcome::sequestered`] is zero). A water, body or age that
/// is not finite and positive, a vapour pressure that is not, or a background no heavier than
/// water gives [`OxygenOutcome::NONE`].
///
/// # Examples
///
/// An Earth with a hundredth of a bar of nitrogen at 288 K and no oxygen sink builds oxygen until
/// it holds its water back; with Earth's 0.78 bar it builds none:
///
/// ```
/// use hyperion_sim::planetary::chem::escape::{MoistAir, XuvExposure, cold_trap_failure_oxygen};
/// use hyperion_sim::planetary::context::XuvHistory;
/// use hyperion_sim::units::consts::{EARTH_MASS_KG, EARTH_RADIUS_M, METRES_PER_AU};
/// use hyperion_sim::units::{Kelvin, Kilograms, Metres, Pascals, SolarLuminosities, SolarMasses, Years};
///
/// let sun = XuvHistory::new(SolarMasses::new(1.0), SolarLuminosities::new(0.7));
/// let exposure = XuvExposure::of(&sun, Metres::new(METRES_PER_AU), 0.0);
/// let (mass, radius) = (Kilograms::new(EARTH_MASS_KG), Metres::new(EARTH_RADIUS_M));
/// let air = |nitrogen: f64| MoistAir {
///     vapour_pressure: Pascals::new(1_705.0),
///     background_pressure: Pascals::new(nitrogen),
///     background_molar_mass_g_per_mol: 28.014,
///     background_heat_capacity_j_per_kg_k: 1_040.0,
///     latent_heat_j_per_kg: 2.465e6,
///     surface_temperature: Kelvin::new(288.0),
/// };
/// let oxygen = |nitrogen: f64| {
///     cold_trap_failure_oxygen(
///         Kilograms::new(1.4e21), mass, radius, &air(nitrogen), &exposure, 0.0, Years::new(4e9),
///     )
/// };
/// assert!(oxygen(1e3).kept().value() > 0.0);
/// assert!(oxygen(7.8e4).freed().value() <= 0.0);
/// ```
#[must_use]
pub fn cold_trap_failure_oxygen(
    water: Kilograms,
    mass: Kilograms,
    radius: Metres,
    air: &MoistAir,
    exposure: &XuvExposure,
    sink_mol_per_m2_year: f64,
    age: Years,
) -> OxygenOutcome {
    let (Some(inventory), Some(t), Some(loss), Some((m, r)), Some(vapour)) = (
        positive(water.value()),
        positive(age.value()),
        WaterLoss::new(mass, radius, exposure),
        bulk(mass, radius),
        positive(air.vapour_pressure.value()),
    ) else {
        return OxygenOutcome::NONE;
    };
    let water_molecule = Gas::Water.molecular_mass().value();
    let separation = air.background_molar_mass_g_per_mol * ATOMIC_MASS_CONSTANT_KG - water_molecule;
    let base = air.background_pressure.value().max(0.0);
    if separation.is_nan() || separation <= 0.0 || base.is_nan() || base.is_infinite() {
        return OxygenOutcome::NONE;
    }
    let gravity = GRAVITATIONAL_CONSTANT * m / (r * r);
    let area = 4.0 * PI * r * r;
    let homopause = COLD_TRAP_HOMOPAUSE_TEMPERATURE;
    // Wordsworth and Pierrehumbert's Φ ÷ f, molecules m⁻² s⁻¹, times the area and the molecule.
    let full_rate = WATER_AIR_DIFFUSION.per_metre_second_at(homopause) * gravity * separation
        / (BOLTZMANN_CONSTANT * homopause.value())
        * area
        * water_molecule;
    let (ramp_start, ramp_end) = MOIST_RAMP;
    let diffusion_rate = |oxygen: f64| {
        let added = oxygen * gravity / area;
        let ramp = smoothstep((moisture(air, added) - ramp_start) / (ramp_end - ramp_start));
        ramp * vapour / (vapour + base + added) * full_rate
    };
    let drag_free = loss.drag_free_rate();
    let per_water = oxygen_per_water();
    let sink_rate = oxygen_sink_kg_per_year(sink_mol_per_m2_year, r) / SECONDS_PER_JULIAN_YEAR;
    let (mut oxygen, mut water_left) = (0.0, inventory);
    let (mut freed, mut dragged, mut sunk) = (0.0, 0.0, 0.0);
    for (start, end) in grid_steps(t) {
        if water_left <= 0.0 && oxygen <= 0.0 {
            break;
        }
        let seconds = (end - start) * SECONDS_PER_JULIAN_YEAR;
        let energy_limit = loss.water_lost_by(end) - loss.water_lost_by(start);
        let used = |held: f64| {
            (seconds * diffusion_rate(held))
                .min(energy_limit)
                .min(water_left)
        };
        let left_behind = |held: f64| per_water * used(held).min(seconds * drag_free);
        let sink_taken = sink_rate * seconds;
        // Backward Euler: the air's oxygen at the step's end, o = oxygen + left_behind(o) − the
        // sink's take, held at zero. The left side less the right rises with o, since a fuller air
        // loses less water.
        let excess = |held: f64| held - oxygen - left_behind(held) + sink_taken;
        let solved = if excess(0.0) >= 0.0 {
            0.0
        } else {
            let (mut low, mut high) = (
                0.0,
                oxygen + per_water * water_left.min(seconds * drag_free),
            );
            for _ in 0..64 {
                let mid = f64::midpoint(low, high);
                if excess(mid) < 0.0 {
                    low = mid;
                } else {
                    high = mid;
                }
            }
            f64::midpoint(low, high)
        };
        let (used_now, left_now) = (used(solved), left_behind(solved));
        // The step's balance at the solved loss, so that the sink takes neither more than the air
        // held nor less than nothing.
        let next = (oxygen + left_now - sink_taken).max(0.0);
        water_left = (water_left - used_now).max(0.0);
        freed += per_water * used_now;
        dragged += per_water * used_now - left_now;
        sunk += oxygen + left_now - next;
        oxygen = next;
    }
    // The water lost is the inventory less what is left, so that the two sum to the inventory by
    // construction and no loss exceeds it, by rounding or otherwise.
    OxygenOutcome {
        water_lost: Kilograms::new(inventory - water_left),
        freed: Kilograms::new(freed),
        dragged: Kilograms::new(dragged),
        sequestered: Kilograms::ZERO,
        sunk: Kilograms::new(sunk),
        kept: Kilograms::new(oxygen),
    }
}

/// Titan's net destruction of methane by photolysis, molecules m⁻² s⁻¹ referred to the surface:
/// 1.5 × 10¹⁴, Yung, Allen and Pinto's (1984, ApJS 55, 465, read, p. 492: "The net destruction rate
/// of CH₄, which must be balanced by an equivalent upward flux at the tropopause, is 1.5 × 10¹⁰
/// cm⁻² s⁻¹"). It would take Titan's present methane in some 30 Myr (Hörst 2017, JGR Planets 122,
/// 432, read, §8: "destroyed by the present day atmospheric chemistry in ∼30 Myr").
pub const TITAN_METHANE_DESTRUCTION_PER_M2_S: f64 = 1.5e14;

/// The insolation of Titan, at Saturn's 9.58 au of the Sun: [`SOLAR_CONSTANT_W_PER_M2`] ÷ 9.58²,
/// 14.83 W m⁻², against which a body's methane photolysis is scaled ([`methane_outcome`]).
pub const TITAN_INSOLATION: WattsPerSquareMetre =
    WattsPerSquareMetre::new(SOLAR_CONSTANT_W_PER_M2 / (9.58 * 9.58));

/// Where a body's methane went: what escaped through its nitrogen, what its photolysis turned to
/// hydrocarbons (the haze's carbon, P14.T24.c's), and what it keeps. The parts sum to the
/// reservoir.
#[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd)]
pub struct MethaneOutcome {
    escaped: Kilograms,
    photolysed: Kilograms,
    kept: Kilograms,
}

impl MethaneOutcome {
    /// No methane, the default.
    pub const NONE: Self = Self {
        escaped: Kilograms::ZERO,
        photolysed: Kilograms::ZERO,
        kept: Kilograms::ZERO,
    };

    /// The methane that escaped.
    #[must_use]
    pub const fn escaped(&self) -> Kilograms {
        self.escaped
    }

    /// The methane photolysed to hydrocarbons.
    #[must_use]
    pub const fn photolysed(&self) -> Kilograms {
        self.photolysed
    }

    /// The methane kept.
    #[must_use]
    pub const fn kept(&self) -> Kilograms {
        self.kept
    }
}

/// Where the methane `reservoir` of a body of mass `mass` and radius `radius` goes by `age`, its
/// air holding it at the mixing ratio `mixing_ratio` at the homopause temperature `temperature`
/// under the insolation `insolation_over_titan` times Titan's ([`TITAN_INSOLATION`])
/// (science-p14-t24f §4):
///
/// - **escape** through nitrogen at most at the diffusion limit ([`diffusion_limited_flux_per_m2_s`], b
///   [`METHANE_NITROGEN_DIFFUSION`]), over the whole surface. Titan's measured escape, 2.5–3.0 ×
///   10⁹ cm⁻² s⁻¹, is "diffusion limited" (Yelle et al. 2008, JGR 113, E10003, from memory), and
///   the formula at its 1.48% and 150 K gives about 6 × 10⁹, twice it (stated by a test);
/// - **photolysis** to hydrocarbons at [`TITAN_METHANE_DESTRUCTION_PER_M2_S`], which is limited by
///   the ultraviolet photons, so scaled by the insolation, over the whole surface.
///
/// Both run at constant rates over the age, as a surface reservoir resupplies the air, until the
/// reservoir is gone, which they share in proportion to their rates. A reservoir that is not finite
/// and positive gives [`MethaneOutcome::NONE`]; a body or age that is not keeps it all; an
/// insolation that is not finite and positive photolyses none.
///
/// # Examples
///
/// Titan keeps most of a methane reservoir of 2.2 × 10⁻³ of its mass over the Solar System's age:
///
/// ```
/// use hyperion_sim::planetary::chem::escape::methane_outcome;
/// use hyperion_sim::units::{Kelvin, Kilograms, Metres, Years};
///
/// let (mass, radius) = (Kilograms::new(1.345e23), Metres::new(2.575e6));
/// let reservoir = Kilograms::new(2.2e-3 * mass.value());
/// let titan = methane_outcome(reservoir, 0.0148, Kelvin::new(150.0), mass, radius, 1.0, Years::new(4.57e9));
/// assert!(titan.kept().value() > 0.5 * reservoir.value());
/// ```
#[must_use]
pub fn methane_outcome(
    reservoir: Kilograms,
    mixing_ratio: f64,
    temperature: Kelvin,
    mass: Kilograms,
    radius: Metres,
    insolation_over_titan: f64,
    age: Years,
) -> MethaneOutcome {
    let Some(store) = positive(reservoir.value()) else {
        return MethaneOutcome::NONE;
    };
    let (Some((_, r)), Some(t)) = (bulk(mass, radius), positive(age.value())) else {
        return MethaneOutcome {
            kept: reservoir,
            ..MethaneOutcome::NONE
        };
    };
    let methane = Gas::Methane.molecular_mass();
    let per_molecule = 4.0 * PI * r * r * methane.value() * t * SECONDS_PER_JULIAN_YEAR;
    let escape = per_molecule
        * diffusion_limited_flux_per_m2_s(
            mixing_ratio,
            methane,
            Gas::Nitrogen.molecular_mass(),
            METHANE_NITROGEN_DIFFUSION.per_metre_second_at(temperature),
            mass,
            radius,
            temperature,
        );
    let photolysis = per_molecule
        * TITAN_METHANE_DESTRUCTION_PER_M2_S
        * positive(insolation_over_titan).unwrap_or(0.0);
    let demand = escape + photolysis;
    let (escaped, photolysed, kept) = if demand <= store {
        (escape, photolysis, store - demand)
    } else {
        let share = store / demand;
        (escape * share, photolysis * share, 0.0)
    };
    MethaneOutcome {
        escaped: Kilograms::new(escaped),
        photolysed: Kilograms::new(photolysed),
        kept: Kilograms::new(kept),
    }
}

/// The hydrogen content, as a fraction of a carbon dioxide air's atoms, below which an M dwarf's
/// ultraviolet turns its carbon dioxide into carbon monoxide and oxygen: 10⁻⁶ (Gao et al. 2015, ApJ
/// 806, 249, read, abstract: the catalytic cycle of H₂O₂ that keeps it "breaks down for
/// atmospheric hydrogen mixing ratios <1 ppm").
pub const GAO_HYDROGEN_THRESHOLD: f64 = 1e-6;

/// The share of a desiccated carbon dioxide air converted to carbon monoxide and oxygen: 0.4 (Gao
/// et al. 2015, abstract: "∼40% of the atmospheric CO₂ being converted to CO and O₂ on a time scale
/// of 1 Myr").
///
/// Below 0.1 ppm of hydrogen the recombination of CO and O takes over, and 40% is converted (their
/// §4: "only 40% of the atmospheric CO₂ is converted"); between 0.1 and 1 ppm their cycle of O₃
/// holds half, "an upper limit of ∼50%" converted; above 1 ppm the cycle of H₂O₂ keeps "80 – 90% of
/// the original CO₂" at their 26 ppm. The share is 40% at and below the threshold, falling to none
/// over the decade above it ([`desiccated_carbon_dioxide`]).
pub const GAO_CONVERTED_SHARE: f64 = 0.4;

/// The effective temperature below which a host is an M dwarf for [`desiccated_carbon_dioxide`]:
/// 3,900 K, between Pecaut and Mamajek's (2013, ApJS 208, 9, Table 5, as the classifier holds it)
/// K9V at 3,930 K and M0V at 3,850 K. Gao et al. (2015) model the spectrum of GJ 436, an M2.5
/// dwarf, whose far-ultraviolet to near-ultraviolet ratio, far above a Sun-like star's, drives the
/// conversion.
pub const M_DWARF_EDGE: Kelvin = Kelvin::new(3_900.0);

/// Which light a desiccated carbon dioxide air is under ([`desiccated_carbon_dioxide`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum UltravioletHost {
    /// An M dwarf's, below [`M_DWARF_EDGE`]: rich in far ultraviolet, poor in near.
    MDwarf,
    /// A hotter star's, whose near ultraviolet keeps the carbon dioxide (Gao et al. 2015, abstract:
    /// "Around Sun-like stars, CO₂ photolysis by Far-UV (FUV) radiation is balanced by
    /// recombination reactions").
    Hotter,
}

impl UltravioletHost {
    /// The light of a host of effective temperature `effective_temperature`: an M dwarf's below
    /// [`M_DWARF_EDGE`], a hotter star's at or above it and at a temperature that is not a number.
    #[must_use]
    pub fn of(effective_temperature: Kelvin) -> Self {
        if effective_temperature < M_DWARF_EDGE {
            Self::MDwarf
        } else {
            Self::Hotter
        }
    }
}

/// What a desiccated carbon dioxide air holds after its photolysis, mol.
#[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd)]
pub struct DesiccatedCarbonDioxide {
    carbon_monoxide: f64,
    oxygen: f64,
    carbon_dioxide: f64,
}

impl DesiccatedCarbonDioxide {
    /// The carbon monoxide made, mol. The substance registry has no row for it, so the record
    /// carries it from P14.T54.b (recorded).
    #[must_use]
    pub const fn carbon_monoxide_mol(&self) -> f64 {
        self.carbon_monoxide
    }

    /// The oxygen made, mol of O₂.
    #[must_use]
    pub const fn oxygen_mol(&self) -> f64 {
        self.oxygen
    }

    /// The carbon dioxide left, mol.
    #[must_use]
    pub const fn carbon_dioxide_mol(&self) -> f64 {
        self.carbon_dioxide
    }
}

/// The photolysis of a desiccated carbon dioxide air of `co2_moles` whose hydrogen is
/// `hydrogen_atom_fraction` of its atoms, under the light `host` (P14.T53.a; Gao et al. 2015, ApJ
/// 806, 249, read): 2 CO₂ → 2 CO + O₂ for [`GAO_CONVERTED_SHARE`] of it under an M dwarf at and
/// below [`GAO_HYDROGEN_THRESHOLD`], falling by a smooth step in log₁₀ H to none at ten times the
/// threshold, and none under a hotter star. Carbon and oxygen atoms, and so mass, are conserved.
///
/// The ramp lies above the threshold, so that the threshold's 40% holds below it, as Gao et al.'s
/// abstract states; above 10 ppm the conversion is taken as none, below their 26 ppm's 10–20%
/// (the lane's, recorded). A carbon dioxide that is not finite and positive converts none, as does
/// a hydrogen fraction that is not a number; a hydrogen fraction that is not positive is
/// desiccated wholly.
///
/// # Examples
///
/// ```
/// use hyperion_sim::planetary::chem::escape::{UltravioletHost, desiccated_carbon_dioxide};
/// use hyperion_sim::units::Kelvin;
///
/// let dry = desiccated_carbon_dioxide(100.0, 1e-7, UltravioletHost::of(Kelvin::new(3_400.0)));
/// assert!((dry.carbon_monoxide_mol() - 40.0).abs() < 1e-9);
/// assert!((dry.oxygen_mol() - 20.0).abs() < 1e-9);
/// assert!((dry.carbon_dioxide_mol() - 60.0).abs() < 1e-9);
/// let sunlit = desiccated_carbon_dioxide(100.0, 1e-7, UltravioletHost::Hotter);
/// assert!(sunlit.carbon_monoxide_mol() <= 0.0);
/// ```
#[must_use]
pub fn desiccated_carbon_dioxide(
    co2_moles: f64,
    hydrogen_atom_fraction: f64,
    host: UltravioletHost,
) -> DesiccatedCarbonDioxide {
    let Some(carbon_dioxide) = positive(co2_moles) else {
        return DesiccatedCarbonDioxide {
            carbon_monoxide: 0.0,
            oxygen: 0.0,
            carbon_dioxide: co2_moles.max(0.0),
        };
    };
    let share = match host {
        UltravioletHost::Hotter => 0.0,
        UltravioletHost::MDwarf => {
            if hydrogen_atom_fraction.is_nan() {
                0.0
            } else if hydrogen_atom_fraction <= 0.0 {
                GAO_CONVERTED_SHARE
            } else {
                let decades = math::log10(hydrogen_atom_fraction / GAO_HYDROGEN_THRESHOLD);
                GAO_CONVERTED_SHARE * (1.0 - smoothstep(decades))
            }
        }
    };
    let converted = share * carbon_dioxide;
    DesiccatedCarbonDioxide {
        carbon_monoxide: converted,
        oxygen: 0.5 * converted,
        carbon_dioxide: carbon_dioxide - converted,
    }
}

/// The hydrogen and helium a body keeps as air, by mass, from a thin envelope of
/// `envelope_fraction` of its mass `mass` and helium mass fraction `helium_mass_fraction`: (H₂, He)
/// = ((1 − Y) f M, Y f M) below [`THIN_ENVELOPE_FRACTION`] (P14.T53.a; until now dropped), and none
/// at or above it, where the envelope is a gas envelope with no surface beneath, or for a fraction
/// or mass that is not finite and positive. A helium fraction is held to [0, 1], one that is not a
/// number taken as 0.
#[must_use]
pub fn thin_envelope_air(
    envelope_fraction: f64,
    mass: Kilograms,
    helium_mass_fraction: f64,
) -> (Kilograms, Kilograms) {
    let (Some(fraction), Some(m)) = (positive(envelope_fraction), positive(mass.value())) else {
        return (Kilograms::ZERO, Kilograms::ZERO);
    };
    if fraction >= THIN_ENVELOPE_FRACTION {
        return (Kilograms::ZERO, Kilograms::ZERO);
    }
    let helium = if helium_mass_fraction.is_nan() {
        0.0
    } else {
        helium_mass_fraction.clamp(0.0, 1.0)
    };
    let envelope = fraction * m;
    (
        Kilograms::new((1.0 - helium) * envelope),
        Kilograms::new(helium * envelope),
    )
}

/// The elements whose outcomes escape records, in [`EscapeOutcome::elements`]'s order: hydrogen,
/// helium, carbon, nitrogen, oxygen and argon.
pub const ESCAPE_ELEMENTS: [Element; 6] = [
    Element::H,
    Element::He,
    Element::C,
    Element::N,
    Element::O,
    Element::Ar,
];

/// Where a body's inventory of one element went, by the element's mass: escaped to space, airborne,
/// sequestered in its melt, rock or ice, or photolysed to hydrocarbons and hazes.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct ElementOutcome {
    element: Element,
    escaped: Kilograms,
    airborne: Kilograms,
    sequestered: Kilograms,
    photolysed: Kilograms,
}

impl ElementOutcome {
    /// The outcome of `element`, each part held at zero or above (one that is not a number taken as
    /// zero).
    #[must_use]
    pub fn new(
        element: Element,
        escaped: Kilograms,
        airborne: Kilograms,
        sequestered: Kilograms,
        photolysed: Kilograms,
    ) -> Self {
        let part = |mass: Kilograms| Kilograms::new(mass.value().max(0.0));
        Self {
            element,
            escaped: part(escaped),
            airborne: part(airborne),
            sequestered: part(sequestered),
            photolysed: part(photolysed),
        }
    }

    /// The element.
    #[must_use]
    pub const fn element(&self) -> Element {
        self.element
    }

    /// The element's mass escaped to space.
    #[must_use]
    pub const fn escaped(&self) -> Kilograms {
        self.escaped
    }

    /// The element's mass in the air.
    #[must_use]
    pub const fn airborne(&self) -> Kilograms {
        self.airborne
    }

    /// The element's mass sequestered in the melt, rock or ice.
    #[must_use]
    pub const fn sequestered(&self) -> Kilograms {
        self.sequestered
    }

    /// The element's mass photolysed to hydrocarbons and hazes.
    #[must_use]
    pub const fn photolysed(&self) -> Kilograms {
        self.photolysed
    }

    /// The element's whole inventory, the four parts' sum.
    #[must_use]
    pub fn inventory(&self) -> Kilograms {
        self.escaped + self.airborne + self.sequestered + self.photolysed
    }
}

/// Why an [`EscapeOutcome`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildEscapeOutcomeError {
    /// The outcome at `index` is of `found`, not of [`ESCAPE_ELEMENTS`]'s `expected`.
    ElementOutOfPlace {
        /// The outcome's place.
        index: usize,
        /// The element that belongs there.
        expected: Element,
        /// The element given.
        found: Element,
    },
}

impl core::fmt::Display for BuildEscapeOutcomeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ElementOutOfPlace {
                index,
                expected,
                found,
            } => write!(
                f,
                "escape outcome {index} is of {}, not {}",
                found.symbol(),
                expected.symbol()
            ),
        }
    }
}

impl std::error::Error for BuildEscapeOutcomeError {}

/// A body's escape, as P14.T54.b puts it on the record (P14.T53.a, "each element's outcome
/// recorded"): the outcomes of hydrogen, helium, carbon, nitrogen, oxygen and argon, its
/// envelope's helium where it has an envelope, the shoreline's shares, its water's oxygen, the
/// carbon monoxide its carbon dioxide's photolysis made, the share of its carbon the weathering
/// thermostat left airborne and its methane's outcome. The derivation fills it under the escape's
/// batch rules
/// ([`Atmosphere::escape`](crate::planetary::derive::atmosphere::Atmosphere::escape)).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct EscapeOutcome {
    elements: [ElementOutcome; 6],
    envelope: Option<EnvelopeHelium>,
    shoreline: Shoreline,
    oxygen: OxygenOutcome,
    carbon_monoxide: Kilograms,
    airborne_carbon_share: f64,
    methane: MethaneOutcome,
}

impl EscapeOutcome {
    /// The escape of the parts given, `elements` in [`ESCAPE_ELEMENTS`]'s order.
    ///
    /// # Errors
    ///
    /// [`BuildEscapeOutcomeError::ElementOutOfPlace`] if an outcome is not of the element its place
    /// holds.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::planetary::chem::escape::{
    ///     BuildEscapeOutcomeError, ESCAPE_ELEMENTS, ElementOutcome, EscapeOutcome, OxygenOutcome,
    ///     Shoreline,
    /// };
    /// use hyperion_sim::substance::Element;
    /// use hyperion_sim::units::Kilograms;
    ///
    /// let kg = Kilograms::new;
    /// let elements = ESCAPE_ELEMENTS.map(|e| ElementOutcome::new(e, kg(1.0), kg(2.0), kg(0.0), kg(0.0)));
    /// let outcome = EscapeOutcome::new(elements, None, Shoreline::NONE, OxygenOutcome::NONE, kg(0.0))?;
    /// let nitrogen = outcome.element(Element::N).expect("nitrogen is recorded");
    /// assert!((nitrogen.inventory().value() - 3.0).abs() < 1e-12);
    /// # Ok::<(), BuildEscapeOutcomeError>(())
    /// ```
    pub fn new(
        elements: [ElementOutcome; 6],
        envelope: Option<EnvelopeHelium>,
        shoreline: Shoreline,
        oxygen: OxygenOutcome,
        carbon_monoxide: Kilograms,
    ) -> Result<Self, BuildEscapeOutcomeError> {
        for (index, (outcome, &expected)) in elements.iter().zip(&ESCAPE_ELEMENTS).enumerate() {
            if outcome.element != expected {
                return Err(BuildEscapeOutcomeError::ElementOutOfPlace {
                    index,
                    expected,
                    found: outcome.element,
                });
            }
        }
        Ok(Self {
            elements,
            envelope,
            shoreline,
            oxygen,
            carbon_monoxide: Kilograms::new(carbon_monoxide.value().max(0.0)),
            airborne_carbon_share: 1.0,
            methane: MethaneOutcome::NONE,
        })
    }

    /// The same escape with `share` of its carbon left airborne by the weathering thermostat, held
    /// to [0, 1] (one that is not a number taken as 1): a temperate world's, the rest stored in
    /// carbonate. [`new`](Self::new) leaves it all airborne, as it is on every other world.
    #[must_use]
    pub fn with_airborne_carbon_share(self, share: f64) -> Self {
        Self {
            airborne_carbon_share: if share.is_nan() {
                1.0
            } else {
                share.clamp(0.0, 1.0)
            },
            ..self
        }
    }

    /// The same escape with its methane's outcome `methane` ([`methane_outcome`]): a cold world's
    /// reservoir, escaped through its nitrogen, photolysed and kept. [`new`](Self::new) records none.
    #[must_use]
    pub const fn with_methane(self, methane: MethaneOutcome) -> Self {
        Self { methane, ..self }
    }

    /// Every element's outcome, in [`ESCAPE_ELEMENTS`]'s order.
    #[must_use]
    pub const fn elements(&self) -> &[ElementOutcome; 6] {
        &self.elements
    }

    /// The outcome of `element`, where escape records it.
    #[must_use]
    pub fn element(&self, element: Element) -> Option<&ElementOutcome> {
        self.elements
            .iter()
            .find(|outcome| outcome.element == element)
    }

    /// The envelope's helium, where the body has an envelope.
    #[must_use]
    pub const fn envelope(&self) -> Option<&EnvelopeHelium> {
        self.envelope.as_ref()
    }

    /// The cosmic shoreline's shares.
    #[must_use]
    pub const fn shoreline(&self) -> &Shoreline {
        &self.shoreline
    }

    /// The oxygen of the body's lost water.
    #[must_use]
    pub const fn oxygen(&self) -> &OxygenOutcome {
        &self.oxygen
    }

    /// The carbon monoxide made by its carbon dioxide's photolysis ([`desiccated_carbon_dioxide`]),
    /// with half its moles of oxygen. Both are the record's alone: the air keeps its carbon
    /// dioxide until P14.T54.b carries them.
    #[must_use]
    pub const fn carbon_monoxide(&self) -> Kilograms {
        self.carbon_monoxide
    }

    /// The share of its carbon the weathering thermostat left airborne, 0 to 1: 1 but on a
    /// temperate world with water, whose carbonate holds the rest.
    #[must_use]
    pub const fn airborne_carbon_share(&self) -> f64 {
        self.airborne_carbon_share
    }

    /// Its methane's outcome: none but on a cold world whose carbon is airborne as methane.
    #[must_use]
    pub const fn methane(&self) -> &MethaneOutcome {
        &self.methane
    }
}

#[cfg(test)]
mod tests;
