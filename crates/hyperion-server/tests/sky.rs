//! The sky over a real socket (rendering plan R06, R06.T11.a–d and T11.g): the census as bulk
//! jobs, which a range query overtakes within a bounded wait and a cancel stops, its answer
//! against the sim's own census, its stars and band in R03's bulk frames as the manifest states
//! them, the stars, texels and host discs against the sim's for the same query with the sky's cell
//! cache off, as it is by default since R06.T8.o, its census's cells served again from that cache
//! at R06.T8.n's sized budget, set explicitly, the galaxy's tables built once for every sky of it
//! and started when its universe is opened, the fields a request it cannot serve names, the switch
//! turned off (it is on by default since rendering plan R13's R13.T2), under which `sky` is
//! `unsupported`, and a sky arriving nearest first, each reply the census to its stated radii and
//! the last the census of its whole plan.
//!
//! Every sky's query states RM3's synthetic ceiling, V 5.0 (rendering plan R13, R13.T2.b), its cut
//! being deep enough here, so layers C, D and E list only within their caps towards each star's
//! band texel in every reply, the final one included, and the sim's census each test holds a reply
//! to is that query's, merged to its plan's completeness (`merge_shells`) as the server merges
//! every reply. A forced cap stands in for the real boundary, stating no count beyond it.
//!
//! The harness keeps the sky off. Every server here but those of the switch's own tests, which turn
//! it off themselves, turns it on ([`SkyService::Served`]) and forces its sky's caps to a small
//! radius ([`SkyCaps::forced`]), so that a census near the Sun takes seconds in a test build rather
//! than the thousands of CPU-seconds of one to the derived caps. Most of those seconds are the few
//! cells of layers D and E about the observer, whose every system the census generates there, so
//! a census of more cells runs longer mostly by its jobs' count. A served sky also marches its
//! band, 24,576 rays, some 27 CPU-s in a test build, so the tests of the census's stars, the cut
//! and the tables' build, which read no texel, have their server march a coarse band
//! ([`COARSE_BAND`], through the test seam [`SkyCaps::with_band_spec`]); the tests of the band's
//! texels, of the manifest and the cell cache, and of the jobs' scheduling keep the standard one.
//! Forced caps read tables that hold no star, but for the test of the band's light, which builds
//! the galaxy's own on the pool. Each wait is on what the server reports (its statistics, or the
//! frames it sends), bounded by the harness's patience. The sim's reference censuses of one query
//! census each of its cells once ([`CellParts`]), however many of them fold it.
//!
//! Every server here serves the feature members, so its sky marches every ray and star sightline
//! through plan 09's features' gas beside the smooth field (P09.T40.a), and every sim reference
//! marches its own through the same gas at the epoch ([`EpochGas`]), over the galaxy's index built
//! once for the binary ([`gas_index`]).

mod common;

use std::collections::BTreeMap;
use std::num::{NonZeroU32, NonZeroUsize};
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use common::{BinaryHeader, Frame, RawFrame, TestClient, TestServer};
use hyperion_protocol::{
    ConeDto, ErrorCode, EyeDto, GalacticPosition, HostDiscDto, MAX_SKY_STARS, MassLayer,
    OpenUniverseRequest, RequestBody, RequestError, RequestId, ResponseBody, SKY_STAR_BYTES,
    SKY_TEXEL_BYTES, ServerMessage, SkyGapDto, SkyRequest, SkyResponse, SystemIdHex,
    SystemsInRangeRequest, UniverseIdHex, UniverseTime,
};
use hyperion_server::compute::{BulkGate, SkyCaps, SkyCellCounters};
use hyperion_server::limits::MAX_BINARY_FRAME_BYTES;
use hyperion_server::{HeldSkies, KeptSkies, ServerConfigBuilder, ServerStats, SkyQos, SkyService};
use hyperion_sim::Seed;
use hyperion_sim::coords::{GalacticPosition as SimPosition, UnitVector};
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::catalogue_classes::{CatalogueClassSource, NoClassCellCache};
use hyperion_sim::galaxy::features::catalogue::NoFeatureCache;
use hyperion_sim::galaxy::features::centre::NoCentreOrbitCache;
use hyperion_sim::galaxy::features::gas_overlay::{FeatureGas, FeatureGasIndex};
use hyperion_sim::galaxy::features::members::KeepInteriors;
use hyperion_sim::galaxy::features::members::source::FeatureMemberSource;
use hyperion_sim::galaxy::gas::modifiers::{GasModifier, GasModifierSource, NoModifiers};
use hyperion_sim::galaxy::gas::noise::NoiseCache;
use hyperion_sim::galaxy::global_list::{
    GlobalList, GlobalListSource, StreamNumber, TubeLookup, TubeTable,
};
use hyperion_sim::galaxy::placement::{CellKey, SystemRecord, generate_cell};
use hyperion_sim::id::Layer;
use hyperion_sim::observe::Observer;
use hyperion_sim::sky::EyeObserver;
use hyperion_sim::sky::band::{
    BandMarch, BandSpec, BandTexel, CompleteTo, CubeFace, march_rows, sum_rows,
};
use hyperion_sim::sky::caps::{REAL_LIMIT_LY, SYNTHETIC_CEILING_V};
use hyperion_sim::sky::census::{
    CellOffsets, CensusPlan, CensusTallies, Completeness, MemberPart, NoSkyCellCache, Shell,
    SkyCensus, SkyContext, SkyQuery, SkyQueryBuilder, SkySource, SkyStar, census_cell,
    census_members, census_plan, census_plan_with_edges, merge_census, merge_shells,
};
use hyperion_sim::sky::dgl::Illumination;
use hyperion_sim::sky::disc::{HostDisc, host_discs};
use hyperion_sim::sky::envelope::BrightnessEnvelope;
use hyperion_sim::sky::eye::{SkyBackground, SpRatio, star_colour_offset};
use hyperion_sim::sky::limits::{Glare, eye_cut, eye_offsets, limit_map};
use hyperion_sim::sky::luminosity::LuminosityTables;
use hyperion_sim::stellar::system::SystemStars;
use hyperion_sim::tables::star_colour::LUMINANCE_RGB;
use hyperion_sim::units::consts::METRES_PER_LIGHT_YEAR;
use hyperion_sim::units::{CandelasPerSquareMetre, LightYears, Magnitudes, Years};
use tempfile::TempDir;

/// The seed of every universe these tests create.
const SEED: u64 = 0x4d2;

/// Near the Sun's place on the solar circle, ly, as the sim's sky tests stand.
const SUN_LY: [i32; 3] = [0, 26_000, 68];

/// The camera's limit the tests ask, V.
const CAMERA_LIMIT_V: f64 = 9.0;

/// A census that runs to its end within seconds: some 500 cells in 3 jobs near the Sun.
const SMALL_CAP_LY: f64 = 30.0;

/// A census of many jobs, which keeps one worker busy long enough to be overtaken and cancelled:
/// some 22,000 cells in 85 jobs near the Sun, about 35 s of one worker in a test build.
const LONG_CAP_LY: f64 = 120.0;

/// The galaxy of [`SEED`] as the server builds it, with its full potential, built once for the
/// binary.
fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| Galaxy::new(Seed::new(SEED)).with_full_potential())
}

/// A server that serves the sky, its caps forced to `radius_ly` over tables that hold no star,
/// with `workers` CPU workers, and its data directory, which the caller keeps until the server has
/// stopped.
async fn sky_server(radius_ly: f64, workers: usize) -> (TestServer, TempDir) {
    let caps = SkyCaps::forced(LightYears::new(radius_ly)).expect("a small forced cap");
    sky_server_with(caps, workers).await
}

/// The band a server marches for a test that never reads the band's texels: 16² texels a face, of
/// twelve nodes a decade, 1,536 rays against the standard band's 24,576 (Design note 14). Such a
/// test asserts what the census, the tables, the cut or the time give, never a texel, and asks the
/// sim's census of its query at this band, as the server's is (a star's texel can set its radius);
/// the standard band's texels are asserted, texel for texel, by the tests that keep it.
const COARSE_BAND: BandSpec =
    BandSpec::new(16, 12).expect("16² texels a face and 12 nodes a decade");

/// A server as [`sky_server`]'s, but marching the coarse band [`COARSE_BAND`], and its data
/// directory: for a test that never reads the band's texels.
async fn coarse_sky_server(radius_ly: f64, workers: usize) -> (TestServer, TempDir) {
    let caps = SkyCaps::forced(LightYears::new(radius_ly))
        .expect("a small forced cap")
        .with_band_spec(COARSE_BAND);
    sky_server_with(caps, workers).await
}

/// A server as [`coarse_sky_server`]'s that keeps no sky's final reply, so that a sky asked again
/// is censused again (R06.T11.l), and its data directory.
async fn unkept_coarse_sky_server(radius_ly: f64, workers: usize) -> (TestServer, TempDir) {
    let caps = SkyCaps::forced(LightYears::new(radius_ly))
        .expect("a small forced cap")
        .with_band_spec(COARSE_BAND);
    let data_dir = tempfile::tempdir().expect("a temporary directory");
    let config = sky_config(data_dir.path(), caps, workers)
        .kept_skies(KeptSkies::Off)
        .build();
    (TestServer::start_with(config).await, data_dir)
}

/// A server that serves the sky to `caps`, with `workers` CPU workers, and its data directory.
///
/// Its sky's cell cache has the default budget, 0, which turns it off (R06.T8.o).
async fn sky_server_with(caps: SkyCaps, workers: usize) -> (TestServer, TempDir) {
    let data_dir = tempfile::tempdir().expect("a temporary directory");
    let config = sky_config(data_dir.path(), caps, workers).build();
    (TestServer::start_with(config).await, data_dir)
}

/// R06.T8.n's sized budget of the sky's cell cache, 1,280 MiB, in bytes: one near-Sun sky's
/// entries and a 1,000 ly jump's. A test that reads the cache sets it, since the server's default
/// is 0, off (R06.T8.o; decided 2026-10-10, `decision-r06-t8n.md`).
const SIZED_SKY_CACHE_BYTES: usize = 1_280 << 20;

/// A server as [`sky_server`]'s, its caps forced to `radius_ly`, with the sky's cell cache at
/// [`SIZED_SKY_CACHE_BYTES`] rather than off, and its data directory. It keeps no sky's final
/// reply, so that a sky asked again reads the cells again (R06.T11.l).
async fn cached_sky_server(radius_ly: f64, workers: usize) -> (TestServer, TempDir) {
    let caps = SkyCaps::forced(LightYears::new(radius_ly)).expect("a small forced cap");
    let data_dir = tempfile::tempdir().expect("a temporary directory");
    let config = sky_config(data_dir.path(), caps, workers)
        .sky_cache_bytes(SIZED_SKY_CACHE_BYTES)
        .kept_skies(KeptSkies::Off)
        .build();
    (TestServer::start_with(config).await, data_dir)
}

/// The configuration of a server over `data_dir` that serves the sky to `caps`, with `workers` CPU
/// workers.
fn sky_config(data_dir: &Path, caps: SkyCaps, workers: usize) -> ServerConfigBuilder {
    TestServer::config(data_dir)
        .workers(NonZeroUsize::new(workers).expect("a worker or more"))
        .sky_caps(caps)
        .sky_service(SkyService::Served)
}

/// A client with a universe of [`SEED`] created and opened, so that its galaxy is warm, and where
/// the server serves the sky its galaxy's sky tables built, which the open starts on the pool
/// (R06.T11.g): a sky asked next finds the pool free for its jobs, and counts only its own.
async fn opened(server: &TestServer) -> (TestClient, UniverseIdHex) {
    let mut client = server.connected().await;
    let universe = client.create_universe("Sky", SEED).await.id;
    let body = RequestBody::OpenUniverse(OpenUniverseRequest {
        universe: universe.clone(),
    });
    match client.request(body).await {
        Ok(ResponseBody::OpenUniverse(_)) => {}
        other => panic!("expected the opened universe, got {other:?}"),
    }
    // An open that looked for the tables, with the sky served, started their build, unless they
    // were held; with the sky off it looks for none.
    if server.stats().sky_tables().cache().misses() > 0 {
        server
            .stats_until("the open's sky tables are built", |stats| {
                stats.sky_tables().builds() > 0
            })
            .await;
    }
    (client, universe)
}

/// A sky near the Sun at the epoch, to [`CAMERA_LIMIT_V`], with no eye.
fn sky(universe: &UniverseIdHex) -> SkyRequest {
    SkyRequest {
        universe: universe.clone(),
        observer: GalacticPosition {
            cell_ly: SUN_LY,
            offset_m: [0.0; 3],
        },
        time: UniverseTime::default(),
        eye: None,
        camera_limit_v: Some(CAMERA_LIMIT_V),
        n_max: None,
        cone: None,
        exclude_system: None,
    }
}

/// The default eye.
fn eye() -> EyeDto {
    EyeDto {
        field_factor: EyeObserver::DEFAULT_FIELD_FACTOR,
        age_years: EyeObserver::DEFAULT_AGE_YEARS,
        pigmentation: EyeObserver::DEFAULT_PIGMENTATION,
    }
}

/// A sky's answer as the server sent it: the request's ID, its bulk frames in the order they
/// arrived, and the `partial_response` or terminal response after them.
struct Sent {
    id: RequestId,
    frames: Vec<(BinaryHeader, Vec<u8>)>,
    response: SkyResponse,
}

impl Sent {
    /// The payload, the frames' payloads joined in the order they arrived.
    fn payload(&self) -> Vec<u8> {
        self.frames
            .iter()
            .flat_map(|(_, payload)| payload.iter().copied())
            .collect()
    }
}

/// The last answer to a sky that must be served: its chunks, then its response.
async fn served(client: &mut TestClient, request: SkyRequest) -> Sent {
    let id = client.send_request(RequestBody::Sky(request)).await;
    answer_to(client, id).await
}

/// The last answer to the sky `id`, which the client sent and must be served: its chunks, then its
/// response, after any answers before it.
async fn answer_to(client: &mut TestClient, id: RequestId) -> Sent {
    replies_to(client, id)
        .await
        .pop()
        .expect("a sky ends in its response")
}

