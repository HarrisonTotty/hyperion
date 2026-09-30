//! The galactic centre as a feature of its own: the nuclear cluster's profile about the central
//! black hole, its distribution functions, the marks that flatten and turn it, and its classes
//! (plan 09, phase 6, P09.T24–T26).
//!
//! The cluster is a budget of its own (Design note 5): its law and its mass inside the reach are
//! plan 02's [`NuclearClusterParams`](crate::galaxy::params::NuclearClusterParams), and its black
//! hole is the galaxy's. [`CentreProfile`] holds the stars' profile and the potential the members
//! move in, the black hole's, the cluster's and the rest of the galaxy's spherical average
//! ([`profile`]); one [`DistributionFunction`] per distinct profile (stars, black holes, the young
//! clockwise disc and the isotropic young stars: Design note 14's three and ruling 144.6's split
//! of the young) is found by Eddington inversion in that potential ([`df`]);
//! [`OrbitMarks`] and [`LossCone`] thin the candidates after their velocity ([`marks`]); and
//! [`CentreClasses`] says what the cluster holds and on which profile ([`classes`]).
//!
//! [`members`] places the members on the twelve-level grid under the centre's member IDs
//! (P09.T27), and plan 03's `resolve` dispatches a centre ID there; nothing else reads the centre
//! until the Kepler regime (P09.T28.b) and its range query (P09.T29).

pub mod classes;
pub mod df;
pub mod marks;
pub mod members;
pub mod profile;
#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use classes::{CentreClass, CentreClassKind, CentreClasses, CentreTracer};
pub use df::{DistributionFunction, EnergyGrid};
pub use marks::{LossCone, OrbitMarks};
pub use members::ExceedCentreIndexError;
pub use profile::{BuildCentreError, CentreProfile, SlopeBreak, TracerProfile, TracerShape};

use crate::galaxy::Galaxy;
use crate::galaxy::imf::MassFunction;
use crate::galaxy::params::GalaxyParams;
use crate::units::{LightYears, SolarMasses};

/// The galactic centre: its profile, its four distribution functions and their marks, its loss
/// cone and its classes (module documentation).
///
/// # Examples
///
/// ```no_run
/// use hyperion_sim::Seed;
/// use hyperion_sim::galaxy::Galaxy;
/// use hyperion_sim::galaxy::features::centre::{CentreModel, CentreTracer};
///
/// let galaxy = Galaxy::new(Seed::new(1));
/// let centre = CentreModel::new(&galaxy)?;
/// // The black holes are more concentrated than the stars.
/// let inside = |t| centre.distribution(t).fraction_within(3.0);
/// assert!(inside(CentreTracer::BlackHoles) > inside(CentreTracer::Stars));
/// # Ok::<(), hyperion_sim::galaxy::features::centre::BuildCentreError>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct CentreModel {
    profile: CentreProfile,
    tracers: [TracerProfile; 4],
    distributions: [DistributionFunction; 4],
    marks: [OrbitMarks; 4],
    loss_cone: LossCone,
    acceptance: [f64; 4],
    classes: CentreClasses,
}

impl CentreModel {
    /// The centre of `galaxy`.
    ///
    /// # Errors
    ///
    /// [`BuildCentreError`] if the black hole's or the cluster's mass is not valid, or a profile
    /// has no isotropic distribution function in the potential (P09.T24.b), which no galaxy the
    /// builder allows does (`no_drawn_centre_fails_its_inversion`).
    pub fn new(galaxy: &Galaxy) -> Result<Self, BuildCentreError> {
        let params = galaxy.params();
        let profile = CentreProfile::new(params.nuclear_cluster(), params.black_hole().mass())?
            .in_galaxy(galaxy.mass_model());
        Self::from_parts(profile, galaxy.mass_function())
    }

    /// The centre of a galaxy of `params`, without building the galaxy: what [`new`](Self::new)
    /// builds for a galaxy of these parameters, bit for bit, since both read the parameters' mass
    /// function and build the same mass model (which this builds, and no potential tables).
    ///
    /// # Errors
    ///
    /// As [`new`](Self::new).
    pub fn from_params(params: &GalaxyParams) -> Result<Self, BuildCentreError> {
        let imf = params.mass_function().to_mass_function();
        Self::from_parts(CentreProfile::from_params(params)?, imf.as_ref())
    }

