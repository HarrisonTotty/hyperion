/**
 * What a body's medium is assembled from, and the key the atmosphere cache holds it under (plan
 * R08, R08.T10.a; Design note 14): the light half of `assemble.ts`, which re-exports it.
 *
 * @remarks
 * The render thread computes {@link atmosphereKey} on every `body_detail` reply, so this module
 * takes no optics data: beside type imports it imports only `fnv1a64` and `lib/system/rotation.ts`'s
 * `spinRateAt` (whose own imports are `lib/orbit.ts`'s). `assemble.ts`, which takes the absorbers'
 * and the materials' tables, runs in the optics worker. `assembleKey.test.ts` holds this module's
 * import graph to that.
 */

import type { BodyIdHex } from "@hyperion/protocol";

import type { BodyDetail } from "../../lib/system/model";
import { spinRateAt } from "../../lib/system/rotation";
import type { Rgb } from "../photometry/toneCurve";
import { fnv1a64 } from "../terrain/workers/f32Digest";
import type { BakeSpectrum } from "./spectralColour";

/**
 * The version of the client's atmosphere optics, which the cache keys on beside each body's
 * inputs (Design note 14): a client constant, not the generator version, since the atmosphere is
 * not generated output. Raised by any change to the optics or the assembly that changes a medium
 * built from the same inputs.
 *
 * @remarks
 * 2 since R08.T10.d (`decision-r08-approximate-plate.md` §8, item 1, 2026-10-27): H's lines take
 * H's own width from its table, so a medium that draws them changes.
 */
export const ATMOSPHERE_OPTICS_VERSION = 2;

/** One star lighting a body, as `lightsAt` lists it: its body ID and its spectrum in the bake bins. */
export interface BodyLight {
  readonly star: BodyIdHex;
  /** R06's `HostDiscDto.bake_spectrum`, W m⁻² nm⁻¹ per lux. */
  readonly spectrum: BakeSpectrum;
}

/** A kept scene's statement of a body's atmosphere where it sets no medium of its own. */
export type KeptAtmosphere =
  /** No atmosphere set (R07's `TEST GIANT`): `ATMOSPHERE: NOT YET MODELLED`. */
  | { readonly kind: "unset" }
  /** Set airless: no medium and no label (`signoff-2.md` item 6). */
  | { readonly kind: "airless" };

/**
 * What a body's medium is assembled from (the Provides' `BodyAtmosphereInput`).
 *
 * - `record`: a server body, its `body_detail` reply converted (`lib/system/bodiesWire.ts`'s
 *   `toBodyDetail`), its suns in `lightsAt`'s order (at most R07's `MAX_BODY_LIGHTS`, 2, which give
 *   the medium's light slots; none allowed), and its ground's albedo per channel, the cache's
 *   p q of R07's `BodyPhotometry` (`decision-r08-t10a-leans.md` item 2), clamped to [0, 1] here.
 * - `kept`: a kept scene's body that sets no medium. A scene's fixture medium is prebuilt by the
 *   scene and handed to the cache directly, never assembled.
 */
export type BodyAtmosphereInput =
  | {
      readonly kind: "record";
      readonly detail: BodyDetail;
      readonly lights: ReadonlyArray<BodyLight>;
      readonly groundAlbedo: Rgb;
    }
  | { readonly kind: "kept"; readonly atmosphere: KeptAtmosphere };

/**
 * A ground albedo held to [0, 1] per channel, as `AtmosphereMedium.groundAlbedo` requires: p q
 * can pass 1 on a bright icy photometric law (`decision-r08-t10a-leans.md` item 2).
 *
 * @throws RangeError for a channel that is not a finite number: a caller's bug.
 */
export function clampedGroundAlbedo(albedo: Rgb): Rgb {
  return [clampedChannel(albedo[0]), clampedChannel(albedo[1]), clampedChannel(albedo[2])];
}

/** One channel of {@link clampedGroundAlbedo}. */
function clampedChannel(value: number): number {
  if (!Number.isFinite(value)) {
    throw new RangeError(`a ground albedo is a finite number in each channel, not ${value}`);
  }
  return Math.min(Math.max(value, 0), 1);
}

/**
 * A value's canonical text: JSON with each object's keys sorted, so that equal values give equal
 * text however they were built. An absent member is left out, as JSON leaves it.
 */
function canonical(value: unknown): string {
  if (Array.isArray(value)) {
    return `[${value.map((item) => (item === undefined ? "null" : canonical(item))).join(",")}]`;
  }
  if (typeof value === "object" && value !== null) {
    const members = Object.entries(value)
      .filter(([, member]) => member !== undefined)
      .toSorted(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
      .map(([key, member]) => `${JSON.stringify(key)}:${canonical(member)}`);
    return `{${members.join(",")}}`;
  }
  // A record's leaves are strings, finite numbers, booleans and nulls, which JSON spells exactly
  // (a number by its shortest round-trip form, which tells every two doubles apart).
  return JSON.stringify(value);
}

/**
 * The key the atmosphere cache holds a body's medium under (Design note 14): the body, the
 * {@link ATMOSPHERE_OPTICS_VERSION} and an FNV-1a 64 digest of every input the medium is built
 * from, `<body ID>/v<version>/<16 hexadecimal digits>`; for a kept scene's body,
 * `kept/<unset or airless>/v<version>`.
 *
 * @remarks
 * The digest is over the canonical text ({@link canonical}) of the record's surface, envelope,
 * figure, mass and bulk sections, the rotation's spin rate at the record's time (`spinRateAt`, or
 * none without an `ok` rotation), the ground's albedo as it is drawn ({@link clampedGroundAlbedo})
 * and each light's star and spectrum, in order. The record's time itself, its granted level, its
 * position and its orbit are left out: a reply at a later time with the same spin rate keeps the
 * key, and a changed inventory, light or ground changes it. A digest is no proof of equality; two
 * bodies' inputs colliding in 64 bits is not provided against.
 */
export function atmosphereKey(input: BodyAtmosphereInput): string {
  let key: string;
  switch (input.kind) {
    case "kept":
      key = `kept/${input.atmosphere.kind}/v${ATMOSPHERE_OPTICS_VERSION}`;
      break;
    case "record": {
      const { record, time } = input.detail;
      const inputs = {
        surface: record.surface,
        envelope: record.envelope,
        figure: record.figure,
        spinRateRadPerS:
          record.rotation.state === "ok" ? spinRateAt(record.rotation.value, time) : null,
        massMearth: record.massMearth,
        bulk: record.bulk,
        groundAlbedo: clampedGroundAlbedo(input.groundAlbedo),
        lights: input.lights.map(({ star, spectrum }) => ({ star, spectrum })),
      };
      const digest = fnv1a64(new TextEncoder().encode(canonical(inputs)));
      key = `${record.id}/v${ATMOSPHERE_OPTICS_VERSION}/${digest.toString(16).padStart(16, "0")}`;
      break;
    }
  }
  return key;
}
