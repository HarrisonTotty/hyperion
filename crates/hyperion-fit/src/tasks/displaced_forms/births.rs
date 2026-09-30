//! The births of the displaced form table's orbits (plan 15, P15.T6.b): where each kicked object
//! starts, how fast it moves before its kick, the kick, and how long ago.
//!
//! - **Where.** The thin-disc source is born in the thin birth layer: the young disc's envelope
//!   without arms, on the thin disc's radial profile with its central hole, `Σ ∝ exp(−R_h ÷ R −
//!   R ÷ R_d)`, and the young disc's own cored vertical profile (plan 08, Design note 14: the
//!   forms know nothing of the sub-discs' heights). Every other source is born from its
//!   population's density in the sim's fields, summed over the population's components (the
//!   halo's mixture, for one), by exact rejection on logarithmic cells of the root cube's octant
//!   ([`CellSampler`]): each component's envelope never rises with |x|, |y| or |z| (plan 02), so
//!   its value at a cell's nearest corner bounds it over the cell.
//! - **How fast.** The disc-born (thin, thick and nuclear disc) start on the circular velocity at
//!   their radius; the halo-, bulge- and bar-born draw their velocity from plan 08's velocity
//!   ellipsoid of the component they were born in ([`KinematicTables::ellipsoid`]). P15.T6.b
//!   named an isotropic Jeans dispersion for these, since plan 08 did not exist at the first run;
//!   it does, and T6.e's check with plan 08's laws is then this run itself.
//! - **The kick.** Speeds log-uniform in `u` (speed over `v_c`, or over the nuclear disc's own
//!   circular speed for its source) within the class's speed bin, the bins together spanning 0.02
//!   to 6 so that every bin fills whatever the law; directions isotropic. The unkicked control of
//!   a barred source has no kick.
//! - **How long ago.** The thin source's time since death is drawn from the thin disc's declining
//!   formation history (the young disc and the sub-discs, by their counts and age distributions)
//!   restricted to the class's age bin; the last bin runs to the oldest thin star. The old
//!   sources, whose forms have no age bins, draw it from the whole of their population's history.
//!   A massive star dies within about 40 Myr of its birth, so the time since death is taken as the
//!   age.
//!
//! Every draw comes from a counter-based generator keyed by the run's seed, the class and the
//! orbit's number within it ([`Draws`]), so an orbit is a pure function of its number.

use hyperion_sim::coords::ROOT_HALF_WIDTH_LY;
use hyperion_sim::galaxy::fields::VerticalProfile;
use hyperion_sim::galaxy::fields::disc::{RadialProfile, THIN_DISC_HOLE_LENGTHS};
use hyperion_sim::galaxy::fields::{Component, ComponentId, Fields};
use hyperion_sim::galaxy::kinematics::{EllipsoidAxes, KinematicTables};
use hyperion_sim::galaxy::{PointLy, Population};
use hyperion_sim::math;
use hyperion_sim::units::{KilometresPerSecond, Years};

/// A counter-based generator: `SplitMix64`'s finaliser (Steele, Lea and Flood 2014) over a key made
/// of the run's seed, the class and the orbit's number, and a counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Draws {
    key: u64,
    counter: u64,
}

impl Draws {
    /// The draws of orbit `orbit` of class `class` in the run of `seed`.
    #[must_use]
    pub fn new(seed: u64, class: u64, orbit: u64) -> Self {
        let key = mix(seed ^ mix(class.wrapping_add(0x6a09_e667_f3bc_c908)) ^ mix(orbit));
        Self { key, counter: 0 }
    }

    /// The next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.counter = self.counter.wrapping_add(1);
        mix(self.key ^ self.counter.wrapping_mul(0x9e37_79b9_7f4a_7c15))
    }

    /// A uniform in the open interval (0, 1): 53 bits and a half.
    pub fn uniform(&mut self) -> f64 {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a 53-bit integer is exact in an f64"
        )]
        let k = (self.next_u64() >> 11) as f64;
        (k + 0.5) / 9_007_199_254_740_992.0
    }

    /// A standard normal, by the inverse of its CDF.
    pub fn normal(&mut self) -> f64 {
        math::normal_quantile(self.uniform())
    }

    /// An isotropic unit vector.
    pub fn direction(&mut self) -> [f64; 3] {
        let cos_theta = 2.0 * self.uniform() - 1.0;
        let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
        let (s, c) = math::sin_cos(2.0 * core::f64::consts::PI * self.uniform());
        [sin_theta * c, sin_theta * s, cos_theta]
    }
}

