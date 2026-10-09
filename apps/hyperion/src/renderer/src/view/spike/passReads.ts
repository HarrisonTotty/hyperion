/**
 * The descent spike's view of R01's pass-time reads (plan R05, T14.k, the orchestrator's ruling on
 * T14.j's open question): which resolves' times are still being read back, so that the run's
 * control can wait for them before it takes its report, and so that a read still outstanding
 * after the wait is counted missing for that reason, not as a resolve the timer dropped.
 *
 * @remarks
 * R01's timer reads a resolve's times by mapping its staging buffer, `pass times readback <k>`,
 * just after the submission that resolved them (`PassTimer.resolve`), so the run's resolve number
 * at that moment (`ResolveCounter.value`, which follows the engine's own count) is the read's. A
 * read ends when its times are reported, or when its mapping fails. A resolve the timer dropped
 * while every buffer was in flight makes no read, so it is never in flight. The buffer's
 * `mapAsync` is replaced on the instance, as the pipeline shim replaces the device's methods,
 * because the browser's WebGPU calls refuse a proxy.
 */

/** The label R01's timer gives each staging buffer it maps for reading (`PassTimer`'s pairs). */
export const PASS_TIMES_STAGING_LABEL = /^pass times readback \d+$/;

/** The pass-time reads in flight, by the run's resolve number. */
export class PassReads {
  readonly #numberNow: () => number;
  readonly #inFlight = new Set<number>();
  /** Each wait's check, run whenever a read ends. */
  readonly #waits = new Set<() => void>();

  /** @param numberNow - The run's resolve number now (`ResolveCounter.value`). */
  constructor(numberNow: () => number) {
    this.#numberNow = numberNow;
  }

  /** A read begins now, of the resolve numbered now; returns that number. */
  began(): number {
    const n = this.#numberNow();
    this.#inFlight.add(n);
    return n;
  }

  /** Resolve `n`'s read has ended: its times were reported, or its mapping failed. */
  ended(n: number): void {
    if (this.#inFlight.delete(n)) {
      for (const check of this.#waits) {
        check();
      }
    }
  }

  /** The resolves whose reads are in flight now, ascending. */
  inFlight(): number[] {
    return [...this.#inFlight].toSorted((a, b) => a - b);
  }

  /** Whether any read of a resolve numbered above `after` and up to `through` is in flight. */
  #pending(after: number, through: number): boolean {
    for (const n of this.#inFlight) {
      if (n > after && n <= through) {
        return true;
      }
    }
    return false;
  }

  /**
   * Waits until no read of a resolve numbered above `after` and up to `through` is in flight, or
   * until `timeoutMs` has passed.
   *
   * @remarks
   * The range is a run's frames' (`SpikeMetrics`' first engine frame, exclusive, to its last
   * frame's): a read that began before the numbering followed the engine carries a number at or
   * below the first, and is never waited for.
   *
   * @returns Whether every such read ended in time.
   */
  settled(after: number, through: number, timeoutMs: number): Promise<boolean> {
    if (!this.#pending(after, through)) {
      return Promise.resolve(true);
    }
    return new Promise((resolve) => {
      let timer: ReturnType<typeof setTimeout> | undefined;
      const check = (): void => {
        if (!this.#pending(after, through)) {
          finish(true);
        }
      };
      const finish = (ended: boolean): void => {
        clearTimeout(timer);
        this.#waits.delete(check);
        resolve(ended);
      };
      timer = setTimeout(() => {
        finish(false);
      }, timeoutMs);
      this.#waits.add(check);
    });
  }
}

/**
 * `device`, its `createBuffer` replaced on the instance so that each staging buffer of R01's timer
 * tells `reads` when its read begins and when its mapping fails; the reports end the others.
 */
export function trackPassReads(device: GPUDevice, reads: PassReads): GPUDevice {
  const createBuffer = device.createBuffer.bind(device);
  device.createBuffer = (descriptor) => {
    const buffer = createBuffer(descriptor);
    if (PASS_TIMES_STAGING_LABEL.test(descriptor.label ?? "")) {
      const mapAsync = buffer.mapAsync.bind(buffer);
      buffer.mapAsync = (mode, offset, size) => {
        const n = reads.began();
        const mapping = mapAsync(mode, offset, size);
        // The timer reports the failure; here it only ends the read.
        mapping.catch(() => {
          reads.ended(n);
        });
        return mapping;
      };
    }
    return buffer;
  };
  return device;
}
