//! The surface's handlers (rendering plan R09): `survey_pass` (R09.T19.a) and `surface_field`
//! (R09.T19.b).
//!
//! A `survey_pass` starts from [`openable_universe`], as every handler that names a universe does,
//! checks its fields and decodes the body's ID ([`SurveyRequest`]), takes the body's system from
//! the body cache as `body_detail` does ([`bodies_of`]), and reads the level of the body's coarse
//! field on an interactive pool job ([`SurfaceService::field_level`]). The pass is then recorded
//! through the universe's survey log, which the surface service opens on its first use and keeps:
//! [`SurveyLog::record`](crate::knowledge::surveys::SurveyLog::record) is the only way coverage
//! grows, and this request its stand-in caller until sensors exist (Design note 16). Recording
//! raises the body's revision on the universe's scene topic, whose subscriptions name it in their
//! next notification's `surface_revisions`. A pass of a body whose inputs are modelled also starts
//! the body's field, so that the `surface_field` that follows finds it computed or computing
//! (Design note 19).
//!
//! A `surface_field` resolves its body in the same order, answers a body whose inputs are not
//! modelled `not_modelled` from the same interactive job, with no bulk slot taken, and reads the
//! body's coverage from the survey log, the revision the client says it holds checked against it,
//! before it asks the service for the field, computed once and kept. The payload is then encoded
//! on a bulk job, gated by the coverage ([`gate`]): the surveyed cells with their codes and their
//! margin, the climate over them and every crater that reaches them, and with `have_revision`
//! only what the client's revision did not hold (Design notes 15 to 17). Its bytes are R03's bulk
//! payload, sent as binary frames before the answer ([`Answer`]).
//!
//! [`SurfaceService::field_level`]: crate::surface::SurfaceService::field_level

use std::sync::Arc;

use axum::body::Bytes;
use hyperion_protocol::{
    BodyIdHex, ErrorCode, RequestError, ResponseBody, SurfaceFieldDto, SurfaceFieldRequest,
    SurveyPassDto, SurveyPassRequest,
};
use hyperion_sim::id::SystemId;
use hyperion_sim::planetary::ResolveBodyError;
use hyperion_sim::planetary::surface::SurfaceInputsError;
use hyperion_surface::field::{
    CoarseField, CoarseLevel, Cover, CoverRange, FieldView, ResolutionCode, SYNTHESIS_MARGIN_CELLS,
};
use hyperion_surface::wire::encode_payload;

use super::universe::openable_universe;
use super::{bodies_of, request_error};
use crate::AppState;
use crate::bulk::{Answer, BulkPayload};
use crate::compute::sky::bulk;
use crate::compute::{CancelToken, GalaxyKey, GenerateBodiesError, GeneratedSystem};
use crate::convert::{
    FieldRequest, SurveyRequest, body_refusal, cover_dto, open_surveys_refusal, record_section,
    record_survey_refusal, surface_header, surface_refusal,
};
use crate::knowledge::surveys::{CoverageHistory, CoverageRevision};
use crate::surface::{FieldKey, FieldLevel, SurfaceJobError};

/// Records one survey pass of a body and answers with the body's new coverage revision.
///
/// The level of the body's field is read on an interactive job, which then checks the pass's cells
/// against the field's cell count. A body whose inputs are not modelled yet is surveyed all the
/// same, at the level of its record's figure ([`FieldLevel::NotModelled`]; ruled 2026-10-13), and
/// starts no field; one whose inputs are modelled starts its field, which the answer does not wait
/// for. A pass that has reached the log stays there if the client cancels the request meanwhile, as
/// a universe created after its requester gave up stays created.
///
/// # Errors
///
/// Those of [`openable_universe`]; `bad_request` naming `from` or `to` outside the clock window,
/// `to` before `from`, `resolution_m` that is not finite and positive or coarser than about
/// 36,000 km, and `cells` holding more than `MAX_SURVEY_RANGES` ranges; on the field `body`,
/// `unknown_system`, `bad_request` for an index outside plan 14's layout and `unknown_body` for one
/// at which the system holds no body, as `body_detail` refuses them, and `bad_request` for a body
/// with no solid surface or with no figure to give its field's level; the galaxy's; `bad_request`
/// naming `cells` for runs that are none, empty, out of order, overlapping or past the field's last
/// cell; `queue_full`, `internal` or `cancelled` from the pool's jobs; `unsupported_save_format` or
/// `internal` if the universe's survey log cannot be opened; and `storage_failed` if the pass
/// cannot be saved.
pub(crate) async fn survey_pass(
    state: Arc<AppState>,
    request: SurveyPassRequest,
    token: CancelToken,
) -> Result<ResponseBody, RequestError> {
    let universe = openable_universe(&state, &request.universe)?;
    let wanted = SurveyRequest::try_from(&request)?;
    let key = universe.key();
    let system = system_of(&state, key, wanted.system(), &request.body).await?;
    let field = FieldKey::new(key, wanted.body());
    let level = state
        .surface
        .field_level(field, &system, token)
        .await
        .map_err(|error| surface_refusal(&request.body, error))?;
    let pass = wanted.into_pass(level.level())?;
    let log = state
        .surface
        .survey_log(state.registry.store(), universe.id())
        .await
        .map_err(|error| open_surveys_refusal(&error))?;
    let revision = log
        .record(pass)
        .await
        .map_err(|error| record_survey_refusal(&error))?;
    let field_started = match level {
        FieldLevel::Modelled(_) => state.surface.prefetch(field, &system),
        FieldLevel::NotModelled { .. } => false,
    };
    tracing::debug!(
        body = request.body.as_str(),
        %revision,
        field_started,
        "recorded a survey pass"
    );
    Ok(ResponseBody::SurveyPass(SurveyPassDto {
        revision: revision.get(),
    }))
}

