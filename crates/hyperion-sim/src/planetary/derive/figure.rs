//! A body's figure (plan 14, P14.T46.c–e): its rotational spheroid by the Darwin–Radau relation,
//! and the datum its heights are measured from.
//!
//! # The flattening (P14.T46.c)
//!
//! With the rotational parameter q = ω² a³ ÷ GM at the equatorial radius a and the moment of
//! inertia C ÷ M a² of [`moment_of_inertia_factor`], a body in hydrostatic equilibrium has the
//! flattening ([`darwin_radau_flattening`]; Murray and Dermott 1999, _Solar System Dynamics_,
//! §4.6, the Darwin–Radau relation, in the form of Bourda and Capitaine 2004, A&A 428, 691)
//!
//! f = (5 ÷ 2) q ÷ [1 + (25 ÷ 4)(1 − (3 ÷ 2) C ÷ M a²)²].
//!
//! a is not known before f, so [`BodyFigure::derive`] iterates from a = `R_vol`, the record's
//! volumetric radius, with a = `R_vol` (1 − f)^(−⅓) and c = a (1 − f), the volume kept, f capped
//! inside the loop, until a pass changes f by under 10⁻¹², at most [`MAX_FIGURE_PASSES`] passes
//! (Saturn takes 13). ω is the rotation rate at the record's time, so f follows the despin and a
//! giant's contraction.
//!
//! - **Hydrostatic only above a size.** Below [`HYDROSTATIC_RADIUS_ICE`] for an icy surface and
//!   [`HYDROSTATIC_RADIUS_ROCK`] for the rest, a body is a sphere of its mean radius
//!   ([`FigureLaw::Sphere`]): it holds the shape its rock gives it, not the one its spin asks for
//!   (Lineweaver and Norman 2010, arXiv:1004.1091, the "potato radius").
//! - **A synchronous body's tide.** A body locked 1:1 takes [`SYNCHRONOUS_TIDAL_FACTOR`] times the
//!   spin's flattening ([`FigureLaw::RotationalAndTidal`]): in first-order hydrostatic theory the
//!   tidal potential on a synchronous body is three times the rotational one, giving axes in the
//!   ratio (a − c) : (b − c) : (a − b) = 4 : 1 : 3 (Dermott 1979, Icarus 37, 575; Murray and
//!   Dermott 1999, §4.7), whose best spheroid about the pole, (a + b) ÷ 2 against c, is 2.5 times
//!   as flattened as the spin alone. The triaxial long axis toward the primary is not drawn.
//! - **A cap** of [`FLATTENING_CAP`] ([`FigureLaw::Capped`]), beyond which first-order theory
//!   fails.
//!
//! With the class factors and NASA's fact-sheet GM, mean radius and sidereal period the relation
//! gives Earth 0.00334 (observed 0.00335), Mars 0.00442 (0.00589: the class's 0.33 against its
//! 0.366 and the non-hydrostatic Tharsis load), Jupiter 0.0648 (0.0649), Saturn 0.0985 at 0.21
//! (0.0980), Uranus 0.0200 (0.0229: one ice-giant factor does not fit both) and Neptune 0.0177
//! (0.0171); the tests hold them.
//!
//! # The datum (P14.T46.e)
//!
//! The figure is the one height datum (R07 Design note 19, R05 Design note 5): every height a
//! later generator derives is geodetic, along the spheroid's normal. A body with a hydrogen and
//! helium envelope takes its 1-bar level, which its radius is quoted at ([`Datum::OneBar`]); every
//! other body its solid or liquid surface ([`Datum::SolidSurface`]). The record's radius stays
//! the volumetric mean, so density and gravity are unchanged.

use hyperion_surface::spheroid::Spheroid;

use crate::math;
use crate::planetary::derive::PlanetClass;
use crate::planetary::derive::atmosphere::SurfaceMaterial;
use crate::planetary::derive::composition::MassFractions;
use crate::planetary::derive::rotation::{SpinOrbitResonance, SpinState, moment_of_inertia_factor};
use crate::planetary::params::{
    FLATTENING_CAP, HYDROSTATIC_RADIUS_ICE, HYDROSTATIC_RADIUS_ROCK, SYNCHRONOUS_TIDAL_FACTOR,
};
use crate::units::consts::GRAVITATIONAL_CONSTANT;
use crate::units::{Kilograms, Metres};

