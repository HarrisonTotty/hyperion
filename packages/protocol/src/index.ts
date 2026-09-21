// The types in ./generated are produced by ts-rs from crates/hyperion-protocol
// (run `just gen-protocol`). Re-export each new message type here.
import type { Census } from "./generated/Census";
import type { ClientMessage } from "./generated/ClientMessage";
import type { CreateUniverseRequest } from "./generated/CreateUniverseRequest";
import type { DensityMap } from "./generated/DensityMap";
import type { DensityMapRequest } from "./generated/DensityMapRequest";
import type { ErrorCode } from "./generated/ErrorCode";
import type { GalacticPosition } from "./generated/GalacticPosition";
import type { GalaxyParameters } from "./generated/GalaxyParameters";
import type { GalaxyParametersRequest } from "./generated/GalaxyParametersRequest";
import type { LayerCensus } from "./generated/LayerCensus";
import type { LayerStatus } from "./generated/LayerStatus";
import type { MapPopulation } from "./generated/MapPopulation";
import type { MapView } from "./generated/MapView";
import type { MassLayer } from "./generated/MassLayer";
import type { OpenUniverseRequest } from "./generated/OpenUniverseRequest";
import type { Parameter } from "./generated/Parameter";
import type { ParameterGroup } from "./generated/ParameterGroup";
import type { ParameterOrigin } from "./generated/ParameterOrigin";
import type { ParameterValue } from "./generated/ParameterValue";
import type { Population } from "./generated/Population";
import type { RequestBody } from "./generated/RequestBody";
import type { RequestError } from "./generated/RequestError";
import type { RequestId } from "./generated/RequestId";
import type { ResponseBody } from "./generated/ResponseBody";
import type { SeedHex } from "./generated/SeedHex";
import type { ServerMessage } from "./generated/ServerMessage";
import type { SystemIdHex } from "./generated/SystemIdHex";
import type { SystemRecord } from "./generated/SystemRecord";
import type { SystemsInRange } from "./generated/SystemsInRange";
import type { SystemsInRangeRequest } from "./generated/SystemsInRangeRequest";
import type { Unit } from "./generated/Unit";
import type { UniverseIdHex } from "./generated/UniverseIdHex";
import type { UniverseInfo } from "./generated/UniverseInfo";
import type { UniverseList } from "./generated/UniverseList";
import type { UniverseStatus } from "./generated/UniverseStatus";
import type { UniverseTime } from "./generated/UniverseTime";

export type {
  Census,
  ClientMessage,
  CreateUniverseRequest,
  DensityMap,
  DensityMapRequest,
  ErrorCode,
  GalacticPosition,
  GalaxyParameters,
  GalaxyParametersRequest,
  LayerCensus,
  LayerStatus,
  MapPopulation,
  MapView,
  MassLayer,
  OpenUniverseRequest,
  Parameter,
  ParameterGroup,
  ParameterOrigin,
  ParameterValue,
  Population,
  RequestBody,
  RequestError,
  RequestId,
  ResponseBody,
  SeedHex,
  ServerMessage,
  SystemIdHex,
  SystemRecord,
  SystemsInRange,
  SystemsInRangeRequest,
  Unit,
  UniverseIdHex,
  UniverseInfo,
  UniverseList,
  UniverseStatus,
  UniverseTime,
};

export { PROTOCOL_VERSION } from "./generated/ProtocolVersion";

export { decodeDensityMap, type DecodedDensityMap } from "./densityMap";
export { hexToU64, isHex64, u64ToHex } from "./hex";
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
