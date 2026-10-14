//! A body's coarse field reaches a client only as far as Knowledge reaches (rendering plan R09,
//! R09.T19.c; the rendering brainstorm's Testing; R09's Verification, "Agreement" and
//! "Knowledge"), over a real socket, as R03's `scene_knowledge` bounds the scene.
//!
//! A server takes a reference world's inputs for every solid body (`reference_inputs`), since plan
//! 14's surface section is not inhabited yet. One orbital pass surveys a region of a generated
//! body: a block of cells at a cube corner, across two face edges; a ragged run at a face's
//! centre; and ragged runs about the field's widest crater and its largest drainage, once the
//! coarse pass places them (R09.T12.d and T14; the reference fields hold neither yet). Its payload
//! is then asked for over the socket, decoded as a client decodes it, and checked:
//!
//! - no block carries a cell outside the region and its margin of `SYNTHESIS_MARGIN_CELLS`, a
//!   climate record over another cell or a crater that reaches none of its cells, and every
//!   record is the server's own;
//! - margin cells are marked: code 0 on the wire, and held but not surveyed in the client's
//!   `PartialField`, which holds nothing else;
//! - the cover's resolution codes are the pass's, and after a second pass, finer and overlapping
//!   the first's edge, each cell's finest, in the delta and the whole field alike;
//! - the client's synthesis over its `PartialField` gives the server's own heights and gradients,
//!   bit for bit, on a lattice over every surveyed cell and along the surveyed region's edge, at
//!   band levels from 0 to the deepest, and refuses a point where it holds nothing;
//! - no response or notification, read as it arrived, has a key named `surface_seed`, and no frame,
//!   text or binary, holds the body's surface seed's value;
//! - the answer's header and block 0 carry the server's albedo scale, which an airless or
//!   thin-aired body's field has (R10.T10.a).
//!
//! Those payloads, on the Ceres-like world, are one block in one bulk frame each, so a second case
//! surveys two whole faces of the Mars-like world, whose payload is several blocks in several
//! frames, and checks it block by block.
//!
//! The server's heights are the synthesis over the coarse pass of the body's surface seed on the
//! same inputs, with the albedo scale the service sets, which `surface_service.rs` shows is the
//! field the server serves, bit for bit; every
//! record the client holds is checked against it as well. The detail seed is a stand-in shared by
//! both sides: R09.T1.b puts the body's own on the wire.
//!
//! Until the coarse pass's steps fill the records (R09.T11 to T15), every cell of a reference
//! field holds the same record, so the record checks bind which cells are held and at what codes
//! rather than which record sits where, and the heights compared vary only by the small craters,
//! though every read the synthesis makes must still find its cell held. The checks tighten by
//! themselves as the steps land (R09's Risks, T19.c's deferred findings).
//!
//! The heights' points are few because each is dear: the first query at a new point at a deep band
//! level took about 1 ms on the Ceres-like world in the test profile, at a load near 13, and about
//! 4 ms on the Mars-like one, the small craters' uncached draw that R09.T9's bench measured (R09's
//! Risks, T19.c's deferred findings). After the second pass
//! only its cells are sampled again, since a cell the first pass surveyed reads the same records
//! from a field that has only grown.
//!
//! Nothing here depends on how quickly the machine runs: each step waits for what the one before
//! it brought (an answer, or the scene telling of a revision), never for a time.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use common::surface::{
    FieldViaWire, ReferenceWorld, SEED, decode_all, field_via_wire, insert_all, partial_field,
    reference_inputs, served_field, surface_system,
};
use common::{TestClient, TestServer, patiently};
use hyperion_protocol::{
    BodyDetailRequest, BodyIdHex, CellRangeDto, ClientMessage, CoverDto, CoverRangeDto,
    CreateUniverseRequest, DetailLevelDto, FramePositionDto, KinematicsDto, NotificationBody,
    RequestBody, ResponseBody, SceneShipRequest, SceneSubscribeRequest, SeedHex, ServerMessage,
    SubscribeRequest, SubscriptionTopic, SurfaceFieldRequest, SurveyPassDto, SurveyPassRequest,
    SurveySourceDto, SystemBodiesRequest, SystemIdHex, UniverseIdHex, UniverseTime,
};
use hyperion_sim::Seed;
use hyperion_sim::id::BodyId;
use hyperion_sim::planetary::hooks::{SurfaceSeed, surface_seed};
use hyperion_sim::rng::DetailSeed;
use hyperion_sim::units::Metres;
use hyperion_surface::cube::{Edge, Face, MAX_LEVEL, PATCH_QUADS, PatchKey, SampleGrid};
use hyperion_surface::field::{
    CoarseField, CoarseLevel, Cover, CoverRange, FieldView, PartialField, ResolutionCode,
    SYNTHESIS_MARGIN_CELLS, cell_at_index, cell_index,
};
use hyperion_surface::height::HeightSample;
use hyperion_surface::synth::{BandLevel, QueryHeightError, SynthCache, Synthesiser};
use hyperion_surface::wire::DecodedBlock;
use serde_json::Value;
use tokio::sync::{oneshot, watch};
use tokio::task::JoinHandle;

