//! Globular-cluster streams per globular cluster (plan 15, P15.T11): the multiplier k by which the
//! sim's [`galaxy::global_list::orphans`](hyperion_sim::galaxy::global_list::orphans) draws
//! Poisson(max(0, k × expected globulars − living globulars with a tube)) orphan streams (plan 10,
//! Design note 10).
//!
//! **P15.T11.a** registered the brainstorm's 1.5, which plan 10's P10.T3.b had entered by hand.
//! **P15.T11.b**, revision 1, fits k against the known streams. Plan 10's generator runs at the
//! Milky Way's values ([`GalaxyParams::milky_way_like`]) in several galaxies of their seeds: each
//! galaxy's living globulars' tubes and a sample of its orphans, drawn past the galaxy's own count
//! ([`DebrisPlan::orphan_beyond_count`]), are sprayed and reduced to their tube tables (P10.T5–T6)
//! and observed from the plane at 8 kpc through the detectability cut ([`observe`]). An orphan's
//! draws are keyed by its index alone and the multiplier sets only how many there are, so the
//! orphans of every k on the grid are the first of one sequence, and the count a galaxy shows at k
//! is its living streams seen plus the share of its orphans seen times the mean of its orphans'
//! Poisson count, `max(0, k N − L)`: the expected count, with the Poisson draw of how many orphans
//! replaced by its mean. The count is averaged over the galaxies and over observers spaced evenly
//! in azimuth, and k is where it meets the census's globular-like streams inside the same distance
//! and latitude ([`census`]), the one-sigma interval where it meets that count plus or minus its
//! square root, each linear between the grid's nodes.
//!
//! The table holds two items: the held multiplier, the brainstorm's 1.5, which every generator
//! version below 27 reads, and the fitted one, `ORPHAN_STREAMS_PER_GLOBULAR_27`, which the sim's
//! `orphan_streams` hold reads from 27 on (the `displaced_forms_26` pattern). The value is a
//! parameter of the generator version whatever the fit says.
//!
//! The census is galstreams v1.2.1 (Mateu 2023, MNRAS 520, 5225): Bonaca and Price-Whelan's
//! count (2025, New Astron. Rev. 100, 101713, §2.2) could not be reached for this fit, and their
//! "over 120 reported, wide and thin" stands as the plan cites it.

pub mod census;
pub mod observe;

use std::num::NonZeroUsize;

use hyperion_sim::Seed;
use hyperion_sim::galaxy::consts::LIGHT_YEARS_PER_KILOPARSEC;
use hyperion_sim::galaxy::global_list::orphans::MASS_LAW;
use hyperion_sim::galaxy::global_list::share::GLOBULAR_TUBES_INSIDE;
use hyperion_sim::galaxy::global_list::spray::{Spray, SprayForce, SprayParams, TRACERS_PER_ARM};
use hyperion_sim::galaxy::global_list::tube::{KERNEL_TRACERS, KNOTS_PER_ARM};
use hyperion_sim::galaxy::global_list::{
    Debris, DebrisPlan, StreamNumber, StreamOrigin, StreamSpec, TubeTable,
};
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::{Galaxy, Population};

use self::census::{Census, StreamKind};
use self::observe::{Cut, SmoothHalo, observers, seen_masses};
use crate::emit::{RustTable, TableItem};
use crate::manifest::{Manifest, ManifestParamError, SimFingerprint};
use crate::parallel::map_reduce_chunks;
use crate::task::{FitTask, RunTaskError, TaskClass, TaskOutput, check_manifest};

/// The task's revision: 1, P15.T11.b's fit against the known streams.
pub const VERSION: u32 = 1;

/// The task's name.
const TASK: &str = "streams";

/// The census of known streams the fit counts against.
pub const DATASET: &str = "stream_census";

/// The brainstorm's multiplier, the held item that every generator version below 27 reads
/// ("Streams and accreted structure": "globular streams number about 1.5 per globular").
pub const BRAINSTORM: f64 = 1.5;

/// The multiples of the cut's contrast the fit is made at, the cut's own first: then none, half and
/// twice, for how far the cut moves k.
pub const CONTRAST_PROBES: [f64; 4] = [1.0, 0.0, 0.5, 2.0];

/// How much farther from the centre than the observer's reach a progenitor's pericentre may lie
/// for its tube to be built: the reach is the observer's radius plus the cut's distance, and a
/// tube lies inside its pericentre by at most its turning points' spread, about `η r ÷ 2` with `η`
/// some hundredths (plan 10, "Bounding shells"), so a quarter more misses none and saves the
/// sprays of streams that never come near.
pub const REACH_MARGIN: f64 = 1.25;

