// The stride morph (plan R14, R14.T3.c; Design note 3): each lit vertex placed so that a camera
// pass's cluster drawn at any stride shows the surface every other draws, and a stride flips with
// nothing moving. A vertex at morph level j, its grid place a multiple of 2^j but not of 2^(j + 1)
// (`stride_morph_level`, capped at 3, a block's corners), is what the stride-2^(j + 1) mesh drops;
// `lit_place`'s place of it moves towards the mean of its two neighbours at 2^j along its quad's
// side or on its (0, 0)–(1, 1) diagonal (`stride_parents`), each placed so in turn, by k_j =
// clamp((d − U) ÷ (β U), 0, 1), d the grid vertex's distance and U its band towards stride
// 2^(j + 1) (`stride_band`, the shared bands of `clusters/strides.ts`: its block's band, the
// largest need of it and its sides' blocks from its slot's record's errors under its instance's
// scale; on a block's side the smaller of its two blocks'; on a patch's edge its instance's shared
// band there), held at each contact as the CDLOD morph is; at 0 for the level-2 vertices of an
// edge a finer patch touches, whose vertices there are block corners, falling to it over the hold's
// onset by the instance's share (`Instance.strides.z`); and at 1 for the level-0
// vertices of an edge a coarser patch touches, which stand at the coarser grid's midpoints
// (R10.T8.c's hold at k = 1), so that they follow its vertices wherever those move. The morph mixes
// the height, the grid vertex's height, the CDLOD k and the datum's normal with the place, so that
// the varyings flip with it. The cull takes a stride only where every vertex it drops is wholly
// moved (`clusterCull.wgsl`'s `camera_stride_level`), so each cluster draws exactly this surface.
// The TypeScript twin is `clusters/strides.ts`' `stridePlace`.
//
// Composed after terrainSurvey.wgsl, whose `lit_place` it moves, and before terrainLit.wgsl,
// whose vertex stage calls `stride_place`. Each material's own source declares STRIDE_MORPH: true
// for the lit surface (`terrainLitShade.wgsl`), false for the overlay's depth and the casters,
// which keep the stride-1 surface and their own strides' meshes.

// `strides.ts`' STRIDE_BAND_FRACTION, STRIDE_MORPH_LEVELS and STRIDE_NODES.
const STRIDE_BAND_FRACTION : f32 = 0.02;
const STRIDE_MORPH_LEVELS : u32 = 3u;
const STRIDE_NODES : u32 = 9u;
const STRIDE_NO_NODE : u32 = 0xffffffffu;

// The morph level of grid vertex (x, y), 0 to 3 (`strides.ts`' `strideMorphLevel`).
fn stride_morph_level(x : u32, y : u32) -> u32 {
  return firstTrailingBit(x | y | 8u);
}

// The two neighbours (x, y) of morph level `level` moves between, (ax, ay, bx, by)
// (`strideParents`).
fn stride_parents(x : u32, y : u32, level : u32) -> vec4u {
  let s = 1u << level;
  let ox = (x >> level) & 1u;
  let oy = (y >> level) & 1u;
  if (ox == 1u && oy == 1u) {
    return vec4u(x - s, y - s, x + s, y + s);
  }
  if (ox == 1u) {
    return vec4u(x - s, y, x + s, y);
  }
  return vec4u(x, y - s, x, y + s);
}

// What the morph mixes of a placed vertex: where it stands less the camera, its height, its grid
// vertex's height, its CDLOD k and its datum's normal.
struct StrideValues {
  at : vec3f,
  heightM : f32,
  gridHeightM : f32,
  morph : f32,
  datumNormal : vec3f,
}

fn stride_values(p : LitPlace) -> StrideValues {
  var v : StrideValues;
  v.at = p.fromCameraM;
  v.heightM = p.heightM;
  v.gridHeightM = p.vertex.heightM;
  v.morph = p.vertex.morph;
  v.datumNormal = p.vertex.datumNormal;
  return v;
}

// `a` moved towards the mean of `b` and `c` by `k`.
fn stride_mix(a : StrideValues, b : StrideValues, c : StrideValues, k : f32) -> StrideValues {
  var v : StrideValues;
  v.at = mix(a.at, (b.at + c.at) * 0.5, k);
  v.heightM = mix(a.heightM, (b.heightM + c.heightM) * 0.5, k);
  v.gridHeightM = mix(a.gridHeightM, (b.gridHeightM + c.gridHeightM) * 0.5, k);
  v.morph = mix(a.morph, (b.morph + c.morph) * 0.5, k);
  v.datumNormal = mix(a.datumNormal, (b.datumNormal + c.datumNormal) * 0.5, k);
  return v;
}

