//! Rings: every giant's tenuous dusty ring, and the occasional massive ring system like Saturn's
//! (plan 14, P14.T20).
//!
//! # What a giant's rings are
//!
//! - **A dusty ring**, on every giant ([`PlanetClass::GasGiant`] and [`PlanetClass::IceGiant`]):
//!   a tenuous sheet of silicate dust, of normal optical depth under 10⁻³, as every giant of the
//!   Solar System has ([`DUSTY_OPTICAL_DEPTH`]).
//! - **A massive ring**, with probability [`MASSIVE_ICY_RING_PROBABILITY`] (0.15) on a giant whose
//!   cloud tops are colder than [`ICY_RING_TEMPERATURE`] (170 K), of porous water ice, and with
//!   probability [`MASSIVE_ROCKY_RING_PROBABILITY`] (0.03) on a hotter one, of rock. Both
//!   probabilities are parameters of the generator version, because ring lifetimes are disputed
//!   ([`params`](crate::planetary::params)).
//!
//! A massive ring runs from [`RING_INNER_EDGE_RADII`] (1.1) planetary radii to a drawn fraction,
//! 0.6–1.0 ([`RING_OUTER_EDGE_ROCHE_FRACTION`]), of the planet's fluid Roche limit for its
//! material, and a dusty ring from the same inner edge to the same drawn fraction of the way from
//! it to the Roche limit for rock, so that every giant denser than 225 kg m⁻³ has one (the plan
//! gives no extent for a dusty ring; Jupiter's run from 1.4 to 1.81 Jupiter radii, and on out as
//! the gossamer rings, 0.9 of its Roche limit for rock and beyond, de Pater et al. 2017). The
//! materials: [`RingMaterial::PorousIce`] of 600 kg m⁻³, or [`RingMaterial::Rock`] of 2,500
//! kg m⁻³ ([`roche_limit_fluid`]). A ring whose outer edge would not lie beyond its inner one is
//! not there: about a giant of low density whose Roche limit for rock lies inside 1.1 planetary
//! radii no rocky ring forms, and inside 1.1 ÷ 0.6 = 1.83 radii only some draws leave a massive
//! one. A ring lies in its planet's equatorial plane, as every
//! satellite orbit of phase D is referred to its parent's equator; its radii are distances from
//! the planet's centre in that plane. A massive ring has a mass of 10⁻⁹–10⁻⁷ of its planet's
//! ([`MASSIVE_RING_MASS_RATIO`]), gaps at the 2:1 and 3:2 inner resonances of the regular moons
//! passed in that fall inside it, and an optical depth from its surface density.
//!
//! # The moons
//!
//! The regular moons (P14.T17) are placed before the rings (P14.T22.a's order), and come in here
//! as a plain slice of [`RingMoon`]s, their semi-major axes and masses: this module reads nothing
//! of `moons`, and P14.T22.a connects the two. Two rules read them:
//!
//! - **Gaps.** A moon of semi-major axis a clears a gap at its (p + 1):p inner resonance, at
//!   a (p ÷ (p + 1))^⅔ from the planet: 0.630 a for 2:1, where Mimas's clears Saturn's Cassini
//!   Division, and 0.763 a for 3:2. Only the innermost moons reach inside a ring; every
//!   resonance of every moon passed that falls inside a massive ring is recorded.
//! - **No moon over 10 km inside a massive ring.** A moon heavier than a 10 km sphere of the
//!   ring's own material ([`RingMaterial::moonlet_mass`]) lying inside the ring's drawn outer edge
//!   cuts the ring back to its orbit. So a moon of that size is never inside a massive ring,
//!   whichever order they are placed in.
//!
//! # Draws
//!
//! On [`tags::RING_SYSTEM`], keyed by the planet's [`BodyId`] (design note 4): word 0 the
//! massive ring's mark, word 1 the rank of its outer edge, word 2 the rank of its mass, word 3
//! the rank of the dusty ring's outer edge and word 4 the rank of its optical depth
//! ([`RingDraws`]); words 5–7 are reserved ([`RING_WORDS`]). A planet that is no giant draws
//! nothing.
//!
//! # Index
//!
//! A planet's rings are its sub-indices from `0x80` (design note 3): the dusty ring is
//! [`BodySub::Ring`]`(0)`, the massive ring, if any, `Ring(1)`. A free-floating object's rings sit
//! at the stellar level's `0x80` upward, in the same order, so the same function serves a rogue
//! planet (P14.T27.b).

use core::f64::consts::PI;
use std::error::Error;
use std::fmt;

use crate::id::BodyId;
use crate::math;
use crate::planetary::derive::{PlanetClass, roche_limit_fluid};
use crate::planetary::index::{BodyIndex, BodySub};
use crate::planetary::params::{MASSIVE_ICY_RING_PROBABILITY, MASSIVE_ROCKY_RING_PROBABILITY};
use crate::rng::{Mark, ObjectKey, Seed, Stream, Threshold, tags};
use crate::stellar::draws::UnitUniform;
use crate::units::{Kelvin, Kilograms, KilogramsPerCubicMetre, Metres};

