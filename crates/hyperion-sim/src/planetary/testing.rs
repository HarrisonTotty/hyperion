//! Test helpers of the planetary stage (plan 14's Provides): synthetic hosts and samples of real
//! systems, for this crate's tests and, behind its `testing` feature, for other crates'.
//!
//! - [`synthetic_star`] and [`synthetic_binary`] are [`SystemContext::builder`] with the median
//!   star's draws, under a caller-supplied [`SystemId`] so that the stage's streams differ between
//!   samples. The builder itself gives the other choices: plan 06's own draws of each star
//!   ([`SyntheticDraws::OfUniverse`](super::context::SyntheticDraws::OfUniverse)), another
//!   sphere of influence, an encounter environment.
//! - [`sample_contexts`] draws real systems from a galaxy of the Milky Way's parameters: the
//!   systems nearest a Sun-like point that pass a [`SampleFilter`], for the statistical tests
//!   (design note 20).
//! - For this crate's tests alone, `with_every_24_hold` runs a closure with every landed hold at
//!   generator version 24 in force together, so that a test of a 24 figure reads 24 as it will
//!   ship (decision-t24i-airless-state D3.3).
//! - [`partial_pressures`] states an air by hand, for other crates' tests of what a derivation
//!   reads of one.

use std::error::Error;
use std::fmt;

use super::context::{BuildSystemContextError, SystemContext};
use super::derive::atmosphere::{Gas, PartialPressures};
use crate::Seed;
use crate::coords::GalacticPosition;
use crate::galaxy::imf::MassBand;
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::placement::{LayerSpec, STELLAR_LAYERS, SystemRecord, generate_cell};
use crate::galaxy::query::{QuerySphere, cells_in_sphere, count_cells_in_sphere};
use crate::galaxy::{BuildGalaxyError, Galaxy};
use crate::id::SystemId;
use crate::orbit::Eccentricity;
use crate::time::UniverseTime;
use crate::units::{Dex, LightYears, Metres, Pascals, SolarMasses, Years};

/// An air's partial pressures stated by hand, each gas's `pressure(gas)` in [`Gas::ALL`]'s order,
/// unvalidated, as the crate's own tests state one: for other crates' tests of what a derivation
/// reads of an air, as the server's conversion of P14.T51.j's oxidised surface does (a caller's
/// values are its own to check, as `SurfaceModifiers::derive` checks its inputs').
#[must_use]
pub fn partial_pressures(pressure: impl Fn(Gas) -> Pascals) -> PartialPressures {
    PartialPressures::from_fn(pressure)
}

/// A single star of initial mass `mass` (0.08–150 M☉), \[Fe/H\] `fe_h` and age at the epoch
/// `age`, as the system `id`: the builder's defaults otherwise (the median star's draws, the
/// solar neighbourhood's sphere of influence).
///
/// # Errors
///
/// As [`SystemContextBuilder::build`](super::context::SystemContextBuilder::build): a mass, age or
/// \[Fe/H\] out of range.
pub fn synthetic_star(
    id: SystemId,
    mass: SolarMasses,
    fe_h: Dex,
    age: Years,
) -> Result<SystemContext, BuildSystemContextError> {
    SystemContext::builder()
        .system(id)
        .star(mass)
        .fe_h(fe_h)
        .age_at_epoch(age)
        .build()
}

/// A binary as the system `id`: a primary of `m1` M☉ and a companion of `m2` M☉ (a brown dwarf
/// below 0.08 M☉) on a relative orbit of semi-major axis `a` and eccentricity `e`, in the
/// reference plane with the companion at periapsis at the epoch, with \[Fe/H\] `fe_h` and age at
/// the epoch `age`; the builder's defaults otherwise.
///
/// # Errors
///
/// As [`SystemContextBuilder::build`](super::context::SystemContextBuilder::build): masses, age or
/// \[Fe/H\] out of range, a companion heavier than the primary, an orbit that cannot be built or
/// whose period or eccentricity plan 11's draw would not give, or one beyond the tidal cut.
pub fn synthetic_binary(
    id: SystemId,
    m1: SolarMasses,
    m2: SolarMasses,
    a: Metres,
    e: Eccentricity,
    fe_h: Dex,
    age: Years,
) -> Result<SystemContext, BuildSystemContextError> {
    SystemContext::builder()
        .system(id)
        .binary(m1, m2, a, e)
        .fe_h(fe_h)
        .age_at_epoch(age)
        .build()
}

