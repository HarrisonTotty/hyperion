/**
 * Contacts and census lines as the server sends them (plan 12, P12.T9.a), for the alert list's,
 * the alerts hook's and the chart marks' tests (P12.T12).
 */
import {
  type ClassCensusDto,
  type ContactDto,
  galacticPositionFromLy,
  type NotificationOf,
  universeTimeFromYears,
} from "@hyperion/protocol";

/** What a contact at `bearing` level carries, without its tag. */
type BearingFields = Omit<Extract<ContactDto, { level: "bearing" }>, "level">;

/** What a resolved contact carries, without its tag. */
type ResolvedFields = Omit<Extract<ContactDto, { level: "resolved" }>, "level">;

/**
 * A contact not yet resolved: number 1, first seen at UT +0.50 yr and last at +0.52 yr, from
 * 047° +12° in the observer's local frame, 3.20 × 10⁻¹² W m⁻² in V, at its onset, unacknowledged.
 */
export function aBearingContact(overrides: Partial<BearingFields> = {}): ContactDto {
  return {
    level: "bearing",
    contact: 1,
    kind: "unclassified",
    first_seen: universeTimeFromYears(0.5),
    last_seen: universeTimeFromYears(0.52),
    bearing: { azimuth_deg: 47, elevation_deg: 12, frame: "local" },
    flux_w_m2: 3.2e-12,
    band: "v",
    phase: "onset",
    acknowledged: false,
    ...overrides,
  };
}

/** The host of {@link aResolvedContact}, a pinned recurrent nova's stand-in. */
export const NOVA_HOST = "00000000000000c3";

/** Its designation, as the wire derives it from the ID. */
export const NOVA_DESIGNATION = "HYP C3-NOVA";

/**
 * A resolved nova: number 2, its host {@link NOVA_HOST} at `hostLy` (apparent and present alike),
 * 4210 yr of light age, seen from 090° +00°, 5.0 × 10⁻¹³ W m⁻² in X-rays, in progress.
 */
export function aResolvedContact(
  hostLy: readonly [number, number, number],
  overrides: Partial<ResolvedFields> = {},
): ContactDto {
  return {
    level: "resolved",
    contact: 2,
    kind: "nova",
    first_seen: universeTimeFromYears(0.25),
    last_seen: universeTimeFromYears(0.5),
    bearing: { azimuth_deg: 90, elevation_deg: 0, frame: "local" },
    flux_w_m2: 5e-13,
    band: "x_ray",
    phase: "in_progress",
    acknowledged: false,
    host: NOVA_HOST,
    designation: NOVA_DESIGNATION,
    light_age_yr: 4210,
    apparent_position: galacticPositionFromLy(hostLy),
    present_position: galacticPositionFromLy(hostLy),
    curvature_error_ly: 0.01,
    ...overrides,
  };
}

/** A class's census line. */
export function aCensusLine(
  status: ClassCensusDto["status"],
  className: ClassCensusDto["class"] = "accreting_white_dwarf",
): ClassCensusDto {
  return { class: className, status, reach_ly: 12_000, expected_hosts: 40 };
}

/** An alerts notification numbered `sequence`, carrying `contacts` and `census`. */
export function alertsNotification(
  sequence: number,
  contacts: ReadonlyArray<ContactDto>,
  census: ReadonlyArray<ClassCensusDto> = [],
): NotificationOf<"alerts"> {
  return { topic: "alerts", sequence, contacts: [...contacts], census: [...census] };
}