/// Words of the [`tags::RING_SYSTEM`] stream that a planet reads or reserves: words 0–4 its
/// draws, 5–7 reserved.
pub const RING_WORDS: u64 = 8;

/// The cloud-top temperature below which a giant's massive ring is of ice: 170 K (P14.T20).
///
/// Plan 14's figure, the temperature at which water ice condenses from a protoplanetary disc's
/// gas (the snow line's; Hayashi 1981, and the circumplanetary ice line that P14.T17.b uses). A
/// giant colder than this keeps an icy ring against sublimation; a hotter one can hold only rock.
pub const ICY_RING_TEMPERATURE: Kelvin = Kelvin::new(170.0);

/// A ring's inner edge, in its planet's radii: 1.1 (P14.T20).
///
/// Plan 14's figure. Saturn's C ring begins at 1.24 Saturn radii (74,658 km) and its D ring at
/// 1.11; Jupiter's halo ring at 1.4 Jupiter radii (Burns et al. 2004, in "Jupiter: The Planet,
/// Satellites and Magnetosphere", Table 11.1).
pub const RING_INNER_EDGE_RADII: f64 = 1.1;

/// The range of the drawn fraction of the fluid Roche limit at which a ring ends: 0.6–1.0,
/// uniform (P14.T20).
///
/// Plan 14's figures. Saturn's A ring ends at 136,775 km, 0.91 of its fluid Roche limit for
/// porous ice of 600 kg m⁻³ (2.456 × 58,232 km × (687 ÷ 600)^⅓ = 149,600 km).
pub const RING_OUTER_EDGE_ROCHE_FRACTION: (f64, f64) = (0.6, 1.0);

/// The range of a massive ring's mass as a share of its planet's: 10⁻⁹–10⁻⁷, log-uniform
/// (P14.T20).
///
/// Plan 14's figures. Saturn's rings hold 1.54 ± 0.49 × 10¹⁹ kg (Iess et al. 2019, Science 364,
/// eaat2965, from Cassini's Grand Finale gravity field), 2.7 × 10⁻⁸ of Saturn's 5.683 × 10²⁶ kg.
pub const MASSIVE_RING_MASS_RATIO: (f64, f64) = (1e-9, 1e-7);

/// The range of a dusty ring's normal optical depth: 10⁻⁷–10⁻⁵, log-uniform (P14.T20).
///
/// Plan 14 asks only for "optical depth under 10⁻³". The range is this module's, from Jupiter's
/// rings, the type of a dusty ring: the halo and main rings' normal optical depths are a few ×
/// 10⁻⁶ and the Amalthea gossamer ring's about 10⁻⁷ (de Pater et al. 2017, arXiv:1707.00806,
/// Table 6.1; Throop et al. 2004, Icarus 172, 59, 4.7 × 10⁻⁶ in the main ring's dust).
pub const DUSTY_OPTICAL_DEPTH: (f64, f64) = (1e-7, 1e-5);

/// The radius of a dusty ring's grains, m: 15 µm, which sets the mass its optical depth implies
/// (τ = 3Σ ÷ 4ρs). This module's figure, of the order of the dust in Jupiter's main ring (Throop
/// et al. 2004), not re-checked against the paper; nothing but the dusty ring's mass reads it.
pub const DUST_GRAIN_RADIUS: Metres = Metres::new(15e-6);

/// The size distribution of a massive ring's particles, a differential power law n(s) ∝ s⁻³
/// between these radii, m: 1 cm to 5 m.
///
/// Zebker, Marouf and Tyler (1985, Icarus 64, 531) fit Voyager's radio occultation of Saturn's
/// rings with power laws of index 2.7–3.0 between radii of about 1 cm and 5 m; Cuzzi et al. (2009,
/// in "Saturn from Cassini–Huygens", ch. 15) review the same. The distribution fixes how much
/// optical depth a unit of surface density gives, [`RingMaterial::extinction`].
pub const RING_PARTICLE_RADII: (Metres, Metres) = (Metres::new(0.01), Metres::new(5.0));

/// The radius of the moonlet that no massive ring may contain: 10 km (P14.T20, "no moon over 10 km
/// is placed inside a massive ring").
pub const RING_MOONLET_RADIUS: Metres = Metres::new(10e3);

/// The inner resonances a moon clears gaps at, as (p + 1, p): 2:1 and 3:2 (P14.T20).
pub const GAP_RESONANCES: [(u8, u8); 2] = [(2, 1), (3, 2)];

/// What a ring is made of, which sets its density and so its Roche limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RingMaterial {
    /// Porous water ice of 600 kg m⁻³, the ring particles of Saturn (P14.T15's and T20's figure).
    /// Tiscareno (2013, "Planetary Rings", arXiv:1112.3305, eq. 1 and §1.2) finds Saturn's rings
    /// ending where the Roche critical density is about 400 kg m⁻³.
    PorousIce,
    /// Rock of 2,500 kg m⁻³, the dust of Jupiter's rings and a hot giant's massive ring (P14.T20).
    Rock,
}

