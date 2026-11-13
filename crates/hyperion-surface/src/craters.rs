//! The craters' shared physics: one cumulative crater density and one family of fresh crater
//! shapes for the coarse pass and the synthesis, here because the synthesis runs in this crate and
//! plan 14 calls the density from the sim (plan R09, Design notes 12 and 13).
//!
//! - [`CraterParams`] is a body's crater contract with plan 14, which a coarse field's header
//!   carries: the lunar-equivalent crater density N☾(>1 km), what screens small impactors, the
//!   surface gravity, the target factor, the target's scaling class and density, the projectiles'
//!   density, the system's age and, optionally, a mean impact velocity.
//! - [`cumulative_density`] is the body's crater density N(>D) per square kilometre: Neukum,
//!   Ivanov and Hartmann 2001's lunar production polynomial with a0 = log₁₀ N☾(>1 km), extended
//!   beyond its 10 m–300 km range by its end slopes, carried to the body's own craters by π-group
//!   scaling when [`CraterParams::impact_velocity`] is given, and tapered on the differential
//!   production by the atmosphere's screening. [`production_density`] is the same before
//!   screening, [`own_density_at_1km`] the body's own N(>1 km) and [`saturation`] Trask's
//!   equilibrium cap. [`diameter_in_octave`] inverts the density within a [`DiameterOctave`] by
//!   bisection in log D with a fixed iteration count.
//! - [`transition_diameter`] is the simple-to-complex transition, and [`crater_shape`] a fresh
//!   crater's [`CraterShape`]: its depth, rim, floor and central peak by morphological zone
//!   (`decision-r09-t2.md` item 3, science-r09-craters.md F2), and the width of its
//!   volume-balanced exterior (Design note 13).
//!
//! # Bodies the lunar laws do not describe
//!
//! Every law here is fitted on rocky bodies, mostly the Moon, and reaches other bodies through
//! these inputs alone:
//!
//! - _Gravity_ sets the transition diameter (∝ 1 ÷ g) and so every complex crater's dimensions,
//!   the regolith bowls' size scale, and, with the impact velocity, the diameter map.
//! - _The target_ enters in three ways (science-r09-craters.md items 1–3):
//!   - `k_target` scales the transition diameter: 1 for rock and 0.12 for an ice-rich crust
//!     (Design note 12, after science-r09-craters.md item 1: the morphological transitions of nine
//!     icy bodies give 0.125, 0.06–0.26; Ganymede's 2.1 ± 0.2 km, Schenk et al. 2004; Rhea's
//!     12 ± 2 km, Aponte-Hernández et al. 2021; Ceres's mixed crust behaves as ice). No
//!     temperature term is resolvable between 40 and 150 K. Ganymede's complex depths come out
//!     within 22% and its rims within 5% of Schenk 1991's (`decision-r09-t2.md` item 3).
//!   - Its [`TargetClass`] sets the diameter map's π-group exponents and its bowls: non-porous
//!     targets, rock and ice alike, and porous ones, sand-like regolith or porous ice, whose bowls
//!     are 0.14 D deep.
//!   - Its density enters the map: ice is non-porous at 940 kg m⁻³, so an icy target's crater is
//!     wider than a rocky one's for the same projectile.
//! - _The projectiles_ enter through their density: the screening's projectile scale and the
//!   map's density ratio, 3,000 kg m⁻³ for stony ones and about 500 for comets (P14.T48.f).
//! - _The atmosphere_ enters through [`Screening`]: one taper for every atmosphere, fitted on
//!   Venus's craters (Design note 12), from an airless body's none to any column mass. An
//!   atmosphere far thicker than Venus's only moves the cutoff.
//! - _Time_ enters through N☾(>1 km), which plan 14 takes from the [`lunar_chronology`] counted
//!   from the body's own system's formation, at the contract's system age, and
//!   [`chronology_age`] inverts it for craters' ages (science-r09-craters.md item 5).
//!
//! The strength regime is not modelled: on low-gravity icy bodies the map over-enlarges the
//! craters below Y ÷ (`ρ_t` g), kilometres at a strength Y of about 10⁶ Pa (plan R09, Risks).
//!
//! All arithmetic goes through `hyperion_base::math`, sums run in a fixed order and every
//! quadrature and bisection has a fixed number of nodes or steps, so the server and the client's
//! WebAssembly get the same bits.

