/**
 * The maturity layer's albedo nodes as the lit terrain draws them (plan R10, R10.T1.c's client half;
 * decision-r10-t1c-client-scope §1.4): the TypeScript twin of `shaders/maturityNodes.wgsl`, the
 * formula `hyperion_surface`'s `maturity_node_albedo` gives, which the shader evaluates per vertex.
 *
 * @remarks
 * A material carries {@link MATURITY_NODES} nodes a band level, its drawn albedo T at the
 * maturities m_j = m_body q_j, q_j = 1 − (1 − u_j³)², u_j = j ÷ (K − 1) ({@link maturityNodeShare}):
 * node 0 fresh ground, node K − 1 the body's background maturity m_body. A vertex's maturity
 * reaches the client as a unorm16 code c of the deficit ratio r = √(1 − m ÷ m_body)
 * (`bake::encode_maturity`), r = c ÷ 65,535 ({@link maturityCodeRatio}), in which the nodes are
 * uniform in u = ∛(1 − r). So the shader needs neither m_body nor a square root: t = (K − 1) u, at
 * most K − 1, i = min(⌊t⌋, K − 2) and f = t − i ({@link maturitySegment}), and the albedo is
 * (1 − f) Tᵢ + f Tᵢ₊₁ in each channel, linear in T ({@link maturityNodeAlbedo}). Code 0 draws node
 * K − 1 and code 65,535 node 0, each to the bit. Where m_body is 0 every code is 0 and every node
 * the same, so node K − 1 is the Rust formula's node 0.
 *
 * The twin works in `f64` and the shader in `f32`, so their parity is held by the formula's lines,
 * pinned against this module's constants (`maturityNodes.test.ts`), and by tolerances, never by
 * emulating `f32`.
 */

/**
 * K, the albedo nodes a material carries a band level: `hyperion_surface`'s `MATURITY_NODES`, the
 * least K whose interpolation holds the ruling's 0.5% on the `testing` worlds (16; the module's
 * `maturityNodeCount()`, which `fieldsWasm.test.ts` holds it to).
 */
export const MATURITY_NODES = 16;

/** A maturity code's largest value, unorm16's 65,535: fresh ground's code. */
export const MATURITY_CODE_MAX = 65_535;

/** The last node's coordinate, K − 1. */
const LAST_NODE = MATURITY_NODES - 1;

/**
 * Node `node`'s maturity as a share of m_body, q = 1 − (1 − u³)² with u = j ÷ (K − 1), 0 at node
 * 0 and 1 at node K − 1 (`hyperion_surface`'s `maturity_node_share`).
 *
 * @throws RangeError for a node that is not an integer in [0, K − 1].
 */
export function maturityNodeShare(node: number): number {
  if (!Number.isInteger(node) || node < 0 || node > LAST_NODE) {
    throw new RangeError(
      `a maturity node is one of ${String(MATURITY_NODES)}, got ${String(node)}`,
    );
  }
  const u = node / LAST_NODE;
  const fresh = 1 - u * u * u;
  return 1 - fresh * fresh;
}

/**
 * A maturity code's deficit ratio r = c ÷ 65,535, as WGSL's `unpack2x16unorm` gives it: 0 at the
 * body's background maturity m_body, 1 on fresh ground.
 */
export function maturityCodeRatio(code: number): number {
  return code / MATURITY_CODE_MAX;
}

/**
 * The deficit ratio r = √(1 − q) of a maturity share q = m ÷ m_body in [0, 1], unrounded: the
 * ratio a code rounds (`bake::encode_maturity`), for a twin evaluated at a maturity rather than a
 * code, as `maturity_node_albedo` takes it.
 */
export function maturityRatioOfShare(share: number): number {
  return Math.sqrt(1 - share);
}

/** Where a deficit ratio falls in the node table: between node {@link node} and the next. */
export interface MaturitySegment {
  /** The segment's first node i, in [0, K − 2]. */
  readonly node: number;
  /** The share f of the way from node i to node i + 1, in [0, 1]. */
  readonly fraction: number;
}

/**
 * The segment of the node table at deficit ratio `ratio` (r in [0, 1], {@link maturityCodeRatio}):
 * u = ∛(1 − r), t = min((K − 1) u, K − 1), i = min(⌊t⌋, K − 2) and f = t − i, as
 * `maturity_segment` in `shaders/maturityNodes.wgsl`.
 */
export function maturitySegment(ratio: number): MaturitySegment {
  const u = Math.cbrt(1 - ratio);
  const t = Math.min(LAST_NODE * u, LAST_NODE);
  const node = Math.min(Math.floor(t), MATURITY_NODES - 2);
  return { node, fraction: t - node };
}

/**
 * The albedo between a segment's two nodes, (1 − f) Tᵢ + f Tᵢ₊₁ in each of three channels, as
 * `maturity_mix` in `shaders/maturityNodes.wgsl`.
 *
 * @param nodes - A node table, node j's three channels at `first + 3j`, in any channel order,
 *   which the result keeps.
 * @param first - Where node 0's first channel is in `nodes`.
 * @throws RangeError where a channel the segment reads lies outside `nodes`.
 */
export function maturityNodeAlbedo(
  nodes: ArrayLike<number>,
  first: number,
  segment: MaturitySegment,
): [number, number, number] {
  const { node, fraction } = segment;
  const channel = (j: number, c: number): number => {
    const value = nodes[first + 3 * j + c];
    if (value === undefined) {
      throw new RangeError(`node ${String(j)} lies outside a table of ${String(nodes.length)}`);
    }
    return value;
  };
  const mixed = (c: number): number =>
    (1 - fraction) * channel(node, c) + fraction * channel(node + 1, c);
  return [mixed(0), mixed(1), mixed(2)];
}
