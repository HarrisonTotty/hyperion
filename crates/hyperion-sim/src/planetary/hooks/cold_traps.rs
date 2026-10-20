//! Cold traps on airless bodies (plan 14, P14.T51.f; science-r09-drainage.md §4 and §8): the share
//! of a body's surface whose annual maximum temperature stays below a condensable's threshold, in
//! permanent shadows too small for any zonal profile to see, the share of that whose ice is
//! exposed, and P14.T51.a's airless branch keyed on the zonal annual maximum, not the mean.
//!
//! All of it is held at generator version 24 (`COLD_TRAPS_VERSION`; decision-bump-plan-after-22
//! §1.3), so that nothing on the record moves before P14.T54.b's bump; the record's partition,
//! which the `p14-snapshot` lane's hold kept off it until P14.T54.a, publishes P14.T51.a's
//! condensates meanwhile.
//!
//! # Which bodies
//!
//! A body in the airless state (P14.T13.c) whose air is not collisional at its surface: its exobase
//! lies at the ground, so the column's number density p ÷ (m̄ g) is below 1 ÷ `σ_c`
//! ([`COLLISION_CROSS_SECTION_M2`]), that is p < m̄ g ÷ `σ_c`, with m̄ its air's mean molecular mass,
//! or nitrogen's where its air is empty. On the Moon that is 2.5 × 10⁻⁷ Pa. It is not the airless
//! state's 100 Pa: Pluto and Triton, at about 1 Pa, are collisional, their mean free path
//! millimetres against a scale height of about 20 km (decision-p14-t51a-part2 §3.3), and keep
//! P14.T51.a's edges. A body locked 1:1 to its seasonal host has no star circling its sky, and gets
//! no cold traps and keeps P14.T51.a's branch, a stated limit (`hyperion_surface::cold_trap`'s
//! limits).
//!
//! # Which condensables
//!
//! Every species of the body's exchangeable inventory (P14.T50.a's column, as P14.T51.a reads it)
//! whose registry row has a phase diagram, and water and carbon dioxide, which impactors deliver
//! to any body ([`DELIVERED`]), in registry order. Sulphur dioxide, hydrogen sulphide and carbon
//! monoxide have phase diagrams but no gas columns, so no inventory holds them: a closed set, for
//! the composition audit.
//!
//! # The threshold
//!
//! One rule for every species: the temperature at which its net Hertz–Knudsen sublimation into
//! vacuum, on its T49.b saturation curve, is P14.T51.a's 144 kg m⁻² Gyr⁻¹ (Schorghofer and
//! Williams 2020, PSJ 1, 54, Table 1: 110 K for water), P14.T51.a's sublimation edge at no partial
//! pressure (`partition::exosphere_threshold`). It gives water 110 K, within 1 K, and carbon
//! dioxide the registry's 58.18 K at 144 kg m⁻² Gyr⁻¹ (the research estimated about 57 K); at
//! Schorghofer et al. 2021's
//! stricter 10 kg m⁻² Gyr⁻¹ (GRL 48, e2021GL095533) carbon dioxide's is 55.53 K (their 55.6 K on
//! Bryson et al. 1974's curve), which the tests read. `trap_threshold` gives it, and the cold
//! traps, R09.T13.f's placement and R09.T13.d's airless frost points all read it there.
//!
//! # The zonal annual maximum
//!
//! No profile of the conditions holds one: their zonal profile is annual-mean, and their warmest
//! temperature one global value. So it is built here on flat ground in equilibrium with the star
//! at its highest:
//!
//! σ `T_max`⁴ = ((1 − A) `S_p` sin e + q) ÷ ε,
//!
//! with e the star disk's flux-weighted elevation at min(90°, e₀ + δ), e₀ the colatitude
//! (`hyperion_surface::cold_trap::flux_weighted_elevation`, the one the cold traps' shadows take),
//! `S_p` the flux at periapsis, A the bare regolith's Bond albedo (P14.T51.i, below), q the heat
//! flow and ε [`REGOLITH_EMISSIVITY`].
//! It is taken at the annual-mean profile's area points, and its cold area read by the same rule,
//! linear between them. It falls toward the pole, so its cold area is a polar cap, nothing or the
//! whole surface. On the Moon it is 158 K at the pole and 227 K at 85°, and on Mercury 172 K at
//! the pole (110 K with the Sun as a point). It is symmetric about the equator: a southern
//! latitude takes its northern mirror's (`ColdTrapGround::annual_maximum`).
//!
//! # The airless branch, from 24
//!
//! On such a body every solid's edge is its threshold, compared against the zonal annual maximum
//! (science-r09-drainage.md §4.3 item 6). This settles P14.T51.a's deferred item: a species alone
//! in an exosphere has dominance 1, so its edge under the blend was its under-air one (208.65 K
//! for carbon dioxide, which would lay the Moon a polar cap of it). A solid lies zonally where the
//! maximum is below its threshold, at most the area its reservoir covers at P14.T24.b's 1.93 km
//! sheet, and its mass is what that area holds, at most its reservoir. Water takes the same rule,
//! the partition's condensed water as its reservoir, where P14.T24.b's state gate published none on
//! an airless silicate body. An ice-rich crust keeps P14.T24.b's ice: taking it to the maximum
//! would cut Europa's to about 0.014, so its water takes no cold traps either.
//!
//! # The cold traps
//!
//! A species' cold-trap fraction `f_ct` is `hyperion_surface::cold_trap::fraction` (R09.T7.d's
//! closed form, Hayne et al. 2021's template with Ingersoll et al. 1992's shadow temperature)
//! integrated over latitude by area, ∫ f(φ) cos φ dφ from the equator to the pole, by four-point
//! Gauss–Legendre rules on the bands [`TRAP_BAND_EDGES_DEG`], 24 nodes, within 5 × 10⁻⁴ of a fine
//! midpoint rule (the tests' bound; −3.99 × 10⁻⁴ measured for the Moon's water under craters of
//! distribution B). The model is symmetric about the equator, so that is the share of
//! the whole surface. It is taken only at latitudes whose zonal annual maximum is at least the
//! threshold, so the traps never lie under the zonal cap. Its inputs:
//!
//! - the star's greatest declination in the body's sky, min(ε, 180° − ε) of its present
//!   obliquity to its seasonal orbit, labelled, since cold traps record the obliquity's history;
//! - the flux at periapsis, the orbit-mean flux × √(1 − e²) ÷ (1 − e)², and the star's angular
//!   radius there;
//! - the bare regolith's Bond albedo (P14.T51.i), [`REGOLITH_EMISSIVITY`] and the heat flow
//!   (P14.T24.b);
//! - the terrain, stand-ins labelled as such: a crater fraction of [`SATURATED_CRATER_FRACTION`] ×
//!   min(1, N(>100 m) ÷ its saturation), from the body's crater contract and Trask's saturation
//!   ([`CRATER_REFERENCE_DIAMETER`]); Hayne et al.'s distribution B of d/D, the one that passes
//!   every band test (science-r09-drainage.md §8.13.3); plains at an RMS slope of
//!   [`PLAINS_RMS_SLOPE`] (5.7°); and the lunar share under lateral conduction's cut-off
//!   (`LUNAR_SHARE_UNDER_CUTOFF`, 0.05).
//!
//! The published area is `f_ct` × `f_x`, the exposed ice ([`exposed_fraction`]).
//!
//! A body's flat ground, its shadows and its terrain are one `ColdTrapGround`: at any latitude it
//! gives the zonal annual maximum and each threshold's gated fraction
//! (`ColdTrapGround::fractions_at`), a southern latitude its northern mirror's. `f_ct` is the
//! latitude integral of its fractions, and R09.T13.f's field reads its cells' fractions from the
//! same calls, so that the record's area and the field's cells are one model
//! (decision-r09-t13f-cold-traps §3).
//!
//! # The albedo, and the field (P14.T51.i)
//!
//! The flat ground and the shadows read the bare regolith's Bond albedo about the body's hosts'
//! bands, airless (`DerivedBody::regolith_albedo`, T51.b's `Regolith` row: the lid's from
//! P14.T54.a, weathered from P14.T54.b), not the body's area-mix Bond albedo (decision-p14-t51-batch
//! §6). The shadows lie in the regolith's craters, whose walls are the regolith (Ingersoll, Svitek
//! and Murray 1992; Hayne, Aharonson and Schörghofer 2021, Nat. Astron. 5, 169, Methods' energy
//! balance on a rough surface, take 0.12 for the Moon, after Hayne et al. 2017, JGR Planets 122,
//! 2371), and the zonal maximum asks whether
//! bare ground stays below a threshold, a criterion with no history, where the area mix's global
//! mean is neither the bare ground's nor the frost's. On the table's Moon, Mercury and Ceres the two
//! are the Moon's 0.11, so no figure moves.
//!
//! So the zonal annual maxima and every condensable's `f_ct` read only the sky, the regolith's
//! albedo, the heat flow and the terrain, none of which T13.c's passes change. They are one
//! `ColdTrapField`, built once a body (`ColdTrapField::new`); `publish_over` reads it, the
//! record's `publish` builds it once, and from P14.T54.b every pass reads the same field, so
//! the passes and the record agree by construction at one evaluation a body. Its `f_ct` are one
//! latitude integral for every species at once (`over_latitude_each`), each latitude's fractions
//! one call of `ColdTrapGround::fractions_at`, whose closed form shares one depth integral among
//! the thresholds (`hyperion_surface::cold_trap::fractions`, R09.T7.k). The zonal entries and the
//! reservoirs stay each publish's, per partition, and whether a body qualifies is tested at every
//! publish (`qualifies`).
//!
//! # The reservoir
//!
//! A labelled placeholder (the plan: "this task's to source, from neutron and LCROSS figures for
//! the Moon"):
//!
//! - water's column over its cold-trap area is [`WATER_TRAP_COLUMN`], 22.5 kg m⁻²;
//! - carbon dioxide's is [`LCROSS_CO2_PER_WATER`] of water's by number, over its own cold-trap area;
//! - a species the inventory holds takes what is left of its condensed reservoir after its zonal
//!   deposit, at most water's column over its own cold-trap area.
//!
//! Mercury's larger deposits (Lawrence et al. 2013, *from memory*) are not modelled.
//!
//! # What it publishes
//!
//! For each species with a cold trap that holds an area and a reservoir, a solid entry
//! `{ substance, Solid, reservoir, area_fraction = f_ct × f_x, seasonal: false }`, merged with the
//! zonal entry of its substance and phase, or inserted, and the list re-sorted in P14.T51.a's
//! order (`Condensates::with_cold_traps`).
//!
//! # The exposed fraction
//!
//! The visible ice is not the cold-trap area (science-r09-drainage.md §4.3 item 5). Of the three
//! airless bodies observed, the exposed share of the cold-trapping or permanently shadowed area
//! spans more than an order of magnitude, and no model of supply and gardening explains the
//! spread:
//!
//! - Ceres: bright deposits in 10 of at least 634 permanently shadowed craters, 0.016 (Platz et al.
//!   2016, Nat. Astron. 1, 0007, abstract);
//! - the Moon: "only ∼3.5% of cold traps exhibit ice exposures" (Li et al. 2018, PNAS 115, 8907,
//!   read in full);
//! - Mercury: 43–46% of the permanently shadowed area hosts radar-bright deposits (Deutsch et al.
//!   2016, Icarus 280, 158, abstract).
//!
//! All three via science-r09-drainage.md §4.1, which read them. So `f_x` is drawn once a body,
//! log-uniform in [[`EXPOSED_FRACTION_FLOOR`], [`EXPOSED_FRACTION_CEILING`]] on its own
//! [`tags::BODY_COLD_TRAPS`] stream. It is empirical until a delivery and gardening budget exists
//! (with P14.T55.c).
//!
//! # Limits
//!
//! - Equilibrium at the highest sun stands in for the annual maximum, which overstates the peak on
//!   a fast rotator of high thermal inertia.
//! - A peak-temperature form understates the most volatile species' traps: Schorghofer et al. 2021
//!   find 3.5 times the CO₂ area by the time-averaged loss.
//! - The model is symmetric about the equator, where the Moon's south holds about 60% of its traps
//!   (Hayne et al. 2021).
//! - The obliquity is the present one, and the delivered species lie only in traps.
//!
//! # Determinism
//!
//! One new tag, [`tags::BODY_COLD_TRAPS`], keyed by the body; word 0 read, words 1–7 reserved
//! ([`EXPOSED_FRACTION_WORDS`]). No existing stream gains a draw. The integral sums its bands and
//! nodes in their fixed order, the species are taken in registry order, and nothing is cached: a
//! field is a value its caller builds and passes, never kept behind a call.