pub mod chronology;
pub mod density;
pub mod shape;

pub use chronology::{SOLAR_SYSTEM_AGE, chronology_age, lunar_chronology};
pub use density::{
    BuildDiameterOctaveError, BuildUnitUniformError, DiameterOctave, UnitUniform, crater_cutoff,
    cumulative_density, diameter_in_octave, own_density_at_1km, production_density,
    projectile_scale, saturation,
};
pub use shape::{CraterShape, crater_shape};

use hyperion_base::units::{
    Gigayears, KilogramsPerCubicMetre, KilogramsPerSquareMetre, Metres, MetresPerSecond,
    MetresPerSecondSquared, PerSquareKilometre,
};

/// The Moon's mean impact velocity under P14.T48.f's model, the reference of the diameter map.
///
/// Ivanov 2001's 16.1 km/s, with which he built Mars's function (Ivanov et al. 2002, §5.4), and
/// the debiased 19–20 km/s bracket it (science-r09-craters.md item 4).
pub const LUNAR_IMPACT_VELOCITY: MetresPerSecond = MetresPerSecond::new(16_700.0);

/// The lunar target of the diameter map's reference: non-porous at the lunar highlands crust's bulk
/// density, 2,550 kg m⁻³ at 12% porosity (Wieczorek et al. 2013, Science 339, 671, from GRAIL's
/// gravity; Design note 12,
/// science-r09-craters.md item 2), the rock class's density in P14.T48.f.
pub const LUNAR_TARGET: CraterTarget = CraterTarget {
    class: TargetClass::NonPorous,
    density: KilogramsPerCubicMetre::new(2_550.0),
};

/// The density of the diameter map's reference projectiles and of Design note 12's stony ones,
/// kg m⁻³: 3,000, the value that turns d\* = 1.5 (P ÷ g) ÷ `ρ_p` into its 5.2 m for Earth and
/// 0.52 km for Venus (science-r09-craters.md item 2; P14.T48.f gives it to a rocky source belt's).
pub const LUNAR_PROJECTILE_DENSITY: KilogramsPerCubicMetre = KilogramsPerCubicMetre::new(3_000.0);

/// The Moon's surface gravity as the plan's lunar laws use it, m s⁻²: 1.62 (Design note 12, the
/// figure its transition diameter and its regolith bowls scale from).
const MOON_GRAVITY_M_PER_S2: f64 = 1.62;

/// The Moon's simple-to-complex transition diameter, metres: 19 km (Design note 12, after Pike
/// 1980a; Krüger, Hergarten and Kenkmann 2018 put the lunar zone at 14–17 km to 24–28 km, 0.74–1.47
/// of it).
const MOON_TRANSITION_M: f64 = 19_000.0;

/// The simple-to-complex transition diameter `D_t` of a body: 19 km × (1.62 m s⁻² ÷ g) ×
/// `k_target` (Design note 12; Pike 1980a, Table 3, whose Earth–Moon pair gives g^−1.01).
///
/// It is the centre of a zone, not a line: bowls below 0.8 `D_t`, transitional craters to
/// 1.5 `D_t`, central peaks above, peak rings from about 9 `D_t` and multi-ring basins above about
/// 16 `D_t` ([`Morphology`](crate::field::Morphology)).
///
/// About 19 km on the Moon, 8.3 km on Mercury and Mars, 3.1 km on Earth and 2.6 km on Ganymede
/// (`k_target` 0.12). Susorney et al. 2016 measure 11.7 ± 1.2 km on Mercury and Robbins and Hynek
/// 2012 about 6 km on Mars, the known Mercury–Mars anomaly, which one law in g cannot follow (plan
/// R09, Risks).
#[must_use]
pub fn transition_diameter(p: &CraterParams) -> Metres {
    Metres::new(MOON_TRANSITION_M * (MOON_GRAVITY_M_PER_S2 / p.gravity().value()) * p.k_target())
}

