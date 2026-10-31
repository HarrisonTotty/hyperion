//! The luminous blue variables' sampler's shape, and the scan of plan 06's tracks it is fitted
//! from (plan 09, P09.T43; plan 15's P15.T10.a fits [`tables::lbv`] with it).
//!
//! # What the sampler holds
//!
//! Five mass nodes. Node k covers the initial masses from its own to the next node's
//! ([`LbvSampler::top`] for the last), and holds the ages, since the onset of collapse, within
//! which every star of those masses is a luminous blue variable whenever it is one: the earliest
//! start and the latest end of the window of [`Track::window_where`] with [`PhasePredicate::Lbv`]
//! over a scan of the node's masses and of every metallicity the tracks distinguish
//! ([`window_envelope`]), widened and rounded outward ([`LbvNode::widened`]). Below the lowest node
//! no star has a window. The conditional density of initial mass is the galaxy's mass function over
//! each node's masses, weighted by the node's span of ages; [`LbvProcess`](super::LbvProcess)
//! computes it, since the mass function is the galaxy's.
//!
//! # The scan
//!
//! Plan 06's tracks at median draws ([`Track::full_with`]) with no helium excess, on grids of
//! initial mass and \[Fe/H\] ([`grid`]), under the generator version's stellar rules: a later
//! version's tracks (from 26, the Humphreys–Davidson limit the modern winds enforce, P06.T42.b)
//! move the windows, and `hyperion-fit`'s task `lbv` refits the table from them with one rerun.
//! No draw moves a window: Reimers η, the only draw the living track reads above the stripped
//! window, moves none of 30–150 M☉ at −1 to +0.5 over ±3σ (P09.T43's measurement), and
//! `no_draw_moves_a_window` holds that bit for bit at points from \[Fe/H\] −2.4 to +0.2.
//!
//! [`tables::lbv`]: crate::tables::lbv
//! [`Track::window_where`]: crate::stellar::sse::Track::window_where
//! [`Track::full_with`]: crate::stellar::sse::Track::full_with
//! [`PhasePredicate::Lbv`]: crate::stellar::sse::PhasePredicate::Lbv

use crate::GENERATOR_VERSION;
use crate::stellar::Composition;
use crate::stellar::draws::StarDraws;
use crate::stellar::rules::StellarRules;
use crate::stellar::sse::{PhasePredicate, Track, TrackOptions};
use crate::units::{Dex, HeliumExcess, SolarMasses, Years};

/// One mass node of the sampler.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LbvNode {
    pub(crate) mass: SolarMasses,
    pub(crate) first: Years,
    pub(crate) last: Years,
}

impl LbvNode {
    /// The node of lightest initial mass `mass` whose stars' windows run from `envelope.0` to
    /// `envelope.1`, Julian years since the onset of collapse: the start lowered by the relative
    /// `margin` and rounded down to a whole multiple of `rounding`, the end raised by it and
    /// rounded up.
    ///
    /// # Panics
    ///
    /// If `margin` is outside \[0, 1) or `rounding` is not finite and positive.
    ///
    /// # Examples
    ///
    /// Widened by 2% and rounded out to whole kiloyears:
    ///
    /// ```
    /// use hyperion_sim::galaxy::catalogue_classes::lbv::sampler::LbvNode;
    /// use hyperion_sim::units::{SolarMasses, Years};
    ///
    /// let envelope = (Years::new(5.0e6), Years::new(6.0e6));
    /// let node = LbvNode::widened(SolarMasses::new(40.0), envelope, 0.02, Years::new(1_000.0));
    /// assert_eq!(node.first(), Years::new(4_900_000.0));
    /// assert_eq!(node.last(), Years::new(6_120_000.0));
    /// ```
    #[must_use]
    pub fn widened(
        mass: SolarMasses,
        envelope: (Years, Years),
        margin: f64,
        rounding: Years,
    ) -> Self {
        let (start, end) = (envelope.0.value(), envelope.1.value());
        let step = rounding.value();
        assert!(
            (0.0..1.0).contains(&margin) && step.is_finite() && step > 0.0,
            "a node is widened by a margin in [0, 1) and rounded to a positive span: {margin}, \
             {step} yr"
        );
        Self {
            mass,
            first: Years::new((start * (1.0 - margin) / step).floor() * step),
            last: Years::new((end * (1.0 + margin) / step).ceil() * step),
        }
    }

