//! The luminous blue variables' sampler (plan 15, P15.T10.a), which plan 09's P09.T43 class,
//! `galaxy::catalogue_classes::lbv::LbvProcess`, reads as `tables::lbv::LBV_SAMPLER`.
//!
//! Per mass node of the manifest's five, the earliest start and the latest end of plan 06's
//! luminous-blue-variable windows over the node's masses and every \[Fe/H\] of the manifest's grid,
//! widened by its margin and rounded outward to its whole years: the scan that built P09.T43's
//! scratch table. Neither the scan nor the widening is copied here: each mass's span of windows is
//! the sim's own `sampler::window_span` over `sampler::grid`, under the generator version's stellar
//! rules, and each node is `LbvNode::widened`'s, so a later version's tracks (26's, whose winds
//! enforce the Humphreys–Davidson limit, P06.T42.b) refit the table with one rerun. The task
//! spreads the scan's masses over its threads and folds each node's spans in mass order; an
//! earliest start and a latest end are exact in any order, so the thread count moves no bit.
//!
//! The run's own check is the floor: the scan from the manifest's `floor` up to the lowest node
//! finds no window, so the nodes hold every star that has one, and every node holds one. A run
//! whose floor scan finds a window writes the table as provisional. The plan's acceptance, the
//! million-star sample and the Kolmogorov–Smirnov test of its hosts' masses, is the sim's slow
//! `the_class_test_holds_every_host_of_a_million_layer_e_stars`; the sim's slow
//! `the_table_is_the_scan_of_plan_06_s_tracks` rebuilds the nodes from the table's own scan
//! constants.

use std::fmt::Write as _;
use std::num::NonZeroUsize;

use hyperion_sim::galaxy::catalogue_classes::lbv::sampler::{LbvNode, grid, window_span};
use hyperion_sim::galaxy::imf::MassBand;
use hyperion_sim::stellar::sse::MAX_INITIAL_MASS;
use hyperion_sim::units::{Dex, SolarMasses, Years};

use crate::emit::{RustTable, TableItem, literal};
use crate::manifest::{Manifest, ManifestParamError, SimFingerprint};
use crate::parallel::{BuildThreadPoolError, map_reduce_chunks};
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput, check_manifest};

/// The task's revision.
pub const VERSION: u32 = 0;

/// The table's items: the scan's constants, then the sampler.
pub const ITEMS: [&str; 5] = [
    "MARGIN",
    "ROUNDING_YEARS",
    "SCAN_MASS_STEP",
    "SCAN_FE_H",
    "LBV_SAMPLER",
];

/// The sampler's mass nodes: `LbvSampler`'s five.
pub const NODES: usize = 5;

/// The points whose windows the fingerprint probes, (initial mass M☉, \[Fe/H\]): a mass in each
/// node, from the metal-poorest to the metal-richest of the scan.
const PROBES: [(f64, f64); 5] = [
    (34.5, -0.6),
    (40.0, -2.4),
    (50.0, -1.0),
    (75.0, 0.0),
    (150.0, 0.3),
];

/// The scan's parameters, from the manifest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LbvScanParams {
    /// The nodes' lightest initial masses, M☉, ascending.
    pub node_masses: [f64; NODES],
    /// The last node's heaviest initial mass, M☉.
    pub top: f64,
    /// The scan's step in initial mass, M☉.
    pub mass_step: f64,
    /// The scan's \[Fe/H\], dex: its first, its last and its step.
    pub fe_h: (f64, f64, f64),
    /// The relative margin each node's ages are widened by.
    pub margin: f64,
    /// The years each node's widened ages are rounded outward to.
    pub rounding_years: f64,
    /// The lightest initial mass, M☉, of the floor's scan below the lowest node.
    pub floor: f64,
}