/// What screens a body's small impactors before they crater the ground (Design note 12).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Screening {
    /// Nothing: an airless body.
    None,
    /// An atmosphere, by its column mass P ÷ g: with the contract's projectile density `ρ_p`
    /// ([`CraterParams::projectile_density`]) the projectile scale d\* = 1.5 (P ÷ g) ÷ `ρ_p`
    /// ([`projectile_scale`]), about 5.2 m for Earth, 0.52 km for Venus and 8.5 cm for Mars at
    /// NASA's mean surface pressure of 636 Pa (at 3,000 kg m⁻³), gives the crater cutoff
    /// `D_c` = 20 d\* ([`crater_cutoff`]; Design note 12). A column mass of zero screens nothing,
    /// as [`Screening::None`].
    Atmosphere {
        /// The atmosphere's column mass, the surface pressure over the surface gravity, kg m⁻²:
        /// about 10,300 kg m⁻² on Earth.
        column_mass: KilogramsPerSquareMetre,
    },
    /// A crater cutoff diameter given directly, metres.
    Cutoff {
        /// The cutoff diameter, metres.
        diameter: Metres,
    },
}

crate::field::coded_enum! {
    /// A crater target's scaling class in the gravity regime, one byte, its codes appended only:
    /// the π-group constants of Holsapple 1993 as Singer et al. 2019 (Science 363, 955,
    /// supplement) apply them side by side to non-porous and 30%-porous ice, with ν = 1/3 for
    /// both (science-r09-craters.md item 2).
    ///
    /// Ice is not a class of its own: in the gravity regime "the scaling parameters are similar
    /// for all non-porous materials (e.g., rock versus ice)" (Singer et al. 2019), and "all
    /// non-porous materials have the same gravity regime outcomes as wet sand" (Holsapple 2022,
    /// arXiv 2203.07476, §3.1.4). An icy target differs by its density and its `k_target`.
    TargetClass {
        /// Rock and ice alike: μ 0.55, K₁ 0.095.
        NonPorous = 0,
        /// Dry sand or regolith (Holsapple 1993), 30%-porous ice (Singer et al. 2019) and lunar
        /// regolith (Williams et al. 2014): μ 0.41, K₁ 0.132. Its fresh bowls are 0.14 D deep
        /// ([`crater_shape`]).
        Porous = 1,
    }
}

/// A crater target: its scaling class and its density, which the diameter map reads
/// (science-r09-craters.md item 2).
///
/// Plain data with public fields: [`CraterParams::new`] validates them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CraterTarget {
    /// The target's scaling class.
    pub class: TargetClass,
    /// The target's bulk density, kg m⁻³; finite and positive: 2,550 for a silicate crust and 940
    /// for an ice-rich one (P14.T48.f), or a porous body's bulk density.
    pub density: KilogramsPerCubicMetre,
}

/// The parts of a [`CraterParams`], each in the unit its type states.
///
/// Plain data with public fields: [`CraterParams::new`] validates them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CraterParamsParts {
    /// The lunar-equivalent cumulative crater density above 1 km, N☾(>1 km), per square
    /// kilometre: the Moon's crater density for the body's projectile flux, which is plan 14's
    /// lunar chronology times its belt scaling (Design notes 3 and 12; science-r09-craters.md F1);
    /// finite and non-negative. The body's own N(>1 km) is [`own_density_at_1km`].
    pub n_1km: PerSquareKilometre,
    /// What screens the body's small impactors.
    pub screening: Screening,
    /// The surface gravity, m s⁻²; finite and positive.
    pub gravity: MetresPerSecondSquared,
    /// The target factor `k_target`, dimensionless, which scales the transition diameter: 1 for
    /// rock, 0.12 for an ice-rich crust (Design note 12, sourced by science-r09-craters.md item 1);
    /// finite and positive.
    pub k_target: f64,
    /// The mean impact velocity, m/s, if plan 14 gives one (P14.T48.f always does); finite and
    /// positive.
    pub impact_velocity: Option<MetresPerSecond>,
    /// The projectiles' bulk density, kg m⁻³; finite and positive: 3,000 for a rocky source belt
    /// and 500 for an icy belt or comets (P14.T48.f).
    pub projectile_density: KilogramsPerCubicMetre,
    /// The target: its scaling class and density.
    pub target: CraterTarget,
    /// The age of the body's system, the chronology's clock (science-r09-craters.md item 5),
    /// thousands of millions of years; finite and non-negative.
    pub system_age: Gigayears,
}