/// The most passes of the flattening's iteration: 50, where Saturn takes 13, each pass shrinking
/// the change about ninefold.
pub const MAX_FIGURE_PASSES: u32 = 50;

/// The change in f below which the iteration stops: 10⁻¹².
const FIGURE_TOLERANCE: f64 = 1e-12;

/// The rotational parameter q = ω² a³ ÷ GM of a body of mass `mass` spinning at `rate` rad s⁻¹,
/// at its equatorial radius `equatorial_radius`: the ratio of the centrifugal acceleration at the
/// equator to gravity there.
#[must_use]
pub fn rotational_parameter(rate: f64, equatorial_radius: Metres, mass: Kilograms) -> f64 {
    let a = equatorial_radius.value();
    rate * rate * (a * a * a) / (GRAVITATIONAL_CONSTANT * mass.value())
}

/// The hydrostatic flattening of a body of rotational parameter `q` and moment of inertia
/// `moment_factor` = C ÷ M a², by the Darwin–Radau relation (see the [module](self)
/// documentation): f = (5 ÷ 2) q ÷ [1 + (25 ÷ 4)(1 − (3 ÷ 2) C ÷ M a²)²].
///
/// # Examples
///
/// Earth, at its 23.93 h sidereal day and the rocky class's 0.33, is within 1% of its measured
/// 1 ÷ 298:
///
/// ```
/// use hyperion_sim::planetary::derive::figure::{darwin_radau_flattening, rotational_parameter};
/// use hyperion_sim::units::{Kilograms, Metres};
///
/// let rate = core::f64::consts::TAU / (23.9345 * 3_600.0);
/// let a = Metres::new(6.378e6);
/// let q = rotational_parameter(rate, a, Kilograms::new(5.972e24));
/// let f = darwin_radau_flattening(q, 0.33);
/// assert!((f * 298.257 - 1.0).abs() < 0.01);
/// ```
#[must_use]
pub fn darwin_radau_flattening(q: f64, moment_factor: f64) -> f64 {
    let x = 1.0 - 1.5 * moment_factor;
    2.5 * q / (1.0 + 6.25 * x * x)
}

/// How a body's figure was found (P14.T46.c).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FigureLaw {
    /// Below the hydrostatic size: a sphere of the mean radius.
    Sphere,
    /// The spin's Darwin–Radau flattening.
    Rotational,
    /// A synchronous body's: the spin's flattening times [`SYNCHRONOUS_TIDAL_FACTOR`].
    RotationalAndTidal,
    /// Held at [`FLATTENING_CAP`].
    Capped,
}

/// What a body's heights are measured from (P14.T46.e).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Datum {
    /// The solid or liquid surface.
    SolidSurface,
    /// The 1-bar level of a hydrogen and helium envelope, which its radius is quoted at.
    OneBar,
}

impl Datum {
    /// The datum of a body of class `class`: [`OneBar`](Self::OneBar) for a sub-Neptune, an ice
    /// giant or a gas giant, whose radius is its envelope's, [`SolidSurface`](Self::SolidSurface)
    /// otherwise.
    #[must_use]
    pub const fn of(class: PlanetClass) -> Self {
        match class {
            PlanetClass::Rocky | PlanetClass::Icy => Self::SolidSurface,
            PlanetClass::SubNeptune | PlanetClass::IceGiant | PlanetClass::GasGiant => Self::OneBar,
        }
    }
}

/// What [`BodyFigure::derive`] reads of a body at a record's time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FigureInputs {
    /// The volumetric mean radius, the record's.
    pub radius: Metres,
    /// The mass.
    pub mass: Kilograms,
    /// The class, which sets the moment of inertia and the datum.
    pub class: PlanetClass,
    /// The mass fractions, which set a gas giant's moment of inertia and the surface's material.
    pub fractions: MassFractions,
    /// The spin rate at the time, rad s⁻¹ (`RotationLaw::rate_at`).
    pub rate: f64,
    /// Whether the body is locked at the time, and how.
    pub state: SpinState,
    /// The pole, a unit vector along the galactic axes: the spheroid's symmetry axis.
    pub pole: [f64; 3],
}