/// Answers `surface_field`: the body's coarse field in the cells its survey passes covered, with
/// their margin, as R03's bulk payload before a [`SurfaceFieldDto::Ready`]; or, for a body whose
/// inputs are not modelled, [`SurfaceFieldDto::NotModelled`] naming the section, with no bulk.
///
/// The answer states the body's coverage revision it reflects, 0 before the first pass, whose
/// payload is block 0 alone, the field's header. With `have_revision` (other than 0) the payload is
/// the delta since that revision: the cells of the body's cover and margin that the client's did
/// not hold, or held with another code, and the answer's `coverage` names them alone. The field is
/// computed on the first request for it, or found computing or kept, and the payload encoded on a
/// bulk job; the same request at the same revision gives the same bytes.
///
/// # Errors
///
/// Those of [`openable_universe`]; on the field `body`, `unknown_system`, `bad_request` for an
/// index outside plan 14's layout and `unknown_body` for one at which the system holds no body,
/// as `body_detail` refuses them, and `bad_request` for a body with no solid surface; the
/// galaxy's; `unsupported_save_format` or `internal` if the universe's survey log cannot be opened;
/// `bad_request` naming `have_revision` for a revision above the body's; `queue_full`, `internal`
/// or `cancelled` from the pool's jobs; and `internal` if the body's passes index another level
/// than its field, which only a server fault makes.
pub(crate) async fn field(
    state: Arc<AppState>,
    request: SurfaceFieldRequest,
    token: CancelToken,
) -> Result<Answer, RequestError> {
    let universe = openable_universe(&state, &request.universe)?;
    let asked = FieldRequest::try_from(&request)?;
    let key = universe.key();
    let system = system_of(&state, key, asked.system(), &request.body).await?;
    let field_key = FieldKey::new(key, asked.body());
    // The interactive job reads the record and the inputs, so that a body with no field is
    // answered without a bulk slot (R09.T17's deferred finding).
    match state
        .surface
        .field_level(field_key, &system, token.clone())
        .await
    {
        Ok(FieldLevel::Modelled(_)) => {}
        Ok(FieldLevel::NotModelled { section, .. })
        | Err(SurfaceJobError::Inputs(SurfaceInputsError::NotModelled(section))) => {
            return Ok(Answer::from(ResponseBody::SurfaceField(
                SurfaceFieldDto::NotModelled {
                    section: record_section(section),
                },
            )));
        }
        Err(error) => return Err(surface_refusal(&request.body, error)),
    }
    let log = state
        .surface
        .survey_log(state.registry.store(), universe.id())
        .await
        .map_err(|error| open_surveys_refusal(&error))?;
    let history = log.history(asked.body());
    let latest = history.as_ref().map(|history| history.latest().revision());
    let since = asked.since(latest)?;
    let revision = latest.map_or(0, CoverageRevision::get);
    let field = state
        .surface
        .field(field_key, &system)
        .await
        .map_err(|error| surface_refusal(&request.body, error))?;
    if let Some(history) = &history {
        check_level(&request, history.latest().level(), field.header().level())?;
    }
    let header = Box::new(surface_header(field.header()));
    let gated = bulk(&state.pool, &token, move |_: &CancelToken| {
        gate(&field, history.as_ref(), since)
    })
    .await?;
    let payload = BulkPayload::new(Bytes::from(gated.bytes)).expect(
        "a field's payload, some 15 MB at the most, has far fewer chunks than a u32 counts",
    );
    tracing::debug!(
        body = request.body.as_str(),
        revision,
        since = since.map(CoverageRevision::get),
        bytes = payload.manifest().bytes,
        "answered a body's coarse field"
    );
    Ok(Answer {
        body: ResponseBody::SurfaceField(SurfaceFieldDto::Ready {
            header,
            revision,
            coverage: cover_dto(&gated.cover),
            bulk: payload.manifest(),
        }),
        bulk: Some(payload),
    })
}

/// The planetary system of the body `body` names, `system`, from the body cache, refused as
/// `body_detail` refuses an unresolved system, naming `body`.
///
/// # Errors
///
/// `unknown_system` naming `body`, and the pool's errors.
async fn system_of(
    state: &Arc<AppState>,
    key: GalaxyKey,
    system: SystemId,
    body: &BodyIdHex,
) -> Result<Arc<GeneratedSystem>, RequestError> {
    let galaxy = state.galaxies.get(key).await?;
    bodies_of(state, key, &galaxy, system)
        .await
        .map_err(|error| match error {
            GenerateBodiesError::NoSuchSystem(error) => {
                body_refusal(body, ResolveBodyError::NoSuchSystem(error))
            }
            GenerateBodiesError::Compute(error) => error.into(),
        })
}

/// Refuses a body whose survey passes index `passes`, another level than its field's, `field`:
/// both come from one source's radius, so only a fault of the server's makes them differ, and the
/// payload's cells would be another level's.
///
/// # Errors
///
/// `internal`, logged with both levels.
fn check_level(
    request: &SurfaceFieldRequest,
    passes: CoarseLevel,
    field: CoarseLevel,
) -> Result<(), RequestError> {
    if passes == field {
        return Ok(());
    }
    tracing::error!(
        body = request.body.as_str(),
        passes = passes.get(),
        field = field.get(),
        "a body's survey passes index another level than its coarse field"
    );
    Err(request_error(
        ErrorCode::Internal,
        "the server's survey of the body does not fit its field",
    ))
}

/// A field's payload as the coverage gates it, and the cells it holds.
#[derive(Debug)]
struct Gated {
    /// The payload's blocks.
    bytes: Vec<u8>,
    /// The cells the blocks hold, with their codes, margin cells at [`ResolutionCode::NONE`].
    cover: Cover,
}

/// The payload of `field` that the coverage `history` lets a client hold, and the cells it holds,
/// whole or, with `since`, the delta since that revision (Design notes 15 to 17).
///
/// The body's latest coverage, converted to a [`Cover`] of its runs, is what
/// [`encode_payload`] gates by: its cells with their codes and their margin of
/// [`SYNTHESIS_MARGIN_CELLS`], the climate over them and every crater whose reach touches one; and
/// `since`'s coverage, folded again from the body's first passes, is the cover the client holds,
/// never with its margin, which the encoder adds to both. No history is no pass: block 0 alone.
///
/// # Panics
///
/// If `since` is past the history's latest revision, which the handler refused, or `since` names a
/// revision with no history.
#[must_use]
fn gate(
    field: &CoarseField,
    history: Option<&CoverageHistory>,
    since: Option<CoverageRevision>,
) -> Gated {
    let cover = history.map_or_else(Cover::new, |history| history.latest().to_cover());
    let since = since.map(|revision| {
        history
            .and_then(|history| history.at(revision))
            .expect("a revision at most the body's own is in its history")
            .to_cover()
    });
    let bytes = encode_payload(field, &cover, since.as_ref());
    let cover = sent(field.header().level(), &cover, since.as_ref());
    Gated { bytes, cover }
}

