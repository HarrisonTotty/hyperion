/**
 * The terrain setting's two kinds that the height workers also read (plan R05, Design notes 4, 5
 * and 25), in a module of their own with no imports, so that the worker's program
 * (`tsconfig.worker.json`) can include it without the quality settings' render-thread imports.
 *
 * @remarks
 * `qualitySetting.ts` re-exports both; import them from there everywhere but worker-side code.
 */

/** Where the terrain's normals are evaluated (R05 Design notes 5 and 25). */
export type TerrainNormals =
  /** Twice the mesh's resolution, 129 × 129 a patch: four times the gradients. */
  | "double"
  /** The mesh's own resolution, 65 × 65 a patch. */
  | "mesh";

/** How the vertex stage forms a terrain vertex's camera-relative position (R05 Design note 4). */
export type TerrainVertexPath =
  /** Offsets from the patch origin baked in `f64` by the worker and narrowed to `f32`. */
  | "baked-offsets"
  /** Direction differences formed from small quantities in the shader, from the heights alone. */
  | "face-differences";