/// The detail seed both sides synthesise from: a stand-in for the body's own, which R09.T1.b puts
/// on the wire (`body_detail`'s hooks section). The check is that the two sides agree given the
/// field and one seed, which any seed shows.
const DETAIL_SEED: DetailSeed = DetailSeed::new(0x0919_c19c_5eed_0001);

/// The first pass's resolution, metres: an orbital survey's.
const ORBITAL_M: f64 = 120.0;

/// The second pass's resolution, metres: a close-range survey's, finer than the first.
const CLOSE_RANGE_M: f64 = 10.0;

/// Cells either side of a feature's cell that a ragged run of the first pass takes, in index order.
const RUN_HALF_CELLS: u32 = 20;

/// Points a side of the lattice over each surveyed cell: its corners, its edges' midpoints and its
/// centre.
const LATTICE_POINTS: u32 = 3;

/// Vertices between points along an edge of the surveyed region: every fourth of a cell's 65.
const EDGE_STEP: u32 = 4;

/// The scene time the stand-in ship is set at, an hour after the epoch, and the time the records
/// are asked at.
const TIME: UniverseTime = UniverseTime {
    seconds: 3_600,
    nanos: 0,
};

/// What must never reach a client: the body's surface seed, by its key or by its value.
#[derive(Debug, Clone)]
struct Secret {
    /// The seed as sixteen lowercase hexadecimal digits, as the crate prints 64-bit identifiers.
    hex: String,
    /// The seed in decimal.
    decimal: String,
    /// The seed's eight bytes, little-endian and big-endian.
    bytes: [[u8; 8]; 2],
}

impl Secret {
    fn of(seed: SurfaceSeed) -> Self {
        let value = seed.get();
        Self {
            hex: format!("{value:016x}"),
            decimal: value.to_string(),
            bytes: [value.to_le_bytes(), value.to_be_bytes()],
        }
    }

    /// Checks a text frame as it arrived, and returns its message: no object in its JSON has a
    /// key named `surface_seed`, and its text does not hold the seed's value.
    fn check_text(&self, text: &str) -> ServerMessage {
        let json: Value = serde_json::from_str(text).expect("the server sends JSON");
        assert!(
            !has_key(&json, "surface_seed"),
            "a frame carries a key named `surface_seed`: {text}"
        );
        assert!(
            !text.contains(&self.hex) && !text.contains(&self.decimal),
            "a frame holds the surface seed's value: {text}"
        );
        serde_json::from_value(json).expect("the server sends valid messages")
    }

    /// Checks a payload's bytes: they do not hold the seed's value, in either byte order.
    fn check_binary(&self, payload: &[u8]) {
        for bytes in &self.bytes {
            assert!(
                !payload.windows(8).any(|window| window == bytes),
                "a payload holds the surface seed's value"
            );
        }
    }
}

/// Whether any object within `value` has a key named `key`.
fn has_key(value: &Value, key: &str) -> bool {
    match value {
        Value::Object(members) => members
            .iter()
            .any(|(name, member)| name == key || has_key(member, key)),
        Value::Array(items) => items.iter().any(|item| has_key(item, key)),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => false,
    }
}

/// Says hello on a fresh connection, its answer checked.
async fn hello(client: &mut TestClient, secret: &Secret) {
    client
        .send(&ClientMessage::Hello {
            client_version: "test".to_owned(),
        })
        .await;
    let answer = secret.check_text(&client.next_text().await);
    assert!(
        matches!(answer, ServerMessage::Welcome { .. }),
        "{answer:?}"
    );
}

/// Makes a request on a connection with no subscription and returns its answer, checked as it
/// arrived. Panics on a refusal or any other message.
async fn ask(client: &mut TestClient, secret: &Secret, body: RequestBody) -> ResponseBody {
    let id = client.send_request(body).await;
    match secret.check_text(&client.next_text().await) {
        ServerMessage::Response { id: answered, body } if answered == id => body,
        other => panic!("expected the answer to request {}, got {other:?}", id.0),
    }
}

/// What the scene's watcher read.
#[derive(Debug, Default, Clone, Copy)]
struct Heard {
    /// Text frames read and checked, the subscription's answer included.
    texts: usize,
    /// Of them, notifications that named surface revisions.
    with_revisions: usize,
}

/// A client subscribed to the universe's scene on its own connection, read by a task of its own so
/// that it never stops reading while the test computes: every text frame is checked as it
/// arrives, and the latest revision the notifications name for each body is kept.
#[derive(Debug)]
struct Watcher {
    told: watch::Receiver<BTreeMap<String, u32>>,
    stop: oneshot::Sender<()>,
    task: JoinHandle<Heard>,
}

