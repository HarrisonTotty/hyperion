//! Belts: asteroid belts at a giant's resonances or in wide gaps, a Kuiper-like belt beyond the
//! planets, their fading debris, and their largest members (plan 14, P14.T21.a–c; design notes 3,
//! 4, 5 and 12).
//!
//! # Where belts are
//!
//! One orbit host at a time, from its disc and its planets, all plain arguments ([`BeltHost`]):
//!
//! - **An asteroid belt inside a giant** (P14.T21.a). Inside the innermost giant beyond the snow
//!   line, between the giant's 4:1 and 2:1 inner resonances, 0.397–0.630 of its semi-major axis
//!   ([`ASTEROID_BAND_RESONANCES`]), provided no planet's chaotic zone reaches into that band, with
//!   gaps recorded at the 3:1, 5:2 and 7:3 resonances ([`KIRKWOOD_RESONANCES`]). Its mass is the
//!   disc's solids in the band times a depletion factor drawn log-uniform over 10⁻⁴–10⁻²
//!   ([`DEPLETION_WITH_GIANT`], ruling 84.3).
//! - **Asteroid belts in gaps** (P14.T21.a). In a host with no giant beyond the snow line, one in
//!   every gap between adjacent planets wider than [`GAP_BELT_SPACING`] (40) mutual Hill radii,
//!   between the two planets' chaotic zones, with a depletion factor over 10⁻²–1
//!   ([`DEPLETION_WITHOUT_GIANT`]).
//! - **A Kuiper-like belt** (P14.T21.b). Beyond the outermost planet, between its 3:2 and 2:1
//!   outer resonances, 1.31–1.59 of its semi-major axis ([`KUIPER_BAND_RESONANCES`]), with a
//!   scattered component from there to the disc's outer edge; about a host with no planets, the
//!   disc's outer third. Its mass is the solids of the belt proper, bright or faint (ruling 84.1,
//!   below); the scattered component has none of its own and gives its members wider orbits.
//!
//! Every belt is cut to the disc's edges, which already carry the zone's limits and the strip
//! radius of design note 14 (P14.T3.b), and to outside every planet's chaotic zone, so no belt
//! overlaps one: a planet's chaotic zone reaches 1.3 μ^(2⁄7) a beyond its pericentre and apocentre
//! (Wisdom 1980, [`CHAOTIC_ZONE_COEFFICIENT`]), μ being its mass over its host's. A belt left with
//! no width, or no solids, is not there.
//!
//! # A giant, the snow line and "without giants"
//!
//! Here a giant is a planet of at least [`BELT_GIANT_MASS`] (10 M⊕, the mass from which a body
//! with an envelope is an ice giant), so that an ice giant can sculpt an asteroid belt. The host
//! has a giant "beyond the snow line" when such a planet
//! orbits at or beyond its disc's snow line, and is "without giants" otherwise: a hot Jupiter
//! alone does not sculpt a belt at resonances beyond the snow line, and plan 14's two rules are
//! read as the two cases of one host. Both of a class template's belt rules
//! ([`BeltRule`](crate::planetary::architecture::template::BeltRule)), `Required` and `Allowed`,
//! place a belt wherever these rules find room; the `SolarLike` class's `Required` is the
//! statement that its giants always leave room.
//!
//! # Collisional wear and brightness
//!
//! A belt's planetesimals grind down in a collisional cascade, and it fades (design note 12,
//! "bright when young and fade as 1 ÷ age"). By Wyatt et al.'s model (2007a, ApJ 658, 569, and
//! 2007b, ApJ 663, 365; reviewed by Wyatt 2008, ARA&A 46, 339), for a belt at radius r of width dr
//! about a host of mass M★:
//!
//! - its mass at age t is M(t) = M₀ ÷ (1 + t ÷ `t_c`) (Wyatt et al. 2007a, eq. 14), with the
//!   collisional lifetime of its largest planetesimals `t_c` = 0.009 r^3.5 (dr ÷ r) `D_c`
//!   M★^(−½) ÷ (G(11⁄6, `X_c`) M₀) Myr (their eq. 17, `M_max` `t_age`, with the full G of eqs.
//!   17–18, ruling 84.1; r in au, `D_c` in km, M★ in M☉, M in M⊕; [`collisional_time`],
//!   [`cascade_factor`]);
//! - its fractional luminosity, what an infrared sensor sees, is f = `L_dust` ÷ L★ = σ ÷ 4πr², where
//!   a cascade of index 11⁄6 from the blowout diameter `D_bl` = 0.8 (L★ ÷ M★) (2,700 ÷ ρ) µm (their
//!   eq. 6) to `D_c` has cross-section σ = 3M ÷ 2ρ√(`D_bl` `D_c`), so f = 0.37 r⁻² `D_bl`^(−½)
//!   `D_c`^(−½) M (from their eqs. 3–4 with ρ = 2,700 kg m⁻³; [`fractional_luminosity`]).
//!
//! **Cold belts are bimodal** (ruling 84.1). A Kuiper-like belt keeps, with probability
//! [`BRIGHT_KUIPER_BELT_PROBABILITY`], a share ε of its solids with log₁₀ ε normal about
//! [`BRIGHT_KUIPER_EFFICIENCY_DEX`] with σ = [`BRIGHT_KUIPER_EFFICIENCY_SIGMA_DEX`], and otherwise
//! [`FAINT_KUIPER_EFFICIENCY`], 10⁻³, the Kuiper belt's own loss (Sibthorpe et al. 2018, MNRAS
//! 475, 3046, §5.3). Its scattered component has no mass of its own.
//!
//! **What the surveys detect** (ruling 84, amended). A belt is detected at 100 µm, as DUNES
//! (Montesinos et al. 2016, A&A 593, A51, §4) and DEBRIS (Sibthorpe et al. 2018) detect discs of
//! any temperature, when its fractional luminosity weighted by the 100 µm excess of a blackbody
//! belt at its temperature, [`detection_weight`], reaches 10⁻⁶ at 60 K. The temperature is taken
//! at the blackbody radius, the belt's radius ÷ 2.5 ([`BLACKBODY_RADIUS_SHARE`]; Sibthorpe et al.:
//! true radii are up to 2.5 times the blackbody ones). The three are fitted so that of single FGK
//! hosts of 1–10 Gyr 0.173 have a detected Kuiper-like belt (Eiroa et al. 2013, A&A 555, A11,
//! 20.2 ± 2%; Montesinos et al., 0.22 +0.08 −0.07; Sibthorpe et al., 17.1%), 0.303 one over 10⁻⁷
//! by the same weighting (at most 0.35, Montesinos et al. §5.1), and 0.011 a detected one of
//! blackbody temperature 100 K or more (at most 0.03: Sibthorpe et al. 5 of 275; Patel et al.
//! 2014, 1.8 ± 0.2%).
//!
//! The planetesimal parameters are ρ = 2,700 kg m⁻³, e = 0.05 and `D_c` = 450 km (Wyatt et al.
//! 2007a; Kains, Wyatt and Greaves 2011, MNRAS 414, 2486, whose FGK fit is `D_c` = 450 km and
//! `Q_D*` = 3,700 J kg⁻¹), with `Q_D*` = 495 J kg⁻¹ set so that `D_c`^½ `Q_D*`^(5⁄6) e^(−5⁄3) is
//! Sibthorpe et al.'s (2018, §4) best fit to the DEBRIS FGK stars, 5.5 × 10⁵ km^½ J^(5⁄6)
//! kg^(−5⁄6), the combination that sets a belt's late brightness. Kains et al.'s 2.9 × 10⁶ left
//! 0.027 of hosts with a detected warm belt ([`CASCADE_TOP_DIAMETER`], [`DISPERSAL_THRESHOLD`],
//! [`CASCADE_ECCENTRICITY`], [`CASCADE_DENSITY`]). A scattered component has no mass, so a belt
//! wears and shines as its belt proper.
//!
//! **The collisional cap.** M(t) = M₀ ÷ (1 + t ÷ `t_c`) never exceeds Wyatt et al.'s maximum mass
//! `M_max`(t) = M₀ `t_c` ÷ t, whatever the drawn ε, so f never exceeds `f_max` ∝ r^(7⁄3) ÷ t (their
//! eqs. 17–18, with the full G; [`maximum_fractional_luminosity`]; tested over the sample). With
//! these parameters `f_max` is 6.5 × 10⁻⁷ at 3 au (dr ÷ r = 0.2) about the Sun at 1 Gyr; eq. 20's
//! A0V prefactor gives 1.6 × 10⁻⁸ there.
//!
//! # Largest members
//!
//! Sizes follow N(> D) ∝ D^(−q), q drawn uniform over 2.5–3.5 per belt ([`SIZE_SLOPE`]),
//! normalised so that the bodies from [`SMALLEST_BODY`] (1 km) up hold the belt's primordial mass;
//! the k-th largest has N(> D) = k, D = `D_max` k^(−1⁄q). Those over [`MEMBER_MIN_DIAMETER`]
//! (400 km), at most [`MAX_MEMBERS`] (8), become bodies in the belt's slot with sub-indices 1
//! upward (design note 3), on orbits drawn inside the belt ([`BeltMember`]): the semi-major axis
//! as the disc's solids lie, eccentricity Rayleigh with σ = 0.1 and inclination Rayleigh with
//! σ = 8° to the host's plane, 0.3 and 20° in a scattered component, each eccentricity truncated
//! so that the orbit stays clear of the planets' chaotic zones and inside the disc, and so inside
//! the strip radius (design note 14). A member is derived by P14.T16 as
//! a dwarf planet ([`BeltMember::placed_body`]), and is eligible for P14.T18's giant-impact moon,
//! which P14.T22.a gives it: this module calls nothing of `moons`. The rest of the belt stays a
//! population: its mass, size slope, bounds, mean eccentricity and inclination, and composition
//! class by the side of the snow line its solids lie on.
//!
//! # Slots and draws
//!
//! Belts take the belt slots from [`FIRST_BELT_SLOT`] (`0xE1`) to [`LAST_BELT_SLOT`] (`0xED`),
//! host by host in the order the caller gives, and inside out within a host ([`host_belts`]
//! returns the next free slot); `0xE0` is P14.T28.a's protoplanetary disc, `0xEE` is left for
//! P14.T28.d's white dwarf debris disc, and `0xEF` is the cometary halo's
//! ([`HALO_SLOT`](crate::planetary::halo::HALO_SLOT)). A host that would need more is given what
//! the slots hold.
//!
//! On [`tags::BELT_POPULATION`], keyed by the system's ID with the slot in the draw number (design
//! note 4), the belt in belt slot n reads words 8n (an asteroid belt's depletion rank), 8n + 1 (its
//! size slope's rank), 8n + 2 (a Kuiper-like belt's bright mark) and 8n + 3 to 8n + 4 (its bright
//! efficiency's standard normal); 8n + 5 to 8n + 7 are reserved ([`BeltDraws`]). Each member draws its orbit and
//! radius rank on [`tags::BELT_MEMBER`], keyed by its own [`BodyId`](crate::id::BodyId)
//! ([`MemberDraws`]).

