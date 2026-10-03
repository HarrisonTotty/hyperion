// The `FaceDifferences` vertex path (plan R05, Design note 4): each offset rebuilt from the slot's
// terms, the shared grid and the heights, never from an absolute position. Composed after
// terrain.wgsl. vertexEmulation.ts mirrors `morphOffset` operation for operation.

// The vertex's own-level offset from its patch origin, body-fixed metres.
fn ownOffset(slot : u32, rec : SlotRecord, x : u32, y : u32) -> vec3f {
  return faceDifferencePosition(rec, x, y, heightAt(slot, x, y, 0u));
}

// The formula at vertex (px, py) and its morph height.
fn morphAt(slot : u32, rec : SlotRecord, px : u32, py : u32) -> vec3f {
  return faceDifferencePosition(rec, px, py, heightAt(slot, px, py, 1u));
}

// The morph target's offset: the parent mesh's point, `face_difference_morph_f32`.
fn morphOffset(slot : u32, rec : SlotRecord, x : u32, y : u32) -> vec3f {
  let oddX = (x & 1u) == 1u;
  let oddY = (y & 1u) == 1u;
  if (rec.straddles != 0u || (!oddX && !oddY)) {
    return morphAt(slot, rec, x, y);
  }
  var a : vec3f;
  var b : vec3f;
  if (oddX && !oddY) {
    a = morphAt(slot, rec, x - 1u, y);
    b = morphAt(slot, rec, x + 1u, y);
  } else if (!oddX) {
    a = morphAt(slot, rec, x, y - 1u);
    b = morphAt(slot, rec, x, y + 1u);
  } else {
    a = morphAt(slot, rec, x - 1u, y - 1u);
    b = morphAt(slot, rec, x + 1u, y + 1u);
  }
  return 0.5 * (a + b);
}