#[cfg(test)]
mod tests;

use core::f64::consts::{FRAC_PI_2, PI};

use hyperion_surface::cold_trap::{
    self, BodyParts, DepthRatios, LUNAR_SHARE_UNDER_CUTOFF, Terrain, TerrainParts,
    flux_weighted_elevation,
};
use hyperion_surface::craters::{CraterParams, cumulative_density, saturation};

use crate::id::BodyId;
use crate::math;
use crate::planetary::chem::partition::{
    AIRLESS_ICE_LOSS_KG_PER_M2_GYR, CondensatePhase, Condensates, Cover, Partition, Partitioned,
    SolidEdges, TrappedIce, exosphere_threshold,
};
use crate::planetary::derive::atmosphere::{
    ATOMIC_MASS_CONSTANT_KG, Atmosphere, Gas, PartialPressures, SurfaceState,
};
use crate::planetary::hooks::figures::Crust;
use crate::planetary::hooks::surface::{ColdProfile, REGOLITH_EMISSIVITY, area_below};
use crate::rng::{ObjectKey, Stream, tags};
use crate::substance::SubstanceId;
use crate::tables::gauss_legendre::{GL4_NODES, GL4_WEIGHTS};
use crate::units::consts::{SOLAR_RADIUS_M, STEFAN_BOLTZMANN};
use crate::units::{
    Degrees, Kelvin, KilogramsPerSquareMetre, Metres, MetresPerSecondSquared, Pascals, Radians,
    SolarRadii, WattsPerSquareMetre,
};
use crate::{GENERATOR_VERSION, GeneratorVersion, Seed};

/// The generator version from which the cold traps and their airless branch are on the record: 24,
/// the 23 → 24 batch's bump, P14.T54.b (decision-bump-plan-after-22 §1.3, a rider "behind a hold at
/// 24 on its change to T51.a's airless branch").
///
/// Below it [`ColdTrapRules::in_force`] is [`ColdTrapRules::Held`], so that every published
/// condensate is P14.T51.a's bit for bit. P14.T54.b deletes this hold, [`ColdTrapRules`] and its
/// `Held` arm, and its tripwire (`the_cold_traps_are_held_only_until_the_24_bump`).
pub(crate) const COLD_TRAPS_VERSION: GeneratorVersion = GeneratorVersion::new(24);

/// Which rules the published condensates follow on an airless body (the cold traps' hold,
/// [`COLD_TRAPS_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum ColdTrapRules {
    /// Versions 22's and 23's: P14.T51.a's published condensates, its airless branch keyed on the
    /// annual mean, with no cold traps.
    Held,
    /// The 23 → 24 batch's: on a body with no collisional atmosphere that is not locked to its
    /// star, the airless branch keyed on the zonal annual maximum and the cold traps ([`publish`]).
    Batch,
}

