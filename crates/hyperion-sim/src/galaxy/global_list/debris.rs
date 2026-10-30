//! Which debris gets a tube, and the numbered specifications of every stream and dwarf core
//! (plan 10, P10.T3).
//!
//! Only cold debris gets a tube (brainstorm, "Streams and accreted structure"): what was
//! stripped within the stripping time from a progenitor whose pericentre lies outside the bar's
//! corotation, since the integrator has no rotating bar and debris that passes inside it fans out
//! (Design note 2). Three kinds of progenitor give one:
//!
//! - **Living globulars** (P10.T3.a): every globular of the catalogue's walk, with plan 09's
//!   orbit and history (P09.T13) at its bulk velocity. Its tube holds what it lost within the
//!   stripping time at its history's constant rate. Nothing is drawn.
//! - **Orphans** (P10.T3.b, [`orphans`](super::orphans)).
//! - **Recent dwarfs** (P10.T3.c, [`dwarfs`](super::dwarfs)), which also leave the cores.
//!
//! [`Debris::generate`] numbers them as Design note 12 says: the living globulars in the walk's
//! order, the orphans in theirs, then the dwarfs by accretion time, the most recent first, ties
//! by progenitor number; a core takes its progenitor's place among those that keep one, in the
//! same order. When the halo's drawn discrete share is below the physical share, the dwarfs'
//! tube and core masses are scaled down together to meet it, or to nothing where the globular
//! tubes alone pass it ([`share`](super::share), Design note 1); [`dwarfs`](super::dwarfs) itself
//! keeps them unscaled. A tube scaled to nothing is then
//! dropped, and a core scaled under [`CORE_MIN_SYSTEMS`] too, without letting a fifth core take
//! its number. Every figure depends on the potential, so a change to the mass model moves them
//! all.

use crate::coords::GalacticPosition;
use crate::galaxy::Galaxy;
use crate::galaxy::PointLy;
use crate::galaxy::features::FeatureProcess;
use crate::galaxy::features::catalogue::{FeatureCatalogue, FeatureMarks, bulk_velocity};
use crate::galaxy::features::cluster::ClusterModel;
use crate::galaxy::features::interior::counts::{CANONICAL_SLOPE, class_counts};
use crate::galaxy::features::kinds::globular::history;
use crate::units::{LightYears, SolarMassesPerYear};

use super::dwarfs::{CORE_MIN_SYSTEMS, DwarfDebris, dwarfs};
use super::orphans::orphans;
use super::share::{ShareFigures, scale_cold_masses};
use super::spec::{
    ClusterStars, DwarfCoreSpec, MassLoss, SphericalOrbit, StreamSpec, stripping_time,
};
use super::{DwarfCoreNumber, StreamNumber, StreamOrigin};

/// Every stream and dwarf core of one galaxy, numbered (module documentation): what P10.T4's
/// global list is built from.
///
/// Its `Default` holds none, as the empty list's does.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Debris {
    streams: Vec<StreamSpec>,
    cores: Vec<DwarfCoreSpec>,
    cores_beyond_limit: u32,
}

impl Debris {
    /// The debris of `galaxy` (module documentation).
    ///
    /// It walks every globular of the feature catalogue and draws every orphan's orbit: 0.16 s
    /// at the Milky Way's parameters, 0.11–0.24 s for sweep galaxies of 155–370 streams (release,
    /// pod, 4 vCPU); 0.11 s on the Milky Way fixture since an orphan's attempt whose pericentre
    /// misses skips its apocentre (pod, 16 vCPU).
    ///
    /// # Panics
    ///
    /// If `galaxy` holds no kinematic tables: build it with [`Galaxy::with_full_potential`],
    /// since the globulars' and orphans' orbits start from velocities.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use hyperion_sim::Seed;
    /// use hyperion_sim::galaxy::Galaxy;
    /// use hyperion_sim::galaxy::global_list::{Debris, StreamOrigin};
    /// use hyperion_sim::galaxy::params::GalaxyParams;
    ///
    /// let galaxy = Galaxy::from_params(Seed::new(7), GalaxyParams::milky_way_like())?
    ///     .with_full_potential();
    /// let debris = Debris::generate(&galaxy);
    /// // Most globular streams are orphans (Design note 10).
    /// let orphans = debris
    ///     .streams()
    ///     .iter()
    ///     .filter(|s| s.origin() == StreamOrigin::Orphan)
    ///     .count();
    /// assert!(2 * orphans > debris.streams().len());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn generate(galaxy: &Galaxy) -> Self {
        assert!(
            galaxy.kinematics().is_some(),
            "the global list's debris needs the kinematic tables of Galaxy::with_full_potential"
        );
        let living = living_globulars(galaxy);
        let mut dwarfs = dwarfs(galaxy.seed(), galaxy.params(), galaxy.potential());
        in_list_order(&mut dwarfs);
        // Design note 1: when the drawn share is the smaller, the dwarfs' cold masses are scaled
        // down together to meet it, from the share's own figures (P10.T3.d.a).
        let scale = ShareFigures::of(galaxy.params(), galaxy.potential(), &dwarfs).dwarf_scale();
        scale_cold_masses(&mut dwarfs, scale);
        let limit = usize::from(StreamNumber::LIMIT);
        let dwarf_tubes = dwarfs.iter().filter(|d| d.tube.is_some()).count();
        let mut streams = living;
        streams.truncate(limit);
        let room = limit.saturating_sub(streams.len() + dwarf_tubes);
        let living = u32::try_from(streams.len()).expect("the streams are capped at 4,096");
        let orphans = orphans(galaxy, living, room);
        streams.extend(orphans);
        let mut cores = Vec::new();
        let (mut placed, mut cores_beyond_limit) = (0, 0);
        for d in dwarfs {
            // A tube the share scaled to nothing is dropped, as `dwarfs` gives none of no mass.
            if let Some(tube) = d
                .tube
                .filter(|t| t.mass.value() > 0.0 && streams.len() < limit)
            {
                streams.push(tube);
            }
            if let Some(core) = d.core {
                // The first four in the list's order take the core numbers, as the share counts
                // them; one the share scaled under the floor is dropped and leaves its number to
                // none.
                if placed < DwarfCoreNumber::LIMIT {
                    placed += 1;
                    if core.systems >= CORE_MIN_SYSTEMS {
                        cores.push(core);
                    }
                } else {
                    cores_beyond_limit += 1;
                }
            }
        }
        for (n, stream) in (0_u16..).zip(&mut streams) {
            stream.number = StreamNumber::new(n).expect("the streams are capped at the limit");
        }
        for (n, core) in (0_u8..).zip(&mut cores) {
            core.number = DwarfCoreNumber::new(n).expect("the cores are capped at the limit");
        }
        Self {
            streams,
            cores,
            cores_beyond_limit,
        }
    }

    /// The streams, in number order.
    #[must_use]
    pub fn streams(&self) -> &[StreamSpec] {
        &self.streams
    }

    /// The dwarf cores, in number order, at most four.
    #[must_use]
    pub fn cores(&self) -> &[DwarfCoreSpec] {
        &self.cores
    }

    /// The progenitors that met a core's conditions after four cores had been numbered, and so
    /// have none: plan 01's core number has two bits.
    #[must_use]
    pub fn cores_beyond_limit(&self) -> u32 {
        self.cores_beyond_limit
    }
}

