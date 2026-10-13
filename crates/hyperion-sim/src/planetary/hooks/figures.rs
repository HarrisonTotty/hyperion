//! A body's global figures (plan 14, P14.T24.b, with P14.T48's inputs to the coarse surface):
//! what the surface generator reads of a body's whole surface.
//!
//! Built so far (P14.T48.f): [`impactors`], what strikes a body and what it strikes, which
//! P14.T48.c's crater contract carries beside the crater density
//! ([`CraterParams`](hyperion_surface::craters::CraterParams)). T24.b's `GlobalFigures` and T48's
//! other figures join this module with their tasks.
//!
//! # The impactors (P14.T48.f)
//!
//! A closed form with no draws, from the system the generator made (science-r09-craters.md items 2
//! and 4, the ruling plan 14's task text follows).
//!
//! - **The mean impact velocity.** For a planet, a dwarf planet or a belt's member,
//!   v² = v∞² + `v_esc`²; for a moon, v² = v∞,P² + 3 `v_K,s`² + `v_esc,s`², where v∞,P is its
//!   parent's encounter speed with the impactors, `v_K,s` its orbital speed about its parent and
//!   `v_esc,s` its own escape speed. Two of the three `v_K,s`² are the square of the parent's
//!   escape speed at the moon's orbit, 2 `v_K`² = `v_esc`² (Brasser, Wong and Werner 2025, A&A
//!   695, A276, arXiv 2501.17509, §2.2, after its eq. 2), and the moon's own motion makes the
//!   mean encounter speed about √3 `v_K` (its §2.3, after Zahnle et al. 2003, Icarus 163, 263, not
//!   read); science-r09-craters.md item 4 assembles the sum. The escape speed is √(2 g R), the
//!   bulk section's gravity and radius.
//! - **The encounter speed** is v∞ = U `v_K`(a), with `v_K` = √(G M ÷ a) the circular speed about
//!   the host at the body's (or its parent's) semi-major axis, and U² = `U_src`² + e² + sin² i,
//!   which adds the target's own eccentricity and inclination to its host's plane to the
//!   impactors' own excitation `U_src`² (Öpik, with random phases).
//! - **The impactors' excitation `U_src`²**, by the first rule that applies ([`ImpactorSource`]):
//!   1. _In a belt_ (a belt's member, or an orbit inside one of a belt's annuli, edges included):
//!      that component's own excitation, ⟨e²⟩ + ⟨sin² i⟩ = 2σₑ² + 2 sin² σᵢ from the Rayleigh
//!      widths of plan 14's belts ([`belt_excitation`]): 0.0587 for a belt proper and 0.414 for a
//!      scattered component.
//!   2. _A giant_ (at least [`IMPACTOR_GIANT_MASS`]), or any body at or beyond the innermost
//!      giant's orbit, and their moons: the giants' scattered comets, [`COMET_EXCITATION`]² =
//!      0.30².
//!   3. _Otherwise_, inside the innermost giant or with no giant: the nearest belt of
//!      [`BeltKind::Asteroid`], or failing one the nearest of any kind, by `U_b`² + (1 − a ÷ `a_b`)³
//!      inside it and `U_b`² + (1 − `a_b` ÷ a)³ outside, with `U_b`² and `a_b` its belt proper's
//!      excitation and mid-radius. The cube is the plan's own calibrated form: the minimal
//!      crossing orbit's excess speed grows linearly in the fractional distance, and the real, more
//!      excited crossers faster. With no belt but a giant, [`COMET_EXCITATION`]² + (1 − a ÷ `a_g`)³
//!      from the innermost giant's `a_g`; with neither, [`COMET_EXCITATION`]². The nearest belt is
//!      the one of the least ratio max(a, `a_b`) ÷ min(a, `a_b`), the first in the given order on a
//!      tie; the cube is continuous at `a_b` but not at a belt's edges, where rule 1 takes over.
//! - **The projectiles' density**: [`ROCKY_PROJECTILE_DENSITY`] from a rocky belt,
//!   [`ICY_PROJECTILE_DENSITY`] from an icy belt or the comets.
//! - **The target** ([`Crust`]): porous below a radius of [`POROUS_TARGET_RADIUS`], at the body's
//!   bulk density; otherwise non-porous, at ice Ih's density at its mean surface temperature for
//!   an ice-rich crust or [`SILICATE_TARGET_DENSITY`] for a silicate one. `k_target` is the
//!   crust's, [`ICE_K_TARGET`] or [`SILICATE_K_TARGET`], whatever the class. A crust is ice-rich
//!   ([`Crust::of`]) for a body colder than [`ICE_RICH_TEMPERATURE`] that is a moon formed beyond
//!   its parent's ice line, a member of an icy belt, or a body whose water would make a global
//!   layer at least [`ICE_RICH_LAYER_DEPTH`] deep ([`ice_layer_depth`]).
//!
//! Against the Solar System (this module's tests, on JPL's elements and masses) it gives Mercury
//! 41.4, Venus 25.7, Earth 19.9, the Moon 16.7, Mars 10.55 and Ceres 5.65 km/s, within 0.1 km/s
//! of the ruling's own figures; Mars ÷ the Moon 0.632 (Ivanov 2001's 0.60, Bottke's and Le Feuvre
//! and Wieczorek's 0.53–0.54); the 15 regular moons of Jupiter, Saturn and Uranus within −6.3% and
//! +7.4% of Brasser et al. 2025's Table 5; and Pluto 2.48 km/s against Singer et al. 2019's 2.2.
//!
//! Not modelled (science-r09-craters.md item 4): comets striking the inner planets, a minority of
//! their crater-forming impacts (Ivanov et al. 2002 conclude main-belt asteroids made most of
//! them), and the velocity's spread: the diameter map takes the mean.
//!
//! # Not on the record yet
//!
//! Nothing on a body's record calls [`impactors`] until P14.T48.c's crater contract, built after
//! it in the 21 → 22 batch, whose goldens move once, at the batch's bump. It draws nothing and
//! moves no generated output, so it needs no hold.