impl Watcher {
    /// Subscribes to `universe`'s scene at `full` on a fresh connection, the state checked.
    async fn subscribe(server: &TestServer, secret: &Secret, universe: &UniverseIdHex) -> Self {
        let mut client = server.connect().await;
        hello(&mut client, secret).await;
        let id = client
            .send_request(RequestBody::Subscribe(SubscribeRequest {
                universe: universe.clone(),
                topic: SubscriptionTopic::Scene(SceneSubscribeRequest {
                    detail: DetailLevelDto::Full,
                    cameras: Vec::new(),
                }),
            }))
            .await;
        // The state answers before any notification: a subscription is live only once its answer
        // is queued (R03.T5.b, `subscriptions`).
        match secret.check_text(&client.next_text().await) {
            ServerMessage::Response {
                id: answered,
                body: ResponseBody::Subscribe(_),
            } if answered == id => {}
            other => panic!("expected the scene's state, got {other:?}"),
        }
        let (sender, told) = watch::channel(BTreeMap::<String, u32>::new());
        let (stop, mut stopped) = oneshot::channel::<()>();
        let secret = secret.clone();
        let task = tokio::spawn(async move {
            let mut heard = Heard {
                texts: 2,
                ..Heard::default()
            };
            loop {
                let text = tokio::select! {
                    _ = &mut stopped => return heard,
                    text = client.next_text() => text,
                };
                heard.texts += 1;
                match secret.check_text(&text) {
                    ServerMessage::Notification {
                        body: NotificationBody::Scene(scene),
                        ..
                    } => {
                        if let Some(revisions) = scene.surface_revisions {
                            heard.with_revisions += 1;
                            sender.send_modify(|told| {
                                for raised in revisions {
                                    let held =
                                        told.entry(raised.body.as_str().to_owned()).or_default();
                                    *held = (*held).max(raised.revision);
                                }
                            });
                        }
                    }
                    other => panic!("expected a scene notification, got {other:?}"),
                }
            }
        });
        Self { told, stop, task }
    }

    /// Waits until the scene has told of `body` at `revision` or later.
    async fn told(&mut self, body: &BodyIdHex, revision: u32) {
        let reached = patiently(
            &format!(
                "the scene to tell of {} at revision {revision}",
                body.as_str()
            ),
            self.told.wait_for(|told| {
                told.get(body.as_str())
                    .is_some_and(|&told| told >= revision)
            }),
        )
        .await
        .map(|_| ());
        if reached.is_err() {
            // The task has ended, which it does only when a check fails: show that failure.
            let failed = (&mut self.task)
                .await
                .expect_err("the watcher's task ends only when stopped or on a failed check");
            std::panic::resume_unwind(failed.into_panic());
        }
    }

    /// Stops reading, and returns what was read.
    async fn stop(self) -> Heard {
        // The task may have ended on a failed check, so that nothing receives; its join says so.
        let _ = self.stop.send(());
        patiently("the watcher's task to stop", self.task)
            .await
            .unwrap_or_else(|failed| std::panic::resume_unwind(failed.into_panic()))
    }
}

/// The ID of `body` as the wire writes it.
fn hex(body: BodyId) -> BodyIdHex {
    BodyIdHex::from_parts(body.system().raw(), body.body_index())
}

/// A survey pass of `body` over `cells` at `resolution_m`, in the first hour and a half.
fn pass(
    universe: &UniverseIdHex,
    body: BodyId,
    source: SurveySourceDto,
    resolution_m: f64,
    cells: &BTreeSet<u32>,
) -> SurveyPassRequest {
    let mut runs: Vec<CellRangeDto> = Vec::new();
    for &cell in cells {
        match runs.last_mut() {
            Some(run) if run.end == cell => run.end = cell + 1,
            _ => runs.push(CellRangeDto {
                start: cell,
                end: cell + 1,
            }),
        }
    }
    SurveyPassRequest {
        universe: universe.clone(),
        body: hex(body),
        from: UniverseTime {
            seconds: 0,
            nanos: 0,
        },
        to: UniverseTime {
            seconds: 5_400,
            nanos: 0,
        },
        source,
        resolution_m,
        cells: runs,
    }
}

/// The code of a resolution, as a pass records it.
fn code(resolution_m: f64) -> ResolutionCode {
    ResolutionCode::from_resolution(Metres::new(resolution_m)).expect("a survey's resolution")
}

/// The cells of `level` whose index is in `start..end`, clipped to the face of `start`.
fn run_on_face(level: CoarseLevel, start: u32, end: u32) -> impl Iterator<Item = u32> {
    let per_face = level.cell_count() / 6;
    let face_end = (start / per_face + 1) * per_face;
    start..end.min(face_end)
}

