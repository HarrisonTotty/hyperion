//! The sky's handler (rendering plan R06, R06.T11.a–d; Design notes 5 and 9–17): `sky`, every star
//! brighter than a view's limit as seen from a point at a time, the light of the rest as a band, the
//! eye's limit in every direction, and the observer's own stars as discs, nearest first.
//!
//! The request is checked field by field, as every handler's is: the universe, then `time`,
//! `observer`, `eye`, `camera_limit_v`, `n_max`, `cone` and `exclude_system`, whose ID plan 03's
//! `resolve` looks up in a pool job, as the scene's frames are. The request's illumination comes
//! first (R06.T9.g), then the eye's cut, the eye cut's pre-pass with it, and the request's cut is
//! the deeper of it and the camera's limit, which is taken as asked, neither deepened nor padded;
//! an eye-only request also takes the eye's visibility, by which its caps count each ray (R06.T7.b;
//! `decision-r06-t7b-brackets.md`). Everything the sky computes runs as bulk jobs on the CPU pool,
//! never in the interactive queue ([`compute::sky`](crate::compute::sky)): the galaxy's tables,
//! built once and kept; the illumination; the eye's cut and visibility; the caps' rays, a few to a
//! job, and the census's plan; the host discs of `exclude_system` at the request's time; the census
//! itself, shell by shell nearest first, in jobs of about 50 ms over the server's cell cache,
//! while the band's rays are marched a face's two rows to a job, once for every reply; and after
//! each step of shells its merge, the band's sums and the eye's limits a job a march, the eye
//! offsets, and the payload (R06.T11.d).
//!
//! Each step's answer is the census's JSON (the cut, each layer's cap, tallies and the radius it is
//! complete to, the listed and overflow counts, `valid_until`, the band's shape, the host discs,
//! what is not modelled, and whether it is final) and its bulk payload, in R03's binary frames
//! before it (R06.T11.b): each listed star in the census's order in its wire form
//! ([`wire_star`]), then the band's texels in the cube's face order ([`wire_texel`]), the
//! response's `stars_bytes` and `band_bytes` splitting it and its manifest stating the whole. Every
//! answer but the last is a `partial_response`, sent through the request's [`Replies`] as soon as
//! it is made; the last, final, is the terminal `response`. The band's shape is the query's, 64² a
//! face.
//!
//! The server answers `sky` only behind its landing switch, `--serve-sky` (R06.T11.c); off, as it
//! is by default until R06.T8.g, the request never reaches this handler.

use std::fmt;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Instant;

use futures_util::TryFutureExt;
use futures_util::future::try_join;
use hyperion_protocol::{
    BandSpecDto, ConeDto, ErrorCode, EyeDto, GalacticPosition, HostDiscDto, MAX_CUT_V,
    MAX_SKY_STARS, PowerTwoDto, RequestError, ResponseBody, SkyGapDto, SkyLayerCensusDto,
    SkyRequest, SkyResponse, SystemIdHex, UniverseIdHex,
};
use hyperion_sim::coords::UnitVector;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::placement::{ResolveSystemError, resolve};
use hyperion_sim::galaxy::query::pad_speed;
use hyperion_sim::id::{Layer, SystemId};
use hyperion_sim::observe::Observer;
use hyperion_sim::sky::EyeObserver;
use hyperion_sim::sky::band::{BandMarch, BandTexel};
use hyperion_sim::sky::caps::LayerCap;
use hyperion_sim::sky::census::{
    BuildSkyQueryError, CensusPlan, CensusTallies, Completeness, Cone, LayerTally, SkyCensus,
    SkyQuery, SkyStar,
};
use hyperion_sim::sky::dgl::Illumination;
use hyperion_sim::sky::disc::{HostDisc, host_discs};
use hyperion_sim::sky::eye::{BuildEyeError, SkyBackground, SpRatio, star_colour_offset};
use hyperion_sim::sky::limits::EyeVisibility;
use hyperion_sim::tables::star_colour::LUMINANCE_RGB;
use hyperion_sim::time::{Span, UniverseTime};
use hyperion_sim::units::consts::{
    METRES_PER_LIGHT_YEAR, RADIANS_PER_DEGREE, SECONDS_PER_JULIAN_YEAR,
};
use hyperion_sim::units::{CandelasPerSquareMetre, Degrees, LightYears, Magnitudes};
use tokio::sync::mpsc;

use super::universe::openable_universe;
use super::{Replies, request_error};
use crate::AppState;
use crate::bulk::sky::{EncodedSky, SkyStarWire, SkyTexelWire, encode_sky_payload};
use crate::bulk::{Answer, BulkPayload};
use crate::compute::sky::{CENSUS_JOB_SLICE, CensusInputs, CensusStep, SkyBand};
use crate::compute::{self, CancelToken, ComputeError, GalaxyKey, JobError, Priority, SkyCaps};
use crate::convert::{ConvertRequestError, mass_layer, query_time, root_cube_position, wire_time};

/// The longest a sky holds before it is asked again, s: one Julian year (Design note 13).
const VALID_FOR_S: f64 = SECONDS_PER_JULIAN_YEAR;

/// How far a listed star within [`NEAR_STAR_LY`] may move across the sky before the sky is asked
/// again, rad: a tenth of a pixel at 1080p across 60° (Design note 13).
const TENTH_OF_A_PIXEL_RAD: f64 = 0.1 * 60.0 * RADIANS_PER_DEGREE / 1_080.0;

/// How near a listed star must be for its motion to shorten a sky's life, ly (Design note 13).
const NEAR_STAR_LY: f64 = 1.0;

/// How far a cone's axis may be from unit length and still be taken as the unit vector it names.
const AXIS_LENGTH_TOLERANCE: f64 = 1e-6;