use core::fmt;
use std::error::Error;

use hyperion_surface::craters::{self, CraterTarget, TargetClass};

use crate::math;
use crate::planetary::belts::{
    BELT_GIANT_MASS, Belt, BeltComposition, BeltPart, MEMBER_ECCENTRICITY_WIDTH,
    MEMBER_INCLINATION_WIDTH_DEG, SCATTERED_ECCENTRICITY_WIDTH, SCATTERED_INCLINATION_WIDTH_DEG,
};
use crate::planetary::derive::SnowLineSide;
use crate::planetary::record::BeltKind;
use crate::substance;
use crate::units::{
    EarthMasses, GravitationalParameter, Kelvin, Kilograms, KilogramsPerCubicMetre, Metres,
    MetresPerSecond, MetresPerSecondSquared, Radians,
};

#[cfg(test)]
mod tests;

/// The mass from which a planet is one of the impactors' giants, whose scattered comets strike it
/// and every body at or beyond the innermost one: [`BELT_GIANT_MASS`], 10 M⊕, the giants the
/// belts are placed by (science-r09-craters.md item 4: "a giant (≥ 10 M⊕)").
pub const IMPACTOR_GIANT_MASS: EarthMasses = BELT_GIANT_MASS;

/// The giants' scattered comets' encounter speed in units of the target's circular speed:
/// `U_c` = 0.30 (science-r09-craters.md item 4).
///
/// Brasser, Wong and Werner 2025 (A&A 695, A276, §2.3) find the comets' mean v∞ at Jupiter,
/// Saturn and Uranus 3.17, 3.65 and 2.39 km/s, after Vokrouhlický et al. 2008 and Nesvorný et al.
/// 2017: 0.24, 0.38 and 0.35 of the planets' orbital speeds (their §2.2 quotes 0.27–0.38 from
/// their migration runs). Nesvorný et al. 2023 (PSJ 4, 139, §3.2.1) find 4.1 km/s for the bodies
/// that strike Jupiter, "approximately 30%" of its orbital speed.
pub const COMET_EXCITATION: f64 = 0.30;

/// The density of a rocky belt's projectiles, kg m⁻³: 3,000, Design note 12's stony projectile
/// and the diameter map's reference ([`craters::LUNAR_PROJECTILE_DENSITY`]; science-r09-craters.md
/// item 2), which the ruling keeps over the 2,700 kg m⁻³ plan 14 gives a rocky belt's members
/// ([`ROCKY_MEMBER_DENSITY`](crate::planetary::belts::ROCKY_MEMBER_DENSITY), the cascade's).
pub const ROCKY_PROJECTILE_DENSITY: KilogramsPerCubicMetre = craters::LUNAR_PROJECTILE_DENSITY;

/// The density of an icy belt's projectiles and of the comets, kg m⁻³: 500, Britt, Consolmagno and
/// Merline 2006's nominal cometary bulk density (LPSC 37, 2214).
///
/// It lies inside Groussin et al. 2019's mean of 20 estimates, 480 ± 220 kg m⁻³ (Space Sci. Rev.
/// 215, 29, Table 1), between 9P/Tempel 1's 400 (200–1,000, Richardson et al. 2007, Icarus 190,
/// 357) or 470 (Thomas et al. 2013) and 67P's 532 ± 7 (Jorda et al. 2016, Icarus 277, 257), the
/// one precise measurement, and under the Jupiter-family comets' spin barrier of about 600 (Lowry
/// and Weissman 2003, via Groussin et al.). Brasser, Wong and Werner 2025 take 400 after Wong et
/// al. 2023, and Zahnle et al. 2003 took 600; 400 would scale a crater by (400 ÷ 500)^0.261 =
/// 0.943 (science-r09-craters.md F5.2).
pub const ICY_PROJECTILE_DENSITY: KilogramsPerCubicMetre = KilogramsPerCubicMetre::new(500.0);

/// The radius below which a body is a porous crater target, m: 150 km, the plan's own threshold
/// between Hyperion (135 km, 544 kg m⁻³, over 40% porous; Thomas et al. 2007, Nature 448, 50),
/// which is porous, and Mimas (198 km), which is not; the small Saturnian moons lie below it, at
/// 400–650 kg m⁻³ as science-r09-craters.md item 2 recalls them, unverified.
pub const POROUS_TARGET_RADIUS: Metres = Metres::new(150e3);

/// The density of a non-porous silicate crust as a crater target, kg m⁻³: 2,550, the lunar
/// highlands crust's bulk density from GRAIL (Wieczorek et al. 2013, Science 339, 671), the
/// diameter map's reference target ([`craters::LUNAR_TARGET`]; science-r09-craters.md item 2).
pub const SILICATE_TARGET_DENSITY: KilogramsPerCubicMetre = craters::LUNAR_TARGET.density;

/// The target factor `k_target` of a silicate crust, which scales the simple-to-complex
/// transition: 1 (R09's Design note 12).
pub const SILICATE_K_TARGET: f64 = 1.0;

