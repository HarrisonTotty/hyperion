import type {
  BodyIdHex,
  GalacticPosition,
  HostDiscDto,
  SystemIdHex,
  UniverseTime,
} from "@hyperion/protocol";

import type { Vec3 } from "../../geometry/vec3";
import type { KeplerOrbit } from "../../lib/orbit";
import {
  BODY_MIN_SIZE_CLASS,
  CONTACT_SIZE_CLASS,
  MOON_SIZE_CLASS,
  PLANET_SIZE_CLASSES,
} from "../../lib/system/bodySymbols";
import type { SizeClass, SymbolShape } from "../../spatial/marks";
import type { CameraPose, CraftId, Quaternion } from "../camera/pose";
import type { CameraScene, CameraTarget } from "../camera/state";
import type { ViewPosition } from "../coords/position";
import type { CameraOrigins } from "../coords/relative";
import type { Rotation3 } from "../coords/rotation";
import type { HullOutline } from "./hull";

/**
 * What a body is, as the wireframe marks it below 3 px and the list names it: a planet, a dwarf
 * planet, a moon or a star, or an unresolved contact, whose kind the server withholds (R02.T17).
 */
export type ViewBodyKind = "star" | "planet" | "dwarf_planet" | "moon" | "unresolved";

/**
 * Where a body was when the light the camera sees left it: its retarded geometric centre in the
 * system frame, without aberration, which lighting takes where drawing takes the apparent place
 * (plan R07, T10.a; decision-r07-t8a, follow-up (a)).
 *
 * @remarks
 * The ship's local body is drawn at the present but lit at this retarded time too, as every
 * time-varying state is drawn (the brainstorm's "the local body's included"; ruled for T10.a).
 */
export interface RetardedCentre {
  /**
   * Where the body was when the light the camera sees left it, m from the barycentre, galactic
   * axes.
   */
  readonly centreM: Vec3;
  /** Its velocity relative to the barycentre then, m/s along the galactic axes. */
  readonly velocityMPerS: Vec3;
  /**
   * Light time from the body to the camera, s: the time it is lit at is the scene's less it. It is
   * 0 in a kept scene.
   */
  readonly lightTimeS: number;
}

/** A kept scene's body, at rest where it is drawn: no light time and no motion (R07.T10.a). */
export function staticRetarded(centreM: Vec3): RetardedCentre {
  return { centreM, velocityMPerS: { x: 0, y: 0, z: 0 }, lightTimeS: 0 };
}

/**
 * A body of the view's scene at the scene's time.
 *
 * @remarks
 * Only planets and moons with a Hill radius are camera-frame candidates (Design note 6); a star, or
 * a body whose Hill radius is not known (`null`, below `mass_and_orbit` or placed by `seen`), is
 * not.
 */
export interface ViewBody {
  /** The body. */
  readonly id: BodyIdHex;
  /** The body it orbits, or `null`. */
  readonly parent: BodyIdHex | null;
  /** What it is. */
  readonly kind: ViewBodyKind;
  /** Its designation as the view labels it. */
  readonly designation: string;
  /** Its (equatorial) radius, m. */
  readonly radiusM: number;
  /** Its Hill radius at pericentre, m, or `null` where it is not known. */
  readonly hillRadiusM: number | null;
  /** Its centre as drawn, m from the system's barycentre along the galactic axes. */
  readonly centreM: Vec3;
  /**
   * Its retarded centre, which lighting takes (R07.T10.a): a kept scene's is
   * {@link staticRetarded} at its drawn centre; `null` only for a contact, which never lights,
   * occludes or is eclipsed.
   */
  readonly retarded: RetardedCentre | null;
  /** Its rotation from body-fixed to body axes, or `null` where rotation is not modelled. */
  readonly rotation: Rotation3 | null;
  /**
   * The unit normal of its orbit, along the galactic axes, which stands for its pole while its
   * rotation is not modelled (Design note 14); `null` where it has no orbit drawn from elements,
   * when the frame's +z stands in.
   */
  readonly orbitNormal: Vec3 | null;
  /** Its mark from the ship-wide symbol set, drawn below 3 px (`lib/system/bodySymbols.ts`). */
  readonly symbol: BodyMarkSymbol;
}

/** Whether a scene body is lit in the photorealistic style: a planet, dwarf planet or moon. */
export function isLitKind(kind: ViewBodyKind): boolean {
  let lit: boolean;
  switch (kind) {
    case "planet":
    case "dwarf_planet":
    case "moon":
      lit = true;
      break;
    case "star":
    case "unresolved":
      lit = false;
      break;
  }
  return lit;
}

