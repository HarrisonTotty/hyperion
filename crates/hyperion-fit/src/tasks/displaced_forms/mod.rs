//! The displaced-population form table's orbits (plan 15, P15.T6.a–b): the integrator, the
//! births and the run whose histograms T6.c–f fit into `tables::displaced_forms`.
//!
//! The table itself, and its [`FitTask`](crate::task::FitTask), come with the fits (T6.c–f); until
//! then this module is run by `hyperion-fit orbits`, which writes the histograms to
//! `data/cache/displaced/` (git-ignored) or `--out`, and prints their SHA-256 for `tables.lock`.
//!
//! # The run
//!
//! Orbits come in classes ([`classes`]): the thin disc's 56 (eight speed bins by seven age bins),
//! eight speed bins for each of the five old sources (thick disc, halo, bulge, bar, nuclear disc),
//! and an unkicked control for each of the three barred sources (T6.d compares the bar's
//! elongation with it). Each orbit is born ([`births`]), integrated to the epoch in the sim's
//! potential ([`orbits`]), with the rotating bar for the barred sources, and recorded into its
//! class's histograms ([`histogram`]). An orbit is a pure function of the run's seed, its class
//! and its number within it, and the run is cut into chunks by the manifest's `chunk` alone, so
//! the output does not depend on the thread count (plan 15, Design note 2).
//!
//! The galaxy is the Milky Way fixture (`GalaxyParams::milky_way_like`), built with its full
//! potential and plan 08's velocity laws. Speeds are in units of its `v_c`, or of the nuclear
//! disc's own circular speed for that source, and times in `R_d ÷ v_c` (plan 08, Design note 12).
//! An orbit is bound if its energy at the epoch, bar included, cannot carry it to the sim's
//! escape boundary, twice the dark halo's `r₂₀₀` (ruling 91).
//!
//! The manifests are `manifests/displaced_forms.toml` (the production run, about 1.9 × 10⁷
//! orbits: plan 15's 1.2 × 10⁷ disc-born, 1.6 × 10⁶ for each old population but the nuclear disc,
//! and 10⁵ a class for the nuclear disc, ruling 120.3) and `displaced_forms.smoke.toml`. A run is
//! cut into parts that are written as they finish and read back on a later run, so it resumes
//! where it stopped with the same bytes (ruling 120.4; [`run`], [`Resume`]).

pub mod births;
pub mod histogram;
pub mod orbits;

use std::fmt::Write as _;
use std::num::NonZeroUsize;
use std::path::Path;

use hyperion_sim::Seed;
use hyperion_sim::galaxy::consts::LIGHT_YEARS_PER_YEAR_PER_KM_S;
use hyperion_sim::galaxy::displaced::{AGE_EDGES, GalaxyScales, SPEED_BINS, SPEED_EDGES};
use hyperion_sim::galaxy::fields::Fields;
use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::{BuildGalaxyError, Galaxy, PointLy, Population};
use hyperion_sim::math;
use hyperion_sim::units::LightYears;
use sha2::{Digest, Sha256};

use births::{CellSampler, Draws, ThinBirths, ThinHistory, ellipsoid_velocity};
use histogram::{ClassHistogram, Integration};
use orbits::{Bar, OrbitPotential, OrbitState};

use crate::manifest::{Manifest, ManifestParamError};
use crate::parallel::{BuildThreadPoolError, map_reduce_chunks};

/// The task's name, which its manifests carry.
pub const TASK: &str = "displaced_forms";

/// The kick speeds the bins span together, in `v_c`: 0.02 to 6 (P15.T6.b).
pub const KICK_RANGE: (f64, f64) = (0.02, 6.0);

/// Light-years per year per km/s.
const C: f64 = LIGHT_YEARS_PER_YEAR_PER_KM_S;

/// A source of displaced objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Source {
    /// The thin disc, born in the thin birth layer.
    Thin,
    /// The thick disc.
    Thick,
    /// The stellar halo's mixture.
    Halo,
    /// The boxy bulge.
    Bulge,
    /// The long bar.
    LongBar,
    /// The nuclear disc.
    NuclearDisc,
}

impl Source {
    /// The sources with one class per speed bin and no age bins.
    pub const OLD: [Self; 5] = [
        Self::Thick,
        Self::Halo,
        Self::Bulge,
        Self::LongBar,
        Self::NuclearDisc,
    ];

