/**
 * The `SYSTEM` display's system note: what this generator version does not model, said so that the
 * empty space beyond what is drawn is never read as empty (the orchestrator's rulings 33 and 34;
 * plan 14, P14.T41.b).
 */
import type { SectionDto } from "@hyperion/protocol";

/** The state a record's section is tagged with, which the server sets and the client never infers. */
export type SectionState = SectionDto<unknown>["state"];

/** A planet's tags that the note reads: its moons and its rings. */
export interface PlanetSections {
  readonly moons: SectionState;
  readonly rings: SectionState;
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
 * `tagged` is the server's word on each section, from `system_bodies`. `unserved` is this client's
 * protocol having no way to ask for any body but the stars, which no server it speaks to can
 * answer: the planets are then as absent from the display as the generator's `not_modelled`
 * sections, and the note names them too, until `system_bodies` exists and the tags take over.
 */
export type BodiesKnown =
  { readonly kind: "unserved" } | { readonly kind: "tagged"; readonly sections: SmallBodySections };

/** The phrase that follows the named kinds (the owner's draft of the guide's data states). */
export const NOT_YET_MODELLED = "NOT YET MODELLED";

/** `A`, `A AND B`, `A, B AND C`: a list of names as the note reads it. */
function joinNames(names: ReadonlyArray<string>): string {
  if (names.length <= 1) {
    return names.join("");
  }
  return `${names.slice(0, -1).join(", ")} AND ${names.at(-1) ?? ""}`;
}

/**
 * The system note, or `null` when everything the display could show is modelled.
 *
 * @remarks
 * Each kind is named when its tag says `not_modelled`, and drops out as the server starts to model
 * it: moons and rings when any planet's are not modelled, belts and the halo from the system's own
 * tags. `NOT RESOLVED` and not applicable are not named, since neither says the generator lacks the
 * model. The note is a field label followed by the three-word state, both in upper case, as the
 * owner's draft reads it: `MOONS, RINGS, BELTS AND COMETARY HALO: NOT YET MODELLED`.
 */
export function systemNote(bodies: BodiesKnown): string | null {
  const names: string[] = [];
  if (bodies.kind === "unserved") {
    names.push("PLANETS", "MOONS", "RINGS", "BELTS", "COMETARY HALO");
  } else {
    const { belts, halo, planets } = bodies.sections;
    if (planets.some((planet) => planet.moons === "not_modelled")) {
      names.push("MOONS");
    }
    if (planets.some((planet) => planet.rings === "not_modelled")) {
      names.push("RINGS");
    }
    if (belts === "not_modelled") {
      names.push("BELTS");
    }
    if (halo === "not_modelled") {
      names.push("COMETARY HALO");
    }
  }
  return names.length === 0 ? null : `${joinNames(names)}: ${NOT_YET_MODELLED}`;
}
