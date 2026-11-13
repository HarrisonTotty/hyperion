// The maturity layer's albedo nodes (plan R10, R10.T1.c's client half; decision-r10-t1c-client-scope
// §1.4): where a vertex's maturity code falls in a material's node table, and the albedo between
// two nodes, `hyperion_surface`'s `maturity_node_albedo` (TypeScript twin: `terrain/maturityNodes.ts`).
// A library of no bindings: each includer reads its own codes and nodes and calls these. Composed
// before `terrainLit.wgsl` (the lit terrain's `class_layers`).
//
// A material carries MATURITY_NODES nodes a band level, its drawn albedo at the maturities
// m_body (1 − (1 − u³)²), u = j ÷ (K − 1): node 0 fresh ground, node K − 1 the body's background. A
// vertex's code c holds the deficit ratio r = c ÷ 65,535 = √(1 − m ÷ m_body) (`bake::encode_maturity`,
// as `unpack2x16unorm` reads it), in which the nodes are uniform in u = ∛(1 − r): code 0 draws node
// K − 1 and code 65,535 node 0, each exactly.

// K, the albedo nodes a material carries a level (`hyperion_surface`'s `MATURITY_NODES`).
const MATURITY_NODES : u32 = 16u;
// K − 1, the last node's coordinate.
const MATURITY_LAST_NODE : f32 = 15.0;

// Where a deficit ratio falls in the node table: between node `node` and the next, `fraction` of
// the way.
struct MaturitySegment {
  node : u32,
  fraction : f32,
}

// The segment of deficit ratio `ratio` in [0, 1]: u = ∛(1 − r), the cube root guarded at 0, where
// `pow` is undefined; t = (K − 1) u, at most K − 1; i = min(⌊t⌋, K − 2) and f = t − i.
fn maturity_segment(ratio : f32) -> MaturitySegment {
  let x = 1.0 - ratio;
  let u = select(pow(x, 1.0 / 3.0), 0.0, x <= 0.0);
  let t = min(MATURITY_LAST_NODE * u, MATURITY_LAST_NODE);
  let node = min(u32(floor(t)), MATURITY_NODES - 2u);
  return MaturitySegment(node, t - f32(node));
}

// The albedo between a segment's two nodes `lo` and `hi`, (1 − f) Tᵢ + f Tᵢ₊₁ in each channel,
// linear in T.
fn maturity_mix(lo : vec3f, hi : vec3f, fraction : f32) -> vec3f {
  return (1.0 - fraction) * lo + fraction * hi;
}
