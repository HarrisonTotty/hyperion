// The authoritative rocks' lit draw (plan R11, T4.b; plan R14, R14.T5.c): each vertex of a finest
// patch's rock mesh decoded from its page (`rockPage.wgsl`), a template vertex past its cluster's
// last triangle placed outside the clip volume, one point for all three corners.
//
// The vertex is placed as the patch's own are (`terrainPatch.wgsl`'s rule): its offset from the
// instance's origin, the patch's origin and its pages' origin less the camera, narrowed once from
// `f64` on the CPU in the non-rotating axes, turned from the body-fixed axes by the body's
// rotation, so that nothing reaches the GPU in world coordinates; its position `@invariant`.
//
// Each fragment is lit as the terrain under its rock is (`terrainLit.wgsl`'s `shade_terrain`, the
// lit material's own shading, to the call): the patch's class weights and maturity and its horizon
// map at the rock's place in it, the rock's own normal for the shading and the horizon term, the
// datum's normal at the rock's axis for R08's lookups, the fragment's height above the datum
// along it from the ground at the axis, and the shadow cascades' receiver at the fragment's place
// with its geometric normal. A rock is shaded by its material alone, its ground's (R11 Design note
// 3: no decoration). The meter class is the terrain's. Composed after the lit terrain's source,
// its `Draw` with `clusterBase` (`terrainMaterial.ts`' `withClusterBase`), its vertex and fragment
// stages renamed into functions, the cascades' receiver, the path's source and `rockPage.wgsl`
// (`rockMaterial.ts`).

struct RockVertexOut {
  @builtin(position) @invariant clip : vec4f,
  // The vertex less the camera, body-fixed, metres.
  @location(0) fromCameraBf : vec3f,
  // The rock's outward normal there, body-fixed.
  @location(1) normal : vec3f,
  // Its height above the datum, metres: the ground's at the axis and its rise along the datum's
  // normal there.
  @location(2) heightM : f32,
  // The datum's normal at the rock's axis, body-fixed.
  @location(3) @interpolate(flat) datum : vec3f,
  // The rock's place in its patch, (x, y) ÷ 64, its cell coordinates there, and its patch's
  // terrain slot and horizon face.
  @location(4) @interpolate(flat) place : vec2f,
  @location(5) @interpolate(flat) horizonSt : vec2f,
  @location(6) @interpolate(flat) slots : vec2u,
}

// The rock vertex of template vertex `local` ((corner, triangle, 0)) of a bucket's entry `entry`.
fn rock_vertex(local : vec3f, entry : u32) -> RockVertexOut {
  let item = cluster_lists[draw.clusterBase + entry];
  let instance = rock_instances[item >> 8u];
  let corner = rock_corner(instance, item & 0xffu, u32(local.y), u32(local.x));
  let rock = rock_records[instance.firstRock + corner.rock];
  let rotation = mat3x3f(draw.bodyRotation[0].xyz, draw.bodyRotation[1].xyz, draw.bodyRotation[2].xyz);
  let fromCameraM = instance.originM + rotation * corner.offset;
  let rec = slots[instance.terrainSlot];
  var out : RockVertexOut;
  out.clip = frame.clipProjection * frame.viewRotation * vec4f(fromCameraM, 1.0);
  if (!corner.drawn) {
    out.clip = vec4f(0.0, 0.0, 2.0, 1.0);
  }
  out.fromCameraBf = transpose(rotation) * fromCameraM;
  out.normal = corner.normal;
  out.datum = rock.datum;
  out.place = unpack2x16unorm(rock.place);
  out.horizonSt = horizon_face_st_at(rec, out.place * 64.0);
  out.slots = vec2u(instance.terrainSlot, horizon_face_of(rec));
  // The corner from its rock's base, both about the patch's origin, along the datum's normal there.
  out.heightM = rock.groundM + dot(instance.pageOriginM + corner.offset - rock.baseM, rock.datum);
  return out;
}

// The lit terrain's fragment input for the rock fragment `in`: its place in its patch, its slot,
// no morph, rebake or class sets beyond its own, the grid's kind.
fn rock_lit_in(in : RockVertexOut) -> LitVertexOut {
  var lit : LitVertexOut;
  lit.clip = in.clip;
  lit.patchUv = in.place;
  lit.slot = in.slots.x;
  lit.fromCameraBf = in.fromCameraBf;
  lit.datumNormal = in.datum;
  lit.heightM = in.heightM;
  lit.horizonSt = in.horizonSt;
  lit.morph = 0.0;
  lit.horizonFace = in.slots.y;
  lit.parent = vec2u(NO_PARENT_SLOT, 0u);
  lit.kind = LIT_GRID;
  lit.gridHeightM = in.heightM;
  lit.lipM = 0.0;
  lit.rebakeSlot = in.slots.x;
  lit.rebakeFade = 0.0;
  lit.cells = vec4u(NO_PARENT_SLOT, 0u, 0u, 0u);
  lit.sub = 0u;
  return lit;
}

// The rock fragment `in`'s light, pre-exposed, with the meter class in alpha.
fn rock_fragment(in : RockVertexOut, geometric : vec3f) -> vec4f {
  let lit = rock_lit_in(in);
  let normal = normalize(in.normal);
  let rotation = mat3x3f(draw.bodyRotation[0].xyz, draw.bodyRotation[1].xyz, draw.bodyRotation[2].xyz);
  let sun = terrain_row(FIRST_TERRAIN_LIGHT_ROW).xyz;
  let cascade_lit = cascade_lit_fraction(rotation * in.fromCameraBf, rotation * geometric, rotation * sun);
  let shaded = shade_terrain(lit, normal, false, cascade_lit, own_shade_place(lit));
  var meter = METER_OTHER;
  if (u32(terrain_row(0u).x) > 0u) {
    meter = select(METER_UNLIT_BODY, METER_LIT_BODY, shaded.lit);
  }
  return vec4f(min(shaded.radiance, vec3f(HALF_FLOAT_MAX)), meter);
}

@vertex
fn vertexMain(@location(0) local : vec3f, @builtin(instance_index) entry : u32) -> RockVertexOut {
  return rock_vertex(local, entry);
}

@fragment
fn fragmentMain(in : RockVertexOut) -> @location(0) vec4f {
  let geometric = cascade_geometric_normal(
    dpdxFine(in.fromCameraBf), dpdyFine(in.fromCameraBf), normalize(in.datum)
  );
  return rock_fragment(in, geometric);
}