    /// The lightest initial mass the node covers. It covers up to the next node's, or to
    /// [`LbvSampler::top`] for the last node.
    #[must_use]
    pub const fn mass(&self) -> SolarMasses {
        self.mass
    }

    /// The earliest age at which a star of the node is a luminous blue variable, Julian years
    /// since its onset of collapse, widened by the table's
    /// [`MARGIN`](crate::tables::lbv::MARGIN) and rounded down.
    #[must_use]
    pub const fn first(&self) -> Years {
        self.first
    }

    /// The latest age at which a star of the node is still one, Julian years since its onset of
    /// collapse, widened by the table's [`MARGIN`](crate::tables::lbv::MARGIN) and rounded up.
    #[must_use]
    pub const fn last(&self) -> Years {
        self.last
    }
}

/// The sampler: its mass nodes, in ascending mass, and the top of the last one's masses.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LbvSampler {
    pub(crate) nodes: [LbvNode; 5],
    pub(crate) top: SolarMasses,
}

impl LbvSampler {
    /// The nodes, in ascending mass.
    #[must_use]
    pub const fn nodes(&self) -> &[LbvNode; 5] {
        &self.nodes
    }

    /// The heaviest initial mass the last node covers: 150 M☉, the tracks' top and band E's.
    #[must_use]
    pub const fn top(&self) -> SolarMasses {
        self.top
    }

    /// The index of the node that covers initial mass `mass`, or `None` below the lowest node,
    /// above [`top`](Self::top) or for a mass that is not a number. A node covers its own mass
    /// and not the next node's; the last covers the top.
    #[must_use]
    pub fn node_of(&self, mass: SolarMasses) -> Option<usize> {
        let m = mass.value();
        if !(self.nodes[0].mass.value() <= m && m <= self.top.value()) {
            return None;
        }
        self.nodes.iter().rposition(|node| node.mass.value() <= m)
    }

    /// The initial masses node `k` covers, M☉: from its own to the next node's, or to the top.
    ///
    /// # Panics
    ///
    /// If `k` is not a node's index.
    #[must_use]
    pub fn masses(&self, k: usize) -> (SolarMasses, SolarMasses) {
        let lower = self.nodes[k].mass;
        let upper = self.nodes.get(k + 1).map_or(self.top, |next| next.mass);
        (lower, upper)
    }
}

/// The points of one of the scan's grids: `from + step × i` for i = 0, 1, … while within 10⁻⁹ of
/// `to`, so both ends are included where `to − from` is a whole number of steps.
///
/// # Panics
///
/// If `step` is not finite and positive.
///
/// # Examples
///
/// Five points from −2.4 to −2.3, the last within a hair of rounding:
///
/// ```
/// use hyperion_sim::galaxy::catalogue_classes::lbv::sampler::grid;
///
/// let points: Vec<f64> = grid(-2.4, -2.3, 0.025).collect();
/// assert_eq!(points.len(), 5);
/// assert!((points[4] + 2.3).abs() < 1e-9);
/// ```
pub fn grid(from: f64, to: f64, step: f64) -> impl Iterator<Item = f64> {
    assert!(
        step.is_finite() && step > 0.0,
        "a grid's step is finite and positive: {step}"
    );
    (0_u32..)
        .map(move |i| from + step * f64::from(i))
        .take_while(move |x| *x <= to + 1e-9)
}