/// A body's crater contract with plan 14 (Design note 3's crater contract, Design note 12): the
/// lunar-equivalent density N☾(>1 km), what screens small impactors, the surface gravity, the
/// target's factor, class and density, the projectiles' density, the system's age and, optionally,
/// a mean impact velocity. The surface crate derives the rest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CraterParams {
    parts: CraterParamsParts,
}

/// Why [`CraterParams::new`] refused its parts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildCraterParamsError {
    /// N☾(>1 km) is not finite and non-negative.
    Density(f64),
    /// The gravity is not finite and positive.
    Gravity(f64),
    /// The target factor is not finite and positive.
    TargetFactor(f64),
    /// The impact velocity is not finite and positive.
    ImpactVelocity(f64),
    /// The projectiles' density is not finite and positive.
    ProjectileDensity(f64),
    /// The target's density is not finite and positive.
    TargetDensity(f64),
    /// The system's age is not finite and non-negative.
    SystemAge(f64),
    /// The diameter map's cavity scale, carried here, is not finite and positive: parts each in
    /// range whose combination lies beyond the π-group scaling's, such as a density ratio that
    /// underflows while the impact speed's square overflows.
    DiameterMap(f64),
    /// The screening's column mass is not finite and non-negative, or its cutoff not finite and
    /// positive; or the crater cutoff it gives is out of the density's range, under about
    /// 10⁻³²⁰ m or beyond about 10³⁰⁵ m.
    Screening(Screening),
}

impl std::fmt::Display for BuildCraterParamsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Density(n) => write!(
                f,
                "lunar-equivalent crater density N(>1 km) {n} km⁻² is not finite and non-negative"
            ),
            Self::Gravity(g) => write!(f, "gravity {g} m s⁻² is not finite and positive"),
            Self::TargetFactor(k) => write!(f, "target factor {k} is not finite and positive"),
            Self::ImpactVelocity(v) => {
                write!(f, "impact velocity {v} m/s is not finite and positive")
            }
            Self::ProjectileDensity(rho) => {
                write!(
                    f,
                    "projectile density {rho} kg m⁻³ is not finite and positive"
                )
            }
            Self::TargetDensity(rho) => {
                write!(f, "target density {rho} kg m⁻³ is not finite and positive")
            }
            Self::SystemAge(a) => write!(f, "system age {a} Gyr is not finite and non-negative"),
            Self::DiameterMap(scale) => write!(
                f,
                "the diameter map's cavity scale {scale} is not finite and positive"
            ),
            Self::Screening(s) => write!(f, "screening {s:?} is out of range"),
        }
    }
}

impl std::error::Error for BuildCraterParamsError {}

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

impl CraterParams {
    /// The contract of `parts`.
    ///
    /// # Errors
    ///
    /// [`BuildCraterParamsError`] if the density or the system's age is not finite and
    /// non-negative, the gravity, the target factor, the velocity, the projectiles' density or
    /// the target's is not finite and positive, the screening's parts are out of range, or the
    /// diameter map they make is not finite.
    pub fn new(parts: CraterParamsParts) -> Result<Self, BuildCraterParamsError> {
        let n = parts.n_1km.value();
        if !non_negative(n) {
            return Err(BuildCraterParamsError::Density(n));
        }
        if !positive(parts.gravity.value()) {
            return Err(BuildCraterParamsError::Gravity(parts.gravity.value()));
        }
        if !positive(parts.k_target) {
            return Err(BuildCraterParamsError::TargetFactor(parts.k_target));
        }
        if let Some(v) = parts.impact_velocity
            && !positive(v.value())
        {
            return Err(BuildCraterParamsError::ImpactVelocity(v.value()));
        }
        let projectile = parts.projectile_density.value();
        if !positive(projectile) {
            return Err(BuildCraterParamsError::ProjectileDensity(projectile));
        }
        if !positive(parts.target.density.value()) {
            return Err(BuildCraterParamsError::TargetDensity(
                parts.target.density.value(),
            ));
        }
        if !non_negative(parts.system_age.value()) {
            return Err(BuildCraterParamsError::SystemAge(parts.system_age.value()));
        }
        let screening_ok = match parts.screening {
            Screening::None => true,
            Screening::Atmosphere { column_mass } => non_negative(column_mass.value()),
            Screening::Cutoff { diameter } => positive(diameter.value()),
        };
        if !(screening_ok && density::screening_in_range(parts.screening, parts.projectile_density))
        {
            return Err(BuildCraterParamsError::Screening(parts.screening));
        }
        let params = Self { parts };
        if let Some(scale) = density::map_out_of_range(&params) {
            return Err(BuildCraterParamsError::DiameterMap(scale));
        }
        Ok(params)
    }