impl RingMaterial {
    /// The material's bulk density: 600 kg m⁻³ for porous ice, 2,500 for rock.
    #[must_use]
    pub const fn density(self) -> KilogramsPerCubicMetre {
        match self {
            Self::PorousIce => KilogramsPerCubicMetre::new(600.0),
            Self::Rock => KilogramsPerCubicMetre::new(2_500.0),
        }
    }

    /// The mass of a sphere of [`RING_MOONLET_RADIUS`] (10 km) of this material: the lightest moon
    /// that no massive ring of it may contain.
    #[must_use]
    pub fn moonlet_mass(self) -> Kilograms {
        let r = RING_MOONLET_RADIUS.value();
        Kilograms::new(4.0 / 3.0 * PI * r * r * r * self.density().value())
    }

    /// The normal optical depth that a unit of surface density of a massive ring of this material
    /// gives, m² kg⁻¹: the extinction cross-section of its particles per unit mass.
    ///
    /// For particles of radius s from `s₁` to `s₂` with n(s) ∝ s⁻³ ([`RING_PARTICLE_RADII`]),
    /// τ ÷ Σ = ∫ πs² n ds ÷ ∫ (4⁄3)πs³ρ n ds = 3 ln(`s₂` ÷ `s₁`) ÷ (4ρ (`s₂` − `s₁`)): 1.56 × 10⁻³
    /// m² kg⁻¹ (0.016 cm² g⁻¹) for porous ice, inside the 0.01–0.02 cm² g⁻¹ of Saturn's A ring, so
    /// that a ring of Saturn's mass over its A to C rings has a mean τ of about 0.6. Hedman and
    /// Nicholson (2016, arXiv:1601.07955) measure 400–1,400 kg m⁻² where the B ring's τ is 1.5–5.
    #[must_use]
    pub fn extinction(self) -> f64 {
        let (s1, s2) = (RING_PARTICLE_RADII.0.value(), RING_PARTICLE_RADII.1.value());
        3.0 * math::ln(s2 / s1) / (4.0 * self.density().value() * (s2 - s1))
    }
}

/// Which of a giant's rings a ring is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RingKind {
    /// The tenuous dusty ring every giant has.
    Dusty,
    /// A massive ring system like Saturn's.
    Massive,
}

/// A giant as its rings read it (P14.T20): its index, class, mass, radius and cloud-top
/// temperature, all plain values.
///
/// P14.T22.a builds it from the planet's derivation (P14.T16.a's `DerivedBody`: its class,
/// radius and equilibrium temperature with internal heat, which stands for the temperature at its
/// cloud tops).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RingParent {
    index: BodyIndex,
    class: PlanetClass,
    mass: Kilograms,
    radius: Metres,
    cloud_top_temperature: Kelvin,
}

impl RingParent {
    /// The planet at `index` (a planet's sub-index 0, or a free-floating object's `0x0000`) of
    /// class `class`, mass `mass`, radius `radius` and cloud-top temperature
    /// `cloud_top_temperature`.
    ///
    /// # Errors
    ///
    /// - [`BuildRingParentError::NotAPlanet`] for an index that is not a planet's own
    ///   ([`BodySub::Primary`]).
    /// - [`BuildRingParentError::NotPositive`] for a mass, radius or temperature that is not
    ///   positive and finite.
    pub fn new(
        index: BodyIndex,
        class: PlanetClass,
        mass: Kilograms,
        radius: Metres,
        cloud_top_temperature: Kelvin,
    ) -> Result<Self, BuildRingParentError> {
        if index.sub() != BodySub::Primary {
            return Err(BuildRingParentError::NotAPlanet(index));
        }
        let positive = |x: f64| x.is_finite() && x > 0.0;
        if !(positive(mass.value())
            && positive(radius.value())
            && positive(cloud_top_temperature.value()))
        {
            return Err(BuildRingParentError::NotPositive);
        }
        Ok(Self {
            index,
            class,
            mass,
            radius,
            cloud_top_temperature,
        })
    }

    /// The planet's index.
    #[must_use]
    pub const fn index(&self) -> BodyIndex {
        self.index
    }

    /// Whether the planet is a giant, which alone has rings: a gas or an ice giant.
    #[must_use]
    pub const fn is_giant(&self) -> bool {
        match self.class {
            PlanetClass::GasGiant | PlanetClass::IceGiant => true,
            PlanetClass::Rocky | PlanetClass::Icy | PlanetClass::SubNeptune => false,
        }
    }

    /// The material a massive ring of this planet is made of: porous ice below
    /// [`ICY_RING_TEMPERATURE`], rock above it.
    #[must_use]
    pub fn massive_material(&self) -> RingMaterial {
        if self.cloud_top_temperature < ICY_RING_TEMPERATURE {
            RingMaterial::PorousIce
        } else {
            RingMaterial::Rock
        }
    }