/// `SplitMix64`'s finaliser.
fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// The per-axis cell edges of [`CellSampler`], ly: 0, then 2⁻² to 2¹⁶ doubling.
fn axis_edges() -> Vec<f64> {
    core::iter::once(0.0)
        .chain((-2..=16).map(|k| math::powi(2.0, k)))
        .collect()
}

/// An exact sampler of a population's density over the root cube (module documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct CellSampler {
    components: Vec<ComponentId>,
    edges: Vec<f64>,
    /// The cumulative bound mass of the octant's cells, in order `(i, j, k)`.
    cumulative: Vec<f64>,
    /// The bound density at each cell's nearest corner.
    corner: Vec<f64>,
}

impl CellSampler {
    /// The sampler of `population` in `fields`.
    ///
    /// # Panics
    ///
    /// If the population has no component in `fields`.
    #[must_use]
    pub fn new(fields: &Fields, population: Population) -> Self {
        let components: Vec<ComponentId> = fields
            .component_ids()
            .filter(|&id| fields.component(id).population() == population)
            .collect();
        assert!(!components.is_empty(), "{population:?} has no component");
        let edges = axis_edges();
        let n = edges.len() - 1;
        let mut cumulative = Vec::with_capacity(n * n * n);
        let mut corner = Vec::with_capacity(n * n * n);
        let mut total = 0.0;
        for i in 0..n {
            for j in 0..n {
                for k in 0..n {
                    let p = PointLy::new(edges[i], edges[j], edges[k]);
                    let rho: f64 = components
                        .iter()
                        .map(|&id| fields.component(id).envelope(&p))
                        .sum();
                    let volume = (edges[i + 1] - edges[i])
                        * (edges[j + 1] - edges[j])
                        * (edges[k + 1] - edges[k]);
                    total += rho * volume;
                    cumulative.push(total);
                    corner.push(rho);
                }
            }
        }
        Self {
            components,
            edges,
            cumulative,
            corner,
        }
    }

    /// A point drawn from the population's density inside the cube, and the component it was born
    /// in (by the components' shares of the density there).
    pub fn sample(&self, fields: &Fields, draws: &mut Draws) -> (PointLy, ComponentId) {
        let per_axis = self.edges.len() - 1;
        let total = self.cumulative[self.cumulative.len() - 1];
        loop {
            let target = draws.uniform() * total;
            let cell = self
                .cumulative
                .partition_point(|&c| c <= target)
                .min(per_axis * per_axis * per_axis - 1);
            let axes = [
                cell / (per_axis * per_axis),
                (cell / per_axis) % per_axis,
                cell % per_axis,
            ];
            let [x, y, z] =
                axes.map(|a| self.edges[a] + draws.uniform() * (self.edges[a + 1] - self.edges[a]));
            let p = PointLy::new(x, y, z);
            let each: Vec<f64> = self
                .components
                .iter()
                .map(|&id| fields.component(id).envelope(&p))
                .collect();
            let rho: f64 = each.iter().sum();
            if draws.uniform() * self.corner[cell] >= rho {
                continue;
            }
            let sign = |d: &mut Draws, v: f64| if d.uniform() < 0.5 { -v } else { v };
            let site = PointLy::new(sign(draws, p.x), sign(draws, p.y), sign(draws, p.z));
            let pick = draws.uniform() * rho;
            let mut running = 0.0;
            let mut born = self.components[self.components.len() - 1];
            for (&id, &d) in self.components.iter().zip(&each) {
                running += d;
                if pick < running {
                    born = id;
                    break;
                }
            }
            return (site, born);
        }
    }
}