use core::f64::consts::{PI, TAU};

use crate::id::SystemId;
use crate::math;
use crate::orbit::{Eccentricity, KeplerElements};
use crate::planetary::derive::PlacedBody;
use crate::planetary::disc::DiscProfile;
use crate::planetary::index::{BodyIndex, BodySlot, BodySub};
use crate::planetary::params::{
    BRIGHT_KUIPER_BELT_PROBABILITY, BRIGHT_KUIPER_EFFICIENCY_DEX,
    BRIGHT_KUIPER_EFFICIENCY_SIGMA_DEX, FAINT_KUIPER_EFFICIENCY, ICE_GIANT_MASS,
};
use crate::planetary::placement::classes::CHAOTIC_ZONE_COEFFICIENT;
use crate::planetary::placement::classes::orbits::{
    SystemPlane, mutual_inclination, orientation_in_plane,
};
use crate::planetary::placement::{Neighbour, OrbitHost, mutual_hill_radius};
use crate::planetary::record::BeltKind;
use crate::rng::{Mark, ObjectKey, Seed, Stream, Threshold, tags};
use crate::stellar::draws::{StandardNormal, UnitUniform};
use crate::units::consts::{EARTH_MASS_KG, GM_EARTH, GM_SUN, METRES_PER_AU, SOLAR_MASS_KG};
use crate::units::{
    EarthMasses, GravitationalParameter, Kelvin, KilogramsPerCubicMetre, Metres, Radians,
    SolarLuminosities, SolarMasses, Years,
};

/// The first belt slot a belt takes: `Belt(1)`, slot `0xE1`, since `0xE0` is P14.T28.a's
/// protoplanetary disc.
pub const FIRST_BELT_SLOT: u8 = 1;

/// The last belt slot a belt takes: `Belt(13)`, slot `0xED`; `0xEE` is left for P14.T28.d's debris
/// disc and `0xEF` is the cometary halo's.
pub const LAST_BELT_SLOT: u8 = 13;

/// Words of the [`tags::BELT_POPULATION`] stream that one belt slot owns: the belt in slot n reads
/// words 8n onwards.
pub const BELT_WORDS_PER_SLOT: u64 = 8;

/// Words of a member's [`tags::BELT_MEMBER`] stream it reads: eight uniforms.
pub const MEMBER_WORDS: u64 = 8;

/// The inner resonances of a giant that bound an asteroid belt, as the belt body's orbits to the
/// giant's, (4, 1) and (2, 1): the 4:1 and 2:1, at
/// (1⁄4)^⅔ = 0.397 and (1⁄2)^⅔ = 0.630 of the giant's semi-major axis (P14.T21.a).
///
/// The Solar System's main belt runs from the 4:1 resonance with Jupiter at 2.065 au, where the
/// ν₆ secular resonance also cuts it, to the 2:1 at 3.279 au, the Hecuba gap.
pub const ASTEROID_BAND_RESONANCES: [(u8, u8); 2] = [(4, 1), (2, 1)];

/// The inner resonances at which an asteroid belt's gaps are recorded, as its orbits to the
/// giant's: 3:1, 5:2 and 7:3, the
/// Kirkwood gaps, at 2.502, 2.825 and 2.958 au for Jupiter (P14.T21.a).
pub const KIRKWOOD_RESONANCES: [(u8, u8); 3] = [(3, 1), (5, 2), (7, 3)];

/// The outer resonances of the outermost planet that bound a Kuiper-like belt, as the belt body's
/// orbits to the planet's, (2, 3) and (1, 2): the 3:2 and 2:1, at (3⁄2)^⅔ = 1.310 and 2^⅔ = 1.587
/// of its semi-major axis (P14.T21.b).
///
/// Neptune's put the classical Kuiper belt at 39.4–47.7 au, which holds most of its mass
/// (Pitjeva and Pitjev 2018, Celestial Mechanics and Dynamical Astronomy 130, 57).
pub const KUIPER_BAND_RESONANCES: [(u8, u8); 2] = [(2, 3), (1, 2)];

/// The spacing, in mutual Hill radii, above which a gap between two planets holds an asteroid belt
/// in a host without giants: 40 (P14.T21.a; plan 14's figure, given without a source, twice the
/// observed spacings of 14–20).
pub const GAP_BELT_SPACING: f64 = 40.0;

/// The range of the depletion factor of an asteroid belt inside a giant, log-uniform: 10⁻⁴–10⁻²
/// (P14.T21.a; ruling 84.3, in place of the plan's 10⁻³–10⁻¹).
///
/// The main belt holds 4.5 × 10⁻⁴ M⊕ (De Meo and Carry 2013, Icarus 226, 723; Pitjeva and Pitjev
/// 2018, Astronomy Letters 44, 554, 4.0 × 10⁻⁴), against about 1.25 M⊕ of rock that Hayashi's
/// (1981) nebula puts between 2.06 and 3.28 au, a depletion of about 3.6 × 10⁻⁴, inside this
/// range. No source gives the spread across systems, so the range is this generator's. The median
/// solar disc holds 1.2 M⊕ of solids there, so the Solar System input's belt holds 1.2 × 10⁻⁴ to
/// 0.012 M⊕.
pub const DEPLETION_WITH_GIANT: (f64, f64) = (1e-4, 1e-2);

/// The range of the depletion factor of an asteroid belt in a gap of a host without giants,
/// log-uniform: 10⁻²–1 (P14.T21.a; plan 14's figures, given without a source).
pub const DEPLETION_WITHOUT_GIANT: (f64, f64) = (1e-2, 1.0);

/// The mass from which a planet counts as a giant for the belts: [`ICE_GIANT_MASS`], 10 M⊕.
pub const BELT_GIANT_MASS: EarthMasses = ICE_GIANT_MASS;

/// The diameter of the largest planetesimals of a belt's collisional cascade, `D_c`, km: 450 (Kains,
/// Wyatt and Greaves 2011, best fit to FGK stars' discs; module documentation).
pub const CASCADE_TOP_DIAMETER: f64 = 450.0;

/// The dispersal threshold of a belt's planetesimals, `Q_D*`, J kg⁻¹: 495, solved so that with
/// `D_c` = 450 km and e = 0.05 the combination `D_c`^½ `Q_D*`^(5⁄6) e^(−5⁄3) is Sibthorpe et al.'s
/// (2018, MNRAS 475, 3046, §4.3) FGK best fit, A = 5.5 × 10⁵ km^½ J^(5⁄6) kg^(−5⁄6) (ruling 84: an
/// FGK prefactor for the collisional cap). Their §4.3 and Fig. 5's legend agree on it; the 10⁴
/// of their summary (§6, vi) is taken as a typo (ruling 84). Kains et al. (2011) fit 3,700.
pub const DISPERSAL_THRESHOLD: f64 = 495.0;

/// The planetesimals' mean eccentricity in the collisional lifetime, equal to their inclination
/// in radians: 0.05 (Kains et al. 2011; Wyatt et al. 2007b).
pub const CASCADE_ECCENTRICITY: f64 = 0.05;

/// The planetesimals' density in the cascade, kg m⁻³: 2,700 (Wyatt et al. 2007a, b; Kains et al.
/// 2011), which the coefficient 0.37 of [`fractional_luminosity`] assumes.
pub const CASCADE_DENSITY: f64 = 2_700.0;