/** A body's mark in the ship-wide symbol set: its shape and its size class. */
export interface BodyMarkSymbol {
  /** The shape, which encodes the kind. */
  readonly shape: SymbolShape;
  /** The size class. */
  readonly sizeClass: SizeClass;
}

/**
 * The ship-wide symbol of each kind with no more known of the body (`lib/system/bodySymbols.ts`): a
 * planet the inverted triangle at a smaller planet's size, a dwarf planet the same, its own class
 * raised to the floor, a moon the pentagon, a star the circle at size class 2, an unresolved
 * contact the hexagon. A server scene gives each body its own (R02.T17: a giant's larger triangle,
 * a host's own symbol).
 */
export function bodyKindSymbol(kind: ViewBodyKind): BodyMarkSymbol {
  let symbol: BodyMarkSymbol;
  switch (kind) {
    case "planet":
      symbol = { shape: "triangle-down", sizeClass: PLANET_SIZE_CLASSES.planet };
      break;
    case "dwarf_planet":
      // A dwarf planet's class, 0, is raised to the floor every body symbol is drawn at.
      symbol = { shape: "triangle-down", sizeClass: BODY_MIN_SIZE_CLASS };
      break;
    case "moon":
      symbol = { shape: "pentagon", sizeClass: MOON_SIZE_CLASS };
      break;
    case "star":
      symbol = { shape: "circle", sizeClass: 2 };
      break;
    case "unresolved":
      symbol = { shape: "hexagon", sizeClass: CONTACT_SIZE_CLASS };
      break;
  }
  return symbol;
}

/** A body's rings: an annulus in the body's equatorial plane. */
export interface ViewRing {
  /** The ringed body. */
  readonly body: BodyIdHex;
  /** The inner edge's radius, m. */
  readonly innerRadiusM: number;
  /** The outer edge's radius, m. */
  readonly outerRadiusM: number;
  /** The ring plane's unit normal, along the galactic axes. */
  readonly normal: Vec3;
}

/** An orbit the view draws on request: a body's Kepler orbit about its parent. */
export interface ViewOrbit {
  /** The orbiting body. */
  readonly body: BodyIdHex;
  /** The body at the orbit's focus, or `null` for the barycentre. */
  readonly parent: BodyIdHex | null;
  /** The elements, propagated by `lib/orbit.ts`. */
  readonly orbit: KeplerOrbit;
}

/** A craft's pose at a moment: where it is and how it is turned. */
export interface CraftPose {
  /** Where the craft is. */
  readonly position: ViewPosition;
  /** The rotation from hull axes to the galactic axes. */
  readonly attitude: Quaternion;
}

/** A craft of the scene with its hull and, where it has one, its predicted path. */
export interface ViewCraft {
  /** The craft. */
  readonly id: CraftId;
  /** Its designation as the view labels it. */
  readonly designation: string;
  /** Its hull outline: `TEST_HULL` where no outline is known. */
  readonly hull: HullOutline;
  /** Its pose now. */
  readonly pose: CraftPose;
  /** Its predicted path (R03's `predictedPath`), drawn dashed, or `null`. */
  readonly predictedPath: ReadonlyArray<CraftPose> | null;
  /** Its velocity against its frame's reference, m/s along the galactic axes, or `null`. */
  readonly velocityMPerS: Vec3 | null;
}

/** A star of the sky as the view draws it: a sprite by its apparent magnitude. */
export interface ViewStar {
  /** The star's system. */
  readonly id: SystemIdHex;
  /** The unit direction from the scene's barycentre, along the galactic axes. */
  readonly direction: Vec3;
  /** Its distance, m. */
  readonly distanceM: number;
  /** Its absolute V magnitude. */
  readonly absoluteV: number;
  /** Its effective temperature, K, for its colour, or `null`. */
  readonly tEffK: number | null;
}

/**
 * Where a scene came from: a kept test scene, with its name (`PRECISION TEST`,
 * `FRAME CHANGE TEST`), or the server's scene (R02.T17).
 */
export type SceneProvenance =
  { readonly kind: "kept"; readonly name: string } | { readonly kind: "server" };

/**
 * The engine-agnostic scene a view draws, at one time (plan R02, R02.T11.a).
 *
 * @remarks
 * Every position is `f64`; nothing here is camera-relative. A kept scene builds one per frame from
 * its script; R02.T17 builds one from R03's scene.
 */