/// Every star brighter than the request's cut as seen from its observer at its time, nearest
/// first (R06.T11.a–d): after each step of the census's shells, the census so far with the host
/// discs as JSON, and its stars and band as the bulk payload before it, each but the last sent to
/// `replies` as a `partial_response`, the last, final, returned.
///
/// The tables, the illumination, the eye's cut and visibility, the caps and plan, the census and
/// the band's march, each reply's merge, the band's sums and limits, the eye offsets, the discs and
/// the payloads each run as bulk jobs under `token`, so a cancelled request's queued jobs are
/// skipped and a running census job stops at its next cell. The galaxy's tables are built once,
/// shared by every sky of the galaxy at any time. A reply waits for the connection to take the one
/// before it, and the census runs on meanwhile.
///
/// # Errors
///
/// Those of [`openable_universe`]; `bad_request` naming the first field at fault, in the order
/// `time`, `observer`, `eye.field_factor`, `eye.age_years`, `eye.pigmentation`, `camera_limit_v`
/// (also for a request that asks neither the eye nor a camera), `n_max` and `cone` (also for an
/// eye with a cone, `decision-r06-t8k-cone.md`); `unknown_system` naming `exclude_system` for an
/// ID whose bits are not a system ID or that `resolve` refuses; those of the galaxy cache;
/// `queue_full` if the interactive queue has no room for the lookup of `exclude_system`;
/// `cancelled` or `internal` from any job; and `cancelled` once the request's connection has gone.
pub(crate) async fn answer(
    state: Arc<AppState>,
    request: SkyRequest,
    token: CancelToken,
    replies: Replies,
) -> Result<Answer, RequestError> {
    let started = Instant::now();
    let universe = openable_universe(&state, &request.universe)?;
    let asked = SkyAsk::try_from(&request)?;
    let galaxy = state.galaxies.get(universe.key()).await?;
    if let (Some(system), Some(named)) = (asked.exclude, &request.exclude_system) {
        check_exclude(&state, &galaxy, system, named, token.clone()).await?;
    }
    let pool = &state.pool;
    let tables = state.sky_tables.get(universe.key(), &galaxy).await?;
    phase("tables", started);
    // The request's illumination first: the eye's cut, its visibility and every reply's band read
    // it (decision-r06-t9g-dgl.md, §3.4).
    let light = compute::sky::illumination(pool, &galaxy, &tables, asked.observer, &token).await?;
    phase("illumination", started);
    let seen = match asked.eye {
        Some(eye) => {
            let at = (asked.observer, eye);
            let cut = compute::sky::eye_cut(pool, &galaxy, &tables, at, &light, &token).await?;
            let visibility = if asked.counts_by_visibility() {
                Some(
                    compute::sky::eye_visibility(pool, &galaxy, &tables, at, cut, &light, &token)
                        .await?,
                )
            } else {
                None
            };
            Some(Seen {
                eye,
                cut,
                visibility,
            })
        }
        None => None,
    };
    phase("eye", started);
    let query = Arc::new(asked.query(seen, light, state.sky_caps));
    let edges = state.sky_caps.shell_edges_ly();
    let plan = Arc::new(compute::sky::plan(pool, &galaxy, &tables, &query, edges, &token).await?);
    phase("plan", started);
    let inputs = CensusInputs {
        galaxy,
        key: universe.key(),
        tables,
        cells: Arc::clone(&state.sky_cells),
        query: Arc::clone(&query),
    };
    // Every reply carries the discs, which read no census.
    let hosts = match asked.exclude {
        Some(system) => {
            discs(
                &state,
                universe.key(),
                &inputs.galaxy,
                system,
                &query,
                &token,
            )
            .await?
        }
        None => Vec::new(),
    };
    let steps = compute::sky::delivery_steps(&plan);
    // Every step's progress fits, so the census never waits for a reply to be made.
    let (sink, mut censused) = mpsc::channel(steps.len());
    let census =
        compute::sky::census_in_steps(pool, &inputs, &plan, &steps, CENSUS_JOB_SLICE, sink, &token)
            .map_err(RequestError::from);
    let reply = Reply {
        state: &state,
        request: &request,
        inputs: &inputs,
        plan: &plan,
        hosts: &hosts,
        token: &token,
        started,
    };
    let ((), answer) = try_join(census, reply.each(&mut censused, &replies)).await?;
    Ok(answer)
}

/// The milliseconds since `started`, for the log.
fn millis(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1e3
}

/// Logs that a sky's `step` is done, `started` being when its request began: what T17 times.
fn phase(step: &'static str, started: Instant) {
    tracing::debug!(step, elapsed_ms = millis(started), "sky phase done");
}

/// What every reply of one sky is made from.
struct Reply<'a> {
    state: &'a AppState,
    request: &'a SkyRequest,
    inputs: &'a CensusInputs,
    plan: &'a Arc<CensusPlan>,
    hosts: &'a [HostDiscDto],
    token: &'a CancelToken,
    /// When the request began, for the log's times.
    started: Instant,
}

impl Reply<'_> {
    /// The sky's band marched, then a reply after each step `censused` gives, each but the last
    /// sent to `replies`, and the last returned (R06.T11.d).
    ///
    /// # Errors
    ///
    /// Those of [`Reply::of`] and of [`Replies::send`], and `internal` if the census ends before
    /// its last step.
    async fn each(
        &self,
        censused: &mut mpsc::Receiver<CensusStep>,
        replies: &Replies,
    ) -> Result<Answer, RequestError> {
        let pool = &self.state.pool;
        // The band's rays read no census, so they are marched while it runs, once for every reply
        // the plan's shells can state (R06.T9.f).
        let marches =
            compute::sky::march(pool, self.inputs, self.plan.replies(), self.token).await?;
        phase("march", self.started);
        while let Some(step) = censused.recv().await {
            let last = step.last;
            let answer = self.of(step, &marches).await?;
            if last {
                return Ok(answer);
            }
            replies.send(pool, self.token, answer).await?;
        }
        Err(request_error(
            ErrorCode::Internal,
            "the sky's census ended before its last step",
        ))
    }

    /// The sky's answer after `step` (R06.T11.d): the census of the shells done, its band summed
    /// from the request's one march at that census's radii, with the eye's limits and offsets
    /// where the eye is asked, and the payload, each as bulk jobs.
    ///
    /// # Errors
    ///
    /// Those of [`compute::sky::bulk`] and the band's jobs.
    async fn of(
        &self,
        step: CensusStep,
        marches: &[Arc<BandMarch>],
    ) -> Result<Answer, ComputeError> {
        let (pool, query, last, worked) =
            (&self.state.pool, &self.inputs.query, step.last, step.worked);
        let census =
            compute::sky::merge_step(pool, self.plan, step, query.n_max(), self.token).await?;
        let census = Arc::new(census);
        let completeness = census
            .completeness()
            .expect("a step's census is a census of shells");
        tracing::debug!(
            cut = query.cut().value(),
            listed = census.listed().len(),
            overflow = census.overflow().len(),
            least_edge_ly = completeness.least_edge().map(LightYears::value),
            last,
            census_job_s = worked.as_secs_f64(),
            elapsed_ms = millis(self.started),
            "sky censused to a step"
        );
        debug_assert_eq!(completeness.is_final(), last, "the last step is final");
        let band = compute::sky::band(pool, query, marches, &census, self.token).await?;
        let (observer, listed) = (*query.observer(), Arc::clone(&census));
        let encoded = compute::sky::bulk(pool, self.token, move |_: &CancelToken| {
            payload(&observer, &listed, &band)
        })
        .await?;
        phase("reply", self.started);
        let bulk = BulkPayload::new(encoded.bytes.clone()).expect(
            "a sky's payload, at most N_max's 3 × 10⁵ stars and six faces of texels (7.5 MB), has \
             far fewer chunks than a u32 counts",
        );
        let body = response(
            (self.request.universe.clone(), self.request.observer),
            query,
            self.plan.caps(),
            &census,
            self.hosts.to_vec(),
            &bulk,
            &encoded,
        );
        Ok(Answer {
            body: ResponseBody::Sky(Box::new(body)),
            bulk: Some(bulk),
        })
    }
}

/// The sky's bulk payload (Design note 17): each listed star of `census` as the wire carries it,
/// seen from `observer`, in the census's order, with its eye offset from `band`, then `band`'s
/// texels in the cube's face order.
///
/// Where the eye was not asked, a star's eye offset is its colour offset alone, against a scotopic
/// background ([`scotopic_colour_offset`]), and every texel's eye limit is the wire's "not asked".
/// `band` is of `census`, so it holds an eye offset for each of its listed stars where the eye was
/// asked.
#[must_use]
fn payload(observer: &Observer, census: &SkyCensus, band: &SkyBand) -> EncodedSky {
    let listed = census.listed();
    let stars: Vec<SkyStarWire> = match band.eye_offsets() {
        Some(offsets) => {
            debug_assert_eq!(offsets.len(), listed.len(), "an eye offset per listed star");
            listed
                .iter()
                .zip(offsets)
                .map(|(star, &offset)| wire_star(observer, star, offset))
                .collect()
        }
        None => listed
            .iter()
            .map(|star| {
                let ratio = star.colour().reddened(star.a_v()).sp_ratio();
                wire_star(observer, star, scotopic_colour_offset(ratio))
            })
            .collect(),
    };
    let texels: Vec<SkyTexelWire> = band.texels().iter().map(wire_texel).collect();
    encode_sky_payload(&stars, &texels)
}