/// A body's figure at a record's time (P14.T46.d): its reference spheroid, its pole, the moment
/// of inertia it was found with, how it was found, and the datum.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyFigure {
    spheroid: Spheroid,
    pole: [f64; 3],
    moment_factor: f64,
    law: FigureLaw,
    datum: Datum,
}

impl BodyFigure {
    /// The figure of the body `inputs` describe (see the [module](self) documentation).
    ///
    /// # Panics
    ///
    /// If the radius or the mass is not positive and finite, which no record's is.
    #[must_use]
    pub fn derive(inputs: &FigureInputs) -> Self {
        let moment_factor = moment_of_inertia_factor(inputs.class, &inputs.fractions);
        let threshold = match SurfaceMaterial::of(&inputs.fractions) {
            SurfaceMaterial::Ice => HYDROSTATIC_RADIUS_ICE,
            SurfaceMaterial::Rock => HYDROSTATIC_RADIUS_ROCK,
        };
        let (flattening, law) = if inputs.radius < threshold {
            (0.0, FigureLaw::Sphere)
        } else {
            let tidal = inputs.state == SpinState::Locked(SpinOrbitResonance::Synchronous);
            let solved = hydrostatic_flattening(
                inputs.radius,
                inputs.mass,
                inputs.rate,
                moment_factor,
                tidal,
            );
            let law = if solved.capped {
                FigureLaw::Capped
            } else if tidal {
                FigureLaw::RotationalAndTidal
            } else {
                FigureLaw::Rotational
            };
            (solved.flattening, law)
        };
        Self {
            spheroid: Spheroid::from_volumetric(inputs.radius.value(), flattening)
                .expect("a record's radius is positive and the flattening lies in [0, the cap]"),
            pole: inputs.pole,
            moment_factor,
            law,
            datum: Datum::of(inputs.class),
        }
    }

    /// The reference spheroid (a, a, c), the volume of the record's mean radius.
    #[must_use]
    pub const fn spheroid(&self) -> Spheroid {
        self.spheroid
    }

    /// The pole, the spheroid's symmetry axis: the rotation's, along the galactic axes.
    #[must_use]
    pub const fn pole(&self) -> [f64; 3] {
        self.pole
    }

    /// The moment of inertia C ÷ M a² the flattening was found with
    /// ([`moment_of_inertia_factor`]).
    #[must_use]
    pub const fn moment_factor(&self) -> f64 {
        self.moment_factor
    }

    /// How the figure was found.
    #[must_use]
    pub const fn law(&self) -> FigureLaw {
        self.law
    }

    /// What heights are measured from.
    #[must_use]
    pub const fn datum(&self) -> Datum {
        self.datum
    }
}

/// What the flattening's iteration found.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Solved {
    flattening: f64,
    capped: bool,
    passes: u32,
}

/// The hydrostatic flattening of a body of volumetric radius `radius` and mass `mass` spinning at
/// `rate` with moment of inertia `moment_factor`, times [`SYNCHRONOUS_TIDAL_FACTOR`] if `tidal`,
/// capped at [`FLATTENING_CAP`]: the module documentation's iteration.
fn hydrostatic_flattening(
    radius: Metres,
    mass: Kilograms,
    rate: f64,
    moment_factor: f64,
    tidal: bool,
) -> Solved {
    let r = radius.value();
    let factor = if tidal { SYNCHRONOUS_TIDAL_FACTOR } else { 1.0 };
    let mut solved = Solved {
        flattening: 0.0,
        capped: false,
        passes: 0,
    };
    let mut a = r;
    while solved.passes < MAX_FIGURE_PASSES {
        let q = rotational_parameter(rate, Metres::new(a), mass);
        let free = darwin_radau_flattening(q, moment_factor) * factor;
        let capped = free > FLATTENING_CAP;
        let next = if capped { FLATTENING_CAP } else { free };
        let change = (next - solved.flattening).abs();
        solved = Solved {
            flattening: next,
            capped,
            passes: solved.passes + 1,
        };
        a = r / math::cbrt(1.0 - next);
        if change < FIGURE_TOLERANCE {
            break;
        }
    }
    solved
}

#[cfg(test)]
mod tests {
    use core::f64::consts::TAU;