/// Every answer to the sky `id`, which the client sent and must be served, in the order sent: each
/// `partial_response` with its chunks, not final, then the terminal response, final (R06.T11.d).
async fn replies_to(client: &mut TestClient, id: RequestId) -> Vec<Sent> {
    let mut replies = Vec::new();
    let mut frames = Vec::new();
    loop {
        match client.next_frame().await {
            Frame::Binary(header, payload) => frames.push((header, payload)),
            Frame::Message(ServerMessage::PartialResponse {
                id: answered,
                body: ResponseBody::Sky(response),
            }) if answered == id => {
                assert!(!response.is_final, "a partial response is not final");
                replies.push(Sent {
                    id,
                    frames: std::mem::take(&mut frames),
                    response: *response,
                });
            }
            Frame::Message(ServerMessage::Response {
                id: answered,
                body: ResponseBody::Sky(response),
            }) if answered == id => {
                assert!(response.is_final, "the terminal response is final");
                replies.push(Sent {
                    id,
                    frames,
                    response: *response,
                });
                return replies;
            }
            Frame::Message(other) => {
                panic!("expected the sky's chunks and then its responses, got {other:?}")
            }
        }
    }
}

/// The refusal of a sky that must be refused.
async fn refused(client: &mut TestClient, request: SkyRequest) -> RequestError {
    match client.request(RequestBody::Sky(request)).await {
        Err(error) => error,
        Ok(body) => panic!("expected a refusal, got {body:?}"),
    }
}

/// The observer near the Sun at the epoch, as the sim takes it.
fn observer() -> Observer {
    let at = SimPosition::from_light_years(SUN_LY.map(f64::from)).expect("in the cube");
    Observer::new(at, hyperion_sim::time::UniverseTime::EPOCH).expect("an observer")
}

/// What a sky's jobs read: the luminosity tables, the envelope and the cells' offset bounds.
type Tables = (LuminosityTables, BrightnessEnvelope, CellOffsets);

/// `tables` of [`galaxy`], with its envelope and offset bounds.
fn tables_of(tables: LuminosityTables) -> Tables {
    let galaxy = galaxy();
    (
        tables,
        BrightnessEnvelope::build(galaxy),
        CellOffsets::build(galaxy),
    )
}

/// The tables a forced census reads, as the server builds them: no star in any table.
fn dark_tables() -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(|| tables_of(LuminosityTables::dark(galaxy())))
}

/// The galaxy's features' gas index, as a sky's tables hold it where the feature members are
/// served (P09.T40.a), built once for the binary: some 7 s in a test build.
fn gas_index() -> &'static FeatureGasIndex {
    static INDEX: OnceLock<FeatureGasIndex> = OnceLock::new();
    INDEX.get_or_init(|| FeatureGasIndex::build(galaxy(), &NoFeatureCache))
}

/// Plan 09's features' gas at the epoch, looked up in [`gas_index`], as a sky at the epoch marches
/// it where the feature members are served (P09.T40.a): the gas of every sim reference here. Each
/// thread keeps its own `FeatureGas`, which the sim's own tests hold to the cells' lookup.
struct EpochGas;

impl GasModifierSource for EpochGas {
    fn modifiers_near_segment(&self, a: &SimPosition, b: &SimPosition, out: &mut Vec<GasModifier>) {
        thread_local! {
            static GAS: FeatureGas<'static> =
                FeatureGas::indexed(galaxy(), gas_index(), Years::ZERO);
        }
        GAS.with(|gas| gas.modifiers_near_segment(a, b, out));
    }
}

/// A context over [`dark_tables`].
fn context() -> SkyContext<'static> {
    context_over(dark_tables())
}

/// A context over `tables`, its sightlines and rays through [`EpochGas`].
fn context_over(tables: &Tables) -> SkyContext<'_> {
    context_through(tables, &EpochGas)
}

/// A context over `tables`, its sightlines and rays through `gas` beside the smooth field.
fn context_through<'a>(tables: &'a Tables, gas: &'a dyn GasModifierSource) -> SkyContext<'a> {
    let (tables, envelope, offsets) = tables;
    SkyContext {
        tables,
        envelope,
        offsets,
        noise: NoiseCache::with_capacity(1 << 16),
        cells: &NoSkyCellCache,
        sources: &[],
        modifiers: gas,
    }
}

/// The request's illumination over `tables`, as the server states it on every sky's query
/// (R06.T9.g).
fn illumination_over(tables: &Tables) -> Arc<Illumination> {
    Arc::new(Illumination::march(
        galaxy(),
        &mut context_over(tables),
        &observer(),
    ))
}

/// `builder`'s query at the synthetic ceiling the server states on every sky whose cut allows it
/// (rendering plan R13, R13.T2.b): every cut here does.
fn at_ceiling(builder: SkyQueryBuilder) -> SkyQuery {
    builder
        .synthetic_ceiling(Magnitudes::new(SYNTHETIC_CEILING_V))
        .build()
        .expect("a query at the ceiling")
}

/// The sim's own census of `query` in one pass over its plan's cells, merged to the plan's whole
/// completeness as the server's last reply is.
fn sim_census(query: &SkyQuery) -> SkyCensus {
    sim_census_over(dark_tables(), query).1
}

/// [`sim_census`] with `sources` beside the grid: each shell's members beyond the grid through them
/// in one part ([`census_members`]), and the cells in a context that carries them, as the server's
/// census takes the feature members', the catalogue classes' hosts and the global list's from
/// generator version 27 in its members' jobs (R06.T16.a's switch; which part holds which member
/// changes no star).
fn sim_census_with(query: &SkyQuery, sources: &[&dyn SkySource]) -> SkyCensus {
    let tables = dark_tables();
    let plan = plan_over(tables, query);
    let mut cells = CellParts::over(tables, query);
    cells.ctx.sources = sources;
    let mut parts: Vec<(Vec<SkyStar>, CensusTallies)> = plan
        .shells()
        .map(|shell| {
            let mut stars = Vec::new();
            let whole = (shell, MemberPart::WHOLE);
            let tallies = census_members(galaxy(), &mut cells.ctx, &plan, whole, query, &mut stars);
            (stars, tallies)
        })
        .collect();
    parts.extend(cells.fold(plan.cells()));
    merge_shells(parts, query.n_max(), plan.complete())
}

/// [`sim_census_with`] through the sources the server's census takes from generator version 27:
/// the feature members' (P09.T40.a), the catalogue classes' hosts' (P09.T37.a) and, where the
/// range query registers its source, the global list's (P10.T10.b).
fn sim_census_with_members(query: &SkyQuery) -> SkyCensus {
    let interiors = KeepInteriors::new(galaxy());
    let members = FeatureMemberSource::new(&NoFeatureCache, &interiors);
    let classes = CatalogueClassSource::new().with_cells(&NoClassCellCache);
    let tubes = Tubes::new();
    let list = GlobalListSource::is_registered_in(galaxy()).then(|| {
        GlobalListSource::new(galaxy(), &tubes, &NoCentreOrbitCache).expect("the centre builds")
    });
    let mut sources: Vec<&dyn SkySource> = vec![&members, &classes];
    sources.extend(list.iter().map(|list| list as &dyn SkySource));
    sim_census_with(query, &sources)
}

/// [`galaxy`]'s global list, its streams' tube tables built on first use and kept: the sim's side
/// of the server's `ListedTubes`, for the global list's source (P10.T10.b).
struct Tubes {
    list: GlobalList,
    kept: Mutex<BTreeMap<StreamNumber, Arc<TubeTable>>>,
}

impl Tubes {
    /// [`galaxy`]'s list, no table built yet.
    fn new() -> Self {
        Self {
            list: GlobalList::build(galaxy()),
            kept: Mutex::default(),
        }
    }
}

impl TubeLookup for Tubes {
    fn global_list(&self) -> &GlobalList {
        &self.list
    }

    fn tube(&self, stream: StreamNumber) -> Arc<TubeTable> {
        let mut kept = self.kept.lock().expect("no reference census panics");
        Arc::clone(kept.entry(stream).or_insert_with(|| {
            let spec = self.list.stream(stream).expect("a stream of the list");
            Arc::new(TubeTable::build(galaxy(), spec).expect("the galaxy's full potential"))
        }))
    }
}

/// The sim's own plan and census of `query` over `tables`, the census in one pass over the plan's
/// cells, merged as one part to the plan's whole completeness (`merge_shells`): the one-shot census,
/// but at a synthetic ceiling, where C, D and E list only within their caps towards each star's
/// texel (rendering plan R13, R13.T2.b), as the server's last reply does.
fn sim_census_over(tables: &Tables, query: &SkyQuery) -> (CensusPlan, SkyCensus) {
    let plan = plan_over(tables, query);
    let census = sim_census_of(&mut CellParts::over(tables, query), &plan);
    (plan, census)
}

/// The sim's plan of `query` over `tables`.
fn plan_over(tables: &Tables, query: &SkyQuery) -> CensusPlan {
    let mut noise = NoiseCache::with_capacity(1 << 16);
    census_plan(galaxy(), &tables.0, &tables.1, query, &mut noise)
}

/// [`sim_census_over`]'s census of `plan`, the plan of `cells`' query, from `cells`.
fn sim_census_of(cells: &mut CellParts<'_>, plan: &CensusPlan) -> SkyCensus {
    merge_shells(
        cells.fold(plan.cells()),
        cells.query.n_max(),
        plan.complete(),
    )
}

/// One query's census cells as the sim censuses them over its tables with no cell cache
/// ([`census_cell`]), each cell's stars and tallies censused once, the first time a census folds
/// it, and kept for every census of the query that folds it again.
///
/// A cell's part is a function of the galaxy, the tables, the cell and the query alone, the
/// context's noise cache being a cache that changes no value, and [`census_cell`] only appends to
/// the stars it is given: so a census folded from the kept parts, in its own order, is that census
/// run afresh, bit for bit. A test that holds several replies of one query, or merges one query's
/// census two ways, so censuses each cell once rather than once a reference (some 5 s a census
/// near the Sun).
struct CellParts<'a> {
    query: &'a SkyQuery,
    ctx: SkyContext<'a>,
    parts: BTreeMap<CellKey, (Vec<SkyStar>, CensusTallies)>,
}

impl<'a> CellParts<'a> {
    /// `query`'s cells, none censused yet, over [`dark_tables`].
    fn new(query: &'a SkyQuery) -> Self {
        Self::over(dark_tables(), query)
    }

    /// `query`'s cells, none censused yet, over `tables`.
    fn over(tables: &'a Tables, query: &'a SkyQuery) -> Self {
        Self {
            query,
            ctx: context_over(tables),
            parts: BTreeMap::new(),
        }
    }

    /// The census of the cells `keys`, in their order, as one serial pass over them makes it: each
    /// cell's stars appended and its tallies added, the first cell's starting the sum; `None` for
    /// no cell.
    fn fold(
        &mut self,
        keys: impl IntoIterator<Item = CellKey>,
    ) -> Option<(Vec<SkyStar>, CensusTallies)> {
        let mut stars = Vec::new();
        let mut tallies: Option<CensusTallies> = None;
        for key in keys {
            let (cell_stars, cell) = self.parts.entry(key).or_insert_with(|| {
                let mut cell_stars = Vec::new();
                let cell = census_cell(galaxy(), &mut self.ctx, key, self.query, &mut cell_stars);
                (cell_stars, cell)
            });
            stars.extend_from_slice(cell_stars);
            match &mut tallies {
                Some(sum) => sum.add(cell),
                None => tallies = Some(*cell),
            }
        }
        tallies.map(|tallies| (stars, tallies))
    }
}

/// The wire's name of each capped layer, in the census's order.
const CAPPED: [(Layer, MassLayer); 6] = [
    (Layer::A, MassLayer::A),
    (Layer::B, MassLayer::B),
    (Layer::C, MassLayer::C),
    (Layer::D, MassLayer::D),
    (Layer::E, MassLayer::E),
    (Layer::BrownDwarf, MassLayer::BrownDwarf),
];

