/**
 * The `SYSTEM` display's system note: what this generator version does not model, and what the
 * granted detail level withholds, said so that the empty space beyond what is drawn is never read as
 * empty (the orchestrator's rulings 33, 34 and 113.2; plan 14, P14.T41.b).
 */
import type { SectionDto } from "@hyperion/protocol";

import { type SectionStandIn, sectionStateLabel } from "../../lib/system/bodyWords";
import type { SystemBodies } from "../../lib/system/model";

/** The state a record's section is tagged with, which the server sets and the client never infers. */
export type SectionState = SectionDto<unknown>["state"];

/** A planet's or a dwarf planet's tags that the note reads: its moons, and a planet's rings. */
export interface PlanetSections {
  readonly moons: SectionState;
  /**
   * A planet's rings; `null` for a dwarf planet, whose rings the note does not read, since no
   * generator version gives dwarf planets rings and its own readout says `NOT YET MODELLED`.
   */
  readonly rings: SectionState | null;
}

/**
 * The tags the note is composed from: belts and the cometary halo on the system's record, moons and
 * rings on each planet's (ruling 34.3).
 */
export interface SmallBodySections {
  readonly belts: SectionState;
  readonly halo: SectionState;
  readonly planets: ReadonlyArray<PlanetSections>;
}

/**
 * What the display holds of the bodies beyond its hosts.
 *
 * @remarks
 * `tagged` is the server's word on each section, from `system_bodies`. `unserved` is a server that
 * answers `system_bodies` with `unsupported`, as one built before P14.T36 does: no planet data
 * reaches the console, so the planets are as absent from the display as the generator's
 * `not_modelled` sections, and the note names them too, until the bodies arrive and the tags take
 * over (the orchestrator's ruling 59.1).
 */
export type BodiesKnown =
  { readonly kind: "unserved" } | { readonly kind: "tagged"; readonly sections: SmallBodySections };

/** The states the note names, in its lines' order: not modelled first, then withheld. */
const STAND_INS: ReadonlyArray<SectionStandIn> = ["not_modelled", "not_resolved"];

/** `A`, `A AND B`, `A, B AND C`: a list of names as the note reads it. */
function joinNames(names: ReadonlyArray<string>): string {
  if (names.length <= 1) {
    return names.join("");
  }
  return `${names.slice(0, -1).join(", ")} AND ${names.at(-1) ?? ""}`;
}

/**
 * The small-body kinds whose tags say `state`, in the note's order: moons when any planet's or dwarf
 * planet's are, rings when any planet's are, then belts and the halo from the system's own tags.
 */
function kindsIn(sections: SmallBodySections, state: SectionStandIn): string[] {
  const { belts, halo, planets } = sections;
  const names: string[] = [];
  if (planets.some((planet) => planet.moons === state)) {
    names.push("MOONS");
  }
  if (planets.some((planet) => planet.rings === state)) {
    names.push("RINGS");
  }
  if (belts === state) {
    names.push("BELTS");
  }
  if (halo === state) {
    names.push("COMETARY HALO");
  }
  return names;
}

/**
 * The system note's lines, one per section state, or none when the display withholds nothing.
 *
 * @remarks
 * The first line names the kinds this generator version does not model, the second those the
 * granted detail level withholds, so that neither is read as "none" (the guide's data states;
 * ruling 113.2). Each kind is named when its tag says so, and drops out as the server starts to
 * model or resolve it: moons when any planet's or dwarf planet's say so, rings when any planet's do
 * (a dwarf planet's rings are read on its own readout, so that a belt's members do not hold the word
 * for ever), belts and the halo from the system's own tags. Not applicable is never named. Each line
 * is a field label followed by the state, both in upper case:
 * `MOONS, RINGS, BELTS AND COMETARY HALO: NOT YET MODELLED`, then
 * `BELTS AND COMETARY HALO: NOT RESOLVED`, which at v12 only `contact` gives. A server that cannot
 * serve the bodies has no tags, so its one line names the planets with every small-body kind.
 */
export function systemNotes(bodies: BodiesKnown): ReadonlyArray<string> {
  if (bodies.kind === "unserved") {
    return [`PLANETS, MOONS, RINGS, BELTS AND COMETARY HALO: ${sectionStateLabel("not_modelled")}`];
  }
  const notes: string[] = [];
  for (const state of STAND_INS) {
    const names = kindsIn(bodies.sections, state);
    if (names.length > 0) {
      notes.push(`${joinNames(names)}: ${sectionStateLabel(state)}`);
    }
  }
  return notes;
}

/**
 * The tags a system's bodies carry for the note: the system's belts and halo, each planet's and
 * dwarf planet's moons, and each planet's rings (the orchestrator's ruling 34.3).
 */
export function smallBodySections(bodies: SystemBodies): SmallBodySections {
  return {
    belts: bodies.belts.state,
    halo: bodies.halo.state,
    planets: bodies.bodies
      .filter((body) => body.kind.kind === "planet" || body.kind.kind === "dwarf_planet")
      .map((body) => ({
        moons: body.moons.state,
        rings: body.kind.kind === "planet" ? body.rings.state : null,
      })),
  };
}