/// The fit's parameters, from its manifest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FitParams {
    /// The grid of k: its least and greatest nodes and the steps between them.
    pub grid: (f64, f64, u32),
    /// The first galaxy's seed; galaxy `n` takes `seed + n`.
    pub seed: u64,
    /// The galaxies drawn at the Milky Way's values.
    pub galaxies: u32,
    /// The orphans drawn in each galaxy, from index 0.
    pub orphans: u64,
    /// The observers' distance from the centre, ly.
    pub observer_radius_ly: f64,
    /// The observers in each galaxy, evenly spaced in azimuth.
    pub observers: u32,
    /// The detectability cut.
    pub cut: Cut,
}

impl FitParams {
    /// The parameters `manifest` gives.
    ///
    /// # Errors
    ///
    /// [`ManifestParamError`] for a parameter that is missing or out of its range: a grid that is
    /// not increasing and positive with at least one step, no galaxy, no orphan or more than
    /// 2,048, no observer, or a cut constant that is not positive and finite (the latitude below
    /// 90°, the contrast non-negative).
    pub fn from_manifest(manifest: &Manifest) -> Result<Self, ManifestParamError> {
        let positive = |key: &str| {
            let v = manifest.f64(key)?;
            if v.is_finite() && v > 0.0 {
                Ok(v)
            } else {
                Err(ManifestParamError::new(key, "positive and finite"))
            }
        };
        let count = |key: &str, max: u32| {
            let v = manifest.u64(key)?;
            u32::try_from(v)
                .ok()
                .filter(|v| (1..=max).contains(v))
                .ok_or_else(|| ManifestParamError::new(key, &format!("1 to {max}")))
        };
        let (low, high) = (positive("grid_low")?, positive("grid_high")?);
        if high <= low {
            return Err(ManifestParamError::new("grid_high", "above grid_low"));
        }
        let steps = count("grid_steps", 10_000)?;
        let latitude = positive("min_latitude_deg")?;
        if latitude >= 90.0 {
            return Err(ManifestParamError::new("min_latitude_deg", "below 90"));
        }
        let contrast = manifest.f64("min_contrast")?;
        if !(contrast.is_finite() && contrast >= 0.0) {
            return Err(ManifestParamError::new(
                "min_contrast",
                "non-negative and finite",
            ));
        }
        Ok(Self {
            grid: (low, high, steps),
            seed: manifest.u64("seed")?,
            galaxies: count("galaxies", 1_000)?,
            orphans: u64::from(count("orphans_per_galaxy", 2_048)?),
            observer_radius_ly: positive("observer_kpc")? * LIGHT_YEARS_PER_KILOPARSEC,
            observers: count("observers", 360)?,
            cut: Cut {
                min_mass_solar_masses: positive("min_mass_msun")?,
                max_distance_ly: positive("max_distance_kpc")? * LIGHT_YEARS_PER_KILOPARSEC,
                min_contrast: contrast,
                min_latitude: latitude.to_radians(),
            },
        })
    }

    /// The grid's nodes of k, from the least to the greatest.
    #[must_use]
    pub fn grid(&self) -> Vec<f64> {
        let (low, high, steps) = self.grid;
        (0..=steps)
            .map(|i| low + (high - low) * f64::from(i) / f64::from(steps))
            .collect()
    }
}

/// What the observers of one galaxy see of its globular streams.
#[derive(Debug, Clone, PartialEq)]
pub struct GalaxySeen {
    /// The galaxy's seed.
    pub seed: u64,
    /// Its expected globulars, N, plan 02's untruncated count.
    pub globulars: u32,
    /// Its living globulars with a tube, L.
    pub living: u32,
    /// The orphans drawn, from index 0.
    pub orphans: u64,
    /// The living streams seen, entry `o × P + p` for observer `o` and contrast probe `p` of
    /// [`CONTRAST_PROBES`].
    pub living_seen: Vec<u32>,
    /// The orphans seen, by observer and probe as `living_seen`.
    pub orphans_seen: Vec<u32>,
    /// The orphans drawn that found no orbit, and so are none.
    pub orphans_dropped: u32,
    /// The tubes built: the streams that could reach the observers.
    pub tubes: u32,
    /// The streams whose spray or tube failed, which count as unseen.
    pub failed: u32,
}