/// The coefficient of Wyatt et al.'s (2007a) maximum belt mass at an age, 0.009 (their eq. 17, in
/// M⊕ with r in au, `D_c` in km, M★ in M☉ and the age in Myr, for q = 11⁄6, e = I and ρ = 2,700
/// kg m⁻³): `M_max` = 0.009 r^3.5 (dr ÷ r) `D_c` M★^(−½) ÷ (`t_age` G(11⁄6, `X_c`)).
pub const MAXIMUM_MASS_COEFFICIENT: f64 = 0.009;

/// The blackbody temperature from which a detected belt is warm: 100 K (ruling 84, amended; the
/// warm share is tested to be at most 0.03 of hosts).
pub const WARM_BELT_TEMPERATURE: Kelvin = Kelvin::new(100.0);

/// The temperature at which [`detection_weight`] is 1: 60 K, the detection threshold's reference
/// (ruling 84, amended).
pub const DETECTION_REFERENCE_TEMPERATURE: Kelvin = Kelvin::new(60.0);

/// The fractional luminosity a belt at [`DETECTION_REFERENCE_TEMPERATURE`] needs to be detected
/// at 100 µm: 10⁻⁶ (ruling 84, amended; Montesinos et al. 2016's median limits are 0.8–2.2 ×
/// 10⁻⁶).
pub const DETECTION_THRESHOLD: f64 = 1e-6;

/// The share of a belt's radius at which its dust has its blackbody temperature: 1 ÷ 2.5
/// (Sibthorpe et al. 2018: true radii are up to 2.5 times the blackbody ones; ruling 84, amended).
pub const BLACKBODY_RADIUS_SHARE: f64 = 0.4;

/// hc ÷ (λk) at 100 µm, K: 143.88, the second radiation constant 0.014388 m K over 10⁻⁴ m.
pub const DETECTION_WAVELENGTH_TEMPERATURE: f64 = 143.88;

/// The blackbody temperature of dust at 1 au from a star of 1 L☉, K: 278.3, (L☉ ÷ 16πσ au²)^¼.
pub const BLACKBODY_TEMPERATURE_AT_1_AU: f64 = 278.3;

/// The coefficient of the smallest planetesimal that can break one of the cascade's largest,
/// `X_c` = 10⁻³ (r `Q_D*` ÷ (M★ e²))^⅓, r in au and `Q_D*` in J kg⁻¹ (Wyatt et al. 2007a, below
/// eq. 18, for M★ = 1 M☉; Wyatt et al. 2007b, eq. 11, 1.3 × 10⁻³ [`Q_D*` r M★⁻¹ ÷ (1.25e² +
/// I²)]^⅓, which is 0.99 × 10⁻³ at e = I).
pub const DESTRUCTION_SIZE_COEFFICIENT: f64 = 1e-3;

/// The coefficient of the fractional luminosity of a cascade of index 11⁄6, 0.37: f = 0.37 r⁻²
/// `D_bl`^(−½) `D_c`^(−½) M, r in au, `D_bl` in µm, `D_c` in km, M in M⊕ (from Wyatt et al. 2007b,
/// eqs. 3–4, with ρ = 2,700 kg m⁻³: 3 M⊕ ÷ (8πρ au² √(1 µm × 1 km)) = 0.373).
pub const FRACTIONAL_LUMINOSITY_COEFFICIENT: f64 = 0.373;

/// The blowout diameter about a star of 1 L☉ and 1 M☉ for grains of 2,700 kg m⁻³, µm: 0.8 (Wyatt
/// et al. 2007a, eq. 6), scaling as L★ ÷ M★.
pub const BLOWOUT_DIAMETER_SOLAR: f64 = 0.8;

/// The smallest blowout diameter the fractional luminosity uses, µm: 0.01, a numerical guard of
/// this module's for faint hosts (white dwarfs, brown dwarfs), where radiation pressure removes no
/// grain and the cascade's formula would diverge. No FGK host comes near it.
pub const BLOWOUT_DIAMETER_FLOOR: f64 = 0.01;

/// The range of a belt's size slope q in N(> D) ∝ D^(−q), uniform: 2.5–3.5 (P14.T21.c; plan 14's
/// figures, given without a source). Fraser et al. (2014, ApJ 782, 100) fit the largest Kuiper
/// belt objects with steeper cumulative slopes, about 4.3 for the hot population and 7.5 for the
/// cold one above their magnitude breaks, and about 1 below them.
pub const SIZE_SLOPE: (f64, f64) = (2.5, 3.5);

/// The smallest body counted in a belt's size distribution, m: 1 km, the size the cometary halo
/// counts from (P14.T21.d).
pub const SMALLEST_BODY: Metres = Metres::new(1e3);

/// The diameter above which a belt's largest bodies are named members, m: 400 km (P14.T21.c; plan
/// 14's figure, given without a source), about where a body of rock or ice becomes round.
pub const MEMBER_MIN_DIAMETER: Metres = Metres::new(400e3);

/// The most named members a belt has: 8 (P14.T21.c).
pub const MAX_MEMBERS: u8 = 8;

/// The Rayleigh width of a belt member's eccentricity: 0.1 (P14.T21.c; plan 14's figure, given
/// without a source).
pub const MEMBER_ECCENTRICITY_WIDTH: f64 = 0.1;

/// The Rayleigh width of a belt member's inclination to its host's plane: 8° (P14.T21.c; plan 14's
/// figure, given without a source).
pub const MEMBER_INCLINATION_WIDTH_DEG: f64 = 8.0;

/// The Rayleigh width of a scattered member's eccentricity: 0.3, this module's figure for plan
/// 14's "more for the scattered component". The scattered disc's bodies have eccentricities of
/// 0.2–0.6 (Eris's is 0.44).
pub const SCATTERED_ECCENTRICITY_WIDTH: f64 = 0.3;

/// The Rayleigh width of a scattered member's inclination: 20°, this module's figure. The
/// scattered disc's inclinations reach 40° and more (Eris's is 44°).
pub const SCATTERED_INCLINATION_WIDTH_DEG: f64 = 20.0;

/// The largest eccentricity a member is placed on: 0.99, as for planets, a guard that keeps every
/// orbit bound.
pub const MEMBER_ECCENTRICITY_CAP: f64 = 0.99;

/// The bulk density of a rocky belt's members, kg m⁻³: 2,700, the cascade's (Wyatt et al.
/// 2007a). Vesta's is 3,456 and Ceres's, half water, 2,162.
pub const ROCKY_MEMBER_DENSITY: KilogramsPerCubicMetre = KilogramsPerCubicMetre::new(2_700.0);

/// The bulk density of an icy belt's members, kg m⁻³: 2,000, this module's figure between Pluto's
/// 1,854 (Stern et al. 2015, Science 350, aad1815) and Eris's 2,520 (Holler et al. 2021, Icarus
/// 355, 114130).
pub const ICY_MEMBER_DENSITY: KilogramsPerCubicMetre = KilogramsPerCubicMetre::new(2_000.0);

/// Bisection steps of the largest body's diameter: 96, a fixed count that brackets it to the last
/// bit on every platform.
const DIAMETER_STEPS: u32 = 96;

/// Where a belt lies, by the rule that placed it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BeltSite {
    /// Between the 4:1 and 2:1 resonances of the innermost giant beyond the snow line.
    InsideGiant,
    /// In a gap of more than 40 mutual Hill radii between two planets of a host without giants.
    Gap,
    /// Beyond the outermost planet, from its 3:2 to its 2:1 resonance and scattered beyond.
    BeyondPlanets,
    /// The outer third of the disc of a host without planets.
    OuterDisc,
}

/// Which side of the snow line a belt's solids lie on, which is its composition class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BeltComposition {
    /// Mostly rock: more of its solids lie inside the snow line than beyond.
    Rocky,
    /// Rock and ice: at least half its solids lie beyond the snow line.
    Icy,
}

impl BeltComposition {
    /// The bulk density of the belt's members.
    #[must_use]
    pub const fn member_density(self) -> KilogramsPerCubicMetre {
        match self {
            Self::Rocky => ROCKY_MEMBER_DENSITY,
            Self::Icy => ICY_MEMBER_DENSITY,
        }
    }
}

/// Which part of a belt a component or a member is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BeltPart {
    /// The belt proper: between its resonances, in its gap, or the disc's outer third.
    Main,
    /// A Kuiper-like belt's scattered component, from its 2:1 resonance to the disc's edge.
    Scattered,
}

impl BeltPart {
    /// The mean eccentricity of the part's population: that of a Rayleigh law, σ√(π ÷ 2), of
    /// width [`MEMBER_ECCENTRICITY_WIDTH`] in the belt proper and
    /// [`SCATTERED_ECCENTRICITY_WIDTH`] in a scattered component.
    #[must_use]
    pub fn mean_eccentricity(self) -> f64 {
        widths(self).0 * (PI / 2.0).sqrt()
    }