/// A sky near the Sun takes the feature members: from generator version 27 the field gives them
/// up (P09.T2.c) and the census takes them (R06.T16.a's switch, `decision-r06-t16a-scope.md`), so
/// they are not listed as not modelled, as every sky's were below 27; and the census its jobs
/// merged is the sim's own, layer by layer. Its server marches the coarse band ([`COARSE_BAND`]), whose size and bytes the reply
/// states; the standard band's are asserted by the tests that keep it
/// (`decision-test-coverage.md`, N13). Asked again, warm, once the first sky has kept the envelopes
/// of the streams whose tables it read, it builds and reads only the tables of streams whose
/// envelopes it meets, and its reply and its stars are the cold one's (perf-range-cold-tables).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_sky_near_the_sun_takes_the_feature_members_from_27() {
    let (server, _data_dir) = unkept_coarse_sky_server(SMALL_CAP_LY, 2).await;
    let (mut client, universe) = opened(&server).await;
    let cold = served(&mut client, sky(&universe)).await;
    let (stars, response) = (cold.payload(), cold.response);

    assert!(
        !response.not_modelled.contains(&SkyGapDto::FeatureMembers),
        "{:?}",
        response.not_modelled
    );
    let without_photometry = response
        .census
        .iter()
        .any(|layer| layer.without_photometry > 0);
    assert_eq!(
        response.not_modelled.contains(&SkyGapDto::WhiteDwarfs),
        without_photometry,
        "white dwarfs are not modelled where a census held one: {:?}",
        response.not_modelled
    );

    // The census the jobs merged is the sim's one pass at the ceiling, layer by layer, of the
    // query at the band the server marched.
    let query = at_ceiling(
        SkyQuery::builder(observer(), Magnitudes::new(CAMERA_LIMIT_V)).band_spec(COARSE_BAND),
    )
    .with_caps_forced(LightYears::new(SMALL_CAP_LY))
    .expect("a forced cap");
    // From 27 the server's census takes the members beyond the grid in its members' jobs, as the
    // sim's does through the same sources.
    let sim = sim_census_with_members(&query);
    // The reply states the ceiling and the real limit (R13.T2.b); a forced cap, standing in for
    // the real boundary, states no count beyond it brighter than the ceiling.
    assert_eq!(
        (response.synthetic_ceiling_v, response.real_limit_ly),
        (Some(SYNTHETIC_CEILING_V), Some(REAL_LIMIT_LY))
    );
    assert!(
        response
            .census
            .iter()
            .all(|layer| layer.bright_beyond.is_none())
    );
    assert!(!sim.listed().is_empty(), "the Sun's neighbours are listed");
    assert_eq!(response.cut_v.to_bits(), CAMERA_LIMIT_V.to_bits());
    assert_eq!(
        (response.listed, response.overflow),
        (
            u32::try_from(sim.listed().len()).unwrap(),
            u32::try_from(sim.overflow().len()).unwrap()
        )
    );
    assert_eq!(response.census.len(), CAPPED.len());
    for (dto, (layer, wire)) in response.census.iter().zip(CAPPED) {
        let tally = sim.tallies().layer(layer);
        assert_eq!(dto.layer, wire);
        assert_eq!(
            (dto.cap_ly, dto.rule_bound_ly, dto.expected_beyond),
            (SMALL_CAP_LY, SMALL_CAP_LY, 0.0),
            "a forced cap states nothing beyond it"
        );
        assert_eq!(
            (
                u64::from(dto.cells),
                dto.candidates_opened,
                dto.accepted,
                u64::from(dto.listed),
                u64::from(dto.without_photometry)
            ),
            (
                tally.cells(),
                tally.generated(),
                tally.accepted(),
                tally.listed(),
                tally.without_photometry()
            ),
            "{layer:?}"
        );
        // From 27 the census takes the feature members, so no layer lacks them.
        assert!(!dto.feature_members_absent, "{layer:?}");
    }
    assert_eq!(
        response
            .census
            .iter()
            .map(|layer| layer.listed)
            .sum::<u32>(),
        response.listed
    );

    // The stars and the band in bulk (R06.T11.b and T11.c), the band the server marched, and no
    // disc with no system left out.
    assert_eq!(
        (response.stars_bytes, response.band_bytes),
        (
            u64::from(response.listed) * star_bytes(),
            band_bytes_of(COARSE_BAND)
        )
    );
    assert_eq!(response.band.face_texels, COARSE_BAND.face_texels());
    assert_eq!(response.hosts, []);
    assert_eq!(response.observer, sky(&universe).observer);
    // No listed star lies within a light-year, so the sky holds a Julian year.
    assert_eq!(
        (response.valid_until.seconds, response.valid_until.nanos),
        (31_557_600, 0)
    );

    warm_is_cold(&server, &mut client, sky(&universe), (&stars, &response)).await;

    client.close().await;
    server.stop().await;
}

/// Asks `request` again of `server`, warm, after a cold sky whose stars and reply were `cold`, and
/// checks that it reads fewer tables, builds none, and answers as the cold sky did: the streams
/// whose envelopes the cold sky kept and whose tubes pass far from this one are passed by, and
/// nothing moves (perf-range-cold-tables).
async fn warm_is_cold(
    server: &TestServer,
    client: &mut TestClient,
    request: SkyRequest,
    (stars, response): (&[u8], &SkyResponse),
) {
    let read = server.stats().tube_tables();
    let warm = served(client, request).await;
    let reread = server.stats().tube_tables();
    assert_eq!(&warm.response, response);
    assert_eq!(warm.payload(), stars, "the stars, bit for bit");
    assert_eq!(reread.misses(), read.misses(), "no table built again");
    assert!(
        reread.hits() - read.hits() < read.hits(),
        "the warm sky read {} tables, the cold one {}",
        reread.hits() - read.hits(),
        read.hits()
    );
}

/// A sky whose cut is at or brighter than RM3's ceiling and whose caps lie within the real limit
/// is served without a ceiling (rendering plan R13, R13.T2.b; `decision-r13-t2b-ceiling.md`): its
/// reply states no ceiling, no real limit and no count beyond brighter than one, lists the sim's
/// census of R06, the one-shot census to its caps, and sends the sim's band, texel for texel, both
/// marched through plan 09's features' gas (P09.T40.a). The gas moves the stars: the smooth field's
/// census is not theirs. The sim's references are made on a thread while the server serves.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_shallow_sky_within_the_limit_is_served_as_r06s() {
    // The ruling's cut of 4.6, and the edge of its rule, a cut at the ceiling.
    let shallows = [4.6, SYNTHETIC_CEILING_V];
    // The queries at the band the server marches.
    let queries = shallows.map(|shallow| {
        SkyQuery::builder(observer(), Magnitudes::new(shallow))
            .band_spec(COARSE_BAND)
            .build()
            .expect("a query")
            .with_caps_forced(LightYears::new(SMALL_CAP_LY))
            .expect("a forced cap")
    });
    let references = {
        let queries = queries.clone();
        std::thread::spawn(move || {
            queries.map(|query| {
                // Both merges of one plan's cells, each cell censused once, then the band, and the
                // census through the smooth field alone.
                let plan = plan_over(dark_tables(), &query);
                let mut cells = CellParts::new(&query);
                let r06 = sim_census_of(&mut cells, &plan);
                let one_shot = sim_census_one_shot(&mut cells, &plan);
                let band = sim_band_at(dark_tables(), &query, &plan, &one_shot, COARSE_BAND);
                let mut smooth = CellParts::new(&query);
                smooth.ctx.modifiers = &NoModifiers;
                let smooth = sim_census_one_shot(&mut smooth, &plan);
                (r06, one_shot, band, smooth)
            })
        })
    };
    let (server, _data_dir) = coarse_sky_server(SMALL_CAP_LY, 2).await;
    let (mut client, universe) = opened(&server).await;
    let mut sent = Vec::new();
    for shallow in shallows {
        let reply = served(
            &mut client,
            SkyRequest {
                camera_limit_v: Some(shallow),
                ..sky(&universe)
            },
        )
        .await;
        sent.push(reply);
    }
    let references = references.join().expect("the sim's references");
    for ((shallow, (r06, one_shot, band, smooth)), reply) in
        shallows.iter().zip(&references).zip(&sent)
    {
        let what = format!("a shallow sky at {shallow}");
        let response = &reply.response;
        assert_eq!(response.cut_v.to_bits(), shallow.to_bits());
        assert_eq!(
            (response.synthetic_ceiling_v, response.real_limit_ly),
            (None, None),
            "{what}"
        );
        assert!(
            response
                .census
                .iter()
                .all(|layer| layer.bright_beyond.is_none())
        );
        assert_eq!(listed_on_the_wire(r06), listed_on_the_wire(one_shot));
        assert_stars_are_the_sims(reply, r06, &what);
        let payload = reply.payload();
        let stars_bytes = usize::try_from(response.stars_bytes).unwrap();
        let (texels, rest) = payload[stars_bytes..].as_chunks::<SKY_TEXEL_BYTES>();
        assert_eq!(rest, [] as [u8; 0]);
        assert_eq!(texels.len(), band.len(), "{what}");
        for (index, (texel, bytes)) in band.iter().zip(texels).enumerate() {
            assert_eq!(
                bytes.as_slice(),
                texel_wire(texel).as_slice(),
                "{what}, texel {index}"
            );
        }
        if shallow.total_cmp(&SYNTHETIC_CEILING_V).is_eq() {
            assert!(
                !r06.listed().is_empty(),
                "the Sun's bright neighbours are listed"
            );
        }
        assert_ne!(
            listed_on_the_wire(r06),
            listed_on_the_wire(smooth),
            "{what}: the features' gas moves the stars"
        );
    }
    client.close().await;
    server.stop().await;
}

/// The sim's census of `cells`' query in one pass over the cells of its plan `plan`, merged by
/// R06's one-shot rule (`merge_census`), which lists every star of the cells it opens: a query that
/// states no synthetic ceiling, whose census `merge_shells` merges to the same.
fn sim_census_one_shot(cells: &mut CellParts<'_>, plan: &CensusPlan) -> SkyCensus {
    assert_eq!(cells.query.synthetic_ceiling(), None);
    merge_census(cells.fold(plan.cells()), cells.query.n_max())
}

/// The band at `spec` of `query`'s census `census` over `tables`, complete to the caps of its plan
/// `plan`, summed from the sim's one march of it ([`sim_marches_at`]).
fn sim_band_at(
    tables: &Tables,
    query: &SkyQuery,
    plan: &CensusPlan,
    census: &SkyCensus,
    spec: BandSpec,
) -> Vec<BandTexel> {
    let mut band = Vec::new();
    for march in &sim_marches_at(tables, query, plan, spec) {
        sum_rows(march, census, &CompleteTo::of_caps(plan.caps()), &mut band);
    }
    band
}

/// A shallow sky's band of the galaxy's own light marches plan 09's features' gas (P09.T40.a): over
/// the galaxy's tables, which forced caps otherwise leave dark, its coarse band is the sim's march
/// through the gas, texel for texel, its illumination marched through the gas too, and not the
/// smooth field's. The census overflows nothing, so the band takes no star of it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_shallow_skys_band_of_the_galaxys_light_marches_the_features_gas() {
    // The sim's tables and index, built serially while the server builds its own on the pool.
    let building = std::thread::spawn(|| {
        let tables = tables_of(LuminosityTables::build(galaxy()));
        gas_index();
        tables
    });
    let caps = SkyCaps::forced(LightYears::new(SMALL_CAP_LY))
        .expect("a small forced cap")
        .with_galaxy_tables()
        .with_band_spec(COARSE_BAND);
    let (server, _data_dir) = sky_server_with(caps, 3).await;
    let (mut client, universe) = opened(&server).await;
    let shallow = 4.6;
    let sent = served(
        &mut client,
        SkyRequest {
            camera_limit_v: Some(shallow),
            ..sky(&universe)
        },
    )
    .await;
    assert!(sent.response.is_final);
    let tables = building.join().expect("the sim's tables");
    // The request's illumination, as the server states it on every sky's query (R06.T9.g),
    // marched through the features' gas as its jobs march it, or through the smooth field.
    let query_lit_by = |light: Arc<Illumination>| {
        SkyQuery::builder(observer(), Magnitudes::new(shallow))
            .band_spec(COARSE_BAND)
            .illumination(light)
            .build()
            .expect("a query")
            .with_caps_forced(LightYears::new(SMALL_CAP_LY))
            .expect("a forced cap")
    };
    let query = query_lit_by(illumination_over(&tables));
    let plan = plan_over(&tables, &query);
    let census = sim_census_one_shot(&mut CellParts::over(&tables, &query), &plan);
    let band = sim_band_at(&tables, &query, &plan, &census, COARSE_BAND);
    assert_eq!(census.overflow().len(), 0);
    assert!(
        band.iter().all(|texel| texel.luminance().value() > 0.0),
        "the galaxy's light beyond the caps fills every texel"
    );
    let payload = sent.payload();
    let stars_bytes = usize::try_from(sent.response.stars_bytes).unwrap();
    let (texels, rest) = payload[stars_bytes..].as_chunks::<SKY_TEXEL_BYTES>();
    assert_eq!(rest, [] as [u8; 0]);
    assert_eq!(texels.len(), band.len());
    for (index, (texel, bytes)) in band.iter().zip(texels).enumerate() {
        assert_eq!(
            bytes.as_slice(),
            texel_wire(texel).as_slice(),
            "texel {index}"
        );
    }
    let smooth_query = query_lit_by(Arc::new(Illumination::march(
        galaxy(),
        &mut context_through(&tables, &NoModifiers),
        &observer(),
    )));
    let plan = plan_over(&tables, &smooth_query);
    let mut smooth = Vec::new();
    for march in &sim_marches_through(&tables, &smooth_query, &plan, COARSE_BAND, &NoModifiers) {
        sum_rows(
            march,
            &census,
            &CompleteTo::of_caps(plan.caps()),
            &mut smooth,
        );
    }
    let moved = band
        .iter()
        .zip(&smooth)
        .filter(|(gas, field)| texel_wire(gas) != texel_wire(field))
        .count();
    assert!(
        moved > band.len() / 2,
        "the features' gas moves {moved} texels"
    );
    client.close().await;
    server.stop().await;
}

/// The request's cut is the deeper of the eye's and the camera's: under a shallower camera, the
/// eye's cut, which the server sets from its pre-pass (R06.T9.d), here a dark sky's, since a
/// forced census reads tables that hold no star.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_cut_is_the_eyes_under_a_shallower_camera() {
    let (server, _data_dir) = coarse_sky_server(SMALL_CAP_LY, 2).await;
    let (mut client, universe) = opened(&server).await;
    let response = served(
        &mut client,
        SkyRequest {
            eye: Some(eye()),
            camera_limit_v: Some(7.0),
            ..sky(&universe)
        },
    )
    .await
    .response;
    let light = illumination_over(dark_tables());
    let expected = eye_cut(
        galaxy(),
        &mut context(),
        &observer(),
        &EyeObserver::default(),
        Some(&light),
    );
    assert_eq!(response.cut_v.to_bits(), expected.value().to_bits());
    assert!(
        (8.4..8.6).contains(&response.cut_v),
        "Crumey's darkest limit, 7.99, plus 0.553: {}",
        response.cut_v
    );
    client.close().await;
    server.stop().await;
}

/// The bytes of one listed star on the wire (Design note 17).
fn star_bytes() -> u64 {
    u64::try_from(SKY_STAR_BYTES).expect("24 bytes")
}

/// The texels of the server's band: six faces of 64² (Design note 14).
const BAND_TEXELS: usize = 6 * 64 * 64;

/// The bytes of the server's band on the wire: [`BAND_TEXELS`] of 12 bytes (Design note 17).
fn band_bytes() -> u64 {
    u64::try_from(BAND_TEXELS * SKY_TEXEL_BYTES).expect("288 KiB")
}