/// The first pass's region at `field`'s level: face 0's first 64 cells, the 8 × 8 block in its
/// corner at a cube corner, which reaches across two face edges; 41 cells in index order from the
/// sixth of the 8 × 8 block at face 3's centre, a ragged shape; and ragged runs, in index order,
/// about the cell of `field`'s widest crater and about its cell of largest drainage, if it has
/// either.
fn first_region(field: &CoarseField) -> BTreeSet<u32> {
    let level = field.header().level();
    let mut region: BTreeSet<u32> = (0..64).collect();
    let half = 1_u32 << (level.get() - 1);
    let centre = PatchKey::new(Face::ALL[3], level.get(), half, half).expect("a cell of the level");
    let ragged = cell_index(centre) + 5;
    region.extend(ragged..ragged + 41);
    let widest = field
        .craters()
        .iter()
        .max_by(|a, b| a.diameter.value().total_cmp(&b.diameter.value()))
        .map(|crater| {
            cell_index(PatchKey::containing(level.get(), crater.centre).expect("a coarse level"))
        });
    let drained = (0_u32..)
        .zip(field.synthesis())
        .filter(|(_, cell)| cell.drainage.get() > 0)
        .max_by_key(|(_, cell)| cell.drainage.get())
        .map(|(index, _)| index);
    for feature in [widest, drained].into_iter().flatten() {
        let per_face = level.cell_count() / 6;
        let face_start = feature / per_face * per_face;
        let start = feature.saturating_sub(RUN_HALF_CELLS).max(face_start);
        region.extend(run_on_face(level, start, feature + RUN_HALF_CELLS + 1));
    }
    region
}

/// The second pass's region: face 0's cells 48 to 127, a quarter of the first pass's corner block
/// surveyed again more finely, with the block beside it, which the first pass's margin reached.
fn second_region() -> BTreeSet<u32> {
    (48..128).collect()
}

/// Each surveyed cell's finest code over the passes so far.
type Surveyed = BTreeMap<u32, ResolutionCode>;

/// Adds a pass of `cells` at `code` to `surveyed`: a cell keeps its finest code.
fn add_pass(surveyed: &mut Surveyed, cells: &BTreeSet<u32>, code: ResolutionCode) {
    for &cell in cells {
        surveyed
            .entry(cell)
            .and_modify(|held| *held = held.finer(code))
            .or_insert(code);
    }
}

/// The cover of cells and their codes, in index order.
fn cover_of(cells: impl IntoIterator<Item = (u32, ResolutionCode)>) -> Cover {
    let mut runs: Vec<(u32, u32, ResolutionCode)> = Vec::new();
    for (cell, code) in cells {
        match runs.last_mut() {
            Some((_, end, last)) if *end == cell && *last == code => *end = cell + 1,
            _ => runs.push((cell, cell + 1, code)),
        }
    }
    Cover::from_ranges(
        runs.into_iter().map(|(start, end, code)| {
            CoverRange::new(start, end, code).expect("a run holds a cell")
        }),
    )
    .expect("cells in index order")
}

/// What a client may hold after the passes of `surveyed`: their cells with their codes, and their
/// margin at code 0.
fn held(level: CoarseLevel, surveyed: &Surveyed) -> Cover {
    cover_of(surveyed.iter().map(|(&cell, &code)| (cell, code)))
        .with_margin(level, SYNTHESIS_MARGIN_CELLS)
}

/// The cells of `now` whose code `then` did not hold: a delta's cells (R09.T19.b).
fn delta(now: &Cover, then: &Cover) -> Cover {
    cover_of(now.ranges().iter().flat_map(|range| {
        (range.start()..range.end())
            .filter(move |&cell| then.code(cell) != Some(range.code()))
            .map(move |cell| (cell, range.code()))
    }))
}

/// A cover as the wire writes it.
fn dto(cover: &Cover) -> CoverDto {
    CoverDto {
        ranges: cover
            .ranges()
            .iter()
            .map(|range| CoverRangeDto {
                start: range.start(),
                end: range.end(),
                resolution: range.code().get(),
            })
            .collect(),
    }
}

/// The cell of index `index` at `level`.
fn cell_of(level: CoarseLevel, index: u32) -> PatchKey {
    cell_at_index(level.get(), index).expect("a cell of the level")
}

/// The slot of a cell index in a field's per-cell records.
fn slot(index: u32) -> usize {
    usize::try_from(index).expect("a cell index fits a usize")
}

