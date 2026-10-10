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
use crate::units::{
    Degrees, EarthMasses, Gigayears, JupiterMasses, JupiterRadii, Kelvin, Megayears, Metres,
    Seconds,
};

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
/// Icarus 122, 166), which the `Rocky` and `Icy` classes take
/// ([`tides`](crate::planetary::derive::rotation::tides), P14.T14.d), and until the 21 → 22
/// batch's bump `SubNeptune` too (the [`rotation`](crate::planetary::derive::rotation) module's
/// documentation).
///
/// Earth's and Venus's measured k₂ are 0.30, Mars's 0.17 and Mercury's 0.45 (Lainey 2016,
/// Celestial Mechanics and Dynamical Astronomy 126, 145, Table 1).
pub const ROCKY_LOVE_NUMBER: f64 = 0.3;

/// The tidal quality factor Q of a rocky body: 100 (P14.T14.b, after Gladman et al. 1996), which
/// the `Rocky` and `Icy` classes take ([`tides`](crate::planetary::derive::rotation::tides)), and
/// until the 21 → 22 batch's bump `SubNeptune` too.
pub const ROCKY_TIDAL_Q: f64 = 100.0;

/// The tidal Love number k₂ of a gas giant: 0.4 (P14.T14.b, after Gladman et al. 1996), which the
/// `GasGiant` class takes ([`tides`](crate::planetary::derive::rotation::tides), P14.T14.d), and
/// until the 21 → 22 batch's bump `IceGiant` too.
pub const GIANT_LOVE_NUMBER: f64 = 0.4;

/// The tidal quality factor Q of a gas giant: 10⁵ (P14.T14.b, after Gladman et al. 1996), which
/// the `GasGiant` class takes ([`tides`](crate::planetary::derive::rotation::tides)), and until the
/// 21 → 22 batch's bump `IceGiant` too.
pub const GIANT_TIDAL_Q: f64 = 1e5;

/// The fluid Love number of the core beneath a hydrogen and helium envelope: 0.9 (P14.T14.d), the
/// first term's coefficient in
/// [`love_number_under_envelope`](crate::planetary::derive::rotation::love_number_under_envelope).
///
/// This plan's fit to decision-backlog-1's integration, not a source's. A core of iron and
/// silicate integrates to a fluid k₂ of 0.80–0.96 over 2–20 M⊕ there (Clairaut's equation in
/// Radau's form over Seager et al.'s 2007 equations of state). Earth's own 3J₂ ÷ q is 0.94, with
/// q = ω²a³ ÷ GM = 3.46 × 10⁻³ from the J₂, ω, a and GM of the IERS Conventions 2010 (Table 1.1),
/// and PREM's fluid k₂ is 0.933 (Padovan et al. 2018, A&A 620, A178, Table 1).
pub const ENVELOPED_CORE_LOVE_NUMBER: f64 = 0.9;

/// How much a water layer softens the fluid tide of the core beneath an envelope: 0.6, so that a
/// core half water takes 0.7 of a dry core's k₂ (P14.T14.d).
///
/// This plan's fit to decision-backlog-1's integration, not a source's. That integration's
/// water-rich cores had rock and iron 70.5% iron, against the generator's Earth-like 32.5%; on
/// the generator's composition an independent integration gives a bare core 54% water 0.76 of a
/// dry core's k₂, and 0.66 on the integration's (plan 14's Risks, "A sub-Neptune's tides, as
/// built").
pub const ENVELOPED_CORE_WATER_SOFTENING: f64 = 0.6;

/// The coefficient of a hydrogen and helium envelope's own tidal response, 0.125 f^0.68 in
/// [`love_number_under_envelope`](crate::planetary::derive::rotation::love_number_under_envelope)
/// (P14.T14.d).
///
/// This plan's fit to decision-backlog-1's integration, not a source's. The envelope's response
/// spans a factor of up to 3 between n = 1 and n = 2 polytropes, which the fit splits.
pub const ENVELOPE_LOVE_COEFFICIENT: f64 = 0.125;