/// The bytes on the wire of a band of `spec`, which a test's server marches in place of the
/// standard one: six faces of its texels, each of 12 bytes (Design note 17).
fn band_bytes_of(spec: BandSpec) -> u64 {
    let side = u64::from(spec.face_texels());
    6 * side * side * u64::try_from(SKY_TEXEL_BYTES).expect("12 bytes")
}

/// A listed star's apparent V and distance as the wire carries them (Design note 17): bytes 16–17
/// as millimagnitudes and 12–15 as light-years.
fn wire_v_and_distance(star: &[u8; SKY_STAR_BYTES]) -> (i16, f32) {
    (
        i16::from_le_bytes([star[16], star[17]]),
        f32::from_le_bytes(star[12..16].try_into().expect("4 bytes")),
    )
}

/// Asserts that the stars `sent` lists are the sim's census `sim`'s listed stars, in its order, by
/// the V and distance the wire carries.
#[track_caller]
fn assert_stars_are_the_sims(sent: &Sent, sim: &SkyCensus, what: &str) {
    let payload = sent.payload();
    let stars_bytes = usize::try_from(sent.response.stars_bytes).unwrap();
    let (stars, rest) = payload[..stars_bytes].as_chunks::<SKY_STAR_BYTES>();
    assert!(rest.is_empty(), "{what}: whole stars");
    assert_eq!(sim.listed().len(), stars.len(), "{what}");
    for (index, (star, bytes)) in sim.listed().iter().zip(stars).enumerate() {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the wire's V and distance, as Design note 17 rounds them"
        )]
        let expected = (
            (star.v().value() * 1_000.0).round() as i16,
            star.distance().value() as f32,
        );
        assert_eq!(wire_v_and_distance(bytes), expected, "{what}: star {index}");
    }
}

/// A sky's manifest is what was sent: its chunks in order before the response, each a frame of at
/// most 256 KiB for the request, their bytes the manifest's, split into the listed stars and the
/// band, the stars the sim's census in its order. With the cell cache at R06.T8.n's sized budget,
/// set explicitly since the default is off (R06.T8.o), the same sky asked again is served from it,
/// with the same bytes (Design note 12), and a sky asked from a little way off rebuilds none of the
/// cells both censuses open, its stars the sim's census's (R06.T8.h).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_skys_manifest_matches_what_was_sent() {
    let (server, _data_dir) = cached_sky_server(SMALL_CAP_LY, 2).await;
    let (mut client, universe) = opened(&server).await;
    let sent = served(&mut client, sky(&universe)).await;
    let response = &sent.response;

    let manifest = &response.bulk;
    assert!(manifest.chunks > 0, "the Sun's neighbours are sent");
    assert_eq!(sent.frames.len(), usize::try_from(manifest.chunks).unwrap());
    for (index, (header, payload)) in sent.frames.iter().enumerate() {
        assert_eq!(
            (header.request, header.index, header.count),
            (sent.id.0, u32::try_from(index).unwrap(), manifest.chunks),
            "frame {index}"
        );
        assert!(
            payload.len() + 24 <= MAX_BINARY_FRAME_BYTES,
            "frame {index}: {}",
            payload.len()
        );
    }
    let payload = sent.payload();
    assert_eq!(u64::try_from(payload.len()).unwrap(), manifest.bytes);
    assert_eq!(response.stars_bytes + response.band_bytes, manifest.bytes);
    assert_eq!(
        (response.stars_bytes, response.band_bytes),
        (u64::from(response.listed) * star_bytes(), band_bytes()),
        "the stars, then the band"
    );

    // The stars are the sim's census's listed stars, in its order.
    let query = at_ceiling(SkyQuery::builder(
        observer(),
        Magnitudes::new(CAMERA_LIMIT_V),
    ))
    .with_caps_forced(LightYears::new(SMALL_CAP_LY))
    .expect("a forced cap");
    assert_stars_are_the_sims(&sent, &sim_census(&query), "near the Sun");

    // Asked again, every cell the census looked up is served from the cache, with the same bytes.
    let cold = server.stats().sky_cells();
    let lookups = cold.cache().hits() + cold.cache().misses();
    assert!(lookups > 0, "{cold:?}");
    let again = served(&mut client, sky(&universe)).await;
    let warm = server.stats().sky_cells();
    assert_eq!(again.payload(), payload);
    assert_eq!(
        again.response, *response,
        "computed again, the whole reply is the first's: what the kept skies rely on (R06.T11.l)"
    );
    assert_eq!(
        (
            warm.cache().hits() - cold.cache().hits(),
            warm.cache().misses() - cold.cache().misses(),
            warm.rebuilt(),
            warm.cache().evictions()
        ),
        (lookups, 0, 0, 0),
        "the second census's cells are the first's: {cold:?}, then {warm:?}"
    );
    assert_eq!(
        warm.served() - cold.served(),
        lookups,
        "{cold:?}, then {warm:?}"
    );

    // A sky 20 ly along x, inside the forced cap so that the two plans share most of their cells
    // (a jump of 1,000 ly would share none at this cap), reads the entries the first left: none
    // is rebuilt, the new cells merge into their blocks, and its stars are the sim's census's of
    // its own query, which reads no cache.
    let moved_ly = [SUN_LY[0] + 20, SUN_LY[1], SUN_LY[2]];
    let mut request = sky(&universe);
    request.observer.cell_ly = moved_ly;
    let moved = served(&mut client, request).await;
    let after = server.stats().sky_cells();
    assert_eq!(after.rebuilt(), 0, "{after:?}");
    assert!(
        after.served() > warm.served() && after.missed() > warm.missed(),
        "shared cells served and new ones built: {warm:?}, then {after:?}"
    );
    let at = SimPosition::from_light_years(moved_ly.map(f64::from)).expect("in the cube");
    let query = at_ceiling(SkyQuery::builder(
        Observer::new(at, hyperion_sim::time::UniverseTime::EPOCH).expect("an observer"),
        Magnitudes::new(CAMERA_LIMIT_V),
    ))
    .with_caps_forced(LightYears::new(SMALL_CAP_LY))
    .expect("a forced cap");
    assert!(moved.response.listed > 0, "the moved sky lists stars");
    assert_stars_are_the_sims(&moved, &sim_census(&query), "20 ly along x");

    client.close().await;
    server.stop().await;
}

/// A sky asked again exactly as one finished is answered with its kept final reply alone, the same
/// response and payload, and the pool runs only the job that serialises that reply's frame: no
/// census job and no other of the sky's (R06.T11.l). A sky one field away is computed afresh, and
/// kept beside it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_repeated_identical_sky_is_its_kept_reply_and_queues_no_census_job() {
    let (server, _data_dir) = coarse_sky_server(SMALL_CAP_LY, 2).await;
    let (mut client, universe) = opened(&server).await;
    let first = served(&mut client, sky(&universe)).await;
    let before = server.stats();
    assert_eq!(
        before.sky_replies().entries(),
        1,
        "{:?}",
        before.sky_replies()
    );

    let id = client.send_request(RequestBody::Sky(sky(&universe))).await;
    let again = replies_to(&mut client, id).await;
    let after = server.stats();
    assert_eq!(again.len(), 1, "the kept final reply alone");
    assert_eq!(again[0].response, first.response);
    assert_eq!(again[0].payload(), first.payload());
    let (pool, kept) = (after.pool(), after.sky_replies());
    assert_eq!(
        (
            pool.completed(),
            pool.queued_bulk(),
            pool.queued_background(),
            pool.running()
        ),
        (before.pool().completed() + 1, 0, 0, 0),
        "the reply's frame alone, serialised on the pool: {:?}, then {pool:?}",
        before.pool()
    );
    assert_eq!(
        (kept.hits(), kept.entries()),
        (before.sky_replies().hits() + 1, 1),
        "{kept:?}"
    );

    let fewer = served(
        &mut client,
        SkyRequest {
            n_max: Some(10),
            ..sky(&universe)
        },
    )
    .await;
    let computed = server.stats();
    assert!(
        computed.pool().completed() > after.pool().completed() + 1,
        "computed afresh: {pool:?}, then {:?}",
        computed.pool()
    );
    assert!(fewer.response.listed <= 10, "{}", fewer.response.listed);
    assert_eq!(computed.sky_replies().entries(), 2);
    client.close().await;
    server.stop().await;
}

/// A sky's final reply as the server sent it, raw: its body's JSON text, after the envelope's ID,
/// and its payload, with how many replies came, the partial ones included.
struct RawSky {
    body: String,
    payload: Vec<u8>,
    replies: usize,
}

/// The answer to the sky `id`, which the client sent and must be served, as [`RawSky`].
async fn raw_replies_to(client: &mut TestClient, id: RequestId) -> RawSky {
    let (mut payload, mut replies) = (Vec::new(), 0);
    loop {
        match client.next_raw_frame().await {
            RawFrame::Binary(header, bytes) => {
                assert_eq!(header.request, id.0, "{header:?}");
                payload.extend(bytes);
            }
            RawFrame::Text(text) => {
                match serde_json::from_str(&text).expect("the server sends valid messages") {
                    ServerMessage::PartialResponse { id: answered, .. } if answered == id => {
                        replies += 1;
                        payload.clear();
                    }
                    ServerMessage::Response { id: answered, .. } if answered == id => {
                        let envelope = format!(r#"{{"type":"response","id":{},"body":"#, id.0);
                        let body = text
                            .strip_prefix(&envelope)
                            .expect("the envelope's type and ID lead it")
                            .to_owned();
                        return RawSky {
                            body,
                            payload,
                            replies: replies + 1,
                        };
                    }
                    other => panic!("expected the sky's chunks and responses, got {other:?}"),
                }
            }
        }
    }
}

/// A client of `server` with the saved universe `universe` opened, and where the server serves the
/// sky its galaxy's sky tables built, as [`opened`] leaves a created one.
async fn reopened(server: &TestServer, universe: &UniverseIdHex) -> TestClient {
    let mut client = server.connected().await;
    let body = RequestBody::OpenUniverse(OpenUniverseRequest {
        universe: universe.clone(),
    });
    match client.request(body).await {
        Ok(ResponseBody::OpenUniverse(_)) => {}
        other => panic!("expected the opened universe, got {other:?}"),
    }
    if server.stats().sky_tables().cache().misses() > 0 {
        server
            .stats_until("the open's sky tables are built", |stats| {
                stats.sky_tables().builds() > 0
            })
            .await;
    }
    client
}

/// A sky asked again after a restart is answered from its file, kept under the data directory
/// (R06.T11.n): its JSON and payload byte for byte the first's, with no sky job on the pool.
#[tokio::test]
async fn a_sky_asked_again_after_a_restart_is_answered_from_its_file_byte_for_byte() {
    let caps = SkyCaps::forced(LightYears::new(SMALL_CAP_LY))
        .expect("a small forced cap")
        .with_band_spec(COARSE_BAND);
    let data_dir = tempfile::tempdir().expect("a temporary directory");
    let server = TestServer::start_with(sky_config(data_dir.path(), caps, 2).build()).await;
    let (mut client, universe) = opened(&server).await;
    let id = client.send_request(RequestBody::Sky(sky(&universe))).await;
    let first = raw_replies_to(&mut client, id).await;
    server
        .stats_until("the sky's final reply is written", |stats| {
            stats.sky_files().written() == 1
        })
        .await;
    client.close().await;
    server.stop().await;

    let restarted = TestServer::start_with(sky_config(data_dir.path(), caps, 2).build()).await;
    let mut client = reopened(&restarted, &universe).await;
    let before = restarted.stats();
    let id = client.send_request(RequestBody::Sky(sky(&universe))).await;
    let again = raw_replies_to(&mut client, id).await;
    let after = restarted.stats();
    assert_eq!(again.replies, 1, "the kept final reply alone");
    assert_eq!(again.body, first.body, "the JSON, byte for byte");
    assert_eq!(again.payload, first.payload, "the payload, byte for byte");
    assert!(
        again.payload.len() > 1_000,
        "the stars and the band: {}",
        again.payload.len()
    );
    let (pool, files) = (after.pool(), after.sky_files());
    assert_eq!(
        (pool.completed(), pool.queued_bulk(), pool.running()),
        (before.pool().completed() + 1, 0, 0),
        "the reply's frame alone, serialised on the pool: {:?}, then {pool:?}",
        before.pool()
    );
    assert_eq!(
        (files.hits(), files.entries(), after.sky_replies().entries()),
        (1, 1, 1),
        "read from its file and kept in memory: {files:?}"
    );

    client.close().await;
    restarted.stop().await;
}

/// How far a camera may move from where a sky was seen before its nearest listed star in `sent`
/// shifts a tenth of a pixel at 1080p across 60°, m: the bound by which the client holds a sky,
/// `parallaxShiftPx` (Design note 13), from the star's distance on the wire.
fn tenth_of_a_pixel_m(sent: &Sent) -> f64 {
    let stars = usize::try_from(sent.response.stars_bytes).expect("a payload's length");
    let nearest_ly = sent.payload()[..stars]
        .as_chunks::<SKY_STAR_BYTES>()
        .0
        .iter()
        .map(|star| wire_v_and_distance(star).1)
        .min_by(f32::total_cmp)
        .expect("a sky near the Sun lists stars");
    0.1 * (60.0_f64.to_radians() / 1_080.0) * f64::from(nearest_ly) * METRES_PER_LIGHT_YEAR
}

/// [`sky`] asked `seconds` after the epoch from `metres` along x of its observer.
fn sky_at(universe: &UniverseIdHex, seconds: i64, metres: f64) -> SkyRequest {
    SkyRequest {
        observer: GalacticPosition {
            cell_ly: SUN_LY,
            offset_m: [metres, 0.0, 0.0],
        },
        time: UniverseTime { seconds, nanos: 0 },
        ..sky(universe)
    }
}

/// A sky asked an hour after one kept, from a little way off but within a tenth of a pixel of its
/// nearest star's parallax, is answered with the kept final reply alone, its own `time`, `observer`
/// and `valid_until` unchanged, and the pool runs only the job that serialises its frame
/// (R06.T11.p). One moved past the bound is computed. The unit tests hold the times to [its time,
/// its `valid_until`): a sky a year on is in another time bucket, whose first sky costs some 40 s
/// in a test build.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_sky_asked_an_hour_later_is_its_kept_reply_and_one_past_its_bound_is_computed() {
    let (server, _data_dir) = coarse_sky_server(SMALL_CAP_LY, 2).await;
    let (mut client, universe) = opened(&server).await;
    let first = served(&mut client, sky(&universe)).await;
    let bound_m = tenth_of_a_pixel_m(&first);

    let before = server.stats();
    let hour_later = sky_at(&universe, 3_600, 0.5 * bound_m);
    let id = client.send_request(RequestBody::Sky(hour_later)).await;
    let again = replies_to(&mut client, id).await;
    let after = server.stats();
    assert_eq!(again.len(), 1, "the kept final reply alone");
    assert_eq!(
        again[0].response, first.response,
        "its own time and observer"
    );
    assert_eq!(again[0].payload(), first.payload());
    let pool = after.pool();
    assert_eq!(
        (pool.completed(), pool.queued_bulk(), pool.running()),
        (before.pool().completed() + 1, 0, 0),
        "the reply's frame alone, serialised on the pool: {:?}, then {pool:?}",
        before.pool()
    );
    assert_eq!(
        after.sky_replies().hits(),
        before.sky_replies().hits() + 1,
        "{:?}",
        after.sky_replies()
    );

    let past = sky_at(&universe, 3_600, 2.0 * bound_m);
    let sent = served(&mut client, past.clone()).await;
    assert_eq!(
        (sent.response.time, sent.response.observer),
        (past.time, past.observer),
        "computed for its own time and observer"
    );
    assert!(
        server.stats().pool().completed() > after.pool().completed() + 1,
        "computed afresh"
    );
    client.close().await;
    server.stop().await;
}