impl ColdTrapRules {
    /// The rules this generator version gives: the batch's from [`COLD_TRAPS_VERSION`], and P14.T51.a's
    /// below it.
    #[must_use]
    pub(crate) fn in_force() -> Self {
        if GENERATOR_VERSION >= COLD_TRAPS_VERSION {
            Self::Batch
        } else {
            Self::Held
        }
    }
}

/// The collision cross-section `σ_c` whose inverse bounds a collisional column's number density:
/// 3 × 10⁻¹⁹ m², a hard-sphere cross-section of N₂ and O, *from memory* (labelled). N₂'s kinetic
/// diameter is 364 pm (Ismail et al. 2015, through a secondary), so its π d² is 4.16 × 10⁻¹⁹ m²,
/// against which this is 28% low: kept, and recorded in plan 14's Risks. An air whose column
/// p ÷ (m̄ g) holds fewer molecules than 1 ÷ `σ_c` per square metre has its exobase at the ground.
pub const COLLISION_CROSS_SECTION_M2: f64 = 3e-19;

/// The species impactors deliver to any body, which every airless body is tested for cold traps of
/// whatever its inventory: water and carbon dioxide (science-r09-drainage.md §4; plan 14's
/// P14.T51.f), the two whose lunar columns LCROSS measured (Colaprete et al. 2010, Science 330,
/// 463).
pub const DELIVERED: [Gas; 2] = [Gas::Water, Gas::CarbonDioxide];

/// The crater fraction of a surface whose 100 m craters are at saturation: 0.2, Hayne et al.
/// 2021's lunar best fit (Nat. Astron. 5, 169, "20% craters" with plains at 5.7°, within their
/// 20–50%), labelled a stand-in for every body until the crater state gives one.
pub const SATURATED_CRATER_FRACTION: f64 = 0.2;

/// The diameter at which a surface's crater density is compared with Trask's saturation for its
/// crater fraction: 100 m, the lane's stand-in (labelled).
pub const CRATER_REFERENCE_DIAMETER: Metres = Metres::new(100.0);

/// The plains' directional RMS slope as its tangent: 0.1, 5.7°, Hayne et al. 2021's best single
/// fit to the Moon's LROC shadows (as `hyperion_surface::cold_trap::TerrainParts` cites it),
/// labelled a stand-in for every body.
pub const PLAINS_RMS_SLOPE: f64 = 0.1;

/// Water's column over its cold-trap area, kg m⁻²: 22.5, a labelled placeholder. It is Lunar
/// Prospector's about 1.5 wt% of water-equivalent hydrogen if the hydrogen is confined to the polar
/// shadows (Feldman et al. 2001, JGR 106, 23231, *from memory*), over 1 m of regolith at 1,500 kg
/// m⁻³ (the lunar regolith's bulk density near the surface, *from memory*). LCROSS's 5.6 ± 2.9 wt%
/// (Colaprete et al. 2010, Science 330, 463, not read here), about
/// 84 kg m⁻² over the same metre, is the upper check.
pub const WATER_TRAP_COLUMN: KilogramsPerSquareMetre = KilogramsPerSquareMetre::new(22.5);

/// Carbon dioxide's share of LCROSS's water by number: 0.0217, 2.17% (Colaprete et al. 2010,
/// Science 330, 463, Table 1, *from memory*), a labelled placeholder. Carbon dioxide's column over
/// its own cold-trap area is this times [`WATER_TRAP_COLUMN`] times its molar mass over water's,
/// about 1.19 kg m⁻².
pub const LCROSS_CO2_PER_WATER: f64 = 0.0217;

/// The edges of the latitude bands the cold-trap fraction is integrated over, degrees from the
/// equator: four-point Gauss–Legendre rules on 0–60°, 60–70°, 70–80°, 80–85°, 85–88° and 88–90°,
/// 24 nodes, finer toward the pole where the traps lie (the research's): within 5 × 10⁻⁴ of a
/// midpoint rule at 0.02°, the tests' bound (−3.99 × 10⁻⁴ measured with craters of distribution B,
/// +1.07 × 10⁻⁴ with A).
pub const TRAP_BAND_EDGES_DEG: [f64; 7] = [0.0, 60.0, 70.0, 80.0, 85.0, 88.0, 90.0];

/// The least exposed fraction `f_x`: 0.016, Ceres's bright deposits in 10 of at least 634
/// permanently shadowed craters (10 ÷ 634 = 0.0158; Platz et al. 2016, Nat. Astron. 1, 0007,
/// abstract, via science-r09-drainage.md §4.1). Empirical until a delivery and gardening budget
/// exists (with P14.T55.c).
pub const EXPOSED_FRACTION_FLOOR: f64 = 0.016;

/// The greatest exposed fraction `f_x`: 0.45, within Mercury's 43–46% of its permanently shadowed
/// area that hosts radar-bright deposits (Deutsch et al. 2016, Icarus 280, 158, abstract, via
/// science-r09-drainage.md §4.1). Empirical until a delivery and gardening budget exists (with
/// P14.T55.c).
pub const EXPOSED_FRACTION_CEILING: f64 = 0.45;

/// Words of [`tags::BODY_COLD_TRAPS`] a body reads or reserves: word 0, the exposed fraction's
/// rank, then seven reserved.
pub const EXPOSED_FRACTION_WORDS: u64 = 8;

/// The share `f_x` of the body `body`'s cold traps whose ice is exposed, in the universe of `seed`
/// (P14.T51.f): [`EXPOSED_FRACTION_FLOOR`] × ([`EXPOSED_FRACTION_CEILING`] ÷
/// [`EXPOSED_FRACTION_FLOOR`])^u, u the open uniform of word 0 of its [`tags::BODY_COLD_TRAPS`]
/// stream, so log-uniform in [0.016, 0.45] and held to it. No other word is read.
///
/// The bounds are Ceres's 0.016 and Mercury's 0.43–0.46, with the Moon's 0.035 between them (the
/// [module](self) documentation). It is empirical until a delivery and gardening budget exists
/// (with P14.T55.c). Dimensionless.
///
/// # Examples
///
/// The fraction depends on the seed and the body's ID and nothing else:
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::id::{BodyId, SystemId};
/// use hyperion_sim::planetary::hooks::cold_traps::exposed_fraction;
///
/// let system = SystemId::from_raw(0x0200_0800_2000_0000)?;
/// let moon = BodyId::new(system, 0x1300);
/// let f = exposed_fraction(Seed::new(7), moon);
/// assert!((0.016..=0.45).contains(&f));
/// assert_eq!(f.to_bits(), exposed_fraction(Seed::new(7), moon).to_bits());
/// # Ok::<(), hyperion_sim::id::DecodeSystemIdError>(())
/// ```
#[must_use]
pub fn exposed_fraction(seed: Seed, body: BodyId) -> f64 {
    let mut stream = Stream::open(seed, tags::BODY_COLD_TRAPS, ObjectKey::from(body));
    let u = stream.uniform_open();
    let f =
        EXPOSED_FRACTION_FLOOR * math::powf(EXPOSED_FRACTION_CEILING / EXPOSED_FRACTION_FLOOR, u);
    f.clamp(EXPOSED_FRACTION_FLOOR, EXPOSED_FRACTION_CEILING)
}