/// The thin birth layer: the thin disc's holed radial profile and the young disc's vertical
/// profile (module documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct ThinBirths {
    /// `R_d`, ly.
    length: f64,
    /// `R_h`, ly.
    hole: f64,
    profile: VerticalProfile,
}

impl ThinBirths {
    /// The birth layer of `fields`, whose thin disc has the scale length `length` (ly).
    ///
    /// # Panics
    ///
    /// If `fields` has no young disc.
    #[must_use]
    pub fn new(fields: &Fields, length: f64) -> Self {
        let young = young_component(fields);
        let profile = match young.shape() {
            hyperion_sim::galaxy::fields::Shape::Disc(disc) => {
                debug_assert!(matches!(disc.radial(), RadialProfile::Holed { .. }));
                disc.profile().clone()
            }
            _ => unreachable!("the young disc is a disc"),
        };
        Self {
            length,
            hole: THIN_DISC_HOLE_LENGTHS * length,
            profile,
        }
    }

    /// A birth site, in the cube.
    pub fn sample(&self, draws: &mut Draws) -> PointLy {
        let half = f64::from(ROOT_HALF_WIDTH_LY);
        let column = self.profile.effective_height().value();
        loop {
            // R ∝ R e^(−R ÷ R_d), a gamma of shape 2, thinned by the hole's e^(−R_h ÷ R).
            let radius = -self.length * math::ln(draws.uniform() * draws.uniform());
            if draws.uniform() >= math::exp(-self.hole / radius) {
                continue;
            }
            let target = draws.uniform() * column;
            let height = invert(|h| self.profile.integral_to(h).value(), target, column);
            let height = if draws.uniform() < 0.5 {
                -height
            } else {
                height
            };
            let (sin, cos) = math::sin_cos(2.0 * core::f64::consts::PI * draws.uniform());
            let site = PointLy::new(radius * cos, radius * sin, height);
            if site.x.abs() <= half && site.y.abs() <= half && site.z.abs() <= half {
                return site;
            }
        }
    }
}