/// The exponent of the envelope fraction in a hydrogen and helium envelope's own tidal response:
/// 0.68 ([`ENVELOPE_LOVE_COEFFICIENT`], P14.T14.d).
///
/// This plan's fit to decision-backlog-1's integration, not a source's.
pub const ENVELOPE_LOVE_EXPONENT: f64 = 0.68;

/// The tidal quality factor Q of a body under a hydrogen and helium envelope, the `SubNeptune` and
/// `IceGiant` classes: 10⁴ (P14.T14.d, decision-backlog-1).
///
/// Calibrated on the ice giants' modified quality factor Q′ = 3Q ÷ 2k₂, which it reproduces with
/// [`love_number_under_envelope`](crate::planetary::derive::rotation::love_number_under_envelope):
/// 1.6 × 10⁵ ≲ Q′ ≲ 5.6 × 10⁵ for Uranus, "most probably towards the lower end" (Tittemore and
/// Wisdom 1990, Icarus 85, 394, as Ogilvie 2014, ARA&A 52, 171, §5.4, quotes them), and the
/// bounds Q′ ≳ 9.1 × 10⁴ for Uranus and Q′ ≳ 6.7 × 10⁴ for Neptune that Ariel and Proteus set
/// (Ogilvie 2014, §5.4). The generated Uranus takes Q′ = 1.9 × 10⁵, and Neptune 1.5 × 10⁵, inside
/// the 1.1–4.3 × 10⁵ of 9,000 < Q < 36,000 (Zhang and Hamilton 2008, Icarus 193, 267, as Louden,
/// Laughlin and Millholland 2023, `ApJL` 958, L21, quote it), converted at Gavrilov and Zharkov's
/// (1977) k₂ = 0.127 (this plan's conversion). Hot Neptunes and
/// sub-Neptunes dissipate as weakly: GJ 436b's eccentricity needs Q′ > 10⁵ (Veyette and Muirhead
/// 2018, ApJ 863, 166), and 10⁴ is the Q Louden, Laughlin and Millholland take for sub-Neptunes,
/// the top of the 10³–10⁴ they expect for 1–4 R⊕.
///
/// It stands for the dissipation averaged over the satellites' and the spin's evolution. Uranus's
/// present-day Q from astrometry is 678 ± 231 at an assumed k₂ of 0.300 (Jacobson and Park 2025,
/// AJ 169, 65, §3.3), a Q′ of about 3.4 × 10³, which held over the system's age would contradict
/// Ariel's bound (plan 14's Risks, "A sub-Neptune's tides, as built").
pub const ENVELOPED_TIDAL_Q: f64 = 1e4;
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

/// The probability that a giant colder than 170 K at its cloud tops has a massive icy ring system
/// like Saturn's: 0.15 (P14.T20).
///
/// A parameter of the generator version, as plan 14 has it, because ring lifetimes are disputed:
/// Saturn's rings may be young, 10–100 Myr (Iess et al. 2019, Science 364, eaat2965; Kempf et al.
/// 2023, Science Advances 9, eadf8537), or as old as the Solar System (Crida et al. 2019, Nature
/// Astronomy 3, 967), and so a snapshot of the giants could hold them far more or less often. One
/// of the Solar System's four giants has one.
pub const MASSIVE_ICY_RING_PROBABILITY: f64 = 0.15;

/// The probability that a giant at 170 K or warmer at its cloud tops has a massive rocky ring:
/// 0.03 (P14.T20).
///
/// A parameter of the generator version, as [`MASSIVE_ICY_RING_PROBABILITY`] is. No such ring is
/// known: the transit of J1407 (Mamajek et al. 2012, AJ 143, 72) is the one candidate ring system
/// seen outside the Solar System, and it lies about a companion of unknown nature.
pub const MASSIVE_ROCKY_RING_PROBABILITY: f64 = 0.03;