/// The target factor `k_target` of an ice-rich crust: 0.12.
///
/// The morphological transitions of nine icy bodies give a geometric mean of 0.125 and a range of
/// 0.06–0.26 (Ganymede's 2.1 ± 0.2 km, Schenk et al. 2004; Rhea's 12 ± 2 km, Aponte-Hernández et
/// al. 2021), with no resolvable temperature term between 40 and 150 K; Ceres's mixed crust
/// behaves as ice (science-r09-craters.md item 1).
pub const ICE_K_TARGET: f64 = 0.12;

/// The mean surface temperature below which a body's crust can be ice-rich: 273 K, water's
/// melting point, whatever the body's birthplace; at or above it its ice melts or, airless,
/// sublimates within years (science-r09-craters.md item 2 and F5.4).
pub const ICE_RICH_TEMPERATURE: Kelvin = Kelvin::new(273.0);

/// The depth of global water layer from which a cold body's crust is ice-rich, m: 10 km, the
/// plan's own figure, deeper than any crater below the transition excavates; Europa's 80–170 km
/// and Ceres's 41 km crust lie far above it, as science-r09-craters.md item 2 states them.
pub const ICE_RICH_LAYER_DEPTH: Metres = Metres::new(10e3);

/// The density a body's water is spread at to make its global layer, kg m⁻³: 1,000, liquid
/// water's, as a global equivalent layer is stated (this module's choice: ice's 917–934 would
/// deepen the layer by 7–9%).
pub const WATER_LAYER_DENSITY: KilogramsPerCubicMetre = KilogramsPerCubicMetre::new(1_000.0);

/// A body's orbit about its host as its impactors read it.
///
/// Plain data with public fields: [`impactors`] validates them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostOrbit {
    /// The semi-major axis about the host, m; finite and positive.
    pub semi_major_axis: Metres,
    /// The eccentricity, in `[0, 1)`.
    pub eccentricity: f64,
    /// The inclination to the host's plane, the mean plane of its belts and planets, rad; finite.
    ///
    /// Not the system frame's: a host's plane is tilted against the galactic axes.
    pub inclination: Radians,
}

/// Which belt a body is a named member of, as its impactors read it (P14.T21.c).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BeltMembership {
    /// The part of its belt it orbits in, whose excitation it shares.
    pub part: BeltPart,
    /// Its belt's composition class, which sets its projectiles' density and, if icy, makes its
    /// crust ice-rich.
    pub composition: BeltComposition,
}

/// A body that goes round the host itself: a planet, a dwarf planet or a belt's member.
///
/// Plain data with public fields: [`impactors`] validates them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostOrbiter {
    /// Its orbit about the host.
    pub orbit: HostOrbit,
    /// Its mass, finite and non-negative (positive for a moon's parent), which says whether it is
    /// a giant ([`IMPACTOR_GIANT_MASS`]) and, for a moon's parent, sets the moon's orbital speed.
    pub mass: EarthMasses,
    /// The belt it is a named member of, if any; `None` for a planet or a body that is no
    /// member.
    pub belt: Option<BeltMembership>,
}

/// Where a body goes round, as its impactors read it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImpactorOrbit {
    /// The body goes round the host itself.
    Host(HostOrbiter),
    /// The body is a moon of `parent`.
    Moon {
        /// Its parent, which goes round the host: the impactors are the parent's.
        parent: HostOrbiter,
        /// Its semi-major axis about its parent, m; finite and positive.
        semi_major_axis: Metres,
        /// Which side of its parent's ice line it formed on: beyond it for a moon with ice
        /// ([`DerivedMoon::ice_fraction`](crate::planetary::moons::DerivedMoon::ice_fraction)
        /// above zero), whose crust is ice-rich.
        formed: SnowLineSide,
    },
}

/// One annulus of a source belt, its edges from the host.
///
/// Plain data with public fields: [`ImpactorSources::new`] validates them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceAnnulus {
    /// The inner edge, m; finite and positive.
    pub inner_edge: Metres,
    /// The outer edge, m; finite and no less than the inner edge.
    pub outer_edge: Metres,
}

impl SourceAnnulus {
    /// Its mid-radius, the midpoint of its edges.
    #[must_use]
    pub fn mid_radius(&self) -> Metres {
        Metres::new(f64::midpoint(
            self.inner_edge.value(),
            self.outer_edge.value(),
        ))
    }

    /// Whether a semi-major axis `a` lies inside it, edges included.
    #[must_use]
    fn holds(&self, a: Metres) -> bool {
        self.inner_edge <= a && a <= self.outer_edge
    }
}

/// A belt as the impactors read it: its kind, its composition and its annuli.
///
/// Plain data with public fields: [`ImpactorSources::new`] validates them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceBelt {
    /// Its kind: an asteroid belt is the inner bodies' source before any other (rule 3).
    pub kind: BeltKind,
    /// Its composition class, which sets its projectiles' density.
    pub composition: BeltComposition,
    /// The belt proper.
    pub main: SourceAnnulus,
    /// Its scattered component, if it has one.
    pub scattered: Option<SourceAnnulus>,
}

impl From<&Belt> for SourceBelt {
    fn from(belt: &Belt) -> Self {
        let annulus = |inner_edge, outer_edge| SourceAnnulus {
            inner_edge,
            outer_edge,
        };
        Self {
            kind: belt.kind(),
            composition: belt.composition(),
            main: annulus(belt.main().inner_edge(), belt.main().outer_edge()),
            scattered: belt
                .scattered()
                .map(|part| annulus(part.inner_edge(), part.outer_edge())),
        }
    }
}

/// The impactors' sources about one orbit host: its mass, its belts and its innermost giant.
///
/// P14.T24.b builds one per host, which [`ImpactorSources::new`] validates once, and lends it to
/// each of the host's bodies.
#[derive(Debug, Clone, PartialEq)]
pub struct ImpactorSources {
    host: GravitationalParameter,
    belts: Vec<SourceBelt>,
    innermost_giant: Option<Metres>,
}