    fn from_parts(
        profile: CentreProfile,
        mass_function: &dyn MassFunction,
    ) -> Result<Self, BuildCentreError> {
        let tracers = Self::tracers(&profile);
        let distributions = Self::invert_tracers(&profile, &tracers)?;
        let loss_cone = LossCone::new(profile.black_hole());
        let orbit_marks = CentreTracer::ALL.map(CentreTracer::marks);
        let acceptance = std::array::from_fn(|i| {
            marks::mean_acceptance(&orbit_marks[i], &loss_cone, &distributions[i], &profile)
        });
        let classes = CentreClasses::new(&profile, mass_function);
        Ok(Self {
            profile,
            tracers,
            distributions,
            marks: orbit_marks,
            loss_cone,
            acceptance,
            classes,
        })
    }

    /// The four distribution functions of `profile`, in [`CentreTracer::ALL`]'s order: the
    /// inversions Design note 14 benchmarks together.
    ///
    /// # Errors
    ///
    /// [`BuildCentreError::NegativeDistribution`] if a profile has none.
    pub fn invert(profile: &CentreProfile) -> Result<[DistributionFunction; 4], BuildCentreError> {
        Self::invert_tracers(profile, &Self::tracers(profile))
    }

    /// The four tracers' profiles, in [`CentreTracer::ALL`]'s order.
    fn tracers(profile: &CentreProfile) -> [TracerProfile; 4] {
        CentreTracer::ALL.map(|t| t.profile(profile.stars()))
    }

    /// The distribution functions of `tracers` on one grid of `profile`'s potential.
    fn invert_tracers(
        profile: &CentreProfile,
        tracers: &[TracerProfile; 4],
    ) -> Result<[DistributionFunction; 4], BuildCentreError> {
        let grid = EnergyGrid::new(profile);
        let on = |tracer: &TracerProfile| DistributionFunction::invert_on(&grid, tracer, profile);
        Ok([
            on(&tracers[0])?,
            on(&tracers[1])?,
            on(&tracers[2])?,
            on(&tracers[3])?,
        ])
    }

    /// The profile of `tracer`, normalised to one.
    #[must_use]
    pub fn tracer_profile(&self, tracer: CentreTracer) -> &TracerProfile {
        &self.tracers[tracer.index()]
    }

    /// The marks of `tracer`'s members.
    #[must_use]
    pub fn marks(&self, tracer: CentreTracer) -> &OrbitMarks {
        &self.marks[tracer.index()]
    }

    /// The stars' profile and the potential.
    #[must_use]
    pub fn profile(&self) -> &CentreProfile {
        &self.profile
    }

    /// The distribution function of `tracer`.
    #[must_use]
    pub fn distribution(&self, tracer: CentreTracer) -> &DistributionFunction {
        &self.distributions[tracer.index()]
    }

    /// The share of `tracer`'s candidates its marks accept, which divides its placement density
    /// (Design note 13).
    #[must_use]
    pub fn acceptance(&self, tracer: CentreTracer) -> f64 {
        self.acceptance[tracer.index()]
    }

    /// The loss cone.
    #[must_use]
    pub fn loss_cone(&self) -> &LossCone {
        &self.loss_cone
    }

    /// Its radius, about 2 au at the Milky Way's black hole.
    #[must_use]
    pub fn loss_cone_radius(&self) -> LightYears {
        self.loss_cone.radius()
    }

    /// The classes.
    #[must_use]
    pub fn classes(&self) -> &CentreClasses {
        &self.classes
    }

    /// The mass inside `r` ly, the black hole's and the stars' (not the rest of the galaxy's).
    #[must_use]
    pub fn enclosed_mass(&self, r: LightYears) -> SolarMasses {
        self.profile.enclosed_mass(r.value())
    }