/// The cells [`encode_payload`] sends for `cover` since `since` at `level`, with their codes: the
/// cells of `cover` and its margin that `since` and its margin did not hold, or held with another
/// code, as the encoder chooses them; the whole of `cover` with its margin without `since`.
#[must_use]
fn sent(level: CoarseLevel, cover: &Cover, since: Option<&Cover>) -> Cover {
    let held = cover.with_margin(level, SYNTHESIS_MARGIN_CELLS);
    let Some(since) = since else {
        return held;
    };
    let had = since.with_margin(level, SYNTHESIS_MARGIN_CELLS);
    let mut runs: Vec<(u32, u32, ResolutionCode)> = Vec::new();
    for range in held.ranges() {
        let code = range.code();
        for cell in (range.start()..range.end()).filter(|&cell| had.code(cell) != Some(code)) {
            match runs.last_mut() {
                Some((_, end, last)) if *end == cell && *last == code => *end = cell + 1,
                _ => runs.push((cell, cell + 1, code)),
            }
        }
    }
    Cover::from_ranges(
        runs.into_iter().map(|(start, end, code)| {
            CoverRange::new(start, end, code).expect("a run holds a cell")
        }),
    )
    .expect("runs found in index order are sorted and disjoint")
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use hyperion_protocol::{
        CellRangeDto, ClientMessage, CoverDto, CreateUniverseRequest, DetailLevelDto,
        NotificationBody, RecordSectionDto, RequestBody, RequestId, SceneNotificationDto,
        SceneSubscribeRequest, ServerMessage, SubscribeRequest, SubscriptionTopic,
        SurfaceRevisionDto, SurveySourceDto, UniverseIdHex, UniverseTime, UnsubscribeRequest,
    };
    use hyperion_sim::Seed;
    use hyperion_sim::galaxy::Galaxy;
    use hyperion_sim::galaxy::placement::{CellKey, generate_cell};
    use hyperion_sim::id::{BodyId, Layer, SystemId};
    use hyperion_sim::planetary::record::{BodyRecord, Section};
    use hyperion_sim::planetary::surface::{CoarseInputs, SurfaceInputsError, reference};
    use hyperion_sim::planetary::{SystemContext, generate};
    use hyperion_sim::units::Metres;
    use hyperion_surface::field::coarse_level;
    use hyperion_surface::wire::decode_payload;
    use tokio::time::timeout;

    use super::*;
    use crate::convert::cover_dto;
    use crate::knowledge::surveys::SurveyLog;
    use crate::limits::{MAX_INBOUND_FRAME_BYTES, MAX_SURVEY_RANGES};
    use crate::requests::{Handlers, is_large, to_frame};
    use crate::surface::{InputsSource, RECORD_TIME};
    use crate::testing::{Client, Harness, Received, WAIT};
    use crate::universe::UniverseId;
    use crate::ws::ConnectionLimits;

    /// The seed of the universe these tests survey, the surface service's tests' own.
    const SEED: u64 = 0x4d2;

    /// A source that gives every body whose surface section applies the reference Ceres's
    /// inputs, a level-5 field of 6,144 cells, and refuses a giant's as the record's own reading
    /// would.
    #[derive(Debug)]
    struct Ceres;

    impl InputsSource for Ceres {
        fn inputs(
            &self,
            record: &BodyRecord,
            _: &SystemContext,
        ) -> Result<CoarseInputs, SurfaceInputsError> {
            match record.surface() {
                Section::NotApplicable => Err(SurfaceInputsError::NoSolidSurface),
                Section::Ok(_) | Section::NotModelled | Section::NotResolved => {
                    Ok(reference::ceres_like())
                }
            }
        }
    }

    /// The reference Ceres's level.
    fn ceres_level() -> CoarseLevel {
        let level = coarse_level(reference::ceres_like().radius());
        assert_eq!(level.get(), 5);
        level
    }

    /// Bodies of one generated system of [`SEED`]: two with a solid surface, a giant, and the
    /// level the first solid body's figure gives.
    #[derive(Debug, Clone, Copy)]
    struct Bodies {
        solid: BodyId,
        other_solid: BodyId,
        giant: BodyId,
        figure_level: CoarseLevel,
    }

    /// The first system of the layer-C cell at the solar circle with two bodies whose surface
    /// section applies and one whose does not, generated once per test binary.
    fn bodies() -> &'static Bodies {
        static BODIES: OnceLock<Bodies> = OnceLock::new();
        BODIES.get_or_init(|| {
            let galaxy = Galaxy::new(Seed::new(SEED));
            let mut cell = Vec::new();
            generate_cell(
                &galaxy,
                CellKey::new(Layer::C, [0, 812, 0]).expect("a cell of the grid"),
                &mut cell,
            );
            cell.iter()
                .find_map(|record| {
                    let ctx = SystemContext::for_system(&galaxy, record.id()).ok()?;
                    let planets = generate(Seed::new(SEED), &ctx);
                    let mut solid = Vec::new();
                    let mut giant = None;
                    for body in planets.bodies() {
                        let index = body.index();
                        if !matches!(planets.surface_seed(index), Ok(Some(_))) {
                            continue;
                        }
                        let Ok(record) = planets.body_at(&ctx, index, RECORD_TIME) else {
                            continue;
                        };
                        let id = index.body_id(planets.system());
                        match (record.surface(), record.figure()) {
                            (Section::NotApplicable, _) => giant = giant.or(Some(id)),
                            (_, Section::Ok(figure)) => solid.push((id, figure.spheroid())),
                            _ => {}
                        }
                    }
                    let (&[(first, spheroid), (second, _), ..], Some(giant)) =
                        (solid.as_slice(), giant)
                    else {
                        return None;
                    };
                    Some(Bodies {
                        solid: first,
                        other_solid: second,
                        giant,
                        figure_level: coarse_level(Metres::new(spheroid.volumetric_radius_m())),
                    })
                })
                .expect("a system of the cell has two solid bodies and a giant")
        })
    }

    fn hex(body: BodyId) -> BodyIdHex {
        BodyIdHex::from_parts(body.system().raw(), body.body_index())
    }

    /// A server whose surface service takes `source`'s inputs, with a universe of [`SEED`].
    async fn started(source: impl InputsSource + 'static) -> (Harness, UniverseIdHex) {
        let harness = Harness::start_configured(Handlers, ConnectionLimits::default(), |config| {
            config.surface_inputs(source)
        })
        .await;
        let created = super::super::universe::create(
            Arc::clone(harness.state()),
            CreateUniverseRequest {
                name: "Survey".to_owned(),
                seed: Some(hyperion_protocol::SeedHex::from_u64(SEED)),
            },
        )
        .await
        .unwrap();
        let ResponseBody::CreateUniverse(info) = created else {
            panic!("create_universe answers with the universe: {created:?}");
        };
        (harness, info.id)
    }

    /// An orbital pass of `body` at 120 m over `cells`, in the first hour and a half after the
    /// epoch.
    fn pass(universe: &UniverseIdHex, body: BodyId, cells: &[(u32, u32)]) -> SurveyPassRequest {
        SurveyPassRequest {
            universe: universe.clone(),
            body: hex(body),
            from: UniverseTime {
                seconds: 0,
                nanos: 0,
            },
            to: UniverseTime {
                seconds: 5_400,
                nanos: 250_000_000,
            },
            source: SurveySourceDto::Orbital,
            resolution_m: 120.0,
            cells: cells
                .iter()
                .map(|&(start, end)| CellRangeDto { start, end })
                .collect(),
        }
    }

    async fn survey(
        harness: &Harness,
        request: SurveyPassRequest,
    ) -> Result<ResponseBody, RequestError> {
        timeout(
            WAIT,
            survey_pass(Arc::clone(harness.state()), request, CancelToken::new()),
        )
        .await
        .expect("timed out recording a survey pass")
    }

    fn revision(answer: Result<ResponseBody, RequestError>) -> u32 {
        match answer {
            Ok(ResponseBody::SurveyPass(SurveyPassDto { revision })) => revision,
            other => panic!("expected a survey pass recorded, got {other:?}"),
        }
    }

    fn refused(answer: Result<ResponseBody, RequestError>) -> (ErrorCode, Option<String>) {
        let error = answer.unwrap_err();
        (error.code, error.field)
    }

    fn bad(field: &str) -> (ErrorCode, Option<String>) {
        (ErrorCode::BadRequest, Some(field.to_owned()))
    }

    /// The universe's survey log, as the surface service holds it.
    async fn log(harness: &Harness, universe: &UniverseIdHex) -> SurveyLog {
        harness
            .state()
            .surface
            .survey_log(harness.state().registry.store(), UniverseId::from(universe))
            .await
            .unwrap()
    }

    /// Yields until `ready` holds.
    async fn until(what: &str, ready: impl Fn() -> bool) {
        timeout(WAIT, async {
            while !ready() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("timed out waiting until {what}"));
    }

    /// Each pass of a body raises its revision by one, from 1; the passes reach the universe's
    /// log, which keeps them across an open, and fold into the body's coverage at the field's own
    /// level; and a pass of a body whose inputs are modelled starts its field.
    #[tokio::test]
    async fn survey_passes_are_recorded_and_count_the_body_s_revisions() {
        let (harness, universe) = started(Ceres).await;
        let Bodies {
            solid, other_solid, ..
        } = *bodies();
        assert_eq!(
            revision(survey(&harness, pass(&universe, solid, &[(0, 96), (100, 101)])).await),
            1
        );
        let mut finer = pass(&universe, solid, &[(64, 128)]);
        finer.resolution_m = 2.0;
        finer.source = SurveySourceDto::Landed;
        assert_eq!(revision(survey(&harness, finer).await), 2);
        assert_eq!(
            revision(survey(&harness, pass(&universe, other_solid, &[(6_000, 6_144)])).await),
            1,
            "each body counts its own passes"
        );

        let coverage = log(&harness, &universe).await.coverage(solid).unwrap();
        assert_eq!(
            (
                coverage.level(),
                coverage.revision().get(),
                coverage.surveyed()
            ),
            (ceres_level(), 2, 128),
            "0..96 and 100 under the first pass, 64..128 under the second"
        );
        let coarse = ResolutionCode::from_resolution(Metres::new(120.0)).unwrap();
        let fine = ResolutionCode::from_resolution(Metres::new(2.0)).unwrap();
        assert_eq!(coverage.code(0), Some(coarse));
        assert_eq!(coverage.code(64), Some(fine), "the finer pass wins");
        assert_eq!(coverage.code(100), Some(fine));
        assert_eq!(coverage.code(128), Some(ResolutionCode::NONE));

        // The passes are in the file: a log opened afresh from it folds the same coverage.
        let reopened = SurveyLog::open(
            harness.state().registry.store(),
            UniverseId::from(&universe),
        )
        .await
        .unwrap();
        assert_eq!(reopened.coverage(solid), Some(coverage));

        // Both bodies' fields were started; neither answer waited for them.
        until("both surveyed bodies' fields are computed", || {
            harness.state().surface.counters().computed() == 2
        })
        .await;
        harness.stop().await;
    }

    /// Two first passes of one universe at once open its log once: had each opened its own, both
    /// would be the body's first.
    #[tokio::test]
    async fn two_first_survey_passes_share_one_log() {
        let (harness, universe) = started(Ceres).await;
        let solid = bodies().solid;
        let (one, other) = tokio::join!(
            survey(&harness, pass(&universe, solid, &[(0, 1)])),
            survey(&harness, pass(&universe, solid, &[(1, 2)])),
        );
        let mut revisions = [revision(one), revision(other)];
        revisions.sort_unstable();
        assert_eq!(revisions, [1, 2]);
        harness.stop().await;
    }

    /// Plan 14's surface section is not modelled yet, so the production source answers every
    /// solid body `NotModelled`: its pass is recorded all the same, at the level its figure gives,
    /// and starts no field.
    #[tokio::test]
    async fn a_body_not_modelled_is_surveyed_at_its_figure_s_level() {
        let harness = Harness::start(Handlers).await;
        let created = super::super::universe::create(
            Arc::clone(harness.state()),
            CreateUniverseRequest {
                name: "Unmodelled".to_owned(),
                seed: Some(hyperion_protocol::SeedHex::from_u64(SEED)),
            },
        )
        .await
        .unwrap();
        let ResponseBody::CreateUniverse(info) = created else {
            panic!("create_universe answers with the universe: {created:?}");
        };
        let Bodies {
            solid,
            figure_level,
            ..
        } = *bodies();
        let last = figure_level.cell_count();
        assert_eq!(
            revision(survey(&harness, pass(&info.id, solid, &[(last - 4, last)])).await),
            1
        );
        let coverage = log(&harness, &info.id).await.coverage(solid).unwrap();
        assert_eq!((coverage.level(), coverage.surveyed()), (figure_level, 4));
        assert_eq!(
            refused(survey(&harness, pass(&info.id, solid, &[(last, last + 1)])).await),
            bad("cells"),
            "past the figure level's last cell"
        );
        assert_eq!(harness.state().surface.counters().computed(), 0);
        assert_eq!(
            harness.state().surface.counters().cache().misses(),
            0,
            "no field was started"
        );
        harness.stop().await;
    }

    /// A body with no solid surface (a giant, whose surface section is not applicable) is refused
    /// naming `body`, and nothing is written.
    #[tokio::test]
    async fn a_body_with_no_solid_surface_is_refused() {
        let (harness, universe) = started(Ceres).await;
        let giant = bodies().giant;
        let error = survey(&harness, pass(&universe, giant, &[(0, 1)]))
            .await
            .unwrap_err();
        assert_eq!(
            (error.code, error.field.as_deref()),
            (ErrorCode::BadRequest, Some("body")),
            "{error:?}"
        );
        assert!(error.message.contains("no solid surface"), "{error:?}");
        assert_eq!(log(&harness, &universe).await.coverage(giant), None);
        harness.stop().await;
    }

    /// A body is resolved and refused as `body_detail` refuses it, naming `body`.
    #[tokio::test]
    async fn an_unknown_or_malformed_body_is_refused_as_body_detail_refuses_it() {
        let (harness, universe) = started(Ceres).await;
        let system = bodies().solid.system();
        let with_body = |raw: u64, index: u16| {
            let mut request = pass(&universe, bodies().solid, &[(0, 1)]);
            request.body = BodyIdHex::from_parts(raw, index);
            request
        };
        // A reserved slot, and a sub-index a planet's slot reserves.
        for index in [0xd000, 0x0190] {
            assert_eq!(
                refused(survey(&harness, with_body(system.raw(), index)).await),
                bad("body"),
                "{index:#06x}"
            );
        }
        // Slot 0xCF's first planet: inside plan 14's layout, and held by no system this small.
        assert_eq!(
            refused(survey(&harness, with_body(system.raw(), 0xcf00)).await),
            (ErrorCode::UnknownBody, Some("body".to_owned()))
        );
        // Bits that are no system ID, and a grid ID past any cell's count of candidates.
        let missing = CellKey::new(Layer::C, [0, 812, 0])
            .unwrap()
            .candidate_id(60_000)
            .expect("an index the layer's IDs can hold");
        for raw in [u64::MAX, missing.raw()] {
            assert_eq!(
                refused(survey(&harness, with_body(raw, 0x0100)).await),
                (ErrorCode::UnknownSystem, Some("body".to_owned())),
                "{raw:#018x}"
            );
        }
        harness.stop().await;
    }

    /// Each field out of its range is refused naming it: the times, the resolution and the cells,
    /// and nothing is recorded.
    #[tokio::test]
    async fn a_bad_time_resolution_or_cells_is_refused_naming_its_field() {
        let (harness, universe) = started(Ceres).await;
        let solid = bodies().solid;
        let good = || pass(&universe, solid, &[(0, 1)]);
        let beyond_window = 40_000_000_000;

        let mut early = good();
        early.from.seconds = -beyond_window;
        let mut late = good();
        late.to.seconds = beyond_window;
        let mut backwards = good();
        backwards.from.seconds = 6_000;
        let mut cases = vec![(early, "from"), (late, "to"), (backwards, "to")];
        for resolution_m in [0.0, -1.0, f64::INFINITY, f64::NAN, 4.0e7] {
            let mut request = good();
            request.resolution_m = resolution_m;
            cases.push((request, "resolution_m"));
        }
        let cells = ceres_level().cell_count();
        for runs in [
            &[][..],
            &[(5, 5)],
            &[(10, 20), (15, 30)],
            &[(10, 20), (0, 5)],
            &[(20, 10)],
            &[(cells - 1, cells + 1)],
        ] {
            cases.push((pass(&universe, solid, runs), "cells"));
        }
        for (request, field) in cases {
            let answer = survey(&harness, request.clone()).await;
            assert_eq!(refused(answer), bad(field), "{request:?}");
        }
        assert_eq!(log(&harness, &universe).await.coverage(solid), None);
        harness.stop().await;
    }

    /// At most `MAX_SURVEY_RANGES` ranges, a pass's frame fits the inbound limit even at its
    /// widest, every number at its longest; such a pass is recorded over the socket, and one more
    /// range is refused naming `cells` before the body is looked at.
    #[tokio::test]
    async fn a_survey_pass_of_256_ranges_fits_an_inbound_frame_and_257_are_refused() {
        let widest = ClientMessage::Request {
            id: RequestId(u32::MAX),
            body: RequestBody::SurveyPass(SurveyPassRequest {
                universe: UniverseIdHex::from_u64(u64::MAX),
                body: BodyIdHex::from_parts(u64::MAX, u16::MAX),
                from: UniverseTime {
                    seconds: -31_557_600_000,
                    nanos: 999_999_999,
                },
                to: UniverseTime {
                    seconds: -31_557_600_000,
                    nanos: 999_999_999,
                },
                source: SurveySourceDto::CloseRange,
                // The longest shortest form an `f64` has, 24 characters.
                resolution_m: -f64::MIN_POSITIVE,
                cells: vec![
                    CellRangeDto {
                        start: u32::MAX,
                        end: u32::MAX,
                    };
                    MAX_SURVEY_RANGES
                ],
            }),
        };
        let text = serde_json::to_string(&widest).unwrap();
        assert!(
            text.len() <= MAX_INBOUND_FRAME_BYTES,
            "{} bytes against {MAX_INBOUND_FRAME_BYTES}",
            text.len()
        );

        let (harness, universe) = started(Ceres).await;
        let solid = bodies().solid;
        let runs: Vec<(u32, u32)> = (0..256).map(|n| (2 * n, 2 * n + 1)).collect();
        let mut client = harness.connect().await;
        client.hello().await;
        client
            .request(7, RequestBody::SurveyPass(pass(&universe, solid, &runs)))
            .await;
        match client.next_message().await {
            ServerMessage::Response {
                id: RequestId(7),
                body: ResponseBody::SurveyPass(SurveyPassDto { revision: 1 }),
            } => {}
            other => panic!("expected the pass recorded, got {other:?}"),
        }
        assert_eq!(
            log(&harness, &universe)
                .await
                .coverage(solid)
                .unwrap()
                .surveyed(),
            256
        );

        let mut over = pass(&universe, solid, &runs);
        over.cells.push(CellRangeDto {
            start: 600,
            end: 601,
        });
        over.body = BodyIdHex::from_parts(u64::MAX, 0xd000);
        assert_eq!(over.cells.len(), MAX_SURVEY_RANGES + 1);
        assert_eq!(refused(survey(&harness, over).await), bad("cells"));
        client.close().await;
        harness.stop().await;
    }

    /// Plan 04's row R09: `surface_field` is in the large size class whatever its answer, and a
    /// survey pass's answer is small.
    #[test]
    fn the_surface_field_is_large_and_a_survey_pass_is_not() {
        let not_modelled = ResponseBody::SurfaceField(SurfaceFieldDto::NotModelled {
            section: RecordSectionDto::Surface,
        });
        assert!(is_large(&not_modelled));
        assert!(!is_large(&ResponseBody::SurveyPass(SurveyPassDto {
            revision: 1
        })));
        // Both serialise as any response does.
        let frame = to_frame(&ServerMessage::Response {
            id: RequestId(3),
            body: not_modelled,
        });
        assert!(
            frame.contains(r#""kind":"surface_field","state":"not_modelled""#),
            "{frame}"
        );
    }

    /// The system holding [`bodies`] is the one their IDs name.
    #[test]
    fn the_surveyed_bodies_share_a_system() {
        let Bodies {
            solid,
            other_solid,
            giant,
            ..
        } = *bodies();
        let system: SystemId = solid.system();
        assert_eq!((other_solid.system(), giant.system()), (system, system));
        assert_ne!(solid, other_solid);
    }

    /// A `surface_field` request for `body`, holding `have_revision`.
    fn field_request(
        universe: &UniverseIdHex,
        body: BodyId,
        have_revision: Option<u32>,
    ) -> SurfaceFieldRequest {
        SurfaceFieldRequest {
            universe: universe.clone(),
            body: hex(body),
            have_revision,
        }
    }

    async fn ask(harness: &Harness, request: SurfaceFieldRequest) -> Result<Answer, RequestError> {
        timeout(
            WAIT,
            field(Arc::clone(harness.state()), request, CancelToken::new()),
        )
        .await
        .expect("timed out asking for a surface field")
    }

    /// What a `ready` answer says, and its payload's bytes.
    #[derive(Debug, PartialEq)]
    struct Served {
        revision: u32,
        coverage: CoverDto,
        bytes: Vec<u8>,
    }

    fn served(answer: Result<Answer, RequestError>) -> Served {
        let Ok(Answer {
            body:
                ResponseBody::SurfaceField(SurfaceFieldDto::Ready {
                    header,
                    revision,
                    coverage,
                    bulk: manifest,
                }),
            bulk: Some(payload),
        }) = answer
        else {
            panic!("expected a field served in bulk, got {answer:?}");
        };
        assert_eq!(payload.manifest(), manifest);
        let blocks = decode_payload(payload.bytes()).expect("the payload decodes");
        let decoded = blocks[0].header().expect("block 0 carries the header");
        assert_eq!(
            *header,
            surface_header(decoded),
            "the answer's header is block 0's"
        );
        // The blocks' covers, in cell-index order, are the cells the answer names.
        let held = Cover::from_ranges(
            blocks
                .iter()
                .flat_map(|block| block.cover().ranges().iter().copied()),
        )
        .expect("the blocks' covers are in order");
        assert_eq!(
            cover_dto(&held),
            coverage,
            "the answer names the payload's cells"
        );
        Served {
            revision,
            coverage,
            bytes: payload.bytes().to_vec(),
        }
    }

    /// The cells of a cover as the wire names them, with their codes.
    fn cells(coverage: &CoverDto) -> Vec<(u32, u8)> {
        coverage
            .ranges
            .iter()
            .flat_map(|range| (range.start..range.end).map(|cell| (cell, range.resolution)))
            .collect()
    }

    /// The cover of `runs` at `code`, as a pass covers them.
    fn cover_of(runs: &[(u32, u32)], code: ResolutionCode) -> Cover {
        Cover::from_ranges(
            runs.iter()
                .map(|&(start, end)| CoverRange::new(start, end, code).unwrap()),
        )
        .unwrap()
    }

    /// Each cell of `now` with its margin, at `level`, whose code `then` with its margin did not
    /// hold: a delta's cells, found cell by cell.
    fn delta(level: CoarseLevel, now: &Cover, then: &Cover) -> Vec<(u32, u8)> {
        let (now, then) = (
            now.with_margin(level, SYNTHESIS_MARGIN_CELLS),
            then.with_margin(level, SYNTHESIS_MARGIN_CELLS),
        );
        (0..level.cell_count())
            .filter_map(|cell| {
                let code = now.code(cell)?;
                (then.code(cell) != Some(code)).then_some((cell, code.get()))
            })
            .collect()
    }

    /// A field asked for with no `have_revision` holds the surveyed cells with their codes and
    /// their margin; after a second pass, the delta since the first revision holds only the cells
    /// it added or surveyed more finely, and the margin newly reached, while the whole field holds
    /// both passes' cells.
    #[tokio::test]
    async fn a_second_pass_s_delta_carries_only_new_cells() {
        let (harness, universe) = started(Ceres).await;
        let solid = bodies().solid;
        let level = ceres_level();
        let coarse = ResolutionCode::from_resolution(Metres::new(120.0)).unwrap();
        let fine = ResolutionCode::from_resolution(Metres::new(2.0)).unwrap();
        assert_eq!(
            revision(survey(&harness, pass(&universe, solid, &[(0, 64)])).await),
            1
        );
        let first = served(ask(&harness, field_request(&universe, solid, None)).await);
        let first_cover = cover_of(&[(0, 64)], coarse);
        assert_eq!(first.revision, 1);
        assert_eq!(
            first.coverage,
            cover_dto(&first_cover.with_margin(level, SYNTHESIS_MARGIN_CELLS)),
            "the surveyed cells at their code and the margin at 0"
        );

        let mut second = pass(&universe, solid, &[(32, 48), (3_000, 3_064)]);
        second.resolution_m = 2.0;
        assert_eq!(revision(survey(&harness, second).await), 2);
        let since = served(ask(&harness, field_request(&universe, solid, Some(1))).await);
        assert_eq!(since.revision, 2);
        let both = Cover::from_ranges([
            CoverRange::new(0, 32, coarse).unwrap(),
            CoverRange::new(32, 48, fine).unwrap(),
            CoverRange::new(48, 64, coarse).unwrap(),
            CoverRange::new(3_000, 3_064, fine).unwrap(),
        ])
        .unwrap();
        let sent = cells(&since.coverage);
        assert_eq!(sent, delta(level, &both, &first_cover));
        let at = |cell: u32| {
            sent.iter()
                .find(|&&(at, _)| at == cell)
                .map(|&(_, code)| code)
        };
        assert_eq!(at(0), None, "a cell surveyed once is not sent again");
        assert_eq!(
            at(40),
            Some(fine.get()),
            "a cell surveyed again more finely is"
        );
        assert_eq!(at(3_000), Some(fine.get()), "and so is a new one");
        assert!(
            sent.iter().all(|&(cell, _)| (32..48).contains(&cell)
                || first_cover
                    .with_margin(level, SYNTHESIS_MARGIN_CELLS)
                    .code(cell)
                    .is_none()),
            "nothing the first answer held unchanged"
        );

        let whole = served(ask(&harness, field_request(&universe, solid, None)).await);
        assert_eq!(
            whole.coverage,
            cover_dto(&both.with_margin(level, SYNTHESIS_MARGIN_CELLS))
        );
        assert!(whole.bytes.len() > since.bytes.len());
        harness.stop().await;
    }

    /// The same request at the same revision gives the same bytes, whole or as a delta, from the
    /// field kept; and a request naming the latest revision gets block 0 alone.
    #[tokio::test]
    async fn the_same_surface_field_request_twice_gives_identical_bytes() {
        let (harness, universe) = started(Ceres).await;
        let solid = bodies().solid;
        revision(survey(&harness, pass(&universe, solid, &[(0, 96), (100, 101)])).await);
        revision(survey(&harness, pass(&universe, solid, &[(2_000, 2_100)])).await);
        for have in [None, Some(1), Some(2)] {
            let once = served(ask(&harness, field_request(&universe, solid, have)).await);
            let twice = served(ask(&harness, field_request(&universe, solid, have)).await);
            assert_eq!(once, twice, "have_revision {have:?}");
        }
        assert_eq!(
            harness.state().surface.counters().computed(),
            1,
            "one pass served them all"
        );
        let latest = served(ask(&harness, field_request(&universe, solid, Some(2))).await);
        assert_eq!(latest.coverage, CoverDto { ranges: Vec::new() });
        assert_eq!(decode_payload(&latest.bytes).unwrap().len(), 1);
        harness.stop().await;
    }

    /// A body with no pass is served at revision 0, block 0 alone, as it is asked with 0; a
    /// revision above the body's is refused naming `have_revision`.
    #[tokio::test]
    async fn a_body_with_no_pass_is_served_as_its_header_alone() {
        let (harness, universe) = started(Ceres).await;
        let solid = bodies().solid;
        let none = served(ask(&harness, field_request(&universe, solid, None)).await);
        assert_eq!(
            (none.revision, none.coverage.ranges.len()),
            (0, 0),
            "no cell"
        );
        let blocks = decode_payload(&none.bytes).unwrap();
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].cells().is_empty());
        assert_eq!(
            served(ask(&harness, field_request(&universe, solid, Some(0))).await),
            none
        );
        let ahead = ask(&harness, field_request(&universe, solid, Some(1)))
            .await
            .unwrap_err();
        assert_eq!(
            (ahead.code, ahead.field.as_deref()),
            (ErrorCode::BadRequest, Some("have_revision")),
            "{ahead:?}"
        );
        revision(survey(&harness, pass(&universe, solid, &[(0, 1)])).await);
        let ahead = ask(&harness, field_request(&universe, solid, Some(2)))
            .await
            .unwrap_err();
        assert_eq!(ahead.field.as_deref(), Some("have_revision"));
        harness.stop().await;
    }

    /// Plan 14's surface section is not modelled yet, so under the production source a solid body
    /// answers `not_modelled` naming it, with no bulk and no pass run, whether or not it was
    /// surveyed.
    #[tokio::test]
    async fn a_body_not_modelled_answers_not_modelled_with_no_bulk() {
        let harness = Harness::start(Handlers).await;
        let created = super::super::universe::create(
            Arc::clone(harness.state()),
            CreateUniverseRequest {
                name: "Unmodelled".to_owned(),
                seed: Some(hyperion_protocol::SeedHex::from_u64(SEED)),
            },
        )
        .await
        .unwrap();
        let ResponseBody::CreateUniverse(info) = created else {
            panic!("create_universe answers with the universe: {created:?}");
        };
        let solid = bodies().solid;
        let not_modelled = Ok(Answer {
            body: ResponseBody::SurfaceField(SurfaceFieldDto::NotModelled {
                section: RecordSectionDto::Surface,
            }),
            bulk: None,
        });
        assert_eq!(
            ask(&harness, field_request(&info.id, solid, None)).await,
            not_modelled
        );
        revision(survey(&harness, pass(&info.id, solid, &[(0, 1)])).await);
        assert_eq!(
            ask(&harness, field_request(&info.id, solid, Some(1))).await,
            not_modelled
        );
        assert_eq!(harness.state().surface.counters().cache().misses(), 0);
        assert_eq!(harness.state().pool.counters().queued_bulk(), 0);
        harness.stop().await;
    }

    /// A body is resolved and refused as `body_detail` refuses it, naming `body`, and a giant has
    /// no solid surface.
    #[tokio::test]
    async fn a_surface_field_of_no_solid_body_is_refused_naming_body() {
        let (harness, universe) = started(Ceres).await;
        let Bodies { solid, giant, .. } = *bodies();
        let with_body = |raw: u64, index: u16| {
            let mut request = field_request(&universe, solid, None);
            request.body = BodyIdHex::from_parts(raw, index);
            request
        };
        let system = solid.system().raw();
        let cases = [
            (field_request(&universe, giant, None), ErrorCode::BadRequest),
            (with_body(system, 0xd000), ErrorCode::BadRequest),
            (with_body(system, 0xcf00), ErrorCode::UnknownBody),
            (with_body(u64::MAX, 0x0100), ErrorCode::UnknownSystem),
        ];
        for (request, code) in cases {
            let error = ask(&harness, request.clone()).await.unwrap_err();
            assert_eq!(
                (error.code, error.field.as_deref()),
                (code, Some("body")),
                "{request:?}: {error:?}"
            );
        }
        harness.stop().await;
    }

    /// `future`'s output within [`WAIT`]: the loops that skip the scene's heartbeats would
    /// otherwise wait for ever, since each heartbeat restarts a frame's own wait.
    async fn within<T>(what: &str, future: impl Future<Output = T>) -> T {
        timeout(WAIT, future)
            .await
            .unwrap_or_else(|_| panic!("timed out waiting for {what}"))
    }

    /// A scene subscription of the universe on a fresh connection, its number.
    async fn subscribe(client: &mut Client, universe: &UniverseIdHex, id: u32) -> u32 {
        client
            .request(
                id,
                RequestBody::Subscribe(SubscribeRequest {
                    universe: universe.clone(),
                    topic: SubscriptionTopic::Scene(SceneSubscribeRequest {
                        detail: DetailLevelDto::Contact,
                        cameras: Vec::new(),
                    }),
                }),
            )
            .await;
        within("the subscription", async {
            loop {
                match client.next_message().await {
                    ServerMessage::Response {
                        id: answered,
                        body: ResponseBody::Subscribe(subscribed),
                    } if answered == RequestId(id) => return subscribed.subscription,
                    ServerMessage::Notification { .. } => {}
                    other => panic!("expected `subscribed`, got {other:?}"),
                }
            }
        })
        .await
    }

    /// The next scene notification on `subscription` that names surface revisions; heartbeats
    /// between are skipped.
    async fn revisions_notified(client: &mut Client, subscription: u32) -> SceneNotificationDto {
        within("surface revisions", async {
            loop {
                match client.next_message().await {
                    ServerMessage::Notification {
                        subscription: on,
                        body: NotificationBody::Scene(notification),
                    } if on == subscription => {
                        if notification.surface_revisions.is_some() {
                            return notification;
                        }
                    }
                    other => panic!("expected a notification on {subscription}, got {other:?}"),
                }
            }
        })
        .await
    }

    /// `surface_field` asked over `client` as request `id`: its chunks rejoined, then its answer.
    async fn ask_over(client: &mut Client, id: u32, request: SurfaceFieldRequest) -> Served {
        client.request(id, RequestBody::SurfaceField(request)).await;
        within("the surface field", async {
            let mut payload = Vec::new();
            loop {
                match client.next_frame().await {
                    Received::Binary(frame) => {
                        payload.extend_from_slice(&frame[crate::bulk::HEADER_BYTES..]);
                    }
                    Received::Message(ServerMessage::Response {
                        id: answered,
                        body:
                            ResponseBody::SurfaceField(SurfaceFieldDto::Ready {
                                revision,
                                coverage,
                                bulk,
                                ..
                            }),
                    }) if answered == RequestId(id) => {
                        assert_eq!(u64::try_from(payload.len()).unwrap(), bulk.bytes);
                        return Served {
                            revision,
                            coverage,
                            bytes: payload,
                        };
                    }
                    Received::Message(ServerMessage::Notification { .. }) => {}
                    Received::Message(other) => panic!("expected the field, got {other:?}"),
                }
            }
        })
        .await
    }

    /// A survey pass raises its body's revision on a scene subscription of the universe made
    /// before the universe's first pass: the first rise is named alone, and later ones with each
    /// body's latest revision.
    #[tokio::test]
    async fn a_survey_pass_raises_the_body_s_revision_on_the_scene_topic() {
        let (harness, universe) = started(Ceres).await;
        let Bodies {
            solid, other_solid, ..
        } = *bodies();
        let mut client = harness.connect().await;
        client.hello().await;
        let subscription = subscribe(&mut client, &universe, 1).await;
        revision(survey(&harness, pass(&universe, solid, &[(0, 8)])).await);
        let first = revisions_notified(&mut client, subscription).await;
        assert_eq!(
            first.surface_revisions,
            Some(vec![SurfaceRevisionDto {
                body: hex(solid),
                revision: 1,
            }])
        );
        revision(survey(&harness, pass(&universe, solid, &[(8, 16)])).await);
        revision(survey(&harness, pass(&universe, other_solid, &[(0, 8)])).await);
        let mut told = Vec::new();
        while told.len() < 2 {
            let notified = revisions_notified(&mut client, subscription).await;
            for revision in notified.surface_revisions.unwrap() {
                told.retain(|held: &SurfaceRevisionDto| held.body != revision.body);
                told.push(revision);
            }
        }
        told.sort_by(|a, b| a.body.as_str().cmp(b.body.as_str()));
        let mut expected = vec![
            SurfaceRevisionDto {
                body: hex(solid),
                revision: 2,
            },
            SurfaceRevisionDto {
                body: hex(other_solid),
                revision: 1,
            },
        ];
        expected.sort_by(|a, b| a.body.as_str().cmp(b.body.as_str()));
        assert_eq!(told, expected);
        client.close().await;
        harness.stop().await;
    }

    /// A client that unsubscribes is told nothing while it is away, and on subscribing again asks
    /// with the revision it holds: the answer is the cells surveyed while it was away, over the
    /// socket as a bulk payload.
    #[tokio::test]
    async fn a_resubscribed_client_s_have_revision_returns_the_cells_surveyed_while_away() {
        let (harness, universe) = started(Ceres).await;
        let solid = bodies().solid;
        let level = ceres_level();
        let code = ResolutionCode::from_resolution(Metres::new(120.0)).unwrap();
        let mut client = harness.connect().await;
        client.hello().await;
        let subscription = subscribe(&mut client, &universe, 1).await;
        revision(survey(&harness, pass(&universe, solid, &[(0, 64)])).await);
        let told = revisions_notified(&mut client, subscription).await;
        let held = told.surface_revisions.unwrap()[0].revision;
        let first = ask_over(&mut client, 2, field_request(&universe, solid, None)).await;
        assert_eq!((held, first.revision), (1, 1));

        client
            .request(
                3,
                RequestBody::Unsubscribe(UnsubscribeRequest { subscription }),
            )
            .await;
        within("the unsubscription", async {
            loop {
                match client.next_message().await {
                    ServerMessage::Response {
                        id: RequestId(3),
                        body: ResponseBody::Unsubscribe,
                    } => break,
                    ServerMessage::Notification { .. } => {}
                    other => panic!("expected the unsubscription, got {other:?}"),
                }
            }
        })
        .await;
        assert_eq!(
            revision(survey(&harness, pass(&universe, solid, &[(4_000, 4_032)])).await),
            2
        );
        let between = client.ping(4).await;
        assert!(
            between
                .iter()
                .all(|message| !matches!(message, ServerMessage::Notification { .. })),
            "nothing is told while unsubscribed: {between:?}"
        );

        let again = subscribe(&mut client, &universe, 5).await;
        assert_ne!(again, subscription);
        let away = ask_over(&mut client, 6, field_request(&universe, solid, Some(held))).await;
        assert_eq!(away.revision, 2);
        let before = cover_of(&[(0, 64)], code);
        let now = cover_of(&[(0, 64), (4_000, 4_032)], code);
        assert_eq!(cells(&away.coverage), delta(level, &now, &before));
        assert!(
            cells(&away.coverage)
                .iter()
                .any(|&(cell, resolution)| cell == 4_000 && resolution == code.get())
        );
        let blocks = decode_payload(&away.bytes).unwrap();
        assert!(
            blocks[0].header().is_some(),
            "a delta's block 0 carries the header"
        );

        // The rise made while it was away is not replayed: the next notification names only the
        // body surveyed since it subscribed again.
        let other = bodies().other_solid;
        revision(survey(&harness, pass(&universe, other, &[(0, 8)])).await);
        assert_eq!(
            revisions_notified(&mut client, again)
                .await
                .surface_revisions,
            Some(vec![SurfaceRevisionDto {
                body: hex(other),
                revision: 1,
            }])
        );
        client.close().await;
        harness.stop().await;
    }
}
