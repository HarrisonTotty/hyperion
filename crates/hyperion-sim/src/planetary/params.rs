//! The parameters of the planetary stage that belong to the generator version, as named constants
//! in one place (plan 14, "Generator version").
//!
//! Changing any of these moves generated bodies and needs a generator-version bump. The class
//! weight table and its constants are [`architecture`](super::architecture)'s, whose module
//! documentation is their single written definition (P14.T4). The plan gathers here the spacing
//! floors, the ring probabilities, the pulsar-planet probability, [`SATELLITE_STABILITY_FRACTION`]
//! and the white dwarf pollution fit, each added by the task that first uses it. The disc's own figures, which are measurements with
//! their sources beside the physics that uses them, live in [`disc`](super::disc).

use crate::planetary::derive::irradiation::BondAlbedo;
use crate::planetary::disc::{ROCK_MASS_FRACTION, WATER_ICE_MASS_FRACTION};
use crate::units::{EarthMasses, Gigayears, JupiterMasses, JupiterRadii, Kelvin, Megayears};

/// How far out a prograde satellite on a circular orbit about a planet on a circular orbit stays
/// bound: 0.4895 of the planet's Hill radius (design note 14).
///
/// Domingos, Winter and Yokoyama (2006, MNRAS 373, 1227, abstract), whose fit of the critical
/// semi-major axis is 0.4895 (1 − 1.0305 eₚ − 0.2738 eₛ) `R_H`; Rosario-Franco et al. (2020, AJ
/// 159, 260, Table 1) tabulate it as 0.4895 ± 0.0363, and Namouni (2010, ApJ 719, L145) quotes
/// 0.48. Rosario-Franco et al. find 0.40 when all twenty starting phases must survive 10⁵ years
/// (their §3.1.1). The same fraction cuts every system at 0.49 of its sphere of influence and
/// bounds moons inside Hill spheres.
pub const SATELLITE_STABILITY_FRACTION: f64 = 0.4895;

/// The floor on the spacing of two small planets on circular orbits: 10 mutual Hill radii
/// (design note 7).
///
/// Pu and Wu (2015, ApJ 807, 44), abstract and eq. 12: systems of Kepler-like planets survive a
/// thousand million years only if spaced by about 10 mutual Hill radii when circular and coplanar.
pub const SPACING_FLOOR_SMALL_CIRCULAR: f64 = 10.0;

/// How the small planets' floor rises with their mean eccentricity: 80 mutual Hill radii per unit
/// of mean eccentricity (design note 7; ruling 38).
///
/// Pu and Wu (2015, eq. 14): the threshold rises by one mutual Hill radius for each 0.01 of σₑ,
/// the Rayleigh scale of the eccentricities, 100 per unit σₑ. The mean of a Rayleigh distribution
/// is √(π ÷ 2) σₑ = 1.25 σₑ, so per unit of mean eccentricity the slope is 100 ÷ 1.25 = 80, and
/// the floor reaches 12 at a mean eccentricity of 0.025, their σₑ of 0.02.
pub const SPACING_FLOOR_ECCENTRICITY_SLOPE: f64 = 80.0;

/// The small planets' floor at its highest: 12 mutual Hill radii (design note 7; Pu and Wu 2015,
/// abstract, "∼12 if planetary orbits have eccentricities ∼0.02").
pub const SPACING_FLOOR_SMALL_ECCENTRIC: f64 = 12.0;

/// The floor on the spacing of a pair that includes a giant: 7 mutual Hill radii (design note 7).
///
/// Plan 14's figure, after Chambers, Wetherill and Boss (1996, Icarus 119, 261), Marzari and
/// Weidenschilling (2002, Icarus 156, 570) and Chatterjee et al. (2008, ApJ 686, 580), and the
/// least certain number of the floor. Two giants are Hill stable beyond 2√3 ≈ 3.5 (Gladman 1993);
/// Chatterjee et al.'s three-giant systems, spaced in the same mutual Hill radius (their eq. B2),
/// are mostly stable for 10⁹ years at 5.5 (their Fig. 29), so 7 is conservative. Jupiter and
/// Saturn sit 7.9 apart.
pub const SPACING_FLOOR_GIANT: f64 = 7.0;

/// The mass from which a planet counts as a giant for the spacing floor: 0.1 Jupiter masses,
/// about 32 M⊕ (design note 7).
pub const SPACING_GIANT_MASS: JupiterMasses = JupiterMasses::new(0.1);

/// The least gap between an inner planet's apocentre and its outer neighbour's pericentre, in
/// mutual Hill radii: 2√3 ≈ 3.46 (design note 7).
///
/// Gladman (1993, Icarus 106, 247): two planets on circular orbits separated by more than 2√3
/// mutual Hill radii can never meet. Applied at closest approach it makes "no overlapping orbits"
/// a theorem of the generator.
pub const HILL_STABLE_GAP: f64 = 3.464_101_615_137_754_6;