/// What a body's cold traps read of its sky and its draw, from the record path (P14.T51.f): the
/// star's greatest declination, its flux and angular radius at periapsis, and the exposed fraction.
///
/// Plain data with crate-visible fields.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ColdTrapSky {
    /// The star's greatest declination δ in the body's sky, rad in [0, π/2]: min(ε, π − ε) of its
    /// present obliquity, labelled (the obliquity's history is not modelled).
    pub(crate) greatest_declination: Radians,
    /// The flux at periapsis `S_p`, W m⁻².
    pub(crate) periapsis_flux: WattsPerSquareMetre,
    /// The star's angular radius α at periapsis, rad: 0 for a point.
    pub(crate) star_radius: Radians,
    /// The exposed fraction `f_x` ([`exposed_fraction`]).
    pub(crate) exposed_fraction: f64,
}

impl ColdTrapSky {
    /// The sky of a body of obliquity `obliquity` to its seasonal orbit, of semi-major axis and
    /// eccentricity `orbit`, under an orbit-mean flux `orbit_mean_flux` from a star of radius
    /// `star_radius` (`None` for none, a point), with the exposed fraction `exposed_fraction`.
    ///
    /// - δ = min(ε, π − ε) ([`greatest_declination`]), so that a retrograde rotator such as Venus
    ///   (177.4°) has 2.6°.
    /// - `S_p` = F √(1 − e²) ÷ (1 − e)² ([`periapsis_flux`]).
    /// - α = asin(R ÷ a (1 − e)), and π/2 where the star would fill the sky, which the cold traps'
    ///   inputs then refuse.
    ///
    /// δ and `S_p` are [`ColdTrapGround::new`]'s arithmetic, so that a ground built from the same
    /// obliquity, flux and eccentricity sees this sky to the bit.
    #[must_use]
    pub(crate) fn at_periapsis(
        obliquity: Radians,
        orbit_mean_flux: WattsPerSquareMetre,
        (semi_major_axis, eccentricity): (Metres, f64),
        star_radius: Option<SolarRadii>,
        exposed_fraction: f64,
    ) -> Self {
        let near = 1.0 - eccentricity;
        let ratio = star_radius.map_or(0.0, |r| {
            r.value() * SOLAR_RADIUS_M / (semi_major_axis.value() * near)
        });
        let angular = if ratio < 1.0 {
            math::asin(ratio)
        } else {
            FRAC_PI_2
        };
        Self {
            greatest_declination: greatest_declination(obliquity),
            periapsis_flux: periapsis_flux(orbit_mean_flux, eccentricity),
            star_radius: Radians::new(angular),
            exposed_fraction,
        }
    }
}

/// The star's greatest declination δ in the sky of a body of obliquity `obliquity` to its seasonal
/// orbit, rad in [0, π/2] for an obliquity in [0, π]: min(ε, π − ε), the present obliquity's,
/// labelled (the obliquity's history is not modelled).
#[must_use]
fn greatest_declination(obliquity: Radians) -> Radians {
    let tilt = obliquity.value();
    let reversed = PI - tilt;
    Radians::new(if reversed < tilt { reversed } else { tilt })
}

/// The flux at periapsis `S_p`, W m⁻², of the orbit-mean flux `orbit_mean_flux` on a seasonal orbit
/// of eccentricity `eccentricity`, in [0, 1): F √(1 − e²) ÷ (1 − e)², the orbit mean
/// L ÷ (4π a² √(1 − e²)) taken to L ÷ (4π a² (1 − e)²) (P14.T12's mean flux).
#[must_use]
fn periapsis_flux(orbit_mean_flux: WattsPerSquareMetre, eccentricity: f64) -> WattsPerSquareMetre {
    let e = eccentricity;
    let near = 1.0 - e;
    WattsPerSquareMetre::new(orbit_mean_flux.value() * (1.0 - e * e).sqrt() / (near * near))
}

/// The Moon's sky as the plan's tests take it (science-r09-drainage.md §8; R09.T7.d's real Moon):
/// the Sun at most 1.54° from its equator, 1,361 W m⁻² at 1 au taken to Earth's perihelion,
/// 0.98329 au, the Sun's 0.2711° there, and the exposed fraction Li et al. 2018's 0.035.
#[cfg(test)]
#[must_use]
pub(crate) fn lunar_sky() -> ColdTrapSky {
    ColdTrapSky {
        greatest_declination: Radians::from(Degrees::new(1.54)),
        periapsis_flux: WattsPerSquareMetre::new(1361.0 / (0.98329 * 0.98329)),
        star_radius: Radians::from(Degrees::new(0.2711)),
        exposed_fraction: 0.035,
    }
}

/// What the record's cold traps read beyond a body's figures and conditions
/// (`GlobalFigures::condensates_under`; P14.T51.f, P14.T51.i): their rules, their sky and draw, and
/// the bare regolith's Bond albedo its derivation gives.
///
/// Plain data with crate-visible fields.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RecordColdTraps<'a> {
    /// Which rules they follow, their hold at 24.
    pub(crate) rules: ColdTrapRules,
    /// The body's sky and its exposed fraction.
    pub(crate) sky: &'a ColdTrapSky,
    /// The bare regolith's Bond albedo about the body's hosts' bands, airless
    /// (`DerivedBody::regolith_albedo`), 0 to 1.
    pub(crate) regolith_albedo: f64,
}

#[cfg(test)]
impl<'a> RecordColdTraps<'a> {
    /// The record's cold traps under `rules` and the sky `sky`, at the Bond albedo of the
    /// atmosphere `air` as the regolith's: what the tests of bodies whose regolith is the airless
    /// rock's take, the Moon's 0.11, so that their figures are P14.T51.f's.
    #[must_use]
    pub(crate) fn at_the_albedo_of(
        rules: ColdTrapRules,
        sky: &'a ColdTrapSky,
        air: &Atmosphere,
    ) -> Self {
        Self {
            rules,
            sky,
            regolith_albedo: air.albedo().value(),
        }
    }
}

/// What a body's cold traps read beyond its partition and its cover, on the record path
/// (`GlobalFigures::condensates_under`).
///
/// Plain data with crate-visible fields.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ColdTrapBody<'a> {
    /// Its atmosphere: its state, its pressure and gases, which decide whether it qualifies. Its
    /// area-mix Bond albedo is not read (P14.T51.i).
    pub(crate) atmosphere: &'a Atmosphere,
    /// Its conditions' annual-mean profile, whose area points the zonal annual maximum takes, and
    /// which tells a locked body.
    pub(crate) profile: &'a ColdProfile,
    /// Its surface gravity.
    pub(crate) gravity: MetresPerSecondSquared,
    /// Its heat flow (P14.T24.b), W m⁻².
    pub(crate) heat_flow: WattsPerSquareMetre,
    /// Its crust: an ice-rich one keeps P14.T24.b's water ice.
    pub(crate) crust: Crust,
    /// Its crater contract (P14.T48.c), which sets its crater fraction.
    pub(crate) craters: &'a CraterParams,
    /// Its sky and its exposed fraction.
    pub(crate) sky: &'a ColdTrapSky,
    /// Its bare regolith's Bond albedo about its hosts' bands, airless (P14.T51.i;
    /// `DerivedBody::regolith_albedo`), 0 to 1, which its flat ground and its shadows read.
    pub(crate) regolith_albedo: f64,
}

/// The published condensates of the partition `partition` over the cover `cover` with the cold
/// traps (P14.T51.f, under [`ColdTrapRules::Batch`]): P14.T51.a's publish on a body that does not
/// qualify ([`qualifies`]), and on one that does the zonal annual maximum's branch
/// ([`SolidEdges::Exosphere`]) with each condensable's cold-trapped ice merged in, over the field
/// it builds once ([`ColdTrapField::of`], P14.T51.i), at the regolith albedo its derivation always
/// gives (`DerivedBody::regolith_albedo`). A body whose profile is not zonal, which has no field,
/// keeps P14.T51.a's publish. From P14.T54.b the record takes the passes' field instead
/// ([`publish_over`]), so that a body builds one field, not one for its passes and one here.
#[must_use]
pub(crate) fn publish(
    partition: &Partition,
    cover: &Cover<'_>,
    body: &ColdTrapBody<'_>,
) -> Condensates {
    if !qualifies(body) {
        return partition.publish(cover);
    }
    let Some(field) = ColdTrapField::of(body, partition) else {
        return partition.publish(cover);
    };
    published_over(partition, cover, body, &field)
}

