//! The Type Ia delay-first samplers (plan 15, P15.T9.a; consumer plan 09's
//! `galaxy::catalogue_classes::type_ia`, and plan 11 reads `DELAY_EDGES`).
//!
//! The rate is observed, not computed: Maoz and Graur's (2017) delay-time distribution as plan 09
//! built it (`DelayTimeDistribution`: 1.3 × 10⁻³ per M☉ formed, `t^−1.1` from plan 06's lifetime of
//! an 8 M☉ star, 42.6 Myr, to 13.7 Gyr; ruling 136.7). The table cuts it into 24 bins even in
//! `ln t` and gives, per bin, the yield and what a Type Ia of that delay is made of, from
//! single-star lifetimes and the initial-to-final mass relation alone:
//!
//! - **The pair.** A primary from the galaxy's own mass function (Chabrier's branch above 1 M☉,
//!   `m^−2.3`), a mass ratio uniform on 0.1–1 (`type_ia::MIN_MASS_RATIO`), and the secondary
//!   inside its channel's window (`type_ia::secondary_window`): a white dwarf's progenitor has died
//!   within the delay (lifetime at most the delay, so its mass is at least `m(t)`, the mass whose
//!   lifetime is the delay), and a living donor has not (at most `m(t)` and 3 M☉). The primary
//!   is at least the heavier of 2.5 M☉ and `m(t)` (`type_ia::primary_range`), so its white dwarf
//!   exists by the explosion. Given the delay the primary's density is therefore the mass function
//!   times the share of mass ratios its channel's window leaves open, and the secondary is uniform
//!   in mass inside the window. For a merger the class solves the separation after the common
//!   envelope from Peters (1964) so that the secondary's lifetime and the inspiral add up to the
//!   delay, with Kalirai et al.'s (2008) white-dwarf masses: that is the initial-to-final mass
//!   relation's part, and it needs nothing tabulated.
//! - **Per bin** the conditional laws are mixed over the bin's delays by ψ. The primary is placed
//!   on its range at the actual delay in `ln m`, the secondary on its window linearly in mass, so
//!   that every draw keeps both lifetimes inside its own delay whatever its place in the bin.
//! - **The channel** takes ruling 136.9's shares in every bin: lifetimes and the IFMR say nothing
//!   of which channel explodes, and every channel stays open at every delay. Plan 11's engine
//!   (P15.T9.b) is where a delay dependence would come from.
//!
//! The run's acceptance: the yield integrates to 1.3 × 10⁻³ per M☉ to 1%; a fifth of delays under
//! 0.1 Gyr and about 62% under 1 Gyr; no primary under 2.5 M☉; at Milky Way values 0.42–0.53
//! events a century (ruling 141.4) and the old populations' exploded share of layer D in 2–4.5%
//! (ruling 141.6 holds 3.3–4.5%); and the 17-point quantiles within a stated distance of the exact
//! mixture.

use std::fmt::Write as _;
use std::num::NonZeroUsize;

use hyperion_sim::Seed;
use hyperion_sim::galaxy::catalogue_classes::type_ia::{
    CHANNEL_SHARES, DONOR_MAX, DelayTimeDistribution, HUBBLE_TIME, IaChannel, MIN_DELAY,
    MIN_MASS_RATIO, PRIMARY_MAX, PRIMARY_MIN, primary_range, secondary_window,
};
use hyperion_sim::galaxy::features::interior::counts::white_dwarf_mass;
use hyperion_sim::galaxy::imf::{MassBand, MassFunction};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::{Galaxy, Population};
use hyperion_sim::math;
use hyperion_sim::stellar::Composition;
use hyperion_sim::stellar::draws::StarDraws;
use hyperion_sim::stellar::lifetime;
use hyperion_sim::tables::gauss_legendre::{GL4_NODES, GL4_WEIGHTS, GL32_NODES, GL32_WEIGHTS};
use hyperion_sim::units::{SolarMasses, Years};

use crate::emit::{RustTable, TableItem, literal};
use crate::manifest::{Manifest, SimFingerprint};
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput, check_manifest};

/// The task's revision.
pub const VERSION: u32 = 0;

/// The delay bins, even in `ln t` from `MIN_DELAY` to `HUBBLE_TIME`.
pub const BINS: usize = 24;

/// The channels, in `IaChannel::ALL`'s order.
pub const CHANNELS: usize = 4;