    /// The mean inclination of the part's population to its host's plane, rad, likewise from
    /// [`MEMBER_INCLINATION_WIDTH_DEG`] and [`SCATTERED_INCLINATION_WIDTH_DEG`].
    #[must_use]
    pub fn mean_inclination(self) -> Radians {
        Radians::new(widths(self).1 * (PI / 2.0).sqrt())
    }
}

/// One annulus of a belt: its edges and the mass of planetesimals it held at the start.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeltComponent {
    part: BeltPart,
    inner_edge: Metres,
    outer_edge: Metres,
    solids: EarthMasses,
    initial_mass: EarthMasses,
}

impl BeltComponent {
    /// Which part of the belt it is.
    #[must_use]
    pub const fn part(&self) -> BeltPart {
        self.part
    }

    /// Its inner edge, from the host.
    #[must_use]
    pub const fn inner_edge(&self) -> Metres {
        self.inner_edge
    }

    /// Its outer edge, from the host.
    #[must_use]
    pub const fn outer_edge(&self) -> Metres {
        self.outer_edge
    }

    /// The disc's solids between its edges, before depletion.
    #[must_use]
    pub const fn solids(&self) -> EarthMasses {
        self.solids
    }

    /// Its mass at the start, after depletion and before collisional wear: none for a scattered
    /// component (ruling 84.1).
    #[must_use]
    pub const fn initial_mass(&self) -> EarthMasses {
        self.initial_mass
    }

    /// Its mean radius r, the midpoint of its edges, which the cascade's formulae read.
    #[must_use]
    pub fn radius(&self) -> Metres {
        Metres::new(f64::midpoint(
            self.inner_edge.value(),
            self.outer_edge.value(),
        ))
    }

    /// Its relative width dr ÷ r.
    #[must_use]
    pub fn relative_width(&self) -> f64 {
        (self.outer_edge.value() - self.inner_edge.value()) / self.radius().value()
    }

    /// The collisional lifetime of its largest planetesimals at the start about a host of mass
    /// `host_mass` ([`collisional_time`]).
    #[must_use]
    pub fn collisional_time(&self, host_mass: SolarMasses) -> Years {
        collisional_time(
            self.radius(),
            self.relative_width(),
            host_mass,
            self.initial_mass,
        )
    }

    /// Its mass at age `age` about a host of mass `host_mass`: M₀ ÷ (1 + age ÷ `t_c`) (Wyatt et al.
    /// 2007a, eq. 14). The whole initial mass at an age of zero or less.
    #[must_use]
    pub fn mass_at(&self, age: Years, host_mass: SolarMasses) -> EarthMasses {
        let age = age.value().max(0.0);
        let t_c = self.collisional_time(host_mass).value();
        self.initial_mass / (1.0 + age / t_c)
    }
}

/// A belt's asteroid-belt gap at a resonance with its giant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeltGap {
    resonance: (u8, u8),
    radius: Metres,
}

impl BeltGap {
    /// The resonance, as (p + k, p): (3, 1), (5, 2) or (7, 3).
    #[must_use]
    pub const fn resonance(&self) -> (u8, u8) {
        self.resonance
    }

    /// The gap's distance from the host.
    #[must_use]
    pub const fn radius(&self) -> Metres {
        self.radius
    }
}

/// A belt's population (P14.T21.a–b), with its largest members (P14.T21.c): a body in its belt
/// slot, sub-index 0.
#[derive(Debug, Clone, PartialEq)]
pub struct Belt {
    index: BodyIndex,
    host: OrbitHost,
    host_mass: SolarMasses,
    kind: BeltKind,
    site: BeltSite,
    main: BeltComponent,
    scattered: Option<BeltComponent>,
    gaps: Vec<BeltGap>,
    depletion: f64,
    size_slope: f64,
    composition: BeltComposition,
    largest_diameter: Metres,
    members: Vec<BeltMember>,
}

impl Belt {
    /// The belt's body index, its slot's sub-index 0.
    #[must_use]
    pub const fn index(&self) -> BodyIndex {
        self.index
    }

    /// The orbit host it goes round.
    #[must_use]
    pub const fn host(&self) -> OrbitHost {
        self.host
    }

    /// Its host's zero-age mass, which its collisional wear reads.
    #[must_use]
    pub const fn host_mass(&self) -> SolarMasses {
        self.host_mass
    }

    /// Its kind: an asteroid belt or a Kuiper-like belt.
    #[must_use]
    pub const fn kind(&self) -> BeltKind {
        self.kind
    }

    /// The rule that placed it.
    #[must_use]
    pub const fn site(&self) -> BeltSite {
        self.site
    }

    /// The belt proper.
    #[must_use]
    pub const fn main(&self) -> &BeltComponent {
        &self.main
    }

    /// A Kuiper-like belt's scattered component, if the disc reaches beyond its 2:1 resonance.
    #[must_use]
    pub const fn scattered(&self) -> Option<&BeltComponent> {
        self.scattered.as_ref()
    }

    /// Its components, the belt proper first.
    pub fn components(&self) -> impl Iterator<Item = &BeltComponent> {
        core::iter::once(&self.main).chain(self.scattered.as_ref())
    }

    /// Its inner edge, the belt proper's.
    #[must_use]
    pub const fn inner_edge(&self) -> Metres {
        self.main.inner_edge
    }

    /// Its outer edge: the scattered component's, if it has one.
    #[must_use]
    pub fn outer_edge(&self) -> Metres {
        self.scattered
            .as_ref()
            .map_or(self.main.outer_edge, |s| s.outer_edge)
    }

    /// Its gaps at the Kirkwood resonances, inside out (an asteroid belt inside a giant's; none for
    /// the others).
    #[must_use]
    pub fn gaps(&self) -> &[BeltGap] {
        &self.gaps
    }

    /// The factor its solids were depleted by: log-uniform for an asteroid belt, and for a
    /// Kuiper-like one bright or faint ([`BeltDraws::kuiper_efficiency`]).
    #[must_use]
    pub const fn depletion(&self) -> f64 {
        self.depletion
    }

    /// Its size slope q, in N(> D) ∝ D^(−q).
    #[must_use]
    pub const fn size_slope(&self) -> f64 {
        self.size_slope
    }

    /// Its composition class, by the side of the snow line its solids lie on.
    #[must_use]
    pub const fn composition(&self) -> BeltComposition {
        self.composition
    }

    /// The diameter of its largest body, at which N(> D) = 1.
    #[must_use]
    pub const fn largest_diameter(&self) -> Metres {
        self.largest_diameter
    }

    /// Its named members, by sub-index.
    #[must_use]
    pub fn members(&self) -> &[BeltMember] {
        &self.members
    }

    /// The blackbody temperature of its dust about a host of luminosity `luminosity`, at the
    /// blackbody radius, [`BLACKBODY_RADIUS_SHARE`] of the belt proper's: 278.3 K (L ÷ L☉)^¼
    /// (`r_bb` ÷ au)^(−½).
    #[must_use]
    pub fn blackbody_temperature(&self, luminosity: SolarLuminosities) -> Kelvin {
        let r_au = BLACKBODY_RADIUS_SHARE * self.main.radius().value() / METRES_PER_AU;
        Kelvin::new(
            BLACKBODY_TEMPERATURE_AT_1_AU * luminosity.value().max(0.0).sqrt().sqrt() / r_au.sqrt(),
        )
    }

    /// Its fractional luminosity at age `age` about a host of luminosity `luminosity`, weighted as
    /// a 100 µm survey sees it: f × [`detection_weight`] of its blackbody temperature. The belt is
    /// detected when this reaches [`DETECTION_THRESHOLD`] (ruling 84, amended).
    #[must_use]
    pub fn detectable_luminosity(&self, age: Years, luminosity: SolarLuminosities) -> f64 {
        let f = self.fractional_luminosity(age, luminosity);
        if f <= 0.0 {
            return 0.0;
        }
        f * detection_weight(self.blackbody_temperature(luminosity))
    }

    /// Its mass at the start, after depletion.
    #[must_use]
    pub fn initial_mass(&self) -> EarthMasses {
        self.components()
            .fold(EarthMasses::ZERO, |sum, c| sum + c.initial_mass)
    }

    /// Its mass at age `age`, each component worn down collisionally.
    #[must_use]
    pub fn mass_at(&self, age: Years) -> EarthMasses {
        self.components().fold(EarthMasses::ZERO, |sum, c| {
            sum + c.mass_at(age, self.host_mass)
        })
    }

    /// Its fractional luminosity `L_dust` ÷ L★ at age `age` about a host now of luminosity
    /// `luminosity` ([`fractional_luminosity`] of each component's mass at that age, summed): what
    /// an infrared sensor sees. Zero about a dark host.
    #[must_use]
    pub fn fractional_luminosity(&self, age: Years, luminosity: SolarLuminosities) -> f64 {
        self.components()
            .map(|c| {
                fractional_luminosity(
                    c.mass_at(age, self.host_mass),
                    c.radius(),
                    luminosity,
                    self.host_mass,
                )
            })
            .sum()
    }
}