impl LbvScanParams {
    /// The parameters of `manifest`.
    ///
    /// # Errors
    ///
    /// [`ManifestParamError`] if one is missing, is not a number, or is out of range: the nodes not
    /// five ascending masses above the floor and below the top, the top above the tracks'
    /// heaviest, a step not positive, the \[Fe/H\] grid not ascending, the margin outside \[0, 1)
    /// or the rounding not positive.
    pub fn of(manifest: &Manifest) -> Result<Self, ManifestParamError> {
        let node_masses: [f64; NODES] = numbers(manifest, "node_masses")?
            .try_into()
            .map_err(|_| ManifestParamError::new("node_masses", "five masses"))?;
        let fe_h: [f64; 3] = numbers(manifest, "fe_h")?
            .try_into()
            .map_err(|_| ManifestParamError::new("fe_h", "[first, last, step]"))?;
        let params = Self {
            node_masses,
            top: manifest.f64("top")?,
            mass_step: manifest.f64("mass_step")?,
            fe_h: (fe_h[0], fe_h[1], fe_h[2]),
            margin: manifest.f64("margin")?,
            rounding_years: manifest.f64("rounding_years")?,
            floor: manifest.f64("floor")?,
        };
        let positive = |x: f64| x.is_finite() && x > 0.0;
        if !positive(params.floor) {
            return Err(ManifestParamError::new("floor", "positive"));
        }
        let ascending = params.node_masses.windows(2).all(|w| w[0] < w[1])
            && params.floor < params.node_masses[0]
            && params.node_masses[NODES - 1] < params.top;
        if !ascending {
            return Err(ManifestParamError::new(
                "node_masses",
                "five ascending masses between `floor` and `top`",
            ));
        }
        if params.top > MAX_INITIAL_MASS.value() {
            return Err(ManifestParamError::new(
                "top",
                "at most the tracks' heaviest initial mass",
            ));
        }
        if !positive(params.mass_step) {
            return Err(ManifestParamError::new("mass_step", "positive"));
        }
        if !(params.fe_h.0.is_finite() && params.fe_h.0 <= params.fe_h.1 && positive(params.fe_h.2))
        {
            return Err(ManifestParamError::new(
                "fe_h",
                "[first, last, step], ascending with a positive step",
            ));
        }
        if !(0.0..1.0).contains(&params.margin) {
            return Err(ManifestParamError::new("margin", "in [0, 1)"));
        }
        if !positive(params.rounding_years) {
            return Err(ManifestParamError::new("rounding_years", "positive"));
        }
        Ok(params)
    }

    /// The scan's \[Fe/H\] grid as the sim's scan reads it.
    fn fe_h_grid(&self) -> (Dex, Dex, Dex) {
        (
            Dex::new(self.fe_h.0),
            Dex::new(self.fe_h.1),
            Dex::new(self.fe_h.2),
        )
    }

    /// The initial masses node `k` covers, M☉: from its own to the next node's, or to the top.
    fn node_range(&self, k: usize) -> (f64, f64) {
        (
            self.node_masses[k],
            self.node_masses.get(k + 1).copied().unwrap_or(self.top),
        )
    }

    /// The scan's masses, M☉, with the interval of each: the floor's, 0, from `floor` to below the
    /// lowest node, then node k's, k + 1, from its mass to the next node's, both ends included.
    fn masses(&self) -> Vec<(usize, f64)> {
        let lowest = self.node_masses[0];
        let floor = grid(self.floor, lowest, self.mass_step)
            .filter(|m| *m < lowest)
            .map(|m| (0, m));
        let nodes = (0..NODES).flat_map(|k| {
            let (lo, hi) = self.node_range(k);
            grid(lo, hi, self.mass_step).map(move |m| (k + 1, m))
        });
        floor.chain(nodes).collect()
    }
}

/// The array of numbers `key` of `manifest`'s parameters.
fn numbers(manifest: &Manifest, key: &str) -> Result<Vec<f64>, ManifestParamError> {
    manifest
        .params()
        .get(key)
        .and_then(toml::Value::as_array)
        .and_then(|a| a.iter().map(toml::Value::as_float).collect())
        .ok_or_else(|| ManifestParamError::new(key, "an array of floats"))
}

/// The scan's result.
#[derive(Debug, Clone, PartialEq)]
pub struct LbvFit {
    /// The parameters it was run with.
    pub params: LbvScanParams,
    /// Per interval, the floor's then each node's, the windows' earliest start and latest end,
    /// Julian years since the onset of collapse, or `None` if no track of it has one.
    pub envelopes: [Option<(f64, f64)>; NODES + 1],
    /// The tracks the scan built.
    pub tracks: u64,
}

impl LbvFit {
    /// The sampler's nodes, or `None` if a node holds no window.
    #[must_use]
    pub fn nodes(&self) -> Option<[LbvNode; NODES]> {
        let p = &self.params;
        let mut nodes = Vec::with_capacity(NODES);
        for k in 0..NODES {
            let (start, end) = self.envelopes[k + 1]?;
            nodes.push(LbvNode::widened(
                SolarMasses::new(p.node_masses[k]),
                (Years::new(start), Years::new(end)),
                p.margin,
                Years::new(p.rounding_years),
            ));
        }
        // One node was pushed per index, so the conversion to the array cannot fail.
        nodes.try_into().ok()
    }