impl GalaxySeen {
    /// The count observer `o` expects at k with contrast probe `p`: the living streams it sees
    /// plus its share of the orphans times their mean count, `max(0, k N − L)` (module
    /// documentation).
    ///
    /// # Panics
    ///
    /// If `o` is not one of the galaxy's observers or `p` not one of [`CONTRAST_PROBES`].
    #[must_use]
    pub fn expected(&self, k: f64, o: usize, p: usize) -> f64 {
        let at = o * CONTRAST_PROBES.len() + p;
        #[expect(
            clippy::cast_precision_loss,
            reason = "orphans drawn number at most 2,048"
        )]
        let share = f64::from(self.orphans_seen[at]) / self.orphans as f64;
        let mean = (k * f64::from(self.globulars) - f64::from(self.living)).max(0.0);
        f64::from(self.living_seen[at]) + share * mean
    }

    /// [`expected`](Self::expected) averaged over the galaxy's observers, summed in their order.
    #[must_use]
    pub fn expected_mean(&self, k: f64, p: usize) -> f64 {
        let observers = self.living_seen.len() / CONTRAST_PROBES.len();
        let sum = (0..observers).fold(0.0, |sum, o| sum + self.expected(k, o, p));
        #[expect(clippy::cast_precision_loss, reason = "at most 360 observers")]
        let n = observers as f64;
        sum / n
    }
}

/// The fit: the counts on the grid and where they meet the census.
#[derive(Debug, Clone, PartialEq)]
pub struct StreamsFit {
    /// The census's globular-like streams inside the cut, the count matched.
    pub observed: u32,
    /// The census's streams, and those it marks dwarf, disc and wide.
    pub census: [u32; 4],
    /// What each galaxy shows.
    pub galaxies: Vec<GalaxySeen>,
    /// The grid's nodes of k.
    pub grid: Vec<f64>,
    /// The mean expected count at each node, for each contrast probe.
    pub counts: Vec<[f64; 4]>,
    /// k at each contrast probe, where its counts meet the census's; `None` off the grid.
    pub k: [Option<f64>; 4],
    /// The one-sigma interval of k at the cut's contrast: where the counts meet the census's count
    /// less and plus its square root.
    pub interval: (Option<f64>, Option<f64>),
    /// k of each galaxy alone, at the cut's contrast.
    pub per_galaxy: Vec<Option<f64>>,
    /// The share of the halo's budget the globular tubes hold at k, inside 65,000 ly, as the
    /// sim's discrete share counts them; `None` without a k.
    pub share: Option<f64>,
}

impl StreamsFit {
    /// Whether the acceptance holds: k on the grid, inside 0.5–4.5, with both ends of its
    /// interval, and the globular tubes at 0.3–1% of the halo's budget there.
    #[must_use]
    pub fn passes(&self) -> bool {
        let inside = |k: Option<f64>| k.is_some_and(|k| (0.5..=4.5).contains(&k));
        inside(self.k[0])
            && inside(self.interval.0)
            && inside(self.interval.1)
            && self.share.is_some_and(|s| (0.003..=0.01).contains(&s))
    }

    /// The value committed: k, or the grid's end nearest the census's count where the counts do
    /// not reach it.
    #[must_use]
    pub fn value(&self) -> f64 {
        self.k[0].unwrap_or_else(|| {
            let (first, last) = (self.counts[0][0], self.counts[self.counts.len() - 1][0]);
            if f64::from(self.observed) < first.min(last) {
                self.grid[0]
            } else {
                self.grid[self.grid.len() - 1]
            }
        })
    }

    /// The fitted item's doc comment: its value, interval and how far the cut moves it.
    #[must_use]
    pub fn doc(&self, params: &FitParams) -> Vec<String> {
        let shown =
            |k: Option<f64>| k.map_or_else(|| "off the grid".to_owned(), |k| format!("{k:.2}"));
        let (low, high, _) = params.grid;
        let cut = params.cut;
        wrap(&format!(
            "Globular-cluster streams per globular cluster of the untruncated system, living \
             clusters' tubes and orphans together, from generator version 27 (P15.T11.b): {}, one \
             sigma {}–{}. Fitted on a grid of {low:.2}–{high:.2} so that plan 10's generator at the \
             Milky Way's values ({} galaxies), seen from the plane at {:.0} kpc through the \
             detectability cut (above {:.0} M☉ of a stream within {:.0} kpc, where its axis is at \
             least {:.0} times as dense as the smooth halo and more than {:.0}° from the plane), \
             shows the {} globular-like known streams the census finds within the same distance \
             and latitude (galstreams v1.2.1, Mateu 2023, MNRAS 520, 5225; `data/stream_census`), \
             with their Poisson error. The cut moves it: {} without the density condition, {} at \
             {:.0} times the halo and {} at {:.0}.",
            shown(self.k[0]),
            shown(self.interval.0),
            shown(self.interval.1),
            self.galaxies.len(),
            params.observer_radius_ly / LIGHT_YEARS_PER_KILOPARSEC,
            cut.min_mass_solar_masses,
            cut.max_distance_ly / LIGHT_YEARS_PER_KILOPARSEC,
            cut.min_contrast,
            cut.min_latitude.to_degrees(),
            self.observed,
            shown(self.k[1]),
            shown(self.k[2]),
            cut.min_contrast * CONTRAST_PROBES[2],
            shown(self.k[3]),
            cut.min_contrast * CONTRAST_PROBES[3],
        ))
    }

