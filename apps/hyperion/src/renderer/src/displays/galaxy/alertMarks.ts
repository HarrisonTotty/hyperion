/**
 * The local chart's marks of the sensor detections (plan 12, P12.T12): a bearing ray for a contact
 * not yet resolved, the transient's four-point star on a resolved host inside the fetched sphere,
 * and a ray with its range for a resolved host outside it.
 */
import {
  type BearingDto,
  type ContactDto,
  galacticDeltaLy,
  galacticPositionFromLy,
  type SystemIdHex,
} from "@hyperion/protocol";

import { formatBearing, formatLengthLy } from "../../lib/format";
import type { CentreLy, ChartResult } from "../../lib/galaxy/model";
import { localFrameAt } from "../../geometry/frame";
import { add, norm, scale, sub, vec3, type Vec3 } from "../../geometry/vec3";
import { OBSERVER_MARK_ID } from "../../spatial/drawList";
import type { PointMark, SegmentMark, SizeClass, SpatialScene } from "../../spatial/marks";
import { answeredMode } from "./chartModel";

/** What a transient's mark and a bearing ray are labelled with: the guide's event kind. */
export const TRANSIENT_LABEL = "TRANSIENT";

/**
 * A transient's size class: the largest, 1 rem across, as a feature's is, so that its four points
 * read open south of the plane and the star stands out about the host's own smaller symbol.
 */
export const TRANSIENT_SIZE_CLASS: SizeClass = 4;

/**
 * Where a transient's label ranks among the chart's: above every feature's (`1e12` and their mass)
 * and every system's, since a detection is what the chart's operator is to be made aware of, and
 * among the transients by contact number, the first seen first.
 */
const TRANSIENT_LABEL_PRIORITY = 1e13;

/**
 * How far a bearing ray is drawn from the observer, ly: a million, past every chart's view at any
 * zoom (the root cube's diagonal is 227,023 ly, the sim's `ROOT_CUBE_DIAGONAL`), so that the
 * view's clip ends it at the display's edge, as the guide draws a bearing without a range.
 */
export const RAY_REACH_LY = 1_000_000;

/**
 * The decimals a ray's range is written with, in light-years: the light age's 0.01 yr, the
 * distance its light has come, since a host outside the fetched sphere lies beyond the chart's own
 * distances and its unit (`LIGHT AGE` in the guide's nomenclature).
 */
const RANGE_DECIMALS = 2;

/** What a transient's mark's ID starts with, apart from a system's sixteen hex digits. */
const TRANSIENT_MARK_PREFIX = "transient:";

/** What a bearing ray's ID starts with. */
const BEARING_RAY_PREFIX = "bearing:";

/** The ID of a contact's transient mark on the chart. */
export function transientMarkId(contact: number): string {
  return `${TRANSIENT_MARK_PREFIX}${String(contact)}`;
}

/** The ID of a contact's bearing ray on the chart. */
export function bearingRayId(contact: number): string {
  return `${BEARING_RAY_PREFIX}${String(contact)}`;
}

/**
 * The unit direction, along the `GALACTIC` axes, from which a contact's light reaches an observer
 * at `observerLy` (plan 12, Design note 12): azimuth from the observer's coreward through spinward,
 * elevation positive north, or, in the `galactic_x` frame within a light-year of the axis, azimuth
 * from +x through +y. The inverse of the sim's `observe::bearing`.
 */
export function bearingRay(observerLy: CentreLy, bearing: BearingDto): Vec3 {
  const azimuth = (bearing.azimuth_deg * Math.PI) / 180;
  const elevation = (bearing.elevation_deg * Math.PI) / 180;
  let forward: Vec3;
  let left: Vec3;
  let up: Vec3;
  switch (bearing.frame) {
    case "local": {
      const frame = localFrameAt(vec3(...observerLy));
      forward = frame.coreward;
      left = frame.spinward;
      up = frame.north;
      break;
    }
    case "galactic_x":
      forward = vec3(1, 0, 0);
      left = vec3(0, 1, 0);
      up = vec3(0, 0, 1);
      break;
  }
  const horizontal = Math.cos(elevation);
  return add(
    add(
      scale(forward, horizontal * Math.cos(azimuth)),
      scale(left, horizontal * Math.sin(azimuth)),
    ),
    scale(up, Math.sin(elevation)),
  );
}

/** What a contact's bearing reads, `047° +12°`, or `047° +12° FROM +X` near the axis. */
export function bearingWords(bearing: BearingDto): string {
  return formatBearing(bearing.azimuth_deg, bearing.elevation_deg, bearing.frame);
}

/** The marks the chart draws for its contacts, added to its scene by {@link withAlertMarks}. */
export interface AlertMarks {
  readonly points: ReadonlyArray<PointMark>;
  readonly segments: ReadonlyArray<SegmentMark>;
}

/** The marks of no contacts. */
const NO_MARKS: AlertMarks = { points: [], segments: [] };

