/**
 * The harness's patience for the client's tests: an upper bound on a slow case's wait, such as a
 * hook whose fixed work a loaded machine only makes slower, never wrong. It is the same variable
 * as the server's integration tests' (`crates/hyperion-server/tests/common/mod.rs`'s
 * `patience()`), with the same default and the same rule.
 *
 * @remarks
 * This is the harness's patience and not a property of the code under test, so it can be raised
 * for a machine slower or busier than any a case was measured on. No assertion depends on it: a
 * test of the code's own timing states its bound itself. The bound is the patience itself, neither
 * scaled nor floored, as the variable is an absolute number of seconds and not a factor.
 *
 * The rule is the server's. Unset, the patience is {@link DEFAULT_PATIENCE_SECS}. Set, the value is
 * trimmed of white space and read as a whole number of seconds above 0, a leading `+` and leading
 * zeros allowed, as Rust's `trim().parse::<u64>()` takes them; anything else is refused. The two
 * trims differ only on U+0085, which Rust's takes off and JavaScript's keeps, and U+FEFF, the
 * reverse. One limit is the client's alone: a timer holds at most 2³¹ − 1 ms (Node fires a longer
 * one at once, with a warning), so a value past it is refused here, where Rust's `Duration` takes
 * any whole number of seconds a `u64` holds.
 *
 * Under vitest, `import.meta.env` reads the process's environment.
 */

/** The variable that sets the patience in whole seconds, the server harness's `PATIENCE_ENV`. */
export const PATIENCE_VARIABLE = "HYPERION_TEST_PATIENCE_SECS";

/** The patience in seconds when the variable is unset, the server harness's `DEFAULT_PATIENCE`. */
export const DEFAULT_PATIENCE_SECS = 180;

/** The longest delay a timer holds, ms: 2³¹ − 1, the most a signed 32-bit integer holds. */
const LONGEST_TIMER_MS = 2 ** 31 - 1;

/** A whole number of seconds as Rust's `parse::<u64>()` reads one: ASCII digits, an optional `+`. */
const WHOLE_SECONDS = /^\+?\d+$/u;

/**
 * The patience in milliseconds for the variable's value, `undefined` where unset: the server
 * harness's rule, and the timer's limit.
 *
 * @param raw - The variable's value.
 * @throws RangeError naming the variable, for a value that is not a string holding a whole number
 *   of seconds above 0, or that is longer than a timer holds.
 */
export function patienceMsOf(raw: unknown): number {
  if (raw === undefined) {
    return DEFAULT_PATIENCE_SECS * 1_000;
  }
  const shown = typeof raw === "string" ? JSON.stringify(raw) : `a ${typeof raw}`;
  const rule = "must be a whole number of seconds above 0";
  const trimmed = typeof raw === "string" ? raw.trim() : "";
  const ms = WHOLE_SECONDS.test(trimmed) ? Number(trimmed) * 1_000 : 0;
  if (ms <= 0) {
    throw new RangeError(`${PATIENCE_VARIABLE} is ${shown}, and ${rule}`);
  }
  if (ms > LONGEST_TIMER_MS) {
    const longest = Math.floor(LONGEST_TIMER_MS / 1_000);
    throw new RangeError(
      `${PATIENCE_VARIABLE} is ${shown}, and ${rule} and at most ${String(longest)}, ` +
        "the longest a timer holds",
    );
  }
  return ms;
}

/**
 * This test run's patience in milliseconds, from its environment, for a hook's or a test's limit.
 *
 * @throws RangeError naming the variable, as {@link patienceMsOf} does, for a value it refuses.
 */
export function testPatienceMs(): number {
  const env: Readonly<Record<string, unknown>> = import.meta.env;
  return patienceMsOf(env[PATIENCE_VARIABLE]);
}