    /// Where the stars inside weigh as much as the black hole.
    #[must_use]
    pub fn influence_radius(&self) -> LightYears {
        self.profile.influence_radius()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GENERATOR_VERSION;
    use crate::Seed;
    use crate::id::SystemId;
    use crate::rng::{ObjectKey, Stream, tags};
    use hyperion_testkit::golden;
    use hyperion_testkit::golden::GoldenWriter;

    /// The Milky Way centre's profile, potential, distribution functions, marks, a few drawn
    /// velocities and its classes, as float bits: the pieces most exposed to a platform
    /// difference (plan 09, Verification).
    #[test]
    #[expect(clippy::many_single_char_names, reason = "the golden's short locals")]
    fn centre_golden() {
        let model = testing::milky_way_centre();
        let p = model.profile();
        let mut w = GoldenWriter::new();
        w.header(GENERATOR_VERSION.get());
        for r in [1e-5, 1e-3, 0.1, 3.0, 10.0, 128.0] {
            w.f64(&format!("psi({r})"), p.psi(r));
            w.f64(&format!("mass({r})"), p.enclosed_mass(r).value());
        }
        w.f64("influence radius", model.influence_radius().value());
        w.f64("loss cone", model.loss_cone_radius().value());
        for tracer in CentreTracer::ALL {
            let df = model.distribution(tracer);
            let n = df.values().len();
            for j in [0, n / 5, 2 * n / 5, 3 * n / 5, 4 * n / 5, n - 1] {
                w.f64(&format!("{tracer:?} f[{j}]"), df.values()[j]);
            }
            for r in [1e-5, 0.01, 1.0, 30.0] {
                w.f64(&format!("{tracer:?} density({r})"), df.density(r));
            }
            w.f64(&format!("{tracer:?} acceptance"), model.acceptance(tracer));
            w.f64(
                &format!("{tracer:?} peak factor"),
                model.marks(tracer).peak_factor(),
            );
            for n in 0..4_u64 {
                let position = [0.3, -0.2 * f64::from(u32::try_from(n).unwrap()), 0.1];
                // Feature-level members 1–4 of the centre stand in for P09.T27's members.
                let key = ObjectKey::from(SystemId::from_raw(0xF000_0007_0000_0001 + n).unwrap());
                let mut s = Stream::open(Seed::new(9), tags::CENTRE_VELOCITY, key);
                let v = df.draw_velocity(p, position, &mut s).unwrap();
                let mut m = Stream::open(Seed::new(9), tags::CENTRE_MARKS, key);
                let kept = model
                    .marks(tracer)
                    .apply(model.loss_cone(), position, v, &mut m);
                for (axis, c) in ["x", "y", "z"].iter().zip(v) {
                    w.f64(&format!("{tracer:?} v{n}.{axis}"), c);
                }
                w.line(&format!("{tracer:?} v{n} kept {}", kept.is_some()));
            }
        }
        for c in model.classes().classes() {
            w.f64(
                &format!("class {:?} {:?} component {}", c.band, c.kind, c.component),
                c.expected,
            );
        }
        golden!("galaxy/features/centre", w.as_str());
    }

    /// P09.T24.b and ruling 144.1: no drawn galaxy's centre fails its inversions (f positive at
    /// every node, for every tracer), over 64 seeds; nor does a centre at the extreme black hole
    /// to cluster ratios the draws allow in practice, in the Milky Way's potential: black holes
    /// 1.5 dex either side of the Milky Way's (±4σ of the M–σ scatter's 0.38 dex), about clusters
    /// from 10⁶ M☉ to plan 02's cap (ruling 144.5b).
    #[test]
    fn no_drawn_centre_fails_its_inversion() {
        use crate::galaxy::imf::MassFunctionKind;
        use crate::galaxy::params::NuclearClusterParams;
        use crate::galaxy::potential::MassModel;
        for n in 0..64_u64 {
            let params =
                GalaxyParams::from_seed(Seed::new(0x0924_0000 | n), MassFunctionKind::default());
            let profile = CentreProfile::from_params(&params).unwrap();
            if let Err(e) = CentreModel::invert(&profile) {
                panic!("seed {n}: {e}");
            }
        }
        let model = MassModel::new(&GalaxyParams::milky_way_like());
        let cap = NuclearClusterParams::mass_cap(MassFunctionKind::Chabrier).value();
        for cluster in [1e6, 2.5e7, cap] {
            for dex in [-1.5, -0.76, 0.76, 1.5] {
                let black_hole = 4.3e6 * crate::math::exp10(dex);
                let profile = CentreProfile::from_shape(
                    TracerShape::nuclear_cluster(),
                    SolarMasses::new(cluster),
                    SolarMasses::new(black_hole),
                )
                .unwrap()
                .in_galaxy(&model);
                if let Err(e) = CentreModel::invert(&profile) {
                    panic!("cluster {cluster:e}, black hole {black_hole:e}: {e}");
                }
            }
        }
    }
}