impl ImpactorSources {
    /// The sources about a host of gravitational parameter `host`, with the belts `belts`, in
    /// their generated order, which breaks a tie between equally near belts, and the planets
    /// `planets`, as their masses and semi-major axes about it, in any order.
    ///
    /// # Errors
    ///
    /// - [`BuildImpactorSourcesError::HostParameter`] for a parameter that is not finite and
    ///   positive;
    /// - [`BuildImpactorSourcesError::BeltEdges`] for an annulus whose edges are not finite,
    ///   positive and ordered;
    /// - [`BuildImpactorSourcesError::Planet`] for a planet whose mass is not finite and
    ///   non-negative, or whose semi-major axis is not finite and positive.
    pub fn new(
        host: GravitationalParameter,
        belts: Vec<SourceBelt>,
        planets: impl IntoIterator<Item = (EarthMasses, Metres)>,
    ) -> Result<Self, BuildImpactorSourcesError> {
        if !positive(host.value()) {
            return Err(BuildImpactorSourcesError::HostParameter(host));
        }
        for belt in &belts {
            for annulus in core::iter::once(belt.main).chain(belt.scattered) {
                let (inner, outer) = (annulus.inner_edge, annulus.outer_edge);
                if !(positive(inner.value()) && outer.value().is_finite() && inner <= outer) {
                    return Err(BuildImpactorSourcesError::BeltEdges(inner, outer));
                }
            }
        }
        let mut innermost_giant: Option<Metres> = None;
        for (mass, a) in planets {
            if !(non_negative(mass.value()) && positive(a.value())) {
                return Err(BuildImpactorSourcesError::Planet(mass, a));
            }
            if mass >= IMPACTOR_GIANT_MASS && innermost_giant.is_none_or(|giant| a < giant) {
                innermost_giant = Some(a);
            }
        }
        Ok(Self {
            host,
            belts,
            innermost_giant,
        })
    }

    /// The host's gravitational parameter, m³ s⁻²: the star's, or a close binary's sum for a
    /// circumbinary orbit.
    #[must_use]
    pub const fn host(&self) -> GravitationalParameter {
        self.host
    }

    /// The host's belts, in their generated order.
    #[must_use]
    pub fn belts(&self) -> &[SourceBelt] {
        &self.belts
    }

    /// The semi-major axis of the host's innermost giant, a planet of at least
    /// [`IMPACTOR_GIANT_MASS`]; `None` for a host with none.
    #[must_use]
    pub const fn innermost_giant(&self) -> Option<Metres> {
        self.innermost_giant
    }
}

/// Why [`ImpactorSources::new`] refused its parts, with the value it refused.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildImpactorSourcesError {
    /// The host's gravitational parameter is not finite and positive.
    HostParameter(GravitationalParameter),
    /// A belt's annulus, its inner and outer edges, is not finite, positive and ordered.
    BeltEdges(Metres, Metres),
    /// A planet's mass is not finite and non-negative, or its semi-major axis is not finite and
    /// positive.
    Planet(EarthMasses, Metres),
}

impl fmt::Display for BuildImpactorSourcesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HostParameter(mu) => write!(
                f,
                "host parameter {} m³ s⁻² is not finite and positive",
                mu.value()
            ),
            Self::BeltEdges(inner, outer) => write!(
                f,
                "belt edges {} m to {} m are not finite, positive and ordered",
                inner.value(),
                outer.value()
            ),
            Self::Planet(mass, a) => write!(
                f,
                "planet of {} M⊕ at {} m is out of range",
                mass.value(),
                a.value()
            ),
        }
    }
}

impl Error for BuildImpactorSourcesError {}

/// What [`impactors`] reads of a body: where it goes round, its bulk and surface, and its host's
/// sources.
///
/// Plain data with public fields: [`impactors`] validates them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImpactorBody<'a> {
    /// Where it goes round.
    pub orbit: ImpactorOrbit,
    /// Its mean radius, m, finite and positive: the bulk section's.
    pub radius: Metres,
    /// Its bulk density, kg m⁻³, finite and positive: the bulk section's, a porous target's.
    pub density: KilogramsPerCubicMetre,
    /// Its mean surface gravity, m s⁻², finite and positive: the bulk section's.
    pub surface_gravity: MetresPerSecondSquared,
    /// Its mean surface temperature, K, finite and non-negative: P14.T24.a's.
    pub surface_temperature: Kelvin,
    /// Its water, ice and liquid, at its surface and in its crust, kg, finite and non-negative:
    /// the volatile inventory's (P14.T13.a), or a moon's ice.
    pub water: Kilograms,
    /// Its host's sources.
    pub sources: &'a ImpactorSources,
}

/// Whether a body formed icy, as its crust reads it ([`Crust::of`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Birthplace {
    /// A moon formed beyond its parent's ice line, or an icy belt's member.
    Icy,
    /// Any other body, whose crust is ice-rich by its water layer alone.
    Other,
}

/// What a body's crust is, as its craters and its relief read it (science-r09-craters.md items 1
/// and 2, and F5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Crust {
    /// Rock: [`SILICATE_K_TARGET`], and [`SILICATE_TARGET_DENSITY`] for a non-porous target.
    Silicate,
    /// Ice-rich: [`ICE_K_TARGET`], and ice Ih's density for a non-porous target.
    IceRich,
}