    /// The sources integrated with the rotating bar.
    pub const BARRED: [Self; 3] = [Self::Bulge, Self::LongBar, Self::NuclearDisc];

    /// The source's name in the output.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Thin => "thin",
            Self::Thick => "thick",
            Self::Halo => "halo",
            Self::Bulge => "bulge",
            Self::LongBar => "bar",
            Self::NuclearDisc => "nuclear",
        }
    }

    /// The population the source is born from (the thin source from the young disc's layer).
    #[must_use]
    pub const fn population(self) -> Population {
        match self {
            Self::Thin => Population::YoungThinDisc,
            Self::Thick => Population::ThickDisc,
            Self::Halo => Population::Halo,
            Self::Bulge => Population::Bulge,
            Self::LongBar => Population::LongBar,
            Self::NuclearDisc => Population::NuclearDisc,
        }
    }

    /// Whether its orbits feel the rotating bar.
    #[must_use]
    pub const fn is_barred(self) -> bool {
        matches!(self, Self::Bulge | Self::LongBar | Self::NuclearDisc)
    }

    /// Whether it starts on the circular velocity (the discs) rather than a drawn one.
    #[must_use]
    pub const fn is_disc(self) -> bool {
        matches!(self, Self::Thin | Self::Thick | Self::NuclearDisc)
    }
}

/// One class of orbits: a source, its speed bin (`None` for the unkicked control), and for the
/// thin source its age bin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OrbitClass {
    /// The source.
    pub source: Source,
    /// The speed bin, 0–7; `None` for the unkicked control.
    pub speed: Option<usize>,
    /// The age bin, 0–6, for the thin source.
    pub age: Option<usize>,
}

impl OrbitClass {
    /// The class's name in the output: `thin_s3_a5`, `halo_s0`, `bar_control`.
    #[must_use]
    pub fn name(&self) -> String {
        let mut s = self.source.name().to_owned();
        match self.speed {
            Some(v) => {
                write!(s, "_s{v}").expect("writing to a String cannot fail");
            }
            None => s.push_str("_control"),
        }
        if let Some(a) = self.age {
            write!(s, "_a{a}").expect("writing to a String cannot fail");
        }
        s
    }
}

/// Every class, in the run's order: the thin disc by speed then age, the old sources by source
/// then speed, then the barred sources' controls.
#[must_use]
pub fn classes() -> Vec<OrbitClass> {
    let mut out = Vec::with_capacity(99);
    for speed in 0..SPEED_BINS {
        for age in 0..=AGE_EDGES.len() {
            out.push(OrbitClass {
                source: Source::Thin,
                speed: Some(speed),
                age: Some(age),
            });
        }
    }
    for source in Source::OLD {
        for speed in 0..SPEED_BINS {
            out.push(OrbitClass {
                source,
                speed: Some(speed),
                age: None,
            });
        }
    }
    for source in Source::BARRED {
        out.push(OrbitClass {
            source,
            speed: None,
            age: None,
        });
    }
    out
}

/// The run's parameters, from its manifest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunParams {
    /// The seed of every draw.
    pub seed: u64,
    /// The seed the Milky Way fixture is built with (its halo's lesser progenitors' kinematics).
    pub galaxy_seed: u64,
    /// Orbits per thin-disc class.
    pub disc_orbits: u64,
    /// Orbits per old-source class but the nuclear disc's.
    pub old_orbits: u64,
    /// Orbits per nuclear-disc class (ruling 120.3: 10⁵).
    pub nuclear_orbits: u64,
    /// Orbits per control.
    pub control_orbits: u64,
    /// The step, as a fraction of the circular period at the pericentre: 1 ÷ 200.
    pub step_fraction: f64,
    /// The nuclear disc's step fraction (ruling 120.3: 1 ÷ 100 only if the reduced run shows at
    /// most 10% more refinement).
    pub nuclear_step_fraction: f64,
    /// The least step, years.
    pub min_step_years: f64,
    /// The Jacobi integral's largest relative drift an orbit may keep: 10⁻⁴.
    pub drift_tolerance: f64,
    /// How many times an orbit's step may be halved to meet it.
    pub max_halvings: u32,
    /// The bar's quadrupole amplitude over the circular speed squared at its radius.
    pub bar_strength: f64,
    /// The bar's corotation radius over its half-length; 0 for the fixture's own.
    pub corotation_ratio: f64,
    /// Orbits per chunk of the parallel run.
    pub chunk: u64,
    /// Orbits per part, the unit a run writes to its state directory and resumes from.
    pub part_orbits: u64,
}

