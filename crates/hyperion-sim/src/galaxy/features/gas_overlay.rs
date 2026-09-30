//! The gas features add to plan 07's field: superbubbles as holes, clouds and embedded regions as
//! Plummer balls (plan 09, P09.T5).
//!
//! [`FeatureGas`] implements plan 07's [`GasModifierSource`]. For a segment it looks up the
//! catalogue's features whose reach touches it and yields a [`GasModifier::Hole`] for each nursery's
//! superbubble at the evaluation time ([`Superbubble`]: radius and interior density from P09.T4.b),
//! and a [`GasModifier::Cloud`] for each cloud (its Plummer core radius, central density and the
//! site's dust per hydrogen) and each embedded star-forming region (its gas mass in a Plummer ball
//! whose half-mass radius is half its size). Plan 07 integrates them. The supernova shell window of
//! phase 4 reads the smooth field only, never these.
//!
//! What a segment gets is a pure function of the unordered pair of its ends, as the trait asks: the
//! ends are put in a canonical order first, so that `a → b` and `b → a` look up the same features
//! and test them against the same line. Features sit at their epoch positions; their bulk motion,
//! at most some ten light-years over plan 01's source horizon, is not applied.
//!
//! Plan 09 as written has the server pass `FeatureGas` where it passed `NoModifiers`. The server has
//! no such call site yet (plan 07's sight lines are not served), so there is nothing to change
//! there until one exists.

use crate::coords::GalacticPosition;
use crate::galaxy::Galaxy;
use crate::galaxy::gas::modifiers::{GasModifier, GasModifierSource};
use crate::units::consts::METRES_PER_LIGHT_YEAR;
use crate::units::{LightYears, Years};

use super::catalogue::{FeatureCatalogue, FeatureCellCache, FeatureMarks, FeatureRecord};
use super::kinds::cloud::{PLUMMER_HALF_MASS_RATIO, plummer_central_density};
#[cfg(doc)]
use super::kinds::nursery::Superbubble;
use super::kinds::nursery::{NurseryMarks, Superbubble as Bubble};

/// The features' gas at one evaluation time, as a source of plan 07's modifiers (P09.T5).
pub struct FeatureGas<'a> {
    galaxy: &'a Galaxy,
    cache: &'a dyn FeatureCellCache,
    time: Years,
}

impl std::fmt::Debug for FeatureGas<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FeatureGas")
            .field("seed", &self.galaxy.seed())
            .field("time", &self.time)
            .finish_non_exhaustive()
    }
}

impl<'a> FeatureGas<'a> {
    /// The source of `galaxy`'s features `time` after the epoch, looking cells up through `cache`.
    #[must_use]
    pub fn new(galaxy: &'a Galaxy, cache: &'a dyn FeatureCellCache, time: Years) -> Self {
        Self {
            galaxy,
            cache,
            time,
        }
    }

    /// The modifiers of one feature at the source's time, appended to `out`.
    fn feature_modifiers(&self, feature: &FeatureRecord, out: &mut Vec<GasModifier>) {
        let centre = *feature.position();
        match feature.marks() {
            FeatureMarks::Cloud(cloud) => push_valid(
                out,
                GasModifier::Cloud {
                    centre,
                    core_radius: cloud.core_radius(),
                    central_density: cloud.central_density(),
                    dust_per_hydrogen: cloud.dust_per_hydrogen(),
                },
            ),
            FeatureMarks::Nursery(marks) => self.nursery_modifiers(feature, marks, out),
            FeatureMarks::OpenCluster(_) | FeatureMarks::Globular(_) => {}
        }
    }

    fn nursery_modifiers(
        &self,
        feature: &FeatureRecord,
        marks: &NurseryMarks,
        out: &mut Vec<GasModifier>,
    ) {
        let centre = *feature.position();
        let age = marks.age_at_epoch() + self.time;
        let gas = self.galaxy.gas();
        if let Some(bubble) = Bubble::of(
            marks,
            age,
            gas.mean_density(&centre),
            gas.params().neutral_height(),
            self.galaxy.feature_shares().supernova_clock(),
        ) {
            push_valid(
                out,
                GasModifier::Hole {
                    centre,
                    radius: bubble.radius(),
                    interior: bubble.interior_density(),
                },
            );
        }
        let mass = marks.gas_mass_at(age);
        if mass.value() > 0.0 {
            let core = marks.size_at(age) * (0.5 / PLUMMER_HALF_MASS_RATIO);
            push_valid(
                out,
                GasModifier::Cloud {
                    centre,
                    core_radius: core,
                    central_density: plummer_central_density(mass, core),
                    dust_per_hydrogen: gas.dust_per_hydrogen(&centre),
                },
            );
        }
    }
}

/// Pushes `modifier` if it is one plan 07 accepts: every modifier made here is, and the debug
/// build says so.
fn push_valid(out: &mut Vec<GasModifier>, modifier: GasModifier) {
    debug_assert!(
        modifier.is_valid(),
        "an invalid feature gas modifier {modifier:?}"
    );
    if modifier.is_valid() {
        out.push(modifier);
    }
}

impl GasModifierSource for FeatureGas<'_> {
    fn modifiers_near_segment(
        &self,
        a: &GalacticPosition,
        b: &GalacticPosition,
        out: &mut Vec<GasModifier>,
    ) {
        let (a, b) = canonical(a, b);
        let [ax, ay, az] = a.to_light_years_f64();
        let d = a
            .displacement_to(b)
            .metres()
            .map(|m| m / METRES_PER_LIGHT_YEAR);
        let half = 0.5 * (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        let middle =
            GalacticPosition::from_light_years([ax + 0.5 * d[0], ay + 0.5 * d[1], az + 0.5 * d[2]]);
        let Some(middle) = middle else {
            return;
        };
        let features =
            FeatureCatalogue::near(self.galaxy, &middle, LightYears::new(half), self.cache);
        for feature in features {
            if segment_distance(a, &d, feature.position()) <= feature.reach().value() {
                self.feature_modifiers(&feature, out);
            }
        }
    }
}

/// The two ends in a fixed order, by light-year cell and then offset, whichever end was first.
fn canonical<'p>(
    a: &'p GalacticPosition,
    b: &'p GalacticPosition,
) -> (&'p GalacticPosition, &'p GalacticPosition) {
    let key = |p: &GalacticPosition| (p.cell().to_array(), p.offset_metres());
    let (ka, kb) = (key(a), key(b));
    let order = ka.0.cmp(&kb.0).then_with(|| {
        ka.1.iter()
            .zip(&kb.1)
            .map(|(x, y)| x.total_cmp(y))
            .find(|o| o.is_ne())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    if order.is_gt() { (b, a) } else { (a, b) }
}

/// The distance, ly, from `point` to the segment from `start` along `along` (ly).
fn segment_distance(start: &GalacticPosition, along: &[f64; 3], point: &GalacticPosition) -> f64 {
    let offset = start
        .displacement_to(point)
        .metres()
        .map(|m| m / METRES_PER_LIGHT_YEAR);
    let dot = |u: &[f64; 3], v: &[f64; 3]| u[0] * v[0] + u[1] * v[1] + u[2] * v[2];
    let length2 = dot(along, along);
    let t = if length2 > 0.0 {
        (dot(&offset, along) / length2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let miss = [0, 1, 2].map(|k| offset[k] - t * along[k]);
    dot(&miss, &miss).sqrt()
}