impl Crust {
    /// The crust of a body of birthplace `birthplace`, at mean surface temperature
    /// `surface_temperature`, holding `water` over a radius `radius`.
    ///
    /// It is ice-rich when the body lies under [`ICE_RICH_TEMPERATURE`] and either formed icy or
    /// holds a global water layer of at least [`ICE_RICH_LAYER_DEPTH`] ([`ice_layer_depth`]). It
    /// is the one predicate of the crust's mechanical factors: `k_target`, the ice target's
    /// density, and P14.T48.a's `k_comp` and ice lithosphere.
    /// [`SurfaceMaterial`](crate::planetary::derive::atmosphere::SurfaceMaterial) classes the
    /// surface's albedo instead (science-r09-craters.md F5.4).
    #[must_use]
    pub fn of(
        birthplace: Birthplace,
        surface_temperature: Kelvin,
        water: Kilograms,
        radius: Metres,
    ) -> Self {
        let cold = surface_temperature < ICE_RICH_TEMPERATURE;
        let icy = match birthplace {
            Birthplace::Icy => true,
            Birthplace::Other => ice_layer_depth(water, radius) >= ICE_RICH_LAYER_DEPTH,
        };
        if cold && icy {
            Self::IceRich
        } else {
            Self::Silicate
        }
    }

    /// Its target factor `k_target`.
    #[must_use]
    pub const fn k_target(self) -> f64 {
        match self {
            Self::Silicate => SILICATE_K_TARGET,
            Self::IceRich => ICE_K_TARGET,
        }
    }

    /// Its density as a non-porous target: a silicate crust's [`SILICATE_TARGET_DENSITY`], or an
    /// ice-rich crust's ice Ih at the body's mean surface temperature `surface_temperature`.
    ///
    /// Ice Ih's is [`ice_ih_density`](crate::substance::ice_ih_density)'s: IAPWS R10-06(2009),
    /// 933.8 kg m⁻³ at 0 K to 916.7 at the melting point. Water ice is of the non-porous class in
    /// the gravity regime ("the scaling parameters are similar for all non-porous materials (e.g.,
    /// rock versus ice)", Singer et al. 2019, Science 363, 955, supplement). It is the pure ice's
    /// value: rock dust raises it and porosity lowers it, and P14.T51.c's primary crust replaces it
    /// (science-r09-craters.md F5.3).
    #[must_use]
    pub fn non_porous_density(self, surface_temperature: Kelvin) -> KilogramsPerCubicMetre {
        match self {
            Self::Silicate => SILICATE_TARGET_DENSITY,
            Self::IceRich => substance::ice_ih_density(surface_temperature),
        }
    }
}

/// Which rule gave a body's impactors (the [module](self) documentation's three).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImpactorSource {
    /// Rule 1: a belt the body is a member of or orbits inside, by that component's excitation.
    InBelt {
        /// The component's excitation `U_b`².
        excitation: f64,
        /// The belt's composition.
        composition: BeltComposition,
    },
    /// Rule 2: the giants' comets, for a giant, a body at or beyond the innermost giant, and their
    /// moons.
    Comets,
    /// Rule 3: the nearest belt, from inside or outside it.
    NearestBelt {
        /// Its excitation `U_b`² plus the cube of the fractional distance.
        excitation: f64,
        /// The belt's composition.
        composition: BeltComposition,
    },
    /// Rule 3 with no belt: the comets, from inside the innermost giant, by
    /// [`COMET_EXCITATION`]² plus the cube of the fractional distance, or alone with no giant.
    InnerComets {
        /// [`COMET_EXCITATION`]² plus the cube.
        excitation: f64,
    },
}

impl ImpactorSource {
    /// The impactors' own excitation `U_src`², dimensionless.
    #[must_use]
    pub const fn excitation(self) -> f64 {
        match self {
            Self::InBelt { excitation, .. }
            | Self::NearestBelt { excitation, .. }
            | Self::InnerComets { excitation } => excitation,
            Self::Comets => COMET_EXCITATION * COMET_EXCITATION,
        }
    }

    /// The projectiles' density: [`ROCKY_PROJECTILE_DENSITY`] from a rocky belt and
    /// [`ICY_PROJECTILE_DENSITY`] from an icy belt or the comets.
    #[must_use]
    pub const fn projectile_density(self) -> KilogramsPerCubicMetre {
        match self {
            Self::InBelt { composition, .. } | Self::NearestBelt { composition, .. } => {
                match composition {
                    BeltComposition::Rocky => ROCKY_PROJECTILE_DENSITY,
                    BeltComposition::Icy => ICY_PROJECTILE_DENSITY,
                }
            }
            Self::Comets | Self::InnerComets { .. } => ICY_PROJECTILE_DENSITY,
        }
    }

    /// The source of the impactors striking `orbiter`, or a moon of it, under `sources`.
    #[must_use]
    fn of(orbiter: &HostOrbiter, sources: &ImpactorSources) -> Self {
        let a = orbiter.orbit.semi_major_axis;
        // Rule 1: a member shares its part's excitation; any body orbiting in an annulus, that
        // annulus's.
        if let Some(membership) = orbiter.belt {
            return Self::InBelt {
                excitation: belt_excitation(membership.part),
                composition: membership.composition,
            };
        }
        for belt in &sources.belts {
            let parts = [
                (BeltPart::Main, Some(belt.main)),
                (BeltPart::Scattered, belt.scattered),
            ];
            for (part, annulus) in parts {
                if annulus.is_some_and(|annulus| annulus.holds(a)) {
                    return Self::InBelt {
                        excitation: belt_excitation(part),
                        composition: belt.composition,
                    };
                }
            }
        }
        // Rule 2.
        if orbiter.mass >= IMPACTOR_GIANT_MASS || sources.innermost_giant.is_some_and(|g| a >= g) {
            return Self::Comets;
        }
        // Rule 3: an asteroid belt first, then any belt, then the innermost giant.
        let asteroid = nearest_belt(
            sources
                .belts
                .iter()
                .filter(|b| b.kind == BeltKind::Asteroid),
            a,
        );
        if let Some(belt) = asteroid.or_else(|| nearest_belt(sources.belts.iter(), a)) {
            let a_b = belt.main.mid_radius();
            return Self::NearestBelt {
                excitation: belt_excitation(BeltPart::Main) + approach(a, a_b),
                composition: belt.composition,
            };
        }
        let comets = COMET_EXCITATION * COMET_EXCITATION;
        Self::InnerComets {
            excitation: match sources.innermost_giant {
                // Rule 2 has taken every body at or beyond the giant, so a < a_g here.
                Some(a_g) => comets + approach(a, a_g),
                None => comets,
            },
        }
    }
}

