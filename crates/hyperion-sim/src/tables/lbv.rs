//! The luminous blue variables' sampler: the initial masses and the ages at which a star can be a
//! luminous blue variable (plan 15, P15.T10.a; read by plan 09's P09.T43 class).
//!
//! @provisional: hand-made scratch values (plan 09, P09.T43), not written by `hyperion-fit`, so not
//! in [`MANIFEST`](super::MANIFEST) until plan 15's P15.T10.a replaces them.
//! since-generator-version: 24
//! source: plan 06's tracks at median draws, scanned as "How it was built" says
//!
//! **Scratch.** Plan 09's P09.T43 entered this table by hand from plan 06's tracks, as plan 09's
//! Consumes allows before plan 15's fit. It belongs to the generator version all the same: plan
//! 09's class draws its candidates' masses and ages from it, and its record-only prefilter reads
//! it on both sides of the class test, so a change to it moves the class's candidates.
//!
//! # What it holds
//!
//! Five mass nodes. Node k covers the initial masses from its own to the next node's (150 M☉, the
//! tracks' top, for the last), and holds the ages, since the onset of collapse, within which every
//! star of those masses is a luminous blue variable whenever it is one: the earliest start and the
//! latest end of the window of [`Track::window_where`] with [`PhasePredicate::Lbv`], over a scan of
//! the node's masses and of every metallicity the tracks distinguish, widened by
//! [`MARGIN`] and rounded outward to [`ROUNDING_YEARS`]. Below the lowest node no star has a
//! window. The conditional density of initial mass is the galaxy's mass function over each node's
//! masses, weighted by the node's span of ages; plan 09's process computes it, since the mass
//! function is the galaxy's.
//!
//! # How it was built
//!
//! Plan 06's tracks at median draws, [`Track::full`]: initial masses from each node's to the next
//! node's in steps of [`SCAN_MASS_STEP`], both ends included, at \[Fe/H\] from [`SCAN_FE_H`]'s
//! first to its last in steps of its third, with no helium excess. The scan's \[Fe/H\] reaches
//! past both ends of the metal fractions the tracks' formulae see (`Composition::z_fit`, held to
//! 10⁻⁴–0.03, about −2.3 to +0.2), so every metallicity a young star draws is one the scan holds.
//! No draw moves a window: Reimers η, the only draw the living track reads above the stripped
//! window, moves none of 30–150 M☉ at −1 to +0.5 over ±3σ (P09.T43's measurement), and a test of
//! plan 09's class holds that bit for bit at points from \[Fe/H\] −2.4 to +0.2. A slow test of this
//! module recomputes the table from the scan.
//!
//! The lightest windows are short ones, a few hundred years near \[Fe/H\] −0.6 from 34.1 M☉ (none
//! from 32 to 34 M☉ at 0.1 M☉ and 0.025 dex); from about 36.3 M☉ the metal-poorest stars have
//! windows of 10⁴–10⁵ years. Those edges are where `Track::window_where`'s 257 samples a segment
//! find windows, which a window of a few hundred years in a long segment can fall between. The lowest node sits at 32 M☉, below both. The nodes were placed to
//! minimise the sampler's proposed span of ages per layer-E system, about 0.23 Myr.
//!
//! [`Track::window_where`]: crate::stellar::sse::Track::window_where
//! [`Track::full`]: crate::stellar::sse::Track::full
//! [`PhasePredicate::Lbv`]: crate::stellar::sse::PhasePredicate::Lbv

use crate::units::{Dex, SolarMasses, Years};

/// The relative margin each node's ages are widened by: 2%, outward at both ends, for the masses
/// and metallicities between the scan's points. A scan four times coarser in mass and in \[Fe/H\]
/// missed the finer scan's latest end by up to 2.0% (the lowest node's, P09.T43's measurement).
pub const MARGIN: f64 = 0.02;

/// The years each node's widened ages are rounded outward to: 1,000, so that the table's values are
/// whole kiloyears.
pub const ROUNDING_YEARS: f64 = 1_000.0;

/// The scan's step in initial mass.
pub const SCAN_MASS_STEP: SolarMasses = SolarMasses::new(0.25);

/// The scan's \[Fe/H\]: the first, the last and the step.
pub const SCAN_FE_H: (Dex, Dex, Dex) = (Dex::new(-2.4), Dex::new(0.3), Dex::new(0.025));

/// One mass node of the sampler.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LbvNode {
    mass: SolarMasses,
    first: Years,
    last: Years,
}

impl LbvNode {
    /// The lightest initial mass the node covers. It covers up to the next node's, or to
    /// [`LbvSampler::top`] for the last node.
    #[must_use]
    pub const fn mass(&self) -> SolarMasses {
        self.mass
    }

    /// The earliest age at which a star of the node is a luminous blue variable, Julian years
    /// since its onset of collapse, widened by [`MARGIN`] and rounded down.
    #[must_use]
    pub const fn first(&self) -> Years {
        self.first
    }

    /// The latest age at which a star of the node is still one, Julian years since its onset of
    /// collapse, widened by [`MARGIN`] and rounded up.
    #[must_use]
    pub const fn last(&self) -> Years {
        self.last
    }
}