    /// The acceptance line: the figures the header and the lock file record.
    #[must_use]
    pub fn acceptance(&self) -> String {
        let shown = |k: Option<f64>| k.map_or_else(|| "none".to_owned(), |k| format!("{k:.3}"));
        let per: Vec<f64> = self.per_galaxy.iter().flatten().copied().collect();
        let (least, most) = per
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &k| {
                (a.min(k), b.max(k))
            });
        let tubes: u32 = self.galaxies.iter().map(|g| g.tubes).sum();
        let failed: u32 = self.galaxies.iter().map(|g| g.failed).sum();
        format!(
            "k {} (one sigma {}–{}; 0.5–4.5) against {} ± {:.1} globular-like known streams \
             inside the cut ({} in the census: {} dwarf, {} disc, {} wide); globular tubes {} of \
             the halo at k (0.3–1%); without the density condition {}, at half and twice the \
             contrast {} and {}; per galaxy {}; {} galaxies, {} tubes built, {} failed",
            shown(self.k[0]),
            shown(self.interval.0),
            shown(self.interval.1),
            self.observed,
            f64::from(self.observed).sqrt(),
            self.census[0],
            self.census[1],
            self.census[2],
            self.census[3],
            self.share
                .map_or_else(|| "none".to_owned(), |s| format!("{:.3}%", 100.0 * s)),
            shown(self.k[1]),
            shown(self.k[2]),
            shown(self.k[3]),
            if per.is_empty() {
                "none".to_owned()
            } else {
                format!("{least:.3}–{most:.3} over {}", per.len())
            },
            self.galaxies.len(),
            tubes,
            failed,
        )
    }
}

/// Where the piecewise-linear `counts` on `grid` first reach `target`, linear between nodes;
/// `None` if they stay below it or start above it.
#[must_use]
pub fn crossing(grid: &[f64], counts: &[f64], target: f64) -> Option<f64> {
    if counts.first().is_some_and(|&c| c > target) {
        return None;
    }
    grid.windows(2)
        .zip(counts.windows(2))
        .find(|(_, c)| c[0] <= target && target <= c[1] && c[1] > c[0])
        .map(|(k, c)| k[0] + (target - c[0]) / (c[1] - c[0]) * (k[1] - k[0]))
}

/// The fit over `galaxies` of `params` against the census's `observed` streams (module
/// documentation).
#[must_use]
pub fn fit_of(params: &FitParams, census: &Census, galaxies: Vec<GalaxySeen>) -> StreamsFit {
    let grid = params.grid();
    let observed = census.globular_in_cut();
    #[expect(clippy::cast_precision_loss, reason = "at most 1,000 galaxies")]
    let n = galaxies.len() as f64;
    let counts: Vec<[f64; 4]> = grid
        .iter()
        .map(|&k| {
            core::array::from_fn(|p| {
                galaxies.iter().fold(0.0, |s, g| s + g.expected_mean(k, p)) / n
            })
        })
        .collect();
    let column = |p: usize| -> Vec<f64> { counts.iter().map(|c| c[p]).collect() };
    let target = f64::from(observed);
    let k: [Option<f64>; 4] = core::array::from_fn(|p| crossing(&grid, &column(p), target));
    let sigma = target.sqrt();
    let interval = (
        crossing(&grid, &column(0), target - sigma),
        crossing(&grid, &column(0), target + sigma),
    );
    let per_galaxy = galaxies
        .iter()
        .map(|g| {
            let own: Vec<f64> = grid.iter().map(|&k| g.expected_mean(k, 0)).collect();
            crossing(&grid, &own, target)
        })
        .collect();
    let milky_way = GalaxyParams::milky_way_like();
    let share = k[0].map(|k| {
        k * f64::from(milky_way.accretion().globular_count())
            * MASS_LAW.mean_solar_masses()
            * GLOBULAR_TUBES_INSIDE
            / milky_way.population_mass(Population::Halo).value()
    });
    StreamsFit {
        observed,
        census: [
            census.total(),
            census.of_kind(StreamKind::Dwarf),
            census.of_kind(StreamKind::Disc),
            census.of_kind(StreamKind::Wide),
        ],
        galaxies,
        grid,
        counts,
        k,
        interval,
        per_galaxy,
        share,
    }
}

