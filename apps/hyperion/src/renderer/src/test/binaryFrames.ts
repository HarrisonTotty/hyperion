import { BINARY_FRAME_HEADER_BYTES, BINARY_FRAME_MAGIC } from "@hyperion/protocol";

/**
 * One binary frame as the server's `encode_header` and `chunk` build it
 * (`crates/hyperion-server/src/bulk.rs`): the 24-byte little-endian header, then `payload`.
 */
export function binaryFrame(
  request: number,
  index: number,
  count: number,
  payload: readonly number[],
): ArrayBuffer {
  const bytes = new ArrayBuffer(BINARY_FRAME_HEADER_BYTES + payload.length);
  const view = new DataView(bytes);
  BINARY_FRAME_MAGIC.forEach((byte, at) => {
    view.setUint8(at, byte);
  });
  view.setUint8(4, 1);
  view.setUint16(6, BINARY_FRAME_HEADER_BYTES, true);
  view.setUint32(8, request, true);
  view.setUint32(12, index, true);
  view.setUint32(16, count, true);
  view.setUint32(20, payload.length, true);
  new Uint8Array(bytes, BINARY_FRAME_HEADER_BYTES).set(payload);
  return bytes;
}