/// The sampler: its mass nodes, in ascending mass, and the top of the last one's masses.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LbvSampler {
    nodes: [LbvNode; 5],
    top: SolarMasses,
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
        let upper = self.nodes.get(k + 1).map_or(self.top, |next| next.mass);
        (self.nodes[k].mass, upper)
    }
}

/// The luminous blue variables' sampler (module documentation): scratch, from plan 06's tracks,
/// until P15.T10.a.
pub const LBV_SAMPLER: LbvSampler = LbvSampler {
    nodes: [
        LbvNode {
            mass: SolarMasses::new(32.0),
            first: Years::new(6_171_000.0),
            last: Years::new(6_644_000.0),
        },
        LbvNode {
            mass: SolarMasses::new(36.0),
            first: Years::new(4_951_000.0),
            last: Years::new(6_424_000.0),
        },
        LbvNode {
            mass: SolarMasses::new(43.0),
            first: Years::new(4_074_000.0),
            last: Years::new(5_573_000.0),
        },
        LbvNode {
            mass: SolarMasses::new(55.0),
            first: Years::new(3_422_000.0),
            last: Years::new(4_724_000.0),
        },
        LbvNode {
            mass: SolarMasses::new(97.0),
            first: Years::new(2_101_000.0),
            last: Years::new(3_898_000.0),
        },
    ],
    top: SolarMasses::new(150.0),
};

#[cfg(test)]
mod tests {
    #[cfg(not(target_family = "wasm"))]
    use std::thread;

    use hyperion_testkit::float::assert_same_bits;

    use super::*;
    use crate::stellar::Composition;
    use crate::stellar::draws::StarDraws;
    use crate::stellar::sse::{PhasePredicate, Track};
    use crate::units::{Dex, HeliumExcess};

    /// The earliest start and the latest end, Julian years, of the LBV windows of the scan's
    /// masses from `lo` to `hi` (both included) at every one of its \[Fe/H\], or `None` if no star
    /// of them has one.
    fn envelope(lo: f64, hi: f64) -> Option<(f64, f64)> {
        let (first, last, step) = (
            SCAN_FE_H.0.value(),
            SCAN_FE_H.1.value(),
            SCAN_FE_H.2.value(),
        );
        // The grids' points, up to their last within a hair of rounding.
        let steps = |from: f64, to: f64, by: f64| {
            (0_u32..)
                .map(move |i| from + by * f64::from(i))
                .take_while(move |x| *x <= to + 1e-9)
        };
        let mut found: Option<(f64, f64)> = None;
        for m in steps(lo, hi, SCAN_MASS_STEP.value()) {
            let m = SolarMasses::new(m);
            for fe_h in steps(first, last, step) {
                let fe_h = Dex::new(fe_h);
                let composition = Composition::from_fe_h(fe_h, HeliumExcess::ZERO);
                let track = Track::full(m, &composition, &StarDraws::median());
                if let Some(window) = track.window_where(PhasePredicate::Lbv) {
                    let (start, end) = (window.start().value(), window.end().value());
                    found = Some(found.map_or((start, end), |(a, b)| (a.min(start), b.max(end))));
                }
            }
        }
        found
    }

    /// The nodes cover band E's top in ascending order, and each node's ages are whole kiloyears,
    /// ordered, and before the lifetime of a star of the node's mass at solar metallicity ends
    /// by much.
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

    /// The table is the scan's (module documentation): each node's earliest start and latest end
    /// over its masses and every metallicity, widened by 2% and rounded outward to whole
    /// kiloyears. Some 51,000 tracks, on as many threads as nodes natively.
    #[test]
    #[ignore = "slow: the scan of some 51,000 tracks that builds the table"]
    fn the_table_is_the_scan_of_plan_06_s_tracks() {
        let sampler = LBV_SAMPLER;
        let node = |k: usize| {
            let (lo, hi) = sampler.masses(k);
            let (start, end) = envelope(lo.value(), hi.value()).expect("every node holds a window");
            let first = (start * (1.0 - MARGIN) / ROUNDING_YEARS).floor() * ROUNDING_YEARS;
            let last = (end * (1.0 + MARGIN) / ROUNDING_YEARS).ceil() * ROUNDING_YEARS;
            eprintln!("node {k}: windows {start}–{end} yr, held as {first}–{last}");
            (first, last)
        };
        #[cfg(not(target_family = "wasm"))]
        let nodes: Vec<(f64, f64)> = thread::scope(|scope| {
            let workers: Vec<_> = (0..sampler.nodes.len())
                .map(|k| scope.spawn(move || node(k)))
                .collect();
            workers
                .into_iter()
                .map(|worker| worker.join().expect("a scan worker"))
                .collect()
        });
        #[cfg(target_family = "wasm")]
        let nodes: Vec<(f64, f64)> = (0..sampler.nodes.len()).map(node).collect();
        for (k, (first, last)) in nodes.into_iter().enumerate() {
            assert_same_bits(sampler.nodes[k].first.value(), first);
            assert_same_bits(sampler.nodes[k].last.value(), last);
        }
    }
}