/// Checks an answer and its blocks against `server`'s field: the answer names the blocks' cells,
/// `sent`, and its header is block 0's; and no block carries a cell outside `held` or with another
/// code, a climate record but its cells' own or a crater that reaches none of its cells, and every
/// record it carries is the server's.
fn check_blocks(
    answer: &FieldViaWire,
    blocks: &[DecodedBlock],
    held: &Cover,
    sent: &Cover,
    server: &CoarseField,
) {
    let level = server.header().level();
    let header = blocks[0].header().expect("block 0 carries the header");
    // The reference worlds name their own body, whatever body was asked for (R09.T17's deferred
    // finding), so the answer is compared with the header it was encoded from.
    let body = header.body();
    assert_eq!(
        answer.header.body,
        BodyIdHex::from_parts(body.raw_system_id(), body.body_index()),
        "the answer's header is block 0's"
    );
    assert_eq!(answer.header.level, level.get());
    assert_eq!(header.body(), server.header().body());
    // The albedo scale R10.T10.a sets on an airless or thin-aired body's field, in the answer and
    // in block 0, is the server's (the module's documentation).
    let scale = server.header().albedo_scale().map(f64::to_bits);
    assert!(
        scale.is_some(),
        "a reference world's field has an albedo scale"
    );
    assert_eq!(
        answer.header.albedo_scale.map(f64::to_bits),
        scale,
        "the answer's albedo scale"
    );
    assert_eq!(
        header.albedo_scale().map(f64::to_bits),
        scale,
        "block 0's albedo scale"
    );
    let union = Cover::from_ranges(
        blocks
            .iter()
            .flat_map(|block| block.cover().ranges().iter().copied()),
    )
    .expect("the blocks' covers follow one another in index order");
    assert_eq!(&union, sent, "the blocks hold the cells to send");
    assert_eq!(
        answer.coverage,
        dto(sent),
        "the answer names the blocks' cells"
    );
    for block in blocks {
        let at = block.index();
        for range in block.cover().ranges() {
            for cell in range.start()..range.end() {
                assert_eq!(
                    held.code(cell),
                    Some(range.code()),
                    "block {at}: cell {cell} is in the survey or its margin, at its code"
                );
            }
        }
        for (cell, record) in block.cover().cells().zip(block.cells()) {
            assert_eq!(
                record,
                &server.synthesis()[slot(cell)],
                "block {at}: cell {cell}"
            );
        }
        let mut parents: Vec<u32> = block
            .cover()
            .cells()
            .map(|cell| {
                cell_index(
                    cell_of(level, cell)
                        .parent()
                        .expect("a coarse cell has a parent"),
                )
            })
            .collect();
        parents.dedup();
        assert_eq!(
            block.climate_indices(),
            parents,
            "block {at}: the climate records over its cells, once each"
        );
        for (&parent, record) in block.climate_indices().iter().zip(block.climate()) {
            assert_eq!(record, &server.climate_layer()[slot(parent)], "block {at}");
        }
        for crater in block.craters() {
            assert!(
                crater
                    .reach
                    .cells()
                    .any(|cell| block.cover().contains(cell)),
                "block {at}: a crater reaches one of its cells"
            );
            assert!(
                server.craters().contains(crater),
                "block {at}: a crater is the server's"
            );
        }
    }
}

/// Checks the client's field against `held`, cell by cell over the whole level: a held cell is
/// the server's, its climate and the craters that reach it too, at its code, surveyed only if its
/// code is a survey's; and a cell not held is not there at all.
fn check_held(client: &PartialField, held: &Cover, server: &CoarseField) {
    let level = server.header().level();
    for index in 0..level.cell_count() {
        let cell = cell_of(level, index);
        let code = held.code(index);
        assert_eq!(client.resolution(cell), code, "cell {index}'s code");
        assert_eq!(
            client.is_surveyed(cell),
            code.is_some_and(ResolutionCode::is_surveyed),
            "cell {index} is surveyed only if a pass covered it"
        );
        if code.is_some() {
            assert_eq!(client.cell(cell), server.cell(cell), "cell {index}");
            assert_eq!(client.climate(cell), server.climate(cell), "cell {index}");
            assert_eq!(
                client.craters_reaching(cell).collect::<Vec<_>>(),
                server.craters_reaching(cell).collect::<Vec<_>>(),
                "cell {index}: the craters that reach it"
            );
        } else {
            assert_eq!(client.cell(cell), None, "cell {index} is not held");
            assert_eq!(client.climate(cell), None, "cell {index} is not held");
            assert_eq!(client.craters_reaching(cell).count(), 0, "cell {index}");
        }
    }
}

/// The band levels the heights are compared at: 0; the field's own level L; L + 4; and the
/// deepest, [`MAX_LEVEL`], which evaluates the band limit's contributions on every body (R09.T8).
fn band_levels(level: CoarseLevel) -> [BandLevel; 4] {
    [0, level.get(), level.get() + 4, MAX_LEVEL]
        .map(|level| BandLevel::new(level).expect("a band level"))
}