/// The height at which the non-decreasing `f` reaches `target`, by bisection on `[0, 64 h]`.
fn invert(f: impl Fn(f64) -> f64, target: f64, h: f64) -> f64 {
    let (mut lo, mut hi) = (0.0, 64.0 * h);
    for _ in 0..60 {
        let mid = f64::midpoint(lo, hi);
        if f(mid) < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    f64::midpoint(lo, hi)
}

/// The young disc's component.
fn young_component(fields: &Fields) -> &Component {
    fields
        .components()
        .iter()
        .find(|c| c.population() == Population::YoungThinDisc)
        .expect("every galaxy has a young disc")
}

/// The thin disc's formation history as a mixture of its components' age distributions,
/// weighted by their counts (module documentation, "How long ago").
#[derive(Debug, Clone, PartialEq)]
pub struct ThinHistory {
    components: Vec<(f64, ComponentId)>,
}

impl ThinHistory {
    /// The thin disc's history in `fields`: the young disc and the sub-discs.
    #[must_use]
    pub fn new(fields: &Fields) -> Self {
        let components = fields
            .component_ids()
            .filter(|&id| {
                matches!(
                    fields.component(id).population(),
                    Population::YoungThinDisc | Population::OldThinDisc
                )
            })
            .map(|id| (fields.component(id).count(), id))
            .collect();
        Self { components }
    }

    /// The oldest thin star, years.
    #[must_use]
    pub fn oldest(&self, fields: &Fields) -> f64 {
        self.components
            .iter()
            .map(|&(_, id)| fields.component(id).ages().max().value())
            .fold(0.0, f64::max)
    }

    /// The thin stars born between `lo_years` and `hi_years` ago, counted over the components: the
    /// weight of that span of the history, up to one constant.
    #[must_use]
    pub fn weight(&self, fields: &Fields, lo_years: f64, hi_years: f64) -> f64 {
        self.components
            .iter()
            .map(|&(count, id)| {
                let ages = fields.component(id).ages();
                count * (ages.cdf(Years::new(hi_years)) - ages.cdf(Years::new(lo_years))).max(0.0)
            })
            .sum()
    }

    /// An age between `lo` and `hi` years (both at least 0), drawn from the history restricted to
    /// it.
    ///
    /// # Panics
    ///
    /// If no thin star is born between `lo` and `hi`.
    pub fn sample(&self, fields: &Fields, lo: f64, hi: f64, draws: &mut Draws) -> f64 {
        let masses: Vec<f64> = self
            .components
            .iter()
            .map(|&(count, id)| {
                let ages = fields.component(id).ages();
                count * (ages.cdf(Years::new(hi)) - ages.cdf(Years::new(lo))).max(0.0)
            })
            .collect();
        let total: f64 = masses.iter().sum();
        assert!(total > 0.0, "no thin star is born between {lo} and {hi} yr");
        let pick = draws.uniform() * total;
        let mut running = 0.0;
        let mut chosen = self.components.len() - 1;
        for (i, m) in masses.iter().enumerate() {
            running += m;
            if pick < running {
                chosen = i;
                break;
            }
        }
        let ages = fields.component(self.components[chosen].1).ages();
        let (a, b) = (ages.cdf(Years::new(lo)), ages.cdf(Years::new(hi)));
        ages.quantile(a + draws.uniform() * (b - a))
            .value()
            .clamp(lo, hi)
    }
}

/// A velocity drawn from `component`'s ellipsoid at `p` in `kinematics`, km/s along the galactic
/// axes.
pub fn ellipsoid_velocity(
    kinematics: &KinematicTables,
    component: ComponentId,
    p: &PointLy,
    draws: &mut Draws,
) -> [f64; 3] {
    let e = kinematics.ellipsoid(component, p);
    let mean = e.mean().map(KilometresPerSecond::value);
    let sigma = e.sigma().map(KilometresPerSecond::value);
    let local: [f64; 3] = core::array::from_fn(|i| mean[i] + sigma[i] * draws.normal());
    let r_cyl = math::hypot(p.x, p.y);
    let (cos, sin) = if r_cyl > 0.0 {
        (p.x / r_cyl, p.y / r_cyl)
    } else {
        (1.0, 0.0)
    };
    match e.axes() {
        EllipsoidAxes::Cylindrical => {
            let [vr, vphi, vz] = local;
            [vr * cos - vphi * sin, vr * sin + vphi * cos, vz]
        }
        EllipsoidAxes::Spherical => {
            // r outward, θ from +z, φ spinward.
            let [vr, vtheta, vphi] = local;
            let r = math::hypot(r_cyl, p.z);
            let (st, ct) = if r > 0.0 {
                (r_cyl / r, p.z / r)
            } else {
                (1.0, 0.0)
            };
            let v_cyl = vr * st + vtheta * ct;
            let vz = vr * ct - vtheta * st;
            [v_cyl * cos - vphi * sin, v_cyl * sin + vphi * cos, vz]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_historys_weights_add_over_spans() {
        use hyperion_sim::galaxy::params::GalaxyParams;
        use hyperion_sim::galaxy::potential::MassModel;
        let params = GalaxyParams::milky_way_like();
        let fields = Fields::new(&params, &MassModel::new(&params));
        let history = ThinHistory::new(&fields);
        let oldest = history.oldest(&fields);
        let whole = history.weight(&fields, 0.0, oldest);
        let parts = history.weight(&fields, 0.0, 1e8) + history.weight(&fields, 1e8, oldest);
        assert!(
            whole > 0.0 && (parts / whole - 1.0).abs() < 1e-12,
            "{parts} {whole}"
        );
    }

    #[test]
    fn draws_are_uniform_and_keyed() {
        let mut first = Draws::new(7, 1, 2);
        let mut same = Draws::new(7, 1, 2);
        let mut other = Draws::new(7, 1, 3);
        assert_eq!(first.next_u64(), same.next_u64());
        assert_ne!(first.next_u64(), other.next_u64());
        let mut draws = Draws::new(1, 0, 0);
        let mean = (0..100_000).map(|_| draws.uniform()).sum::<f64>() / 100_000.0;
        assert!((mean - 0.5).abs() < 0.005, "{mean}");
        let direction = draws.direction();
        assert!((direction.iter().map(|x| x * x).sum::<f64>() - 1.0).abs() < 1e-12);
    }
}
