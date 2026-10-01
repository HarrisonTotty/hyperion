//! Wire protocol shared by the HYPERION server and its clients.
//!
//! Every message is a JSON object discriminated by a `type` field, and requests carry a body
//! discriminated by `kind` (see [`ClientMessage`] and [`RequestBody`]). This crate holds wire types
//! only: the server converts the simulation's types to and from them, and nothing here depends on
//! the simulation. The one behaviour it has is the text form of 64-bit values, which cross the wire
//! as 16 lowercase hexadecimal digits because a JSON number cannot carry a `u64` into JavaScript.
//!
//! The TypeScript bindings in `packages/protocol/src/generated` are generated from these types by
//! `ts-rs` when running `cargo test` (`just gen-protocol`); never edit them by hand.
//!
//! # Binary frames
//!
//! A request whose kind asks for bulk is answered with binary frames and then its terminal JSON
//! `response`, whose body carries a [`BulkManifestDto`] of the frames' count and total payload
//! bytes (rendering plan R03, Design note 10). No other message is ever binary: a client that asks
//! for no bulk receives none. Each frame is at most 262,144 bytes, header included, and starts with
//! a 24-byte header, little-endian:
//!
//! | Bytes | Field |
//! | ----- | ----- |
//! | 0–3   | the magic `HYPB` (`48 59 50 42`) |
//! | 4     | the format, 1 |
//! | 5     | reserved, 0 |
//! | 6–7   | the header's length, 24 (`u16`) |
//! | 8–11  | the request's ID (`u32`) |
//! | 12–15 | the chunk's index, from 0 (`u32`) |
//! | 16–19 | the chunk count (`u32`) |
//! | 20–23 | the payload's length in this frame (`u32`) |
//!
//! The payload follows the header. The chunks come in order, before the terminal response; a
//! cancelled or failed request stops its chunks and ends in `cancelled` or its `request_error`,
//! and the client drops what it had. There is no checksum: TCP and the WebSocket framing guard the
//! bytes, and the count and length checks catch the server's own bugs. The frames are not a type
//! here, since this crate holds JSON wire types.

mod bulk;
mod envelope;
mod galaxy;
mod modelled;
mod orbit;
mod planetary;
mod primitives;
mod scene;
mod stellar;
#[cfg(test)]
pub(crate) mod testing;
mod universe;

pub use bulk::BulkManifestDto;
pub use envelope::{
    ClientMessage, ErrorCode, NotificationBody, REQUEST_KINDS, RequestBody, RequestError,
    RequestId, ResponseBody, ServerMessage, SubscribeRequest, Subscribed, SubscriptionState,
    SubscriptionTopic, UnsubscribeRequest,
};
pub use galaxy::{
    Census, DensityMap, DensityMapRequest, GalaxyParameters, GalaxyParametersRequest, LayerCensus,
    LayerStatus, MapPopulation, MapView, MassLayer, Parameter, ParameterGroup, ParameterOrigin,
    ParameterValue, Population, SystemRecord, SystemsInRange, SystemsInRangeRequest, Unit,
};
pub use modelled::Modelled;
pub use orbit::{HierarchyDto, HierarchyNodeDto, OrbitDto};
pub use planetary::{
    ArchitectureClassDto, BeltComponentDto, BeltCompositionDto, BeltDto, BeltGapDto, BeltKindDto,
    BeltSiteDto, BodyDetailDto, BodyDetailRequest, BodyEventDto, BodyEventsDto, BodyEventsRequest,
    BodyHooksDto, BodyKindDto, BodyOrbitDto, BodyRecordDto, BodyStateDto, BodySummaryDto,
    BodySurfaceDto, BulkPropertiesDto, CometaryHaloDto, DestructionCauseDto, DetailLevelDto,
    HabitableZoneDto, MassFractionsDto, MoonOriginDto, OrbitHostDto, PlanetClassDto, PopulationDto,
    RingDto, RingGapDto, RingKindDto, RingMaterialDto, SectionDto, SystemBodiesDto,
    SystemBodiesRequest, SystemPlaneDto, ZoneDto,
};
pub use primitives::{
    BodyIdHex, DetailSeedHex, GalacticPosition, ParseBodyIdHexError, ParseHex64Error, SeedHex,
    SystemIdHex, UniverseIdHex, UniverseTime,
};
pub use scene::{
    BodyGrantDto, CameraReportDto, FramePositionDto, KinematicsDto, SceneArrivalDto, SceneBodyDto,
    SceneCamerasRequest, SceneClockDto, SceneClockStateDto, SceneCraftDto, SceneNotificationDto,
    SceneShipRequest, SceneShipSet, SceneStateDto, SceneSubscribeRequest, SceneSystemDto,
    SeenPositionDto,
};
pub use stellar::{
    BinaryClassDto, CataclysmicKindDto, HighMassXrayBinaryKindDto, KickModeDto, NatalKickDto,
    ObjectKindDto, PhaseDto, PlanetaryNebulaDto, PulsarDto, RemnantDto, StarEventDto,
    StarEventKindDto, StarSummaryDto, StellarBriefDto, SystemExistenceDto, SystemSummaryDto,
    SystemSummaryRequest, VariabilityDto, VariableKindDto, XrayBinaryKindDto,
};
pub use universe::{
    CreateUniverseRequest, OpenUniverseRequest, UniverseInfo, UniverseList, UniverseStatus,
};

/// The version of the wire protocol, sent in every `welcome`.
///
/// A client compares it with its own and refuses a server that differs. Adding a request kind or
/// an optional field does not bump it, since an older server answers the new kind with
/// `unsupported`; removing or changing a message, a field or a string does. Version 2 added the
/// request convention, which a version 1 server cannot serve at all.
///
/// The first `notification` and the first binary frames are additions of the same kind and leave
/// it at 2 (the rendering brainstorm's open question 21, ruled in plan R03's Design note 12): the
/// server sends a notification only on a subscription the client opened, and binary frames only in
/// answer to a request whose kind asks for bulk, so a version 2 client that sends neither receives
/// neither, and a newer client asking an older server gets `unsupported`. The ruling holds only
/// while neither is ever sent unasked; `crates/hyperion-server/tests/websocket.rs`'s
/// `a_client_that_asks_for_no_push_and_no_bulk_receives_only_known_text_frames` pins it.
pub const PROTOCOL_VERSION: u32 = 2;

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    /// Where ts-rs writes bindings: `TS_RS_EXPORT_DIR` at run time, else its default.
    fn export_dir() -> PathBuf {
        std::env::var_os("TS_RS_EXPORT_DIR")
            .map_or_else(|| PathBuf::from("./bindings"), PathBuf::from)
    }

    /// Writes `ProtocolVersion.ts` beside the generated types, so that the client's constant comes
    /// from this one. The name puts it under the `export_bindings` filter of `just gen-protocol`.
    #[test]
    fn export_bindings_protocol_version() {
        let dir = export_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let text = format!(
            "// This file was generated from `hyperion_protocol::PROTOCOL_VERSION` by \
             `just gen-protocol`.\n\
             // Do not edit this file manually.\n\
             \n\
             /**\n \
             * The version of the wire protocol, sent in every `welcome`. A server that sends\n \
             * another version cannot serve this client.\n \
             */\n\
             export const PROTOCOL_VERSION = {PROTOCOL_VERSION};\n"
        );
        std::fs::write(dir.join("ProtocolVersion.ts"), text).unwrap();
    }

    #[test]
    fn protocol_version_is_two() {
        assert_eq!(PROTOCOL_VERSION, 2);
    }
}