/// The Rayleigh widths of a part's eccentricities and inclinations (rad).
#[must_use]
fn widths(part: BeltPart) -> (f64, f64) {
    let degree = PI / 180.0;
    match part {
        BeltPart::Main => (
            MEMBER_ECCENTRICITY_WIDTH,
            MEMBER_INCLINATION_WIDTH_DEG * degree,
        ),
        BeltPart::Scattered => (
            SCATTERED_ECCENTRICITY_WIDTH,
            SCATTERED_INCLINATION_WIDTH_DEG * degree,
        ),
    }
}

/// A belt's named member (P14.T21.c): one of its largest bodies, in the belt's slot with
/// sub-index 1 upward, on its own orbit about the belt's host.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeltMember {
    index: BodyIndex,
    part: BeltPart,
    diameter: Metres,
    mass: EarthMasses,
    orbit: KeplerElements,
    radius_rank: UnitUniform,
}

impl BeltMember {
    /// The member's body index: its belt's slot, [`BodySub::Member`] from 1.
    #[must_use]
    pub const fn index(&self) -> BodyIndex {
        self.index
    }

    /// Which part of the belt it orbits in.
    #[must_use]
    pub const fn part(&self) -> BeltPart {
        self.part
    }

    /// Its diameter in the belt's size distribution, D = `D_max` k^(−1⁄q) for the k-th largest.
    #[must_use]
    pub const fn diameter(&self) -> Metres {
        self.diameter
    }

    /// Its mass, a sphere of that diameter at its belt's member density.
    #[must_use]
    pub const fn mass(&self) -> EarthMasses {
        self.mass
    }

    /// Its primordial orbit about the belt's host, in the system frame.
    #[must_use]
    pub const fn orbit(&self) -> &KeplerElements {
        &self.orbit
    }

    /// Its radius rank, the one number its derivation draws (design note 8).
    #[must_use]
    pub const fn radius_rank(&self) -> UnitUniform {
        self.radius_rank
    }

    /// The member as P14.T16's derivation takes it, a dwarf planet formed where it orbits:
    /// [`derive_body`](crate::planetary::derive::derive_body) of this, with the belt host's disc,
    /// gives its radius, composition and temperatures.
    ///
    /// # Panics
    ///
    /// Never: a member's mass and semi-major axis are positive by construction.
    #[must_use]
    pub fn placed_body(&self) -> PlacedBody {
        PlacedBody::new(
            self.mass,
            self.orbit,
            self.orbit.semi_major_axis(),
            self.radius_rank,
        )
        .expect("a member's mass and semi-major axis are positive")
    }
}

/// An orbit host as its belts read it (P14.T21): which host it is, its disc, its planets and its
/// plane, all plain arguments.
///
/// P14.T30.a builds it from the host's zone, disc and placed planets.
#[derive(Debug, Clone, Copy)]
pub struct BeltHost<'a> {
    host: OrbitHost,
    disc: &'a DiscProfile,
    planets: &'a [Neighbour],
    plane: SystemPlane,
}

impl<'a> BeltHost<'a> {
    /// The orbit host `host`, with its disc `disc`, its planets `planets` (in any order) and its
    /// planetary plane `plane`.
    #[must_use]
    pub const fn new(
        host: OrbitHost,
        disc: &'a DiscProfile,
        planets: &'a [Neighbour],
        plane: SystemPlane,
    ) -> Self {
        Self {
            host,
            disc,
            planets,
            plane,
        }
    }

    /// The host's mass, its disc's.
    #[must_use]
    pub const fn mass(&self) -> SolarMasses {
        self.disc.host_mass()
    }
}

/// A host's belts, and the next free belt slot.
#[derive(Debug, Clone, PartialEq)]
pub struct HostBelts {
    belts: Vec<Belt>,
    next_slot: u8,
}

impl HostBelts {
    /// The belts, inside out.
    #[must_use]
    pub fn belts(&self) -> &[Belt] {
        &self.belts
    }

    /// The belts, by value.
    #[must_use]
    pub fn into_belts(self) -> Vec<Belt> {
        self.belts
    }

    /// The first belt slot after this host's: where the next host's belts begin.
    #[must_use]
    pub const fn next_slot(&self) -> u8 {
        self.next_slot
    }
}

/// The draws of the belt in one belt slot, on [`tags::BELT_POPULATION`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeltDraws {
    /// The rank of an asteroid belt's depletion factor in its log-uniform range. Word 8n.
    pub depletion: UnitUniform,
    /// The rank of its size slope in [`SIZE_SLOPE`]. Word 8n + 1.
    pub size_slope: UnitUniform,
    /// Whether a Kuiper-like belt is bright, against [`BRIGHT_KUIPER_BELT_PROBABILITY`]. Word
    /// 8n + 2.
    pub bright: Mark,
    /// A bright Kuiper-like belt's efficiency, the standard normal of its log. Words 8n + 3 and
    /// 8n + 4.
    pub efficiency: StandardNormal,
}

impl BeltDraws {
    /// The draws of the belt in belt slot `slot` of `system`, in the universe of `seed`.
    ///
    /// # Panics
    ///
    /// Never: an open uniform lies inside (0, 1) and a Box–Muller variate is finite.
    #[must_use]
    pub fn for_slot(seed: Seed, system: SystemId, slot: u8) -> Self {
        let mut stream = Stream::open(seed, tags::BELT_POPULATION, ObjectKey::from(system));
        stream.seek(u64::from(slot) * BELT_WORDS_PER_SLOT);
        let depletion = open_rank(&mut stream);
        let size_slope = open_rank(&mut stream);
        let bright = Mark::from_word(stream.next_u64());
        let efficiency =
            StandardNormal::new(stream.standard_normal()).expect("a Box–Muller variate is finite");
        Self {
            depletion,
            size_slope,
            bright,
            efficiency,
        }
    }

    /// The share of its solids a Kuiper-like belt keeps (ruling 84.1): with probability
    /// [`BRIGHT_KUIPER_BELT_PROBABILITY`] a bright belt's, 10^(μ + σz) of
    /// [`BRIGHT_KUIPER_EFFICIENCY_DEX`] and [`BRIGHT_KUIPER_EFFICIENCY_SIGMA_DEX`], held to 1, and
    /// otherwise [`FAINT_KUIPER_EFFICIENCY`].
    #[must_use]
    pub fn kuiper_efficiency(&self) -> f64 {
        if self
            .bright
            .is_below(Threshold::from_probability(BRIGHT_KUIPER_BELT_PROBABILITY))
        {
            math::exp10(
                BRIGHT_KUIPER_EFFICIENCY_DEX
                    + BRIGHT_KUIPER_EFFICIENCY_SIGMA_DEX * self.efficiency.value(),
            )
            .min(1.0)
        } else {
            FAINT_KUIPER_EFFICIENCY
        }
    }
}

/// A member's draws on [`tags::BELT_MEMBER`], keyed by its own ID: words 0–7 in field order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MemberDraws {
    /// Which component it orbits in, by the shares of the disc's solids between their edges. Word
    /// 0.
    pub part: UnitUniform,
    /// Its semi-major axis's rank in the component's solids. Word 1.
    pub semi_major_axis: UnitUniform,
    /// Its eccentricity's rank in the truncated Rayleigh law. Word 2.
    pub eccentricity: UnitUniform,
    /// Its inclination's rank in the Rayleigh law. Word 3.
    pub inclination: UnitUniform,
    /// Its node on the host's plane, as a share of a turn. Word 4.
    pub node: UnitUniform,
    /// Its argument of periapsis, as a share of a turn. Word 5.
    pub periapsis: UnitUniform,
    /// Its mean anomaly at the epoch, as a share of a turn. Word 6.
    pub mean_anomaly: UnitUniform,
    /// Its radius rank (design note 8). Word 7.
    pub radius_rank: UnitUniform,
}

impl MemberDraws {
    /// The draws of member `member` of `system`, in the universe of `seed`.
    #[must_use]
    pub fn for_member(seed: Seed, system: SystemId, member: BodyIndex) -> Self {
        let mut stream = Stream::open(
            seed,
            tags::BELT_MEMBER,
            ObjectKey::from(member.body_id(system)),
        );
        let mut rank = || open_rank(&mut stream);
        Self {
            part: rank(),
            semi_major_axis: rank(),
            eccentricity: rank(),
            inclination: rank(),
            node: rank(),
            periapsis: rank(),
            mean_anomaly: rank(),
            radius_rank: rank(),
        }
    }
}

/// The next word of `stream` as a rank strictly inside (0, 1).
#[must_use]
fn open_rank(stream: &mut Stream) -> UnitUniform {
    UnitUniform::new(stream.uniform_open()).expect("an open uniform lies strictly inside (0, 1)")
}

/// `lo` × (`hi` ÷ `lo`)^`rank`.
#[must_use]
fn log_uniform((lo, hi): (f64, f64), rank: UnitUniform) -> f64 {
    lo * math::powf(hi / lo, rank.value())
}

/// Where a body that completes p orbits for a planet's q lies, as a share of the planet's
/// semi-major axis: (q ÷ p)^⅔, below 1 for an inner resonance such as (4, 1) and above it for an
/// outer one such as (2, 3).
#[must_use]
fn resonance_ratio((p, q): (u8, u8)) -> f64 {
    math::powf(f64::from(q) / f64::from(p), 2.0 / 3.0)
}

