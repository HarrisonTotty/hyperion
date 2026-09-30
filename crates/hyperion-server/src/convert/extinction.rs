//! The `extinction_map` and `extinction` requests and their answers (plan 07, P07.T10.a and
//! T10.c): the checks of the requests' fields, and the wire form of an extinction map and of a
//! line of sight.

use hyperion_protocol::{
    ExtinctionMap, ExtinctionMapRequest, ExtinctionRequest, ExtinctionResult, ExtinctionTarget,
    MapView, TargetExtinction,
};
use hyperion_sim::coords::{GalacticPosition, LyCell};
use hyperion_sim::galaxy::gas::ccm::Band;
use hyperion_sim::galaxy::gas::extinction::Sightline;
use hyperion_sim::id::SystemId;
use hyperion_sim::time::UniverseTime;

use super::{ConvertRequestError, query_time};
use crate::compute::{
    CodeDepth, ExtinctionMapKey, GalaxyKey, MapResolution, QuantisedMap, RawDensityMap,
};
use crate::limits::MAX_EXTINCTION_TARGETS;

/// An `extinction_map` request, checked: the map asked for and the depth of its codes.
///
/// The universe is looked up before this, as every galaxy handler does; `resolution` and `bits`
/// are then checked as a density map's are, each naming its own field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ExtinctionMapQuery {
    view: MapView,
    resolution: MapResolution,
    depth: CodeDepth,
}

impl ExtinctionMapQuery {
    /// The map to compute, over the galaxy `galaxy` names.
    #[must_use]
    pub(crate) fn key(self, galaxy: GalaxyKey) -> ExtinctionMapKey {
        ExtinctionMapKey::new(galaxy, self.view, self.resolution)
    }

    /// The depth the codes are quantised to.
    #[must_use]
    pub(crate) fn depth(self) -> CodeDepth {
        self.depth
    }
}

impl TryFrom<&ExtinctionMapRequest> for ExtinctionMapQuery {
    type Error = ConvertRequestError;

    /// Checks `resolution` against the four widths M1 serves and `bits` against 8 and 16.
    fn try_from(request: &ExtinctionMapRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            view: request.view,
            resolution: MapResolution::try_from(request.resolution)
                .map_err(|error| ConvertRequestError::new("resolution", error))?,
            depth: CodeDepth::try_from(request.bits)
                .map_err(|error| ConvertRequestError::new("bits", error))?,
        })
    }
}

/// The answer to `extinction_map`: the raster's geometry, the codes' range in log₁₀ of magnitudes,
/// and the codes, as [`density_map`](super::density_map) builds a density map's. It takes the
/// request by value, to move the universe's ID into the answer from the pool job it runs on.
#[must_use]
pub(crate) fn extinction_map(
    request: ExtinctionMapRequest,
    raw: &RawDensityMap,
    quantised: &QuantisedMap,
) -> ExtinctionMap {
    ExtinctionMap {
        universe: request.universe,
        view: request.view,
        width_px: raw.width_px(),
        height_px: raw.height_px(),
        centre_ly: raw.centre_ly(),
        ly_per_px: raw.ly_per_px(),
        bits: quantised.depth().bits(),
        floor_log10_mag: quantised.floor_log10(),
        ceiling_log10_mag: quantised.ceiling_log10(),
        data_base64: quantised.to_base64(),
    }
}

/// Where one line of sight of a checked `extinction` request ends.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum LineEnd {
    /// A system ID, decoded but not yet resolved.
    System(SystemId),
    /// A system ID whose bits are no system ID at all: answered `no_such_system`, as one that
    /// resolves to nothing is.
    NotASystem,
    /// A point inside the root cube.
    Position(GalacticPosition),
}

/// An `extinction` request, checked: the origin, the time and each target (plan 07, P07.T10.c).
///
/// The universe is looked up before this. Then, in order: `time` inside the clock window, as a
/// range query's; `origin`, a canonical position inside the root cube; `targets`, from one to
/// [`MAX_EXTINCTION_TARGETS`]; and each position target, canonical and inside the root cube, the
/// refusal naming `targets`. A system target is only decoded here: an ID that names no system
/// answers that target `no_such_system` and does not fail the request.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ExtinctionQuery {
    origin: GalacticPosition,
    time: UniverseTime,
    ends: Vec<LineEnd>,
}

impl ExtinctionQuery {
    /// Where every line starts.
    #[must_use]
    pub(crate) fn origin(&self) -> &GalacticPosition {
        &self.origin
    }

    /// The instant the system targets are placed at.
    #[must_use]
    pub(crate) fn time(&self) -> UniverseTime {
        self.time
    }

    /// Where each line ends, in the request's order.
    #[must_use]
    pub(crate) fn ends(&self) -> &[LineEnd] {
        &self.ends
    }
}

impl TryFrom<&ExtinctionRequest> for ExtinctionQuery {
    type Error = ConvertRequestError;