impl RunParams {
    /// The parameters of `manifest`.
    ///
    /// # Errors
    ///
    /// [`ManifestParamError`] if a parameter is missing or of the wrong type.
    pub fn from_manifest(manifest: &Manifest) -> Result<Self, ManifestParamError> {
        Ok(Self {
            seed: manifest.u64("seed")?,
            galaxy_seed: manifest.u64("galaxy_seed")?,
            disc_orbits: manifest.u64("disc_orbits_per_class")?,
            old_orbits: manifest.u64("old_orbits_per_class")?,
            nuclear_orbits: manifest.u64("nuclear_orbits_per_class")?,
            control_orbits: manifest.u64("control_orbits")?,
            step_fraction: manifest.f64("step_fraction")?,
            nuclear_step_fraction: manifest.f64("nuclear_step_fraction")?,
            min_step_years: manifest.f64("min_step_years")?,
            drift_tolerance: manifest.f64("drift_tolerance")?,
            max_halvings: u32::try_from(manifest.u64("max_halvings")?)
                .map_err(|_| ManifestParamError::new("max_halvings", "a small integer"))?,
            bar_strength: manifest.f64("bar_strength")?,
            corotation_ratio: manifest.f64("corotation_ratio")?,
            chunk: manifest.u64("chunk")?.max(1),
            part_orbits: manifest.u64("part_orbits")?.max(1),
        })
    }

    /// The orbits of `class`.
    #[must_use]
    pub fn orbits_of(&self, class: &OrbitClass) -> u64 {
        match (class.source, class.speed) {
            (_, None) => self.control_orbits,
            (Source::Thin, Some(_)) => self.disc_orbits,
            (Source::NuclearDisc, Some(_)) => self.nuclear_orbits,
            (_, Some(_)) => self.old_orbits,
        }
    }

    /// The step fraction of `source`'s orbits.
    #[must_use]
    pub fn step_fraction_of(&self, source: Source) -> f64 {
        match source {
            Source::NuclearDisc => self.nuclear_step_fraction,
            Source::Thin | Source::Thick | Source::Halo | Source::Bulge | Source::LongBar => {
                self.step_fraction
            }
        }
    }
}

