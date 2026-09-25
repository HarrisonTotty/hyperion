// The types in ./generated are produced by ts-rs from crates/hyperion-protocol
// (run `just gen-protocol`). Re-export each new message type here.
import type { ArchitectureClassDto } from "./generated/ArchitectureClassDto";
import type { BeltKindDto } from "./generated/BeltKindDto";
import type { BeltComponentDto } from "./generated/BeltComponentDto";
import type { BeltCompositionDto } from "./generated/BeltCompositionDto";
import type { BeltDto } from "./generated/BeltDto";
import type { BeltGapDto } from "./generated/BeltGapDto";
import type { BeltSiteDto } from "./generated/BeltSiteDto";
import type { CometaryHaloDto } from "./generated/CometaryHaloDto";
import type { PopulationDto } from "./generated/PopulationDto";
import type { RingDto } from "./generated/RingDto";
import type { RingGapDto } from "./generated/RingGapDto";
import type { RingKindDto } from "./generated/RingKindDto";
import type { RingMaterialDto } from "./generated/RingMaterialDto";
import type { BodyDetailDto } from "./generated/BodyDetailDto";
import type { BodyDetailRequest } from "./generated/BodyDetailRequest";
import type { BodyEventDto } from "./generated/BodyEventDto";
import type { BodyEventsDto } from "./generated/BodyEventsDto";
import type { BodyEventsRequest } from "./generated/BodyEventsRequest";
import type { BodyHooksDto } from "./generated/BodyHooksDto";
import type { BodyIdHex } from "./generated/BodyIdHex";
import type { BodyKindDto } from "./generated/BodyKindDto";
import type { BodyOrbitDto } from "./generated/BodyOrbitDto";
import type { BodyRecordDto } from "./generated/BodyRecordDto";
import type { BodyStateDto } from "./generated/BodyStateDto";
import type { BodySummaryDto } from "./generated/BodySummaryDto";
import type { BodySurfaceDto } from "./generated/BodySurfaceDto";
import type { BulkPropertiesDto } from "./generated/BulkPropertiesDto";
import type { Census } from "./generated/Census";
import type { ClientMessage } from "./generated/ClientMessage";
import type { CreateUniverseRequest } from "./generated/CreateUniverseRequest";
import type { DensityMap } from "./generated/DensityMap";
import type { DensityMapRequest } from "./generated/DensityMapRequest";
import type { DestructionCauseDto } from "./generated/DestructionCauseDto";
import type { DetailLevelDto } from "./generated/DetailLevelDto";
import type { ErrorCode } from "./generated/ErrorCode";
import type { GalacticPosition } from "./generated/GalacticPosition";
import type { GalaxyParameters } from "./generated/GalaxyParameters";
import type { GalaxyParametersRequest } from "./generated/GalaxyParametersRequest";
import type { HabitableZoneDto } from "./generated/HabitableZoneDto";
import type { HierarchyDto } from "./generated/HierarchyDto";
import type { HierarchyNodeDto } from "./generated/HierarchyNodeDto";
import type { KickModeDto } from "./generated/KickModeDto";
import type { LayerCensus } from "./generated/LayerCensus";
import type { LayerStatus } from "./generated/LayerStatus";
import type { MapPopulation } from "./generated/MapPopulation";
import type { MapView } from "./generated/MapView";
import type { MassFractionsDto } from "./generated/MassFractionsDto";
import type { MassLayer } from "./generated/MassLayer";
import type { MoonOriginDto } from "./generated/MoonOriginDto";
import type { NatalKickDto } from "./generated/NatalKickDto";
import type { ObjectKindDto } from "./generated/ObjectKindDto";
import type { OpenUniverseRequest } from "./generated/OpenUniverseRequest";
import type { OrbitDto } from "./generated/OrbitDto";
import type { OrbitHostDto } from "./generated/OrbitHostDto";
import type { Parameter } from "./generated/Parameter";
import type { ParameterGroup } from "./generated/ParameterGroup";
import type { ParameterOrigin } from "./generated/ParameterOrigin";
import type { ParameterValue } from "./generated/ParameterValue";
import type { PhaseDto } from "./generated/PhaseDto";
import type { PlanetClassDto } from "./generated/PlanetClassDto";
import type { PlanetaryNebulaDto } from "./generated/PlanetaryNebulaDto";
import type { Population } from "./generated/Population";
import type { PulsarDto } from "./generated/PulsarDto";
import type { RemnantDto } from "./generated/RemnantDto";
import type { RequestBody } from "./generated/RequestBody";
import type { RequestError } from "./generated/RequestError";
import type { RequestId } from "./generated/RequestId";
import type { ResponseBody } from "./generated/ResponseBody";
import type { SectionDto } from "./generated/SectionDto";
import type { SeedHex } from "./generated/SeedHex";
import type { ServerMessage } from "./generated/ServerMessage";
import type { StarEventDto } from "./generated/StarEventDto";
import type { StarEventKindDto } from "./generated/StarEventKindDto";
import type { StarSummaryDto } from "./generated/StarSummaryDto";
import type { StellarBriefDto } from "./generated/StellarBriefDto";
import type { SurfaceSeedHex } from "./generated/SurfaceSeedHex";
import type { SystemBodiesDto } from "./generated/SystemBodiesDto";
import type { SystemBodiesRequest } from "./generated/SystemBodiesRequest";
import type { SystemExistenceDto } from "./generated/SystemExistenceDto";
import type { SystemIdHex } from "./generated/SystemIdHex";
import type { SystemPlaneDto } from "./generated/SystemPlaneDto";
import type { SystemRecord } from "./generated/SystemRecord";
import type { SystemSummaryDto } from "./generated/SystemSummaryDto";
import type { SystemSummaryRequest } from "./generated/SystemSummaryRequest";
import type { SystemsInRange } from "./generated/SystemsInRange";
import type { SystemsInRangeRequest } from "./generated/SystemsInRangeRequest";
import type { Unit } from "./generated/Unit";
import type { UniverseIdHex } from "./generated/UniverseIdHex";
import type { UniverseInfo } from "./generated/UniverseInfo";
import type { UniverseList } from "./generated/UniverseList";
import type { UniverseStatus } from "./generated/UniverseStatus";
import type { UniverseTime } from "./generated/UniverseTime";
import type { VariabilityDto } from "./generated/VariabilityDto";
import type { VariableKindDto } from "./generated/VariableKindDto";
import type { ZoneDto } from "./generated/ZoneDto";

