/**
 * In-memory stand-in for the browser `ResizeObserver`, which jsdom lacks, letting tests say when
 * layout has changed.
 *
 * @remarks
 * `test/setup.ts` stubs it in for every test. jsdom has no layout, so a test that needs a size
 * stubs the size it reads (such as `clientHeight`) and then calls {@link FakeResizeObserver.resizeAll}.
 */
export class FakeResizeObserver {
  static instances: FakeResizeObserver[] = [];

  readonly observed = new Set<Element>();

  constructor(readonly callback: ResizeObserverCallback) {
    FakeResizeObserver.instances.push(this);
  }

  /** Reports a resize to every observer that watches an element, as the browser does after layout. */
  static resizeAll(): void {
    for (const observer of FakeResizeObserver.instances) {
      if (observer.observed.size > 0) {
        observer.callback([], observer);
      }
    }
  }

  observe(target: Element): void {
    this.observed.add(target);
  }

  unobserve(target: Element): void {
    this.observed.delete(target);
  }

  disconnect(): void {
    this.observed.clear();
  }
}
