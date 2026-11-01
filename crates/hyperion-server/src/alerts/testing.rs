//! The alert scans' tests' fixtures (plan 12, P12.T8.b): seed 9's pinned supernova, the sensors
//! that see it, and a server whose state the walks read.
//!
//! The supernova is the one core collapse of the 288 cells about the solar circle that goes off in
//! the clock window's future, a Type IIP 874 years after the epoch in core-collapse cell
//! [-3, 48, 0]; `tests/alerts_scans.rs` watches it over a socket.

use std::num::NonZeroUsize;
use std::sync::{Arc, OnceLock};

use hyperion_sim::alerts::{
    AlertBand, BandLimits, ClassSet, OneShotClass, OneShotEvent, SensorHorizon, class_reach,
};
use hyperion_sim::coords::GalacticPosition;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::catalogue_classes::core_collapse::CoreCollapseProcess;
use hyperion_sim::galaxy::catalogue_classes::grid::CatalogueClassCell;
use hyperion_sim::galaxy::catalogue_classes::{CatalogueCellKey, ClassId};
use hyperion_sim::galaxy::gas::ccm::Band;
use hyperion_sim::observe::{Observer, light_time};
use hyperion_sim::time::{Span, UniverseTime};
use hyperion_sim::units::WattsPerSquareMetre;
use hyperion_sim::{GENERATOR_VERSION, Seed};
use tempfile::TempDir;

use super::{ScanRequest, Subscription};
use crate::compute::GalaxyKey;
use crate::{Server, ServerConfig, ServerConfigBuilder, SkyService};

/// The galaxy of the pinned supernova.
pub(crate) const SEED: u64 = 9;

/// The core-collapse cell that holds it.
pub(crate) const PINNED_CELL: [i32; 3] = [-3, 48, 0];

/// A day, s.
pub(crate) const DAY: i64 = 86_400;

/// The galaxy of [`SEED`], built once for the tests.
pub(crate) fn galaxy() -> &'static Arc<Galaxy> {
    static GALAXY: OnceLock<Arc<Galaxy>> = OnceLock::new();
    GALAXY.get_or_init(|| Arc::new(Galaxy::new(Seed::new(SEED))))
}

/// The galaxy's key.
pub(crate) const fn key() -> GalaxyKey {
    GalaxyKey::new(SEED, GENERATOR_VERSION)
}

/// The pinned supernova: the latest death of [`PINNED_CELL`] in the clock window's future.
pub(crate) fn pinned() -> OneShotEvent {
    let galaxy = galaxy();
    let process = CoreCollapseProcess::new(galaxy);
    let key = CatalogueCellKey::new(ClassId::CORE_COLLAPSE, PINNED_CELL).unwrap();
    CatalogueClassCell::generate(galaxy, &process, key)
        .entries()
        .iter()
        .filter_map(|entry| process.event(galaxy, entry.value()))
        .filter(|event| {
            let years = event.emitted().since_epoch().as_julian_years_f64();
            (0.0..990.0).contains(&years)
        })
        .max_by_key(OneShotEvent::emitted)
        .expect("seed 9's core collapse goes off 874 years after the epoch")
}

/// The place `away` light-years from `event`'s site, spinward along x.
pub(crate) fn beside(event: &OneShotEvent, away: f64) -> GalacticPosition {
    let [x, y, z] = event.site().to_light_years_f64();
    GalacticPosition::from_light_years([x + away, y, z]).unwrap()
}

/// When `event`'s light reaches `at`.
pub(crate) fn arrival(event: &OneShotEvent, at: &GalacticPosition) -> UniverseTime {
    event
        .emitted()
        .checked_add(light_time(event.site().distance_to(at)))
        .unwrap()
}

/// `t` and `seconds` later, or earlier for a negative count.
pub(crate) fn after(t: UniverseTime, seconds: i64) -> UniverseTime {
    if seconds < 0 {
        t.checked_sub(Span::from_seconds(-seconds)).unwrap()
    } else {
        t.checked_add(Span::from_seconds(seconds)).unwrap()
    }
}

/// A V sensor of these limits, W m⁻².
pub(crate) fn v_sensor(detect: f64, resolve: f64) -> SensorHorizon {
    let w = WattsPerSquareMetre::new;
    let limits = BandLimits::new(w(detect), w(resolve)).unwrap();
    SensorHorizon::new([(AlertBand::Photometric(Band::V), limits)]).unwrap()
}

/// The V sensor through which the core collapses reach `ly` light-years, resolving at ten times
/// its detection limit: a class's reach goes as its limit's inverse square root.
pub(crate) fn reaching(ly: f64) -> SensorHorizon {
    let probe = 1e-6;
    let reach = class_reach(ClassId::CORE_COLLAPSE, &v_sensor(probe, probe)).value();
    let ratio = reach / ly;
    let detect = probe * ratio * ratio;
    v_sensor(detect, detect * 10.0)
}

/// The subscription of `sensor` to the core collapses at `at` from `t`, and its first scan.
pub(crate) fn core_collapses(
    at: GalacticPosition,
    t: UniverseTime,
    sensor: SensorHorizon,
) -> (Subscription, ScanRequest) {
    Subscription::new(
        Observer::new(at, t).unwrap(),
        sensor,
        ClassSet::of(ClassId::CORE_COLLAPSE),
        hyperion_sim::alerts::DEFAULT_HOST_LIMIT,
    )
}

/// A server whose state the walks read, its configuration `configure` makes of one with two
/// workers, the sky off and the global list's warm-up off, over its own data directory, kept
/// with it.
pub(crate) async fn server(
    configure: impl FnOnce(ServerConfigBuilder) -> ServerConfigBuilder,
) -> (Server, TempDir) {
    let data_dir = tempfile::tempdir().unwrap();
    let config = configure(
        ServerConfig::builder()
            .data_dir(data_dir.path())
            .workers(NonZeroUsize::new(2).unwrap())
            .sky_service(SkyService::Unsupported)
            .global_list_warm_up(crate::GlobalListWarmUp::Off),
    )
    .build();
    (Server::start(config).await.unwrap(), data_dir)
}