    use super::*;
    use crate::planetary::derive::radius::CoreComposition;
    use crate::planetary::params::{
        ENVELOPED_MOMENT_OF_INERTIA, GAS_GIANT_MOMENT_OF_INERTIA, ICY_MOMENT_OF_INERTIA,
        ROCKY_MOMENT_OF_INERTIA, SATURN_LIKE_MOMENT_OF_INERTIA,
    };

    /// A body of NASA's fact sheets: GM, km³ s⁻²; volumetric mean radius, km; sidereal rotation
    /// period, h; the moment factor the generator gives it; and its measured flattening.
    struct Planet {
        name: &'static str,
        gm_km3_s2: f64,
        radius_km: f64,
        period_h: f64,
        moment_factor: f64,
        observed: f64,
    }

    /// NASA's planetary fact sheets (nssdc.gsfc.nasa.gov/planetary/factsheet), and Ceres from
    /// Dawn (Park et al. 2016, Nature 537, 515: GM 62.6284 km³ s⁻², mean radius 469.7 km, period
    /// 9.074 h), with the measured flattening of 0.075 plan 14's check states.
    const PLANETS: [Planet; 7] = [
        Planet {
            name: "Earth",
            gm_km3_s2: 0.398_60e6,
            radius_km: 6_371.0,
            period_h: 23.9345,
            moment_factor: ROCKY_MOMENT_OF_INERTIA,
            observed: 0.003_35,
        },
        Planet {
            name: "Mars",
            gm_km3_s2: 0.042_828e6,
            radius_km: 3_389.5,
            period_h: 24.6229,
            moment_factor: ROCKY_MOMENT_OF_INERTIA,
            observed: 0.005_89,
        },
        Planet {
            name: "Jupiter",
            gm_km3_s2: 126.687e6,
            radius_km: 69_911.0,
            period_h: 9.9250,
            moment_factor: GAS_GIANT_MOMENT_OF_INERTIA,
            observed: 0.064_87,
        },
        Planet {
            name: "Saturn",
            gm_km3_s2: 37.931e6,
            radius_km: 58_232.0,
            period_h: 10.656,
            moment_factor: SATURN_LIKE_MOMENT_OF_INERTIA,
            observed: 0.097_96,
        },
        Planet {
            name: "Uranus",
            gm_km3_s2: 5.7940e6,
            radius_km: 25_362.0,
            period_h: 17.24,
            moment_factor: ENVELOPED_MOMENT_OF_INERTIA,
            observed: 0.022_93,
        },
        Planet {
            name: "Neptune",
            gm_km3_s2: 6.8351e6,
            radius_km: 24_622.0,
            period_h: 16.11,
            moment_factor: ENVELOPED_MOMENT_OF_INERTIA,
            observed: 0.017_08,
        },
        Planet {
            name: "Ceres",
            gm_km3_s2: 62.6284,
            radius_km: 469.7,
            period_h: 9.074,
            moment_factor: ICY_MOMENT_OF_INERTIA,
            observed: 0.075,
        },
    ];

    fn solve(p: &Planet, tidal: bool) -> Solved {
        hydrostatic_flattening(
            Metres::new(p.radius_km * 1e3),
            Kilograms::new(p.gm_km3_s2 * 1e9 / GRAVITATIONAL_CONSTANT),
            TAU / (p.period_h * 3_600.0),
            p.moment_factor,
            tidal,
        )
    }

    /// The inverse relation, C ÷ M a² = (2 ÷ 3)[1 − (2 ÷ 5)√(5q ÷ 2f − 1)].
    fn moment_factor_of(q: f64, f: f64) -> f64 {
        (2.0 / 3.0) * (1.0 - 0.4 * (2.5 * q / f - 1.0).sqrt())
    }