/// The run could not start or finish.
#[derive(Debug, thiserror::Error)]
pub enum RunOrbitsError {
    /// A manifest parameter is missing or wrong.
    #[error(transparent)]
    Param(#[from] ManifestParamError),
    /// The Milky Way fixture could not be built.
    #[error("the Milky Way fixture could not be built")]
    Galaxy(#[source] BuildGalaxyError),
    /// The thread pool could not be built.
    #[error(transparent)]
    Threads(#[from] BuildThreadPoolError),
    /// A part in the state directory could not be read or written.
    #[error("part {part} in the state directory")]
    Part {
        /// The part's number.
        part: u64,
        /// Why.
        #[source]
        source: std::io::Error,
    },
    /// A part in the state directory is another run's, or malformed.
    #[error("part {part} in the state directory is not this run's: {reason}")]
    ForeignPart {
        /// The part's number.
        part: u64,
        /// Why.
        reason: String,
    },
}

/// What every orbit reads: the galaxy, its scales, the two potentials and the samplers.
struct Setup {
    galaxy: Galaxy,
    scales: GalaxyScales,
    plain: OrbitPotential,
    barred: OrbitPotential,
    thin: ThinBirths,
    history: ThinHistory,
    samplers: Vec<(Source, CellSampler)>,
    oldest_thin: f64,
    escape_potential: f64,
}

impl Setup {
    fn new(params: &RunParams) -> Result<Self, RunOrbitsError> {
        let fixture = GalaxyParams::milky_way_like();
        let half_length = fixture.bar().half_length().value();
        let galaxy = Galaxy::from_params(Seed::new(params.galaxy_seed), fixture)
            .map_err(RunOrbitsError::Galaxy)?
            .with_full_potential();
        let tables = galaxy.potential().clone();
        let scales = GalaxyScales::new(galaxy.params(), &tables);
        let omega = if params.corotation_ratio > 0.0 {
            tables
                .omega(LightYears::new(params.corotation_ratio * half_length))
                .value()
        } else {
            tables.bar_pattern_speed().value()
        };
        let bar = Bar::new(&tables, half_length, params.bar_strength, omega);
        let escape_potential = tables.potential_in_plane(tables.escape_boundary());
        let plain = OrbitPotential::new(tables.clone(), None)
            .expect("a galaxy with its full potential has the grid");
        let barred = OrbitPotential::new(tables, Some(bar))
            .expect("a galaxy with its full potential has the grid");
        let fields: &Fields = galaxy.fields();
        let thin = ThinBirths::new(fields, scales.r_d().value());
        let history = ThinHistory::new(fields);
        let oldest_thin = history.oldest(fields);
        let samplers = Source::OLD
            .iter()
            .map(|&s| (s, CellSampler::new(fields, s.population())))
            .collect();
        Ok(Self {
            galaxy,
            scales,
            plain,
            barred,
            thin,
            history,
            samplers,
            oldest_thin,
            escape_potential,
        })
    }

    fn sampler(&self, source: Source) -> &CellSampler {
        &self
            .samplers
            .iter()
            .find(|(s, _)| *s == source)
            .expect("every old source has a sampler")
            .1
    }

    /// The speed scale of `source`, km/s.
    fn v_ref(&self, source: Source) -> f64 {
        match source {
            Source::NuclearDisc => self.scales.nuclear_v_c().value(),
            Source::Thin | Source::Thick | Source::Halo | Source::Bulge | Source::LongBar => {
                self.scales.v_c().value()
            }
        }
    }

    /// Orbit `k` of class `class` (number `index` in [`classes`]), recorded into `record`.
    fn orbit(
        &self,
        params: &RunParams,
        index: usize,
        class: &OrbitClass,
        k: u64,
        record: &mut ClassHistogram,
    ) {
        let mut draws = Draws::new(params.seed, u64::try_from(index).expect("99 classes"), k);
        let fields = self.galaxy.fields();
        let v_ref = self.v_ref(class.source);
        let tau_unit = self.scales.tau_unit().value();
        // Where, when and how fast before the kick.
        let (p, tau, v0) = match class.source {
            Source::Thin => {
                let p = self.thin.sample(&mut draws);
                let age = class.age.expect("thin classes have an age bin");
                let lo = if age == 0 {
                    0.0
                } else {
                    AGE_EDGES[age - 1] * tau_unit
                };
                let hi = AGE_EDGES
                    .get(age)
                    .map_or(self.oldest_thin, |e| e * tau_unit);
                let tau = self.history.sample(fields, lo, hi, &mut draws);
                (p, tau, self.circular(&p))
            }
            source => {
                let (p, component) = self.sampler(source).sample(fields, &mut draws);
                let ages = fields.component(component).ages();
                let born = ages.cdf(hyperion_sim::units::Years::new(0.0));
                let tau = ages
                    .quantile(born + draws.uniform() * (1.0 - born))
                    .value()
                    .max(0.0);
                let v0 = if source.is_disc() {
                    self.circular(&p)
                } else {
                    let kinematics = self
                        .galaxy
                        .kinematics()
                        .expect("the galaxy is built with its velocity laws");
                    ellipsoid_velocity(kinematics, component, &p, &mut draws).map(|v| v * C)
                };
                (p, tau, v0)
            }
        };
        let kick = kick(class, v_ref, &mut draws);
        let start = OrbitState {
            x: [p.x, p.y, p.z],
            v: core::array::from_fn(|i| v0[i] + kick[i]),
        };
        let potential = if class.source.is_barred() {
            &self.barred
        } else {
            &self.plain
        };
        let step = potential.step_for(
            &start,
            params.step_fraction_of(class.source),
            params.min_step_years,
        );
        let (end, integration) = if tau > 0.0 {
            let checked = potential.integrate_checked(
                start,
                tau,
                step,
                params.drift_tolerance,
                params.max_halvings,
            );
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a whole, non-negative number of steps below 2⁵³"
            )]
            let steps = checked.steps as u64;
            let integration = Integration {
                steps,
                refined: checked.halvings > 0,
                over_tolerance: checked.drift > params.drift_tolerance,
                drift: checked.drift,
            };
            (checked.end, integration)
        } else {
            (start, Integration::default())
        };
        let bound = potential.energy(&end) < self.escape_potential;
        let r = math::hypot(end.x[0], end.x[1]);
        let (cos, sin) = if r > 0.0 {
            (end.x[0] / r, end.x[1] / r)
        } else {
            (1.0, 0.0)
        };
        let scale = 1.0 / (C * v_ref);
        let v = [
            (end.v[0] * cos + end.v[1] * sin) * scale,
            (-end.v[0] * sin + end.v[1] * cos) * scale,
            end.v[2] * scale,
        ];
        record.record(end.x, v, bound, integration);
    }

    /// The circular velocity at `p`, spinward in the plane, ly/yr.
    fn circular(&self, p: &PointLy) -> [f64; 3] {
        let r = math::hypot(p.x, p.y);
        if r <= 0.0 {
            return [0.0; 3];
        }
        let v = self.plain.tables().v_circ(LightYears::new(r)).value() * C;
        [-v * p.y / r, v * p.x / r, 0.0]
    }
}

/// The kick of an orbit of `class` whose speed scale is `v_ref` km/s, ly/yr: log-uniform in `u`
/// within its speed bin, isotropic; none for a control.
fn kick(class: &OrbitClass, v_ref: f64, draws: &mut Draws) -> [f64; 3] {
    match class.speed {
        Some(bin) => {
            let lo = if bin == 0 {
                KICK_RANGE.0
            } else {
                SPEED_EDGES[bin - 1]
            };
            let hi = SPEED_EDGES.get(bin).copied().unwrap_or(KICK_RANGE.1);
            let u = lo * math::exp(draws.uniform() * math::ln(hi / lo));
            draws.direction().map(|c| c * u * v_ref * C)
        }
        None => [0.0; 3],
    }
}

/// Where a run keeps its parts, and how many it may compute before it stops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resume<'a> {
    /// The state directory: one file a part, `part-NNNNNN.txt`, written whole (to a temporary
    /// name, then renamed) once the part is done.
    pub dir: &'a Path,
    /// The most parts to compute in this invocation; `None` for all.
    pub max_parts: Option<u64>,
}