    /// The planet's mean density.
    #[must_use]
    pub fn density(&self) -> KilogramsPerCubicMetre {
        let r = self.radius.value();
        KilogramsPerCubicMetre::new(self.mass.value() / (4.0 / 3.0 * PI * r * r * r))
    }

    /// The planet's fluid Roche limit for a body of `material` ([`roche_limit_fluid`]).
    #[must_use]
    pub fn roche_limit(&self, material: RingMaterial) -> Metres {
        roche_limit_fluid(self.radius, self.density(), material.density())
    }
}

/// A [`RingParent`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuildRingParentError {
    /// The index was not a planet's own.
    NotAPlanet(BodyIndex),
    /// A mass, radius or temperature was not positive and finite.
    NotPositive,
}

impl fmt::Display for BuildRingParentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAPlanet(index) => {
                write!(f, "body index {:#06x} is not a planet's own", index.get())
            }
            Self::NotPositive => {
                f.write_str("a ring parent's mass, radius and temperature must be positive")
            }
        }
    }
}

impl Error for BuildRingParentError {}

/// A regular moon as the rings read it: its semi-major axis about the planet and its mass
/// (P14.T17's output, passed in by P14.T22.a).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RingMoon {
    semi_major_axis: Metres,
    mass: Kilograms,
}

impl RingMoon {
    /// A moon of mass `mass` at semi-major axis `semi_major_axis` from its planet.
    ///
    /// # Panics
    ///
    /// In debug builds, unless both are positive and finite.
    #[must_use]
    pub fn new(semi_major_axis: Metres, mass: Kilograms) -> Self {
        debug_assert!(semi_major_axis.value().is_finite() && semi_major_axis.value() > 0.0);
        debug_assert!(mass.value().is_finite() && mass.value() > 0.0);
        Self {
            semi_major_axis,
            mass,
        }
    }

    /// The moon's semi-major axis about its planet.
    #[must_use]
    pub const fn semi_major_axis(&self) -> Metres {
        self.semi_major_axis
    }

    /// The moon's mass.
    #[must_use]
    pub const fn mass(&self) -> Kilograms {
        self.mass
    }

    /// Where the moon's (p + 1):p inner resonance lies from the planet: a (p ÷ (p + 1))^⅔.
    #[must_use]
    pub fn inner_resonance(&self, (outer, inner): (u8, u8)) -> Metres {
        let ratio = f64::from(inner) / f64::from(outer);
        Metres::new(self.semi_major_axis.value() * math::powf(ratio, 2.0 / 3.0))
    }
}

/// A gap in a ring, cleared by a moon's inner resonance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RingGap {
    moon: usize,
    resonance: (u8, u8),
    radius: Metres,
}

impl RingGap {
    /// The position in the slice of moons passed of the moon whose resonance this is.
    #[must_use]
    pub const fn moon(&self) -> usize {
        self.moon
    }

    /// The resonance, as (p + 1, p): (2, 1) or (3, 2).
    #[must_use]
    pub const fn resonance(&self) -> (u8, u8) {
        self.resonance
    }

    /// The gap's distance from the planet.
    #[must_use]
    pub const fn radius(&self) -> Metres {
        self.radius
    }
}

/// One ring of a giant (P14.T20).
#[derive(Debug, Clone, PartialEq)]
pub struct Ring {
    index: BodyIndex,
    kind: RingKind,
    material: RingMaterial,
    inner_edge: Metres,
    outer_edge: Metres,
    mass: Kilograms,
    optical_depth: f64,
    gaps: Vec<RingGap>,
}

impl Ring {
    /// The ring's body index, a sub-index of its planet's slot from `0x80`.
    #[must_use]
    pub const fn index(&self) -> BodyIndex {
        self.index
    }

    /// Which ring it is.
    #[must_use]
    pub const fn kind(&self) -> RingKind {
        self.kind
    }

    /// What it is made of.
    #[must_use]
    pub const fn material(&self) -> RingMaterial {
        self.material
    }

    /// Its inner edge, from the planet's centre in its equatorial plane.
    #[must_use]
    pub const fn inner_edge(&self) -> Metres {
        self.inner_edge
    }

    /// Its outer edge, from the planet's centre in its equatorial plane.
    #[must_use]
    pub const fn outer_edge(&self) -> Metres {
        self.outer_edge
    }

    /// Its mass: drawn for a massive ring, and for a dusty ring the dust its optical depth implies
    /// for grains of [`DUST_GRAIN_RADIUS`].
    #[must_use]
    pub const fn mass(&self) -> Kilograms {
        self.mass
    }

    /// Its mean normal optical depth.
    #[must_use]
    pub const fn optical_depth(&self) -> f64 {
        self.optical_depth
    }

    /// Its gaps, inside out (a massive ring's; a dusty ring has none).
    #[must_use]
    pub fn gaps(&self) -> &[RingGap] {
        &self.gaps
    }