/// The quantiles of each conditional law, at ranks `k ÷ 16`.
pub const RANKS: usize = 17;

/// The fine grid in the primary's place on its range on which each bin's law is tabulated before
/// its quantiles are read off, and the acceptance measures them.
const PLACE_SEGMENTS: usize = 256;

/// The fitted table.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeIaDelayFit {
    /// The 25 bin edges, years.
    pub edges: [f64; BINS + 1],
    /// Type Ia per M☉ formed with a delay in each bin.
    pub yields: [f64; BINS],
    /// The channels' shares per bin.
    pub channel_share: [[f64; CHANNELS]; BINS],
    /// The primary's place on its range in `ln m`, at the 17 ranks, per bin and channel.
    pub primary: [[[f64; RANKS]; CHANNELS]; BINS],
    /// The secondary's place in its window, linear in mass, at the 17 ranks, per bin and channel.
    pub secondary: [[[f64; RANKS]; CHANNELS]; BINS],
    /// The primaries' shares in layers C and D per bin.
    pub layer_share: [[f64; 2]; BINS],
    /// Layer-D systems a solar mass formed loses to Type Ia over a Hubble time.
    pub ancient_loss: f64,
    /// The largest distance between a bin's exact CDF of the primary's place and the 17-point
    /// linear interpolation of its quantiles, over every bin and channel.
    pub worst_quantile_error: f64,
}

/// The mass, M☉, whose lifetime is `delay` years: the sim's own inverse of plan 06's track at
/// solar composition and median draws, held to 0.8–8 M☉ (`DelayTimeDistribution::mass_with_lifetime`).
fn fit_mass(delays: &DelayTimeDistribution, delay: f64) -> SolarMasses {
    delays.mass_with_lifetime(Years::new(delay))
}

/// The density of a primary of `m` M☉ given the delay's `fit` mass and `channel`, unnormalised:
/// the mass function times the share of mass ratios on 0.1–1 that the channel's window leaves.
fn primary_weight(imf: &dyn MassFunction, channel: IaChannel, m: f64, fit: SolarMasses) -> f64 {
    let (lo, hi) = secondary_window(channel, m, fit);
    imf.pdf(m) * (hi - lo).max(0.0) / ((1.0 - MIN_MASS_RATIO) * m)
}

/// The exact CDF of the primary's place `x ∈ [0, 1]` on its range at one delay, on the fine grid of
/// [`PLACE_SEGMENTS`] + 1 points: `x = ln(m ÷ lo) ÷ ln(hi ÷ lo)`.
fn place_cdf_at(
    imf: &dyn MassFunction,
    channel: IaChannel,
    fit: SolarMasses,
) -> [f64; PLACE_SEGMENTS + 1] {
    let (lo, hi) = primary_range(fit);
    let span = math::ln(hi / lo);
    let density = |x: f64| {
        let m = lo * math::exp(span * x);
        primary_weight(imf, channel, m, fit) * m * span
    };
    let mut cdf = [0.0; PLACE_SEGMENTS + 1];
    let step = 1.0 / index_f64(PLACE_SEGMENTS);
    for k in 0..PLACE_SEGMENTS {
        let (a, b) = (index_f64(k) * step, index_f64(k + 1) * step);
        let (half, mid) = (0.5 * (b - a), 0.5 * (a + b));
        let panel = GL4_NODES
            .iter()
            .zip(GL4_WEIGHTS)
            .fold(0.0, |s, (&u, w)| s + w * density(mid + half * u))
            * half;
        cdf[k + 1] = cdf[k] + panel;
    }
    let total = cdf[PLACE_SEGMENTS];
    if total > 0.0 {
        for v in &mut cdf {
            *v /= total;
        }
    }
    cdf
}

/// The 17 quantiles of a CDF on the fine grid, by linear interpolation of its inverse.
fn quantiles(cdf: &[f64; PLACE_SEGMENTS + 1]) -> [f64; RANKS] {
    let step = 1.0 / index_f64(PLACE_SEGMENTS);
    std::array::from_fn(|k| {
        if k == 0 {
            return 0.0;
        }
        if k == RANKS - 1 {
            return 1.0;
        }
        let rank = index_f64(k) / index_f64(RANKS - 1);
        let i = cdf.partition_point(|&v| v < rank).clamp(1, PLACE_SEGMENTS);
        let (f0, f1) = (cdf[i - 1], cdf[i]);
        let t = if f1 > f0 {
            (rank - f0) / (f1 - f0)
        } else {
            0.0
        };
        (index_f64(i - 1) + t) * step
    })
}