/// Where a planet's chaotic zone ends on either side: its pericentre less, and its apocentre
/// plus, 1.3 μ^(2⁄7) of its semi-major axis (Wisdom 1980).
#[must_use]
fn chaotic_zone(planet: &Neighbour, host_mass: SolarMasses) -> (f64, f64) {
    let mu = planet.mass().value() * EARTH_MASS_KG / (host_mass.value() * SOLAR_MASS_KG);
    let a = planet.semi_major_axis().value();
    let e = planet.eccentricity();
    let half = CHAOTIC_ZONE_COEFFICIENT * math::powf(mu, 2.0 / 7.0) * a;
    (a * (1.0 - e) - half, a * (1.0 + e) + half)
}

/// Whether a planet is one of the belts' giants.
#[must_use]
fn is_giant(planet: &Neighbour) -> bool {
    planet.mass() >= BELT_GIANT_MASS
}

/// The 100 µm excess of a blackbody belt at temperature `temperature` relative to one at
/// [`DETECTION_REFERENCE_TEMPERATURE`] of the same fractional luminosity about the same host:
/// R₁₀₀(T) ÷ R₁₀₀(60 K), with R₁₀₀(T) = `B_ν`(T) ÷ σT⁴ ∝ 1 ÷ (T⁴ (e^(hν ÷ kT) − 1)) at 100 µm,
/// since a belt's flux is f L★ spread as a blackbody at T (ruling 84, amended). Zero at 0 K.
///
/// # Examples
///
/// A belt at 60 K counts in full; one at 150 K needs about six times the fractional luminosity:
///
/// ```
/// use hyperion_sim::planetary::belts::detection_weight;
/// use hyperion_sim::units::Kelvin;
///
/// assert!((detection_weight(Kelvin::new(60.0)) - 1.0).abs() < 1e-12);
/// let hot = detection_weight(Kelvin::new(150.0));
/// assert!((0.1..0.25).contains(&hot));
/// ```
#[must_use]
pub fn detection_weight(temperature: Kelvin) -> f64 {
    let x = DETECTION_WAVELENGTH_TEMPERATURE;
    let excess = |t: f64| 1.0 / (t * t * t * t * math::exp_m1(x / t));
    let t = temperature.value();
    if t <= 0.0 {
        return 0.0;
    }
    excess(t) / excess(DETECTION_REFERENCE_TEMPERATURE.value())
}

/// Wyatt et al.'s (2007a, eqs. 17–18) largest fractional luminosity a belt at radius `r` of
/// relative width `relative_width` about a host of mass `host_mass` and luminosity `luminosity`
/// can keep at age `age`: that of `M_max`(t) = 0.009 r^3.5 (dr ÷ r) `D_c` M★^(−½) ÷ (t G), with the
/// full G ([`cascade_factor`]) and this module's parameters, ∝ r^(7⁄3) ÷ t while `X_c` ≪ 1. Any
/// belt's [`fractional_luminosity`] at that age lies below it. Infinite where G is zero or at an
/// age of zero or less.
#[must_use]
pub fn maximum_fractional_luminosity(
    r: Metres,
    relative_width: f64,
    host_mass: SolarMasses,
    luminosity: SolarLuminosities,
    age: Years,
) -> f64 {
    if age.value() <= 0.0 {
        return f64::INFINITY;
    }
    let unit = collisional_time(r, relative_width, host_mass, EarthMasses::new(1.0)).value();
    fractional_luminosity(
        EarthMasses::new(unit / age.value()),
        r,
        luminosity,
        host_mass,
    )
}

/// The factor G(11⁄6, `X_c`) = `X_c`^(−½) + 0.67 `X_c`^(−1.5) + 0.2 `X_c`^(−2.5) − 1.87 of a belt at
/// radius `r` about a host of mass `host_mass`, with `X_c` = 10⁻³ (r `Q_D*` ÷ (M★ e²))^⅓ (Wyatt et
/// al. 2007a, eqs. 17–18 and below; ruling 84.1). It falls to zero at `X_c` = 1, where no
/// planetesimal of the cascade can break its largest, and is taken as zero beyond. The small-`X_c`
/// form 0.2 `X_c`^(−2.5) is 0.5–0.8 of the full G at 40 au, where `X_c` ≈ 0.20.
#[must_use]
pub fn cascade_factor(r: Metres, host_mass: SolarMasses) -> f64 {
    let r_au = r.value() / METRES_PER_AU;
    let x = DESTRUCTION_SIZE_COEFFICIENT
        * math::cbrt(
            r_au * DISPERSAL_THRESHOLD
                / (host_mass.value() * CASCADE_ECCENTRICITY * CASCADE_ECCENTRICITY),
        );
    if x >= 1.0 {
        return 0.0;
    }
    (1.0 / x.sqrt() + 0.67 * math::powf(x, -1.5) + 0.2 * math::powf(x, -2.5) - 1.87).max(0.0)
}

/// The collisional lifetime of the largest planetesimals of a belt of mass `mass` at radius `r`,
/// of relative width `relative_width`, about a host of mass `host_mass`: `t_c` = `M_max` `t_age`
/// ÷ M = 0.009 r^3.5 (dr ÷ r) `D_c` M★^(−½) ÷ (G M) Myr (Wyatt et al. 2007a, eq. 17, with the
/// full G of [`cascade_factor`]; the module documentation's parameters). Infinite for a massless
/// belt, or one whose G is zero.
///
/// # Examples
///
/// The classical Kuiper belt, 0.02 M⊕ at 39–48 au, has not worn down in the Sun's lifetime:
///
/// ```
/// use hyperion_sim::planetary::belts::collisional_time;
/// use hyperion_sim::units::{AstronomicalUnits, EarthMasses, Metres, SolarMasses};
///
/// let r = Metres::from(AstronomicalUnits::new(43.5));
/// let t = collisional_time(r, 0.2, SolarMasses::new(1.0), EarthMasses::new(0.02));
/// assert!(t.value() > 1e12);
/// ```
#[must_use]
pub fn collisional_time(
    r: Metres,
    relative_width: f64,
    host_mass: SolarMasses,
    mass: EarthMasses,
) -> Years {
    let g = cascade_factor(r, host_mass);
    if g <= 0.0 || mass.value() <= 0.0 {
        return Years::new(f64::INFINITY);
    }
    let r_au = r.value() / METRES_PER_AU;
    let myr =
        MAXIMUM_MASS_COEFFICIENT * math::powf(r_au, 3.5) * relative_width * CASCADE_TOP_DIAMETER
            / (host_mass.value().sqrt() * g * mass.value());
    Years::new(myr * 1e6)
}

/// The fractional luminosity `L_dust` ÷ L★ of a belt of mass `mass` at radius `r` about a host of
/// luminosity `luminosity` and mass `host_mass`: f = 0.37 r⁻² `D_bl`^(−½) `D_c`^(−½) M, with
/// `D_bl` = 0.8 (L★ ÷ M★) µm (Wyatt et al. 2007a, b; module documentation). Zero about a dark
/// host.
///
/// # Examples
///
/// The Kuiper belt's mass, at its radius about the Sun, gives the order of its measured
/// fractional luminosity, about 10⁻⁷ (Vitense et al. 2012, A&A 540, A30):
///
/// ```
/// use hyperion_sim::planetary::belts::fractional_luminosity;
/// use hyperion_sim::units::{AstronomicalUnits, EarthMasses, Metres, SolarLuminosities, SolarMasses};
///
/// let r = Metres::from(AstronomicalUnits::new(43.5));
/// let f = fractional_luminosity(EarthMasses::new(0.02), r, SolarLuminosities::new(1.0), SolarMasses::new(1.0));
/// assert!((5e-8..5e-7).contains(&f));
/// ```
#[must_use]
pub fn fractional_luminosity(
    mass: EarthMasses,
    r: Metres,
    luminosity: SolarLuminosities,
    host_mass: SolarMasses,
) -> f64 {
    if luminosity.value() <= 0.0 || mass.value() <= 0.0 {
        return 0.0;
    }
    let r_au = r.value() / METRES_PER_AU;
    let blowout = (BLOWOUT_DIAMETER_SOLAR * luminosity.value() / host_mass.value())
        .max(BLOWOUT_DIAMETER_FLOOR);
    FRACTIONAL_LUMINOSITY_COEFFICIENT * mass.value()
        / (r_au * r_au * (blowout * CASCADE_TOP_DIAMETER).sqrt())
}