    /// The ring's area in its plane, m².
    #[must_use]
    fn area(inner: Metres, outer: Metres) -> f64 {
        PI * (outer.value() * outer.value() - inner.value() * inner.value())
    }
}

/// A planet's draws on [`tags::RING_SYSTEM`] (module documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RingDraws {
    /// Whether a massive ring exists, against its probability. Word 0.
    pub massive: Mark,
    /// The rank of the massive ring's outer edge in its Roche-limit fraction. Word 1.
    pub massive_extent: UnitUniform,
    /// The rank of the massive ring's mass in its log-uniform range. Word 2.
    pub massive_mass: UnitUniform,
    /// The rank of the dusty ring's outer edge. Word 3.
    pub dusty_extent: UnitUniform,
    /// The rank of the dusty ring's optical depth in its log-uniform range. Word 4.
    pub dusty_depth: UnitUniform,
}

impl RingDraws {
    /// The draws of the planet `planet` in the universe of `seed`.
    ///
    /// # Panics
    ///
    /// Never: an open uniform lies strictly inside (0, 1).
    #[must_use]
    pub fn for_planet(seed: Seed, planet: BodyId) -> Self {
        let mut stream = Stream::open(seed, tags::RING_SYSTEM, ObjectKey::from(planet));
        let massive = Mark::from_word(stream.next_u64());
        let mut rank =
            || UnitUniform::new(stream.uniform_open()).expect("an open uniform is inside (0, 1)");
        Self {
            massive,
            massive_extent: rank(),
            massive_mass: rank(),
            dusty_extent: rank(),
            dusty_depth: rank(),
        }
    }
}

/// `lo` + (`hi` − `lo`) × `rank`.
#[must_use]
fn linear((lo, hi): (f64, f64), rank: UnitUniform) -> f64 {
    lo + (hi - lo) * rank.value()
}

/// `lo` × (`hi` ÷ `lo`)^`rank`: log-uniform between `lo` and `hi`.
#[must_use]
fn log_uniform((lo, hi): (f64, f64), rank: UnitUniform) -> f64 {
    lo * math::powf(hi / lo, rank.value())
}

/// The rings of `planet`, whose regular moons are `moons`, from the draws `draws` (P14.T20): a
/// dusty ring for every giant, then a massive ring where one exists, in index order. Nothing for a
/// planet that is not a giant.
///
/// # Panics
///
/// Never: a planet's slot holds rings 0 and 1, and a [`RingParent`] is a planet's own index.
///
/// # Examples
///
/// A Saturn with its median draws but a massive ring: an icy ring inside its Roche limit for
/// porous ice, with Mimas's 2:1 resonance, the Cassini Division, inside it.
///
/// ```
/// use hyperion_sim::planetary::derive::PlanetClass;
/// use hyperion_sim::planetary::rings::{RingDraws, RingKind, RingMaterial, RingMoon, RingParent, rings};
/// use hyperion_sim::planetary::{BodyIndex, BodySlot, BodySub};
/// use hyperion_sim::rng::Mark;
/// use hyperion_sim::stellar::draws::UnitUniform;
/// use hyperion_sim::units::{Kelvin, Kilograms, Metres};
///
/// let index = BodyIndex::new(BodySlot::Planet(6), BodySub::Primary)?;
/// let saturn = RingParent::new(
///     index,
///     PlanetClass::GasGiant,
///     Kilograms::new(5.683e26),
///     Metres::new(58_232e3),
///     Kelvin::new(95.0),
/// )?;
/// let mimas = RingMoon::new(Metres::new(185_539e3), Kilograms::new(3.75e19));
/// let draws = RingDraws {
///     massive: Mark::from_word(0),
///     massive_extent: UnitUniform::HALF,
///     massive_mass: UnitUniform::HALF,
///     dusty_extent: UnitUniform::HALF,
///     dusty_depth: UnitUniform::HALF,
/// };
/// let rings = rings(&saturn, &[mimas], &draws);
/// let massive = rings.iter().find(|r| r.kind() == RingKind::Massive).expect("a massive ring");
/// assert_eq!(massive.material(), RingMaterial::PorousIce);
/// assert!(massive.outer_edge() <= saturn.roche_limit(RingMaterial::PorousIce));
/// let cassini = massive.gaps()[0].radius().value();
/// assert!((cassini - 116_900e3).abs() < 1_000e3);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn rings(planet: &RingParent, moons: &[RingMoon], draws: &RingDraws) -> Vec<Ring> {
    if !planet.is_giant() {
        return Vec::new();
    }
    let inner = planet.radius * RING_INNER_EDGE_RADII;
    let slot = planet.index.slot();
    let index = |n: u8| {
        BodyIndex::new(slot, BodySub::Ring(n)).expect("a planet's slot holds rings 0 and 1")
    };
    let mut out = Vec::with_capacity(2);

    let dust = RingMaterial::Rock;
    let dusty_outer = inner
        + (planet.roche_limit(dust) - inner)
            * linear(RING_OUTER_EDGE_ROCHE_FRACTION, draws.dusty_extent);
    if dusty_outer > inner {
        let tau = log_uniform(DUSTY_OPTICAL_DEPTH, draws.dusty_depth);
        // τ = 3Σ ÷ 4ρs for grains of one radius s.
        let sigma = tau * 4.0 * dust.density().value() * DUST_GRAIN_RADIUS.value() / 3.0;
        out.push(Ring {
            index: index(0),
            kind: RingKind::Dusty,
            material: dust,
            inner_edge: inner,
            outer_edge: dusty_outer,
            mass: Kilograms::new(sigma * Ring::area(inner, dusty_outer)),
            optical_depth: tau,
            gaps: Vec::new(),
        });
    }

    let material = planet.massive_material();
    let probability = match material {
        RingMaterial::PorousIce => MASSIVE_ICY_RING_PROBABILITY,
        RingMaterial::Rock => MASSIVE_ROCKY_RING_PROBABILITY,
    };
    if !draws
        .massive
        .is_below(Threshold::from_probability(probability))
    {
        return out;
    }
    let drawn_edge =
        planet.roche_limit(material) * linear(RING_OUTER_EDGE_ROCHE_FRACTION, draws.massive_extent);
    let moonlet = material.moonlet_mass();
    let outer = moons
        .iter()
        .filter(|moon| moon.mass >= moonlet)
        .map(|moon| moon.semi_major_axis)
        .fold(drawn_edge, |edge, a| if a < edge { a } else { edge });
    if outer <= inner {
        return out;
    }
    let mass = planet.mass * log_uniform(MASSIVE_RING_MASS_RATIO, draws.massive_mass);
    let mut gaps: Vec<RingGap> = moons
        .iter()
        .enumerate()
        .flat_map(|(n, moon)| {
            GAP_RESONANCES.iter().map(move |&resonance| RingGap {
                moon: n,
                resonance,
                radius: moon.inner_resonance(resonance),
            })
        })
        .filter(|gap| inner < gap.radius && gap.radius < outer)
        .collect();
    gaps.sort_by(|a, b| {
        a.radius
            .value()
            .total_cmp(&b.radius.value())
            .then(a.moon.cmp(&b.moon))
    });
    let sigma = mass.value() / Ring::area(inner, outer);
    out.push(Ring {
        index: index(1),
        kind: RingKind::Massive,
        material,
        inner_edge: inner,
        outer_edge: outer,
        mass,
        optical_depth: sigma * material.extinction(),
        gaps,
    });
    out
}