/// The discs of `system`'s stars at the query's time (Design note 16; R06.T11.c), in one bulk job
/// under `token`: the system's stars from the server's system cache, generated if they are not
/// held, then `host_discs`.
///
/// A system whose stars are not generated, a centre member until plan 09's system stage or a rogue
/// planet, which has none, has no disc to draw: the reply's hosts are then empty.
///
/// # Errors
///
/// Those of [`compute::sky::bulk`], whose job panics if `system` names no system of the galaxy,
/// which [`check_exclude`] has ruled out: it is answered `internal`.
async fn discs(
    state: &Arc<AppState>,
    key: GalaxyKey,
    galaxy: &Arc<Galaxy>,
    system: SystemId,
    query: &SkyQuery,
    token: &CancelToken,
) -> Result<Vec<HostDiscDto>, ComputeError> {
    let (shared, galaxy, t) = (
        Arc::clone(state),
        Arc::clone(galaxy),
        query.observer().time(),
    );
    compute::sky::bulk(&state.pool, token, move |_: &CancelToken| {
        match shared.systems.get_or_generate(key, &galaxy, system) {
            Ok(stars) => host_discs(&galaxy, &stars, t)
                .iter()
                .map(host_disc)
                .collect(),
            // A system the stellar stage does not generate has no star to draw as a disc.
            Err(
                ResolveSystemError::KindNotGenerated | ResolveSystemError::LayerNotGenerated(_),
            ) => Vec::new(),
            Err(ResolveSystemError::NoSuchSystem) => {
                panic!("the excluded system {system} was resolved before the sky's jobs")
            }
        }
    })
    .await
}

/// A host star's disc as the wire carries it (Design note 16): every array in B, V, R order.
#[must_use]
fn host_disc(disc: &HostDisc) -> HostDiscDto {
    let colour = disc.colour();
    HostDiscDto {
        star: disc.star().get(),
        radius_m: disc.radius().value(),
        teff_k: disc.teff().value(),
        log_g: disc.log_g(),
        mean_luminance_cd_m2: disc.mean_luminance().map(CandelasPerSquareMetre::value),
        central_luminance_cd_m2: disc.central_luminance().map(CandelasPerSquareMetre::value),
        limb: disc.limb().map(|law| PowerTwoDto {
            c: law.c(),
            alpha: law.alpha(),
        }),
        chroma: colour.chroma(),
        lux_per_v0: colour.lux_per_v0(),
        bake_spectrum: colour.bake_spectrum(),
    }
}

/// A `sky` request, checked: the observer, what is asked of the eye and the camera, the count
/// budget, the cone and the system to leave out.
#[derive(Debug, Clone, PartialEq)]
struct SkyAsk {
    observer: Observer,
    eye: Option<EyeObserver>,
    camera_limit: Option<Magnitudes>,
    n_max: NonZeroU32,
    cone: Option<Cone>,
    exclude: Option<SystemId>,
}

impl TryFrom<&SkyRequest> for SkyAsk {
    type Error = RequestError;

    /// Checks `time`, `observer`, `eye`, `camera_limit_v`, `n_max`, `cone` and `exclude_system`, in
    /// that order. Whether `exclude_system` names a system of the galaxy is looked up after this.
    fn try_from(request: &SkyRequest) -> Result<Self, Self::Error> {
        let time = query_time(&request.time)?;
        let position = root_cube_position("observer", &request.observer)?;
        let observer = Observer::new(position, time)
            .expect("a position in the root cube, seen at a time in the clock window");
        let eye = request.eye.as_ref().map(eye_observer).transpose()?;
        let camera_limit = request.camera_limit_v.map(camera_limit).transpose()?;
        if eye.is_none() && camera_limit.is_none() {
            return Err(ConvertRequestError::new(
                "camera_limit_v",
                "a sky is asked for the eye, a camera's limit or both, and this asks for neither",
            )
            .into());
        }
        let n_max = n_max(request.n_max)?;
        let cone = match &request.cone {
            Some(_) if eye.is_some() => {
                return Err(
                    ConvertRequestError::new("cone", BuildSkyQueryError::ConeWithEye).into(),
                );
            }
            Some(cone) => Some(field_stop(cone)?),
            None => None,
        };
        let exclude = request
            .exclude_system
            .as_ref()
            .map(|system| {
                SystemId::from_raw(system.to_u64()).map_err(|error| unknown_exclude(system, error))
            })
            .transpose()?;
        Ok(Self {
            observer,
            eye,
            camera_limit,
            n_max,
            cone,
            exclude,
        })
    }
}

/// The eye a request asks, as the server has seen it: its cut, the eye cut's pre-pass, and for a
/// request with no camera part its visibility (R06.T7.b).
#[derive(Debug, Clone, PartialEq)]
struct Seen {
    eye: EyeObserver,
    cut: Magnitudes,
    visibility: Option<EyeVisibility>,
}

impl SkyAsk {
    /// Whether the census's caps count each ray to the eye's own limit about it, its visibility
    /// (R06.T7.b): for a request of the eye with no camera part alone, never by comparing cuts. A
    /// camera's cull is its own, and at 120° shallower than the eye's cut, so the eye's visibility
    /// caps would not cover it (decided 2026-10-08, `decision-r06-t7b-brackets.md`).
    #[must_use]
    fn counts_by_visibility(&self) -> bool {
        self.eye.is_some() && self.camera_limit.is_none()
    }

    /// The census's query, once the eye's cut is known: to the deeper of the eye's cut and the
    /// camera's limit, as asked, with the eye at its own cut if a camera's is deeper (R06.T9.j),
    /// its caps counted by the eye's visibility where `seen` has it, the request's `illumination`
    /// (R06.T9.g), and every cap forced where `caps` forces them. `seen` is the request's eye with
    /// its cut, or `None` where the request does not ask the eye.
    ///
    /// # Panics
    ///
    /// If `seen` is `None` and the request asks no camera's limit either. A checked request asks
    /// the eye, a camera or both, and the handler passes the eye with its cut whenever the request
    /// asks it, and its visibility and illumination for the request's observer.
    #[must_use]
    fn query(
        &self,
        seen: Option<Seen>,
        illumination: Arc<Illumination>,
        caps: SkyCaps,
    ) -> SkyQuery {
        let cut = match (seen.as_ref().map(|seen| seen.cut), self.camera_limit) {
            (Some(eye_cut), Some(camera)) => {
                if camera.value() > eye_cut.value() {
                    camera
                } else {
                    eye_cut
                }
            }
            (Some(cut), None) | (None, Some(cut)) => cut,
            (None, None) => panic!("a sky's query is asked for the eye, a camera or both"),
        };
        let mut builder = SkyQuery::builder(self.observer, cut).n_max(self.n_max);
        if let Some(Seen {
            eye,
            cut,
            visibility,
        }) = seen
        {
            builder = builder.eye(eye).eye_cut(cut);
            if let Some(visibility) = visibility {
                builder = builder.eye_visibility(visibility);
            }
        }
        builder = builder.illumination(illumination);
        if let Some(cone) = self.cone {
            builder = builder.cone(cone);
        }
        if let Some(system) = self.exclude {
            builder = builder.exclude(system);
        }
        let query = builder.build().expect(
            "a checked request's cut, n_max and cone build, the eye's cut is the cut or under it, \
             and its visibility and illumination are the request's observer's",
        );
        caps.on(query)
    }
}