/// The run's orbits laid out by class: every class's first orbit number and the total.
struct Layout {
    classes: Vec<OrbitClass>,
    starts: Vec<u64>,
    total: u64,
}

impl Layout {
    fn new(params: &RunParams) -> Self {
        let classes = classes();
        let mut starts = Vec::with_capacity(classes.len() + 1);
        let mut total = 0_u64;
        for class in &classes {
            starts.push(total);
            total += params.orbits_of(class);
        }
        starts.push(total);
        Self {
            classes,
            starts,
            total,
        }
    }

    fn parts(&self, params: &RunParams) -> u64 {
        self.total.div_ceil(params.part_orbits)
    }
}

/// One part's records: the classes it touched, in order.
type Part = Vec<(usize, ClassHistogram)>;

/// Computes part `part` of the run: its orbits cut into the manifest's chunks, reduced in order.
fn compute_part(
    setup: &Setup,
    params: &RunParams,
    layout: &Layout,
    part: u64,
    threads: NonZeroUsize,
) -> Result<Part, RunOrbitsError> {
    let lo = part * params.part_orbits;
    let hi = (lo + params.part_orbits).min(layout.total);
    let mut records: Part = Vec::new();
    map_reduce_chunks(
        hi - lo,
        params.chunk,
        threads,
        |range| {
            let mut partial: Part = Vec::new();
            for offset in range {
                let i = lo + offset;
                let class = layout.starts.partition_point(|&s| s <= i) - 1;
                if partial.last().is_none_or(|(c, _)| *c != class) {
                    let barred = layout.classes[class].source.is_barred();
                    partial.push((class, ClassHistogram::new(barred)));
                }
                let (_, record) = partial.last_mut().expect("pushed above");
                let orbit = i - layout.starts[class];
                setup.orbit(params, class, &layout.classes[class], orbit, record);
            }
            partial
        },
        |partial| {
            for (class, record) in partial {
                match records.last_mut() {
                    Some((c, r)) if *c == class => r.merge(&record),
                    _ => records.push((class, record)),
                }
            }
        },
    )?;
    Ok(records)
}