/// After a restart, a sky asked later than those kept, from a little way off, is answered from the
/// latest of their files that holds for it, its JSON's fields and payload the kept reply's, then
/// from memory, with no sky job but its frame's (R06.T11.p). With `--hold-kept-skies=false` a sky
/// asked half an hour later was computed.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn after_a_restart_a_later_sky_is_answered_from_the_latest_file_that_holds_for_it() {
    let caps = SkyCaps::forced(LightYears::new(SMALL_CAP_LY))
        .expect("a small forced cap")
        .with_band_spec(COARSE_BAND);
    let data_dir = tempfile::tempdir().expect("a temporary directory");
    let config = sky_config(data_dir.path(), caps, 2).held_skies(HeldSkies::Exact);
    let exact = TestServer::start_with(config.build()).await;
    let (mut client, universe) = opened(&exact).await;
    let first = served(&mut client, sky(&universe)).await;
    let before = exact.stats().pool().completed();
    let half_hour = served(&mut client, sky_at(&universe, 1_800, 0.0)).await;
    assert_eq!(
        half_hour.response.time.seconds, 1_800,
        "computed for its own time"
    );
    assert!(
        exact.stats().pool().completed() > before + 1,
        "computed afresh"
    );
    exact
        .stats_until("both skies' final replies are written", |stats| {
            stats.sky_files().written() == 2
        })
        .await;
    client.close().await;
    exact.stop().await;

    let server = TestServer::start_with(sky_config(data_dir.path(), caps, 2).build()).await;
    let mut client = reopened(&server, &universe).await;
    let bound_m = tenth_of_a_pixel_m(&first);
    let answered = [
        ("from its file", sky_at(&universe, 3_600, 0.5 * bound_m)),
        ("from memory", sky_at(&universe, 5_400, 0.25 * bound_m)),
    ];
    for (what, request) in answered {
        let before = server.stats();
        let id = client.send_request(RequestBody::Sky(request)).await;
        let again = replies_to(&mut client, id).await;
        let after = server.stats();
        assert_eq!(again.len(), 1, "{what}: the kept final reply alone");
        assert_eq!(
            again[0].response, half_hour.response,
            "{what}: the latest seen, its own time and observer"
        );
        assert_eq!(again[0].payload(), half_hour.payload(), "{what}");
        let pool = after.pool();
        assert_eq!(
            (pool.completed(), pool.queued_bulk(), pool.running()),
            (before.pool().completed() + 1, 0, 0),
            "{what}: the reply's frame alone, serialised on the pool: {:?}, then {pool:?}",
            before.pool()
        );
    }
    let (files, kept) = (server.stats().sky_files(), server.stats().sky_replies());
    assert_eq!((files.hits(), files.held()), (1, 1), "{files:?}");
    assert_eq!(kept.hits(), 1, "{kept:?}");
    client.close().await;
    server.stop().await;
}

/// A sky's jobs after its first reply are lowered to the background class where the server's
/// `--sky-qos` says so, and none otherwise (R06.T11.o): the pool counts them on every platform,
/// though only macOS's twins run them at quality-of-service class background.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_skys_jobs_after_its_first_reply_are_lowered_where_the_server_says() {
    /// Two shells a layer, so that the sky arrives in two steps or more, within 15 ly.
    const EDGES_LY: [u32; 1] = [10];
    let caps = SkyCaps::forced(LightYears::new(15.0))
        .expect("a small forced cap")
        .with_shell_edges(&EDGES_LY)
        .with_band_spec(COARSE_BAND);
    let mut counted = Vec::new();
    for qos in [SkyQos::Utility, SkyQos::Background] {
        let data_dir = tempfile::tempdir().expect("a temporary directory");
        let config = sky_config(data_dir.path(), caps, 2).sky_qos(qos).build();
        let server = TestServer::start_with(config).await;
        let (mut client, universe) = opened(&server).await;
        let before = server.stats().pool();
        let id = client.send_request(RequestBody::Sky(sky(&universe))).await;
        let replies = replies_to(&mut client, id).await;
        assert!(
            replies.len() > 1,
            "a sky of several steps: {}",
            replies.len()
        );
        let after = server.stats().pool();
        counted.push((
            after.lowered() - before.lowered(),
            after.completed() - before.completed(),
        ));
        client.close().await;
        server.stop().await;
    }
    let [(utility, _), (background, ran)] = counted[..] else {
        unreachable!("two servers")
    };
    assert_eq!(utility, 0, "{counted:?}");
    assert!(
        background > 0 && background < ran,
        "the jobs after the first reply alone: {counted:?}"
    );
}

/// Where the log test's subscriber writes: a buffer it shares.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Captured {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl tracing_subscriber::fmt::MakeWriter<'_> for Captured {
    type Writer = Self;

    fn make_writer(&self) -> Self {
        self.clone()
    }
}

impl Captured {
    /// A subscriber of `info` and above that writes here, for `tracing::subscriber::set_default`.
    fn subscriber(&self) -> impl tracing::Subscriber + Send + Sync + use<> {
        tracing_subscriber::fmt()
            .with_writer(self.clone())
            .with_max_level(tracing::Level::INFO)
            .with_ansi(false)
            .finish()
    }

    /// What was written, as text.
    fn text(&self) -> String {
        String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
    }
}

/// A sky logs at `info` that it started and that its final reply is made, with the wall time and
/// its jobs' time, and a sky asked again exactly that it is answered with its kept reply
/// (R06.T11.m). The runtime runs on the test's thread, so the subscriber set for it sees the
/// handler's lines.
#[tokio::test]
async fn a_sky_logs_its_start_and_its_final_reply_at_info() {
    let captured = Captured::default();
    let _default = tracing::subscriber::set_default(captured.subscriber());
    let (server, _data_dir) = coarse_sky_server(SMALL_CAP_LY, 2).await;
    let (mut client, universe) = opened(&server).await;
    let first = served(&mut client, sky(&universe)).await;
    let _ = served(&mut client, sky(&universe)).await;
    client.close().await;
    server.stop().await;

    let log = captured.text();
    let lines = |message: &str| -> Vec<&str> {
        log.lines().filter(|line| line.contains(message)).collect()
    };
    assert_eq!(lines("a sky started").len(), 2, "{log}");
    assert_eq!(
        lines("a sky is answered with its kept final reply").len(),
        1,
        "{log}"
    );
    let finals = lines("a sky's final reply is made");
    assert_eq!(finals.len(), 1, "{log}");
    let listed = format!("listed={}", first.response.listed);
    for field in [listed.as_str(), "wall_s=", "job_s=", "replies="] {
        assert!(finals[0].contains(field), "{field} in {}", finals[0]);
    }
}

/// A sky cancelled while its census runs logs at `info` that it was given up, with its wall time
/// and its jobs' seconds so far (R06.T11.m): the cancel aborts its task at an `.await`, so only the
/// request's drop sees it.
#[tokio::test]
async fn a_cancelled_sky_logs_at_info_that_it_was_given_up() {
    let captured = Captured::default();
    let _default = tracing::subscriber::set_default(captured.subscriber());
    let (server, _data_dir) = sky_server(LONG_CAP_LY, 1).await;
    let (mut client, universe) = opened(&server).await;
    let sky_id = client.send_request(RequestBody::Sky(sky(&universe))).await;
    census_under_way(&server).await;
    client.cancel(sky_id).await;
    expect_cancelled(&mut client, sky_id).await;
    server
        .stats_until("the cancelled sky has ended", |stats| {
            stats.requests().in_flight() == 0
        })
        .await;
    client.close().await;
    server.stop().await;

    let log = captured.text();
    let ended: Vec<&str> = log
        .lines()
        .filter(|line| line.contains("a sky was given up before its final reply"))
        .collect();
    assert_eq!(ended.len(), 1, "{log}");
    for field in ["wall_s=", "job_s="] {
        assert!(ended[0].contains(field), "{field} in {}", ended[0]);
    }
    assert!(!log.contains("a sky's final reply is made"), "{log}");
}

/// Waits until the one worker holds a census job and more wait in the bulk queue, and none in the
/// interactive queue: the sky's census is under way, and nothing else is.
async fn census_under_way(server: &TestServer) -> ServerStats {
    let stats = server
        .stats_until("the sky's census jobs are in the pool", |stats| {
            stats.pool().running() == 1 && stats.pool().queued_bulk() > 0
        })
        .await;
    assert_eq!(
        stats.pool().queued_interactive(),
        0,
        "the census never enters the interactive queue: {:?}",
        stats.pool()
    );
    stats
}

/// A range query near the Sun, which the tests send while a sky's census runs.
fn range_query(universe: &UniverseIdHex) -> RequestBody {
    RequestBody::SystemsInRange(SystemsInRangeRequest {
        universe: universe.clone(),
        centre: GalacticPosition {
            cell_ly: SUN_LY,
            offset_m: [0.0; 3],
        },
        radius_ly: 10.0,
        time: UniverseTime::default(),
        min_layer: MassLayer::A,
        limit: 100,
        include_stellar: false,
        mode: hyperion_protocol::QueryModeDto::Now,
    })
}

/// Reads the next message, which must be the range query `id`'s answer.
async fn expect_range_answer(client: &mut TestClient, id: RequestId) {
    match client.next_message().await {
        ServerMessage::Response {
            id: answered,
            body: ResponseBody::SystemsInRange(_),
        } => assert_eq!(answered, id),
        other => panic!("expected the range query's answer before the sky's, got {other:?}"),
    }
}

/// A range query sent while a sky's census runs is answered before the sky, and within a bounded
/// wait: the census is bulk work, the pool's workers take interactive work first, and a census job
/// runs for about 50 ms before it hands the rest of its cells back (R06.T11.d).
///
/// The wait is counted in jobs and held to an order, not to a margin of time. The server's one
/// worker holds each bulk job it takes at the test's gate ([`BulkGate`]), and the test lets one
/// through only while the query has a job queued: the job in hand when the query's job was queued.
/// The worker must then take the query's job, not the census's next, so from the query's sending
/// to its answer it completes the query's own jobs, as many as on an idle pool, and at most one
/// bulk job before each, however loaded the machine. A census to 120 ly is one step, so the sky
/// sends nothing until its whole census is done.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_range_query_sent_while_a_skys_jobs_run_is_answered_first() {
    let gate = BulkGate::default();
    let caps = SkyCaps::forced(LightYears::new(LONG_CAP_LY)).expect("a small forced cap");
    let data_dir = tempfile::tempdir().expect("a temporary directory");
    let config = sky_config(data_dir.path(), caps, 1).bulk_gate(gate.clone());
    let server = TestServer::start_with(config.build()).await;
    let (mut client, universe) = opened(&server).await;
    // The query's own jobs, on an idle pool: a second query's, since from generator version 27 the
    // first builds the galaxy's global list and its streams' tables for every later one, once
    // (P10.T10.b's registration).
    let warm = client.send_request(range_query(&universe)).await;
    expect_range_answer(&mut client, warm).await;
    let idle = server.stats().pool().completed();
    let alone = client.send_request(range_query(&universe)).await;
    expect_range_answer(&mut client, alone).await;
    let own = server.stats().pool().completed() - idle;
    assert!(own > 0, "the range query runs on the pool");

    let sky_id = client.send_request(RequestBody::Sky(sky(&universe))).await;
    census_under_way(&server).await;
    gate.close();
    let before = server
        .stats_until("the worker holds the sky's next job at the gate", |stats| {
            gate.held() == 1 && stats.pool().queued_interactive() == 0
        })
        .await
        .pool()
        .completed();
    let query_id = client.send_request(range_query(&universe)).await;
    let mut let_through = 0;
    let answer = loop {
        tokio::select! {
            message = client.next_message() => break message,
            _ = server.stats_until("the query has a job queued behind the held one", |stats| {
                gate.held() == 1 && stats.pool().queued_interactive() > 0
            }) => {
                gate.admit();
                let_through += 1;
            }
        }
    };
    match answer {
        ServerMessage::Response {
            id,
            body: ResponseBody::SystemsInRange(_),
        } => assert_eq!(id, query_id),
        other => panic!("expected the range query's answer before the sky's, got {other:?}"),
    }
    let pool = server.stats().pool();
    assert!(
        pool.running() + pool.queued_bulk() > 0,
        "the sky was still being censused when the query was answered: {pool:?}"
    );
    assert!(
        let_through <= own,
        "the worker took the query's job each time it let a bulk job go: {let_through} bulk jobs \
         let through for the query's {own}"
    );
    assert_eq!(
        pool.completed() - before,
        own + let_through,
        "only the query's jobs and the bulk jobs let through completed meanwhile: {pool:?}"
    );

    // The sky is given up rather than waited for, its jobs let go as the cancel finds them.
    gate.open();
    client.cancel(sky_id).await;
    expect_cancelled(&mut client, sky_id).await;
    client.close().await;
    server.stop().await;
}

