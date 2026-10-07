//! The sky's handler (rendering plan R06, R06.T11.a and T11.b; Design notes 5 and 9–17): `sky`,
//! every star brighter than a view's limit as seen from a point at a time.
//!
//! The request is checked field by field, as every handler's is: the universe, then `time`,
//! `observer`, `eye`, `camera_limit_v`, `n_max`, `cone` and `exclude_system`, whose ID plan 03's
//! `resolve` looks up in a pool job, as the scene's frames are. The eye's cut is then the eye cut's
//! pre-pass, and the request's cut is the deeper of it and the camera's limit. Everything the sky
//! computes runs as bulk jobs on the CPU pool, never in the interactive queue
//! ([`compute::sky`](crate::compute::sky)): the tables, the eye's cut, the census's plan, the
//! census itself a few hundred cells to a job over the server's cell cache, the merge, and the
//! payload.
//!
//! The answer is the census's JSON (the cut, each layer's cap and tallies, the listed and overflow
//! counts, `valid_until` and what is not modelled) and its bulk payload, in R03's binary frames
//! before it (R06.T11.b): each listed star in the census's order in its wire form ([`wire_star`]),
//! then the band's texels, the response's `stars_bytes` and `band_bytes` splitting it and its
//! manifest stating the whole. Until R06.T11.c computes the band, the limits and the discs, the
//! band's part is empty (`band_bytes` 0) and the reply has no host disc; the band's shape is the
//! server's, 64² a face.

use std::fmt;
use std::num::NonZeroU32;
use std::sync::Arc;

use hyperion_protocol::{
    BandSpecDto, ConeDto, ErrorCode, EyeDto, MAX_CUT_V, MAX_SKY_STARS, RequestError, ResponseBody,
    SkyGapDto, SkyLayerCensusDto, SkyRequest, SkyResponse, SystemIdHex,
};
use hyperion_sim::coords::UnitVector;
use hyperion_sim::galaxy::Galaxy;
use hyperion_sim::galaxy::placement::resolve;
use hyperion_sim::galaxy::query::pad_speed;
use hyperion_sim::id::{Layer, SystemId};
use hyperion_sim::observe::Observer;
use hyperion_sim::sky::EyeObserver;
use hyperion_sim::sky::caps::LayerCap;
use hyperion_sim::sky::census::{
    BuildSkyQueryError, CensusTallies, Cone, LayerTally, SkyCensus, SkyQuery, SkyStar,
};
use hyperion_sim::sky::eye::{BuildEyeError, SkyBackground, SpRatio, star_colour_offset};
use hyperion_sim::tables::star_colour::LUMINANCE_RGB;
use hyperion_sim::time::{Span, UniverseTime};
use hyperion_sim::units::consts::{
    METRES_PER_LIGHT_YEAR, RADIANS_PER_DEGREE, SECONDS_PER_JULIAN_YEAR,
};
use hyperion_sim::units::{CandelasPerSquareMetre, Degrees, LightYears, Magnitudes};

use super::universe::openable_universe;
use crate::AppState;
use crate::bulk::sky::{EncodedSky, SkyStarWire, encode_sky_payload};
use crate::bulk::{Answer, BulkPayload};
use crate::compute::sky::CensusInputs;
use crate::compute::{self, CancelToken, JobError, Priority, SkyCaps};
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