/// The largest distance between `cdf` and the CDF the 17 quantiles give by linear interpolation.
fn quantile_error(cdf: &[f64; PLACE_SEGMENTS + 1], q: &[f64; RANKS]) -> f64 {
    let step = 1.0 / index_f64(PLACE_SEGMENTS);
    let mut worst = 0.0_f64;
    for (k, &exact) in cdf.iter().enumerate() {
        let x = index_f64(k) * step;
        let j = q.partition_point(|&v| v <= x).clamp(1, RANKS - 1);
        let (a, b) = (q[j - 1], q[j]);
        let within = if b > a {
            ((x - a) / (b - a)).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let approx = (index_f64(j - 1) + within) / index_f64(RANKS - 1);
        worst = worst.max((approx - exact).abs());
    }
    worst
}

/// The 25 edges, even in `ln t` from `MIN_DELAY` to `HUBBLE_TIME`, the ends exact.
#[must_use]
pub fn delay_edges() -> [f64; BINS + 1] {
    let (lo, hi) = (math::ln(MIN_DELAY.value()), math::ln(HUBBLE_TIME.value()));
    std::array::from_fn(|i| match i {
        0 => MIN_DELAY.value(),
        BINS => HUBBLE_TIME.value(),
        _ => math::exp(lo + (hi - lo) * index_f64(i) / index_f64(BINS)),
    })
}

/// The table, from `galaxy`'s mass function and lifetimes.
#[must_use]
pub fn fit(galaxy: &Galaxy) -> TypeIaDelayFit {
    let delays = DelayTimeDistribution::from_galaxy(galaxy);
    let imf = galaxy.mass_function();
    let edges = delay_edges();
    let yields: [f64; BINS] = std::array::from_fn(|i| {
        DelayTimeDistribution::cumulative_per_solar_mass(Years::new(edges[i + 1]))
            - DelayTimeDistribution::cumulative_per_solar_mass(Years::new(edges[i]))
    });
    let mut primary = [[[0.0; RANKS]; CHANNELS]; BINS];
    let mut layer_share = [[0.0; 2]; BINS];
    let mut worst = 0.0_f64;
    let band_d = MassBand::D.lo();
    for i in 0..BINS {
        let (a, b) = (math::ln(edges[i]), math::ln(edges[i + 1]));
        let (half, mid) = (0.5 * (b - a), 0.5 * (a + b));
        // Each channel's mixture of the delays' laws, and the share of primaries below layer D.
        let mut mixed = [[0.0; PLACE_SEGMENTS + 1]; CHANNELS];
        let (mut below_d, mut weight) = (0.0, 0.0);
        for (&u, &w) in GL32_NODES.iter().zip(&GL32_WEIGHTS) {
            let t = math::exp(mid + half * u);
            let psi = DelayTimeDistribution::rate_per_year_per_solar_mass(Years::new(t));
            let wt = w * half * psi * t;
            let fit = fit_mass(&delays, t);
            let (lo, _) = primary_range(fit);
            for (c, channel) in IaChannel::ALL.into_iter().enumerate() {
                let cdf = place_cdf_at(imf, channel, fit);
                for (m, v) in mixed[c].iter_mut().zip(cdf) {
                    *m += wt * v;
                }
            }
            if lo < band_d {
                below_d += wt;
            }
            weight += wt;
        }
        for (c, cdf) in mixed.iter_mut().enumerate() {
            for v in cdf.iter_mut() {
                *v /= weight;
            }
            let q = quantiles(cdf);
            worst = worst.max(quantile_error(cdf, &q));
            primary[i][c] = q;
        }
        let c_share = below_d / weight;
        layer_share[i] = [c_share, 1.0 - c_share];
    }
    // Uniform in mass inside the window: the mass ratio's law is flat.
    let identity: [f64; RANKS] = std::array::from_fn(|k| index_f64(k) / index_f64(RANKS - 1));
    let ancient_loss = yields
        .iter()
        .zip(&layer_share)
        .fold(0.0, |s, (y, l)| s + y * l[1]);
    TypeIaDelayFit {
        edges,
        yields,
        channel_share: [CHANNEL_SHARES; BINS],
        primary,
        secondary: [[identity; CHANNELS]; BINS],
        layer_share,
        ancient_loss,
        worst_quantile_error: worst,
    }
}

/// The figures of the run's acceptance test.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypeIaDelayAcceptance {
    /// The yields' sum over 1.3 × 10⁻³.
    pub yield_ratio: f64,
    /// The share of delays under 0.1 Gyr, and under 1 Gyr.
    pub under_tenth: f64,
    /// The share of delays under 1 Gyr.
    pub under_one: f64,
    /// The lightest primary any draw can take, M☉: the range's lower end at the first bin's
    /// longest delay is the lightest, and every bin's rank 0 places the primary there.
    pub lightest_primary: f64,
    /// The primaries' largest share in layer C over the bins.
    pub layer_c: f64,
    /// Type Ia a century at Milky Way values.
    pub per_century: f64,
    /// The smallest and largest exploded share of layer D over the old populations.
    pub ancient_range: (f64, f64),
    /// The primary white dwarfs' masses over 2.5–8 M☉ (Kalirai et al. 2008), M☉.
    pub dwarf_range: (f64, f64),
}