/// [`publish`] over the field `field`, built once a body (P14.T51.i; decision-p14-t51-batch §6): the
/// one function T13.c's passes call from P14.T54.b, every pass over the same field, so that the
/// record's condensates and the last pass's agree to the bit. Whether the body qualifies is tested
/// here, at every call ([`qualifies`]); the field holds what no pass changes. It must hold every
/// condensable of `partition` that has a threshold ([`ColdTrapField::of`]'s list, or a superset).
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "P14.T54.b's passes call it; the record path builds its own"
    )
)]
#[must_use]
pub(crate) fn publish_over(
    partition: &Partition,
    cover: &Cover<'_>,
    body: &ColdTrapBody<'_>,
    field: &ColdTrapField,
) -> Condensates {
    if !qualifies(body) {
        return partition.publish(cover);
    }
    published_over(partition, cover, body, field)
}

/// The qualifying body `body`'s condensates over the field `field`: the zonal annual maximum's
/// branch, with its traps merged in.
#[must_use]
fn published_over(
    partition: &Partition,
    cover: &Cover<'_>,
    body: &ColdTrapBody<'_>,
    field: &ColdTrapField,
) -> Condensates {
    let warm_below = |t: Kelvin| area_below(&field.maxima, t);
    let zonal = partition.publish_with(
        &Cover {
            cold_area: &warm_below,
            state: cover.state,
            water: cover.water,
        },
        SolidEdges::Exosphere {
            water_keeps_cover: body.crust == Crust::IceRich,
        },
    );
    let traps = trapped_ices(partition, &zonal, body, field);
    zonal.with_cold_traps(&traps)
}

/// Whether the body `body` takes the cold traps ([the module](self)'s "Which bodies"): it rotates
/// under its star (a zonal profile), it is airless and its air is not collisional
/// ([`is_collisional`]). It reads the pass's atmosphere, so it is tested at every publish; the
/// field does not read it.
#[must_use]
pub(crate) fn qualifies(body: &ColdTrapBody<'_>) -> bool {
    matches!(body.profile, ColdProfile::Zonal(_))
        && body.atmosphere.state() == SurfaceState::Airless
        && !is_collisional(body.atmosphere, body.gravity)
}

/// A body's cold-trap field (P14.T51.i; decision-p14-t51-batch §6): its zonal annual maxima at its
/// profile's area points and each condensable's cold-trap fraction `f_ct`, which read only its
/// sky, its bare regolith's Bond albedo, its heat flow and its terrain ([the module](self)'s "The
/// albedo, and the field"). T13.c's passes change none of these, so it is built once a body and
/// read by every publish ([`publish_over`]).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ColdTrapField {
    /// The zonal annual maximum at each area point, `(x, T_max)` with x the sine of the latitude
    /// from 0 to 1, K.
    maxima: Vec<(f64, f64)>,
    /// Each condensable with a threshold, in the order given, with its `f_ct`.
    traps: Vec<FieldTrap>,
}

/// One condensable's place in a [`ColdTrapField`]: its threshold and its cold-trap fraction.
#[derive(Debug, Clone, Copy, PartialEq)]
struct FieldTrap {
    substance: SubstanceId,
    threshold: Kelvin,
    fraction: f64,
}

impl ColdTrapField {
    /// The field of the flat ground `sunlit` over the terrain `terrain` at the area points
    /// `points` (their x read alone), for the condensables `species`, each at its
    /// [`trap_threshold`] (one with none is left out).
    ///
    /// Each `f_ct` is the latitude integral of the ground's [`ColdTrapGround::fractions_at`],
    /// one call a latitude node for every species at once, each species' sum in
    /// `over_latitude`'s order ([`over_latitude_each`]), so that it equals the one-species
    /// integral to the bit. Where the closed form refuses the ground (a star that gives no light or
    /// fills the sky, or an albedo of 1), every `f_ct` is 0 and the maxima stand.
    ///
    /// # Panics
    ///
    /// If an area point's x is outside [−1, 1].
    #[must_use]
    pub(crate) fn new(
        sunlit: &Sunlit,
        terrain: Terrain,
        points: &[(f64, f64)],
        species: &[SubstanceId],
    ) -> Self {
        let listed: Vec<(SubstanceId, Kelvin)> = species
            .iter()
            .filter_map(|&substance| trap_threshold(substance).map(|t| (substance, t)))
            .collect();
        let thresholds: Vec<Kelvin> = listed.iter().map(|&(_, t)| t).collect();
        let mut fractions = vec![0.0; listed.len()];
        if let Some(ground) = ColdTrapGround::under(*sunlit, terrain)
            && !thresholds.is_empty()
        {
            over_latitude_each(&mut fractions, |latitude, values| {
                ground.fractions_at(latitude, &thresholds, values);
            });
        }
        Self {
            maxima: sunlit.maxima(points),
            traps: listed
                .into_iter()
                .zip(fractions)
                .map(|((substance, threshold), fraction)| FieldTrap {
                    substance,
                    threshold,
                    fraction,
                })
                .collect(),
        }
    }

    /// The record path's field of the body `body` of partition `partition`: its ground under its
    /// sky at its regolith's albedo, over [`terrain_of`] its craters, at its profile's area points,
    /// for its [`condensables`], water left out on an ice-rich crust, which keeps P14.T24.b's ice.
    /// `None` where its profile is not zonal.
    #[must_use]
    pub(crate) fn of(body: &ColdTrapBody<'_>, partition: &Partition) -> Option<Self> {
        let albedo = body.regolith_albedo;
        let ColdProfile::Zonal(points) = body.profile else {
            return None;
        };
        let sunlit = Sunlit::new(*body.sky, albedo, body.heat_flow);
        let water = Gas::Water.substance();
        let species: Vec<SubstanceId> = condensables(partition)
            .into_iter()
            .filter(|&substance| !(substance == water && body.crust == Crust::IceRich))
            .collect();
        Some(Self::new(
            &sunlit,
            terrain_of(body.craters),
            points,
            &species,
        ))
    }

    /// The zonal annual maximum at each area point, `(x, T_max)`, K.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn maxima(&self) -> &[(f64, f64)] {
        &self.maxima
    }

    /// The threshold and the cold-trap fraction `f_ct` of `substance`, a share of the whole
    /// surface, 0 to 1; `None` where the field holds no such condensable.
    #[must_use]
    pub(crate) fn trap(&self, substance: SubstanceId) -> Option<(Kelvin, f64)> {
        self.traps
            .iter()
            .find(|trap| trap.substance == substance)
            .map(|trap| (trap.threshold, trap.fraction))
    }
}

/// Whether the air of `atmosphere` is collisional at the surface of a body of surface gravity
/// `gravity`: p ≥ m̄ g ÷ `σ_c` ([`collisional`]), m̄ its column's mean molecular mass, or nitrogen's
/// where it is empty; true for a gas envelope, which has no surface.
#[must_use]
pub(crate) fn is_collisional(atmosphere: &Atmosphere, gravity: MetresPerSecondSquared) -> bool {
    let Some(pressure) = atmosphere.surface_pressure() else {
        return true;
    };
    is_collisional_air(pressure, atmosphere.partial_pressures(), gravity)
}