    /// Whether the floor's scan found no window.
    #[must_use]
    pub const fn floor_holds(&self) -> bool {
        self.envelopes[0].is_none()
    }
}

/// Scans `params`' masses on `threads` threads, one mass's span of windows at a time.
///
/// # Errors
///
/// [`BuildThreadPoolError`] if the thread pool cannot be built.
///
/// # Panics
///
/// If the scan's masses or tracks outnumber a `u64`, which no manifest's grid reaches.
pub fn fit(params: &LbvScanParams, threads: NonZeroUsize) -> Result<LbvFit, BuildThreadPoolError> {
    let masses = params.masses();
    let fe_h = params.fe_h_grid();
    let mut envelopes = [None; NODES + 1];
    let n = u64::try_from(masses.len()).expect("the scan's masses fit in u64");
    map_reduce_chunks(
        n,
        1,
        threads,
        |range| {
            range
                .map(|i| {
                    let (interval, m) = masses[usize::try_from(i).expect("an index of the scan")];
                    let span = window_span(SolarMasses::new(m), fe_h)
                        .map(|(start, end)| (start.value(), end.value()));
                    (interval, span)
                })
                .collect::<Vec<_>>()
        },
        |spans| {
            for (interval, span) in spans {
                let slot: &mut Option<(f64, f64)> = &mut envelopes[interval];
                *slot = match (*slot, span) {
                    (Some((a, b)), Some((start, end))) => Some((a.min(start), b.max(end))),
                    (held, None) => held,
                    (None, found) => found,
                };
            }
        },
    )?;
    let per_mass = u64::try_from(grid(params.fe_h.0, params.fe_h.1, params.fe_h.2).count())
        .expect("the [Fe/H] grid's count fits in u64");
    Ok(LbvFit {
        params: *params,
        envelopes,
        tracks: n * per_mass,
    })
}

/// `LBV_SAMPLER`'s item: `nodes` and the top `top`, M☉, as struct literals of the sim's
/// `LbvSampler`, formatted as rustfmt formats them.
fn sampler_source(nodes: &[LbvNode; NODES], top: f64) -> String {
    let mut sampler = String::from(
        "/// The luminous blue variables' sampler (module documentation), fitted by \
         `hyperion-fit`'s task\n/// `lbv` (P15.T10.a).\npub const LBV_SAMPLER: LbvSampler = \
         LbvSampler {\n    nodes: [\n",
    );
    for node in nodes {
        // Writing to a String cannot fail.
        let _ = write!(
            sampler,
            "        LbvNode {{\n            mass: SolarMasses::new({}),\n            first: \
             Years::new({}),\n            last: Years::new({}),\n        }},\n",
            literal(node.mass().value()),
            literal(node.first().value()),
            literal(node.last().value()),
        );
    }
    // Writing to a String cannot fail.
    let _ = writeln!(
        sampler,
        "    ],\n    top: SolarMasses::new({}),\n}};",
        literal(top)
    );
    sampler
}

/// The table's contents for `fit`'s `nodes`.
#[must_use]
pub fn render(fit: &LbvFit, nodes: &[LbvNode; NODES]) -> RustTable {
    let p = &fit.params;
    let (fe_first, fe_last, fe_step) = p.fe_h;
    let items = vec![
        TableItem::Source(
            "use crate::galaxy::catalogue_classes::lbv::sampler::{LbvNode, LbvSampler};\n\
             use crate::units::{Dex, SolarMasses, Years};\n"
                .to_owned(),
        ),
        TableItem::Scalar {
            name: ITEMS[0].to_owned(),
            doc: vec![
                "The relative margin each node's ages are widened by, outward at both ends, for \
                 the masses"
                    .to_owned(),
                "and metallicities between the scan's points. A scan four times coarser in mass \
                 and in \\[Fe/H\\]"
                    .to_owned(),
                "missed the finer scan's latest end by up to 2.0% (the lowest node's, P09.T43's \
                 measurement)."
                    .to_owned(),
            ],
            value: p.margin,
        },
        TableItem::Scalar {
            name: ITEMS[1].to_owned(),
            doc: vec![
                "The years each node's widened ages are rounded outward to, so that the table's \
                 ages are whole"
                    .to_owned(),
                "multiples of it.".to_owned(),
            ],
            value: p.rounding_years,
        },
        TableItem::Source(format!(
            "/// The scan's step in initial mass.\npub const {}: SolarMasses = \
             SolarMasses::new({});\n",
            ITEMS[2],
            literal(p.mass_step),
        )),
        TableItem::Source(format!(
            "/// The scan's \\[Fe/H\\]: the first, the last and the step.\npub const {}: (Dex, \
             Dex, Dex) = (Dex::new({}), Dex::new({}), Dex::new({}));\n",
            ITEMS[3],
            literal(fe_first),
            literal(fe_last),
            literal(fe_step),
        )),
        TableItem::Source(sampler_source(nodes, p.top)),
    ];
    RustTable {
        summary: vec![
            "The luminous blue variables' sampler: the initial masses and the ages at which a \
             star can be a"
                .to_owned(),
            "luminous blue variable (plan 15, P15.T10.a; read by plan 09's P09.T43 class)."
                .to_owned(),
        ],
        notes: notes(fit),
        items,
    }
}

