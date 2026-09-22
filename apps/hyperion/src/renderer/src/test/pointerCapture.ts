/** The pointers each element holds captured. */
const captured = new WeakMap<Element, Set<number>>();

function capturesOf(element: Element): Set<number> {
  let pointers = captured.get(element);
  if (pointers === undefined) {
    pointers = new Set();
    captured.set(element, pointers);
  }
  return pointers;
}

/**
 * Gives elements the pointer capture methods jsdom lacks: `setPointerCapture`,
 * `releasePointerCapture` and `hasPointerCapture`, which record the pointers each element holds.
 *
 * @remarks
 * `test/setup.ts` installs it before every test. Capture changes no event's target here, since
 * `fireEvent` dispatches to the element it is given; a test asserts what was captured instead.
 */
export function installPointerCapture(): void {
  Object.defineProperties(Element.prototype, {
    setPointerCapture: {
      configurable: true,
      writable: true,
      value(this: Element, pointerId: number): void {
        capturesOf(this).add(pointerId);
      },
    },
    releasePointerCapture: {
      configurable: true,
      writable: true,
      value(this: Element, pointerId: number): void {
        capturesOf(this).delete(pointerId);
      },
    },
    hasPointerCapture: {
      configurable: true,
      writable: true,
      value(this: Element, pointerId: number): boolean {
        return capturesOf(this).has(pointerId);
      },
    },
  });
}