/// A belt component's own excitation `U_b`² = ⟨e²⟩ + ⟨sin² i⟩ = 2σₑ² + 2 sin² σᵢ, dimensionless,
/// from the Rayleigh widths its members are placed by (P14.T21.c): σₑ
/// [`MEMBER_ECCENTRICITY_WIDTH`] and σᵢ [`MEMBER_INCLINATION_WIDTH_DEG`] in the belt proper, 0.0587,
/// and [`SCATTERED_ECCENTRICITY_WIDTH`] and [`SCATTERED_INCLINATION_WIDTH_DEG`] in a scattered
/// component, 0.414 (science-r09-craters.md item 4, whose 0.30 was 0.297, the inclination's factor
/// 2 dropped, F5.1).
///
/// To second order it is Öpik's U² = 2 − 2√(1 − e²) cos i at equal semi-major axes. Over the same
/// untruncated laws, that U²'s exact mean is 0.0586 and 0.408, within 0.2% and 1.4% of these. The
/// exact ⟨e²⟩ + ⟨sin² i⟩, 0.0580 and 0.388, lies 1.0% and 5.0% below it (F5.6). The members'
/// truncation at the crossing limit is not applied: the record's population moments are the
/// untruncated laws' (P14.T48.g).
#[must_use]
pub fn belt_excitation(part: BeltPart) -> f64 {
    let (sigma_e, sigma_i_deg) = match part {
        BeltPart::Main => (MEMBER_ECCENTRICITY_WIDTH, MEMBER_INCLINATION_WIDTH_DEG),
        BeltPart::Scattered => (
            SCATTERED_ECCENTRICITY_WIDTH,
            SCATTERED_INCLINATION_WIDTH_DEG,
        ),
    };
    let sin_i = math::sin(sigma_i_deg.to_radians());
    2.0 * sigma_e * sigma_e + 2.0 * sin_i * sin_i
}

/// The approach term of rule 3, (1 − a ÷ `a_b`)³ inside the reference radius `a_b` and
/// (1 − `a_b` ÷ a)³ outside it: the cube of the fractional distance, zero at `a_b`.
#[must_use]
fn approach(a: Metres, a_b: Metres) -> f64 {
    let x = if a < a_b {
        1.0 - a.value() / a_b.value()
    } else {
        1.0 - a_b.value() / a.value()
    };
    x * x * x
}

/// The nearest of `belts` to a semi-major axis `a`, by its belt proper's mid-radius `a_b`: the
/// least ratio max(a, `a_b`) ÷ min(a, `a_b`), the first on a tie; `None` for no belt.
#[must_use]
fn nearest_belt<'b>(
    belts: impl Iterator<Item = &'b SourceBelt>,
    a: Metres,
) -> Option<&'b SourceBelt> {
    let closeness = |belt: &SourceBelt| {
        let a_b = belt.main.mid_radius().value();
        let a = a.value();
        a.min(a_b) / a.max(a_b)
    };
    belts
        .fold(None, |best: Option<(&SourceBelt, f64)>, belt| {
            let c = closeness(belt);
            match best {
                Some((_, b)) if c <= b => best,
                Some(_) | None => Some((belt, c)),
            }
        })
        .map(|(belt, _)| belt)
}

/// The depth of the global layer a body's water would make, m: its mass spread over a sphere of
/// the body's radius at [`WATER_LAYER_DENSITY`], m ÷ (ρ 4π R²).
#[must_use]
pub fn ice_layer_depth(water: Kilograms, radius: Metres) -> Metres {
    let area = 4.0 * core::f64::consts::PI * radius.value() * radius.value();
    Metres::new(water.value() / (WATER_LAYER_DENSITY.value() * area))
}

/// A body's impactors (P14.T48.f): their mean impact velocity and density, and the crater target
/// they strike, its class, density and factor, which P14.T48.c's crater contract carries.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Impactors {
    velocity: MetresPerSecond,
    source: ImpactorSource,
    target: CraterTarget,
    crust: Crust,
}

impl Impactors {
    /// The mean impact velocity, m/s, finite and positive.
    #[must_use]
    pub const fn velocity(&self) -> MetresPerSecond {
        self.velocity
    }

    /// The projectiles' density, kg m⁻³: [`ROCKY_PROJECTILE_DENSITY`] or
    /// [`ICY_PROJECTILE_DENSITY`].
    #[must_use]
    pub const fn projectile_density(&self) -> KilogramsPerCubicMetre {
        self.source.projectile_density()
    }

    /// The crater target: porous at the body's bulk density below [`POROUS_TARGET_RADIUS`], and
    /// otherwise non-porous at its crust's density.
    #[must_use]
    pub const fn target(&self) -> CraterTarget {
        self.target
    }