    /// The validated parts.
    #[must_use]
    pub const fn parts(&self) -> &CraterParamsParts {
        &self.parts
    }

    /// The lunar-equivalent cumulative crater density above 1 km, N☾(>1 km), per square kilometre:
    /// the production polynomial's a0 as log₁₀ of it (Design note 12), with plan 14's belt
    /// scaling already in it. The body's own N(>1 km) is [`own_density_at_1km`], which is this at
    /// the lunar reference.
    #[must_use]
    pub const fn n_1km(&self) -> PerSquareKilometre {
        self.parts.n_1km
    }

    /// What screens the body's small impactors.
    #[must_use]
    pub const fn screening(&self) -> Screening {
        self.parts.screening
    }

    /// The surface gravity, m s⁻².
    #[must_use]
    pub const fn gravity(&self) -> MetresPerSecondSquared {
        self.parts.gravity
    }

    /// The target factor `k_target`, dimensionless: 1 for rock, 0.12 for an ice-rich crust (Design
    /// note 12, sourced by science-r09-craters.md item 1), which scales the
    /// [`transition_diameter`].
    #[must_use]
    pub const fn k_target(&self) -> f64 {
        self.parts.k_target
    }

    /// The mean impact velocity, m/s, if plan 14 gives one: it switches on the π-group diameter map
    /// from the Moon's craters to the body's ([`cumulative_density`]), which is the identity
    /// without it, and at the lunar reference ([`LUNAR_IMPACT_VELOCITY`], [`LUNAR_TARGET`],
    /// [`LUNAR_PROJECTILE_DENSITY`], g 1.62, `D_t` 19 km; Design note 12).
    #[must_use]
    pub const fn impact_velocity(&self) -> Option<MetresPerSecond> {
        self.parts.impact_velocity
    }

    /// The projectiles' bulk density, kg m⁻³, which sets the screening's projectile scale and
    /// enters the diameter map.
    #[must_use]
    pub const fn projectile_density(&self) -> KilogramsPerCubicMetre {
        self.parts.projectile_density
    }

    /// The target: its scaling class and density.
    #[must_use]
    pub const fn target(&self) -> CraterTarget {
        self.parts.target
    }

    /// The age of the body's system, the chronology's clock ([`lunar_chronology`],
    /// [`chronology_age`]).
    #[must_use]
    pub const fn system_age(&self) -> Gigayears {
        self.parts.system_age
    }
}