/// The diameter `D_max` of the largest body of a belt of mass `mass` whose bodies from
/// [`SMALLEST_BODY`] up follow N(> D) = (D ÷ `D_max`)^(−q), at density `density`.
///
/// The mass of that population is (π ÷ 6) ρ q ∫ D^(2 − q) `D_max`^q dD from the smallest body to
/// `D_max`, increasing in `D_max`; the bisection on ln `D_max` takes a fixed number of steps.
#[must_use]
pub fn largest_diameter(mass: EarthMasses, q: f64, density: KilogramsPerCubicMetre) -> Metres {
    let target = mass.value() * EARTH_MASS_KG;
    let d_min = SMALLEST_BODY.value();
    let population = |d_max: f64| {
        let integral = if (q - 3.0).abs() < 1e-12 {
            math::ln(d_max / d_min)
        } else {
            (math::powf(d_max, 3.0 - q) - math::powf(d_min, 3.0 - q)) / (3.0 - q)
        };
        PI / 6.0 * density.value() * q * math::powf(d_max, q) * integral
    };
    // 10⁶ km bounds every belt: its population would outweigh any disc.
    let (mut lo, mut hi) = (math::ln(d_min), math::ln(1e9));
    for _ in 0..DIAMETER_STEPS {
        let mid = f64::midpoint(lo, hi);
        if population(math::exp(mid)) < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Metres::new(math::exp(f64::midpoint(lo, hi)))
}

/// A belt's place before its mass: its kind, site, components' edges and gaps, and its
/// depletion's range, or the fixed depletion of a Kuiper-like belt.
struct Plan {
    kind: BeltKind,
    site: BeltSite,
    main: (f64, f64),
    scattered: Option<(f64, f64)>,
    gaps: Vec<BeltGap>,
    depletion: Depletion,
}

/// How a belt's solids are depleted.
#[derive(Clone, Copy)]
enum Depletion {
    /// Log-uniform over the range: an asteroid belt's.
    Drawn((f64, f64)),
    /// Bright or faint (ruling 84.1): a Kuiper-like belt's.
    Bimodal,
}

/// The belts of `host` in the universe of `seed`, taking belt slots from `first_slot` (P14.T21.a–c):
/// its asteroid belts inside out, then its Kuiper-like belt, each with its largest members.
///
/// `first_slot` is a belt slot number, [`FIRST_BELT_SLOT`] for a system's first host; each later
/// host starts at the last one's [`HostBelts::next_slot`]. Belts beyond [`LAST_BELT_SLOT`] are not
/// placed.
///
/// # Examples
///
/// The Sun's disc with Jupiter, Saturn, Uranus and Neptune gives the main belt and the Kuiper
/// belt where they are:
///
/// ```
/// use hyperion_sim::id::SystemId;
/// use hyperion_sim::planetary::belts::{BeltHost, FIRST_BELT_SLOT, host_belts};
/// use hyperion_sim::planetary::disc::{self, DiscDraws, DiscHost, Truncation};
/// use hyperion_sim::planetary::placement::classes::orbits::SystemPlane;
/// use hyperion_sim::planetary::placement::{Neighbour, OrbitHost};
/// use hyperion_sim::planetary::record::BeltKind;
/// use hyperion_sim::Seed;
/// use hyperion_sim::units::{AstronomicalUnits, Dex, EarthMasses, Megayears, Metres, Radians};
/// use hyperion_sim::units::{SolarLuminosities, SolarMasses, SolarRadii};
///
/// let au = |x: f64| Metres::from(AstronomicalUnits::new(x));
/// let sun = DiscHost::new(SolarMasses::new(1.0), Dex::new(0.0), SolarLuminosities::new(0.7), SolarRadii::new(0.89))?;
/// let disc = disc::derive(&sun, Megayears::new(3.0), &DiscDraws::MEDIAN, Truncation::NONE);
/// let disc = disc.profile().expect("the median disc exists");
/// let planets = [
///     Neighbour::new(EarthMasses::new(317.8), au(5.203), 0.048),
///     Neighbour::new(EarthMasses::new(95.16), au(9.537), 0.054),
///     Neighbour::new(EarthMasses::new(14.54), au(19.19), 0.047),
///     Neighbour::new(EarthMasses::new(17.15), au(30.07), 0.009),
/// ];
/// let plane = SystemPlane::new(Radians::ZERO, Radians::ZERO)?;
/// let host = BeltHost::new(OrbitHost::Star(0), disc, &planets, plane);
/// let system = SystemId::from_raw(0x0200_0800_2000_0000)?;
/// let belts = host_belts(Seed::new(7), system, &host, FIRST_BELT_SLOT);
/// let [main, kuiper] = belts.belts() else { panic!("two belts") };
/// assert_eq!((main.kind(), kuiper.kind()), (BeltKind::Asteroid, BeltKind::Kuiper));
/// let edge = |m: Metres| AstronomicalUnits::from(m).value();
/// assert!((2.0..2.2).contains(&edge(main.inner_edge())));
/// assert!((3.2..3.35).contains(&edge(main.main().outer_edge())));
/// assert!((39.0..40.0).contains(&edge(kuiper.inner_edge())));
/// assert!((47.0..48.0).contains(&edge(kuiper.main().outer_edge())));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn host_belts(seed: Seed, system: SystemId, host: &BeltHost<'_>, first_slot: u8) -> HostBelts {
    let host_mass = host.mass();
    let mut planets: Vec<Neighbour> = host.planets.to_vec();
    planets.sort_by(|a, b| {
        a.semi_major_axis()
            .value()
            .total_cmp(&b.semi_major_axis().value())
            .then(a.mass().value().total_cmp(&b.mass().value()))
    });
    let zones: Vec<(f64, f64)> = planets.iter().map(|p| chaotic_zone(p, host_mass)).collect();
    let plans = plans(host, &planets, &zones);

    let mut belts = Vec::with_capacity(plans.len());
    let mut slot = first_slot;
    for plan in plans {
        if slot > LAST_BELT_SLOT {
            break;
        }
        if let Some(belt) = build(seed, system, host, &planets, &zones, slot, plan) {
            belts.push(belt);
            slot += 1;
        }
    }
    HostBelts {
        belts,
        next_slot: slot,
    }
}

/// Where `host`'s belts go, inside out, before their masses: the rules of the module
/// documentation applied to its planets `planets`, sorted by semi-major axis, whose chaotic zones
/// are `zones`.
fn plans(host: &BeltHost<'_>, planets: &[Neighbour], zones: &[(f64, f64)]) -> Vec<Plan> {
    let (disc_in, disc_out) = (
        host.disc.inner_edge().value(),
        host.disc.outer_edge().value(),
    );
    let snow = host.disc.snow_line().value();
    let clear = |lo: f64, hi: f64| zones.iter().all(|&(zl, zh)| zh <= lo || zl >= hi);

    let host_mass = host.mass();
    let mut plans = Vec::new();
    let giant_beyond = planets
        .iter()
        .position(|p| is_giant(p) && p.semi_major_axis().value() >= snow);
    if let Some(g) = giant_beyond {
        let a = planets[g].semi_major_axis().value();
        let lo = a * resonance_ratio(ASTEROID_BAND_RESONANCES[0]);
        let hi = a * resonance_ratio(ASTEROID_BAND_RESONANCES[1]);
        if clear(lo, hi) {
            let gaps = KIRKWOOD_RESONANCES
                .iter()
                .map(|&resonance| BeltGap {
                    resonance,
                    radius: Metres::new(a * resonance_ratio(resonance)),
                })
                .collect();
            plans.push(Plan {
                kind: BeltKind::Asteroid,
                site: BeltSite::InsideGiant,
                main: (lo, hi),
                scattered: None,
                gaps,
                depletion: Depletion::Drawn(DEPLETION_WITH_GIANT),
            });
        }
    } else {
        for (pair, zone) in planets.windows(2).zip(zones.windows(2)) {
            let (inner, outer) = (&pair[0], &pair[1]);
            let (a1, a2) = (inner.semi_major_axis(), outer.semi_major_axis());
            let hill = mutual_hill_radius(inner.mass(), outer.mass(), host_mass, a1, a2);
            let spacing = (a2 - a1) / hill;
            if spacing > GAP_BELT_SPACING && clear(zone[0].1, zone[1].0) {
                plans.push(Plan {
                    kind: BeltKind::Asteroid,
                    site: BeltSite::Gap,
                    main: (zone[0].1, zone[1].0),
                    scattered: None,
                    gaps: Vec::new(),
                    depletion: Depletion::Drawn(DEPLETION_WITHOUT_GIANT),
                });
            }
        }
    }
    match (planets.last(), zones.last()) {
        (Some(outermost), Some(&(_, zone_out))) => {
            let a = outermost.semi_major_axis().value();
            let lo = (a * resonance_ratio(KUIPER_BAND_RESONANCES[0])).max(zone_out);
            let hi = a * resonance_ratio(KUIPER_BAND_RESONANCES[1]);
            plans.push(Plan {
                kind: BeltKind::Kuiper,
                site: BeltSite::BeyondPlanets,
                main: (lo, hi),
                scattered: Some((hi.max(zone_out), disc_out)),
                gaps: Vec::new(),
                depletion: Depletion::Bimodal,
            });
        }
        _ => plans.push(Plan {
            kind: BeltKind::Kuiper,
            site: BeltSite::OuterDisc,
            main: (disc_in + 2.0 / 3.0 * (disc_out - disc_in), disc_out),
            scattered: None,
            gaps: Vec::new(),
            depletion: Depletion::Bimodal,
        }),
    }

    plans
}