/// A height's bits: two heights are one only if every bit agrees.
fn bits(sample: HeightSample) -> (u64, [u64; 3]) {
    (sample.height_m.to_bits(), sample.gradient.map(f64::to_bits))
}

/// The points the heights were compared at.
#[derive(Debug, Default, Clone, Copy)]
struct Sampled {
    /// Points over the surveyed cells.
    points: usize,
    /// Of them, points on the surveyed region's edge.
    on_edge: usize,
}

/// What a heights check samples: `cells`, surveyed, of the surveyed cells `surveyed`, and a
/// point in a cell beyond `held`, the cells the client holds.
#[derive(Debug, Clone)]
struct Sampling {
    cells: BTreeSet<u32>,
    surveyed: Surveyed,
    held: Cover,
}

/// Checks that the client's synthesis gives the server's heights and gradients, bit for bit, at
/// every point of a lattice over each of the sampling's cells and along every edge such a cell
/// shares with a cell not surveyed, at each of [`band_levels`]; and that it refuses a point in a
/// cell it does not hold, which the server answers.
fn check_heights(server: &CoarseField, client: &PartialField, sampling: &Sampling) -> Sampled {
    let level = server.header().level();
    let levels = band_levels(level);
    let (ours, theirs) = (
        Synthesiser::new(server, DETAIL_SEED),
        Synthesiser::new(client, DETAIL_SEED),
    );
    let (mut our_cache, mut their_cache) = (SynthCache::default(), SynthCache::default());
    let mut compare = |index: u32, dir: [f64; 3]| {
        for &band in &levels {
            let own = ours
                .height_at(&mut our_cache, dir, band)
                .expect("the server holds the whole field");
            let sent = theirs
                .height_at(&mut their_cache, dir, band)
                .unwrap_or_else(|error| {
                    panic!(
                        "cell {index}, at {dir:?}, band level {}: a surveyed point is refused: \
                         {error}",
                        band.get()
                    )
                });
            assert_eq!(
                bits(sent),
                bits(own),
                "cell {index}, at {dir:?}, band level {}: {sent:?} against the server's {own:?}",
                band.get()
            );
        }
    };
    let mut sampled = Sampled::default();
    let lattice = slot(PATCH_QUADS / (LATTICE_POINTS - 1));
    for &index in &sampling.cells {
        let cell = cell_of(level, index);
        for x in (0..=PATCH_QUADS).step_by(lattice) {
            for y in (0..=PATCH_QUADS).step_by(lattice) {
                compare(index, cell.sample_dir(x, y, SampleGrid::Mesh));
                sampled.points += 1;
            }
        }
        for edge in Edge::ALL {
            if sampling
                .surveyed
                .contains_key(&cell_index(cell.edge_neighbour(edge)))
            {
                continue;
            }
            for along in (0..=PATCH_QUADS).step_by(slot(EDGE_STEP)) {
                let (x, y) = match edge {
                    Edge::UMin => (0, along),
                    Edge::UMax => (PATCH_QUADS, along),
                    Edge::VMin => (along, 0),
                    Edge::VMax => (along, PATCH_QUADS),
                };
                compare(index, cell.sample_dir(x, y, SampleGrid::Mesh));
                sampled.points += 1;
                sampled.on_edge += 1;
            }
        }
    }
    // A cell the client does not hold: the last in index order.
    let far = (0..level.cell_count())
        .rev()
        .find(|&index| sampling.held.code(index).is_none())
        .expect("the passes leave cells unsurveyed");
    let dir = cell_of(level, far).vertex_dir(32, 32);
    ours.height_at(&mut our_cache, dir, levels[0])
        .expect("the server holds the whole field");
    assert!(
        matches!(
            theirs.height_at(&mut their_cache, dir, levels[0]),
            Err(QueryHeightError::NotSurveyed(_))
        ),
        "the client cannot synthesise where it holds nothing"
    );
    sampled
}

/// Runs `work`, named `what`, on a thread of its own and waits for its result within the
/// harness's patience; a panic in `work` is the caller's.
///
/// A thread rather than `spawn_blocking`: a runtime's drop waits for its blocking tasks, so a
/// `spawn_blocking` job that hung would hold the test after its wait had timed out, where a thread
/// whose wait times out is left behind and the test fails.
async fn on_a_thread<T: Send + 'static>(
    what: &str,
    work: impl FnOnce() -> T + Send + 'static,
) -> T {
    let (sender, receiver) = oneshot::channel();
    let thread = std::thread::spawn(move || {
        // The receiver is gone only when the wait timed out, and the test has failed already.
        let _ = sender.send(work());
    });
    match patiently(what, receiver).await {
        Ok(result) => result,
        // The sender was dropped unsent: the work panicked, and the thread is ending.
        Err(_) => std::panic::resume_unwind(
            thread
                .join()
                .expect_err("a thread that sent nothing panicked"),
        ),
    }
}