/**
 * The chart's marks of its contacts (plan 12, P12.T12), about the centre of the answer on show.
 *
 * @remarks
 * - A resolved contact whose host lies inside the fetched sphere is the `transient` four-point
 *   star on the host: at the host's own row where the answer holds it, so that the star stands on
 *   its symbol, else at the host's present position, extrapolated at the latest sighting (the wire's
 *   `present_position`), as the chart draws present positions. It is labelled `TRANSIENT`, plain,
 *   since colour is kept for status.
 * - Every other contact with a bearing, a contact not yet resolved and a resolved one whose host
 *   lies outside the sphere, is a solid 1 px `--text` ray from the observer, drawn only in observed
 *   mode, where the chart draws the observer it leaves (P12.T11): a bearing alone, which says no
 *   range, runs to the display's edge and is labelled there `TRANSIENT` and its direction; a
 *   resolved host's runs to where its light left it and is labelled with that range, the light's
 *   own path, in light-years to 0.01 ly ({@link RANGE_DECIMALS}).
 * - A contact with no bearing, seen from its host's own place, draws nothing.
 */
export function alertMarks(contacts: ReadonlyArray<ContactDto>, result: ChartResult): AlertMarks {
  if (contacts.length === 0) {
    return NO_MARKS;
  }
  const centre = galacticPositionFromLy(result.centreLy);
  const relOf = (position: Parameters<typeof galacticDeltaLy>[1]): Vec3 =>
    vec3(...galacticDeltaLy(centre, position));
  const rows = new Map<SystemIdHex, Vec3>(
    result.systems.map((system) => [system.id, system.relLy]),
  );
  const mode = answeredMode(result);
  const observerLy = mode.kind === "observed" ? mode.observerLy : null;
  const observerRel =
    observerLy === null
      ? null
      : vec3(
          observerLy[0] - result.centreLy[0],
          observerLy[1] - result.centreLy[1],
          observerLy[2] - result.centreLy[2],
        );
  const points: PointMark[] = [];
  const segments: SegmentMark[] = [];
  for (const contact of contacts) {
    if (contact.level === "resolved") {
      const host = rows.get(contact.host) ?? relOf(contact.present_position);
      if (norm(host) <= result.radiusLy) {
        points.push({
          id: transientMarkId(contact.contact),
          position: host,
          shape: "transient",
          sizeClass: TRANSIENT_SIZE_CLASS,
          status: "plain",
          label: TRANSIENT_LABEL,
          labelPriority: TRANSIENT_LABEL_PRIORITY - contact.contact,
        });
        continue;
      }
    }
    if (observerRel === null || observerLy === null || contact.bearing === null) {
      continue;
    }
    const ray: Pick<SegmentMark, "line" | "stroke" | "widthPx" | "fromMark"> = {
      line: "solid",
      stroke: "text",
      widthPx: 1,
      fromMark: OBSERVER_MARK_ID,
    };
    switch (contact.level) {
      case "bearing":
        segments.push({
          id: bearingRayId(contact.contact),
          from: observerRel,
          to: add(observerRel, scale(bearingRay(observerLy, contact.bearing), RAY_REACH_LY)),
          ...ray,
          endLabel: `${TRANSIENT_LABEL} ${bearingWords(contact.bearing)}`,
        });
        break;
      case "resolved": {
        const apparent = relOf(contact.apparent_position);
        const rangeLy = norm(sub(apparent, observerRel));
        segments.push({
          id: bearingRayId(contact.contact),
          from: observerRel,
          to: apparent,
          ...ray,
          endLabel: `${TRANSIENT_LABEL} ${formatLengthLy(rangeLy, RANGE_DECIMALS)} ly`,
        });
        break;
      }
    }
  }
  return points.length === 0 && segments.length === 0 ? NO_MARKS : { points, segments };
}

/**
 * The chart's scene with its contacts' marks added: the transients among the points, drawn,
 * picked and labelled as they are, and the rays after the apparent positions' joiners. The scene
 * itself where there are none, so that an idle chart paints nothing new.
 */
export function withAlertMarks(scene: SpatialScene, marks: AlertMarks): SpatialScene {
  if (marks.points.length === 0 && marks.segments.length === 0) {
    return scene;
  }
  const segments = [...(scene.segments ?? []), ...marks.segments];
  return {
    ...scene,
    points: [...scene.points, ...marks.points],
    ...(segments.length === 0 ? {} : { segments }),
  };
}

/**
 * The host a mark picked on the chart stands for, where it is a transient's: its contact's host,
 * so that picking the star selects the system it stands on; `null` for any other mark.
 */
export function transientHost(
  markId: string,
  contacts: ReadonlyArray<ContactDto>,
): SystemIdHex | null {
  if (!markId.startsWith(TRANSIENT_MARK_PREFIX)) {
    return null;
  }
  const number = Number(markId.slice(TRANSIENT_MARK_PREFIX.length));
  const contact = contacts.find((each) => each.contact === number);
  return contact?.level === "resolved" ? contact.host : null;
}

/** What a scene draws of the contacts, which the legend names: transients, and bearing rays. */
export interface AlertLegend {
  readonly transients: boolean;
  readonly bearings: boolean;
}

/** Whether a scene draws a transient's mark and a bearing ray, for the legend. */
export function alertLegendOf(scene: SpatialScene | null): AlertLegend {
  return {
    transients: scene?.points.some((point) => point.shape === "transient") ?? false,
    bearings:
      scene?.segments?.some((segment) => segment.id.startsWith(BEARING_RAY_PREFIX)) ?? false,
  };
}