/// The eye of `eye`, refused naming the field at fault.
fn eye_observer(eye: &EyeDto) -> Result<EyeObserver, ConvertRequestError> {
    EyeObserver::new(eye.field_factor, eye.age_years, eye.pigmentation).map_err(|error| {
        let field = match error {
            BuildEyeError::FieldFactor => "eye.field_factor",
            BuildEyeError::AgeYears => "eye.age_years",
            BuildEyeError::Pigmentation => "eye.pigmentation",
            BuildEyeError::SpRatio | BuildEyeError::Luminance | BuildEyeError::PhotopicWeight => {
                unreachable!("an eye's constructor checks its own three parameters only: {error}")
            }
        };
        ConvertRequestError::new(field, error)
    })
}

/// The camera's limit, V: finite and at most [`MAX_CUT_V`] (Design note 5).
fn camera_limit(v: f64) -> Result<Magnitudes, ConvertRequestError> {
    if v.is_finite() && v <= MAX_CUT_V {
        Ok(Magnitudes::new(v))
    } else {
        Err(ConvertRequestError::new(
            "camera_limit_v",
            format!("{v} is not a finite V of at most {MAX_CUT_V}"),
        ))
    }
}

/// The most stars to list: from 1 to [`MAX_SKY_STARS`], which is also the default (Design note 11).
fn n_max(asked: Option<u32>) -> Result<NonZeroU32, ConvertRequestError> {
    let n = asked.unwrap_or(MAX_SKY_STARS);
    NonZeroU32::new(n)
        .filter(|n| n.get() <= MAX_SKY_STARS)
        .ok_or_else(|| {
            ConvertRequestError::new("n_max", format!("{n} is not within 1 to {MAX_SKY_STARS}"))
        })
}

/// The cone of `cone`, an instrument's field stop (Design note 5): an axis of unit length and a
/// half-angle within (0°, 90°].
fn field_stop(cone: &ConeDto) -> Result<Cone, ConvertRequestError> {
    let [x, y, z] = cone.axis;
    let length = (x * x + y * y + z * z).sqrt();
    let axis = UnitVector::from_components(cone.axis)
        .filter(|_| (length - 1.0).abs() <= AXIS_LENGTH_TOLERANCE)
        .ok_or_else(|| {
            ConvertRequestError::new(
                "cone",
                format!("the axis {:?} is not a unit vector", cone.axis),
            )
        })?;
    Cone::new(axis, Degrees::new(cone.half_angle_deg))
        .map_err(|error| ConvertRequestError::new("cone", error))
}

/// The refusal of an `exclude_system` that names no system of the universe: `unknown_system`,
/// naming the field.
#[must_use]
fn unknown_exclude(system: &SystemIdHex, reason: impl fmt::Display) -> RequestError {
    RequestError {
        code: ErrorCode::UnknownSystem,
        message: format!(
            "exclude_system {} names no system of this universe: {reason}",
            system.as_str()
        ),
        field: Some("exclude_system".to_owned()),
    }
}

/// Looks `system` up in `galaxy` with plan 03's `resolve`, as plan 04 requires of every ID read
/// from a client, in an interactive pool job, as the scene resolves its frames.
async fn check_exclude(
    state: &AppState,
    galaxy: &Arc<Galaxy>,
    system: SystemId,
    named: &SystemIdHex,
    token: CancelToken,
) -> Result<(), RequestError> {
    let galaxy = Arc::clone(galaxy);
    let check = move |_: &CancelToken| resolve(&galaxy, system).map(|_| ());
    let receiver = state.pool.try_submit(Priority::Interactive, token, check)?;
    receiver
        .await
        .unwrap_or_else(|closed| Err(JobError::from(closed)))?
        .map_err(|error| unknown_exclude(named, error))
}

/// The census's answer (R06.T11.a–d) for the universe and observer `asked`: the cut, each layer's
/// cap, tallies and the radii it is complete to, the counts, the time it holds to, the band's
/// shape, the host discs `hosts`, what is not modelled and whether it is final, with the manifest
/// of `bulk` and where `encoded` splits.
///
/// # Panics
///
/// If `census` is not a census of shells, which every reply's is ([`compute::sky::merge_step`]).
#[must_use]
fn response(
    asked: (UniverseIdHex, GalacticPosition),
    query: &SkyQuery,
    caps: &[LayerCap],
    census: &SkyCensus,
    hosts: Vec<HostDiscDto>,
    bulk: &BulkPayload,
    encoded: &EncodedSky,
) -> SkyResponse {
    let tallies = census.tallies();
    let completeness = census
        .completeness()
        .expect("a reply's census is a census of shells");
    let (universe, observer) = asked;
    SkyResponse {
        universe,
        time: wire_time(query.observer().time()),
        observer,
        valid_until: wire_time(valid_until(query.observer().time(), census.listed())),
        cut_v: query.cut().value(),
        census: caps
            .iter()
            .map(|cap| layer_census(cap, tallies, completeness))
            .collect(),
        listed: narrow(census.listed().len()),
        overflow: narrow(census.overflow().len()),
        band: BandSpecDto {
            face_texels: query.band_spec().face_texels(),
        },
        hosts,
        not_modelled: not_modelled(tallies),
        bulk: bulk.manifest(),
        stars_bytes: encoded.stars_bytes,
        band_bytes: encoded.band_bytes,
        is_final: completeness.is_final(),
    }
}

/// One layer's census as the wire states it (R06's Risks, T8.c's mapping): its cap, its tallies,
/// each count narrowed to the wire's width, and how far `completeness` says it is complete
/// (R06.T11.d): its farthest radius, its radius per ray of the caps' lattice where it has one a
/// ray, and whether its last shell is done.
#[must_use]
fn layer_census(
    cap: &LayerCap,
    tallies: &CensusTallies,
    completeness: &Completeness,
) -> SkyLayerCensusDto {
    let (layer, complete_to) = (cap.layer(), completeness.complete_to());
    let tally = tallies.layer(layer);
    SkyLayerCensusDto {
        layer: mass_layer(layer),
        cap_ly: cap.radius().value(),
        rule_bound_ly: cap.rule_bound().value(),
        expected_beyond: cap.expected_beyond(),
        cells: narrow(tally.cells()),
        candidates_opened: tally.generated(),
        accepted: tally.accepted(),
        listed: narrow(tally.listed()),
        without_photometry: narrow(tally.without_photometry()),
        feature_members_absent: tallies.feature_members_absent(),
        complete_to_ly: complete_to.radius(layer).value(),
        complete_to_rays_ly: complete_to
            .rays_ly(layer)
            .map_or_else(Vec::new, <[f64]>::to_vec),
        is_final: completeness.edge(layer).is_none(),
    }
}

