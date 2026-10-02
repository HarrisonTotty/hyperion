import { add, cross, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import type { ProjectionCamera, Viewport } from "../camera/projection";
import {
  IDENTITY_ROTATION,
  type Rotation3,
  rotateToBody,
  rotation3FromRows,
} from "../coords/rotation";
import { cosineWindows, type Polyline, sampleCurve } from "./curve";

/**
 * How a body is drawn at its size on screen (plan R02, Design note 13): below 3 px its symbol, from
 * 3 to 8 px its limb circle alone, at 8 px and above its limb and graticule.
 */
export type BodyRegime = "symbol" | "limb" | "graticule";

/** The apparent diameter below which a body is its symbol, px: 3, the brainstorm's point regime. */
export const SYMBOL_BELOW_PX = 3;

/** The apparent diameter from which a body has its graticule, px: 8, where 12 lines stop being lines. */
export const GRATICULE_FROM_PX = 8;

/** The apparent diameter from which the graticule is drawn at 15° rather than 30°, px: 64. */
export const FINE_GRATICULE_FROM_PX = 64;

/** The graticule's step, degrees, on a large body and on a small one (and at the low setting). */
export const GRATICULE_STEP_DEG = { fine: 15, coarse: 30 } as const;

/** A body's regime by its apparent diameter. */
export function bodyRegime(diameterPx: number): BodyRegime {
  if (diameterPx < SYMBOL_BELOW_PX) {
    return "symbol";
  }
  return diameterPx < GRATICULE_FROM_PX ? "limb" : "graticule";
}

/**
 * A sphere's apparent diameter on a view, px: 2 asin(r ÷ d) at the view's centre-pixel scale, or
 * infinite from inside it.
 *
 * @param centreM - The sphere's centre from the camera, m.
 */
export function angularDiameterPx(
  centreM: Vec3,
  radiusM: number,
  camera: ProjectionCamera,
  viewport: Viewport,
): number {
  const distanceM = norm(centreM);
  if (!(distanceM > radiusM)) {
    return Number.POSITIVE_INFINITY;
  }
  const pxPerRad = viewport.widthPx / (2 * Math.tan(camera.fovXRad / 2));
  return 2 * Math.asin(radiusM / distanceM) * pxPerRad;
}

/** A body as the wireframe's geometry needs it. */
export interface GraticuleBody {
  /** The body's centre from the camera, m along the camera frame's axes. */
  readonly centreM: Vec3;
  /** Its radius, m. */
  readonly radiusM: number;
  /**
   * Its rotation from body-fixed to body axes, or `null` where rotation is not modelled: the
   * graticule then does not turn (Design note 14).
   */
  readonly rotation: Rotation3 | null;
  /**
   * The pole a body whose rotation is not modelled is drawn about: its orbit normal (Design note
   * 14, as `displays/system/bodyFrame.ts` assumes), or `null` for the frame's +z.
   */
  readonly unmodelledPole: Vec3 | null;
}

/**
 * The rotation a graticule is drawn in: the body's own, or where it is not modelled one whose pole
 * is the given pole (the orbit normal) and whose prime meridian is fixed, so that it does not turn.
 */
export function graticuleRotation(
  body: Pick<GraticuleBody, "rotation" | "unmodelledPole">,
): Rotation3 {
  if (body.rotation !== null) {
    return body.rotation;
  }
  if (body.unmodelledPole === null) {
    return IDENTITY_ROTATION;
  }
  const z = normalise(body.unmodelledPole);
  const [x, y] = perpendicularPair(z);
  // Columns are the body-fixed axes in the frame, so the rows are their components.
  return rotation3FromRows([vec3(x.x, y.x, z.x), vec3(x.y, y.y, z.y), vec3(x.z, y.z, z.z)]);
}

/** One line of a body's wireframe: its limb, a meridian or a parallel. */
export interface GraticuleLine {
  /** What the line is. */
  readonly kind: "limb" | "meridian" | "parallel";
  /** Whether it is drawn a step heavier: the equator and the prime meridian. */
  readonly major: boolean;
  /** Its visible runs, camera-relative. */
  readonly runs: ReadonlyArray<Polyline>;
}

/** A body's wireframe at its regime. */
export interface BodyWireframe {
  /** The regime the body is drawn in. */
  readonly regime: BodyRegime;
  /** Its apparent diameter, px. */
  readonly diameterPx: number;
  /** Its lines: none as a symbol, the limb alone, or the limb and graticule. */
  readonly lines: ReadonlyArray<GraticuleLine>;
}

/** The unit vector of latitude `latRad` and longitude `lonRad` along the body-fixed axes. */
function unitAt(latRad: number, lonRad: number): Vec3 {
  const c = Math.cos(latRad);
  return vec3(c * Math.cos(lonRad), c * Math.sin(lonRad), Math.sin(latRad));
}

/** Two unit vectors perpendicular to `axis` and to each other. */
export function perpendicularPair(axis: Vec3): readonly [Vec3, Vec3] {
  const n = normalise(axis);
  const helper = Math.abs(n.z) < 0.9 ? vec3(0, 0, 1) : vec3(1, 0, 0);
  const e1 = normalise(cross(helper, n));
  return [e1, cross(n, e1)];
}

/**
 * The limb circle: the sphere's silhouette from the camera, the true horizon, subdivided to the
 * sagitta tolerance.
 */
function limb(body: GraticuleBody, camera: ProjectionCamera, viewport: Viewport): Polyline[] {
  const d = norm(body.centreM);
  const toCentre = scale(body.centreM, 1 / d);
  const [u, v] = perpendicularPair(toCentre);
  const along = -(body.radiusM * body.radiusM) / d;
  const across = body.radiusM * Math.sqrt(1 - (body.radiusM / d) ** 2);
  const centreOffset = scale(toCentre, along);
  return sampleCurve(
    {
      point: (theta) =>
        add(
          body.centreM,
          add(
            centreOffset,
            add(scale(u, across * Math.cos(theta)), scale(v, across * Math.sin(theta))),
          ),
        ),
      windows: [[0, 2 * Math.PI]],
      initialSpans: 32,
    },
    camera,
    viewport,
  );
}

/**
 * A body's wireframe (plan R02, Design note 13): by its apparent size, nothing (its symbol is the
 * symbology's), its limb circle, or its limb and a graticule at 15° (30° below 64 px, and at the
 * low setting), the equator and prime meridian a step heavier.
 *
 * @remarks
 * Every point is generated in `f64`, camera-relative, as c + r · R · n, and subdivided until each
 * chord's sagitta is under 0.25 px. The graticule's far hemisphere is removed analytically: a
 * point is drawn only where its outward normal faces the camera (−n · c > r), which is exact for a
 * sphere and needs no depth; each line's facing arc is found in closed form (`cosineWindows`), so a
 * short visible arc near the limb is never missed. The graticule turns with the body through its
 * rotation; with none it is drawn about the orbit normal and does not turn (Design note 14).
 *
 * @param lowSetting - The wireframe's low setting, which draws graticules at 30° only.
 */
export function graticule(
  body: GraticuleBody,
  camera: ProjectionCamera,
  viewport: Viewport,
  lowSetting = false,
): BodyWireframe {
  const diameterPx = angularDiameterPx(body.centreM, body.radiusM, camera, viewport);
  const regime = bodyRegime(diameterPx);
  if (regime === "symbol") {
    return { regime, diameterPx, lines: [] };
  }
  const lines: GraticuleLine[] = [
    { kind: "limb", major: false, runs: limb(body, camera, viewport) },
  ];
  if (regime === "limb") {
    return { regime, diameterPx, lines };
  }
  const rotation = graticuleRotation(body);
  const onSphere = (latRad: number, lonRad: number): Vec3 =>
    rotateToBody(rotation, unitAt(latRad, lonRad));
  // The body's axes turned into the frame, and each one's facing component −axis · c.
  const xAxis = rotateToBody(rotation, vec3(1, 0, 0));
  const yAxis = rotateToBody(rotation, vec3(0, 1, 0));
  const zAxis = rotateToBody(rotation, vec3(0, 0, 1));
  const [fx, fy, fz] = [xAxis, yAxis, zAxis].map((axis) => -dot(axis, body.centreM));
  const facingX = fx ?? 0;
  const facingY = fy ?? 0;
  const facingZ = fz ?? 0;
  const pointOf = (n: Vec3): Vec3 => add(body.centreM, scale(n, body.radiusM));
  const stepDeg =
    lowSetting || diameterPx < FINE_GRATICULE_FROM_PX
      ? GRATICULE_STEP_DEG.coarse
      : GRATICULE_STEP_DEG.fine;
  const stepRad = (stepDeg * Math.PI) / 180;
  const count = Math.round(360 / stepDeg);
  for (let k = 0; k < count; k += 1) {
    const lonRad = k * stepRad;
    lines.push({
      kind: "meridian",
      major: k === 0,
      runs: sampleCurve(
        {
          point: (lat) => pointOf(onSphere(lat, lonRad)),
          // n = cos φ (cos λ x + sin λ y) + sin φ z faces the camera where −n · c > r.
          windows: cosineWindows(
            Math.cos(lonRad) * facingX + Math.sin(lonRad) * facingY,
            facingZ,
            body.radiusM,
            -Math.PI / 2,
            Math.PI / 2,
          ),
          initialSpans: 12,
        },
        camera,
        viewport,
      ),
    });
  }
  for (let k = 1; k < count / 2; k += 1) {
    const latRad = -Math.PI / 2 + k * stepRad;
    lines.push({
      kind: "parallel",
      major: Math.abs(latRad) < 1e-12,
      runs: sampleCurve(
        {
          point: (lon) => pointOf(onSphere(latRad, lon)),
          windows: cosineWindows(
            Math.cos(latRad) * facingX,
            Math.cos(latRad) * facingY,
            body.radiusM - Math.sin(latRad) * facingZ,
            0,
            2 * Math.PI,
          ),
          initialSpans: 24,
        },
        camera,
        viewport,
      ),
    });
  }
  return { regime, diameterPx, lines };
}

/** A body's rings as the wireframe draws them. */
export interface RingGeometry {
  /** The ringed body's centre from the camera, m. */
  readonly centreM: Vec3;
  /** The inner edge's radius, m. */
  readonly innerRadiusM: number;
  /** The outer edge's radius, m. */
  readonly outerRadiusM: number;
  /** The ring plane's unit normal. */
  readonly normal: Vec3;
}

/** The angle between a ring's radial ticks, degrees: 10, as the orbit map draws them. */
export const RING_TICK_STEP_DEG = 10;

/**
 * A ring's ellipses as the camera sees them: its inner and outer edges, subdivided to the sagitta
 * tolerance, and a radial tick every 10° from one to the other (plan R02, Design note 13).
 */
export function ringEllipse(
  ring: RingGeometry,
  camera: ProjectionCamera,
  viewport: Viewport,
): ReadonlyArray<Polyline> {
  const [e1, e2] = perpendicularPair(ring.normal);
  const at = (radiusM: number, theta: number): Vec3 =>
    add(
      ring.centreM,
      add(scale(e1, radiusM * Math.cos(theta)), scale(e2, radiusM * Math.sin(theta))),
    );
  const edge = (radiusM: number): Polyline[] =>
    sampleCurve(
      {
        point: (t) => at(radiusM, t),
        windows: [[0, 2 * Math.PI]],
        initialSpans: 36,
      },
      camera,
      viewport,
    );
  const ticks: Polyline[] = [];
  for (let deg = 0; deg < 360; deg += RING_TICK_STEP_DEG) {
    const theta = (deg * Math.PI) / 180;
    const inner = at(ring.innerRadiusM, theta);
    const outer = at(ring.outerRadiusM, theta);
    // A straight tick, sampled only so that a part behind the near plane is cut away.
    ticks.push(
      ...sampleCurve(
        {
          point: (t) => add(inner, scale(sub(outer, inner), t)),
          windows: [[0, 1]],
          initialSpans: 1,
        },
        camera,
        viewport,
      ),
    );
  }
  return [...edge(ring.innerRadiusM), ...edge(ring.outerRadiusM), ...ticks];
}