    /// The target factor `k_target`, its crust's.
    #[must_use]
    pub const fn k_target(&self) -> f64 {
        self.crust.k_target()
    }

    /// What its crust is, by the ice-rich predicate of science-r09-craters.md item 2 and F5.4
    /// ([`Crust::of`]), which sets `k_target` and the non-porous target's density.
    #[must_use]
    pub const fn crust(&self) -> Crust {
        self.crust
    }

    /// Which rule gave the impactors.
    #[must_use]
    pub const fn source(&self) -> ImpactorSource {
        self.source
    }
}

/// Why [`impactors`] refused a body, with the value it refused.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeriveImpactorsError {
    /// The semi-major axis about the host is not finite and positive.
    SemiMajorAxis(Metres),
    /// The eccentricity is not in `[0, 1)`.
    Eccentricity(f64),
    /// The inclination is not finite.
    Inclination(Radians),
    /// The mass is not finite and non-negative, or a moon's parent's is not positive.
    Mass(EarthMasses),
    /// A moon's semi-major axis about its parent is not finite and positive.
    MoonSemiMajorAxis(Metres),
    /// The radius is not finite and positive.
    Radius(Metres),
    /// The bulk density is not finite and positive.
    Density(KilogramsPerCubicMetre),
    /// The surface gravity is not finite and positive.
    SurfaceGravity(MetresPerSecondSquared),
    /// The surface temperature is not finite and non-negative.
    SurfaceTemperature(Kelvin),
    /// The water is not finite and non-negative.
    Water(Kilograms),
}

impl fmt::Display for DeriveImpactorsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SemiMajorAxis(a) => write!(
                f,
                "semi-major axis {} m is not finite and positive",
                a.value()
            ),
            Self::Eccentricity(e) => write!(f, "eccentricity {e} is not in [0, 1)"),
            Self::Inclination(i) => write!(f, "inclination {} rad is not finite", i.value()),
            Self::Mass(m) => write!(f, "mass {} M⊕ is out of range", m.value()),
            Self::MoonSemiMajorAxis(a) => write!(
                f,
                "moon's semi-major axis {} m is not finite and positive",
                a.value()
            ),
            Self::Radius(r) => write!(f, "radius {} m is not finite and positive", r.value()),
            Self::Density(rho) => write!(
                f,
                "density {} kg m⁻³ is not finite and positive",
                rho.value()
            ),
            Self::SurfaceGravity(g) => write!(
                f,
                "surface gravity {} m s⁻² is not finite and positive",
                g.value()
            ),
            Self::SurfaceTemperature(t) => write!(
                f,
                "surface temperature {} K is not finite and non-negative",
                t.value()
            ),
            Self::Water(w) => write!(f, "water {} kg is not finite and non-negative", w.value()),
        }
    }
}

impl Error for DeriveImpactorsError {}

/// Whether `x` is finite and positive.
#[must_use]
fn positive(x: f64) -> bool {
    x.is_finite() && x > 0.0
}

/// Whether `x` is finite and non-negative.
#[must_use]
fn non_negative(x: f64) -> bool {
    x.is_finite() && x >= 0.0
}

/// Checks a host orbiter's orbit and mass.
fn check_orbiter(orbiter: &HostOrbiter) -> Result<(), DeriveImpactorsError> {
    let HostOrbit {
        semi_major_axis: a,
        eccentricity: e,
        inclination: i,
    } = orbiter.orbit;
    if !positive(a.value()) {
        return Err(DeriveImpactorsError::SemiMajorAxis(a));
    }
    if !(e.is_finite() && (0.0..1.0).contains(&e)) {
        return Err(DeriveImpactorsError::Eccentricity(e));
    }
    if !i.value().is_finite() {
        return Err(DeriveImpactorsError::Inclination(i));
    }
    if !non_negative(orbiter.mass.value()) {
        return Err(DeriveImpactorsError::Mass(orbiter.mass));
    }
    Ok(())
}