// A block's need towards stride 2^(level + 1) under `inst`'s scale, its error from its slot's
// record (`strides.ts`' `strideNeedM`); −1 for none, an unmeasured block's error being +∞.
fn stride_need(inst : Instance, block : u32, level : u32) -> f32 {
  let h = 3u * block + level;
  let e = unpack2x16float(slots[inst.slot].blockErrors[h / 8u][(h / 2u) % 4u])[h % 2u];
  if (inst.strides.y <= 0.0 || !(e >= 0.0) || e > 65504.0) {
    return -1.0;
  }
  return inst.strides.x + e * inst.strides.y;
}

// The shared band of `inst`'s patch edge `edge` at its block `t` (`strides.ts`' `edgeBandIndex`), a
// half float of kilometres.
fn stride_edge_band(inst : Instance, edge : u32, t : u32, level : u32) -> f32 {
  let h = (8u * edge + t) * 3u + level;
  let km = unpack2x16float(inst.edgeBands[h / 8u][(h / 2u) % 4u])[h % 2u];
  return select(-1.0, km * 1000.0, km > 0.0);
}

// The larger of band `a` and part `b`, `a` where `b` is none.
fn stride_part(a : f32, b : f32) -> f32 {
  return select(a, max(a, b), b > 0.0);
}

// The band of `inst`'s block (bx, by) as the vertices inside it take it (`strides.ts`'
// `blockVertexBandM`): the largest need of it and the blocks on its sides, and of its patch's
// edges' shared bands along it, of those not none; none where its own need is.
fn stride_block_band(inst : Instance, bx : u32, by : u32, level : u32) -> f32 {
  let block = 8u * by + bx;
  var band = stride_need(inst, block, level);
  if (band <= 0.0) {
    return -1.0;
  }
  if (bx > 0u) {
    band = stride_part(band, stride_need(inst, block - 1u, level));
  } else {
    band = stride_part(band, stride_edge_band(inst, 0u, by, level));
  }
  if (bx < 7u) {
    band = stride_part(band, stride_need(inst, block + 1u, level));
  } else {
    band = stride_part(band, stride_edge_band(inst, 1u, by, level));
  }
  if (by > 0u) {
    band = stride_part(band, stride_need(inst, block - 8u, level));
  } else {
    band = stride_part(band, stride_edge_band(inst, 2u, bx, level));
  }
  if (by < 7u) {
    band = stride_part(band, stride_need(inst, block + 8u, level));
  } else {
    band = stride_part(band, stride_edge_band(inst, 3u, bx, level));
  }
  return band;
}

// The band of a block's side between `a` and `b`, the bands of its two blocks: the smaller, none
// where either is.
fn stride_side_band(a : f32, b : f32) -> f32 {
  return select(-1.0, min(a, b), a > 0.0 && b > 0.0);
}

// The band of `inst`'s grid vertex (x, y) at morph level `level` (`strides.ts`' `vertexStrideBandM`):
// on a patch's edge its shared band there, on a block's side the smaller of its two blocks' bands,
// else its block's.
fn stride_band(inst : Instance, x : u32, y : u32, level : u32) -> f32 {
  if (x == 0u || x == 64u || y == 0u || y == 64u) {
    let edge = select(select(select(3u, 2u, y == 0u), 1u, x == 64u), 0u, x == 0u);
    return stride_edge_band(inst, edge, min(select(x, y, edge < 2u) / 8u, 7u), level);
  }
  let bx = x / 8u;
  let by = y / 8u;
  let band = stride_block_band(inst, bx, by, level);
  if (x % 8u == 0u) {
    return stride_side_band(band, stride_block_band(inst, bx - 1u, by, level));
  }
  if (y % 8u == 0u) {
    return stride_side_band(band, stride_block_band(inst, bx, by - 1u, level));
  }
  return band;
}

// The contacts' hold at the place `at` less the camera: `morphFactor`'s, 1 with no contact.
fn stride_contact_hold(at : vec3f) -> f32 {
  var hold = 1.0;
  for (var i = 0u; i < contacts.count; i += 1u) {
    let c = contacts.items[i];
    let beyond = length(at - c.centreM) - c.heldRadiusM;
    hold = min(hold, select(select(0.0, 1.0, beyond > 0.0), clamp(beyond / c.rampM, 0.0, 1.0), c.rampM > 0.0));
  }
  return hold;
}