/// The table's notes after its header, one entry a line, for `fit`.
fn notes(fit: &LbvFit) -> Vec<String> {
    let p = &fit.params;
    let text = format!(
        "\
Five mass nodes, in the shape of `galaxy::catalogue_classes::lbv::sampler`'s `LbvSampler`,
which says what they hold. Node k covers the initial masses from its own to the next node's
({top} M☉ for the last), and holds the ages, since the onset of collapse, within which every star
of those masses is a luminous blue variable whenever it is one. The conditional density of
initial mass is the galaxy's mass function over each node's masses, weighted by the node's span
of ages, which plan 09's `LbvProcess` computes, since the mass function is the galaxy's.

Each node's ages are the earliest start and the latest end of plan 06's windows
(`Track::window_where` with `PhasePredicate::Lbv`, through `sampler::window_span`) of the
initial masses from the node's to the next node's in steps of `SCAN_MASS_STEP`, both ends
included, at every \\[Fe/H\\] of `SCAN_FE_H`, with no helium excess, at median draws, under the
generator version's stellar rules: {tracks} tracks, with the floor's. They are widened by
`MARGIN` and rounded outward to `ROUNDING_YEARS` (`LbvNode::widened`). The scan's \\[Fe/H\\]
reaches past both ends of the metal fractions the tracks' formulae see (`Composition::z_fit`,
held to 10⁻⁴–0.03, Hurley, Pols and Tout 2000's fitted range, about −2.3 to +0.2 at
Z☉ = 0.02), so every metallicity a young star draws is one the scan holds. No draw moves a
window (P09.T43's `no_draw_moves_a_window`).

Below the lowest node no star has a window: the scan from {floor} M☉ to it finds none. The
lightest windows are short ones, a few hundred years near \\[Fe/H\\] −0.6 from 34.1 M☉; from
about 36.3 M☉ the metal-poorest stars have windows of 10⁴–10⁵ years (P09.T43's measurements).
The floor is plan 06's: no star below about 34 M☉ reaches the limit's 6 × 10⁵ L☉ (Hurley, Pols
and Tout 2000, section 7.1), while observed luminous blue variables reach initial masses of
about 25 M☉ (Smith 2014, ARA&A 52, 487, section 4.1). Plan 06's tracks are Hurley, Pols and
Tout's formulae, fitted to models of 0.5–50 M☉ (their section 3), so the windows above 50 M☉
are extrapolated. The nodes, the manifest's, were placed to minimise the sampler's proposed
span of ages per layer-E system, about 0.23 Myr (P09.T43).",
        top = p.top,
        tracks = thousands(fit.tracks),
        floor = p.floor,
    );
    text.lines().map(str::to_owned).collect()
}

/// `n` with its thousands separated by commas: 55,481.
fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// The acceptance's figures for `fit`'s `nodes`: each node's windows and ages, and the floor.
fn acceptance(fit: &LbvFit, nodes: &[LbvNode; NODES]) -> String {
    let p = &fit.params;
    let mut out = String::new();
    for (k, node) in nodes.iter().enumerate() {
        let (start, end) = fit.envelopes[k + 1]
            .expect("`nodes` came from `LbvFit::nodes`, which holds a window for every node");
        let (lo, hi) = p.node_range(k);
        // Writing to a String cannot fail.
        let _ = write!(
            out,
            "{}{lo}–{hi} M☉ windows {start:.0}–{end:.0} yr, held as {:.0}–{:.0}",
            if k == 0 { "" } else { "; " },
            node.first().value(),
            node.last().value(),
        );
    }
    // Writing to a String cannot fail.
    let _ = write!(
        out,
        "; no track from {} M☉ to the lowest node has a window ({}); the class's million-star \
         sample and its masses' K–S test at 1% are the sim's slow \
         `the_class_test_holds_every_host_of_a_million_layer_e_stars`",
        p.floor,
        if fit.floor_holds() { "passes" } else { "FAILS" },
    );
    out
}

/// The task: P15.T10.a's sampler, fast.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct LbvTask;

impl FitTask for LbvTask {
    fn name(&self) -> &'static str {
        "lbv"
    }

    fn class(&self) -> TaskClass {
        TaskClass::Fast
    }

    fn table_path(&self) -> &'static str {
        "lbv.rs"
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &ITEMS
    }

    /// The windows' spans at `PROBES` (−1 for a probe with none), under the version's rules,
    /// so that a change to plan 06's tracks or to their rules is caught; and band E's top and
    /// the tracks' heaviest mass.
    fn fingerprint(&self) -> SimFingerprint {
        let mut probes = Vec::with_capacity(2 * PROBES.len() + 2);
        for (m, fe_h) in PROBES {
            let at = Dex::new(fe_h);
            let (start, end) = window_span(SolarMasses::new(m), (at, at, Dex::new(1.0)))
                .map_or((-1.0, -1.0), |(start, end)| (start.value(), end.value()));
            probes.push((format!("window start({m} M☉, [Fe/H] {fe_h})"), start));
            probes.push((format!("window end({m} M☉, [Fe/H] {fe_h})"), end));
        }
        probes.extend([
            ("MassBand::E.hi()".to_owned(), MassBand::E.hi()),
            ("MAX_INITIAL_MASS".to_owned(), MAX_INITIAL_MASS.value()),
        ]);
        SimFingerprint::new(probes)
    }

    fn run(&self, manifest: &Manifest, threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        check_manifest(self, manifest)?;
        let params = LbvScanParams::of(manifest)?;
        let fit = fit(&params, threads)?;
        let nodes = fit.nodes().ok_or_else(|| RunTaskError::Input {
            task: "lbv",
            source: "a node of the manifest holds no window".into(),
        })?;
        Ok(TaskOutput {
            table: render(&fit, &nodes),
            source: "plan 06's tracks (Hurley, Pols and Tout 2000, MNRAS 315, 543; the \
                     luminous-blue-variable criterion after their section 7.1 and Humphreys and \
                     Davidson 1994, PASP 106, 1025) at the median draws, under the generator \
                     version's stellar rules; the floor against Smith 2014, ARA&A 52, 487"
                .to_owned(),
            acceptance: acceptance(&fit, &nodes),
            provisional: (!fit.floor_holds()).then_some("P15.T10.a"),
        })
    }
}