/// A count as the wire's `u32` carries it, held at its largest.
#[must_use]
fn narrow(count: impl TryInto<u32>) -> u32 {
    count.try_into().unwrap_or(u32::MAX)
}

/// What the sky leaves out, each a label on the view (Design note 23; T8.c's mapping and
/// `decision-r06-t16a-scope.md`): the feature members whenever the census lacks them, which is
/// every census until R06.T16.a; the galactic centre's members where a census met one; and the
/// white dwarfs where a generated system held one.
#[must_use]
fn not_modelled(tallies: &CensusTallies) -> Vec<SkyGapDto> {
    let any = |count: fn(&LayerTally) -> u64| {
        Layer::ALL
            .iter()
            .any(|&layer| count(tallies.layer(layer)) > 0)
    };
    let mut gaps = Vec::with_capacity(3);
    if tallies.feature_members_absent() {
        gaps.push(SkyGapDto::FeatureMembers);
    }
    if any(LayerTally::centre_members) {
        gaps.push(SkyGapDto::CentreMembers);
    }
    if any(LayerTally::without_photometry) {
        gaps.push(SkyGapDto::WhiteDwarfs);
    }
    gaps
}

/// The time after which a sky seen at `t` should be asked again (Design note 13): a Julian year
/// on, or sooner if a listed star within [`NEAR_STAR_LY`] could move a tenth of a pixel at 1080p
/// across 60° before then.
///
/// A star's own velocity is not in the census, so each is taken at its layer's pad speed, which
/// every grid record of the layer moves below (plan 03, `pad_speed`): the sky may be asked again
/// early, never late.
#[must_use]
fn valid_until(t: UniverseTime, listed: &[SkyStar]) -> UniverseTime {
    valid_until_of(t, listed.iter().map(|star| (star.distance(), star.layer())))
}

/// [`valid_until`] of stars at each `(distance, layer)`.
#[must_use]
fn valid_until_of(
    t: UniverseTime,
    stars: impl IntoIterator<Item = (LightYears, Layer)>,
) -> UniverseTime {
    let seconds = stars
        .into_iter()
        .filter(|(distance, _)| distance.value() < NEAR_STAR_LY)
        .map(|(distance, layer)| {
            let metres_per_second = pad_speed(layer).value() * 1_000.0;
            TENTH_OF_A_PIXEL_RAD * distance.value() * METRES_PER_LIGHT_YEAR / metres_per_second
        })
        .fold(VALID_FOR_S, f64::min);
    let span = Span::from_seconds_f64(seconds).expect("a year or less is a span");
    t.checked_add(span)
        .expect("a time in the clock window a year on is on the clock")
}

/// A listed star as the wire carries it (Design note 17; R06.T11.a, sent from T11.b), with
/// `eye_offset`.
///
/// It holds its unit direction from `observer` and its distance, its own V, M<sub>V</sub> + DM +
/// v★(A<sub>V</sub>) A<sub>V</sub>, which the census cuts, and its chroma and camera band term after
/// its own reddening, [`StarColour::reddened`] at its A<sub>V</sub> (R06.T9.e), the camera term
/// relative to that V.
///
/// Its eye offset is `sky::limits::eye_offsets`', its own eye limit less its texel's (R06.T9.h;
/// decided 2026-10-06, `decision-r06-t9c-glare.md`): its colour offset at its reddened S/P ratio
/// against its texel's background, and its glare's self-exclusion. Where the eye was not asked,
/// there is no limit map, and it is the colour offset alone against a scotopic background,
/// [`scotopic_colour_offset`]. A star at the observer's own position, which no census lists, would
/// be given no direction.
///
/// [`StarColour::reddened`]: hyperion_sim::sky::colour::StarColour::reddened
#[must_use]
pub(crate) fn wire_star(
    observer: &Observer,
    star: &SkyStar,
    eye_offset: Magnitudes,
) -> SkyStarWire {
    let reddened = star.colour().reddened(star.a_v());
    let toward = observer
        .position()
        .displacement_to(star.apparent())
        .metres();
    let direction = UnitVector::from_components(toward).map_or([0.0; 3], |unit| unit.components());
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the wire carries the direction and the distance as f32 (Design note 17): 0.012″ \
                  of rounding, and distances under the root cube's diagonal"
    )]
    let (direction, distance_ly) = (
        direction.map(|component| component as f32),
        star.distance().value() as f32,
    );
    SkyStarWire {
        direction,
        distance_ly,
        v_mag: star.v().value(),
        chroma: chromaticity(reddened.red_green()),
        eye_offset_mag: eye_offset.value(),
        camera_band_mag: reddened.camera_band_mag(),
    }
}

/// A band texel as the wire carries it (Design note 17; R06.T11.c): its luminance as `f32`, its
/// chromaticity ([`chromaticity`] of its linear Rec. 709 red and green at unit luminance), its eye
/// limit where the eye was asked, and its S/P ratio.
#[must_use]
pub(crate) fn wire_texel(texel: &BandTexel) -> SkyTexelWire {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the wire carries the luminance as f32 (Design note 17), and a sky's lies far \
                  inside f32's range"
    )]
    let luminance_cd_m2 = texel.luminance().value() as f32;
    SkyTexelWire {
        luminance_cd_m2,
        chroma: chromaticity(texel.chroma().map(f64::from)),
        eye_limit_mag: texel.eye_limit().map(Magnitudes::value),
        sp_ratio: texel.sp_ratio(),
    }
}

/// The eye colour offset of light of S/P ratio `sp_ratio` against a background of no light,
/// where every offset is its scotopic 2.5 log₁₀(ρ★ ÷ 2.297) (Crumey 2014, eq. 15;
/// `sky::eye::star_colour_offset`): a star's eye offset where the eye was not asked. The ratio is
/// held within [`SpRatio`]'s 0.01–100, which no starlight leaves.
#[must_use]
fn scotopic_colour_offset(sp_ratio: f64) -> Magnitudes {
    let ratio = SpRatio::new(sp_ratio.clamp(SpRatio::MIN.value(), SpRatio::MAX.value()))
        .expect("a finite ratio held within the accepted range");
    let dark = SkyBackground::new(CandelasPerSquareMetre::ZERO, SpRatio::REFERENCE)
        .expect("no light is a background");
    star_colour_offset(ratio, &dark)
}

/// The wire's chroma of light whose linear Rec. 709 red and green at unit luminance are
/// `red_green`: its chromaticity, r ÷ (r + g + b) and g ÷ (r + g + b), each in [0, 1] (R06's
/// Risks, "Deviations in T10, as built"). Blue is the luminance's remainder,
/// (1 − Y<sub>r</sub> r − Y<sub>g</sub> g) ÷ Y<sub>b</sub> (ITU-R BT.709-6's Y row). Light of no
/// channel sum, which no colour in gamut has, is white's.
#[must_use]
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

#[cfg(test)]
mod tests {
    use hyperion_protocol::{CreateUniverseRequest, GalacticPosition, UniverseIdHex};
    use hyperion_sim::Seed;
    use hyperion_sim::galaxy::placement::CellKey;
    use hyperion_sim::sky::MAX_CUT_V as SIM_MAX_CUT_V;
    use hyperion_sim::sky::REFERENCE_SP_RATIO;
    use hyperion_sim::sky::band::{BandSpec, CompleteTo};
    use hyperion_sim::sky::census::{MAX_N_MAX, merge_census};
    use tokio::time::timeout;