/// Whether an air of surface pressure `pressure` and gases `partials` is collisional at the
/// surface of a body of surface gravity `gravity` ([`is_collisional`]'s test, its arithmetic):
/// P14.T51.f's, which P14.T51.j's oxidised surface reads too.
#[must_use]
pub(crate) fn is_collisional_air(
    pressure: Pascals,
    partials: &PartialPressures,
    gravity: MetresPerSecondSquared,
) -> bool {
    let moles = Gas::ALL.iter().fold(0.0, |sum, &gas| {
        sum + partials.of(gas).value() / gas.molar_mass_g_per_mol()
    });
    let molar_mass = if moles > 0.0 {
        pressure.value() / moles
    } else {
        Gas::Nitrogen.molar_mass_g_per_mol()
    };
    collisional(pressure, molar_mass, gravity)
}

/// Whether a column of surface pressure `pressure`, mean molar mass `molar_mass_g_per_mol` and
/// gravity `gravity` holds at least 1 ÷ `σ_c` molecules per square metre, p ÷ (m̄ g) ≥ 1 ÷ `σ_c`:
/// its exobase above the ground. A NaN is collisional, so that it takes no cold traps.
#[must_use]
pub(crate) fn collisional(
    pressure: Pascals,
    molar_mass_g_per_mol: f64,
    gravity: MetresPerSecondSquared,
) -> bool {
    let molecule = molar_mass_g_per_mol * ATOMIC_MASS_CONSTANT_KG;
    let (column, weight) = (
        pressure.value() * COLLISION_CROSS_SECTION_M2,
        molecule * gravity.value(),
    );
    column.is_nan() || weight.is_nan() || column >= weight
}

/// A body's flat ground and its shadows under its star: what both its zonal annual maximum and its
/// cold traps read. [`ColdTrapGround`] adds its terrain.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Sunlit {
    greatest_declination: Radians,
    periapsis_flux: WattsPerSquareMetre,
    star_radius: Radians,
    albedo: f64,
    heat_flow: WattsPerSquareMetre,
}

impl Sunlit {
    /// The ground under the sky `sky`, of Bond albedo `albedo` and heat flow `heat_flow`: the sky's
    /// greatest declination, flux at periapsis and angular radius.
    #[must_use]
    pub(crate) const fn new(sky: ColdTrapSky, albedo: f64, heat_flow: WattsPerSquareMetre) -> Self {
        Self {
            greatest_declination: sky.greatest_declination,
            periapsis_flux: sky.periapsis_flux,
            star_radius: sky.star_radius,
            albedo,
            heat_flow,
        }
    }

    /// The zonal annual maximum at the colatitude `colatitude`, in [0, π/2]: flat ground in
    /// equilibrium with the star at its highest, σT⁴ = ((1 − A) `S_p` sin e + q) ÷ ε, e the disk's
    /// flux-weighted elevation at min(π/2, e₀ + δ) ([the module](self)'s "The zonal annual
    /// maximum").
    #[must_use]
    pub(crate) fn annual_maximum(&self, colatitude: Radians) -> Kelvin {
        let rising = colatitude.value() + self.greatest_declination.value();
        let highest = if rising < FRAC_PI_2 {
            rising
        } else {
            FRAC_PI_2
        };
        let e = flux_weighted_elevation(Radians::new(highest), self.star_radius).value();
        let absorbed = (1.0 - self.albedo) * self.periapsis_flux.value() * math::sin(e);
        let power = (absorbed + self.heat_flow.value()) / (REGOLITH_EMISSIVITY * STEFAN_BOLTZMANN);
        Kelvin::new(power.sqrt().sqrt())
    }

    /// The zonal annual maximum at the latitude `latitude`, rad in [−π/2, π/2], symmetric about the
    /// equator: [`annual_maximum`](Self::annual_maximum) at the colatitude of its northern mirror.
    ///
    /// # Panics
    ///
    /// If the latitude is outside [−π/2, π/2].
    #[must_use]
    fn annual_maximum_at(&self, latitude: Radians) -> Kelvin {
        let north = northern_mirror(latitude);
        self.annual_maximum(Radians::new(FRAC_PI_2 - north.value()))
    }

    /// The zonal annual maximum at the area points of `points`, `(x, T)` with x the sine of the
    /// latitude from 0 to 1, as `(x, T_max)`, in their order: each at its latitude by
    /// [`ColdTrapGround::annual_maximum`]'s rule.
    ///
    /// # Panics
    ///
    /// If an x is outside [−1, 1].
    #[must_use]
    pub(crate) fn maxima(&self, points: &[(f64, f64)]) -> Vec<(f64, f64)> {
        points
            .iter()
            .map(|&(x, _)| {
                let latitude = Radians::new(math::asin(x));
                (x, self.annual_maximum_at(latitude).value())
            })
            .collect()
    }

    /// The cold traps' body-wide inputs, or `None` where `hyperion_surface::cold_trap::Body`
    /// refuses them: a star that gives no light, fills the sky, or an albedo of 1.
    #[must_use]
    pub(crate) fn shadows(&self) -> Option<cold_trap::Body> {
        // A body the closed form refuses has no cold traps; its zonal annual maximum still lays
        // its zonal ices, so nothing is lost by dropping the error.
        cold_trap::Body::new(BodyParts {
            flux: self.periapsis_flux,
            greatest_declination: self.greatest_declination,
            star_radius: self.star_radius,
            bond_albedo: self.albedo,
            emissivity: REGOLITH_EMISSIVITY,
            heat_flow: self.heat_flow,
        })
        .ok()
    }

    /// The cold-trap fraction `f_ct` of a species of threshold `threshold` over the terrain
    /// `terrain` under the shadows `shadows`: the latitude integral ([`over_latitude`]) of the
    /// ground's gated fraction ([`ColdTrapGround::fractions_at`]), the closed form at the latitudes
    /// whose zonal annual maximum is at least the threshold, so that it never lies under the zonal
    /// cap. A share of the whole surface, 0 to 1.
    ///
    /// The record path takes it through [`ColdTrapGround`] over [`terrain_of`]'s terrain. The tests
    /// take it here over terrains of their own, Hayne et al.'s distribution A among them.
    ///
    /// # Panics
    ///
    /// If the threshold is not finite and positive.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn trap_fraction(
        &self,
        shadows: &cold_trap::Body,
        terrain: &Terrain,
        threshold: Kelvin,
    ) -> f64 {
        ColdTrapGround {
            sunlit: *self,
            shadows: *shadows,
            terrain: *terrain,
        }
        .trap_fraction(threshold)
    }
}

/// The northern mirror |φ| of the latitude `latitude`, rad in [0, π/2]: the cold traps are
/// symmetric about the equator, and a southern latitude's own φ would put the star's highest
/// elevation, e₀ + δ from the colatitude π/2 − φ, past the zenith.
///
/// # Panics
///
/// If the latitude is outside [−π/2, π/2].
#[must_use]
fn northern_mirror(latitude: Radians) -> Radians {
    let phi = latitude.value();
    assert!(
        (-FRAC_PI_2..=FRAC_PI_2).contains(&phi),
        "a latitude must be in [-π/2, π/2], got {phi} rad"
    );
    latitude.abs()
}

/// What a body's cold traps read of its sky and its ground (P14.T51.f; R09.T13.f), each in the
/// unit its type states.
///
/// Plain data with crate-visible fields: [`ColdTrapGround::new`] reads them.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ColdTrapGroundParts<'a> {
    /// The obliquity ε of the spin axis to the seasonal orbit, rad in [0, π]. The star's greatest
    /// declination is min(ε, π − ε), the present one (labelled).
    pub(crate) obliquity: Radians,
    /// The orbit-mean flux of every host at the body, W m⁻² (P14.T12's sum), taken to periapsis as
    /// F √(1 − e²) ÷ (1 − e)².
    pub(crate) orbit_mean_flux: WattsPerSquareMetre,
    /// The seasonal orbit's eccentricity e, in [0, 1).
    pub(crate) eccentricity: f64,
    /// The star's angular radius α at periapsis, rad in [0, π/2): 0 for a point.
    pub(crate) star_radius: Radians,
    /// The bare ground's Bond albedo A, in [0, 1).
    pub(crate) bond_albedo: f64,
    /// The internal heat flow q (P14.T24.b), W m⁻².
    pub(crate) heat_flow: WattsPerSquareMetre,
    /// The crater contract (P14.T48.c), from which [`terrain_of`] takes the terrain's stand-ins.
    pub(crate) craters: &'a CraterParams,
}

