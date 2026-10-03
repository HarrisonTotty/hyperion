/**
 * The sky's decode worker (plan R06, T12; R03 Design note 11): decodes a complete payload off the
 * render thread and posts the typed arrays back transferred.
 *
 * @remarks
 * It holds no logic of its own: {@link decodeSkyPayload} answers each request. No file imports this
 * module; `useSky` starts it with `new Worker(new URL("./decode.worker.ts", import.meta.url))`. It
 * is typed with the worker's library (`tsconfig.worker.json`), so `addEventListener` and
 * `postMessage` are the dedicated worker scope's.
 */

import { decodeSkyPayload, type SkyDecodeRequest, transferablesOf } from "./decodePayload";

addEventListener("message", (event: MessageEvent<SkyDecodeRequest>) => {
  const reply = decodeSkyPayload(event.data);
  postMessage(reply, transferablesOf(reply));
});
