/**
 * A pure function's results kept by key, so that the tests of one file asking for the same
 * expensive result (an exact integral over a grid, a recorded cell) compute it once.
 *
 * @remarks
 * Only for a `compute` that is deterministic, reads nothing but its key and what every test starts
 * from (a page rendered afresh, a fake clock it installs itself), and returns a value that no
 * caller mutates (type it `readonly`). Then the first test to ask computes exactly what any other
 * would have, so which test runs first, alone or in any order, changes no value: the tests stay
 * independent although they share the memo. `keyOf` must tell apart every input `compute` reads.
 * Keep the memo in the file, or the `describe`, whose tests share it. A test of what the work
 * itself does to its surroundings runs the work itself.
 */
export function memoised<K, V>(compute: (key: K) => V, keyOf: (key: K) => string): (key: K) => V {
  const known = new Map<string, { readonly value: V }>();
  return (key) => {
    const id = keyOf(key);
    let entry = known.get(id);
    if (entry === undefined) {
      entry = { value: compute(key) };
      known.set(id, entry);
    }
    return entry.value;
  };
}
