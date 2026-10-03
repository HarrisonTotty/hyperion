// The `BakedOffsets` vertex path (plan R05, Design note 4): each vertex's own-level offset q₀ and
// morph target q₁ from its patch origin, formed in f64 by the bake and narrowed once. Composed
// after terrain.wgsl.

// q₀ then q₁ per vertex, slot by slot (the bake's `offsets` layout).
@group(2) @binding(5) var<storage, read> offsets : array<f32>;

fn offsetAt(slot : u32, x : u32, y : u32, which : u32) -> vec3f {
  let i = (slot * PATCH_VERTICES + y * VERTICES_PER_SIDE + x) * 6u + which * 3u;
  return vec3f(offsets[i], offsets[i + 1u], offsets[i + 2u]);
}

fn ownOffset(slot : u32, rec : SlotRecord, x : u32, y : u32) -> vec3f {
  return offsetAt(slot, x, y, 0u);
}

fn morphOffset(slot : u32, rec : SlotRecord, x : u32, y : u32) -> vec3f {
  return offsetAt(slot, x, y, 1u);
}