/// The probability that a Kuiper-like belt is bright, keeping a sizeable share of its solids:
/// 0.60 (P14.T21.b; ruling 84.1's starting value 0.25, fitted on the slow test's sample of
/// [`belts`](crate::planetary::belts) to the detected share after ruling 84's amendment).
///
/// Most bright belts lie too close in, just beyond compact systems' outermost planets, or too
/// faint for a 100 µm survey to see, so the share of hosts with a detected belt is 0.173.
///
/// Cold belts are bimodal: Sibthorpe et al. (2018, MNRAS 475, 3046, §5.3) find that about half of
/// Sun-like stars either depleted their Kuiper belts or formed fewer planetesimals, the Kuiper
/// belt itself having lost "nearly three orders of magnitude" of its mass.
pub const BRIGHT_KUIPER_BELT_PROBABILITY: f64 = 0.60;

/// The median share of its solids a bright Kuiper-like belt keeps, as a log: −0.25 dex (ruling
/// 84.1's starting value −1, fitted on the slow test's sample).
pub const BRIGHT_KUIPER_EFFICIENCY_DEX: f64 = -0.25;

/// The scatter of a bright Kuiper-like belt's share, dex: 0.5 (ruling 84.1's 0.5–0.8, fitted on
/// the slow test's sample).
pub const BRIGHT_KUIPER_EFFICIENCY_SIGMA_DEX: f64 = 0.5;

/// The share of its solids a faint Kuiper-like belt keeps: 10⁻³, the Kuiper belt's loss of
/// "nearly three orders of magnitude" (Sibthorpe et al. 2018, §5.3; ruling 84.1).
pub const FAINT_KUIPER_EFFICIENCY: f64 = 1e-3;

/// The median primordial rotation period of a rocky body: 15 hours (plan 14, P14.T14.a).
///
/// The plan's figure. Earth's day before the Moon-forming impact is unknown, and Mars's 24.6 hours
/// and the few-hour spins of the giant-impact era (Kokubo and Genda 2010, `ApJL` 714, L21) bracket
/// it; it is the median of [`primordial_period`](crate::planetary::derive::rotation::primordial_period)'s
/// log-normal law.
pub const ROCKY_PRIMORDIAL_PERIOD: Seconds = Seconds::new(54_000.0);

/// The median primordial rotation period of a giant: 10 hours (P14.T14.a), about Jupiter's 9.93
/// and Saturn's 10.6 hours, which tides have barely touched.
pub const GIANT_PRIMORDIAL_PERIOD: Seconds = Seconds::new(36_000.0);

/// The scatter of the primordial rotation period about its median, dex: 0.2, a factor of 1.6
/// (P14.T14.a; provisional).
///
/// The plan gives the law's median and not its width. Planetary-mass companions' spins, which
/// tides have not touched either, spread over about a factor of two about a tenth of break-up
/// (Bryan et al. 2020, AJ 159, 181, not re-read), which 0.2 dex gives within ±1.5σ.
pub const PRIMORDIAL_PERIOD_SCATTER_DEX: f64 = 0.2;

/// The Rayleigh scale of the obliquity of a body that had no giant impact: 10° (P14.T14.a).
pub const QUIET_OBLIQUITY_SCALE: Degrees = Degrees::new(10.0);

/// The eccentricity above which a tidally locked body settles into the 3:2 spin–orbit resonance
/// rather than into synchronous rotation: 0.1 (P14.T14.b, the plan's "about 0.1").
///
/// Mercury, at 0.206, is in 3:2; capture into it needs an eccentricity above about 0.1 for most
/// histories, and synchronous rotation is the stable end state below (Correia and Laskar 2004,
/// Nature 429, 848, not re-read).
pub const SPIN_ORBIT_RESONANCE_ECCENTRICITY: f64 = 0.1;

/// The moment of inertia of a rocky body in units of M R²: 0.33 (P14.T14.b), Earth's 0.3307
/// (Williams 1994, AJ 108, 711), with Mercury's 0.346 and Mars's 0.3645 above it.
pub const ROCKY_MOMENT_OF_INERTIA: f64 = 0.33;

/// The moment of inertia of an icy body in units of M R²: 0.34 (P14.T14.b), between Ganymede's
/// 0.311 and Callisto's 0.355, and Titan's 0.34 (Iess et al. 2010, Science 327, 1367).
pub const ICY_MOMENT_OF_INERTIA: f64 = 0.34;