/// The Sun-like point about which [`sample_contexts`] draws, light-years in the galactic frame:
/// in the plane, 26,000 ly out along +y, clear of the bar, where the other plans' tests take the
/// solar neighbourhood.
pub const SAMPLE_CENTRE_LY: [f64; 3] = [0.0, 26_000.0, 0.0];

/// The radius of the first sphere [`sample_contexts`] searches, light-years; it doubles until
/// the sphere holds enough systems.
const FIRST_RADIUS_LY: f64 = 16.0;

/// The most generation cells one sphere of [`sample_contexts`] may cover, summed over the layers
/// it walks: 2¹⁸, a sphere of about 315 ly in layer A, which holds some 10⁵ systems at the Sun's
/// density.
pub const MAX_SAMPLE_CELLS: u64 = 1 << 18;

/// Which systems [`sample_contexts`] keeps: a band of primary initial mass, and a test of the
/// record.
///
/// The band also decides which layers are walked at all, so a filter for M dwarfs generates no
/// cell of the massive stars' layers. A filter on anything a record does not carry (\[Fe/H\],
/// the stars' states) is a test of the contexts afterwards.
#[derive(Clone, Copy)]
pub struct SampleFilter {
    lo: SolarMasses,
    hi: SolarMasses,
    keep: fn(&SystemRecord) -> bool,
}

impl SampleFilter {
    /// Every system.
    pub const ALL: Self = Self {
        lo: SolarMasses::ZERO,
        hi: SolarMasses::new(f64::INFINITY),
        keep: keep_every,
    };

    /// The systems whose primary's initial mass is at least `lo` and below `hi`, M☉.
    #[must_use]
    pub const fn primary_masses(lo: SolarMasses, hi: SolarMasses) -> Self {
        Self {
            lo,
            hi,
            keep: keep_every,
        }
    }

    /// This filter, keeping only the records for which `keep` is also true.
    #[must_use]
    pub const fn keeping(self, keep: fn(&SystemRecord) -> bool) -> Self {
        Self { keep, ..self }
    }

    /// Whether `record` passes.
    #[must_use]
    fn passes(&self, record: &SystemRecord) -> bool {
        let m = record.primary_initial_mass().value();
        self.lo.value() <= m && m < self.hi.value() && (self.keep)(record)
    }

    /// Whether any primary of `band`, whose masses the placement draws from the closed band, can
    /// pass.
    #[must_use]
    fn may_pass(&self, band: MassBand) -> bool {
        band.lo() < self.hi.value() && self.lo.value() <= band.hi()
    }
}

impl fmt::Debug for SampleFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SampleFilter")
            .field("lo", &self.lo)
            .field("hi", &self.hi)
            .finish_non_exhaustive()
    }
}

/// The record test of a filter that keeps every record.
#[must_use]
fn keep_every(_: &SystemRecord) -> bool {
    true
}

/// The contexts of `n` real systems of the galaxy of `seed` with the Milky Way's parameters
/// ([`GalaxyParams::milky_way_like`]), for the statistical tests: [`sample_records`] of that
/// galaxy, each through [`SystemContext::from_record`], which is what
/// [`SystemContext::for_system`] gives its ID.
///
/// Pass the same `seed` to the planetary generators to generate the systems of that universe. A
/// context costs a stellar track, about a millisecond.
///
/// # Errors
///
/// - [`SampleContextsError::Galaxy`] if the galaxy cannot be built.
/// - [`SampleContextsError::TooFewSystems`] as [`sample_records`].
pub fn sample_contexts(
    n: usize,
    seed: Seed,
    filter: SampleFilter,
) -> Result<Vec<SystemContext>, SampleContextsError> {
    let galaxy = Galaxy::from_params(seed, GalaxyParams::milky_way_like())
        .map_err(SampleContextsError::Galaxy)?;
    Ok(sample_records(&galaxy, n, filter)?
        .iter()
        .map(|record| SystemContext::from_record(&galaxy, record))
        .collect())
}

