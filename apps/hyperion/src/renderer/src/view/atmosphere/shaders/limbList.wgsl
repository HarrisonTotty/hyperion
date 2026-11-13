// The far limb's packed node rays (plan R08, R08.T8.d; `hillaire.ts`'s `LimbRays` `packed`): the
// lists the march's packed entry point (`rayMarchPacked.wgsl`) fills with each limb texel it takes
// and its footprint's node rays, which the limb's node pass marches one a thread
// (`limbNodes.wgsl`) and its resolve sums in node order into the texel (`limbResolve.wgsl`), so
// that a tile no longer waits on its slowest texel's nodes. Each texel's bits are the per-texel
// march's: the same node rays, each marched by the one `marchRay`, and their sum in the same order.
// Prepended to the packed entry point, the node pass, the resolve and the dispatch's count
// (`limbDispatch.wgsl`); it declares nothing.

// A limb texel's entry: the texel (x in the low half, y in the high), its first node ray's slot and
// its count, its footprint's blend and share above the top, and its centre ray's march, taken
// before its node rays and held in f32 for the blend; `texel` LIMB_REFUSED where its nodes found no
// room, which the per-texel march then marches, so that the resolve leaves it.
struct LimbTexel {
  texel : u32,
  first : u32,
  nodes : u32,
  blend : f32,
  centre : vec4f,
  centreTransmittance : vec3f,
  spaceShare : f32,
}

// A node ray: the output's coordinates of its ray and its weight, as `farLimbNodeOf` formed them
// (written by the packed entry point), and its march (written by the node pass).
struct LimbRay {
  uv : vec2f,
  weight : f32,
  light : vec4f,
  transmittance : vec3f,
}

// The lists' counts (`hillaire.ts`'s limb counts buffer): the texels and node rays reserved this
// frame, cleared before the packed entry point, and those within the lists' room, which the
// dispatch's count forms for the node pass and the resolve.
struct LimbCounts {
  reserved : vec2u,
  taken : vec2u,
}

// A texel entry whose nodes found no room.
const LIMB_REFUSED : u32 = 0xffffffffu;

// The node pass's and the resolve's workgroup, a line of invocations.
const LIMB_WORKGROUP : u32 = 64u;