/// What the observers of `params` see of the globular streams of the Milky Way-like galaxy of
/// `seed`, its tubes built on `threads` threads (module documentation).
///
/// # Errors
///
/// [`RunTaskError::Input`] if the galaxy cannot be built, [`RunTaskError::Threads`] if the pool
/// cannot.
///
/// # Panics
///
/// Never for a manifest [`FitParams::from_manifest`] accepts: the streams' count and indices fit
/// every integer type they pass through, at most 2,048 orphans and 4,096 living tubes.
pub fn observe_galaxy(
    seed: Seed,
    params: &FitParams,
    threads: NonZeroUsize,
) -> Result<GalaxySeen, RunTaskError> {
    let input = |source: Box<dyn std::error::Error + Send + Sync>| RunTaskError::Input {
        task: TASK,
        source,
    };
    let galaxy = Galaxy::from_params(seed, GalaxyParams::milky_way_like())
        .map_err(|e| input(Box::new(e)))?
        .with_full_potential();
    let living: Vec<StreamSpec> = Debris::generate(&galaxy)
        .streams()
        .iter()
        .filter(|s| matches!(s.origin(), StreamOrigin::LivingGlobular(_)))
        .cloned()
        .collect();
    let plan = Debris::plan(&galaxy);
    let orphans = numbered_orphans(&plan, &galaxy, living.len(), params.orphans)
        .map_err(|e| input(e.into()))?;
    let tables = galaxy.potential();
    let force = SprayForce::new(tables).map_err(|e| input(Box::new(e)))?;
    let halo = SmoothHalo::of(&galaxy);
    let eyes = observers(params.observer_radius_ly, params.observers);
    let contrasts = CONTRAST_PROBES.map(|m| m * params.cut.min_contrast);
    let reach = REACH_MARGIN * (params.observer_radius_ly + params.cut.max_distance_ly);
    let mean_mass = galaxy.mean_system_mass(Population::Halo);
    let specs: Vec<Option<&StreamSpec>> = living
        .iter()
        .map(Some)
        .chain(orphans.iter().map(Option::as_ref))
        .collect();
    let width = eyes.len() * contrasts.len();
    let mut outcomes: Vec<Seen> = Vec::with_capacity(specs.len());
    let n = u64::try_from(specs.len()).expect("a few thousand streams");
    map_reduce_chunks(
        n,
        1,
        threads,
        |range| {
            range
                .map(|i| {
                    let index = usize::try_from(i).expect("an index below the streams' count");
                    let Some(spec) = specs[index] else {
                        return Seen::None;
                    };
                    if spec.mass().value() <= params.cut.min_mass_solar_masses
                        || spec.orbit().pericentre().value() > reach
                    {
                        return Seen::Far;
                    }
                    let tube = Spray::new(seed, &force, tables, spec, &SprayParams::default())
                        .ok()
                        .and_then(|spray| {
                            TubeTable::from_tracers(seed, mean_mass, spec, &spray.tracers()).ok()
                        });
                    tube.map_or(Seen::Failed, |tube| {
                        Seen::Tube(seen_masses(&tube, &halo, &eyes, &params.cut, &contrasts))
                    })
                })
                .collect::<Vec<_>>()
        },
        |chunk| outcomes.extend(chunk),
    )?;
    let mut living_seen = vec![0; width];
    let mut orphans_seen = vec![0; width];
    let (mut tubes, mut failed) = (0, 0);
    for (i, s) in outcomes.iter().enumerate() {
        let into = if i < living.len() {
            &mut living_seen
        } else {
            &mut orphans_seen
        };
        match s {
            Seen::Tube(masses) => {
                tubes += 1;
                for (count, &mass) in into.iter_mut().zip(masses) {
                    if mass > params.cut.min_mass_solar_masses {
                        *count += 1;
                    }
                }
            }
            Seen::Failed => failed += 1,
            Seen::None | Seen::Far => {}
        }
    }
    let dropped = orphans.iter().filter(|o| o.is_none()).count();
    Ok(GalaxySeen {
        seed: seed.get(),
        globulars: galaxy.params().accretion().globular_count(),
        living: u32::try_from(living.len()).expect("at most 4,096 living tubes"),
        orphans: params.orphans,
        living_seen,
        orphans_seen,
        orphans_dropped: u32::try_from(dropped).expect("at most 2,048 orphans"),
        tubes,
        failed,
    })
}

/// What became of one stream.
#[derive(Debug, Clone, PartialEq)]
enum Seen {
    /// An orphan that found no orbit.
    None,
    /// Too light or too far for the cut: no tube is built.
    Far,
    /// Its spray or its reduction failed.
    Failed,
    /// Its tube, and the mass each observer sees at each contrast.
    Tube(Vec<f64>),
}