/// The earliest start and the latest end, Julian years since the onset of collapse, of the
/// luminous-blue-variable windows ([`Track::window_where`] with [`PhasePredicate::Lbv`]) of plan
/// 06's tracks of initial mass `mass` at every \[Fe/H\] of the [`grid`] `fe_h` (its first, last and
/// step), with no helium excess and median draws, under the generator version's stellar rules; or
/// `None` if none of them has a window.
///
/// # Panics
///
/// As [`grid`], for a step of `fe_h` that is not finite and positive.
#[must_use]
pub fn window_span(mass: SolarMasses, fe_h: (Dex, Dex, Dex)) -> Option<(Years, Years)> {
    window_span_under(mass, fe_h, StellarRules::of_version(GENERATOR_VERSION))
}

/// [`window_span`] under the stellar stage's `rules` in place of the version's.
#[must_use]
pub(crate) fn window_span_under(
    mass: SolarMasses,
    fe_h: (Dex, Dex, Dex),
    rules: StellarRules,
) -> Option<(Years, Years)> {
    let options = TrackOptions::default().with_rules(rules);
    grid(fe_h.0.value(), fe_h.1.value(), fe_h.2.value())
        .filter_map(|fe_h| {
            let composition = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
            Track::full_with(mass, &composition, &StarDraws::median(), options)
                .window_where(PhasePredicate::Lbv)
                .map(|window| (window.start(), window.end()))
        })
        .reduce(union)
}

/// The earliest start and the latest end of the [`window_span`]s of the initial masses of the
/// [`grid`] from `masses.0` to `masses.1` in steps of `mass_step`, or `None` if none of their
/// tracks has a window: a node's ages before [`LbvNode::widened`] widens them.
///
/// # Panics
///
/// As [`grid`], for a step that is not finite and positive.
#[must_use]
pub fn window_envelope(
    masses: (SolarMasses, SolarMasses),
    mass_step: SolarMasses,
    fe_h: (Dex, Dex, Dex),
) -> Option<(Years, Years)> {
    grid(masses.0.value(), masses.1.value(), mass_step.value())
        .filter_map(|m| window_span(SolarMasses::new(m), fe_h))
        .reduce(union)
}

/// The earliest start and the latest end of two spans of ages.
#[must_use]
fn union(a: (Years, Years), b: (Years, Years)) -> (Years, Years) {
    (
        Years::new(a.0.value().min(b.0.value())),
        Years::new(a.1.value().max(b.1.value())),
    )
}

#[cfg(test)]
mod tests {
    #[cfg(not(target_family = "wasm"))]
    use std::thread;

    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::tables::lbv::{LBV_SAMPLER, MARGIN, ROUNDING_YEARS, SCAN_FE_H, SCAN_MASS_STEP};

    /// The nodes cover band E's top in ascending order, each node covering its own mass and its
    /// masses' midpoint, and each node's ages are whole kiloyears and ordered.
    #[test]
    fn the_nodes_are_ordered_and_whole() {
        let sampler = LBV_SAMPLER;
        assert_same_bits(sampler.top.value(), crate::galaxy::imf::MassBand::E.hi());
        for (k, node) in sampler.nodes.iter().enumerate() {
            let (lo, hi) = sampler.masses(k);
            assert!(lo < hi, "node {k}");
            assert!(node.first < node.last, "node {k}");
            for age in [node.first, node.last] {
                assert_same_bits(age.value() % ROUNDING_YEARS, 0.0);
            }
            assert_eq!(sampler.node_of(lo), Some(k));
            assert_eq!(
                sampler.node_of(SolarMasses::new(f64::midpoint(lo.value(), hi.value()))),
                Some(k)
            );
        }
        assert_eq!(sampler.node_of(sampler.top), Some(4));
        assert_eq!(sampler.node_of(SolarMasses::new(31.99)), None);
        assert_eq!(sampler.node_of(SolarMasses::new(150.01)), None);
        assert_eq!(sampler.node_of(SolarMasses::new(f64::NAN)), None);
    }