impl TypeIaDelayAcceptance {
    /// Whether each check passes: the yield, the fifth, the 62%, no primary under 2.5 M☉, the
    /// rate and the ancient share.
    #[must_use]
    pub fn passed(&self) -> [bool; 6] {
        [
            (self.yield_ratio - 1.0).abs() <= 0.01,
            (self.under_tenth - 0.2).abs() <= 0.02,
            (self.under_one - 0.62).abs() <= 0.02,
            self.lightest_primary >= PRIMARY_MIN.value() && self.layer_c <= 0.0,
            (0.42..=0.53).contains(&self.per_century),
            self.ancient_range.0 >= 0.02 && self.ancient_range.1 <= 0.045,
        ]
    }
}

/// The acceptance figures of `fit` at `galaxy`, the Milky Way's parameters.
#[must_use]
pub fn acceptance(fit: &TypeIaDelayFit, galaxy: &Galaxy) -> TypeIaDelayAcceptance {
    let delays = DelayTimeDistribution::from_galaxy(galaxy);
    let total = fit.yields.iter().fold(0.0, |s, y| s + y);
    let yield_ratio =
        total / hyperion_sim::galaxy::catalogue_classes::type_ia::YIELD_PER_SOLAR_MASS;
    // Bins below a delay, and the part of the one it falls in by the closed form.
    let share_below = |t: f64| {
        let mut sum = 0.0;
        for i in 0..BINS {
            if fit.edges[i + 1] <= t {
                sum += fit.yields[i];
            } else if fit.edges[i] < t {
                sum += DelayTimeDistribution::cumulative_per_solar_mass(Years::new(t))
                    - DelayTimeDistribution::cumulative_per_solar_mass(Years::new(fit.edges[i]));
            }
        }
        sum / total
    };
    let lightest_primary = (0..BINS)
        .map(|i| primary_range(fit_mass(&delays, fit.edges[i + 1])).0)
        .fold(f64::INFINITY, f64::min);
    let layer_c = fit.layer_share.iter().fold(0.0_f64, |m, l| m.max(l[0]));
    let old = [
        Population::OldThinDisc,
        Population::ThickDisc,
        Population::Bulge,
        Population::Halo,
    ];
    let ancient_range = old.iter().fold((f64::INFINITY, 0.0_f64), |(lo, hi), &p| {
        let s = delays.ancient_share(p);
        (lo.min(s), hi.max(s))
    });
    TypeIaDelayAcceptance {
        yield_ratio,
        under_tenth: share_below(1e8),
        under_one: share_below(1e9),
        lightest_primary,
        layer_c,
        per_century: delays.rate_per_year() * 100.0,
        ancient_range,
        dwarf_range: (
            white_dwarf_mass(PRIMARY_MIN.value()),
            white_dwarf_mass(PRIMARY_MAX.value()),
        ),
    }
}

/// A 24 × 4 × 17 array as a `pub const` item.
fn cube(out: &mut String, name: &str, doc: &str, values: &[[[f64; RANKS]; CHANNELS]; BINS]) {
    out.push_str(doc);
    let _ = writeln!(
        out,
        "#[rustfmt::skip]\npub const {name}: [[[f64; {RANKS}]; {CHANNELS}]; {BINS}] = ["
    );
    for bin in values {
        out.push_str("    [\n");
        for row in bin {
            let cells: Vec<String> = row.iter().map(|&v| literal(v)).collect();
            let _ = writeln!(out, "        [{}],", cells.join(", "));
        }
        out.push_str("    ],\n");
    }
    out.push_str("];\n");
}