    /// P14.T46.c (c): the six-planet check, each within 2% but Mars (30%) and Uranus (15%), whose
    /// departures are stated, and Ceres within 15%.
    #[test]
    fn the_planets_flatten_as_measured() {
        for p in &PLANETS {
            let tolerance = match p.name {
                "Mars" => 0.30,
                "Uranus" | "Ceres" => 0.15,
                // +3.7% as plan 14's own check computes it; its 2% bound is a slip (Risks).
                "Neptune" => 0.05,
                _ => 0.02,
            };
            let f = solve(p, false).flattening;
            assert!(
                (f / p.observed - 1.0).abs() < tolerance,
                "{}: {f} against {}",
                p.name,
                p.observed
            );
        }
        // Saturn at Jupiter's factor would be 17% too flat: why the blend exists.
        let saturn = Planet {
            moment_factor: GAS_GIANT_MOMENT_OF_INERTIA,
            ..PLANETS[3]
        };
        assert!(solve(&saturn, false).flattening / 0.097_96 > 1.15);
    }

    /// P14.T46.c (c): the inverse relation returns the factor from (q, f) to 10⁻¹².
    #[test]
    fn the_inverse_returns_the_moment_factor() {
        for c in [0.21, 0.23, 0.25, 0.33, 0.34, 0.378, 0.4] {
            for q in [1e-6, 1e-3, 0.0345, 0.155] {
                let f = darwin_radau_flattening(q, c);
                assert!((moment_factor_of(q, f) - c).abs() < 1e-12, "{c} {q}");
            }
        }
    }

    /// P14.T46.c (c): the iteration converges within the pass limit for every body, the cap
    /// included, with no NaN; f never exceeds the cap.
    #[test]
    fn the_iteration_converges_and_respects_the_cap() {
        for p in &PLANETS {
            let solved = solve(p, false);
            assert!(solved.passes < MAX_FIGURE_PASSES, "{}", p.name);
        }
        assert_eq!(solve(&PLANETS[3], false).passes, 13);
        for period_h in [1.0, 2.0, 3.0, 5.0, 10.0, 100.0, 1e4] {
            for c in [0.21, 0.25, 0.33] {
                for radius_km in [300.0, 6_000.0, 70_000.0] {
                    let solved = hydrostatic_flattening(
                        Metres::new(radius_km * 1e3),
                        Kilograms::new(5.972e24 * math::powi(radius_km / 6_371.0, 3)),
                        TAU / (period_h * 3_600.0),
                        c,
                        period_h > 50.0,
                    );
                    let f = solved.flattening;
                    assert!(f.is_finite() && (0.0..=FLATTENING_CAP).contains(&f), "{f}");
                    assert!(
                        solved.passes < MAX_FIGURE_PASSES,
                        "{period_h} {c} {radius_km}"
                    );
                    assert_eq!(solved.capped, f >= FLATTENING_CAP);
                }
            }
        }
        // A 2 h Earth spins near break-up and is held at the cap.
        let fast = hydrostatic_flattening(
            Metres::new(6.371e6),
            Kilograms::new(5.972e24),
            TAU / 7_200.0,
            0.33,
            false,
        );
        assert!(fast.capped);
    }

    /// P14.T46.c (c): Io's synchronous figure, a − c, is 8–10 km with its measured C ÷ M R² of
    /// 0.378 (Anderson et al. 2001) and 7–10 km at the rocky class's 0.33, against 8.7 km
    /// measured (Archinal et al. 2011's axes).
    #[test]
    fn io_is_as_flat_as_its_tide_makes_it() {
        let io = |c: f64| {
            let radius = Metres::new(1_821.6e3);
            let solved = hydrostatic_flattening(
                radius,
                Kilograms::new(5_959.916e9 / GRAVITATIONAL_CONSTANT),
                TAU / (42.459 * 3_600.0),
                c,
                true,
            );
            let s = Spheroid::from_volumetric(radius.value(), solved.flattening).unwrap();
            (s.equatorial_radius_m - s.polar_radius_m) / 1e3
        };
        let measured = io(0.378);
        assert!((8.0..10.0).contains(&measured), "{measured}");
        let class = io(ROCKY_MOMENT_OF_INERTIA);
        assert!((7.0..10.0).contains(&class), "{class}");
    }

    fn inputs(radius_km: f64, fractions: MassFractions, class: PlanetClass) -> FigureInputs {
        FigureInputs {
            radius: Metres::new(radius_km * 1e3),
            // A density of 2,500 kg m⁻³.
            mass: Kilograms::new(
                4.0 / 3.0 * core::f64::consts::PI * math::powi(radius_km * 1e3, 3) * 2_500.0,
            ),
            class,
            fractions,
            rate: TAU / (8.0 * 3_600.0),
            state: SpinState::Despinning,
            pole: [0.0, 0.0, 1.0],
        }
    }