/// Runs [`check_heights`] on a thread of its own ([`on_a_thread`]), and gives the client's field
/// back.
async fn heights_checked(
    server: &Arc<CoarseField>,
    client: PartialField,
    sampling: Sampling,
) -> (PartialField, Sampled) {
    let server = Arc::clone(server);
    on_a_thread("the heights to be compared", move || {
        let sampled = check_heights(&server, &client, &sampling);
        (client, sampled)
    })
    .await
}

/// A server serving a reference world's inputs; a universe of [`SEED`] with the stand-in ship in
/// the surveyed body's system; a client on a connection with no subscription, and a watcher of the
/// universe's scene on another; and the field the server computes for the body, computed here.
#[derive(Debug)]
struct Session {
    server: TestServer,
    /// The server's data directory, kept until the server stops.
    _data_dir: tempfile::TempDir,
    client: TestClient,
    watcher: Watcher,
    secret: Secret,
    universe: UniverseIdHex,
    body: BodyId,
    field: Arc<CoarseField>,
}

impl Session {
    /// Starts a server that gives every solid body `world`'s inputs, and reads the records where a
    /// body's seeds would travel if any did, the body's hooks section among them, every answer
    /// checked as it arrives.
    async fn start(world: ReferenceWorld) -> Self {
        let body = surface_system().solid;
        let seed = surface_seed(Seed::new(SEED), body);
        let secret = Secret::of(seed);
        let field = on_a_thread("the reference world's field", move || {
            served_field(world, body)
        })
        .await;
        let data_dir = tempfile::tempdir().expect("a temporary directory");
        let server = TestServer::start_with(
            TestServer::config(data_dir.path())
                .surface_inputs(reference_inputs(world))
                .build(),
        )
        .await;
        let mut client = server.connect().await;
        hello(&mut client, &secret).await;
        let create = RequestBody::CreateUniverse(CreateUniverseRequest {
            name: "Surveyed".to_owned(),
            seed: Some(SeedHex::from_u64(SEED)),
        });
        let ResponseBody::CreateUniverse(universe) = ask(&mut client, &secret, create).await else {
            panic!("create_universe answers with the universe");
        };
        let universe = universe.id;
        let system = SystemIdHex::from_u64(body.system().raw());
        // The stand-in ship in the body's system, so that the scene holds its bodies' records.
        let ship = RequestBody::SceneShip(SceneShipRequest {
            universe: universe.clone(),
            ship: KinematicsDto {
                position: FramePositionDto::System {
                    system: system.clone(),
                    offset_m: [1.496e11, 0.0, 0.0],
                },
                velocity_m_s: [0.0; 3],
                time: TIME,
            },
            time_rate: 1,
        });
        let answer = ask(&mut client, &secret, ship).await;
        assert!(matches!(answer, ResponseBody::SceneShip(_)), "{answer:?}");
        let watcher = Watcher::subscribe(&server, &secret, &universe).await;
        let detail = RequestBody::BodyDetail(BodyDetailRequest {
            universe: universe.clone(),
            body: hex(body),
            time: TIME,
            detail: DetailLevelDto::Full,
        });
        let answer = ask(&mut client, &secret, detail).await;
        let json = serde_json::to_value(&answer).expect("an answer serialises");
        assert!(
            has_key(&json, "hooks"),
            "the body's record has its hooks section"
        );
        let bodies = RequestBody::SystemBodies(SystemBodiesRequest {
            universe: universe.clone(),
            system,
            time: TIME,
            detail: DetailLevelDto::Full,
        });
        let answer = ask(&mut client, &secret, bodies).await;
        assert!(
            matches!(answer, ResponseBody::SystemBodies(_)),
            "{answer:?}"
        );
        Self {
            server,
            _data_dir: data_dir,
            client,
            watcher,
            secret,
            universe,
            body,
            field: Arc::new(field),
        }
    }

    /// Records a pass of the body over `cells`, answered as revision `revision`, and waits until
    /// the scene tells of it.
    async fn survey(
        &mut self,
        source: SurveySourceDto,
        resolution_m: f64,
        cells: &BTreeSet<u32>,
        revision: u32,
    ) {
        let request = pass(&self.universe, self.body, source, resolution_m, cells);
        assert_eq!(
            ask(
                &mut self.client,
                &self.secret,
                RequestBody::SurveyPass(request)
            )
            .await,
            ResponseBody::SurveyPass(SurveyPassDto { revision })
        );
        self.watcher.told(&hex(self.body), revision).await;
    }