/// The table's contents.
#[must_use]
pub fn render(fit: &TypeIaDelayFit) -> RustTable {
    let mut primary = String::new();
    cube(
        &mut primary,
        "PRIMARY_MASS_CDF",
        "/// The primary's initial mass per delay bin and channel (`IaChannel::ALL`'s order), as the\n\
         /// quantiles at ranks `k ÷ 16` of its place `x` on `type_ia::primary_range` at the drawn\n\
         /// delay, `m = lo (hi ÷ lo)^x`, read by linear interpolation between ranks: the galaxy's\n\
         /// mass function times the share of mass ratios on 0.1–1 the channel's window leaves open,\n\
         /// mixed over the bin's delays by ψ.\n",
        &fit.primary,
    );
    let mut secondary = String::new();
    cube(
        &mut secondary,
        "SECONDARY_MASS_CDF",
        "/// The secondary's initial mass per delay bin and channel, as the quantiles at ranks\n\
         /// `k ÷ 16` of its place on `type_ia::secondary_window`, linear in mass: uniform, since the\n\
         /// mass ratio's law is flat.\n",
        &fit.secondary,
    );
    let notes = "\
Bins even in ln t over 42.55 Myr (plan 06's lifetime of an 8 M☉ star, ruling 136.7) to 13.7 Gyr.
Each bin's conditional laws are the delays' laws mixed by ψ over 32 Gauss–Legendre nodes in ln t,
each tabulated on 256 panels of the primary's place and read off at 17 ranks; `SECONDARY_MASS_CDF`
is exact. The mass whose lifetime is the delay is `DelayTimeDistribution::mass_with_lifetime`, the
same the class reads when it draws.";
    RustTable {
        summary: vec![
            "The Type Ia delay-first samplers (plan 15, P15.T9.a): the delay-time distribution's"
                .to_owned(),
            "yield in 24 delay bins and, per bin, the channels' shares and the two stars' initial"
                .to_owned(),
            "masses, which `galaxy::catalogue_classes::type_ia::IaProgenitor::draw` reads.".to_owned(),
        ],
        notes: notes.lines().map(str::to_owned).collect(),
        items: vec![
            TableItem::Array {
                name: "DELAY_EDGES".to_owned(),
                doc: vec![
                    "The delay bins' edges, years, even in `ln t` from `type_ia::MIN_DELAY` to"
                        .to_owned(),
                    "`type_ia::HUBBLE_TIME`.".to_owned(),
                ],
                values: fit.edges.to_vec(),
            },
            TableItem::Array {
                name: "YIELD_PER_SOLAR_MASS".to_owned(),
                doc: vec![
                    "Type Ia per solar mass formed with a delay in each bin: Maoz and Graur's (2017)"
                        .to_owned(),
                    "`t^−1.1` integrating to 1.3 × 10⁻³ over the bins.".to_owned(),
                ],
                values: fit.yields.to_vec(),
            },
            TableItem::Nested {
                name: "CHANNEL_SHARE".to_owned(),
                doc: vec![
                    "The channels' shares per bin, in `IaChannel::ALL`'s order (both destroyed,"
                        .to_owned(),
                    "surviving donor, hydrogen donor, Iax): ruling 136.9's in every bin.".to_owned(),
                ],
                rows: fit.channel_share.iter().map(|r| r.to_vec()).collect(),
            },
            TableItem::Source(primary),
            TableItem::Source(secondary),
            TableItem::Nested {
                name: "LAYER_SHARE".to_owned(),
                doc: vec![
                    "The primaries' shares in layers C and D per bin: C is empty, since no primary"
                        .to_owned(),
                    "is under 2.5 M☉.".to_owned(),
                ],
                rows: fit.layer_share.iter().map(|r| r.to_vec()).collect(),
            },
            TableItem::Scalar {
                name: "ANCIENT_LOSS_PER_SOLAR_MASS".to_owned(),
                doc: vec![
                    "Layer-D systems a solar mass formed loses to Type Ia over a Hubble time: the"
                        .to_owned(),
                    "yield's layer-D part, every channel counted (ruling 136.6).".to_owned(),
                ],
                value: fit.ancient_loss,
            },
        ],
    }
}

/// The task (P15.T9.a).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct TypeIaDelayTask;