/// Reads the next message, which must be request `id`'s `cancelled`.
async fn expect_cancelled(client: &mut TestClient, id: RequestId) {
    match client.next_message().await {
        ServerMessage::RequestError { id: ended, error } => {
            assert_eq!(ended, id);
            assert_eq!(error.code, ErrorCode::Cancelled);
        }
        other => panic!("expected the sky's `cancelled`, got {other:?}"),
    }
}

/// A cancelled sky's queued census jobs are skipped, the job in hand stops at its next cell, and
/// nothing more is sent for it.
///
/// Once `cancelled` is read the request's token is cancelled, so from then on a job a worker takes
/// is skipped, and at most the one job already in hand completes: the count of completed jobs grows
/// by one at most while the pool drains. Its jobs still queued are skipped, which the pool counts.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_cancelled_sky_stops_its_queued_jobs_and_sends_nothing_further() {
    let (server, _data_dir) = sky_server(LONG_CAP_LY, 1).await;
    let (mut client, universe) = opened(&server).await;
    let sky_id = client.send_request(RequestBody::Sky(sky(&universe))).await;
    let before = census_under_way(&server).await.pool();

    client.cancel(sky_id).await;
    expect_cancelled(&mut client, sky_id).await;
    let at_cancel = server.stats().pool();
    let idle = server
        .stats_until("the cancelled sky's jobs have left the pool", |stats| {
            stats.pool().running() == 0
                && stats.pool().queued_bulk() == 0
                && stats.requests().in_flight() == 0
        })
        .await;
    assert!(
        idle.pool().completed() <= at_cancel.completed() + 1,
        "no job started after the cancel: {at_cancel:?}, then {:?}",
        idle.pool()
    );
    assert!(
        idle.pool().cancelled() > before.cancelled(),
        "the sky's queued jobs were skipped: {before:?}, then {:?}",
        idle.pool()
    );
    assert_eq!(idle.requests().cancelled(), 1);

    // Nothing more comes for the sky: the next message answers this ping.
    match client.ping(9).await {
        ServerMessage::Pong { nonce } => assert_eq!(nonce, 9),
        other => panic!("expected the pong, got {other:?}"),
    }
    client.close().await;
    server.stop().await;
}

/// `n_max` above [`MAX_SKY_STARS`], or zero, is refused naming `n_max`.
#[tokio::test]
async fn n_max_above_the_cap_is_a_bad_request_naming_n_max() {
    let (server, _data_dir) = sky_server(SMALL_CAP_LY, 1).await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Sky", SEED).await.id;
    for n_max in [MAX_SKY_STARS + 1, 0] {
        let error = refused(
            &mut client,
            SkyRequest {
                n_max: Some(n_max),
                ..sky(&universe)
            },
        )
        .await;
        assert_eq!(
            (error.code, error.field.as_deref()),
            (ErrorCode::BadRequest, Some("n_max")),
            "{n_max}: {error:?}"
        );
    }
    client.close().await;
    server.stop().await;
}

/// A request with both the eye and a cone is refused naming `cone`: a cone is an instrument's
/// field stop, which the naked eye has none of (`decision-r06-t8k-cone.md`, item 2).
#[tokio::test]
async fn an_eye_with_a_cone_is_a_bad_request_naming_cone() {
    let (server, _data_dir) = sky_server(SMALL_CAP_LY, 1).await;
    let mut client = server.connected().await;
    let universe = client.create_universe("Sky", SEED).await.id;
    let error = refused(
        &mut client,
        SkyRequest {
            eye: Some(eye()),
            cone: Some(ConeDto {
                axis: [0.0, 0.0, 1.0],
                half_angle_deg: 2.5,
            }),
            ..sky(&universe)
        },
    )
    .await;
    assert_eq!(
        (error.code, error.field.as_deref()),
        (ErrorCode::BadRequest, Some("cone")),
        "{error:?}"
    );
    assert!(
        error.message.contains("field stop"),
        "the refusal says why: {}",
        error.message
    );
    client.close().await;
    server.stop().await;
}

/// A server with the sky's switch turned off, as `--serve-sky=false` turns it, and its data
/// directory, which the caller keeps until the server has stopped.
async fn sky_off_server() -> (TestServer, TempDir) {
    let data_dir = tempfile::tempdir().expect("a temporary directory");
    let config = TestServer::config(data_dir.path())
        .sky_service(SkyService::Unsupported)
        .build();
    (TestServer::start_with(config).await, data_dir)
}

/// With its switch turned off, the server answers `sky` as it did before R06.T11.a,
/// `unsupported`, and no job of it reaches the pool (R06.T11.c).
#[tokio::test]
async fn with_the_switch_off_a_sky_is_unsupported_and_no_job_reaches_the_pool() {
    let (server, _data_dir) = sky_off_server().await;
    let (mut client, universe) = opened(&server).await;
    let before = server.stats();
    let error = refused(&mut client, sky(&universe)).await;
    assert_eq!(error.code, ErrorCode::Unsupported, "{error:?}");
    assert_eq!(error.field, None);
    let after = server.stats();
    assert_eq!(after.pool(), before.pool(), "no job reached the pool");
    assert_eq!(after.sky_tables(), before.sky_tables());
    assert_eq!(after.sky_cells(), before.sky_cells());
    assert_eq!(after.sky_tables().builds(), 0);
    client.close().await;
    server.stop().await;
}

/// The near-Sun test's system to leave out: the first of the Sun's own cell in layer E with a star
/// to draw as a disc.
fn host_record() -> SystemRecord {
    let at = SimPosition::from_light_years(SUN_LY.map(f64::from)).expect("in the cube");
    let mut cell = Vec::new();
    generate_cell(
        galaxy(),
        CellKey::containing(Layer::E, &at).expect("in the cube"),
        &mut cell,
    );
    cell.into_iter()
        .find(|record| {
            let stars = SystemStars::generate(galaxy(), record);
            !host_discs(galaxy(), &stars, hyperion_sim::time::UniverseTime::EPOCH).is_empty()
        })
        .expect("a system of the Sun's cell with a star to draw as a disc")
}

/// `value` × `scale`, rounded to nearest and clamped to [`lo`, `hi`], NaN as 0: the wire's
/// quantisation (Design note 17).
fn quantised(value: f64, scale: f64, lo: f64, hi: f64) -> f64 {
    let scaled = (value * scale).round();
    if scaled.is_nan() {
        0.0
    } else {
        scaled.clamp(lo, hi)
    }
}

/// The wire's chroma of light whose linear Rec. 709 red and green at unit luminance are
/// `red_green`: its chromaticity, as `hyperion_protocol::sky` documents it.
fn chromaticity([r, g]: [f64; 2]) -> [f64; 2] {
    let [yr, yg, yb] = LUMINANCE_RGB;
    let b = (1.0 - yr * r - yg * g) / yb;
    let sum = r + g + b;
    if sum > 0.0 {
        [r / sum, g / sum]
    } else {
        [1.0 / 3.0; 2]
    }
}

/// A fraction of 65,535 as the wire's `u16`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "rounded and clamped to the u16's range first"
)]
fn fraction_bytes(fraction: f64) -> [u8; 2] {
    (quantised(fraction, 65_535.0, 0.0, 65_535.0) as u16).to_le_bytes()
}

/// The bytes the wire carries for the listed star `star`, seen from `observer`, with eye offset
/// `eye_offset`, by `hyperion_protocol::sky`'s table (Design note 17).
#[expect(
    clippy::cast_possible_truncation,
    reason = "the wire's f32 and its integers, each rounded and clamped to its range first"
)]
fn star_wire(observer: &Observer, star: &SkyStar, eye_offset: Magnitudes) -> Vec<u8> {
    let reddened = star.colour().reddened(star.a_v());
    let toward = observer
        .position()
        .displacement_to(star.apparent())
        .metres();
    let direction = UnitVector::from_components(toward).expect("a star away from the observer");
    let mut bytes = Vec::with_capacity(SKY_STAR_BYTES);
    for component in direction.components() {
        bytes.extend_from_slice(&(component as f32).to_le_bytes());
    }
    bytes.extend_from_slice(&(star.distance().value() as f32).to_le_bytes());
    let v = quantised(star.v().value(), 1_000.0, -32_768.0, 32_767.0) as i16;
    bytes.extend_from_slice(&v.to_le_bytes());
    for fraction in chromaticity(reddened.red_green()) {
        bytes.extend_from_slice(&fraction_bytes(fraction));
    }
    bytes.push((quantised(eye_offset.value(), 100.0, -128.0, 127.0) as i8).to_le_bytes()[0]);
    let camera = quantised(reddened.camera_band_mag(), 32.0, -128.0, 127.0) as i8;
    bytes.push(camera.to_le_bytes()[0]);
    bytes
}

/// The bytes the wire carries for the band texel `texel`, by `hyperion_protocol::sky`'s table
/// (Design note 17).
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the wire's f32 and its integers, each rounded and clamped to its range first"
)]
fn texel_wire(texel: &BandTexel) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(SKY_TEXEL_BYTES);
    bytes.extend_from_slice(&(texel.luminance().value() as f32).to_le_bytes());
    for fraction in chromaticity(texel.chroma().map(f64::from)) {
        bytes.extend_from_slice(&fraction_bytes(fraction));
    }
    let limit = texel.eye_limit().map_or(i16::MIN, |limit| {
        (quantised(limit.value(), 1_000.0, -32_768.0, 32_767.0) as i16).max(i16::MIN + 1)
    });
    bytes.extend_from_slice(&limit.to_le_bytes());
    let ratio = quantised(texel.sp_ratio(), 10_000.0, 0.0, 65_535.0) as u16;
    bytes.extend_from_slice(&ratio.to_le_bytes());
    bytes
}

/// The sim's march of the server's band for `query`, its six faces on threads of their own, for
/// every reply `plan`'s shells can state, as the server marches it once a request (R06.T9.f's
/// record asks this comparison to march the server's replies).
fn sim_marches(tables: &Tables, query: &SkyQuery, plan: &CensusPlan) -> Vec<BandMarch> {
    sim_marches_at(tables, query, plan, BandSpec::STANDARD)
}

/// [`sim_marches`] of the band at `spec`.
fn sim_marches_at(
    tables: &Tables,
    query: &SkyQuery,
    plan: &CensusPlan,
    spec: BandSpec,
) -> Vec<BandMarch> {
    sim_marches_through(tables, query, plan, spec, &EpochGas)
}