#[cfg(test)]
mod tests {
    use hyperion_sim::galaxy::catalogue_classes::lbv::sampler::window_envelope;
    use hyperion_testkit::float::assert_same_bits;

    use super::*;

    /// A fit of `params` with every interval's envelope `envelope`, without the scan.
    fn toy(params: LbvScanParams, envelope: Option<(f64, f64)>) -> LbvFit {
        LbvFit {
            params,
            envelopes: [envelope; NODES + 1],
            tracks: 0,
        }
    }

    /// The committed manifest's parameters.
    fn committed() -> LbvScanParams {
        let manifest = Manifest::from_bytes(
            "manifests/lbv.toml".as_ref(),
            include_bytes!("../../manifests/lbv.toml").to_vec(),
        )
        .unwrap();
        LbvScanParams::of(&manifest).unwrap()
    }

    /// The scan's masses run from the floor to below the lowest node, then over each node's from
    /// its mass to the next's, both included, in steps of the manifest's.
    #[test]
    fn the_scan_holds_the_floor_and_each_node_s_masses() {
        let p = committed();
        let masses = p.masses();
        assert!(masses.windows(2).all(|w| w[0].0 <= w[1].0));
        let of = |interval: usize| -> Vec<f64> {
            masses
                .iter()
                .filter(|(i, _)| *i == interval)
                .map(|(_, m)| *m)
                .collect()
        };
        let floor = of(0);
        assert_eq!(floor.first().copied(), Some(p.floor));
        assert!(floor.iter().all(|m| *m < p.node_masses[0]));
        for k in 0..NODES {
            let node = of(k + 1);
            let (lo, hi) = p.node_range(k);
            assert_eq!(node.first().copied(), Some(lo));
            assert!((node.last().unwrap() - hi).abs() < 1e-9, "node {k}");
        }
    }