/// The impactors of the body `body` describes (P14.T48.f): their mean impact velocity, their
/// density and the crater target, by the [module](self) documentation's closed form, with no
/// draw.
///
/// # Errors
///
/// - [`DeriveImpactorsError::SemiMajorAxis`] for a semi-major axis about the host that is not
///   finite and positive, and [`DeriveImpactorsError::MoonSemiMajorAxis`] for a moon's about its
///   parent;
/// - [`DeriveImpactorsError::Eccentricity`] for an eccentricity outside `[0, 1)`, and
///   [`DeriveImpactorsError::Inclination`] for an inclination that is not finite;
/// - [`DeriveImpactorsError::Mass`] for a mass that is not finite and non-negative, or a moon's
///   parent's that is not positive;
/// - [`DeriveImpactorsError::Radius`], [`DeriveImpactorsError::Density`] and
///   [`DeriveImpactorsError::SurfaceGravity`] for a radius, bulk density or gravity that is not
///   finite and positive;
/// - [`DeriveImpactorsError::SurfaceTemperature`] and [`DeriveImpactorsError::Water`] for a
///   temperature or a water mass that is not finite and non-negative.
///
/// # Examples
///
/// The Moon, whose impactors are Earth's, from the asteroid belt inside Jupiter: its mean impact
/// velocity is the diameter map's lunar reference.
///
/// ```
/// use hyperion_sim::math;
/// use hyperion_sim::planetary::belts::BeltComposition;
/// use hyperion_sim::planetary::derive::SnowLineSide;
/// use hyperion_sim::planetary::hooks::figures::{
///     HostOrbit, HostOrbiter, ImpactorBody, ImpactorOrbit, ImpactorSources, SourceAnnulus,
///     SourceBelt, impactors,
/// };
/// use hyperion_sim::planetary::record::BeltKind;
/// use hyperion_sim::units::consts::METRES_PER_AU;
/// use hyperion_sim::units::{
///     EarthMasses, GravitationalParameter, Kelvin, Kilograms, KilogramsPerCubicMetre, Metres,
///     MetresPerSecondSquared, Radians, SolarMasses,
/// };
/// use hyperion_surface::craters::{LUNAR_IMPACT_VELOCITY, TargetClass};
///
/// let au = |x: f64| Metres::new(x * METRES_PER_AU);
/// // The asteroid belt between Jupiter's 4:1 and 2:1 resonances.
/// let jupiter = 5.2029;
/// let belt = SourceBelt {
///     kind: BeltKind::Asteroid,
///     composition: BeltComposition::Rocky,
///     main: SourceAnnulus {
///         inner_edge: au(jupiter * math::powf(0.25, 2.0 / 3.0)),
///         outer_edge: au(jupiter * math::powf(0.5, 2.0 / 3.0)),
///     },
///     scattered: None,
/// };
/// let sun = ImpactorSources::new(
///     GravitationalParameter::from_solar_masses(SolarMasses::new(1.0)),
///     vec![belt],
///     [(EarthMasses::new(1.0), au(1.0)), (EarthMasses::new(317.8), au(jupiter))],
/// )?;
/// let earth = HostOrbiter {
///     orbit: HostOrbit { semi_major_axis: au(1.0), eccentricity: 0.0167, inclination: Radians::ZERO },
///     mass: EarthMasses::new(1.0),
///     belt: None,
/// };
/// let moon = impactors(&ImpactorBody {
///     orbit: ImpactorOrbit::Moon { parent: earth, semi_major_axis: Metres::new(3.844e8), formed: SnowLineSide::Inside },
///     radius: Metres::new(1.7374e6),
///     density: KilogramsPerCubicMetre::new(3_344.0),
///     surface_gravity: MetresPerSecondSquared::new(1.62),
///     surface_temperature: Kelvin::new(250.0),
///     water: Kilograms::ZERO,
///     sources: &sun,
/// })?;
/// let v = moon.velocity().value();
/// assert!((v - LUNAR_IMPACT_VELOCITY.value()).abs() < 100.0);
/// assert_eq!(moon.target().class, TargetClass::NonPorous);
/// assert!((moon.k_target() - 1.0).abs() < 1e-12);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn impactors(body: &ImpactorBody<'_>) -> Result<Impactors, DeriveImpactorsError> {
    let (orbiter, moon) = match body.orbit {
        ImpactorOrbit::Host(orbiter) => {
            check_orbiter(&orbiter)?;
            (orbiter, None)
        }
        ImpactorOrbit::Moon {
            parent,
            semi_major_axis,
            formed,
        } => {
            check_orbiter(&parent)?;
            if !positive(parent.mass.value()) {
                return Err(DeriveImpactorsError::Mass(parent.mass));
            }
            if !positive(semi_major_axis.value()) {
                return Err(DeriveImpactorsError::MoonSemiMajorAxis(semi_major_axis));
            }
            (parent, Some((semi_major_axis, formed)))
        }
    };
    let (radius, density, gravity) = (body.radius, body.density, body.surface_gravity);
    if !positive(radius.value()) {
        return Err(DeriveImpactorsError::Radius(radius));
    }
    if !positive(density.value()) {
        return Err(DeriveImpactorsError::Density(density));
    }
    if !positive(gravity.value()) {
        return Err(DeriveImpactorsError::SurfaceGravity(gravity));
    }
    if !non_negative(body.surface_temperature.value()) {
        return Err(DeriveImpactorsError::SurfaceTemperature(
            body.surface_temperature,
        ));
    }
    if !non_negative(body.water.value()) {
        return Err(DeriveImpactorsError::Water(body.water));
    }

    let source = ImpactorSource::of(&orbiter, body.sources);
    let HostOrbit {
        semi_major_axis: a,
        eccentricity: e,
        inclination: i,
    } = orbiter.orbit;
    let sin_i = math::sin(i.value());
    let encounter = source.excitation() + e * e + sin_i * sin_i;
    let circular_squared = body.sources.host.value() / a.value();
    let v_infinity_squared = encounter * circular_squared;
    let escape_squared = 2.0 * gravity.value() * radius.value();
    let velocity_squared = match moon {
        None => v_infinity_squared + escape_squared,
        Some((a_s, _)) => {
            let parent = GravitationalParameter::from_earth_masses(orbiter.mass);
            let orbital_squared = parent.value() / a_s.value();
            v_infinity_squared + 3.0 * orbital_squared + escape_squared
        }
    };

    let birthplace = match (moon, orbiter.belt) {
        (Some((_, SnowLineSide::Beyond)), _)
        | (
            None,
            Some(BeltMembership {
                composition: BeltComposition::Icy,
                ..
            }),
        ) => Birthplace::Icy,
        (Some((_, SnowLineSide::Inside)), _)
        | (
            None,
            Some(BeltMembership {
                composition: BeltComposition::Rocky,
                ..
            })
            | None,
        ) => Birthplace::Other,
    };
    let crust = Crust::of(birthplace, body.surface_temperature, body.water, radius);
    let target = if radius < POROUS_TARGET_RADIUS {
        CraterTarget {
            class: TargetClass::Porous,
            density,
        }
    } else {
        CraterTarget {
            class: TargetClass::NonPorous,
            density: crust.non_porous_density(body.surface_temperature),
        }
    };
    Ok(Impactors {
        velocity: MetresPerSecond::new(velocity_squared.sqrt()),
        source,
        target,
        crust,
    })
}