    /// The body's field over the socket, whole or since `have_revision`, which must answer at
    /// `revision`: the answer, checked for the seed, and its blocks, decoded.
    async fn field_at(
        &mut self,
        have_revision: Option<u32>,
        revision: u32,
    ) -> (FieldViaWire, Vec<DecodedBlock>) {
        let request = SurfaceFieldRequest {
            universe: self.universe.clone(),
            body: hex(self.body),
            have_revision,
        };
        let answer = field_via_wire(&mut self.client, request).await;
        self.secret.check_text(&answer.text);
        self.secret.check_binary(&answer.payload);
        assert_eq!(answer.revision, revision, "the body's revision");
        let blocks = decode_all(&answer.payload);
        (answer, blocks)
    }

    /// Stops the watcher, which must have been told of `passes` passes, the client and the server.
    async fn stop(self, passes: usize) {
        let heard = self.watcher.stop().await;
        assert!(
            heard.with_revisions >= passes,
            "the scene told of every pass: {heard:?}"
        );
        self.client.close().await;
        self.server.stop().await;
    }
}

/// Surveys a generated body of a server serving `world`'s inputs, and checks that what a client is
/// sent of its field, and what the client makes of it, is bounded by the surveys (the module's
/// documentation).
async fn knowledge_bound(world: ReferenceWorld) {
    let mut session = Session::start(world).await;
    let field = Arc::clone(&session.field);
    let level = field.header().level();

    // One orbital pass over the first region.
    let first = first_region(&field);
    let mut surveyed = Surveyed::new();
    add_pass(&mut surveyed, &first, code(ORBITAL_M));
    session
        .survey(SurveySourceDto::Orbital, ORBITAL_M, &first, 1)
        .await;
    let (whole, blocks) = session.field_at(None, 1).await;
    let first_held = held(level, &surveyed);
    check_blocks(&whole, &blocks, &first_held, &first_held, &field);
    let known = partial_field(&blocks);
    check_held(&known, &first_held, &field);
    let sampling = Sampling {
        cells: first,
        surveyed: surveyed.clone(),
        held: first_held.clone(),
    };
    let (mut known, sampled) = heights_checked(&field, known, sampling).await;
    assert!(
        sampled.on_edge > 0 && sampled.points > sampled.on_edge,
        "{sampled:?}"
    );

    // A finer pass over the corner block's edge: the delta since the first revision carries only
    // the cells it brought or surveyed again, and the client's field takes each cell's finest code.
    let second = second_region();
    add_pass(&mut surveyed, &second, code(CLOSE_RANGE_M));
    session
        .survey(SurveySourceDto::CloseRange, CLOSE_RANGE_M, &second, 2)
        .await;
    let second_held = held(level, &surveyed);
    let (since, delta_blocks) = session.field_at(Some(1), 2).await;
    let sent = delta(&second_held, &first_held);
    check_blocks(&since, &delta_blocks, &second_held, &sent, &field);
    insert_all(&mut known, &delta_blocks);
    check_held(&known, &second_held, &field);
    let (whole, blocks) = session.field_at(None, 2).await;
    check_blocks(&whole, &blocks, &second_held, &second_held, &field);
    check_held(&partial_field(&blocks), &second_held, &field);
    // The first pass's cells read the same records as before from a field that has only grown, so
    // the second pass's are the ones to sample again.
    let sampling = Sampling {
        cells: second,
        surveyed,
        held: second_held,
    };
    let (_, sampled) = heights_checked(&field, known, sampling).await;
    assert!(sampled.on_edge > 0, "{sampled:?}");
    session.stop(2).await;
}

/// On the Ceres-like world, a level-5 field and the quickest: what a client is sent and what it
/// synthesises from it is bounded by the two passes, and is the server's own.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_client_holds_only_the_surveyed_field_and_synthesises_the_server_s_heights() {
    knowledge_bound(ReferenceWorld::Ceres).await;
}

/// On the Mars-like world, a level-7 field: one orbital pass over two whole faces gives a payload
/// of several blocks in several bulk frames, and each block holds only the region's cells and its
/// margin, every record the server's, as does the client's field built from them all. The other
/// test's payloads are one block in one frame each.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_payload_of_several_blocks_and_frames_holds_only_the_surveyed_cells() {
    let mut session = Session::start(ReferenceWorld::Mars).await;
    let field = Arc::clone(&session.field);
    let level = field.header().level();
    let per_face = level.cell_count() / 6;
    let region: BTreeSet<u32> = (per_face..3 * per_face).collect();
    let mut surveyed = Surveyed::new();
    add_pass(&mut surveyed, &region, code(ORBITAL_M));
    session
        .survey(SurveySourceDto::Orbital, ORBITAL_M, &region, 1)
        .await;
    let (whole, blocks) = session.field_at(None, 1).await;
    assert!(
        blocks.len() > 1 && whole.frames.len() > blocks.len(),
        "{} blocks in {} frames",
        blocks.len(),
        whole.frames.len()
    );
    let held = held(level, &surveyed);
    check_blocks(&whole, &blocks, &held, &held, &field);
    check_held(&partial_field(&blocks), &held, &field);
    session.stop(1).await;
}