/// Every star brighter than the request's cut as seen from its observer at its time: the census's
/// JSON, and its stars as the bulk payload before it (R06.T11.a and T11.b; the band and the discs
/// are T11.c's).
///
/// The tables, the eye's cut, the plan, the census, its merge and the payload each run as bulk
/// jobs under `token`, the census a few hundred cells to a job, so a cancelled request's queued
/// jobs are skipped and a running census job stops at its next cell.
///
/// # Errors
///
/// Those of [`openable_universe`]; `bad_request` naming the first field at fault, in the order
/// `time`, `observer`, `eye.field_factor`, `eye.age_years`, `eye.pigmentation`, `camera_limit_v`
/// (also for a request that asks neither the eye nor a camera), `n_max` and `cone` (also for an
/// eye with a cone, `decision-r06-t8k-cone.md`); `unknown_system` naming `exclude_system` for an
/// ID whose bits are not a system ID or that `resolve` refuses; those of the galaxy cache;
/// `queue_full` if the interactive queue has no room for the lookup of `exclude_system`; and
/// `cancelled` or `internal` from any job.
pub(crate) async fn answer(
    state: Arc<AppState>,
    request: SkyRequest,
    token: CancelToken,
) -> Result<Answer, RequestError> {
    let universe = openable_universe(&state, &request.universe)?;
    let asked = SkyAsk::try_from(&request)?;
    let galaxy = state.galaxies.get(universe.key()).await?;
    if let (Some(system), Some(named)) = (asked.exclude, &request.exclude_system) {
        check_exclude(&state, &galaxy, system, named, token.clone()).await?;
    }
    let pool = &state.pool;
    let tables = compute::sky::tables(pool, &galaxy, state.sky_caps, &token).await?;
    let eye = match asked.eye {
        Some(eye) => Some((
            eye,
            compute::sky::eye_cut(pool, &galaxy, &tables, asked.observer, eye, &token).await?,
        )),
        None => None,
    };
    let query = Arc::new(asked.query(eye, state.sky_caps));
    let plan = compute::sky::plan(pool, &galaxy, &tables, &query, &token).await?;
    let inputs = CensusInputs {
        galaxy,
        key: universe.key(),
        tables,
        cells: Arc::clone(&state.sky_cells),
        query: Arc::clone(&query),
    };
    let census = Arc::new(compute::sky::census(pool, &inputs, &plan, &token).await?);
    tracing::debug!(
        cut = query.cut().value(),
        listed = census.listed().len(),
        overflow = census.overflow().len(),
        "sky censused"
    );
    let (observer, listed) = (*query.observer(), Arc::clone(&census));
    let encoded = compute::sky::bulk(pool, &token, move |_: &CancelToken| {
        payload(&observer, &listed)
    })
    .await?;
    let bulk = BulkPayload::new(encoded.bytes.clone()).expect(
        "a sky's payload, at most N_max's 3 × 10⁵ stars and six faces of texels (7.5 MB), has far \
         fewer chunks than a u32 counts",
    );
    let body = response(request, &query, plan.caps(), &census, &bulk, &encoded);
    Ok(Answer {
        body: ResponseBody::Sky(Box::new(body)),
        bulk: Some(bulk),
    })
}