    use super::*;
    use crate::compute::sky::SkyTables;
    use crate::requests::Handlers;
    use crate::testing::{Harness, WAIT};
    use hyperion_sim::galaxy::placement::{SystemRecord, generate_cell};
    use hyperion_sim::sky::luminosity::LuminosityTables;
    use hyperion_sim::stellar::system::SystemStars;

    /// The seed of the galaxy these tests look at the sky of.
    const SEED: u64 = 0x4d2;

    /// Near the Sun's place on the solar circle, ly, as the sim's sky tests stand.
    const SUN_LY: [i32; 3] = [0, 26_000, 68];

    fn request() -> SkyRequest {
        SkyRequest {
            universe: UniverseIdHex::from_u64(42),
            observer: GalacticPosition {
                cell_ly: SUN_LY,
                offset_m: [0.0; 3],
            },
            time: hyperion_protocol::UniverseTime {
                seconds: 0,
                nanos: 0,
            },
            eye: None,
            camera_limit_v: Some(9.0),
            n_max: None,
            cone: None,
            exclude_system: None,
        }
    }

    fn eye() -> EyeDto {
        EyeDto {
            field_factor: 1.4,
            age_years: 25.0,
            pigmentation: 0.5,
        }
    }

    /// The illumination at [`request`]'s observer over tables that hold no star, as a sky's query
    /// states it (R06.T9.g), marched once for the tests.
    fn light() -> Arc<Illumination> {
        static LIGHT: std::sync::OnceLock<Arc<Illumination>> = std::sync::OnceLock::new();
        Arc::clone(LIGHT.get_or_init(|| {
            let galaxy = Galaxy::new(Seed::new(SEED));
            let tables = SkyTables::new(LuminosityTables::dark(&galaxy), &galaxy);
            let observer = SkyAsk::try_from(&request()).unwrap().observer;
            Arc::new(Illumination::march(
                &galaxy,
                &mut tables.march_context(),
                &observer,
            ))
        }))
    }

    fn cone(axis: [f64; 3], half_angle_deg: f64) -> ConeDto {
        ConeDto {
            axis,
            half_angle_deg,
        }
    }

    /// The field a refused request names, and its code.
    fn refused(request: &SkyRequest) -> (ErrorCode, Option<String>) {
        let error = SkyAsk::try_from(request).expect_err("refused");
        (error.code, error.field)
    }

    fn bad(field: &str) -> (ErrorCode, Option<String>) {
        (ErrorCode::BadRequest, Some(field.to_owned()))
    }

    #[test]
    fn the_protocols_limits_are_the_sims() {
        assert_eq!(MAX_CUT_V.to_bits(), SIM_MAX_CUT_V.to_bits());
        assert_eq!(MAX_SKY_STARS, MAX_N_MAX);
        assert_eq!(BandSpec::STANDARD.face_texels(), 64);
    }

    #[test]
    fn every_refusal_names_its_field() {
        let with = |change: fn(&mut SkyRequest)| {
            let mut request = request();
            change(&mut request);
            refused(&request)
        };
        assert!(SkyAsk::try_from(&request()).is_ok());
        assert_eq!(with(|r| r.time.seconds = 40_000_000_000), bad("time"));
        assert_eq!(
            with(|r| r.observer.cell_ly = [0, 70_000, 0]),
            bad("observer"),
            "outside the root cube"
        );
        assert_eq!(
            with(|r| r.observer.offset_m = [f64::NAN, 0.0, 0.0]),
            bad("observer")
        );
        assert_eq!(
            with(|r| r.eye = Some(EyeDto {
                field_factor: 0.0,
                ..eye()
            })),
            bad("eye.field_factor")
        );
        assert_eq!(
            with(|r| r.eye = Some(EyeDto {
                age_years: -1.0,
                ..eye()
            })),
            bad("eye.age_years")
        );
        assert_eq!(
            with(|r| r.eye = Some(EyeDto {
                pigmentation: 2.0,
                ..eye()
            })),
            bad("eye.pigmentation")
        );
        for limit in [MAX_CUT_V + 0.01, f64::NAN, f64::NEG_INFINITY] {
            let mut deep = request();
            deep.camera_limit_v = Some(limit);
            assert_eq!(refused(&deep), bad("camera_limit_v"), "{limit}");
        }
        assert_eq!(
            with(|r| r.camera_limit_v = None),
            bad("camera_limit_v"),
            "neither the eye nor a camera"
        );
        assert_eq!(with(|r| r.n_max = Some(0)), bad("n_max"));
        assert_eq!(with(|r| r.n_max = Some(MAX_SKY_STARS + 1)), bad("n_max"));
        assert!(
            SkyAsk::try_from(&SkyRequest {
                n_max: Some(MAX_SKY_STARS),
                ..request()
            })
            .is_ok()
        );
        assert_eq!(
            with(|r| r.cone = Some(cone([0.0, 0.0, 2.0], 5.0))),
            bad("cone"),
            "an axis that is not a unit vector"
        );
        assert_eq!(with(|r| r.cone = Some(cone([0.0; 3], 5.0))), bad("cone"));
        assert_eq!(
            with(|r| r.cone = Some(cone([1.0, 0.0, 0.0], 0.0))),
            bad("cone")
        );
        assert_eq!(
            with(|r| r.cone = Some(cone([1.0, 0.0, 0.0], 90.5))),
            bad("cone")
        );
        assert!(
            SkyAsk::try_from(&SkyRequest {
                cone: Some(cone([0.6, 0.8, 0.0], 90.0)),
                ..request()
            })
            .is_ok()
        );
        let eye_with_cone = SkyRequest {
            eye: Some(eye()),
            cone: Some(cone([0.0, 0.0, 1.0], 2.5)),
            ..request()
        };
        let error = SkyAsk::try_from(&eye_with_cone).unwrap_err();
        assert_eq!(
            (error.code, error.field.as_deref()),
            (ErrorCode::BadRequest, Some("cone"))
        );
        assert!(
            error.message.contains("no field stop"),
            "the cone ruling's reason: {}",
            error.message
        );
    }

    /// The query's cut is the deeper of the eye's and the camera's, and the eye keeps its own cut
    /// under a camera's deeper one (R06.T9.j).
    #[test]
    fn the_cut_is_the_deeper_of_the_eyes_and_the_cameras() {
        let ask = |eye: Option<EyeDto>, camera: Option<f64>| {
            SkyAsk::try_from(&SkyRequest {
                eye,
                camera_limit_v: camera,
                ..request()
            })
            .unwrap()
        };
        let cuts = |query: &SkyQuery| (query.cut().value(), query.eye_cut().map(Magnitudes::value));
        let seen = || {
            Some(Seen {
                eye: EyeObserver::default(),
                cut: Magnitudes::new(8.5),
                visibility: None,
            })
        };
        let caps = SkyCaps::DERIVED;
        assert_eq!(
            cuts(&ask(Some(eye()), Some(10.0)).query(seen(), light(), caps)),
            (10.0, Some(8.5)),
            "a camera's deeper cut"
        );
        assert_eq!(
            cuts(&ask(Some(eye()), Some(7.0)).query(seen(), light(), caps)),
            (8.5, Some(8.5)),
            "a camera's shallower cut"
        );
        assert_eq!(
            cuts(&ask(Some(eye()), None).query(seen(), light(), caps)),
            (8.5, Some(8.5))
        );
        assert_eq!(
            cuts(&ask(None, Some(9.0)).query(None, light(), caps)),
            (9.0, None)
        );
        let ask = ask(None, Some(9.0));
        let forced = SkyCaps::forced(LightYears::new(30.0)).unwrap();
        assert_eq!(cuts(&ask.query(None, light(), forced)), (9.0, None));
        assert_eq!(ask.query(None, light(), caps).n_max().get(), MAX_SKY_STARS);
    }