/// The header line of part `part` of the run of `manifest_hash`.
fn part_header(manifest_hash: &str, part: u64, parts: u64) -> String {
    format!("# {TASK} part {part} of {parts} manifest-sha256 {manifest_hash}")
}

/// Writes `records` as part `part` into `dir`, whole: to a temporary name, then renamed.
fn write_part(dir: &Path, header: &str, part: u64, records: &Part) -> Result<(), RunOrbitsError> {
    let mut text = String::new();
    text.push_str(header);
    text.push('\n');
    for (class, record) in records {
        record.write(&class.to_string(), &mut text);
    }
    let io = |source| RunOrbitsError::Part { part, source };
    std::fs::create_dir_all(dir).map_err(io)?;
    let path = dir.join(format!("part-{part:06}.txt"));
    let temporary = dir.join(format!("part-{part:06}.txt.partial"));
    std::fs::write(&temporary, text).map_err(io)?;
    std::fs::rename(&temporary, &path).map_err(io)
}

/// Reads part `part` from `dir`, if it is there; an error if it is another run's.
fn read_part(
    dir: &Path,
    header: &str,
    part: u64,
    layout: &Layout,
) -> Result<Option<Part>, RunOrbitsError> {
    let path = dir.join(format!("part-{part:06}.txt"));
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(RunOrbitsError::Part { part, source }),
    };
    let foreign = |reason: String| RunOrbitsError::ForeignPart { part, reason };
    let mut lines = text.lines().peekable();
    if lines.next() != Some(header) {
        return Err(foreign(
            "its header names another manifest or layout".to_owned(),
        ));
    }
    let mut records = Vec::new();
    while lines.peek().is_some() {
        let class_of = |line: &str| -> Option<usize> {
            let index: usize = line.split_whitespace().nth(1)?.parse().ok()?;
            (index < layout.classes.len()).then_some(index)
        };
        let class = lines
            .peek()
            .and_then(|line| class_of(line))
            .ok_or_else(|| foreign("a class line is malformed".to_owned()))?;
        let barred = layout.classes[class].source.is_barred();
        let (_, record) =
            ClassHistogram::read(&mut lines, barred).map_err(|e| foreign(e.to_string()))?;
        records.push((class, record));
    }
    Ok(Some(records))
}

/// Runs the orbits of `params` on `threads` threads: one record per class of [`classes`], in
/// order.
///
/// The orbits are cut into parts of the manifest's `part_orbits`, each computed in the manifest's
/// chunks and merged into the totals in part order, so the output depends on neither the thread
/// count nor where a run was stopped and resumed. With `resume`, each part is written to its
/// state directory when done and read back instead of computed on a later run; the run returns
/// `None` if it stopped at `max_parts` with parts still to do.
///
/// # Errors
///
/// [`RunOrbitsError`] if the fixture or the thread pool cannot be built, or a part cannot be
/// written or is another run's.
///
/// # Panics
///
/// Never: every orbit's number lies in one class's range by construction.
pub fn run(
    params: &RunParams,
    manifest_hash: &str,
    threads: NonZeroUsize,
    resume: Option<Resume<'_>>,
) -> Result<Option<Vec<ClassHistogram>>, RunOrbitsError> {
    let setup = Setup::new(params)?;
    let layout = Layout::new(params);
    let parts = layout.parts(params);
    let mut records: Vec<ClassHistogram> = layout
        .classes
        .iter()
        .map(|c| ClassHistogram::new(c.source.is_barred()))
        .collect();
    let mut computed = 0_u64;
    let mut complete = true;
    for part in 0..parts {
        let header = part_header(manifest_hash, part, parts);
        let loaded = match resume {
            Some(r) => read_part(r.dir, &header, part, &layout)?,
            None => None,
        };
        let part_records = if let Some(records) = loaded {
            records
        } else {
            if resume
                .and_then(|r| r.max_parts)
                .is_some_and(|m| computed >= m)
            {
                complete = false;
                continue;
            }
            let records = compute_part(&setup, params, &layout, part, threads)?;
            if let Some(r) = resume {
                write_part(r.dir, &header, part, &records)?;
            }
            computed += 1;
            records
        };
        for (class, record) in part_records {
            records[class].merge(&record);
        }
    }
    Ok(complete.then_some(records))
}