/// The sky's bulk payload (Design note 17): each listed star of `census` as the wire carries it,
/// seen from `observer`, in the census's order, then the band's texels.
///
/// The band is empty until R06.T11.c computes it, so the payload is the stars alone.
#[must_use]
fn payload(observer: &Observer, census: &SkyCensus) -> EncodedSky {
    let stars: Vec<SkyStarWire> = census
        .listed()
        .iter()
        .map(|star| wire_star(observer, star))
        .collect();
    encode_sky_payload(&stars, &[])
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

impl SkyAsk {
    /// The census's query, once the eye's cut is known: to the deeper of the eye's cut and the
    /// camera's limit, with the eye at its own cut if a camera's is deeper (R06.T9.j), and every
    /// cap forced if `caps` forces them. `eye` is the request's eye with its cut, the eye cut's
    /// pre-pass, or `None` where the request does not ask the eye.
    ///
    /// # Panics
    ///
    /// If `eye` is `None` and the request asks no camera's limit either. A checked request asks the
    /// eye, a camera or both, and the handler passes the eye with its cut whenever the request asks
    /// it.
    #[must_use]
    fn query(&self, eye: Option<(EyeObserver, Magnitudes)>, caps: SkyCaps) -> SkyQuery {
        let cut = match (eye, self.camera_limit) {
            (Some((_, eye_cut)), Some(camera)) => {
                if camera.value() > eye_cut.value() {
                    camera
                } else {
                    eye_cut
                }
            }
            (Some((_, cut)), None) | (None, Some(cut)) => cut,
            (None, None) => panic!("a sky's query is asked for the eye, a camera or both"),
        };
        let mut builder = SkyQuery::builder(self.observer, cut).n_max(self.n_max);
        if let Some((eye, eye_cut)) = eye {
            builder = builder.eye(eye).eye_cut(eye_cut);
        }
        if let Some(cone) = self.cone {
            builder = builder.cone(cone);
        }
        if let Some(system) = self.exclude {
            builder = builder.exclude(system);
        }
        let query = builder.build().expect(
            "a checked request's cut, n_max and cone build, and the eye's cut is the cut or under it",
        );
        match caps.forced_radius() {
            Some(radius) => query
                .with_caps_forced(radius)
                .expect("a forced sky cap is checked when it is made"),
            None => query,
        }
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

/// The census's answer (R06.T11.a): the cut, each layer's cap and tallies, the counts, the time it
/// holds to and what is not modelled, with the manifest of `bulk` and where `encoded` splits.
///
/// The hosts are none until T11.c.
#[must_use]
fn response(
    request: SkyRequest,
    query: &SkyQuery,
    caps: &[LayerCap],
    census: &SkyCensus,
    bulk: &BulkPayload,
    encoded: &EncodedSky,
) -> SkyResponse {
    let tallies = census.tallies();
    SkyResponse {
        universe: request.universe,
        time: wire_time(query.observer().time()),
        observer: request.observer,
        valid_until: wire_time(valid_until(query.observer().time(), census.listed())),
        cut_v: query.cut().value(),
        census: caps.iter().map(|cap| layer_census(cap, tallies)).collect(),
        listed: narrow(census.listed().len()),
        overflow: narrow(census.overflow().len()),
        band: BandSpecDto {
            face_texels: query.band_spec().face_texels(),
        },
        hosts: Vec::new(),
        not_modelled: not_modelled(tallies),
        bulk: bulk.manifest(),
        stars_bytes: encoded.stars_bytes,
        band_bytes: encoded.band_bytes,
    }
}

/// One layer's census as the wire states it (R06's Risks, T8.c's mapping): its cap, and its
/// tallies, each count narrowed to the wire's width.
#[must_use]
fn layer_census(cap: &LayerCap, tallies: &CensusTallies) -> SkyLayerCensusDto {
    let tally = tallies.layer(cap.layer());
    SkyLayerCensusDto {
        layer: mass_layer(cap.layer()),
        cap_ly: cap.radius().value(),
        rule_bound_ly: cap.rule_bound().value(),
        expected_beyond: cap.expected_beyond(),
        cells: narrow(tally.cells()),
        candidates_opened: tally.generated(),
        accepted: tally.accepted(),
        listed: narrow(tally.listed()),
        without_photometry: narrow(tally.without_photometry()),
        feature_members_absent: tallies.feature_members_absent(),
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

/// A listed star as the wire carries it (Design note 17; R06.T11.a, sent from T11.b).
///
/// It holds its unit direction from `observer` and its distance, its own V, M<sub>V</sub> + DM +
/// v★(A<sub>V</sub>) A<sub>V</sub>, which the census cuts, and its chroma, eye offset and camera
/// band term after its own reddening, [`StarColour::reddened`] at its A<sub>V</sub> (R06.T9.e), the
/// camera term relative to that V.
///
/// Until R06.T11.c builds the limit map, its eye offset is its colour offset alone, against a
/// scotopic background: 2.5 log₁₀(ρ★ ÷ 2.297) at its reddened S/P ratio ρ★ (Crumey 2014, eq. 15;
/// `decision-r06-t9c-glare.md`, §4). From T11.c it is `sky::limits::eye_offsets`', its own eye
/// limit less its texel's (R06.T9.h). A star at the observer's own position, which no census
/// lists, would be given no direction.
///
/// [`StarColour::reddened`]: hyperion_sim::sky::colour::StarColour::reddened
#[must_use]
pub(crate) fn wire_star(observer: &Observer, star: &SkyStar) -> SkyStarWire {
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
        eye_offset_mag: scotopic_colour_offset(reddened.sp_ratio()).value(),
        camera_band_mag: reddened.camera_band_mag(),
    }
}

/// The eye colour offset of light of S/P ratio `sp_ratio` against a background of no light,
/// where every offset is its scotopic 2.5 log₁₀(ρ★ ÷ 2.297) (`sky::eye::star_colour_offset`).
/// The ratio is held within [`SpRatio`]'s 0.01–100, which no starlight leaves.
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
    use hyperion_sim::sky::band::BandSpec;
    use hyperion_sim::sky::census::{MAX_N_MAX, merge_census};
    use tokio::time::timeout;

    use super::*;
    use crate::compute::sky::SkyTables;
    use crate::requests::Handlers;
    use crate::testing::{Harness, WAIT};

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
        let seen = Some((EyeObserver::default(), Magnitudes::new(8.5)));
        let caps = SkyCaps::DERIVED;
        assert_eq!(
            cuts(&ask(Some(eye()), Some(10.0)).query(seen, caps)),
            (10.0, Some(8.5)),
            "a camera's deeper cut"
        );
        assert_eq!(
            cuts(&ask(Some(eye()), Some(7.0)).query(seen, caps)),
            (8.5, Some(8.5)),
            "a camera's shallower cut"
        );
        assert_eq!(
            cuts(&ask(Some(eye()), None).query(seen, caps)),
            (8.5, Some(8.5))
        );
        assert_eq!(cuts(&ask(None, Some(9.0)).query(None, caps)), (9.0, None));
        let ask = ask(None, Some(9.0));
        let forced = SkyCaps::forced(LightYears::new(30.0)).unwrap();
        assert_eq!(cuts(&ask.query(None, forced)), (9.0, None));
        assert_eq!(ask.query(None, caps).n_max().get(), MAX_SKY_STARS);
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
        let tables = SkyTables::build(&galaxy, caps);
        let asked = SkyAsk::try_from(&request()).unwrap();
        let query = asked.query(None, caps);
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
            let wire = wire_star(observer, star);
            let reddened = star.colour().reddened(star.a_v());
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
            // Its colour offset alone, scotopic, until R06.T11.c's limit map.
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
            let answering = answer(Arc::clone(harness.state()), request, CancelToken::new());
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