    /// Only an eye-only request counts its caps by the eye's visibility, whatever the camera's
    /// limit, deeper or shallower than the eye's cut (`decision-r06-t7b-brackets.md`).
    #[test]
    fn only_a_request_with_no_camera_part_takes_the_eyes_visibility() {
        let ask = |eye: Option<EyeDto>, camera: Option<f64>| {
            SkyAsk::try_from(&SkyRequest {
                eye,
                camera_limit_v: camera,
                ..request()
            })
            .unwrap()
        };
        assert!(ask(Some(eye()), None).counts_by_visibility());
        for camera in [5.0, 9.0, MAX_CUT_V] {
            assert!(
                !ask(Some(eye()), Some(camera)).counts_by_visibility(),
                "{camera}"
            );
            assert!(!ask(None, Some(camera)).counts_by_visibility(), "{camera}");
        }
    }

    #[test]
    fn every_census_lacks_the_feature_members_until_r06_t16_a() {
        assert_eq!(
            not_modelled(&CensusTallies::default()),
            vec![SkyGapDto::FeatureMembers]
        );
        assert_eq!(
            not_modelled(SkyCensus::empty().tallies()),
            vec![SkyGapDto::FeatureMembers]
        );
    }

    #[test]
    fn a_sky_holds_a_year_unless_a_star_within_a_light_year_could_move_a_tenth_of_a_pixel() {
        let t = UniverseTime::new(3_600, 0).unwrap();
        let year = UniverseTime::new(3_600 + 31_557_600, 0).unwrap();
        assert_eq!(valid_until(t, &[]), year);
        let ly = LightYears::new;
        assert_eq!(
            valid_until_of(t, [(ly(1.0), Layer::A), (ly(40.0), Layer::E)]),
            year,
            "no star within a light-year"
        );
        let days = |stars: &[(LightYears, Layer)]| {
            let until = valid_until_of(t, stars.iter().copied());
            until.checked_since(t).unwrap().as_seconds_f64() / 86_400.0
        };
        // Half a light-year off at layer A's pad speed, 1,000 km/s: 0.1 × (60° ÷ 1,080) × 0.5 ly
        // ÷ 1,000 km/s, about 5.3 days.
        let a = days(&[(ly(0.5), Layer::A)]);
        assert!((a - 5.31).abs() < 0.01, "{a}");
        // Layer E's at 3,000 km/s, a third of that; the nearest, fastest star sets it.
        let e = days(&[
            (ly(0.5), Layer::A),
            (ly(0.5), Layer::E),
            (ly(0.9), Layer::E),
        ]);
        assert!((e - a / 3.0).abs() < 1e-6, "{e}");
    }

    #[test]
    fn chromaticity_is_the_share_of_each_channel() {
        let [r, g] = chromaticity([1.0, 1.0]);
        assert!(
            (r - 1.0 / 3.0).abs() < 1e-12 && (g - 1.0 / 3.0).abs() < 1e-12,
            "white"
        );
        // A red star's light at unit luminance: more red than green, little blue.
        let red = [1.6, 0.85];
        let [r, g] = chromaticity(red);
        let [yr, yg, yb] = LUMINANCE_RGB;
        let b = (1.0 - yr * red[0] - yg * red[1]) / yb;
        assert!((r - 1.6 / (1.6 + 0.85 + b)).abs() < 1e-12);
        assert!(r > g && g > 1.0 - r - g, "{r} {g}");
    }

    /// Each listed star's wire form takes its own V and its reddened colour (R06.T9.e and the
    /// band ruling's addendum item 4), over a small census near the Sun.
    #[test]
    fn a_wire_star_is_its_census_star_after_its_own_reddening() {
        let galaxy = Galaxy::new(Seed::new(SEED));
        let caps = SkyCaps::forced(LightYears::new(25.0)).unwrap();
        let tables = SkyTables::new(LuminosityTables::dark(&galaxy), &galaxy);
        let asked = SkyAsk::try_from(&request()).unwrap();
        let query = asked.query(None, light(), caps);
        let mut ctx = tables.march_context();
        let plan = hyperion_sim::sky::census::census_plan(
            &galaxy,
            ctx.tables,
            ctx.envelope,
            &query,
            &mut ctx.noise,
        );
        let mut stars = Vec::new();
        let mut tallies = CensusTallies::default();
        let cells: Vec<CellKey> = plan.cells().collect();
        for key in cells {
            tallies.add(&hyperion_sim::sky::census::census_cell(
                &galaxy, &mut ctx, key, &query, &mut stars,
            ));
        }
        let census = merge_census([(stars, tallies)], query.n_max());
        assert!(census.listed().len() > 10, "{}", census.listed().len());
        let observer = query.observer();
        for star in census.listed() {
            let reddened = star.colour().reddened(star.a_v());
            // A sky without the eye takes each star's colour offset alone, scotopic.
            let wire = wire_star(observer, star, scotopic_colour_offset(reddened.sp_ratio()));
            assert_eq!(wire.v_mag.to_bits(), star.v().value().to_bits());
            assert_eq!(
                wire.camera_band_mag.to_bits(),
                reddened.camera_band_mag().to_bits()
            );
            assert_eq!(
                wire.chroma.map(f64::to_bits),
                chromaticity(reddened.red_green()).map(f64::to_bits)
            );
            assert!(wire.chroma.iter().all(|c| (0.0..=1.0).contains(c)));
            let colour = 2.5 * hyperion_sim::math::log10(reddened.sp_ratio() / REFERENCE_SP_RATIO);
            assert!(
                (wire.eye_offset_mag - colour).abs() < 1e-12,
                "{} against {colour}",
                wire.eye_offset_mag
            );
            let length = wire
                .direction
                .iter()
                .map(|&c| f64::from(c) * f64::from(c))
                .sum::<f64>()
                .sqrt();
            assert!((length - 1.0).abs() < 1e-6, "{length}");
            assert!(
                (f64::from(wire.distance_ly) - star.distance().value()).abs()
                    < 1e-6 * star.distance().value()
            );
        }
    }

    /// A texel's wire form takes its luminance as `f32`, its chromaticity, its eye limit where the
    /// eye was asked and its ρ (Design note 17).
    #[test]
    fn a_wire_texel_is_its_band_texel() {
        let galaxy = Galaxy::new(Seed::new(SEED));
        let tables = SkyTables::new(LuminosityTables::dark(&galaxy), &galaxy);
        let asked = SkyAsk::try_from(&request()).unwrap();
        let query = asked.query(None, light(), SkyCaps::forced(LightYears::ZERO).unwrap());
        let spec = BandSpec::new(4, 12).unwrap();
        let mut texels = Vec::new();
        hyperion_sim::sky::band::band_rows(
            &galaxy,
            &mut tables.march_context(),
            &query,
            &SkyCensus::empty(),
            &CompleteTo::nowhere(),
            &spec,
            hyperion_sim::sky::band::CubeFace::PosZ,
            0..4,
            &mut texels,
        );
        assert_eq!(texels.len(), 16);
        for texel in &texels {
            let wire = wire_texel(texel);
            #[expect(clippy::cast_possible_truncation, reason = "the wire's f32")]
            let luminance = texel.luminance().value() as f32;
            assert_eq!(wire.luminance_cd_m2.to_bits(), luminance.to_bits());
            assert_eq!(
                wire.chroma.map(f64::to_bits),
                chromaticity(texel.chroma().map(f64::from)).map(f64::to_bits)
            );
            assert_eq!(wire.eye_limit_mag, None, "no eye was asked");
            assert_eq!(wire.sp_ratio.to_bits(), texel.sp_ratio().to_bits());
        }
    }

