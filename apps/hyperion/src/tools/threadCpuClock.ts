/**
 * The thread's CPU-time clock that the descent's demand record times selection by (plan R05,
 * T13.a; `scripts/descentDemand.mjs`), exact on a kernel that counts CPU time in ticks.
 *
 * @remarks
 * Read alone, Linux's thread time (`getrusage(RUSAGE_THREAD)`, behind Node's
 * `process.threadCpuUsage`) is the thread's runtime as of the scheduler's last update. A kernel
 * that counts CPU time in ticks (no `nohz_full`) makes that update once a tick, 1 ms at
 * `CONFIG_HZ=1000`, so a 0.4 ms selection reads 0 or about 1 ms. The process's time
 * (`getrusage(RUSAGE_SELF)`, behind `process.cpuUsage`) first brings the calling thread's runtime
 * up to date, so a thread reading just after it is exact. The pair costs about 1.5 µs (lane B,
 * R05.T7 perf (d)). Elsewhere the extra call is harmless, and the reading's resolution is the
 * system's, unmeasured here: Windows's thread times are known to step at the clock interrupt,
 * 15.6 ms by default, which no call refines.
 */

/** The readings the clock takes, injected so that a test can stand in for the kernel. */
export interface CpuUsageSource {
  /** `process.cpuUsage` outside a test: the process's user and system time, µs. */
  cpuUsage(): NodeJS.CpuUsage;
  /** `process.threadCpuUsage` outside a test: the calling thread's, µs; absent before Node 23.9. */
  readonly threadCpuUsage?: () => NodeJS.CpuUsage;
}

/**
 * The calling thread's CPU time, user and system, as a clock in milliseconds, each reading taken
 * just after the process's so that it is exact (the module's remarks); `undefined` where Node
 * lacks `process.threadCpuUsage`, the record's CPU times then being null.
 */
export function threadCpuClockMs(source: CpuUsageSource): (() => number) | undefined {
  const readThread = source.threadCpuUsage?.bind(source);
  if (readThread === undefined) {
    return undefined;
  }
  return () => {
    source.cpuUsage();
    const { user, system } = readThread();
    return (user + system) / 1000;
  };
}