/// The first `count` orphans of `plan`, numbered as the list would number them under a multiplier
/// that drew them all: after the `living` living globulars, in the order found.
///
/// # Errors
///
/// A message if a number would pass the list's 4,096 streams.
fn numbered_orphans(
    plan: &DebrisPlan,
    galaxy: &Galaxy,
    living: usize,
    count: u64,
) -> Result<Vec<Option<StreamSpec>>, String> {
    let mut next = living;
    let mut orphans = Vec::with_capacity(usize::try_from(count).unwrap_or(0));
    for k in 0..count {
        let number = u16::try_from(next)
            .ok()
            .and_then(|n| StreamNumber::new(n).ok())
            .ok_or_else(|| format!("orphan {k} would be stream {next}, past the list's 4,096"))?;
        let orphan = plan.orphan_beyond_count(galaxy, k, number);
        if orphan.is_some() {
            next += 1;
        }
        orphans.push(orphan);
    }
    Ok(orphans)
}

/// The widest a line of the table's docs or notes runs, in characters, its comment prefix aside.
const TEXT_WIDTH: usize = 96;

/// `text` wrapped to [`TEXT_WIDTH`], broken at spaces.
#[must_use]
fn wrap(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split(' ').filter(|w| !w.is_empty()) {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > TEXT_WIDTH {
            lines.push(core::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    lines.push(line);
    lines
}

/// The table's contents for the fitted multiplier `fitted`, whose doc comment is `fitted_doc`.
#[must_use]
pub fn render(fitted: f64, fitted_doc: Vec<String>) -> RustTable {
    RustTable {
        summary: vec![
            "The streams' multiplier: how many globular-cluster streams a galaxy has per globular (plan 15,"
                .to_owned(),
            "P15.T11; read by plan 10, Design note 10).".to_owned(),
        ],
        notes: wrap(
            "Two items: the brainstorm's 1.5, which plan 10's P10.T3.b entered by hand and \
             P15.T11.a registered, read below generator version 27, and P15.T11.b's fit, read from \
             27 on by the sim's `orphan_streams` hold. The fit counts the thin, globular-like \
             streams of the known census, which is far from complete: Bonaca and Price-Whelan \
             (2025, New Astron. Rev. 100, 101713, §2.2) report over 120 streams, wide and thin \
             together, and galstreams v1.2.1 (Mateu 2023, MNRAS 520, 5225) lists 141. Plan 15 \
             holds the value uncertain threefold. It is a parameter of the generator version \
             whatever the fit says.",
        ),
        items: vec![
            TableItem::Scalar {
                name: "ORPHAN_STREAMS_PER_GLOBULAR".to_owned(),
                doc: vec![
                    "Globular-cluster streams per globular cluster of the untruncated system, living clusters'"
                        .to_owned(),
                    "tubes and orphans together: 1.5 (brainstorm, \"Streams and accreted structure\": \"globular"
                        .to_owned(),
                    "streams number about 1.5 per globular, most of them orphans whose cluster is gone\")."
                        .to_owned(),
                ],
                value: BRAINSTORM,
            },
            TableItem::Scalar {
                name: "ORPHAN_STREAMS_PER_GLOBULAR_27".to_owned(),
                doc: fitted_doc,
                value: fitted,
            },
        ],
    }
}

/// The task: P15.T11.b's fit of the multiplier against the known streams, slow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct StreamsTask;

/// The census's file in its dataset.
const CENSUS_FILE: &str = "census.csv";

impl FitTask for StreamsTask {
    fn name(&self) -> &'static str {
        TASK
    }

    fn class(&self) -> TaskClass {
        TaskClass::Slow
    }

    fn table_path(&self) -> &'static str {
        "streams.rs"
    }

    fn revision(&self) -> u32 {
        VERSION
    }

    fn items(&self) -> &'static [&'static str] {
        &[
            "ORPHAN_STREAMS_PER_GLOBULAR",
            "ORPHAN_STREAMS_PER_GLOBULAR_27",
        ]
    }

    /// The Milky Way fixture's globulars and halo, the smooth halo's density beyond the cut that
    /// the cut compares against, the orphans' mean mass and the share's inside fraction, and the
    /// spray's and the reduction's sizes. The fit's sprays, tubes and orbits are the generator's
    /// own, and its potential is probed by the tasks of plan 02's tables.
    fn fingerprint(&self) -> SimFingerprint {
        let milky_way = GalaxyParams::milky_way_like();
        let mut probes = vec![
            (
                "milky_way.globular_count".to_owned(),
                f64::from(milky_way.accretion().globular_count()),
            ),
            (
                "milky_way.population_mass(Halo) (M☉)".to_owned(),
                milky_way.population_mass(Population::Halo).value(),
            ),
            (
                "orphans::MASS_LAW.mean_solar_masses()".to_owned(),
                MASS_LAW.mean_solar_masses(),
            ),
            (
                "share::GLOBULAR_TUBES_INSIDE".to_owned(),
                GLOBULAR_TUBES_INSIDE,
            ),
            (
                "spray::TRACERS_PER_ARM".to_owned(),
                f64::from(TRACERS_PER_ARM),
            ),
            ("tube::KNOTS_PER_ARM".to_owned(), f64::from(KNOTS_PER_ARM)),
            ("tube::KERNEL_TRACERS".to_owned(), f64::from(KERNEL_TRACERS)),
        ];
        if let Ok(galaxy) = Galaxy::from_params(Seed::new(0x0f11_b000_0000_0000), milky_way) {
            let halo = SmoothHalo::of(&galaxy);
            let ly = LIGHT_YEARS_PER_KILOPARSEC;
            for (name, at) in [
                (
                    "smooth halo at (8, 0, 10) kpc (M☉/ly³)",
                    [8.0 * ly, 0.0, 10.0 * ly],
                ),
                (
                    "smooth halo at (0, 30, 15) kpc (M☉/ly³)",
                    [0.0, 30.0 * ly, 15.0 * ly],
                ),
            ] {
                probes.push((name.to_owned(), halo.density(at)));
            }
        }
        SimFingerprint::new(probes)
    }

    fn run(&self, manifest: &Manifest, threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError> {
        check_manifest(self, manifest)?;
        let params = FitParams::from_manifest(manifest)?;
        let dataset = manifest.load_dataset(DATASET)?;
        let input = |why: String| RunTaskError::Input {
            task: TASK,
            source: why.into(),
        };
        let bytes = dataset
            .files
            .iter()
            .find(|(name, _)| name == CENSUS_FILE)
            .map(|(_, bytes)| bytes)
            .ok_or_else(|| input(format!("the census dataset holds no {CENSUS_FILE}")))?;
        let text = std::str::from_utf8(bytes).map_err(|e| input(e.to_string()))?;
        let census = Census::parse(text).map_err(input)?;
        let mut galaxies = Vec::with_capacity(usize::try_from(params.galaxies).unwrap_or(0));
        for n in 0..params.galaxies {
            let seed = Seed::new(params.seed.wrapping_add(u64::from(n)));
            galaxies.push(observe_galaxy(seed, &params, threads)?);
        }
        let fit = fit_of(&params, &census, galaxies);
        let provisional = (!fit.passes()).then_some("P15.T11.b");
        Ok(TaskOutput {
            table: render(fit.value(), fit.doc(&params)),
            source: "the known streams of galstreams v1.2.1 (Mateu 2023, MNRAS 520, 5225; \
                     github.com/cmateu/galstreams), each marked globular-like or not by its \
                     progenitor or its width (`data/stream_census`), against plan 10's spray and \
                     tubes at the Milky Way's values"
                .to_owned(),
            acceptance: fit.acceptance(),
            provisional,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    /// A manifest of `text`.
    fn manifest(text: &str) -> Manifest {
        Manifest::from_bytes(Path::new("streams.toml"), text.as_bytes().to_vec())
            .expect("the test's manifest parses")
    }

    const PARAMS: &str = "task = \"streams\"\ndatasets = [\"stream_census\"]\n[params]\n\
        grid_low = 0.5\ngrid_high = 4.5\ngrid_steps = 40\nseed = 1\ngalaxies = 2\n\
        orphans_per_galaxy = 16\nobserver_kpc = 8.0\nobservers = 4\nmin_mass_msun = 1000.0\n\
        max_distance_kpc = 30.0\nmin_contrast = 10.0\nmin_latitude_deg = 15.0\n";

    #[test]
    fn the_parameters_are_the_manifests_in_the_generator_s_units() {
        let params = FitParams::from_manifest(&manifest(PARAMS)).unwrap();
        assert_eq!(params.grid, (0.5, 4.5, 40));
        let grid = params.grid();
        assert_eq!(grid.len(), 41);
        assert!((grid[1] - 0.6).abs() < 1e-12 && (grid[40] - 4.5).abs() < 1e-12);
        assert!((params.observer_radius_ly - 26_092.510_216).abs() < 1e-3);
        assert!((params.cut.min_latitude - 15.0_f64.to_radians()).abs() < 1e-15);
        let bad = PARAMS.replace("grid_high = 4.5", "grid_high = 0.4");
        assert_eq!(
            FitParams::from_manifest(&manifest(&bad)),
            Err(ManifestParamError::new("grid_high", "above grid_low"))
        );
        let none = PARAMS.replace("galaxies = 2", "galaxies = 0");
        assert_eq!(
            FitParams::from_manifest(&manifest(&none)),
            Err(ManifestParamError::new("galaxies", "1 to 1000"))
        );
    }

    #[test]
    fn the_crossing_is_linear_between_nodes_and_none_off_the_grid() {
        let grid = [1.0, 2.0, 3.0];
        let counts = [10.0, 30.0, 50.0];
        assert!((crossing(&grid, &counts, 20.0).unwrap() - 1.5).abs() < 1e-15);
        assert!((crossing(&grid, &counts, 50.0).unwrap() - 3.0).abs() < 1e-15);
        assert_eq!(crossing(&grid, &counts, 60.0), None);
        assert_eq!(crossing(&grid, &counts, 5.0), None);
    }

    /// A galaxy's expected count is its living streams seen plus its share of orphans seen times
    /// their mean count, which never falls below none.
    #[test]
    fn a_galaxy_expects_its_living_streams_and_its_share_of_orphans() {
        let probes = CONTRAST_PROBES.len();
        let g = GalaxySeen {
            seed: 0,
            globulars: 100,
            living: 30,
            orphans: 200,
            living_seen: vec![6; probes],
            orphans_seen: vec![50; probes],
            orphans_dropped: 0,
            tubes: 0,
            failed: 0,
        };
        // 6 + 50 ÷ 200 × (2 × 100 − 30).
        assert!((g.expected(2.0, 0, 0) - 48.5).abs() < 1e-12);
        assert!((g.expected(0.2, 0, 0) - 6.0).abs() < 1e-12);
        assert!((g.expected_mean(2.0, 0) - 48.5).abs() < 1e-12);
    }

    #[test]
    fn the_table_holds_the_held_and_the_fitted_multipliers() {
        let table = render(2.25, vec!["Fitted.".to_owned()]);
        assert_eq!(
            table.item_names(),
            [
                "ORPHAN_STREAMS_PER_GLOBULAR",
                "ORPHAN_STREAMS_PER_GLOBULAR_27"
            ]
        );
        let body = table.body().unwrap();
        assert!(
            body.contains("gone\").\npub const ORPHAN_STREAMS_PER_GLOBULAR: f64 = 1.5;\n"),
            "{body}"
        );
        assert!(
            body.ends_with("/// Fitted.\npub const ORPHAN_STREAMS_PER_GLOBULAR_27: f64 = 2.25;\n"),
            "{body}"
        );
    }

    #[test]
    fn the_run_refuses_another_tasks_manifest() {
        let result = StreamsTask.run(
            &manifest(&PARAMS.replace("task = \"streams\"", "task = \"chabrier\"")),
            NonZeroUsize::MIN,
        );
        assert!(
            matches!(
                &result,
                Err(RunTaskError::WrongManifest { task: "streams", found }) if found == "chabrier"
            ),
            "{result:?}"
        );
    }

    /// The orphans the fit draws past the galaxy's count, numbered as [`numbered_orphans`] numbers
    /// them, begin with the list's own orphans, bit for bit, numbers and all: the fit's orphans
    /// at the galaxy's multiplier are the generator's.
    #[test]
    fn the_orphans_drawn_past_the_count_begin_with_the_list_s_own() {
        let galaxy = Galaxy::from_params(Seed::new(7), GalaxyParams::milky_way_like())
            .unwrap()
            .with_full_potential();
        let debris = Debris::generate(&galaxy);
        let plan = Debris::plan(&galaxy);
        let living = debris
            .streams()
            .iter()
            .filter(|s| matches!(s.origin(), StreamOrigin::LivingGlobular(_)))
            .count();
        let count = plan.orphan_count();
        let drawn = numbered_orphans(&plan, &galaxy, living, count + 32).unwrap();
        let listed: Vec<&StreamSpec> = debris
            .streams()
            .iter()
            .filter(|s| s.origin() == StreamOrigin::Orphan)
            .collect();
        let found: Vec<&StreamSpec> = drawn.iter().flatten().collect();
        assert!(
            found.len() > listed.len(),
            "{} {}",
            found.len(),
            listed.len()
        );
        assert_eq!(&found[..listed.len()], &listed[..]);
        assert_eq!(
            usize::from(found[listed.len()].number().get()),
            living + listed.len()
        );
    }

    #[test]
    fn the_text_wraps_at_its_width() {
        let words = "word ".repeat(40);
        let lines = wrap(&words);
        assert!(lines.iter().all(|l| l.chars().count() <= TEXT_WIDTH));
        assert_eq!(lines.join(" "), words.trim_end());
    }
}