/// [`sim_marches_at`], its rays through `gas` beside the smooth field.
fn sim_marches_through(
    tables: &Tables,
    query: &SkyQuery,
    plan: &CensusPlan,
    spec: BandSpec,
    gas: &(dyn GasModifierSource + Sync),
) -> Vec<BandMarch> {
    std::thread::scope(|scope| {
        let marching: Vec<_> = CubeFace::ALL
            .chunks(2)
            .map(|faces| {
                scope.spawn(move || {
                    let mut ctx = context_through(tables, gas);
                    faces
                        .iter()
                        .map(|&face| {
                            march_rows(
                                galaxy(),
                                &mut ctx,
                                query,
                                plan.replies(),
                                &spec,
                                face,
                                0..spec.face_texels(),
                            )
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        marching
            .into_iter()
            .flat_map(|faces| faces.join().expect("a face's march"))
            .collect()
    })
}

/// The band of `query`'s census `census`, complete to `complete_to`, summed from `marches`
/// ([`sim_marches`]'), then where the eye is asked its limit map and eye offsets: the sim's own,
/// as R06.T9.f's `sum_rows`, T9.c's `limit_map` and T9.h's `eye_offsets` give them.
fn sim_band(
    marches: &[BandMarch],
    query: &SkyQuery,
    census: &SkyCensus,
    complete_to: &CompleteTo,
) -> (Vec<BandTexel>, Vec<Magnitudes>) {
    let spec = BandSpec::STANDARD;
    let mut band = Vec::with_capacity(BAND_TEXELS);
    for march in marches {
        sum_rows(march, census, complete_to, &mut band);
    }
    let (Some(&eye), Some(eye_cut)) = (query.eye(), query.eye_cut()) else {
        return (band, Vec::new());
    };
    let glare = Glare::of_listed(query.observer(), census.listed(), &spec, eye_cut);
    limit_map(&eye, &spec, &glare, &mut band);
    let offsets = eye_offsets(&eye, &spec, &glare, &band);
    (band, offsets)
}

/// Asserts that `response` is a sky's final reply, every layer complete to its cap of `cap_ly` in
/// every direction (R06.T11.d).
fn assert_final_to(response: &SkyResponse, cap_ly: f64) {
    assert!(response.is_final);
    for layer in &response.census {
        assert_eq!(
            (
                layer.complete_to_ly,
                layer.complete_to_rays_ly.len(),
                layer.is_final
            ),
            (cap_ly, 0, true),
            "{:?}",
            layer.layer
        );
    }
}

/// Asserts that `sent`, a payload's band bytes, are `band`'s texels as the wire carries them, of
/// the reply `what`.
fn assert_band_sent(band: &[BandTexel], sent: &[u8], what: &str) {
    assert_eq!(band.len(), BAND_TEXELS);
    let (texels, rest) = sent.as_chunks::<SKY_TEXEL_BYTES>();
    assert_eq!(rest, [] as [u8; 0]);
    assert_eq!(texels.len(), BAND_TEXELS);
    for (index, (texel, bytes)) in band.iter().zip(texels).enumerate() {
        assert_eq!(
            bytes.as_slice(),
            texel_wire(texel).as_slice(),
            "{what}, texel {index}"
        );
    }
}

/// The host disc `disc` as the reply should carry it.
fn sim_host(disc: &HostDisc) -> HostDiscDto {
    let colour = disc.colour();
    HostDiscDto {
        star: disc.star().get(),
        radius_m: disc.radius().value(),
        teff_k: disc.teff().value(),
        log_g: disc.log_g(),
        mean_luminance_cd_m2: disc.mean_luminance().map(CandelasPerSquareMetre::value),
        central_luminance_cd_m2: disc.central_luminance().map(CandelasPerSquareMetre::value),
        limb: disc.limb().map(|law| hyperion_protocol::PowerTwoDto {
            c: law.c(),
            alpha: law.alpha(),
        }),
        chroma: colour.chroma(),
        lux_per_v0: colour.lux_per_v0(),
        bake_spectrum: colour.bake_spectrum(),
    }
}

/// A sky near the Sun returns the stars, texels and host discs the sim returns for the same query,
/// byte for byte as the wire carries them (R06.T11.c), at RM3's synthetic ceiling (R13.T2.b): the
/// sim's census at the ceiling, its C, D and E listed within their caps towards each star's texel
/// in the final reply too, and the band holding the light beyond.
///
/// The server builds the galaxy's own tables on its pool, staged from `LuminosityTables::plan`,
/// while the test builds them serially, so the band's light is the tables' and the caps are forced.
/// The request asks the eye and a camera's deeper cut, so the band keeps the eye's light beside its
/// own (R06.T9.j); a small `n_max`, so that the overflow's stars are splatted into the band; and a
/// system of the Sun's cell left out, whose stars come back as discs at the request's time.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_sky_near_the_sun_returns_the_stars_texels_and_host_discs_the_sim_returns() {
    // The sim's tables, built serially while the server builds its own on the pool.
    let building = std::thread::spawn(|| tables_of(LuminosityTables::build(galaxy())));
    let caps = SkyCaps::forced(LightYears::new(SMALL_CAP_LY))
        .expect("a small forced cap")
        .with_galaxy_tables();
    let (server, _data_dir) = sky_server_with(caps, 3).await;
    let (mut client, universe) = opened(&server).await;
    let host = host_record();
    let n_max = 16;
    let sent = served(
        &mut client,
        SkyRequest {
            eye: Some(eye()),
            n_max: Some(n_max),
            exclude_system: Some(SystemIdHex::from_u64(host.id().raw())),
            ..sky(&universe)
        },
    )
    .await;
    let response = &sent.response;
    let tables = building.join().expect("the sim's tables");
    assert_eq!(server.stats().sky_tables().builds(), 1);
    // At the default budget, 0, the sky's cell cache is off (R06.T8.o): the census read none, as
    // the sim's below reads none, and the server holds no cell of it.
    assert_eq!(server.stats().sky_cells(), SkyCellCounters::default());

    // The same query in the sim: the illumination, the eye's cut, then the cut, the census, the
    // band and the discs.
    let observer = observer();
    let eye_observer = EyeObserver::default();
    let light = illumination_over(&tables);
    let eye_cut = eye_cut(
        galaxy(),
        &mut context_over(&tables),
        &observer,
        &eye_observer,
        Some(&light),
    );
    let cut = eye_cut.value().max(CAMERA_LIMIT_V);
    assert_eq!(response.cut_v.to_bits(), cut.to_bits());
    let query = at_ceiling(
        SkyQuery::builder(observer, Magnitudes::new(cut))
            .eye(eye_observer)
            .eye_cut(eye_cut)
            .n_max(std::num::NonZeroU32::new(n_max).expect("not zero"))
            .exclude(host.id())
            .illumination(light),
    )
    .with_caps_forced(LightYears::new(SMALL_CAP_LY))
    .expect("a forced cap");
    let (plan, census) = sim_census_over(&tables, &query);
    assert_eq!(census.listed().len(), usize::try_from(n_max).unwrap());
    assert!(
        !census.overflow().is_empty(),
        "the census overflows into the band"
    );
    assert_eq!(
        (response.listed, response.overflow),
        (
            u32::try_from(census.listed().len()).unwrap(),
            u32::try_from(census.overflow().len()).unwrap()
        )
    );
    // To 30 ly every layer is one shell, so the sky is one reply, final, complete to its caps.
    assert_final_to(response, SMALL_CAP_LY);
    let marches = sim_marches(&tables, &query, &plan);
    let (band, offsets) = sim_band(&marches, &query, &census, &CompleteTo::of_caps(plan.caps()));
    assert!(
        band.iter().all(|texel| texel.luminance().value() > 0.0),
        "the galaxy's light beyond the caps fills every texel"
    );

    // The stars, then the texels, byte for byte, of the standard band.
    let payload = sent.payload();
    assert_eq!(
        (response.stars_bytes, response.band_bytes),
        (u64::from(response.listed) * star_bytes(), band_bytes())
    );
    assert_eq!(response.band.face_texels, 64);
    let (stars, texels) = payload.split_at(usize::try_from(response.stars_bytes).unwrap());
    for (index, ((star, offset), bytes)) in census
        .listed()
        .iter()
        .zip(&offsets)
        .zip(stars.as_chunks::<SKY_STAR_BYTES>().0)
        .enumerate()
    {
        let expected = star_wire(&observer, star, *offset);
        assert_eq!(bytes.as_slice(), expected.as_slice(), "star {index}");
    }
    assert!(band.iter().all(|texel| texel.eye_limit().is_some()));
    assert_band_sent(&band, texels, "the sky");

    // The host's discs at the request's time.
    let stars = SystemStars::generate(galaxy(), &host);
    let discs = host_discs(galaxy(), &stars, observer.time());
    assert_ne!(discs, []);
    assert_eq!(
        response.hosts,
        discs.iter().map(sim_host).collect::<Vec<_>>()
    );

    client.close().await;
    server.stop().await;
}

/// Two identical skies asked at once share one build of the galaxy's tables, and send the same
/// bytes and the same reply (R06.T11.c).
///
/// Their tables hold no star, a build of moments, so the second may find them held rather than
/// join the first's build: `compute::sky_tables`' unit test pins the single flight itself. The
/// server keeps no sky's reply, so that the second is computed whenever it comes (R06.T11.l).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_second_identical_sky_shares_the_tables_build() {
    let caps = SkyCaps::forced(LightYears::new(SMALL_CAP_LY))
        .expect("a small forced cap")
        .with_band_spec(COARSE_BAND);
    let data_dir = tempfile::tempdir().expect("a temporary directory");
    let config = sky_config(data_dir.path(), caps, 3)
        .kept_skies(KeptSkies::Off)
        .build();
    let (server, _data_dir) = (TestServer::start_with(config).await, data_dir);
    let (mut first, universe) = opened(&server).await;
    let mut second = server.connected().await;
    let one = first.send_request(RequestBody::Sky(sky(&universe))).await;
    let other = second.send_request(RequestBody::Sky(sky(&universe))).await;
    let (one, other) = (
        answer_to(&mut first, one).await,
        answer_to(&mut second, other).await,
    );
    assert_eq!(one.payload(), other.payload());
    assert_eq!(one.response, other.response);
    let tables = server.stats().sky_tables();
    assert_eq!(tables.builds(), 1, "{tables:?}");
    assert_eq!(tables.cache().entries(), 1, "{tables:?}");
    first.close().await;
    second.close().await;
    server.stop().await;
}

/// With the sky served, opening a universe starts its galaxy's tables, and the sky asked next finds
/// them held: one build between them (R06.T11.g; decided 2026-10-08,
/// `decision-r06-t11d-first-sky.md` §2). Opening the universe again starts nothing.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn opening_a_universe_starts_its_sky_tables_which_a_sky_shares() {
    let (server, _data_dir) = coarse_sky_server(SMALL_CAP_LY, 3).await;
    let (mut client, universe) = opened(&server).await;
    let built = server
        .stats_until("the open's sky tables are held", |stats| {
            stats.sky_tables().cache().entries() == 1
        })
        .await
        .sky_tables();
    assert_eq!(built.builds(), 1, "{built:?}");
    let _ = served(&mut client, sky(&universe)).await;
    let asked = server.stats().sky_tables();
    assert_eq!(asked.builds(), 1, "{asked:?}");
    assert_eq!(
        asked.cache().hits(),
        built.cache().hits() + 1,
        "the sky found the open's tables: {built:?}, then {asked:?}"
    );
    let body = RequestBody::OpenUniverse(OpenUniverseRequest {
        universe: universe.clone(),
    });
    match client.request(body).await {
        Ok(ResponseBody::OpenUniverse(_)) => {}
        other => panic!("expected the opened universe, got {other:?}"),
    }
    let reopened = server.stats().sky_tables();
    assert_eq!(
        (reopened.builds(), reopened.cache().hits()),
        (1, asked.cache().hits() + 1),
        "held: the open starts nothing: {reopened:?}"
    );
    client.close().await;
    server.stop().await;
}

/// With the sky's switch turned off, opening a universe starts no sky tables and looks for none
/// (R06.T11.g).
#[tokio::test]
async fn with_the_switch_off_an_open_starts_no_sky_tables() {
    let (server, _data_dir) = sky_off_server().await;
    let (client, _universe) = opened(&server).await;
    let tables = server.stats().sky_tables();
    assert_eq!(
        (
            tables.builds(),
            tables.cache().misses(),
            tables.cache().hits()
        ),
        (0, 0, 0),
        "{tables:?}"
    );
    client.close().await;
    server.stop().await;
}

/// A sky in another time bucket of DN13's 1,000 years shares the build of the first: one table per
/// galaxy at the reference time serves the whole clock window (decided 2026-10-03,
/// `decision-r06-tables.md`, item B.2; R06.T11.c).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_second_sky_in_another_time_bucket_shares_the_build() {
    let (server, _data_dir) = coarse_sky_server(SMALL_CAP_LY, 3).await;
    let (mut client, universe) = opened(&server).await;
    let now = served(&mut client, sky(&universe)).await.response;
    let built = server.stats().sky_tables();
    assert_eq!(built.builds(), 1, "{built:?}");
    // 900 Julian years before the epoch, within the clock window's ±1,000 years.
    let earlier = UniverseTime {
        seconds: -900 * 31_557_600,
        nanos: 0,
    };
    let then = served(
        &mut client,
        SkyRequest {
            time: earlier,
            ..sky(&universe)
        },
    )
    .await
    .response;
    assert_eq!(then.time, earlier);
    assert_ne!(then.valid_until, now.valid_until);
    let shared = server.stats().sky_tables();
    assert_eq!(shared.builds(), 1, "{shared:?}");
    assert_eq!(
        shared.cache().hits(),
        built.cache().hits() + 1,
        "the second sky found the first's tables: {built:?}, then {shared:?}"
    );
    client.close().await;
    server.stop().await;
}

/// The shell edges of the nearest-first test, ly: four below its caps of [`SMALL_CAP_LY`], so that
/// C, D and E are censused in five shells each.
const TEST_SHELL_EDGES_LY: [u32; 4] = [10, 15, 20, 25];

/// The edge, ly, from which the nearest-first test's C shells come a step late, as C's beyond
/// 2,000 ly do near the Sun: its fourth and fifth, from 20 and 25 ly.
const TEST_C_DEFERRED_BEYOND_LY: NonZeroU32 = NonZeroU32::new(20).unwrap();

/// The edge, ly, from which the nearest-first test's E shells come a step ahead of C's and D's of
/// the same edge, as E's beyond 1,000 ly do near the Sun (R06.T11.h): its third to fifth, from 15,
/// 20 and 25 ly.
const TEST_FAR_FIRST_FROM_LY: NonZeroU32 = NonZeroU32::new(15).unwrap();

/// The steps a sky of `plan` arrives in (R06.T11.d): each layer's shells rank by rank, nearest
/// first, but C's and D's from [`TEST_FAR_FIRST_FROM_LY`] on, by their inner edge, a step late
/// (R06.T11.h), and C's from [`TEST_C_DEFERRED_BEYOND_LY`] on a step later again (R06.T11.g).
fn delivery(plan: &CensusPlan) -> Vec<Vec<Shell>> {
    let mut steps: Vec<Vec<Shell>> = Vec::new();
    // Each layer's edge before the shell at hand, ly.
    let mut inner_ly: BTreeMap<Layer, f64> = BTreeMap::new();
    for shell in plan.shells() {
        let layer = shell.layer();
        let from = inner_ly.get(&layer).copied().unwrap_or(0.0);
        let behind =
            matches!(layer, Layer::C | Layer::D) && from >= f64::from(TEST_FAR_FIRST_FROM_LY.get());
        let late = layer == Layer::C && from >= f64::from(TEST_C_DEFERRED_BEYOND_LY.get());
        if let Some(edge) = shell.edge() {
            inner_ly.insert(layer, edge.value());
        }
        let step = usize::from(shell.index()) + usize::from(behind) + usize::from(late);
        if steps.len() <= step {
            steps.resize_with(step + 1, Vec::new);
        }
        steps[step].push(shell);
    }
    steps.retain(|step| !step.is_empty());
    steps
}

/// The sim's census of `plan`'s shells `done`, serially in their order over [`dark_tables`], from
/// `cells`, the cells of `plan`'s query, merged to their completeness (R06.T8.i's `merge_shells`).
fn sim_shells_census(cells: &mut CellParts<'_>, plan: &CensusPlan, done: &[Shell]) -> SkyCensus {
    let keys = done
        .iter()
        .flat_map(|&shell| plan.shell_slabs(shell).flat_map(|slab| slab.cells()));
    merge_shells(
        cells.fold(keys),
        cells.query.n_max(),
        plan.completeness(done.iter().copied()),
    )
}

/// Each listed star's bytes on the wire, in the census's order, of a sky that asks no eye: its eye
/// offset its colour offset alone against a scotopic background (Design note 17; R06.T11.a).
fn listed_on_the_wire(census: &SkyCensus) -> Vec<Vec<u8>> {
    let dark = SkyBackground::new(CandelasPerSquareMetre::ZERO, SpRatio::REFERENCE)
        .expect("no light is a background");
    census
        .listed()
        .iter()
        .map(|star| {
            let ratio = star.colour().reddened(star.a_v()).sp_ratio();
            let ratio = SpRatio::new(ratio.clamp(SpRatio::MIN.value(), SpRatio::MAX.value()))
                .expect("a ratio held within the accepted range");
            star_wire(&observer(), star, star_colour_offset(ratio, &dark))
        })
        .collect()
}

/// Asserts that `reply`, the `k`th of request `id`, is a whole sky's frames, its chunks numbered
/// from 0 as its manifest counts them, and returns its payload.
fn whole_payload(reply: &Sent, id: RequestId, k: usize) -> Vec<u8> {
    let manifest = &reply.response.bulk;
    assert_eq!(
        reply.frames.len(),
        usize::try_from(manifest.chunks).unwrap()
    );
    for (index, (header, _)) in reply.frames.iter().enumerate() {
        assert_eq!(
            (header.request, header.index, header.count),
            (id.0, u32::try_from(index).unwrap(), manifest.chunks),
            "reply {k}, frame {index}"
        );
    }
    let payload = reply.payload();
    assert_eq!(u64::try_from(payload.len()).unwrap(), manifest.bytes);
    payload
}

/// Each layer's radius and finality as `response`, the `k`th reply, states them, asserted against
/// `completeness`: one radius in every direction, the caps being forced, of `cap_ly` once final.
fn stated_radii(
    response: &SkyResponse,
    completeness: &Completeness,
    cap_ly: f64,
    k: usize,
) -> Vec<(Layer, LightYears, bool)> {
    response
        .census
        .iter()
        .zip(CAPPED)
        .map(|(dto, (layer, wire))| {
            let edge = completeness.edge(layer);
            assert_eq!(dto.layer, wire);
            assert_eq!(
                (dto.complete_to_ly, dto.is_final),
                (edge.map_or(cap_ly, LightYears::value), edge.is_none()),
                "reply {k}, {layer:?}"
            );
            assert_eq!(dto.complete_to_rays_ly, [] as [f64; 0]);
            (layer, LightYears::new(dto.complete_to_ly), dto.is_final)
        })
        .collect()
}

/// The census near the Sun at [`CAMERA_LIMIT_V`] with no ceiling and each layer's cap forced to
/// its radius in `radii`, in one pass, less its stars at or beyond the radius in a layer not final,
/// and in C, D and E, which list so at the synthetic ceiling once final too (R13.T2.b), cut at
/// `n_max`.
fn census_to(radii: &[(Layer, LightYears, bool)], n_max: std::num::NonZeroU32) -> SkyCensus {
    let to_radii: Vec<(Layer, LightYears)> = radii.iter().map(|&(l, r, _)| (l, r)).collect();
    let query = SkyQuery::builder(observer(), Magnitudes::new(CAMERA_LIMIT_V))
        .n_max(n_max)
        .build()
        .expect("a query")
        .with_caps_forced_per_layer(&to_radii)
        .expect("forced caps");
    let forced = sim_census(&query);
    let by_texel = |layer: Layer| matches!(layer, Layer::C | Layer::D | Layer::E);
    let within: Vec<SkyStar> = forced
        .listed()
        .iter()
        .chain(forced.overflow())
        .filter(|star| {
            radii.iter().any(|&(layer, radius, is_final)| {
                layer == star.layer()
                    && ((is_final && !by_texel(layer)) || star.distance() < radius)
            })
        })
        .copied()
        .collect();
    merge_census([(within, *forced.tallies())], n_max)
}

/// The listed stars' bytes in `payload`, a reply's whose response is `response`, each star's own.
fn stars_sent(response: &SkyResponse, payload: &[u8]) -> Vec<Vec<u8>> {
    let stars_bytes = usize::try_from(response.stars_bytes).unwrap();
    let (stars, rest) = payload[..stars_bytes].as_chunks::<SKY_STAR_BYTES>();
    assert_eq!(rest, [] as [u8; 0]);
    stars.iter().map(|star| star.to_vec()).collect()
}

/// A sky arrives nearest first (R06.T11.d): a reply after each step of shells, each a whole sky
/// with its chunks numbered from 0, the last alone final. Each reply's stars are the census to the
/// radii it states, layer by layer, and the census with every cap forced to those radii, less its
/// stars at or beyond them, lists the same; its band is the one march's sum at those radii; and the
/// last reply is the census of the whole plan in one pass, star for star. At RM3's synthetic
/// ceiling (R13.T2.b), C, D and E list within their radii in the last reply too, so it is not the
/// one-shot census of R06, which lists every star of the cells it opens.
///
/// The caps are forced to 30 ly with nearer shell edges than the fixed ones (the server's test
/// seam), so that the census takes seconds: a census of the fixed edges' first shell, C, D and E
/// to 125 ly, is a minute or more of a worker in a test build. The seam's nearer deferrals keep
/// both of the delivery's: E's shells from 15 ly a step ahead of C's and D's (R06.T11.h), so that
/// one reply holds E's 20 ly shell before C's and D's, and C's from 20 ly a step later again.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_sky_arrives_nearest_first_each_reply_the_census_to_its_stated_radii() {
    let caps = SkyCaps::forced(LightYears::new(SMALL_CAP_LY))
        .expect("a small forced cap")
        .with_shell_edges(&TEST_SHELL_EDGES_LY)
        .with_c_deferred_beyond(TEST_C_DEFERRED_BEYOND_LY)
        .with_far_first_from(TEST_FAR_FIRST_FROM_LY);
    let (server, _data_dir) = sky_server_with(caps, 3).await;
    let (mut client, universe) = opened(&server).await;
    let n_max = std::num::NonZeroU32::new(24).expect("not zero");
    let id = client
        .send_request(RequestBody::Sky(SkyRequest {
            n_max: Some(n_max.get()),
            ..sky(&universe)
        }))
        .await;
    let replies = replies_to(&mut client, id).await;

    // The same sky in the sim, at the ceiling.
    let query = at_ceiling(
        SkyQuery::builder(observer(), Magnitudes::new(CAMERA_LIMIT_V))
            .n_max(n_max)
            .illumination(illumination_over(dark_tables())),
    )
    .with_caps_forced(LightYears::new(SMALL_CAP_LY))
    .expect("a forced cap");
    let forced = query.forced_caps().expect("forced caps").to_vec();
    let plan = census_plan_with_edges(&query, forced, &TEST_SHELL_EDGES_LY);
    let steps = delivery(&plan);
    assert_eq!(steps.len(), 7, "{steps:?}");
    assert_eq!(
        steps[2].iter().map(Shell::layer).collect::<Vec<_>>(),
        [Layer::E],
        "E's shell to 20 ly a step ahead of C's and D's: {steps:?}"
    );
    assert_eq!(replies.len(), steps.len(), "a reply a step");
    let marches = sim_marches(dark_tables(), &query, &plan);
    // Every reply's census folds the cells of the shells done so far, each cell censused once.
    let mut cells = CellParts::new(&query);

    let mut done: Vec<Shell> = Vec::new();
    let mut least_edges = Vec::new();
    for (k, (reply, step)) in replies.iter().zip(&steps).enumerate() {
        done.extend_from_slice(step);
        let completeness = plan.completeness(done.iter().copied());
        let response = &reply.response;
        let last = k + 1 == steps.len();
        assert_eq!((response.is_final, completeness.is_final()), (last, last));
        least_edges.push(completeness.least_edge().map(LightYears::value));
        let payload = whole_payload(reply, id, k);
        let radii = stated_radii(response, &completeness, SMALL_CAP_LY, k);

        // The stars are the census of the shells done, in its order, and the census forced to the
        // reply's radii, less its stars beyond them, lists the same.
        let census = sim_shells_census(&mut cells, &plan, &done);
        let sent = stars_sent(response, &payload);
        assert_eq!(sent, listed_on_the_wire(&census), "reply {k}'s stars");
        assert_eq!(
            (response.listed, response.overflow),
            (
                u32::try_from(census.listed().len()).unwrap(),
                u32::try_from(census.overflow().len()).unwrap()
            )
        );
        assert_eq!(
            sent,
            listed_on_the_wire(&census_to(&radii, n_max)),
            "reply {k} against the census forced to its radii"
        );

        // The band: the one march's sum at the reply's radii, texel for texel.
        let (band, _) = sim_band(&marches, &query, &census, completeness.complete_to());
        let stars_bytes = usize::try_from(response.stars_bytes).unwrap();
        assert_band_sent(&band, &payload[stars_bytes..], &format!("reply {k}"));
    }
    // Nearest first: the least edge reached never falls, and the last reply has none.
    assert_eq!(
        least_edges,
        [
            Some(10.0),
            Some(15.0),
            Some(15.0),
            Some(20.0),
            Some(20.0),
            Some(25.0),
            None
        ]
    );

    // The last reply is the whole plan's census in one pass, star for star and tally for tally,
    // folded from the cells the replies' censuses opened.
    let one_shot = sim_census_of(&mut cells, &plan_over(dark_tables(), &query));
    let last = replies.last().expect("a reply");
    assert_eq!(
        stars_sent(&last.response, &last.payload()),
        listed_on_the_wire(&one_shot)
    );
    assert_eq!(
        last.response.overflow,
        u32::try_from(one_shot.overflow().len()).unwrap()
    );
    for (dto, (layer, _)) in last.response.census.iter().zip(CAPPED) {
        let tally = one_shot.tallies().layer(layer);
        assert_eq!(
            (u64::from(dto.cells), dto.accepted, u64::from(dto.listed)),
            (tally.cells(), tally.accepted(), tally.listed()),
            "{layer:?}"
        );
    }

    client.close().await;
    server.stop().await;
}

/// perf-galaxy-build-start's table: on a server of eight workers that serves the sky to its
/// derived caps and warms the global list at an open, as a played server does, the seconds from a
/// universe's open to its open's answer, which waits for the galaxy, and to the first answer
/// after it: on one server the range query of `systems_in_range`'s table, at the Sun-like point to
/// 50 ly, and on another the first reply of a sky near the Sun, after which the sky is cancelled.
/// Each is asked at a first launch over a fresh data directory and again after a relaunch over
/// it. Its figures depend on the machine and the build, so it asserts only that the relaunched
/// range answer is the first's.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "measurement, run by hand: cargo test --release -p hyperion-server --test sky \
            first_answers_after_a_launch_and_a_relaunch -- --ignored --nocapture"]
async fn first_answers_after_a_launch_and_a_relaunch() {
    let workers = NonZeroUsize::new(8).expect("eight");
    eprintln!("| First answer | Launch | Open (s) | Answer after the open (s) |");
    eprintln!("| --- | --- | --- | --- |");
    for asked in ["range 50 ly", "sky's first reply"] {
        let data_dir = tempfile::tempdir().expect("a temporary directory");
        let config = || {
            TestServer::config(data_dir.path())
                .workers(workers)
                .sky_service(SkyService::Served)
                .global_list_warm_up(hyperion_server::GlobalListWarmUp::AtOpen)
                .build()
        };
        let mut universe = None;
        let mut answers = Vec::new();
        for launch in ["first", "relaunched"] {
            let server = TestServer::start_with(config()).await;
            let mut client = server.connected().await;
            let id = match &universe {
                Some(id) => UniverseIdHex::clone(id),
                None => client.create_universe("Sky: measured", SEED).await.id,
            };
            universe = Some(id.clone());
            let started = std::time::Instant::now();
            let body = RequestBody::OpenUniverse(OpenUniverseRequest {
                universe: id.clone(),
            });
            match client.request(body).await {
                Ok(ResponseBody::OpenUniverse(_)) => {}
                other => panic!("expected the opened universe, got {other:?}"),
            }
            let open = started.elapsed();
            // When the first answer arrived, before a sky is cancelled.
            let answered = if asked.starts_with("range") {
                let body = RequestBody::SystemsInRange(SystemsInRangeRequest {
                    universe: id.clone(),
                    centre: GalacticPosition {
                        cell_ly: [0, 26_000, 0],
                        offset_m: [0.0; 3],
                    },
                    radius_ly: 50.0,
                    time: UniverseTime::default(),
                    min_layer: MassLayer::A,
                    limit: hyperion_server::limits::MAX_CENSUS_LIMIT,
                    include_stellar: false,
                    mode: hyperion_protocol::QueryModeDto::Now,
                });
                match client.request(body).await {
                    Ok(ResponseBody::SystemsInRange(answer)) => {
                        answers.push(serde_json::to_string(&answer).unwrap());
                    }
                    other => panic!("expected the systems in range, got {other:?}"),
                }
                started.elapsed()
            } else {
                let sky_id = client.send_request(RequestBody::Sky(sky(&id))).await;
                let (finished, replied) = loop {
                    match client.next_frame().await {
                        Frame::Binary(..) => {}
                        Frame::Message(ServerMessage::PartialResponse { id: answered, .. })
                            if answered == sky_id =>
                        {
                            break (false, started.elapsed());
                        }
                        Frame::Message(ServerMessage::Response { id: answered, .. })
                            if answered == sky_id =>
                        {
                            break (true, started.elapsed());
                        }
                        Frame::Message(other) => panic!("expected the sky's reply, got {other:?}"),
                    }
                };
                if !finished {
                    client.cancel(sky_id).await;
                    loop {
                        match client.next_frame().await {
                            Frame::Message(ServerMessage::RequestError { id: ended, .. })
                                if ended == sky_id =>
                            {
                                break;
                            }
                            Frame::Binary(..) | Frame::Message(_) => {}
                        }
                    }
                }
                replied
            }
            .saturating_sub(open);
            eprintln!(
                "| {asked} | {launch} | {:.2} | {:.2} |",
                open.as_secs_f64(),
                answered.as_secs_f64()
            );
            client.close().await;
            server.stop().await;
        }
        if let [first, relaunched] = &answers[..] {
            assert_eq!(relaunched, first, "the relaunched range answer");
        }
    }
}