export type {
  BeltComponentDto,
  BeltCompositionDto,
  BeltDto,
  BeltGapDto,
  BeltSiteDto,
  CometaryHaloDto,
  PopulationDto,
  RingDto,
  RingGapDto,
  RingKindDto,
  RingMaterialDto,
  ArchitectureClassDto,
  BeltKindDto,
  BodyDetailDto,
  BodyDetailRequest,
  BodyEventDto,
  BodyEventsDto,
  BodyEventsRequest,
  BodyHooksDto,
  BodyIdHex,
  BodyKindDto,
  BodyOrbitDto,
  BodyRecordDto,
  BodyStateDto,
  BodySummaryDto,
  BodySurfaceDto,
  BulkPropertiesDto,
  Census,
  ClientMessage,
  CreateUniverseRequest,
  DensityMap,
  DensityMapRequest,
  DestructionCauseDto,
  DetailLevelDto,
  ErrorCode,
  GalacticPosition,
  GalaxyParameters,
  GalaxyParametersRequest,
  HabitableZoneDto,
  HierarchyDto,
  HierarchyNodeDto,
  KickModeDto,
  LayerCensus,
  LayerStatus,
  MapPopulation,
  MapView,
  MassFractionsDto,
  MassLayer,
  MoonOriginDto,
  NatalKickDto,
  ObjectKindDto,
  OpenUniverseRequest,
  OrbitDto,
  OrbitHostDto,
  Parameter,
  ParameterGroup,
  ParameterOrigin,
  ParameterValue,
  PhaseDto,
  PlanetClassDto,
  PlanetaryNebulaDto,
  Population,
  PulsarDto,
  RemnantDto,
  RequestBody,
  RequestError,
  RequestId,
  ResponseBody,
  SectionDto,
  SeedHex,
  ServerMessage,
  StarEventDto,
  StarEventKindDto,
  StarSummaryDto,
  StellarBriefDto,
  SurfaceSeedHex,
  SystemBodiesDto,
  SystemBodiesRequest,
  SystemExistenceDto,
  SystemIdHex,
  SystemPlaneDto,
  SystemRecord,
  SystemSummaryDto,
  SystemSummaryRequest,
  SystemsInRange,
  SystemsInRangeRequest,
  Unit,
  UniverseIdHex,
  UniverseInfo,
  UniverseList,
  UniverseStatus,
  UniverseTime,
  VariabilityDto,
  VariableKindDto,
  ZoneDto,
};

export { PROTOCOL_VERSION } from "./generated/ProtocolVersion";

export { decodeDensityMap, type DecodedDensityMap } from "./densityMap";
export {
  type BodyIdParts,
  formatBodyId,
  hexToU64,
  isBodyId,
  isHex64,
  parseBodyId,
  u64ToHex,
} from "./hex";
export { galacticDeltaLy, galacticPositionFromLy, METRES_PER_LIGHT_YEAR } from "./position";
export {
  RequestChannel,
  RequestClient,
  type PendingRequest,
  type RequestFailure,
  type RequestKind,
  type RequestOf,
  type RequestOutcome,
  type ResponseFor,
} from "./requests";
export { SECONDS_PER_JULIAN_YEAR, universeTimeFromYears, universeTimeToYears } from "./time";

/** Serializes `message` into the JSON text frame the server expects. */
export function encodeClientMessage(message: ClientMessage): string {
  return JSON.stringify(message);
}

/**
 * Parses a JSON text frame received from the server.
 *
 * @throws SyntaxError if `data` is not valid JSON.
 */
export function decodeServerMessage(data: string): ServerMessage {
  // The server is the source of truth for these types — they are generated
  // from it — so its payloads are trusted rather than re-validated here.
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion
  return JSON.parse(data) as ServerMessage;
}