/// A body's cold traps as functions of latitude (P14.T51.f): its zonal annual maximum, and each
/// threshold's closed-form cold-trap fraction behind it. P14.T51.f's latitude integral and
/// R09.T13.f's per-cell placement both read it, so the record's area and the field's cells are
/// one model.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ColdTrapGround {
    /// The flat ground under the star, for the zonal annual maximum.
    sunlit: Sunlit,
    /// The closed form's body-wide inputs, the same ground's.
    shadows: cold_trap::Body,
    /// The terrain whose shadows trap.
    terrain: Terrain,
}

impl ColdTrapGround {
    /// The ground of `parts`, with δ and `S_p` by the module's arithmetic
    /// ([`ColdTrapSky::at_periapsis`]'s), ε [`REGOLITH_EMISSIVITY`] and the terrain
    /// [`terrain_of`]'s. `None` where `hyperion_surface::cold_trap::Body` refuses them (a star that
    /// gives no light or fills the sky, or an albedo of 1): such a body has no cold traps.
    ///
    /// # Panics
    ///
    /// If a part lies outside its documented range: an obliquity outside [0, π], an eccentricity
    /// outside [0, 1), a flux, star radius or heat flow that is negative or not finite, or an albedo
    /// outside [0, 1]. Those are a caller's fault, not a body without cold traps.
    #[must_use]
    pub(crate) fn new(parts: &ColdTrapGroundParts<'_>) -> Option<Self> {
        let obliquity = parts.obliquity.value();
        assert!(
            (0.0..=PI).contains(&obliquity),
            "an obliquity must be in [0, π], got {obliquity} rad"
        );
        let e = parts.eccentricity;
        assert!(
            (0.0..1.0).contains(&e),
            "an eccentricity must be in [0, 1), got {e}"
        );
        for (part, value) in [
            ("flux", parts.orbit_mean_flux.value()),
            ("star radius", parts.star_radius.value()),
            ("heat flow", parts.heat_flow.value()),
        ] {
            assert!(
                value.is_finite() && value >= 0.0,
                "a {part} must be finite and non-negative, got {value}"
            );
        }
        let albedo = parts.bond_albedo;
        assert!(
            (0.0..=1.0).contains(&albedo),
            "a Bond albedo must be in [0, 1], got {albedo}"
        );
        let sunlit = Sunlit {
            greatest_declination: greatest_declination(parts.obliquity),
            periapsis_flux: periapsis_flux(parts.orbit_mean_flux, parts.eccentricity),
            star_radius: parts.star_radius,
            albedo: parts.bond_albedo,
            heat_flow: parts.heat_flow,
        };
        Self::under(sunlit, terrain_of(parts.craters))
    }

    /// The flat ground `sunlit` over the terrain `terrain`, or `None` where its shadows are
    /// refused ([`Sunlit::shadows`]).
    #[must_use]
    fn under(sunlit: Sunlit, terrain: Terrain) -> Option<Self> {
        let shadows = sunlit.shadows()?;
        Some(Self {
            sunlit,
            shadows,
            terrain,
        })
    }

    /// The zonal annual maximum at `latitude`, rad in [−π/2, π/2], K: flat ground in equilibrium
    /// with the star at its highest, σT⁴ = ((1 − A) `S_p` sin e + q) ÷ ε (the module's "The zonal
    /// annual maximum"). It is symmetric about the equator.
    ///
    /// # Panics
    ///
    /// If the latitude is outside [−π/2, π/2].
    #[must_use]
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "only the tests read it: R09.T13.f's placement reads `fractions_at`, which \
                      applies the zonal maximum itself"
        )
    )]
    pub(crate) fn annual_maximum(&self, latitude: Radians) -> Kelvin {
        self.sunlit.annual_maximum_at(latitude)
    }

    /// The cold-trap fraction at `latitude`, rad in [−π/2, π/2], of each threshold of
    /// `thresholds`, written to `fractions` in their order. Each is a share, 0 to 1, of the ground
    /// at that latitude:
    /// - `hyperion_surface::cold_trap::fraction` where the zonal annual maximum there is at least
    ///   the threshold;
    /// - 0 where it is below, so the traps never lie under the zonal cap.
    ///
    /// It is symmetric about the equator: a southern latitude reads its northern mirror. It does
    /// not fall as a threshold rises up to the zonal annual maximum there, and is 0 above it, under
    /// the zonal cap.
    ///
    /// # Panics
    ///
    /// If `fractions` and `thresholds` differ in length, the latitude is outside [−π/2, π/2], or a
    /// threshold is not finite and positive.
    pub(crate) fn fractions_at(
        &self,
        latitude: Radians,
        thresholds: &[Kelvin],
        fractions: &mut [f64],
    ) {
        assert_eq!(
            thresholds.len(),
            fractions.len(),
            "the thresholds and their fractions must be as many"
        );
        let north = northern_mirror(latitude);
        for threshold in thresholds {
            let t = threshold.value();
            assert!(
                t.is_finite() && t > 0.0,
                "a threshold must be finite and positive, got {t} K"
            );
        }
        let maximum = self
            .sunlit
            .annual_maximum(Radians::new(FRAC_PI_2 - north.value()));
        // The thresholds the zonal maximum reaches take the closed form, in one call that shares
        // its depth integral among them (P14.T51.i; R09.T7.k): each `fraction`'s, to the bit.
        let mut open = Vec::with_capacity(thresholds.len());
        for (k, (fraction, &threshold)) in fractions.iter_mut().zip(thresholds).enumerate() {
            *fraction = 0.0;
            if maximum >= threshold {
                open.push(k);
            }
        }
        if open.is_empty() {
            return;
        }
        let gated: Vec<Kelvin> = open.iter().map(|&k| thresholds[k]).collect();
        let mut shares = vec![0.0; gated.len()];
        cold_trap::fractions(north, &self.shadows, &self.terrain, &gated, &mut shares);
        for (&k, share) in open.iter().zip(shares) {
            fractions[k] = share;
        }
    }

    /// The cold-trap fraction `f_ct` of a species of threshold `threshold`: the latitude integral
    /// ([`over_latitude`]) of its [`fractions_at`](Self::fractions_at), a share of the whole
    /// surface, 0 to 1.
    ///
    /// # Panics
    ///
    /// If the threshold is not finite and positive.
    #[cfg(test)]
    #[must_use]
    fn trap_fraction(&self, threshold: Kelvin) -> f64 {
        over_latitude(|latitude| {
            let mut fraction = [0.0];
            self.fractions_at(latitude, &[threshold], &mut fraction);
            fraction[0]
        })
    }
}

/// ∫ f(φ) cos φ dφ from the equator to the pole, φ in radians: the share of a hemisphere's area, and
/// of the whole surface for a profile symmetric about the equator. Four-point Gauss–Legendre rules
/// on each band of [`TRAP_BAND_EDGES_DEG`], the bands and their nodes summed in their fixed order.
/// The field sums several at once by its rule ([`over_latitude_each`]); this one-profile form is
/// the tests' reference.
#[cfg(test)]
#[must_use]
pub(crate) fn over_latitude(f: impl Fn(Radians) -> f64) -> f64 {
    let mut sum = 0.0;
    for band in TRAP_BAND_EDGES_DEG.windows(2) {
        let low = Radians::from(Degrees::new(band[0])).value();
        let high = Radians::from(Degrees::new(band[1])).value();
        let (half, middle) = (0.5 * (high - low), f64::midpoint(low, high));
        for (&x, &w) in GL4_NODES.iter().zip(&GL4_WEIGHTS) {
            let latitude = middle + half * x;
            sum += half * w * math::cos(latitude) * f(Radians::new(latitude));
        }
    }
    sum
}