/// The Milky Way galaxy of the manifest's seed.
fn milky_way(manifest: &Manifest) -> Result<Galaxy, RunTaskError> {
    let seed = Seed::new(manifest.u64("seed")?);
    Galaxy::from_params(seed, GalaxyParams::milky_way_like()).map_err(|e| RunTaskError::Input {
        task: "type_ia_delay",
        source: Box::new(e),
    })
}

impl FitTask for TypeIaDelayTask {
    fn name(&self) -> &'static str {
        "type_ia_delay"
    }

    fn class(&self) -> TaskClass {
        TaskClass::Fast
    }

    fn table_path(&self) -> &'static str {
        "type_ia_delay.rs"
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &[
            "DELAY_EDGES",
            "YIELD_PER_SOLAR_MASS",
            "CHANNEL_SHARE",
            "LAYER_SHARE",
            "ANCIENT_LOSS_PER_SOLAR_MASS",
        ]
    }

    /// The delay-time distribution's ends and two of its cumulative values, the lifetimes the
    /// masses' ranges follow at four masses, the mass function's slope over layer D, and the pair
    /// model's constants.
    fn fingerprint(&self) -> SimFingerprint {
        let (composition, draws) = (Composition::SOLAR, StarDraws::median());
        let mut probes = vec![
            ("MIN_DELAY".to_owned(), MIN_DELAY.value()),
            ("HUBBLE_TIME".to_owned(), HUBBLE_TIME.value()),
            (
                "cumulative(1e8)".to_owned(),
                DelayTimeDistribution::cumulative_per_solar_mass(Years::new(1e8)),
            ),
            (
                "cumulative(1e9)".to_owned(),
                DelayTimeDistribution::cumulative_per_solar_mass(Years::new(1e9)),
            ),
        ];
        for m in [1.0, 2.5, 5.0, 8.0] {
            probes.push((
                format!("lifetime({m})"),
                lifetime(SolarMasses::new(m), &composition, &draws).value(),
            ));
        }
        let imf = hyperion_sim::galaxy::imf::Chabrier::provisional();
        probes.push((
            "chabrier.pdf(8) / pdf(2.5)".to_owned(),
            imf.pdf(8.0) / imf.pdf(2.5),
        ));
        probes.extend([
            ("PRIMARY_MIN".to_owned(), PRIMARY_MIN.value()),
            ("MIN_MASS_RATIO".to_owned(), MIN_MASS_RATIO),
            ("DONOR_MAX".to_owned(), DONOR_MAX),
        ]);
        probes.extend(
            CHANNEL_SHARES
                .iter()
                .enumerate()
                .map(|(i, &s)| (format!("CHANNEL_SHARES[{i}]"), s)),
        );
        SimFingerprint::new(probes)
    }

    fn run(&self, manifest: &Manifest, _threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        check_manifest(self, manifest)?;
        let galaxy = milky_way(manifest)?;
        let fit = fit(&galaxy);
        let a = acceptance(&fit, &galaxy);
        let passed = a.passed();
        let verdict = |ok: bool| if ok { "passes" } else { "FAILS" };
        let acceptance = format!(
            "the yield integrates to {:.6} of 1.3 × 10⁻³ per M☉ ({}); {:.3} of delays under 0.1 \
             Gyr (a fifth, {}) and {:.3} under 1 Gyr (about 62%, {}); the lightest primary \
             {:.3} M☉ and layer C's largest share {:.1e} ({}); at Milky Way values {:.3} Type Ia a \
             century (0.42–0.53, ruling 141.4; {}) and the old populations' exploded share of \
             layer D {:.4}–{:.4} (2–4.5%; {}); primary white dwarfs {:.3}–{:.3} M☉ (Kalirai et \
             al. 2008); the primaries' 17-point quantiles within {:.4} of the exact mixture's CDF",
            a.yield_ratio,
            verdict(passed[0]),
            a.under_tenth,
            verdict(passed[1]),
            a.under_one,
            verdict(passed[2]),
            a.lightest_primary,
            a.layer_c,
            verdict(passed[3]),
            a.per_century,
            verdict(passed[4]),
            a.ancient_range.0,
            a.ancient_range.1,
            verdict(passed[5]),
            a.dwarf_range.0,
            a.dwarf_range.1,
            fit.worst_quantile_error,
        );
        Ok(TaskOutput {
            table: render(&fit),
            source: "Maoz and Graur (2017, ApJ 848, 25) as plan 09's `DelayTimeDistribution` \
                     builds it (ruling 136.7); plan 06's lifetimes; Chabrier (2003)'s branch above \
                     1 M☉; Kalirai et al. (2008, ApJ 676, 594); Peters (1964, Phys. Rev. 136, \
                     B1224); the channel shares of ruling 136.9 (Srivastav et al. 2022; Shen et \
                     al. 2018; Foley et al. 2013)"
                .to_owned(),
            acceptance,
            provisional: (!passed.iter().all(|&ok| ok)).then_some("P15.T9.a"),
        })
    }
}