/// The run's records as text: a header naming the task, the manifest's bytes' SHA-256 and the
/// parameters, then every class ([`ClassHistogram::write`]).
#[must_use]
pub fn render(manifest: &Manifest, records: &[ClassHistogram]) -> String {
    let mut out = String::new();
    writeln!(
        out,
        "# hyperion-fit {TASK} orbit histograms (plan 15, P15.T6.b)"
    )
    .expect("writing to a String cannot fail");
    writeln!(
        out,
        "# manifest-sha256 {}",
        hex(&Sha256::digest(manifest.bytes()))
    )
    .expect("writing to a String cannot fail");
    for (class, record) in classes().iter().zip(records) {
        record.write(&class.name(), &mut out);
    }
    out
}

/// The SHA-256 of `manifest`'s bytes in lowercase hexadecimal: what a run's parts are keyed by.
#[must_use]
pub fn manifest_hash(manifest: &Manifest) -> String {
    sha256_hex(manifest.bytes())
}

/// The SHA-256 of `bytes` in lowercase hexadecimal.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        write!(s, "{b:02x}").expect("writing to a String cannot fail");
    }
    s
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn smoke() -> Manifest {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("manifests/displaced_forms.smoke.toml");
        Manifest::load(&path).unwrap()
    }

    #[test]
    fn there_are_ninety_nine_classes_with_unique_names() {
        let all = classes();
        assert_eq!(all.len(), 99);
        let mut names: Vec<String> = all.iter().map(OrbitClass::name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), 99);
        assert_eq!(all[0].name(), "thin_s0_a0");
        assert_eq!(all[98].name(), "nuclear_control");
    }

    /// P15.T6.b: the smoke run twice gives identical bytes, and on one and on three threads.
    #[test]
    fn the_smoke_run_is_reproducible_and_thread_independent() {
        let manifest = smoke();
        assert_eq!(manifest.task(), TASK);
        let params = RunParams::from_manifest(&manifest).unwrap();
        let hash = manifest_hash(&manifest);
        let go = |threads: usize| {
            run(&params, &hash, NonZeroUsize::new(threads).unwrap(), None)
                .unwrap()
                .expect("a run without a limit completes")
        };
        let one = render(&manifest, &go(1));
        let again = render(&manifest, &go(1));
        let records = go(3);
        let three = render(&manifest, &records);
        assert_eq!(sha256_hex(one.as_bytes()), sha256_hex(again.as_bytes()));
        assert_eq!(one, three);
        for (class, record) in classes().iter().zip(&records) {
            assert_eq!(record.orbits, params.orbits_of(class), "{}", class.name());
            assert_eq!(
                record.in_cube_bound + record.in_cube_unbound + record.outside,
                record.orbits
            );
            assert!(record.over_tolerance <= record.refined, "{}", class.name());
        }
        // Ruling 120.4: stopped after one part, then after two more, then finished, the run gives
        // the uninterrupted run's bytes; a part of another manifest is refused.
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/tmp/displaced_forms_resume_test");
        let _ = std::fs::remove_dir_all(&dir); // A leftover of an earlier run, if any.
        let threads = NonZeroUsize::new(2).unwrap();
        let limited = |max_parts| {
            run(
                &params,
                &hash,
                threads,
                Some(Resume {
                    dir: &dir,
                    max_parts,
                }),
            )
            .unwrap()
        };
        assert!(Layout::new(&params).parts(&params) > 3);
        assert!(limited(Some(1)).is_none());
        assert!(limited(Some(2)).is_none());
        let resumed = limited(None).expect("the last invocation finishes");
        assert_eq!(render(&manifest, &resumed), one);
        let other = run(
            &params,
            "another",
            threads,
            Some(Resume {
                dir: &dir,
                max_parts: None,
            }),
        );
        assert!(matches!(
            other,
            Err(RunOrbitsError::ForeignPart { part: 0, .. })
        ));
        std::fs::remove_dir_all(&dir).unwrap();
        // The slowest thin class stays bound and in the cube, and turns with the disc.
        let slow = &records[0];
        assert_eq!(slow.in_cube_bound, slow.orbits);
        #[expect(clippy::cast_precision_loss, reason = "a small count")]
        let mean_phi = slow.moments[1] / slow.in_cube_bound as f64;
        assert!(mean_phi > 0.8, "{mean_phi}");
    }
}
