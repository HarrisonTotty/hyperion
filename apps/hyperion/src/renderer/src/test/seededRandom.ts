/**
 * A seeded source of uniform numbers in [0, 1), so that tests over "random" inputs are
 * repeatable.
 *
 * @remarks
 * Mulberry32: small and fast, and good enough to scatter test inputs. Not for anything else.
 */
export function seededRandom(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let mixed = state;
    mixed = Math.imul(mixed ^ (mixed >>> 15), mixed | 1);
    mixed ^= mixed + Math.imul(mixed ^ (mixed >>> 7), mixed | 61);
    return ((mixed ^ (mixed >>> 14)) >>> 0) / 4_294_967_296;
  };
}

/** A uniform number in [`min`, `max`) from `random`. */
export function between(random: () => number, min: number, max: number): number {
  return min + (max - min) * random();
}
