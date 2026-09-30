/**
 * Compute kernels in twins: a reference without subgroups and, where one is worth having, a
 * subgroup variant, chosen by the device's features and never by which GPU it is.
 *
 * @remarks
 * A pair read back to the CPU is `bit-exact`, and the smoke harness runs both paths and compares
 * bytes. A pair whose paths differ in summation order is `presentation-only` and is never read back:
 * the engine refuses the readback (R01 Design note 16).
 */

import type { GpuCapabilities } from "./platform";

/** A kernel's two WGSL variants and what may be done with its result. */
export interface KernelPair {
  readonly name: string;
  /** WGSL without subgroups: the reference. */
  readonly reference: string;
  /** WGSL with `enable subgroups;`, or `null` when the kernel has only its reference. */
  readonly subgroup: string | null;
  /**
   * `bit-exact` when both paths give the same bytes and the result may reach the CPU;
   * `presentation-only` when they may differ in summation order and it may not.
   */
  readonly readback: "bit-exact" | "presentation-only";
}

/** The variant a device runs. */
export interface KernelSelection {
  readonly path: "reference" | "subgroup";
  readonly wgsl: string;
}

/**
 * The variant to run.
 *
 * @returns The subgroup variant only when the device has `subgroups` and the pair has one; the
 * reference otherwise.
 */
export function selectKernel(pair: KernelPair, capabilities: GpuCapabilities): KernelSelection {
  if (capabilities.subgroups && pair.subgroup !== null) {
    return { path: "subgroup", wgsl: pair.subgroup };
  }
  return { path: "reference", wgsl: pair.reference };
}

/** `wgsl` without its comments, line and block, so that a directive reads as the compiler reads it. */
function withoutComments(wgsl: string): string {
  return wgsl.replaceAll(/\/\*[\s\S]*?\*\//gu, " ").replaceAll(/\/\/[^\n]*/gu, " ");
}

/**
 * Whether `wgsl` holds an `enable` directive naming `extension`, wherever the directive stands:
 * WGSL allows several on a line and comments inside one, and `enable` is a reserved word.
 */
function enables(wgsl: string, extension: string): boolean {
  for (const match of withoutComments(wgsl).matchAll(/\benable\s+([^;]+);/gu)) {
    const names = (match[1] ?? "").split(",").map((name) => name.trim());
    if (names.includes(extension)) {
      return true;
    }
  }
  return false;
}

/**
 * Refuses a module that enables both `f16` and `subgroups`.
 *
 * @remarks
 * A coarse but mechanical form of the brainstorm's "subgroup operations never take f16 operands"
 * (R01 Design note 16).
 *
 * @param name - The kernel's name, for the error.
 * @throws Error naming the kernel when the module enables both.
 */
export function assertNoF16Subgroups(name: string, wgsl: string): void {
  if (enables(wgsl, "f16") && enables(wgsl, "subgroups")) {
    throw new Error(`kernel ${name} enables both f16 and subgroups`);
  }
}

/** The unit roundoff of IEEE 754 binary32, 2⁻²⁴. */
const F32_UNIT_ROUNDOFF = 2 ** -24;

/**
 * The bound on the error of an `f32` sum of `n` terms in any order: γ(n − 1) · Σ|xᵢ|, with
 * γ(k) = ku ÷ (1 − ku) and u = 2⁻²⁴.
 *
 * @remarks
 * Higham, _Accuracy and Stability of Numerical Algorithms_, 2nd ed., §4.2, for recursive summation
 * in any order, which covers a subgroup reduction's unspecified order (WGSL §17.12.1, §15.7). The
 * smoke harness checks a presentation-only sum against it over finite, normal inputs (R01 Design
 * note 16).
 *
 * @param n - Terms summed, at least 1.
 * @param sumAbs - Σ|xᵢ|, computed in `f64`.
 * @throws Error when `n` is not a positive integer or (n − 1)u reaches 1, where the bound fails.
 */
export function highamBound(n: number, sumAbs: number): number {
  if (!Number.isInteger(n) || n < 1) {
    throw new Error(`a sum has at least one term, not ${n}`);
  }
  const ku = (n - 1) * F32_UNIT_ROUNDOFF;
  if (ku >= 1) {
    throw new Error(`the bound does not hold for ${n} terms`);
  }
  return (ku / (1 - ku)) * sumAbs;
}
