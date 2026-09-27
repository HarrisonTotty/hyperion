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
//! The manifests are `manifests/displaced_forms.toml` (the production run, about 1.4 × 10⁷
//! orbits: plan 15's 1.2 × 10⁷ disc-born and 1.6 × 10⁶ per old population) and
//! `displaced_forms.smoke.toml`.

pub mod births;
pub mod histogram;
pub mod orbits;

use std::fmt::Write as _;
use std::num::NonZeroUsize;

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
    /// Orbits per old-source class.
    pub old_orbits: u64,
    /// Orbits per control.
    pub control_orbits: u64,
    /// The step, as a fraction of the circular period at the pericentre: 1 ÷ 200.
    pub step_fraction: f64,
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
            control_orbits: manifest.u64("control_orbits")?,
            step_fraction: manifest.f64("step_fraction")?,
            min_step_years: manifest.f64("min_step_years")?,
            drift_tolerance: manifest.f64("drift_tolerance")?,
            max_halvings: u32::try_from(manifest.u64("max_halvings")?)
                .map_err(|_| ManifestParamError::new("max_halvings", "a small integer"))?,
            bar_strength: manifest.f64("bar_strength")?,
            corotation_ratio: manifest.f64("corotation_ratio")?,
            chunk: manifest.u64("chunk")?.max(1),
        })
    }

    /// The orbits of `class`.
    #[must_use]
    pub fn orbits_of(&self, class: &OrbitClass) -> u64 {
        match (class.source, class.speed) {
            (_, None) => self.control_orbits,
            (Source::Thin, Some(_)) => self.disc_orbits,
            (_, Some(_)) => self.old_orbits,
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
        let step = potential.step_for(&start, params.step_fraction, params.min_step_years);
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

/// Runs the orbits of `params` on `threads` threads: one record per class of [`classes`], in
/// order.
///
/// # Errors
///
/// [`RunOrbitsError`] if the fixture or the thread pool cannot be built.
///
/// # Panics
///
/// Never: every orbit's number lies in one class's range by construction.
pub fn run(
    params: &RunParams,
    threads: NonZeroUsize,
) -> Result<Vec<ClassHistogram>, RunOrbitsError> {
    let setup = Setup::new(params)?;
    let classes = classes();
    let counts: Vec<u64> = classes.iter().map(|c| params.orbits_of(c)).collect();
    let mut starts = Vec::with_capacity(counts.len() + 1);
    let mut total = 0_u64;
    for &n in &counts {
        starts.push(total);
        total += n;
    }
    starts.push(total);
    let mut records: Vec<ClassHistogram> = classes
        .iter()
        .map(|c| ClassHistogram::new(c.source.is_barred()))
        .collect();
    map_reduce_chunks(
        total,
        params.chunk,
        threads,
        |range| {
            let mut partial: Vec<(usize, ClassHistogram)> = Vec::new();
            for i in range {
                let class = starts.partition_point(|&s| s <= i) - 1;
                if partial.last().is_none_or(|(c, _)| *c != class) {
                    partial.push((
                        class,
                        ClassHistogram::new(classes[class].source.is_barred()),
                    ));
                }
                let (_, record) = partial.last_mut().expect("pushed above");
                setup.orbit(params, class, &classes[class], i - starts[class], record);
            }
            partial
        },
        |partial| {
            for (class, record) in partial {
                records[class].merge(&record);
            }
        },
    )?;
    Ok(records)
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
        let one = render(&manifest, &run(&params, NonZeroUsize::MIN).unwrap());
        let again = render(&manifest, &run(&params, NonZeroUsize::MIN).unwrap());
        let records = run(&params, NonZeroUsize::new(3).unwrap()).unwrap();
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
        // The slowest thin class stays bound and in the cube, and turns with the disc.
        let slow = &records[0];
        assert_eq!(slow.in_cube_bound, slow.orbits);
        #[expect(clippy::cast_precision_loss, reason = "a small count")]
        let mean_phi = slow.moments[1] / slow.in_cube_bound as f64;
        assert!(mean_phi > 0.8, "{mean_phi}");
    }
}