/// Puts `dwarfs` in the list's order (Design note 12): by accretion time, the most recent first,
/// ties by progenitor number.
pub(crate) fn in_list_order(dwarfs: &mut [DwarfDebris]) {
    dwarfs.sort_by(|a, b| {
        a.accreted
            .value()
            .total_cmp(&b.accreted.value())
            .then(a.progenitor.cmp(&b.progenitor))
    });
}

/// The living globulars with a tube, in the catalogue's walk order, numbered 0 until the list
/// numbers them (P10.T3.a).
#[must_use]
pub(crate) fn living_globulars(galaxy: &Galaxy) -> Vec<StreamSpec> {
    let tables = galaxy.potential();
    let corotation = tables.bar_corotation().value();
    let last_merger = galaxy.params().accretion().last_major_merger();
    FeatureCatalogue::walk_process(galaxy, FeatureProcess::Globular)
        .filter_map(|feature| {
            let FeatureMarks::Globular(marks) = feature.marks() else {
                return None;
            };
            let velocity = bulk_velocity(galaxy, &feature)?;
            let h = history(
                galaxy,
                feature.position(),
                Some(velocity),
                marks.mass(),
                marks.half_mass_radius(),
                marks.age(),
            );
            if h.pericentre.value() <= corotation {
                return None;
            }
            let orbit = SphericalOrbit::between(tables, h.pericentre, h.apocentre)?;
            let tidal = LightYears::from(
                tables.tidal_radius(marks.mass(), &PointLy::new(h.pericentre.value(), 0.0, 0.0)),
            );
            let window = stripping_time(&orbit, tidal, [last_merger, marks.age()]);
            let mass = h.mass_loss_rate * window.value();
            // As `(mass > 0.0).then(..)` had it: a NaN mass keeps no stream.
            if mass.is_nan() || mass <= 0.0 {
                return None;
            }
            // What the cluster still holds sets what its stream holds (P10.T8.a): plan 09's model
            // of a living globular. A globular with no model is dissolved, which one with a bulk
            // velocity and a tube is not; it would keep nothing, as an orphan, and its stream is
            // kept so that no stream's number moves.
            let (kept, depleted_slope) = ClusterModel::from_record(galaxy, &feature).map_or(
                (0.0, CANONICAL_SLOPE),
                |model| {
                    let counts = class_counts(galaxy, &model, feature.reach());
                    (counts.scale, counts.depleted_slope)
                },
            );
            Some(StreamSpec {
                number: StreamNumber(0),
                origin: StreamOrigin::LivingGlobular(feature.id()),
                position: GalacticPosition::ORIGIN.displacement_to(feature.position()),
                velocity,
                orbit,
                stripping_time: window,
                mass: crate::units::SolarMasses::new(mass),
                progenitor_mass: marks.mass(),
                tidal_radius: tidal,
                mass_loss: MassLoss::Steady(SolarMassesPerYear::new(h.mass_loss_rate)),
                stars: Some(ClusterStars {
                    fe_h: marks.fe_h(),
                    age: marks.age(),
                    kept,
                    depleted_slope,
                }),
            })
        })
        .collect()
}