/// The `n` systems of `galaxy` nearest [`SAMPLE_CENTRE_LY`] at the epoch that pass `filter`,
/// nearest first, ties broken by ID.
///
/// A volume-limited sample, so over every mass it is the galaxy's own mixture of masses,
/// populations and ages at the solar circle. It searches spheres from 16 ly, doubling the radius
/// until one holds `n` systems that pass, in the layers the filter's mass band reaches. The
/// answer is a pure function of the galaxy, `n` and the filter, and the first `k` of it are the
/// sample of `k`.
///
/// # Errors
///
/// [`SampleContextsError::TooFewSystems`] if no layer holds the filter's masses, or if the sphere
/// that would hold `n` of them covers more than [`MAX_SAMPLE_CELLS`] cells.
///
/// # Panics
///
/// Never: the Sun-like point lies inside the root cube, and every sphere searched has a positive,
/// finite radius.
pub fn sample_records(
    galaxy: &Galaxy,
    n: usize,
    filter: SampleFilter,
) -> Result<Vec<SystemRecord>, SampleContextsError> {
    let centre = GalacticPosition::from_light_years(SAMPLE_CENTRE_LY)
        .expect("the Sun-like point lies inside the root cube");
    let layers: Vec<_> = STELLAR_LAYERS
        .iter()
        .filter(|spec| filter.may_pass(spec.band()))
        .map(LayerSpec::layer)
        .collect();
    let mut radius = FIRST_RADIUS_LY;
    let mut found = Vec::new();
    let mut cell = Vec::new();
    loop {
        let sphere = QuerySphere::new(
            centre,
            LightYears::new(radius),
            UniverseTime::EPOCH,
            LightYears::ZERO,
        )
        .expect("a positive, finite radius at the epoch, where no pad is needed");
        let cells: u64 = layers
            .iter()
            .map(|&layer| count_cells_in_sphere(layer, &sphere))
            .sum();
        if layers.is_empty() || cells > MAX_SAMPLE_CELLS {
            return Err(SampleContextsError::TooFewSystems {
                wanted: n,
                found: found.len(),
            });
        }
        let limit = Metres::from(LightYears::new(radius));
        found.clear();
        for &layer in &layers {
            for key in cells_in_sphere(layer, &sphere) {
                generate_cell(galaxy, key, &mut cell);
                for record in &cell {
                    let distance = centre.distance_to(record.epoch_position());
                    if distance <= limit && filter.passes(record) {
                        found.push((distance, *record));
                    }
                }
            }
        }
        if found.len() >= n {
            found
                .sort_by(|(d1, r1), (d2, r2)| d1.total_cmp(d2).then_with(|| r1.id().cmp(&r2.id())));
            return Ok(found
                .into_iter()
                .take(n)
                .map(|(_, record)| record)
                .collect());
        }
        radius *= 2.0;
    }
}

/// [`sample_contexts`] or [`sample_records`] could not draw its sample.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SampleContextsError {
    /// The galaxy could not be built.
    Galaxy(BuildGalaxyError),
    /// Fewer systems pass the filter than were asked for, inside the largest sphere searched.
    TooFewSystems {
        /// The number asked for.
        wanted: usize,
        /// The number found in the last sphere searched.
        found: usize,
    },
}

impl fmt::Display for SampleContextsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Galaxy(_) => f.write_str("the sample's galaxy cannot be built"),
            Self::TooFewSystems { wanted, found } => write!(
                f,
                "only {found} of the {wanted} systems asked for pass the filter near the sun-like \
                 point"
            ),
        }
    }
}

impl Error for SampleContextsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Galaxy(error) => Some(error),
            Self::TooFewSystems { .. } => None,
        }
    }
}