    /// P14.T46.c (c): a body below its material's hydrostatic radius is a sphere, ice from
    /// 200 km and rock from 300 km.
    #[test]
    fn small_bodies_are_spheres() {
        let rock = MassFractions::solid(0.3, 0.7, 0.0);
        let ice = MassFractions::solid(0.1, 0.4, 0.5);
        for (radius_km, fractions, class, law) in [
            (250.0, rock, PlanetClass::Rocky, FigureLaw::Sphere),
            (250.0, ice, PlanetClass::Icy, FigureLaw::Rotational),
            (199.0, ice, PlanetClass::Icy, FigureLaw::Sphere),
            (301.0, rock, PlanetClass::Rocky, FigureLaw::Rotational),
        ] {
            let figure = BodyFigure::derive(&inputs(radius_km, fractions, class));
            assert_eq!(figure.law(), law, "{radius_km} km");
            if law == FigureLaw::Sphere {
                assert!(figure.spheroid().flattening().abs() < f64::MIN_POSITIVE);
            } else {
                assert!(figure.spheroid().flattening() > 0.0);
            }
            assert!(
                (figure.spheroid().volumetric_radius_m() / (radius_km * 1e3) - 1.0).abs() < 1e-12
            );
        }
    }

    /// P14.T46.e (e): a giant's datum is the 1-bar level, a rocky or icy body's its surface; a
    /// synchronous body takes the tidal law.
    #[test]
    fn datum_and_law_by_kind() {
        let giant = MassFractions::of(
            CoreComposition::new(0.3, 0.5).unwrap(),
            1.0 - crate::planetary::params::JUPITER_HEAVY_ELEMENT_FRACTION,
        );
        let figure = BodyFigure::derive(&FigureInputs {
            radius: Metres::new(6.99e7),
            mass: Kilograms::new(1.898e27),
            ..inputs(1.0, giant, PlanetClass::GasGiant)
        });
        assert_eq!(figure.datum(), Datum::OneBar);
        assert_eq!(figure.law(), FigureLaw::Rotational);
        assert!((figure.moment_factor() - GAS_GIANT_MOMENT_OF_INERTIA).abs() < 1e-15);
        assert_eq!(Datum::of(PlanetClass::SubNeptune), Datum::OneBar);
        assert_eq!(Datum::of(PlanetClass::IceGiant), Datum::OneBar);
        assert_eq!(Datum::of(PlanetClass::Rocky), Datum::SolidSurface);
        assert_eq!(Datum::of(PlanetClass::Icy), Datum::SolidSurface);
        // A 40 h spin keeps both figures under the cap, so their ratio is the tide's factor.
        let rock = MassFractions::solid(0.3, 0.7, 0.0);
        let slow = FigureInputs {
            rate: TAU / (40.0 * 3_600.0),
            ..inputs(1_800.0, rock, PlanetClass::Rocky)
        };
        let locked = BodyFigure::derive(&FigureInputs {
            state: SpinState::Locked(SpinOrbitResonance::Synchronous),
            ..slow
        });
        let free = BodyFigure::derive(&slow);
        assert_eq!(locked.law(), FigureLaw::RotationalAndTidal);
        // q ∝ a³ ∝ 1 ÷ (1 − f) at a fixed volume, so the converged ratio is the factor times
        // (1 − f_free) ÷ (1 − f_locked).
        let (f_locked, f_free) = (locked.spheroid().flattening(), free.spheroid().flattening());
        let expected = SYNCHRONOUS_TIDAL_FACTOR * (1.0 - f_free) / (1.0 - f_locked);
        assert!(
            (f_locked / f_free / expected - 1.0).abs() < 1e-9,
            "{f_locked} {f_free}"
        );
        assert!((f_locked / f_free - SYNCHRONOUS_TIDAL_FACTOR).abs() < 0.02);
        let three_to_two = BodyFigure::derive(&FigureInputs {
            state: SpinState::Locked(SpinOrbitResonance::ThreeToTwo),
            ..inputs(1_800.0, rock, PlanetClass::Rocky)
        });
        assert_eq!(three_to_two.law(), FigureLaw::Rotational);
    }
}