// The stride morph's k of `inst`'s grid vertex (x, y) at morph level `level`, its grid vertex at
// `at` less the camera (`strides.ts`' `strideMorphFactor` of its `vertexStrideBandM`, with its
// holds); 0 for a band of none or past `f32`'s range.
fn stride_factor(inst : Instance, level : u32, x : u32, y : u32, at : vec3f) -> f32 {
  if (level >= STRIDE_MORPH_LEVELS) {
    return 0.0;
  }
  let edges = holdBitsAt(x, y) & 0xfu;
  let held = (inst.parentQuadrant >> 2u) & edges;
  // The share of its stride morph a hold at k = 0 still leaves the vertex as it switches on.
  var share = 1.0;
  if (level == 2u && held != 0u) {
    share = f32((bitcast<u32>(inst.strides.z) >> (8u * firstTrailingBit(held))) & 0xffu) / 255.0;
    if (share <= 0.0) {
      return 0.0;
    }
  }
  if (level == 0u && ((inst.parentQuadrant >> 10u) & edges) != 0u) {
    return 1.0;
  }
  let band = stride_band(inst, x, y, level);
  if (!(band > 0.0 && band < 3.0e38)) {
    return 0.0;
  }
  let k = clamp((length(at) - band) / (STRIDE_BAND_FRACTION * band), 0.0, 1.0) * share;
  return min(k, stride_contact_hold(at));
}

// The node of (x, y) among the first `count` of `xs` and `ys`, added where it is not one; the last
// node where none is left, which no vertex of the grid reaches.
fn stride_node(
  xs : ptr<function, array<u32, STRIDE_NODES>>,
  ys : ptr<function, array<u32, STRIDE_NODES>>,
  count : ptr<function, u32>,
  x : u32,
  y : u32,
) -> u32 {
  for (var i = 0u; i < *count; i += 1u) {
    if ((*xs)[i] == x && (*ys)[i] == y) {
      return i;
    }
  }
  if (*count >= STRIDE_NODES) {
    return STRIDE_NODES - 1u;
  }
  (*xs)[*count] = x;
  (*ys)[*count] = y;
  *count = *count + 1u;
  return *count - 1u;
}

// Where vertex `grid` of `instance` stands under the stride morph: `lit_place`'s, moved (see the
// module's head). A sub-patch, a material without the morph and an instance without bands keep
// `lit_place`'s.
fn stride_place(grid : vec3f, instance : u32) -> LitPlace {
  var own = lit_place(grid, instance);
  if (!STRIDE_MORPH) {
    return own;
  }
  let inst = instances[instance];
  let x = u32(grid.x);
  let y = u32(grid.y);
  let level = stride_morph_level(x, y);
  if (inst.strides.y <= 0.0 || (inst.sub.x & 31u) != 0u || level >= STRIDE_MORPH_LEVELS) {
    return own;
  }
  let k0 = stride_factor(inst, level, x, y, own.vertex.fromCameraM);
  if (k0 <= 0.0) {
    return own;
  }
  var xs : array<u32, STRIDE_NODES>;
  var ys : array<u32, STRIDE_NODES>;
  var levels : array<u32, STRIDE_NODES>;
  var ks : array<f32, STRIDE_NODES>;
  var pa : array<u32, STRIDE_NODES>;
  var pb : array<u32, STRIDE_NODES>;
  var values : array<StrideValues, STRIDE_NODES>;
  xs[0] = x;
  ys[0] = y;
  levels[0] = level;
  ks[0] = k0;
  values[0] = stride_values(own);
  var count = 1u;
  // Breadth first, each vertex placed once, with one `lit_place` for every one but the first.
  for (var i = 0u; i < count; i += 1u) {
    if (i > 0u) {
      let p = lit_place(vec3f(f32(xs[i]), f32(ys[i]), grid.z), instance);
      values[i] = stride_values(p);
      levels[i] = stride_morph_level(xs[i], ys[i]);
      ks[i] = stride_factor(inst, levels[i], xs[i], ys[i], p.vertex.fromCameraM);
    }
    pa[i] = STRIDE_NO_NODE;
    pb[i] = STRIDE_NO_NODE;
    if (ks[i] > 0.0) {
      let parents = stride_parents(xs[i], ys[i], levels[i]);
      pa[i] = stride_node(&xs, &ys, &count, parents.x, parents.y);
      pb[i] = stride_node(&xs, &ys, &count, parents.z, parents.w);
    }
  }
  // The coarsest first: a vertex's neighbours lie at a coarser level than it.
  for (var l = STRIDE_MORPH_LEVELS; l > 0u; l -= 1u) {
    for (var i = 0u; i < count; i += 1u) {
      if (levels[i] == l - 1u && pa[i] != STRIDE_NO_NODE) {
        values[i] = stride_mix(values[i], values[pa[i]], values[pb[i]], ks[i]);
      }
    }
  }
  let v = values[0];
  own.fromCameraM = v.at;
  own.heightM = v.heightM;
  own.vertex.heightM = v.gridHeightM;
  own.vertex.morph = v.morph;
  own.vertex.datumNormal = v.datumNormal;
  return own;
}