    /// Every envelope of `a` is `b`'s, bit for bit.
    fn assert_same_envelopes(a: &[Option<(f64, f64)>], b: &[Option<(f64, f64)>]) {
        assert_eq!(a.len(), b.len());
        for (k, (x, y)) in a.iter().zip(b).enumerate() {
            match (x, y) {
                (Some(x), Some(y)) => {
                    assert_same_bits(x.0, y.0);
                    assert_same_bits(x.1, y.1);
                }
                (None, None) => {}
                _ => panic!("interval {k}: {x:?} against {y:?}"),
            }
        }
    }

    /// On a coarse grid, 140 tracks, the fit is the same on one thread and on three, and each
    /// node's envelope is the sim's `window_envelope` of its masses.
    #[test]
    fn the_fit_is_the_sim_s_envelope_on_any_thread_count() {
        let coarse = LbvScanParams {
            mass_step: 4.0,
            fe_h: (-2.4, 0.3, 0.9),
            ..committed()
        };
        let one = fit(&coarse, NonZeroUsize::MIN).unwrap();
        let three = fit(&coarse, NonZeroUsize::new(3).unwrap()).unwrap();
        assert_eq!(one.tracks, 140);
        assert_eq!(three.tracks, one.tracks);
        assert_same_envelopes(&one.envelopes, &three.envelopes);
        let sim: Vec<Option<(f64, f64)>> = (0..NODES)
            .map(|k| {
                let (lo, hi) = coarse.node_range(k);
                window_envelope(
                    (SolarMasses::new(lo), SolarMasses::new(hi)),
                    SolarMasses::new(coarse.mass_step),
                    coarse.fe_h_grid(),
                )
                .map(|(start, end)| (start.value(), end.value()))
            })
            .collect();
        assert_same_envelopes(&one.envelopes[1..], &sim);
        assert!(one.floor_holds());
    }

    /// A manifest's nodes out of order, a top above the tracks', a margin of 1 or more, or a
    /// floor that is not positive is refused, naming its key.
    #[test]
    fn a_manifest_out_of_range_is_refused() {
        let text = String::from_utf8(include_bytes!("../../manifests/lbv.toml").to_vec()).unwrap();
        for (from, to, error) in [
            (
                "[32.0, 36.0,",
                "[36.0, 32.0,",
                ManifestParamError::new(
                    "node_masses",
                    "five ascending masses between `floor` and `top`",
                ),
            ),
            (
                "top = 150.0",
                "top = 151.0",
                ManifestParamError::new("top", "at most the tracks' heaviest initial mass"),
            ),
            (
                "margin = 0.02",
                "margin = 1.5",
                ManifestParamError::new("margin", "in [0, 1)"),
            ),
            (
                "floor = 24.0",
                "floor = 0.0",
                ManifestParamError::new("floor", "positive"),
            ),
        ] {
            assert!(text.contains(from), "{from}");
            let bytes = text.replace(from, to).into_bytes();
            let manifest = Manifest::from_bytes("manifests/lbv.toml".as_ref(), bytes).unwrap();
            assert_eq!(LbvScanParams::of(&manifest), Err(error));
        }
    }

    /// The rendered table declares [`ITEMS`] in order, its nodes' bits as `literal` writes them,
    /// and a floor that finds a window makes the run provisional.
    #[test]
    fn the_table_declares_its_items_and_the_floor_decides_provisional() {
        let fit = toy(committed(), Some((5.0e6, 6.0e6)));
        let nodes = fit.nodes().unwrap();
        let body = render(&fit, &nodes).body().unwrap();
        let declared: Vec<&str> = body
            .lines()
            .filter_map(|l| l.strip_prefix("pub const "))
            .filter_map(|l| l.split(':').next())
            .collect();
        assert_eq!(declared, ITEMS);
        assert!(body.contains("first: Years::new(4_900_000.0),"), "{body}");
        assert!(body.contains("last: Years::new(6_120_000.0),"), "{body}");
        assert!(!fit.floor_holds());
        assert!(acceptance(&fit, &nodes).contains("(FAILS)"));
        let mut held = fit;
        held.envelopes[0] = None;
        assert!(held.floor_holds());
        held.envelopes[3] = None;
        assert_eq!(held.nodes(), None);
    }
}