/// The moment of inertia of a body under a hydrogen and helium envelope, sub-Neptunes and ice
/// giants, in units of M R²: 0.23 (P14.T14.b), Uranus's 0.22 and Neptune's 0.24 (Helled, Anderson
/// and Schubert 2010, Icarus 210, 446, not re-read). Below the plan's 0.33–0.4, which no body with
/// an envelope reaches.
pub const ENVELOPED_MOMENT_OF_INERTIA: f64 = 0.23;

/// The moment of inertia of a gas giant in units of M R²: 0.25 (P14.T14.b), Jupiter's 0.254
/// (Hubbard and Marley 1989, Icarus 78, 102; 0.2756 from Juno's field, Ni 2018, not re-read) with
/// Saturn's 0.21 below. Below the plan's 0.33–0.4, as for [`ENVELOPED_MOMENT_OF_INERTIA`]. A
/// Jupiter-like giant's: at or below [`JUPITER_HEAVY_ELEMENT_FRACTION`] (P14.T46.a).
pub const GAS_GIANT_MOMENT_OF_INERTIA: f64 = 0.25;

/// The moment of inertia of a Saturn-like gas giant in units of M R²: 0.21, Saturn's 0.210 (NASA's
/// Saturn fact sheet, C ÷ M a²; a more centrally condensed giant has the smaller factor, Hubbard
/// and Marley 1989, Icarus 78, 102). A gas giant's factor falls linearly from
/// [`GAS_GIANT_MOMENT_OF_INERTIA`] at [`JUPITER_HEAVY_ELEMENT_FRACTION`] to this at
/// [`SATURN_HEAVY_ELEMENT_FRACTION`] and holds beyond (P14.T46.a, decision-p14-phase-j 1).
pub const SATURN_LIKE_MOMENT_OF_INERTIA: f64 = 0.21;

/// A giant's heavy-element mass fraction Z = 1 − envelope at 1 Jupiter mass: 0.182 17, Thorngren
/// et al.'s (2016) 57.9 M⊕ ÷ 317.83 M⊕ (`composition::giant_heavy_elements`, which a test
/// reproduces to 10⁻¹²).
pub const JUPITER_HEAVY_ELEMENT_FRACTION: f64 = 0.182_173_773_012_805_7;

/// A giant's heavy-element mass fraction Z at Saturn's 95.16 M⊕ by the same law: 0.291 57,
/// 57.9 M⊕ × (95.16 ÷ 317.83)^0.61 ÷ 95.16 M⊕ (a test reproduces it to 10⁻¹²).
pub const SATURN_HEAVY_ELEMENT_FRACTION: f64 = 0.291_570_634_037_780_8;

/// The mean radius below which an icy body is not hydrostatic and holds a sphere's figure: 200 km
/// (P14.T46.c; Lineweaver and Norman 2010, arXiv:1004.1091, the "potato radius" of about 200 km
/// for icy moons; Mimas, 198 km, is about round).
pub const HYDROSTATIC_RADIUS_ICE: Metres = Metres::new(2.0e5);

/// The mean radius below which a rocky body holds a sphere's figure: 300 km (P14.T46.c;
/// Lineweaver and Norman 2010, about 300 km for asteroids; Vesta, 263 km, is not round).
pub const HYDROSTATIC_RADIUS_ROCK: Metres = Metres::new(3.0e5);

/// The factor by which a synchronous body's tide flattens its best spheroid beyond its spin's:
/// 2.5 (P14.T46.c). In first-order hydrostatic theory the tidal potential on a synchronous body is
/// three times the rotational one, giving axes in the ratio (a − c) : (b − c) : (a − b) =
/// 4 : 1 : 3 (Dermott 1979, Icarus 37, 575; Murray and Dermott 1999, §4.7), whose (a + b) ÷ 2
/// against c is 2.5 times the spin's a − c. Io's 0.378 and 42.46 h give 9.0 km against 8.7 km
/// measured (decision-p14-phase-j, 3).
pub const SYNCHRONOUS_TIDAL_FACTOR: f64 = 2.5;

/// The largest flattening a figure takes: 0.2 (P14.T46.c, as R07 asks), beyond which first-order
/// theory fails.
pub const FLATTENING_CAP: f64 = 0.2;