export interface ViewScene {
  /** Where the scene came from, and a kept scene's name. */
  readonly provenance: SceneProvenance;
  /** The scene's time. */
  readonly time: UniverseTime;
  /** Simulation seconds per real second. */
  readonly timeRate: number;
  /** The scene's system. */
  readonly system: SystemIdHex;
  /**
   * The system's barycentre in the galactic frame, or `null` where the client does not know it:
   * a server scene's messages do not carry it (R02.T17), so it is known only for a system the
   * client has been told of. Without it nothing is expressed in the galactic frame.
   */
  readonly barycentre: GalacticPosition | null;
  /** The system's tidal radius, m, which bounds a free camera (Design note 7). */
  readonly tidalRadiusM: number;
  /** The bodies. */
  readonly bodies: ReadonlyArray<ViewBody>;
  /** The rings. */
  readonly rings: ReadonlyArray<ViewRing>;
  /** The orbits drawn. */
  readonly orbits: ReadonlyArray<ViewOrbit>;
  /** The craft, the own ship among them. */
  readonly craft: ReadonlyArray<ViewCraft>;
  /** The stars. */
  readonly stars: ReadonlyArray<ViewStar>;
  /** The own ship's craft, or `null` where there is none. */
  readonly ownShip: CraftId | null;
  /** The pose a camera with no own ship starts at. */
  readonly defaultPose: CameraPose;
  /**
   * A kept scene's host discs, standing in for the sky's (R07.T8.a, decision-r07-t8a); a server
   * scene leaves it unset and is lit by its held sky's `hosts`.
   */
  readonly hostDiscs?: ReadonlyArray<HostDiscDto>;
}

/**
 * Where every frame's origin and every craft of a scene is: the scene's {@link CameraOrigins}.
 * @throws Error from a lookup, when asked for a body or craft the scene does not have, or for the
 *   barycentre of a scene that does not know it.
 */
export function sceneOrigins(scene: ViewScene): CameraOrigins {
  const bodies = new Map(scene.bodies.map((body) => [body.id, body]));
  const craft = new Map(scene.craft.map((c) => [c.id, c]));
  const bodyOf = (id: BodyIdHex): ViewBody => {
    const body = bodies.get(id);
    if (body === undefined) {
      throw new Error(`the scene has no body ${id}`);
    }
    return body;
  };
  return {
    systemBarycentre: (system) => {
      if (system !== scene.system) {
        throw new Error(`the scene holds system ${scene.system}, not ${system}`);
      }
      if (scene.barycentre === null) {
        throw new Error(`the position of system ${system} is not known`);
      }
      return scene.barycentre;
    },
    bodyCentreM: (id) => bodyOf(id).centreM,
    bodyFixedRotation: (id) => bodyOf(id).rotation,
    craftPosition: (id) => {
      const found = craft.get(id);
      if (found === undefined) {
        throw new Error(`the scene has no craft ${id}`);
      }
      return found.pose.position;
    },
  };
}

/**
 * The scene as the camera sees it (R02.T9's {@link CameraScene}): its frames, its targets (bodies,
 * then craft other than the own ship) and its own ship with its hull's eye point.
 *
 * @throws Error if the scene's own ship is not among its craft.
 */
export function cameraSceneOf(scene: ViewScene): CameraScene {
  const own = scene.craft.find((c) => c.id === scene.ownShip);
  if (scene.ownShip !== null && own === undefined) {
    throw new Error(`the scene's own ship ${scene.ownShip} is not among its craft`);
  }
  const targets: CameraTarget[] = [
    ...scene.bodies.map((body): CameraTarget => ({ kind: "body", body: body.id })),
    ...scene.craft
      .filter((c) => c.id !== scene.ownShip)
      .map((c): CameraTarget => ({ kind: "craft", craft: c.id })),
  ];
  return {
    system: scene.system,
    tidalRadiusM: scene.tidalRadiusM,
    origins: sceneOrigins(scene),
    frameBodies: scene.bodies.flatMap((body) =>
      body.kind !== "star" && body.hillRadiusM !== null
        ? [{ id: body.id, parent: body.parent, hillRadiusM: body.hillRadiusM }]
        : [],
    ),
    targets,
    ownShip:
      own === undefined
        ? null
        : {
            craft: own.id,
            attitude: own.pose.attitude,
            eyePointM: own.hull.eyePointM,
            lengthM: own.hull.lengthM,
          },
    defaultPose: scene.defaultPose,
  };
}