/// The tests' `over_latitude` of several profiles at once (P14.T51.i): `f` writes each profile's
/// value at a
/// latitude into its slot of the slice it is given, which is as long as `sums`, and each profile's
/// sum runs in `over_latitude`'s order with its operations, so that each equals its own
/// `over_latitude` to the bit. `sums` is overwritten.
pub(crate) fn over_latitude_each(sums: &mut [f64], mut f: impl FnMut(Radians, &mut [f64])) {
    let mut values = vec![0.0; sums.len()];
    sums.fill(0.0);
    for band in TRAP_BAND_EDGES_DEG.windows(2) {
        let low = Radians::from(Degrees::new(band[0])).value();
        let high = Radians::from(Degrees::new(band[1])).value();
        let (half, middle) = (0.5 * (high - low), f64::midpoint(low, high));
        for (&x, &w) in GL4_NODES.iter().zip(&GL4_WEIGHTS) {
            let latitude = middle + half * x;
            f(Radians::new(latitude), &mut values);
            let weight = half * w * math::cos(latitude);
            for (sum, &value) in sums.iter_mut().zip(&values) {
                *sum += weight * value;
            }
        }
    }
}

/// The terrain of a body of crater contract `craters` ([the module](self)'s stand-ins): a crater
/// fraction of [`SATURATED_CRATER_FRACTION`] × min(1, N(>[`CRATER_REFERENCE_DIAMETER`]) ÷ its
/// saturation), Hayne et al.'s distribution B, plains at atan [`PLAINS_RMS_SLOPE`] and the lunar
/// share under the conduction cut-off.
///
/// # Panics
///
/// Never: every part is in its range.
#[must_use]
pub(crate) fn terrain_of(craters: &CraterParams) -> Terrain {
    let d = CRATER_REFERENCE_DIAMETER;
    let filled = cumulative_density(craters, d).value() / saturation(d).value();
    let share = if filled.is_nan() || filled <= 0.0 {
        0.0
    } else if filled < 1.0 {
        filled
    } else {
        1.0
    };
    Terrain::new(TerrainParts {
        crater_fraction: SATURATED_CRATER_FRACTION * share,
        crater_depths: DepthRatios::HAYNE_B,
        plains_slope: Radians::new(math::atan(PLAINS_RMS_SLOPE)),
        share_under_cutoff: LUNAR_SHARE_UNDER_CUTOFF,
    })
    .expect("a crater fraction in [0, 0.2], a slope of 5.7° and a share of 0.05 are in range")
}

/// The condensables a body of partition `partition` is tested for cold traps of, in registry
/// order: its inventory's species whose row has a phase diagram, and [`DELIVERED`].
#[must_use]
pub(crate) fn condensables(partition: &Partition) -> Vec<SubstanceId> {
    let mut species: Vec<SubstanceId> = partition
        .species()
        .iter()
        .map(Partitioned::substance)
        .filter(|s| s.substance().phase_diagram().is_some())
        .collect();
    for gas in DELIVERED {
        let id = gas.substance();
        if !species.contains(&id) {
            species.push(id);
        }
    }
    species.sort_unstable();
    species
}

/// The cold-trap threshold of `substance` (the module's "The threshold"), K: its sublimation edge
/// into vacuum at [`AIRLESS_ICE_LOSS_KG_PER_M2_GYR`] ([`exosphere_threshold`]), within 1 K of
/// 110 K for water and 58.18 K for carbon dioxide. `None` for a row with no phase diagram or molar
/// mass, which takes no cold traps. P14.T51.f's traps, R09.T13.f's placement and R09.T13.d's
/// airless frost points all read it, so the three are one rule.
#[must_use]
pub(crate) fn trap_threshold(substance: SubstanceId) -> Option<Kelvin> {
    exosphere_threshold(substance, AIRLESS_ICE_LOSS_KG_PER_M2_GYR)
}

/// The cold-trapped ices of a qualifying body `body` of partition `partition`, whose zonal entries
/// are `zonal`, over its field `field`: one for each condensable ([`condensables`]) whose `f_ct` in
/// the field is above 0, its area `f_ct` × `f_x` and its reservoir [`trap_reservoir`]'s. Water takes
/// none on an ice-rich crust.
#[must_use]
fn trapped_ices(
    partition: &Partition,
    zonal: &Condensates,
    body: &ColdTrapBody<'_>,
    field: &ColdTrapField,
) -> Vec<TrappedIce> {
    let water = Gas::Water.substance();
    let mut traps = Vec::new();
    for substance in condensables(partition) {
        if substance == water && body.crust == Crust::IceRich {
            continue;
        }
        let Some((threshold, trapped)) = field.trap(substance) else {
            debug_assert!(
                trap_threshold(substance).is_none(),
                "the field holds every condensable with a threshold"
            );
            continue;
        };
        if trapped.is_nan() || trapped <= 0.0 {
            continue;
        }
        traps.push(TrappedIce {
            substance,
            reservoir: trap_reservoir(substance, trapped, partition, zonal),
            area_fraction: trapped * body.sky.exposed_fraction,
            threshold,
        });
    }
    traps
}

/// The reservoir, over the whole surface, of the cold traps of `substance`, which cover
/// `trapped` of the surface, on a body of partition `partition` and zonal entries `zonal` ([the
/// module](self)'s "The reservoir"): a species the inventory holds takes what is left of its
/// condensed reservoir after its zonal deposit, at most [`WATER_TRAP_COLUMN`] over its cold-trap
/// area; a delivered one its placeholder column over it ([`delivered_column`]).
#[must_use]
pub(crate) fn trap_reservoir(
    substance: SubstanceId,
    trapped: f64,
    partition: &Partition,
    zonal: &Condensates,
) -> KilogramsPerSquareMetre {
    let most = WATER_TRAP_COLUMN.value() * trapped;
    if partition.of(substance).is_none() {
        return delivered_column(substance) * trapped;
    }
    let held = partition
        .condensate(substance)
        .map_or(0.0, |c| c.reservoir().value());
    let laid = zonal
        .entry(substance, CondensatePhase::Solid)
        .map_or(0.0, |e| e.reservoir().value());
    let left = held - laid;
    KilogramsPerSquareMetre::new(if left.is_nan() || left <= 0.0 {
        0.0
    } else if left < most {
        left
    } else {
        most
    })
}

/// The placeholder column over its cold-trap area of a delivered species with no inventory:
/// [`WATER_TRAP_COLUMN`] for water, [`LCROSS_CO2_PER_WATER`] of it by number for carbon dioxide
/// (× 44.009 ÷ 18.015, the registry's molar masses), and none for any other.
#[must_use]
pub(crate) fn delivered_column(substance: SubstanceId) -> KilogramsPerSquareMetre {
    let water = WATER_TRAP_COLUMN.value();
    if substance == Gas::Water.substance() {
        WATER_TRAP_COLUMN
    } else if substance == Gas::CarbonDioxide.substance() {
        let by_mass = Gas::CarbonDioxide.molar_mass_g_per_mol() / Gas::Water.molar_mass_g_per_mol();
        KilogramsPerSquareMetre::new(water * LCROSS_CO2_PER_WATER * by_mass)
    } else {
        KilogramsPerSquareMetre::ZERO
    }
}