#[cfg(test)]
mod golden;

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    fn lunar() -> CraterParamsParts {
        CraterParamsParts {
            n_1km: PerSquareKilometre::new(0.018),
            screening: Screening::None,
            gravity: MetresPerSecondSquared::new(1.62),
            k_target: 1.0,
            impact_velocity: None,
            projectile_density: LUNAR_PROJECTILE_DENSITY,
            target: LUNAR_TARGET,
            system_age: SOLAR_SYSTEM_AGE,
        }
    }

    #[test]
    fn crater_params_refuse_parts_out_of_range() {
        let ok = CraterParams::new(lunar()).unwrap();
        assert_eq!(ok.parts(), &lunar());
        assert_eq!(ok.n_1km(), PerSquareKilometre::new(0.018));
        assert_eq!(ok.screening(), Screening::None);
        assert_eq!(ok.projectile_density(), LUNAR_PROJECTILE_DENSITY);
        assert_eq!(ok.target(), LUNAR_TARGET);
        assert_eq!(ok.system_age(), SOLAR_SYSTEM_AGE);
        let refuse = |edit: fn(&mut CraterParamsParts)| {
            let mut parts = lunar();
            edit(&mut parts);
            CraterParams::new(parts).unwrap_err()
        };
        assert_eq!(
            refuse(|p| p.n_1km = PerSquareKilometre::new(-1.0)),
            BuildCraterParamsError::Density(-1.0)
        );
        assert_eq!(
            refuse(|p| p.gravity = MetresPerSecondSquared::ZERO),
            BuildCraterParamsError::Gravity(0.0)
        );
        assert_eq!(
            refuse(|p| p.k_target = 0.0),
            BuildCraterParamsError::TargetFactor(0.0)
        );
        assert_eq!(
            refuse(|p| p.impact_velocity = Some(MetresPerSecond::new(-1.0))),
            BuildCraterParamsError::ImpactVelocity(-1.0)
        );
        assert_eq!(
            refuse(|p| p.projectile_density = KilogramsPerCubicMetre::ZERO),
            BuildCraterParamsError::ProjectileDensity(0.0)
        );
        assert_eq!(
            refuse(|p| p.target.density = KilogramsPerCubicMetre::new(-2.0)),
            BuildCraterParamsError::TargetDensity(-2.0)
        );
        assert_eq!(
            refuse(|p| p.system_age = Gigayears::new(-0.5)),
            BuildCraterParamsError::SystemAge(-0.5)
        );
        assert!(matches!(
            refuse(|p| p.system_age = Gigayears::new(f64::NAN)),
            BuildCraterParamsError::SystemAge(a) if a.is_nan()
        ));
        // A system of age zero holds a surface of age zero.
        let mut newborn = lunar();
        newborn.system_age = Gigayears::ZERO;
        assert!(CraterParams::new(newborn).is_ok());
        for bad in [
            Screening::Atmosphere {
                column_mass: KilogramsPerSquareMetre::new(-164.0),
            },
            Screening::Atmosphere {
                column_mass: KilogramsPerSquareMetre::new(f64::INFINITY),
            },
            Screening::Cutoff {
                diameter: Metres::ZERO,
            },
        ] {
            let mut parts = lunar();
            parts.screening = bad;
            assert_eq!(
                CraterParams::new(parts),
                Err(BuildCraterParamsError::Screening(bad))
            );
        }
        // A column so heavy over projectiles so light that its cutoff overflows the quadrature's
        // reach is refused; the same column over stony projectiles is not.
        let heavy = Screening::Atmosphere {
            column_mass: KilogramsPerSquareMetre::new(1e300),
        };
        let mut parts = lunar();
        parts.screening = heavy;
        parts.projectile_density = KilogramsPerCubicMetre::new(1e-10);
        assert_eq!(
            CraterParams::new(parts),
            Err(BuildCraterParamsError::Screening(heavy))
        );
        parts.projectile_density = KilogramsPerCubicMetre::new(1e10);
        assert!(CraterParams::new(parts).is_ok());
        // Each part in range, but a density ratio of 10⁻⁴⁰⁰ times a speed's square of 10³¹⁰ makes
        // a cavity scale of 0 × ∞: refused rather than carried into the density as a NaN.
        let mut extreme = lunar();
        extreme.impact_velocity = Some(MetresPerSecond::new(1e155));
        extreme.projectile_density = KilogramsPerCubicMetre::new(1e-200);
        extreme.target.density = KilogramsPerCubicMetre::new(1e200);
        assert!(matches!(
            CraterParams::new(extreme),
            Err(BuildCraterParamsError::DiameterMap(scale)) if !scale.is_finite()
        ));
    }

    #[test]
    fn target_classes_have_their_codes() {
        assert_eq!(
            TargetClass::ALL,
            &[TargetClass::NonPorous, TargetClass::Porous]
        );
        assert_eq!(u8::from(TargetClass::NonPorous), 0);
        assert_eq!(u8::from(TargetClass::Porous), 1);
        assert_eq!(TargetClass::try_from(1), Ok(TargetClass::Porous));
        assert_eq!(
            TargetClass::try_from(2),
            Err(crate::field::DecodeFieldCodeError {
                kind: "TargetClass",
                code: 2
            })
        );
    }
}