/// The lightest core that holds a hydrogen and helium envelope in the composition solve: 1.5 M⊕
/// (P14.T11.c).
///
/// A body lighter than this whose radius lies above its dry curve is clamped to that curve
/// instead of given an envelope, and a heavier one is never given so much envelope that its core
/// falls below it. Plan 14 states the floor for bodies formed inside the snow line; the solve
/// applies it beyond the snow line as well, to icy cores, where the plan is silent.
pub const ENVELOPE_CORE_FLOOR: EarthMasses = EarthMasses::new(1.5);

/// The most water a body formed inside the snow line may hold: 0.1% of its mass (P14.T11.c).
///
/// A small body whose radius lies above the rock curve is clamped to rock with this much water at
/// most, so that nothing formed inside the snow line comes out as a water world (design note 8).
pub const INNER_WATER_CAP: f64 = 0.001;

/// The most water a body formed beyond the snow line may hold: the share of water ice in the
/// disc's solids there, 0.571 ÷ (0.489 + 0.571) = 53.9% (P14.T11.c).
///
/// This is Lodders's (2003, Table 11) ice-to-rock ratio of the disc of [`disc`](super::disc), so a
/// body cannot hold more water than the solids it grew from. It lies inside the 1/2 (rock to water
/// ice) to 2/3 (rock to all ices) that Zeng et al. (2019, Materials and Methods) take for icy
/// cores. A larger radius takes an envelope over a core of this composition.
pub const OUTER_WATER_CAP: f64 =
    WATER_ICE_MASS_FRACTION / (ROCK_MASS_FRACTION + WATER_ICE_MASS_FRACTION);

/// The age at which the composition solve reads a body's radius: 5 Gyr (P14.T11.c).
///
/// Chen and Kipping's (2017) relation describes the planets observed today, mostly about stars of
/// several Gyr, so the drawn radius is taken as the body's radius at this age, the representative
/// age of Lopez and Fortney's (2014) models, and the envelope fraction solved there is the body's
/// primordial one. The envelope's radius at other ages follows from its thermal evolution.
pub const COMPOSITION_REFERENCE_AGE: Gigayears = Gigayears::new(5.0);

/// The Bond albedo of every body until the atmosphere loop of P14.T13 closes: 0.3 (P14.T12.a).
pub const BOND_ALBEDO_BEFORE_ATMOSPHERES: BondAlbedo = BondAlbedo::from_fraction(0.3);

/// The envelope below which a body is classed by its core: 0.1% of its mass (P14.T16.a).
///
/// Venus's atmosphere is 10⁻⁴ of its mass, so an envelope this thin is an atmosphere over a
/// surface rather than the deep hydrogen of a sub-Neptune, whose envelopes Lopez and Fortney (2014)
/// model from 0.1% up. This plan's classification, not a source's.
pub const THIN_ENVELOPE_FRACTION: f64 = 0.001;

/// The water fraction from which a body without an envelope is icy rather than rocky: 10% of its
/// mass (P14.T16.a).
///
/// Europa, about 8% water, is then rocky, and Ganymede, Callisto and Titan, near half water, icy.
/// This plan's classification, not a source's.
pub const ICY_WATER_FRACTION: f64 = 0.1;

/// The mass from which a body with an envelope, not dominated by it, is an ice giant rather than a
/// sub-Neptune: 10 M⊕ (P14.T16.a).
///
/// The critical core mass beyond which a core accretes gas in runaway (Mizuno 1980, Progress of
/// Theoretical Physics 64, 544; Pollack et al. 1996, Icarus 124, 62), which the observed
/// sub-Neptunes of 2–4 R⊕ mostly lie below and Uranus and Neptune (14.5 and 17.1 M⊕) above.
pub const ICE_GIANT_MASS: EarthMasses = EarthMasses::new(10.0);

/// The envelope fraction from which a body is a gas giant: half its mass (P14.T16.a).
///
/// Saturn's envelope is about 74% of its mass (its 25 M⊕ of heavy elements, Saumon and Guillot
/// 2004, as Fortney, Marley and Barnes 2007, §6.1, quote) and Uranus's and Neptune's 10–20% in
/// interior models (the solve gives 71%, 8% and 6%), so any fraction between separates them; half
/// is where hydrogen and helium dominate.
pub const GAS_GIANT_ENVELOPE_FRACTION: f64 = 0.5;

/// The tidal Love number k₂ of a rocky body: 0.3 (plan 14, P14.T14.b, after Gladman et al. 1996,
/// Icarus 122, 166).
pub const ROCKY_LOVE_NUMBER: f64 = 0.3;

/// The tidal quality factor Q of a rocky body: 100 (P14.T14.b, after Gladman et al. 1996).
pub const ROCKY_TIDAL_Q: f64 = 100.0;

/// The tidal Love number k₂ of a giant: 0.4 (P14.T14.b, after Gladman et al. 1996).
pub const GIANT_LOVE_NUMBER: f64 = 0.4;

/// The tidal quality factor Q of a giant: 10⁵ (P14.T14.b, after Gladman et al. 1996).
pub const GIANT_TIDAL_Q: f64 = 1e5;
/// The largest radius a giant planet takes: 2 Jupiter radii (P14.T11.d).
///
/// Plan 14's cap on the inflated radius of
/// [`radius_giant`](crate::planetary::derive::radius::radius_giant). The largest hot Jupiters come
/// close to it (Thorngren and Fortney 2018, §1: radii "sometimes approaching 2 Jupiter radii"),
/// and Thorngren and Fortney's models pass it for strongly heated planets below about 0.6 Jupiter
/// masses, a population that is not observed (their §2).
pub const GIANT_RADIUS_CAP: JupiterRadii = JupiterRadii::new(2.0);