    /// No star below the lowest node has a window: from 24 to 32 M☉ no track has one at any
    /// metallicity the scan holds, which a quarter of them, every 0.1 dex, checks.
    #[test]
    fn no_star_below_the_lowest_node_is_a_luminous_blue_variable() {
        let (first, last) = (SCAN_FE_H.0.value(), SCAN_FE_H.1.value());
        for m in [24.0, 28.0, 31.0, 32.0] {
            for k in 0..=27 {
                let fe_h = Dex::new(first + 0.1 * f64::from(k));
                assert!(fe_h.value() <= last + 1e-9);
                let composition = Composition::from_fe_h(fe_h, HeliumExcess::ZERO);
                let track = Track::full(SolarMasses::new(m), &composition, &StarDraws::median());
                assert_eq!(
                    track.window_where(PhasePredicate::Lbv),
                    None,
                    "{m} M☉ at {fe_h:?}"
                );
            }
        }
    }

    /// A span is the union of each metallicity's window, which a single-point grid reads alone;
    /// and the scan follows its rules: under 26's, whose winds enforce the Humphreys–Davidson
    /// limit, a heavy star's span moves, so that 26's refit is the task's rerun.
    #[test]
    fn a_span_is_its_tracks_windows_under_its_rules() {
        let m = SolarMasses::new(60.0);
        let fe_h = Dex::new(0.0);
        let window = Track::full(
            m,
            &Composition::from_fe_h(fe_h, HeliumExcess::ZERO),
            &StarDraws::median(),
        )
        .window_where(PhasePredicate::Lbv)
        .expect("a 60 M☉ star of solar metallicity is a luminous blue variable for a while");
        let one = (fe_h, fe_h, Dex::new(0.1));
        let span = window_span(m, one).unwrap();
        assert_same_bits(span.0.value(), window.start().value());
        assert_same_bits(span.1.value(), window.end().value());
        let two = (fe_h, Dex::new(0.1), Dex::new(0.1));
        let both = window_span(m, two).unwrap();
        assert!(both.0 <= window.start() && both.1 >= window.end());
        let held = window_span_under(m, one, StellarRules::of_version(GENERATOR_VERSION)).unwrap();
        assert_same_bits(held.0.value(), span.0.value());
        assert_same_bits(held.1.value(), span.1.value());
        assert_ne!(
            window_span_under(m, one, StellarRules::every_26_hold()),
            window_span(m, one)
        );
    }

    /// The table is the scan's (module documentation): each node's earliest start and latest end
    /// over its masses and every metallicity, widened by the table's margin and rounded outward to
    /// its whole kiloyears, as `hyperion-fit`'s task `lbv` writes it. Some 51,000 tracks, on as
    /// many threads as nodes natively.
    #[test]
    #[ignore = "slow: the scan of some 51,000 tracks that builds the table"]
    fn the_table_is_the_scan_of_plan_06_s_tracks() {
        let sampler = LBV_SAMPLER;
        let node = |k: usize| {
            let masses = sampler.masses(k);
            let envelope = window_envelope(masses, SCAN_MASS_STEP, SCAN_FE_H)
                .expect("every node holds a window");
            let node = LbvNode::widened(masses.0, envelope, MARGIN, Years::new(ROUNDING_YEARS));
            eprintln!(
                "node {k}: windows {}–{} yr, held as {}–{}",
                envelope.0.value(),
                envelope.1.value(),
                node.first.value(),
                node.last.value()
            );
            node
        };
        #[cfg(not(target_family = "wasm"))]
        let nodes: Vec<LbvNode> = thread::scope(|scope| {
            let workers: Vec<_> = (0..sampler.nodes.len())
                .map(|k| scope.spawn(move || node(k)))
                .collect();
            workers
                .into_iter()
                .map(|worker| worker.join().expect("a scan worker"))
                .collect()
        });
        #[cfg(target_family = "wasm")]
        let nodes: Vec<LbvNode> = (0..sampler.nodes.len()).map(node).collect();
        for (k, node) in nodes.into_iter().enumerate() {
            assert_same_bits(sampler.nodes[k].mass.value(), node.mass.value());
            assert_same_bits(sampler.nodes[k].first.value(), node.first.value());
            assert_same_bits(sampler.nodes[k].last.value(), node.last.value());
        }
    }
}