    /// A host's disc on the wire is the sim's, field for field, in B, V, R order (Design note 16).
    #[test]
    fn a_host_disc_on_the_wire_is_the_sims() {
        let galaxy = Galaxy::new(Seed::new(19));
        let mut cell = Vec::new();
        generate_cell(
            &galaxy,
            CellKey::new(Layer::E, [0, 203, 0]).unwrap(),
            &mut cell,
        );
        let discs: Vec<HostDisc> = cell
            .iter()
            .map(|record| {
                host_discs(
                    &galaxy,
                    &SystemStars::generate(&galaxy, record),
                    UniverseTime::EPOCH,
                )
            })
            .find(|discs| !discs.is_empty())
            .expect("a system of the cell with a star to draw");
        for disc in &discs {
            let dto = host_disc(disc);
            let colour = disc.colour();
            assert_eq!(dto.star, disc.star().get());
            assert_eq!(
                [dto.radius_m, dto.teff_k, dto.log_g, dto.lux_per_v0].map(f64::to_bits),
                [
                    disc.radius().value(),
                    disc.teff().value(),
                    disc.log_g(),
                    colour.lux_per_v0()
                ]
                .map(f64::to_bits)
            );
            for channel in 0..3 {
                assert_eq!(
                    dto.mean_luminance_cd_m2[channel].to_bits(),
                    disc.mean_luminance()[channel].value().to_bits()
                );
                assert_eq!(
                    dto.central_luminance_cd_m2[channel].to_bits(),
                    disc.central_luminance()[channel].value().to_bits()
                );
                assert_eq!(
                    (
                        dto.limb[channel].c.to_bits(),
                        dto.limb[channel].alpha.to_bits()
                    ),
                    (
                        disc.limb()[channel].c().to_bits(),
                        disc.limb()[channel].alpha().to_bits()
                    )
                );
            }
            assert_eq!(
                dto.chroma.map(f32::to_bits),
                colour.chroma().map(f32::to_bits)
            );
            assert_eq!(
                dto.bake_spectrum.map(f64::to_bits),
                colour.bake_spectrum().map(f64::to_bits)
            );
        }
    }

    /// The first record of `layer`'s cells near the Sun that `keep` takes, walking out along x.
    fn near_the_sun(
        galaxy: &Galaxy,
        layer: Layer,
        keep: impl Fn(&SystemRecord) -> bool,
    ) -> SystemRecord {
        let at = hyperion_sim::coords::GalacticPosition::from_light_years(SUN_LY.map(f64::from))
            .unwrap();
        let sun = CellKey::containing(layer, &at).unwrap();
        let size = i32::try_from(sun.size_ly()).unwrap();
        let [x, y, z] = sun.origin_ly().map(|ly| ly.div_euclid(size));
        let mut cell = Vec::new();
        (0..64)
            .find_map(|step| {
                generate_cell(
                    galaxy,
                    CellKey::new(layer, [x + step, y, z]).ok()?,
                    &mut cell,
                );
                cell.iter().find(|record| keep(record)).copied()
            })
            .expect("a record near the Sun")
    }

    /// An excluded system's stars come back as their discs at the query's time, and a rogue
    /// planet, whose stars are not generated, as none (R06.T11.c).
    #[tokio::test]
    async fn an_excluded_systems_stars_are_its_discs_and_a_rogue_planet_has_none() {
        let harness = Harness::start(Handlers).await;
        let galaxy = Arc::new(Galaxy::new(Seed::new(SEED)));
        let key = GalaxyKey::new(SEED, hyperion_sim::GENERATOR_VERSION);
        let query = SkyAsk::try_from(&request())
            .unwrap()
            .query(None, light(), SkyCaps::DERIVED);
        let t = query.observer().time();
        let lit = |record: &SystemRecord| {
            !host_discs(&galaxy, &SystemStars::generate(&galaxy, record), t).is_empty()
        };
        let host = near_the_sun(&galaxy, Layer::E, lit);
        let rogue = near_the_sun(&galaxy, Layer::RoguePlanet, |_| true);
        let token = CancelToken::new();
        for (system, expected) in [
            (
                host.id(),
                host_discs(&galaxy, &SystemStars::generate(&galaxy, &host), t)
                    .iter()
                    .map(host_disc)
                    .collect::<Vec<_>>(),
            ),
            (rogue.id(), Vec::new()),
        ] {
            let found = timeout(
                WAIT,
                discs(harness.state(), key, &galaxy, system, &query, &token),
            )
            .await
            .expect("timed out drawing the discs")
            .unwrap();
            assert_eq!(found, expected, "{system}");
        }
        assert!(!host_discs(&galaxy, &SystemStars::generate(&galaxy, &host), t).is_empty());
        harness.stop().await;
    }

    async fn universe(harness: &Harness) -> UniverseIdHex {
        let creating = super::super::universe::create(
            Arc::clone(harness.state()),
            CreateUniverseRequest {
                name: "Sky".to_owned(),
                seed: Some(hyperion_protocol::SeedHex::from_u64(SEED)),
            },
        );
        let created = timeout(WAIT, creating)
            .await
            .expect("timed out creating the universe")
            .unwrap();
        let ResponseBody::CreateUniverse(info) = created else {
            panic!("create_universe answers with the universe: {created:?}");
        };
        info.id
    }

    /// An ID of the right form that `resolve` places nowhere is refused, before any bulk job.
    #[tokio::test]
    async fn an_exclude_system_the_galaxy_does_not_hold_is_refused_naming_it() {
        let harness = Harness::start(Handlers).await;
        let id = universe(&harness).await;
        // The index after any cell's last candidate, and bits that no grid places.
        let missing = CellKey::new(Layer::C, [0, 812, 0])
            .unwrap()
            .candidate_id(60_000)
            .expect("an index the layer's IDs can hold");
        for raw in [missing.raw(), u64::MAX] {
            let request = SkyRequest {
                universe: id.clone(),
                exclude_system: Some(SystemIdHex::from_u64(raw)),
                ..request()
            };
            // The server's caps are derived, so a table build would take minutes: the refusal
            // comes before it.
            let answering = answer(
                Arc::clone(harness.state()),
                request,
                CancelToken::new(),
                Replies::detached(),
            );
            let error = timeout(WAIT, answering)
                .await
                .expect("the refusal comes before any table is built")
                .unwrap_err();
            assert_eq!(
                (error.code, error.field.as_deref()),
                (ErrorCode::UnknownSystem, Some("exclude_system")),
                "{raw:#018x}"
            );
        }
        assert_eq!(harness.state().pool.counters().queued_bulk(), 0);
        harness.stop().await;
    }
}