    fn try_from(request: &ExtinctionRequest) -> Result<Self, Self::Error> {
        let time = query_time(&request.time)?;
        let origin = position_in_cube("origin", &request.origin)?;
        if request.targets.is_empty() {
            return Err(ConvertRequestError::new(
                "targets",
                "at least one target is needed",
            ));
        }
        if request.targets.len() > MAX_EXTINCTION_TARGETS {
            return Err(ConvertRequestError::new(
                "targets",
                format!(
                    "at most {MAX_EXTINCTION_TARGETS} targets may be asked for at once, not {}",
                    request.targets.len()
                ),
            ));
        }
        let ends = request
            .targets
            .iter()
            .map(|target| match target {
                ExtinctionTarget::System { id } => Ok(
                    SystemId::from_raw(id.to_u64()).map_or(LineEnd::NotASystem, LineEnd::System)
                ),
                ExtinctionTarget::Position { position } => {
                    position_in_cube("targets", position).map(LineEnd::Position)
                }
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { origin, time, ends })
    }
}

/// A canonical position inside the root cube, or the refusal of the field `field`.
fn position_in_cube(
    field: &'static str,
    position: &hyperion_protocol::GalacticPosition,
) -> Result<GalacticPosition, ConvertRequestError> {
    let checked = GalacticPosition::new(LyCell::new(position.cell_ly), position.offset_m)
        .map_err(|error| ConvertRequestError::new(field, error))?;
    if !checked.in_root_cube() {
        return Err(ConvertRequestError::new(
            field,
            "the position lies outside the galaxy's root cube",
        ));
    }
    Ok(checked)
}

/// One line's answer: A(V), E(B-V), A(K) and the two hydrogen columns.
#[must_use]
pub(crate) fn target_extinction(line: &Sightline) -> TargetExtinction {
    TargetExtinction::Ok {
        a_v_mag: line.a_v().value(),
        e_b_v_mag: line.reddening().value(),
        a_k_mag: line.in_band(Band::K).value(),
        hydrogen_column_per_cm2: line.hydrogen_column().value(),
        neutral_hydrogen_column_per_cm2: line.neutral_hydrogen_column().value(),
    }
}

/// The answer to `extinction`: the request's origin and time, echoed, and one answer per target in
/// its order.
#[must_use]
pub(crate) fn extinction_result(
    request: ExtinctionRequest,
    targets: Vec<TargetExtinction>,
) -> ExtinctionResult {
    ExtinctionResult {
        universe: request.universe,
        origin: request.origin,
        time: request.time,
        targets,
    }
}

#[cfg(test)]
mod tests {
    use hyperion_protocol::{ErrorCode, RequestError, SystemIdHex, UniverseIdHex};

    use super::*;

    fn wire_position(cell_ly: [i32; 3]) -> hyperion_protocol::GalacticPosition {
        hyperion_protocol::GalacticPosition {
            cell_ly,
            offset_m: [0.0; 3],
        }
    }

    fn request(targets: Vec<ExtinctionTarget>) -> ExtinctionRequest {
        ExtinctionRequest {
            universe: UniverseIdHex::from_u64(42),
            origin: wire_position([0, 26_000, 0]),
            time: hyperion_protocol::UniverseTime::default(),
            targets,
        }
    }

    fn refusal(request: &ExtinctionRequest) -> RequestError {
        RequestError::from(ExtinctionQuery::try_from(request).unwrap_err())
    }

    #[test]
    fn a_request_is_checked_field_by_field_in_order() {
        let position = |cell| ExtinctionTarget::Position {
            position: wire_position(cell),
        };
        let good = request(vec![position([100, 26_000, 0])]);
        let query = ExtinctionQuery::try_from(&good).unwrap();
        assert_eq!(query.time(), UniverseTime::EPOCH);
        assert!(matches!(query.ends(), [LineEnd::Position(_)]));

        let mut late = good.clone();
        late.time.seconds = i64::MAX;
        late.origin = wire_position([70_000, 0, 0]);
        assert_eq!(refusal(&late).field.as_deref(), Some("time"));

        let mut outside = good.clone();
        outside.origin = wire_position([70_000, 0, 0]);
        let error = refusal(&outside);
        assert_eq!(
            (error.code, error.field.as_deref()),
            (ErrorCode::BadRequest, Some("origin"))
        );

        for targets in [
            Vec::new(),
            vec![position([0, 0, 0]); MAX_EXTINCTION_TARGETS + 1],
            vec![position([0, 0, -70_000])],
        ] {
            assert_eq!(refusal(&request(targets)).field.as_deref(), Some("targets"));
        }
        assert!(
            ExtinctionQuery::try_from(&request(vec![position([0, 0, 0]); MAX_EXTINCTION_TARGETS]))
                .is_ok()
        );
    }

    #[test]
    fn an_id_that_is_no_system_id_is_a_target_with_no_system_and_not_a_refusal() {
        // A grid ID with a spare bit set (plan 01's ID layout) is no system's.
        let bits = 0x0200_0800_2000_0000 | (1 << 58);
        assert!(SystemId::from_raw(bits).is_err());
        let query = ExtinctionQuery::try_from(&request(vec![ExtinctionTarget::System {
            id: SystemIdHex::from_u64(bits),
        }]))
        .unwrap();
        assert_eq!(query.ends(), [LineEnd::NotASystem]);
    }
}