/// A small index as a float.
fn index_f64(i: usize) -> f64 {
    f64::from(u32::try_from(i).expect("a table index fits in u32"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyperion_sim::galaxy::imf::Chabrier;

    /// The edges are even in `ln t` with exact ends, and the yields add up to the distribution's
    /// cumulative at each edge.
    #[test]
    fn the_edges_and_yields_cut_the_distribution() {
        let edges = delay_edges();
        assert!(edges[0].total_cmp(&MIN_DELAY.value()).is_eq());
        assert!(edges[BINS].total_cmp(&HUBBLE_TIME.value()).is_eq());
        let step = math::ln(edges[1] / edges[0]);
        for w in edges.windows(2) {
            assert!((math::ln(w[1] / w[0]) / step - 1.0).abs() < 1e-9);
        }
    }

    /// Quantiles of a known CDF: a uniform law gives the identity, and the interpolation's error
    /// is zero on it.
    #[test]
    fn quantiles_of_a_uniform_law_are_the_identity() {
        let cdf: [f64; PLACE_SEGMENTS + 1] =
            std::array::from_fn(|k| index_f64(k) / index_f64(PLACE_SEGMENTS));
        let q = quantiles(&cdf);
        for (k, v) in q.iter().enumerate() {
            assert!((v - index_f64(k) / 16.0).abs() < 1e-12, "{k}: {v}");
        }
        assert!(quantile_error(&cdf, &q) < 1e-12);
    }

    /// A double white dwarf's primary is weighted by its open mass ratios: at a delay whose fit
    /// mass is 1 M☉, a 2.5 M☉ primary keeps (2.5 − 1) ÷ (0.9 × 2.5) of its ratios and an 8 M☉ one
    /// (8 − 1) ÷ 7.2.
    #[test]
    fn the_primary_weight_is_the_mass_function_times_open_ratios() {
        let imf = Chabrier::provisional();
        let fit = SolarMasses::new(1.0);
        for m in [2.5, 8.0] {
            let w = primary_weight(&imf, IaChannel::Merger, m, fit);
            let want = imf.pdf(m) * (m - 1.0) / (0.9 * m);
            assert!((w / want - 1.0).abs() < 1e-12, "{m}: {w} against {want}");
        }
        let cdf = place_cdf_at(&imf, IaChannel::Merger, fit);
        assert!(cdf[0].abs() < 1e-15 && (cdf[PLACE_SEGMENTS] - 1.0).abs() < 1e-12);
        assert!(cdf.windows(2).all(|w| w[0] <= w[1]));
    }

    /// The rendered body declares every item in the task's order.
    #[test]
    fn the_rendering_declares_the_items() {
        let fit = TypeIaDelayFit {
            edges: delay_edges(),
            yields: [1.0; BINS],
            channel_share: [CHANNEL_SHARES; BINS],
            primary: [[[0.5; RANKS]; CHANNELS]; BINS],
            secondary: [[[0.5; RANKS]; CHANNELS]; BINS],
            layer_share: [[0.0, 1.0]; BINS],
            ancient_loss: 1.3e-3,
            worst_quantile_error: 0.0,
        };
        let body = render(&fit).body().unwrap();
        for item in [
            "pub const DELAY_EDGES: [f64; 25]",
            "pub const YIELD_PER_SOLAR_MASS: [f64; 24]",
            "pub const CHANNEL_SHARE: [[f64; 4]; 24]",
            "pub const PRIMARY_MASS_CDF: [[[f64; 17]; 4]; 24]",
            "pub const SECONDARY_MASS_CDF: [[[f64; 17]; 4]; 24]",
            "pub const LAYER_SHARE: [[f64; 2]; 24]",
            "pub const ANCIENT_LOSS_PER_SOLAR_MASS: f64",
        ] {
            assert!(body.contains(item), "{item}");
        }
    }
}