/// The equilibrium temperature from which a giant is held at Thorngren and Fortney's (2018) model
/// radius in full: 1,000 K (P14.T11.d).
///
/// Plan 14's "equilibrium temperatures over 1,000 K", which is Thorngren and Fortney's threshold
/// of inflation, 0.2 × 10⁹ erg s⁻¹ cm⁻² (their §1, after Miller and Fortney 2011). Below it the
/// model radius fades out to [`GIANT_INFLATION_FADE_START`]
/// ([`radius_giant`](crate::planetary::derive::radius::radius_giant)).
pub const GIANT_INFLATION_ONSET: Kelvin = Kelvin::new(1_000.0);

/// The equilibrium temperature below which no giant is held above its cooling radius: 500 K, the
/// coolest line of Thorngren and Fortney's (2018) Fig. 2 (P14.T11.d).
pub const GIANT_INFLATION_FADE_START: Kelvin = Kelvin::new(500.0);

/// The earliest host age at which a giant planet forms: 0.5 Myr (design note 12, P14.T28.a).
///
/// A giant's formation age is uniform in log between this and its disc's lifetime. It is the end
/// of the protostellar phase, classes 0 and I, the first 0.5 Myr of plan 06's tracks
/// ([`Phase::Protostar`](crate::stellar::Phase::Protostar)), while the disc is still being fed and
/// the star is still gaining mass. The plan gives the figure without a source.
pub const EARLIEST_GIANT_FORMATION: Megayears = Megayears::new(0.5);

/// The earliest host age at which a terrestrial planet's magma ocean ends: 10 Myr (design note 12,
/// P14.T28.a).
///
/// The end is drawn uniform in log between this and [`MAGMA_OCEAN_END_LATEST`]. The plan gives the
/// range without a source; it spans the giant-impact phase that ends terrestrial accretion, whose
/// last impact on Earth, the Moon's, came some 30–100 Myr after the Solar System formed.
pub const MAGMA_OCEAN_END_EARLIEST: Megayears = Megayears::new(10.0);

/// The latest host age at which a terrestrial planet's magma ocean ends: 100 Myr (design note 12,
/// P14.T28.a). See [`MAGMA_OCEAN_END_EARLIEST`].
pub const MAGMA_OCEAN_END_LATEST: Megayears = Megayears::new(100.0);

/// The tidal mass `M_c` of a planet's engulfment reach f, f⁸ = 1 + `M_p` ÷ `M_c`: 3.1 M⊕ (design
/// note 11, P14.T28.b; ruling 62).
///
/// A planet is destroyed once its semi-major axis is inside f times the largest radius its host
/// has had, with f⁸ = 1 + `M_p` ÷ `M_c`
/// ([`engulfment_reach`](crate::planetary::hosts::evolved::engulfment_reach)). Zahn's (1977)
/// equilibrium tide in a convective envelope, which Mustill and Villaver (2012, ApJ 761, 121,
/// eqs. 1–5) integrate, draws a planet in at ȧ ∝ `M_p` (R★ ÷ a)⁸, so the reach beyond the
/// photosphere grows as the eighth root of the planet's mass, and a planet of negligible mass is
/// engulfed only by the photosphere itself, f = 1.
///
/// `M_c` is fitted to their Figure 7: the initial semi-major axes, at the start of the thermally
/// pulsing AGB, of the outermost circular Terrestrial (1 M⊕), Neptunian (17.1 M⊕) and Jovian
/// (318 M⊕) planets engulfed about stars of 1, 1.5, 2, 2.5, 3.5 and 5 M☉, read from the figure's
/// vector paths against each star's largest AGB radius there (1.58, 2.41, 3.00, 3.30, 4.15 and
/// 5.18 au). Each ratio is f times the share of the star's mass left at its largest radius, a
/// factor of each star alone, which the transform's adiabatic expansion supplies from the host's
/// own track. The least-squares fit of the logarithms, with one such factor per star, gives
/// `M_c` = 3.10 M⊕ with the exponent held at Zahn's ⅛, and an exponent of 0.129 when it is left
/// free; the eighteen critical axes are reproduced to 1.8% rms and 4% at worst. So f is 1.036 for
/// the Earth, 1.264 for Neptune and 1.786 for Jupiter, and does not depend on the host's mass:
/// the Jovian-to-Terrestrial ratio of their critical axes is 1.65–1.82 with no trend from 1 to
/// 5 M☉. Their eccentric planets (e = 0.2) are engulfed from 5% further out for a Jupiter, whose
/// orbit the tides circularise first, to 22% for an Earth, whose pericentre meets the envelope;
/// the transform tests the semi-major axis alone.
pub const ENGULFMENT_TIDAL_MASS: EarthMasses = EarthMasses::new(3.1);
