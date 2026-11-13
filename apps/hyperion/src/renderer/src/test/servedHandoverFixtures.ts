/**
 * A served world's answers over R03's fake socket (plan R10, R10.T15.b): the slice's Earth's scene,
 * its sky's discs, its field and its record, as the hand-over check's served world reads them.
 */
import type { BodyIdHex, DetailSeedHex, SceneStateDto } from "@hyperion/protocol";

import { TERRAIN_HOST_DISCS } from "../view/scenes/terrainScene";
import {
  fieldBlock,
  fieldCover,
  fieldPayload,
  latestFieldAsk,
  moonLikeHeader,
  readyField,
  sendFieldFrames,
} from "./coarseFieldFixtures";
import type { FakeWebSocket } from "./FakeWebSocket";
import { earthDetail } from "./planetaryFixture";
import { stateNearEarth } from "./sceneFixture";
import { skyResponse } from "./skyFixtures";

/**
 * Opens the latest `subscribe` on `state`: the ship's scene in the slice's Earth's frame unless
 * given.
 */
export function answerServedScene(
  socket: FakeWebSocket,
  state: SceneStateDto = stateNearEarth(),
): void {
  socket.serverAnswers("subscribe", () => ({
    kind: "subscribe",
    subscription: 1,
    state: { topic: "scene", ...state },
  }));
}

/** Answers the latest `sky` with no star and `TERRAIN TEST`'s disc. */
export function answerServedSky(socket: FakeWebSocket): void {
  socket.serverAnswers("sky", (body) => ({
    kind: "sky",
    ...skyResponse(body, new Uint8Array(0), 0, 0, 0),
    hosts: [...TERRAIN_HOST_DISCS],
  }));
}

/**
 * Answers the latest `surface_field` for `body` with a Moon-like header at level 5 and `cells` cells
 * from cell 0 at resolution code `code` (9, surveyed, unless given; 0 marks a margin), in one block
 * over one frame.
 */
export function answerServedField(
  socket: FakeWebSocket,
  body: BodyIdHex,
  cells: number,
  code = 9,
): void {
  const { id } = latestFieldAsk(socket);
  const bulk = sendFieldFrames(socket, id, fieldPayload([fieldBlock(64, 1)]), 64);
  socket.serverResponds(
    id,
    readyField(moonLikeHeader(body), 1, fieldCover([0, cells, code]), bulk),
  );
}

/** The detail seed {@link answerServedRecord} gives unless told otherwise, of its wire form. */
export const SERVED_DETAIL_SEED: DetailSeedHex = "a37587e7ad5c6a57";

/**
 * Answers the latest `body_detail` with the slice's Earth's record, its hooks `ok` and carrying
 * `detailSeed` ({@link SERVED_DETAIL_SEED} unless given), or not modelled for `null`.
 */
export function answerServedRecord(
  socket: FakeWebSocket,
  detailSeed: DetailSeedHex | null = SERVED_DETAIL_SEED,
): void {
  const detail = earthDetail();
  socket.serverAnswers("body_detail", () => ({
    ...detail,
    record: {
      ...detail.record,
      hooks:
        detailSeed === null
          ? { state: "not_modelled" }
          : { state: "ok", value: { detail_seed: { state: "ok", value: detailSeed } } },
    },
  }));
}