/// `run`'s result with every landed hold at generator version 24 in force together on this
/// thread, for a test that asserts a 24 figure (decision-t24i-airless-state D3.3, from P14.T24.i's
/// follow-up): T53.a's census set the escape's rules alone, and T24.i's and T49.n's tests the unlit
/// mean's alone, so the floor's absence on the escape's batch path fell between them. Each switch
/// sets its rules back as it returns, a panic in `run` included, and a test may set one hold back
/// inside it (`with_unlit_mean(UnlitMeanRules::Held, ..)`, say) to compare 24 with and without it.
///
/// The holds, each with its switch:
///
/// - the escape's, `ESCAPE_VERSION` (P14.T53.a), at `EscapeRules::Batch` through `with_rules`,
///   which switches the Rayleigh floor's, `RAYLEIGH_FLOOR_VERSION` (P14.T24.h.a), with it;
/// - the unlit mean's, `UNLIT_MEAN_VERSION` (P14.T24.i), at `UnlitMeanRules::Floored` through
///   `with_unlit_mean`;
/// - the moon bulk's, `MOON_BULK_VERSION` (P14.T56.f), at `MoonBulkRules::Batch` through
///   `with_moon_bulk`;
/// - the cold traps', `COLD_TRAPS_VERSION` (P14.T51.f), at `ColdTrapRules::Batch` through
///   `with_cold_trap_rules`, which a system reads as it is generated (its `RecordRules`);
/// - the read densities', `READ_DENSITIES_VERSION` (P14.T49.n), at `ReadDensitiesRules::Read`
///   through `with_read_densities`: solid argon's and the other phases P14.T49.n read, and the
///   liquids by temperature;
/// - the compressed liquids', `COMPRESSED_LIQUIDS_VERSION` (P14.T49.n), at
///   `CompressedLiquidsRules::Compressed` through `with_compressed_liquids`: every liquid the
///   record reads compressed to its body's surface pressure.
/// - the row darkening's, `ROW_DARKENING_VERSION` (P14.T49.k), at `RowDarkeningRules::Darkened`
///   through `with_row_darkening`: the crystalline rows darkened by their cited fine phases
///   (`basalt_reduced`'s graphite, `ferric`'s nanophase oxide);
/// - the ocean floor's, `OCEAN_FLOOR_VERSION` (P14.T51.g.a), at `OceanFloorRules::Floored`
///   through `with_ocean_floor`: a regular moon's published ocean ends at the shallower of its
///   rock and ice V's melting pressure at its 273 K base;
/// - the small bodies' types', `SMALL_BODY_TYPES_VERSION` (P14.T51.e), at
///   `SmallBodyRules::Batch` through `with_small_body_rules`, which a system reads as it is
///   generated (its `RecordRules`): every planet's, member's and moon's taxonomic class, not
///   modelled where the record's modifiers are, which the air's sections' hold (`AirRules`, set
///   through `PlanetarySystem::with_record_rules` alone) keeps off the record;
/// - the tenuous air's, `TENUOUS_VERSION` (P14.T53.c), at `TenuousRules::Tenuous` through
///   `with_tenuous`: the ice-held nitrogen and argon at their ice's own temperature, the `Tenuous`
///   state above its floor, and no ocean under a `Snowball`'s ice in its liquids;
/// - the ocean's, `OCEAN_VERSION` (P14.T51.g.b), at `OceanRules::Solved` through
///   `with_ocean_rules`: every body's subsurface ocean is the solve's, and a moon's ocean branch
///   its spherical shell's (it supersedes the ocean floor's arm where both are in force).
///
/// A system or a record is read under the rules in force when it is generated: one generated
/// outside the helper (the tests' caches, `whole` and `golden_systems`, are version 23's) is not
/// 24's, so a test generates afresh inside it. A later hold at 24 adds its switch here, and every
/// 24 test then reads it. P14.T54.b deletes this helper with the holds.
#[cfg(test)]
pub(crate) fn with_every_24_hold<R>(run: impl FnOnce() -> R) -> R {
    use crate::planetary::chem::escape::{EscapeRules, with_rules};
    use crate::planetary::chem::tenuous::{TenuousRules, with_tenuous};
    use crate::planetary::derive::atmosphere::{UnlitMeanRules, with_unlit_mean};
    use crate::planetary::hooks::cold_traps::{ColdTrapRules, with_cold_trap_rules};
    use crate::planetary::hooks::ocean::{
        OceanFloorRules, OceanRules, with_ocean_floor, with_ocean_rules,
    };
    use crate::planetary::system::moon_ice::{MoonBulkRules, with_moon_bulk};
    use crate::planetary::system::small_bodies::{SmallBodyRules, with_small_body_rules};
    use crate::substance::optics::{RowDarkeningRules, with_row_darkening};
    use crate::substance::{
        CompressedLiquidsRules, ReadDensitiesRules, with_compressed_liquids, with_read_densities,
    };
    with_rules(EscapeRules::Batch, || {
        with_unlit_mean(UnlitMeanRules::Floored, || {
            with_moon_bulk(MoonBulkRules::Batch, || {
                with_cold_trap_rules(ColdTrapRules::Batch, || {
                    with_read_densities(ReadDensitiesRules::Read, || {
                        with_compressed_liquids(CompressedLiquidsRules::Compressed, || {
                            with_row_darkening(RowDarkeningRules::Darkened, || {
                                with_ocean_floor(OceanFloorRules::Floored, || {
                                    with_small_body_rules(SmallBodyRules::Batch, || {
                                        with_tenuous(TenuousRules::Tenuous, || {
                                            with_ocean_rules(OceanRules::Solved, run)
                                        })
                                    })
                                })
                            })
                        })
                    })
                })
            })
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::placement::Existence;
    use crate::id::Layer;
    use crate::time::UniverseTime;
    use crate::units::AstronomicalUnits;

    const SEED: u64 = 0x0e14_0001_d000_0005;

    fn galaxy() -> Galaxy {
        Galaxy::from_params(Seed::new(SEED), GalaxyParams::milky_way_like())
            .expect("the Milky Way fixture's gas is mostly neutral")
    }

    fn centre() -> GalacticPosition {
        GalacticPosition::from_light_years(SAMPLE_CENTRE_LY).unwrap()
    }

    #[test]
    fn a_sample_is_the_nearest_systems_that_pass_nearest_first() {
        let galaxy = galaxy();
        let sample = sample_records(&galaxy, 400, SampleFilter::ALL).unwrap();
        assert_eq!(sample.len(), 400);
        let distances: Vec<Metres> = sample
            .iter()
            .map(|r| centre().distance_to(r.epoch_position()))
            .collect();
        assert!(distances.windows(2).all(|d| d[0] <= d[1]));
        let mut ids: Vec<SystemId> = sample.iter().map(SystemRecord::id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 400, "no system twice");
        // Every system of the five layers inside the farthest sampled distance is in the sample.
        let farthest = LightYears::from(distances[399]);
        let sphere =
            QuerySphere::new(centre(), farthest, UniverseTime::EPOCH, LightYears::ZERO).unwrap();
        let mut cell = Vec::new();
        let mut inside = 0;
        for spec in STELLAR_LAYERS {
            for key in cells_in_sphere(spec.layer(), &sphere) {
                generate_cell(&galaxy, key, &mut cell);
                inside += cell
                    .iter()
                    .filter(|r| centre().distance_to(r.epoch_position()) <= distances[399])
                    .count();
            }
        }
        assert_eq!(inside, 400);
        // A volume-limited sample is mostly M dwarfs.
        let layer_a = sample.iter().filter(|r| r.layer() == Layer::A).count();
        assert!(layer_a > 200, "{layer_a} of 400 in layer A");
        // The sample of k is the first k of a larger one.
        assert_eq!(
            sample_records(&galaxy, 150, SampleFilter::ALL).unwrap(),
            sample[..150]
        );
    }

    #[test]
    fn a_filter_keeps_its_band_and_its_test() {
        let galaxy = galaxy();
        let (lo, hi) = (SolarMasses::new(0.8), SolarMasses::new(1.2));
        let old = |r: &SystemRecord| r.age_at_epoch().value() > 1e9;
        let filter = SampleFilter::primary_masses(lo, hi).keeping(old);
        let sample = sample_records(&galaxy, 60, filter).unwrap();
        assert_eq!(sample.len(), 60);
        for record in &sample {
            let m = record.primary_initial_mass();
            assert!(lo <= m && m < hi, "{m:?}");
            assert!(old(record));
            assert_eq!(record.layer(), Layer::C);
        }
        // Only layer C can hold these masses, so only it is walked.
        assert!(!filter.may_pass(MassBand::B) && filter.may_pass(MassBand::C));
        assert!(!filter.may_pass(MassBand::D));
        // A layer-B primary may be drawn at exactly 0.75 M☉, so a band from there walks B too.
        let edge = SampleFilter::primary_masses(SolarMasses::new(0.75), hi);
        assert!(edge.may_pass(MassBand::B) && edge.may_pass(MassBand::C));
    }

    #[test]
    fn a_filter_no_layer_can_meet_is_refused() {
        let galaxy = galaxy();
        let heavy = SampleFilter::primary_masses(SolarMasses::new(200.0), SolarMasses::new(300.0));
        assert_eq!(
            sample_records(&galaxy, 1, heavy),
            Err(SampleContextsError::TooFewSystems {
                wanted: 1,
                found: 0
            })
        );
    }

    #[test]
    fn a_sample_the_largest_sphere_cannot_hold_is_refused() {
        let galaxy = galaxy();
        let none = SampleFilter::primary_masses(SolarMasses::new(0.08), SolarMasses::new(0.5))
            .keeping(|_| false);
        assert_eq!(
            sample_records(&galaxy, 1, none),
            Err(SampleContextsError::TooFewSystems {
                wanted: 1,
                found: 0
            })
        );
    }

    #[test]
    fn sampled_contexts_are_their_systems_contexts() {
        let galaxy = galaxy();
        let seed = Seed::new(SEED);
        let contexts = sample_contexts(24, seed, SampleFilter::ALL).unwrap();
        assert_eq!(contexts.len(), 24);
        let records = sample_records(&galaxy, 24, SampleFilter::ALL).unwrap();
        for (context, record) in contexts.iter().zip(&records) {
            assert_eq!(context.id(), record.id());
            assert_eq!(
                Ok(context),
                SystemContext::for_system(&galaxy, record.id()).as_ref()
            );
        }
        // The same seed, the same sample.
        assert_eq!(
            contexts,
            sample_contexts(24, seed, SampleFilter::ALL).unwrap()
        );
        // Every sampled system exists at the epoch in the solar neighbourhood's mixture.
        assert!(
            contexts
                .iter()
                .all(|c| c.existence_at(UniverseTime::EPOCH) == Existence::Exists)
        );
    }

    #[test]
    fn the_synthetic_hosts_are_the_builder_s() {
        let key = crate::galaxy::placement::CellKey::new(Layer::C, [0, 812, 0]).unwrap();
        let id = key.candidate_id(5).unwrap();
        let (m, fe_h, age) = (SolarMasses::new(0.8), Dex::new(0.1), Years::new(3e9));
        assert_eq!(
            synthetic_star(id, m, fe_h, age),
            SystemContext::builder()
                .system(id)
                .star(m)
                .fe_h(fe_h)
                .age_at_epoch(age)
                .build()
        );
        let (a, e) = (
            Metres::from(AstronomicalUnits::new(40.0)),
            Eccentricity::new(0.4).unwrap(),
        );
        let pair = synthetic_binary(id, m, SolarMasses::new(0.3), a, e, fe_h, age).unwrap();
        assert_eq!(pair.stars().len(), 2);
        assert_eq!(
            Ok(pair),
            SystemContext::builder()
                .system(id)
                .binary(m, SolarMasses::new(0.3), a, e)
                .fe_h(fe_h)
                .age_at_epoch(age)
                .build()
        );
        assert!(matches!(
            synthetic_star(id, SolarMasses::new(-1.0), fe_h, age),
            Err(BuildSystemContextError::MassOutsideRange { star: 0, .. })
        ));
    }

    /// Inside [`with_every_24_hold`] every landed hold at 24 gives its 24 rules, one set back
    /// inside it gives its hold's alone, and outside it each gives the version's, 23's (P14.T24.i's
    /// follow-up, decision-t24i-airless-state D3.3).
    #[test]
    fn every_24_hold_is_in_force_inside_the_helper() {
        use crate::planetary::chem::escape::EscapeRules;
        use crate::planetary::chem::tenuous::TenuousRules;
        use crate::planetary::derive::atmosphere::{UnlitMeanRules, with_unlit_mean};
        use crate::planetary::derive::photometry::area_mix::RayleighFloorRules;
        use crate::planetary::hooks::cold_traps::ColdTrapRules;
        use crate::planetary::hooks::ocean::{OceanFloorRules, OceanRules};
        use crate::planetary::system::moon_ice::MoonBulkRules;
        use crate::substance::optics::RowDarkeningRules;
        use crate::substance::{CompressedLiquidsRules, ReadDensitiesRules, with_read_densities};
        let rules = || {
            (
                EscapeRules::in_force(),
                RayleighFloorRules::in_force(),
                UnlitMeanRules::in_force(),
                MoonBulkRules::in_force(),
                ColdTrapRules::in_force(),
                ReadDensitiesRules::in_force(),
                CompressedLiquidsRules::in_force(),
                RowDarkeningRules::in_force(),
                OceanFloorRules::in_force(),
                TenuousRules::in_force(),
                OceanRules::in_force(),
            )
        };
        let at_24 = (
            EscapeRules::Batch,
            RayleighFloorRules::Floored,
            UnlitMeanRules::Floored,
            MoonBulkRules::Batch,
            ColdTrapRules::Batch,
            ReadDensitiesRules::Read,
            CompressedLiquidsRules::Compressed,
            RowDarkeningRules::Darkened,
            OceanFloorRules::Floored,
            TenuousRules::Tenuous,
            OceanRules::Solved,
        );
        let at_23 = (
            EscapeRules::Held,
            RayleighFloorRules::Held,
            UnlitMeanRules::Held,
            MoonBulkRules::Held,
            ColdTrapRules::Held,
            ReadDensitiesRules::Held,
            CompressedLiquidsRules::Saturated,
            RowDarkeningRules::Held,
            OceanFloorRules::Held,
            TenuousRules::Held,
            OceanRules::Held,
        );
        assert_eq!(rules(), at_23, "the version's, outside the helper");
        assert_eq!(with_every_24_hold(rules), at_24);
        assert_eq!(
            with_every_24_hold(|| {
                crate::planetary::chem::tenuous::with_tenuous(TenuousRules::Held, rules)
            })
            .9,
            TenuousRules::Held,
            "every 24 hold but the tenuous air's"
        );
        assert_eq!(
            with_every_24_hold(|| with_unlit_mean(UnlitMeanRules::Held, rules)),
            (
                EscapeRules::Batch,
                RayleighFloorRules::Floored,
                UnlitMeanRules::Held,
                MoonBulkRules::Batch,
                ColdTrapRules::Batch,
                ReadDensitiesRules::Read,
                CompressedLiquidsRules::Compressed,
                RowDarkeningRules::Darkened,
                OceanFloorRules::Floored,
                TenuousRules::Tenuous,
                OceanRules::Solved,
            ),
            "every 24 hold but the unlit mean's"
        );
        assert_eq!(
            with_every_24_hold(|| with_read_densities(ReadDensitiesRules::Held, rules)),
            (
                EscapeRules::Batch,
                RayleighFloorRules::Floored,
                UnlitMeanRules::Floored,
                MoonBulkRules::Batch,
                ColdTrapRules::Batch,
                ReadDensitiesRules::Held,
                CompressedLiquidsRules::Compressed,
                RowDarkeningRules::Darkened,
                OceanFloorRules::Floored,
                TenuousRules::Tenuous,
                OceanRules::Solved,
            ),
            "every 24 hold but the read densities'"
        );
        assert_eq!(rules(), at_23, "set back after");
    }

    #[test]
    fn error_messages_are_lower_case_without_trailing_punctuation() {
        let messages = [SampleContextsError::TooFewSystems {
            wanted: 10,
            found: 3,
        }
        .to_string()];
        for message in messages {
            let first = message.chars().next().unwrap();
            assert!(!first.is_uppercase(), "{message}");
            assert!(!message.ends_with(['.', '!', '?']), "{message}");
        }
        assert!(format!("{:?}", SampleFilter::ALL).starts_with("SampleFilter"));
    }
}