/// The rings of `planet`, whose regular moons are `moons`, in the universe of `seed`: [`rings`]
/// of the planet's own draws ([`RingDraws::for_planet`]), where `system` is the planet's system.
#[must_use]
pub fn generate_rings(
    seed: Seed,
    system: crate::id::SystemId,
    planet: &RingParent,
    moons: &[RingMoon],
) -> Vec<Ring> {
    if !planet.is_giant() {
        return Vec::new();
    }
    let draws = RingDraws::for_planet(seed, planet.index.body_id(system));
    rings(planet, moons, &draws)
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::stats::{ALPHA, assert_poisson_count};

    use super::*;
    use crate::id::SystemId;
    use crate::planetary::index::BodySlot;
    use crate::units::consts::{EARTH_MASS_KG, JUPITER_MASS_KG};

    const SEED: Seed = Seed::new(0x0005_a7e2);

    fn planet(slot: u8, class: PlanetClass, mass: f64, radius_km: f64, t: f64) -> RingParent {
        let index = BodyIndex::new(BodySlot::Planet(slot), BodySub::Primary).unwrap();
        RingParent::new(
            index,
            class,
            Kilograms::new(mass),
            Metres::new(radius_km * 1e3),
            Kelvin::new(t),
        )
        .unwrap()
    }

    fn saturn() -> RingParent {
        planet(6, PlanetClass::GasGiant, 5.683e26, 58_232.0, 95.0)
    }

    fn system(n: u64) -> SystemId {
        SystemId::from_raw(0x0200_0800_2000_0000 + (n << 8)).unwrap()
    }

    fn draws_with(massive: bool) -> RingDraws {
        RingDraws {
            massive: Mark::from_word(if massive { 0 } else { u64::MAX }),
            massive_extent: UnitUniform::HALF,
            massive_mass: UnitUniform::HALF,
            dusty_extent: UnitUniform::HALF,
            dusty_depth: UnitUniform::HALF,
        }
    }

    /// A sample of giants of 10 M⊕ to 13 M♃, of densities 200–3,000 kg m⁻³ and cloud tops of
    /// 50–2,000 K, each with two regular moons, from a fixed generator.
    fn sampled_giants(n: u64) -> Vec<(SystemId, RingParent, Vec<RingMoon>)> {
        let mut lcg = hyperion_testkit::lcg::Lcg::new(0x51_7e5);
        (0..n)
            .map(|i| {
                let mass = 10.0
                    * EARTH_MASS_KG
                    * math::powf(
                        13.0 * JUPITER_MASS_KG / (10.0 * EARTH_MASS_KG),
                        lcg.next_f64(),
                    );
                let density = 200.0 * math::powf(15.0, lcg.next_f64());
                let radius = math::cbrt(mass / (4.0 / 3.0 * PI * density));
                let t = 50.0 * math::powf(40.0, lcg.next_f64());
                let class = if mass > 50.0 * EARTH_MASS_KG {
                    PlanetClass::GasGiant
                } else {
                    PlanetClass::IceGiant
                };
                let slot = 1 + u8::try_from(i % 8).unwrap();
                let index = BodyIndex::new(BodySlot::Planet(slot), BodySub::Primary).unwrap();
                let parent = RingParent::new(
                    index,
                    class,
                    Kilograms::new(mass),
                    Metres::new(radius),
                    Kelvin::new(t),
                )
                .unwrap();
                let moons = (0..2)
                    .map(|_| {
                        RingMoon::new(
                            Metres::new(radius * (1.5 + 8.0 * lcg.next_f64())),
                            Kilograms::new(mass * 1e-9 * math::powf(1e5, lcg.next_f64())),
                        )
                    })
                    .collect();
                (system(i), parent, moons)
            })
            .collect()
    }

    /// P14.T20: rings are a giant's alone.
    #[test]
    fn a_planet_that_is_no_giant_has_no_rings() {
        for class in [
            PlanetClass::Rocky,
            PlanetClass::Icy,
            PlanetClass::SubNeptune,
        ] {
            let body = planet(3, class, 5.97e24, 6_371.0, 255.0);
            assert!(rings(&body, &[], &draws_with(true)).is_empty());
            assert!(generate_rings(SEED, system(1), &body, &[]).is_empty());
        }
    }

    /// P14.T20: every giant has a tenuous dusty ring, `Ring(0)`, of optical depth under 10⁻³.
    #[test]
    fn every_giant_has_a_tenuous_dusty_ring() {
        for (id, parent, moons) in sampled_giants(2_000) {
            let rings = generate_rings(SEED, id, &parent, &moons);
            let dusty = rings.iter().find(|r| r.kind() == RingKind::Dusty);
            // A dusty ring is missing only where the Roche limit for rock lies inside its edge.
            let reach = parent.roche_limit(RingMaterial::Rock);
            if reach > parent.radius * RING_INNER_EDGE_RADII {
                let dusty = dusty.unwrap_or_else(|| panic!("no dusty ring about {parent:?}"));
                assert!(dusty.optical_depth() < 1e-3 && dusty.optical_depth() > 0.0);
                assert_eq!(dusty.index().sub(), BodySub::Ring(0));
                assert_eq!(dusty.index().parent(), Some(parent.index()));
                assert!(dusty.gaps().is_empty());
            }
        }
    }

    /// P14.T20, **rings inside Roche limits**: every sampled ring ends inside the fluid Roche
    /// limit for its material and begins outside its planet; no heavy moon lies inside a massive
    /// ring, and every gap lies inside its ring.
    #[test]
    fn rings_lie_inside_roche_limits() {
        let mut massive = 0;
        for (id, parent, moons) in sampled_giants(4_000) {
            for ring in generate_rings(SEED, id, &parent, &moons) {
                assert!(
                    ring.outer_edge() <= parent.roche_limit(ring.material()),
                    "{ring:?}"
                );
                assert!(ring.inner_edge() >= parent.radius, "{ring:?}");
                assert!(ring.inner_edge() < ring.outer_edge());
                for gap in ring.gaps() {
                    assert!(ring.inner_edge() < gap.radius() && gap.radius() < ring.outer_edge());
                }
                if ring.kind() == RingKind::Massive {
                    massive += 1;
                    let ratio = ring.mass().value() / parent.mass.value();
                    assert!((1e-9..=1e-7).contains(&ratio), "{ratio}");
                    for moon in &moons {
                        if moon.mass() >= ring.material().moonlet_mass() {
                            assert!(moon.semi_major_axis() >= ring.outer_edge());
                        }
                    }
                }
            }
        }
        assert!(massive > 100, "only {massive} massive rings");
    }

    /// P14.T20: about 15% of cold giants have massive icy rings, and 3% of hot ones rocky rings
    /// (Poisson intervals).
    #[test]
    fn about_fifteen_percent_of_cold_giants_have_massive_rings() {
        let n = 4_000_u64;
        for (t, material, p) in [
            (95.0, RingMaterial::PorousIce, MASSIVE_ICY_RING_PROBABILITY),
            (900.0, RingMaterial::Rock, MASSIVE_ROCKY_RING_PROBABILITY),
        ] {
            let count = (0..n)
                .filter(|&i| {
                    let giant = planet(5, PlanetClass::GasGiant, 1.898e27, 69_911.0, t);
                    generate_rings(SEED, system(i), &giant, &[])
                        .iter()
                        .any(|r| r.kind() == RingKind::Massive && r.material() == material)
                })
                .count();
            #[expect(clippy::cast_precision_loss, reason = "a count of 4,000")]
            let mean = p * n as f64;
            assert_poisson_count("massive rings", u64::try_from(count).unwrap(), mean, ALPHA);
        }
    }

    /// P14.T20: Saturn with a massive ring has Mimas's 2:1 gap, the Cassini Division, near
    /// 117,000 km, inside a ring that ends inside its Roche limit for porous ice, and an optical
    /// depth of the order of its rings' (0.1–5).
    #[test]
    fn saturn_s_ring_has_the_cassini_division() {
        let mimas = RingMoon::new(Metres::new(185_539e3), Kilograms::new(3.75e19));
        let enceladus = RingMoon::new(Metres::new(238_042e3), Kilograms::new(1.08e20));
        let rings = rings(&saturn(), &[mimas, enceladus], &draws_with(true));
        let ring = rings
            .iter()
            .find(|r| r.kind() == RingKind::Massive)
            .unwrap();
        assert_eq!(ring.material(), RingMaterial::PorousIce);
        assert_eq!(ring.index().sub(), BodySub::Ring(1));
        let roche = saturn().roche_limit(RingMaterial::PorousIce).value();
        assert!((roche - 149_600e3).abs() < 1_000e3, "{roche}");
        let cassini = ring.gaps()[0];
        assert_eq!((cassini.moon(), cassini.resonance()), (0, (2, 1)));
        assert!((cassini.radius().value() - 116_882e3).abs() < 100e3);
        assert!(
            (0.1..5.0).contains(&ring.optical_depth()),
            "{}",
            ring.optical_depth()
        );
    }

    /// P14.T20: a moon over 10 km inside a massive ring's drawn edge cuts it back to its orbit; a
    /// smaller one does not, and clears only its gaps.
    #[test]
    fn no_moon_over_ten_kilometres_lies_inside_a_massive_ring() {
        let heavy = RingMoon::new(Metres::new(110_000e3), Kilograms::new(1e17));
        let light = RingMoon::new(Metres::new(110_000e3), Kilograms::new(1e14));
        let cut = rings(&saturn(), &[heavy], &draws_with(true));
        let kept = rings(&saturn(), &[light], &draws_with(true));
        let edge = |rings: &[Ring]| {
            rings
                .iter()
                .find(|r| r.kind() == RingKind::Massive)
                .unwrap()
                .outer_edge()
        };
        assert_eq!(edge(&cut), heavy.semi_major_axis());
        assert!(edge(&kept) > light.semi_major_axis());
        assert!(RingMaterial::PorousIce.moonlet_mass() > light.mass());
    }

    /// P14.T20: a giant's massive ring is of ice below 170 K at its cloud tops and of rock above.
    #[test]
    fn a_massive_ring_is_icy_when_cold_and_rocky_when_hot() {
        let cold = planet(5, PlanetClass::GasGiant, 1.898e27, 69_911.0, 169.0);
        let hot = planet(5, PlanetClass::GasGiant, 1.898e27, 69_911.0, 170.0);
        assert_eq!(cold.massive_material(), RingMaterial::PorousIce);
        assert_eq!(hot.massive_material(), RingMaterial::Rock);
        let rocky = rings(&hot, &[], &draws_with(true));
        assert_eq!(rocky.last().unwrap().material(), RingMaterial::Rock);
    }

    /// Design note 4: a planet's ring draws are words 0–4 of its own `ring.system` stream, and
    /// two calls agree.
    #[test]
    fn ring_draws_are_the_planet_s_own_words() {
        let body = saturn().index().body_id(system(3));
        let draws = RingDraws::for_planet(SEED, body);
        let stream = Stream::open(SEED, tags::RING_SYSTEM, ObjectKey::from(body));
        assert_eq!(draws.massive, Mark::from_word(stream.word_at(0)));
        assert_eq!(draws, RingDraws::for_planet(SEED, body));
        let other = RingDraws::for_planet(SEED, saturn().index().body_id(system(4)));
        assert_ne!(draws, other);
        assert_eq!(
            generate_rings(SEED, system(3), &saturn(), &[]),
            generate_rings(SEED, system(3), &saturn(), &[])
        );
    }

    /// A ring parent refuses what is not a planet's own index, and a mass that is not positive.
    #[test]
    fn a_ring_parent_is_a_planet() {
        let moon = BodyIndex::new(BodySlot::Planet(2), BodySub::Moon(1)).unwrap();
        let build = |index, mass| {
            RingParent::new(
                index,
                PlanetClass::GasGiant,
                Kilograms::new(mass),
                Metres::new(7e7),
                Kelvin::new(100.0),
            )
        };
        assert_eq!(
            build(moon, 1e27),
            Err(BuildRingParentError::NotAPlanet(moon))
        );
        assert_eq!(
            build(saturn().index(), -1.0),
            Err(BuildRingParentError::NotPositive)
        );
        assert_eq!(
            BuildRingParentError::NotPositive.to_string(),
            "a ring parent's mass, radius and temperature must be positive"
        );
    }
}