/// The belt `plan` in belt slot `slot`, cut to the disc and its solids; `None` if nothing is
/// left of it.
fn build(
    seed: Seed,
    system: SystemId,
    host: &BeltHost<'_>,
    planets: &[Neighbour],
    zones: &[(f64, f64)],
    slot: u8,
    plan: Plan,
) -> Option<Belt> {
    let disc = host.disc;
    let (disc_in, disc_out) = (disc.inner_edge().value(), disc.outer_edge().value());
    let draws = BeltDraws::for_slot(seed, system, slot);
    let depletion = match plan.depletion {
        Depletion::Drawn(range) => log_uniform(range, draws.depletion),
        Depletion::Bimodal => draws.kuiper_efficiency(),
    };
    let component = |part: BeltPart, (lo, hi): (f64, f64)| {
        let (lo, hi) = (lo.max(disc_in), hi.min(disc_out));
        if hi <= lo {
            return None;
        }
        let solids = disc.solid_mass_between(Metres::new(lo), Metres::new(hi));
        (solids.value() > 0.0).then(|| BeltComponent {
            part,
            inner_edge: Metres::new(lo),
            outer_edge: Metres::new(hi),
            solids,
            initial_mass: solids * depletion,
        })
    };
    let main = component(BeltPart::Main, plan.main);
    let scattered = plan
        .scattered
        .and_then(|range| component(BeltPart::Scattered, range));
    // The scattered component has no mass of its own (ruling 84.1): its bodies are the belt's,
    // scattered out, and it gives the belt's members their wider orbits. A Kuiper-like belt whose
    // resonant band the disc does not reach keeps its scattered part as its main one, with mass.
    let (main, scattered) = match (main, scattered) {
        (Some(main), scattered) => (
            main,
            scattered.map(|s| BeltComponent {
                initial_mass: EarthMasses::ZERO,
                ..s
            }),
        ),
        (None, Some(scattered)) => (
            BeltComponent {
                part: BeltPart::Main,
                ..scattered
            },
            None,
        ),
        (None, None) => return None,
    };
    let index = BodyIndex::new(BodySlot::Belt(slot), BodySub::Primary)
        .expect("a belt slot of 1–13 is in the layout");
    let (inner, outer) = (
        main.inner_edge,
        scattered.map_or(main.outer_edge, |s| s.outer_edge),
    );
    let snow = disc.snow_line();
    let beyond = disc.solid_mass_between(Metres::new(snow.value().max(inner.value())), outer);
    let total = disc.solid_mass_between(inner, outer);
    let composition = if beyond.value() * 2.0 >= total.value() {
        BeltComposition::Icy
    } else {
        BeltComposition::Rocky
    };
    let size_slope = SIZE_SLOPE.0 + (SIZE_SLOPE.1 - SIZE_SLOPE.0) * draws.size_slope.value();
    let initial = main.initial_mass + scattered.map_or(EarthMasses::ZERO, |s| s.initial_mass);
    let density = composition.member_density();
    let largest = largest_diameter(initial, size_slope, density);
    let gaps = plan
        .gaps
        .into_iter()
        .filter(|gap| inner < gap.radius && gap.radius < main.outer_edge)
        .collect();
    let mut belt = Belt {
        index,
        host: host.host,
        host_mass: disc.host_mass(),
        kind: plan.kind,
        site: plan.site,
        main,
        scattered,
        gaps,
        depletion,
        size_slope,
        composition,
        largest_diameter: largest,
        members: Vec::new(),
    };
    belt.members = members(seed, system, host, planets, zones, &belt, slot);
    Some(belt)
}

/// The named members of `belt` (P14.T21.c): its bodies over [`MEMBER_MIN_DIAMETER`], at most
/// [`MAX_MEMBERS`], largest first, each on an orbit inside the belt.
fn members(
    seed: Seed,
    system: SystemId,
    host: &BeltHost<'_>,
    planets: &[Neighbour],
    zones: &[(f64, f64)],
    belt: &Belt,
    slot: u8,
) -> Vec<BeltMember> {
    let q = belt.size_slope;
    let density = belt.composition.member_density().value();
    let total = belt.components().map(|c| c.solids.value()).sum::<f64>();
    let main_share = belt.main.solids.value() / total;
    (1..=MAX_MEMBERS)
        .map_while(|k| {
            let diameter = belt.largest_diameter.value() * math::powf(f64::from(k), -1.0 / q);
            (diameter > MEMBER_MIN_DIAMETER.value()).then_some((k, diameter))
        })
        .map(|(k, diameter)| {
            let index = BodyIndex::new(BodySlot::Belt(slot), BodySub::Member(k))
                .expect("members 1–8 of a belt slot are in the layout");
            let draws = MemberDraws::for_member(seed, system, index);
            let component = match belt.scattered.as_ref() {
                Some(scattered) if draws.part.value() >= main_share => scattered,
                Some(_) | None => &belt.main,
            };
            let r = diameter / 2.0;
            let mass = EarthMasses::new(4.0 / 3.0 * PI * r * r * r * density / EARTH_MASS_KG);
            let a = member_axis(host.disc, component, draws.semi_major_axis);
            let (e_width, i_width) = widths(component.part);
            let e_limit = eccentricity_limit(a, planets, zones, host.disc);
            let e = truncated_rayleigh(e_width, e_limit, draws.eccentricity);
            let inclination = mutual_inclination(i_width, draws.inclination);
            let orientation = orientation_in_plane(
                &host.plane,
                inclination,
                Radians::new(TAU * draws.node.value()),
                Radians::new(TAU * draws.periapsis.value()),
            )
            .expect("an inclination in [0, π] and finite angles build an orientation");
            let mu = GravitationalParameter::new(
                GM_SUN * belt.host_mass.value() + GM_EARTH * mass.value(),
            );
            let orbit = KeplerElements::from_semi_major_axis(
                Metres::new(a),
                mu,
                Eccentricity::new(e).expect("a truncated eccentricity lies in [0, 0.99]"),
                orientation,
                Radians::new(TAU * draws.mean_anomaly.value()),
            )
            .expect("a member's axis and μ are positive and finite");
            BeltMember {
                index,
                part: component.part,
                diameter: Metres::new(diameter),
                mass,
                orbit,
                radius_rank: draws.radius_rank,
            }
        })
        .collect()
}

/// A semi-major axis inside `component` at rank `rank` of its solids' distribution in radius: the
/// disc's solids per unit radius fall as e^(−r ÷ `r_c`) on either side of the snow line, and the
/// quantile inverts that taper in closed form on the side the rank falls on.
fn member_axis(disc: &DiscProfile, component: &BeltComponent, rank: UnitUniform) -> f64 {
    let (lo, hi) = (component.inner_edge.value(), component.outer_edge.value());
    let total = disc
        .solid_mass_between(component.inner_edge, component.outer_edge)
        .value();
    let snow = disc.snow_line().value().clamp(lo, hi);
    let inside = disc
        .solid_mass_between(component.inner_edge, Metres::new(snow))
        .value();
    let target = rank.value() * total;
    let rc = disc.characteristic_radius().value();
    // ∫ e^(−r ÷ r_c) dr from a to x is a share s of that from a to b: x = a − r_c ln(1 − s (1 −
    // e^(−(b − a) ÷ r_c))).
    let invert = |a: f64, b: f64, share: f64| {
        let span = -math::exp_m1(-(b - a) / rc);
        (a - rc * math::ln_1p(-share * span)).clamp(a, b)
    };
    if target <= inside && inside > 0.0 {
        invert(lo, snow, target / inside)
    } else {
        let beyond = total - inside;
        let share = if beyond > 0.0 {
            (target - inside) / beyond
        } else {
            0.5
        };
        invert(snow, hi, share.clamp(0.0, 1.0))
    }
}

/// The largest eccentricity at which an orbit of semi-major axis `a` stays clear of every
/// planet's chaotic zone and inside the disc: its pericentre beyond the nearest zone inside it (or
/// the disc's inner edge) and its apocentre inside the nearest zone beyond it (or the disc's outer
/// edge, which lies inside the strip radius of design note 14), held to
/// [`MEMBER_ECCENTRICITY_CAP`].
fn eccentricity_limit(
    a: f64,
    planets: &[Neighbour],
    zones: &[(f64, f64)],
    disc: &DiscProfile,
) -> f64 {
    let floor = planets
        .iter()
        .zip(zones)
        .filter(|(p, _)| p.semi_major_axis().value() < a)
        .map(|(_, &(_, hi))| hi)
        .fold(disc.inner_edge().value(), f64::max);
    let ceiling = planets
        .iter()
        .zip(zones)
        .filter(|(p, _)| p.semi_major_axis().value() > a)
        .map(|(_, &(lo, _))| lo)
        .fold(disc.outer_edge().value(), f64::min);
    (1.0 - floor / a)
        .min(ceiling / a - 1.0)
        .clamp(0.0, MEMBER_ECCENTRICITY_CAP)
}

/// The quantile at `rank` of a Rayleigh law of width `width` truncated at `limit`.
fn truncated_rayleigh(width: f64, limit: f64, rank: UnitUniform) -> f64 {
    if limit <= 0.0 {
        return 0.0;
    }
    let cdf_limit = -math::exp_m1(-(limit * limit) / (2.0 * width * width));
    let p = rank.value() * cdf_limit;
    (width * (-2.0 * math::ln_1p(-p)).sqrt()).clamp(0.0, limit)
}

#[cfg(test)]
mod tests;
