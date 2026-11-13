/**
 * The inspection profile's count of React's commits (`--inspection --timing`, check-only; lane
 * perf-render-thread-2, the owner's Mac): each commit over a profiled window, and each component
 * rendered in it, with which of its props changed and whether its state did, so that a profile
 * tells a component rendered for what it shows from one rendered for its parent alone.
 *
 * @remarks
 * React reports each commit to a DevTools hook on the page's global,
 * `__REACT_DEVTOOLS_GLOBAL_HOOK__`, which `react-dom` takes as it loads, in its production build as
 * in development; the timing launch installs one ({@link installReactCommitHook}) before
 * `react-dom` loads (`reactCommitHook.ts`, `main.tsx`'s first import). A commit's fibers are walked
 * as React DevTools walks them: a subtree whose children are its previous commit's own was not
 * rendered, and a component of a subtree that was rendered where it mounted or did work
 * (`PerformedWork`). Its props are compared key by key with its last render's, and its state
 * hooks' values (those with an update queue); a render with neither changed was its parent's, or a
 * context's. These are React's internals, read at a trust boundary: a fiber not shaped as expected
 * counts nothing. The walk costs the main thread a little at each commit, so a counted window's
 * React time is a little over a player's.
 */

import type { InspectionReactCommits, InspectionReactComponent } from "../../../../../preload/api";

/** React's fiber tags of the components that render: function, class, forward-ref, simple memo. */
const COMPONENT_TAGS: ReadonlySet<unknown> = new Set([0, 1, 11, 15]);

/** React's class component's tag, whose state is one object rather than hooks. */
const CLASS_TAG = 1;

/** React's `PerformedWork` flag: the fiber's component was called in the render committed. */
const PERFORMED_WORK = 1;

/** The components a window lists, the most rendered first. */
export const REACT_COMPONENTS_LISTED = 80;

/** The changed props a component lists, the most often changed first. */
export const REACT_PROPS_LISTED = 6;

/** One component's renders over a window. */
interface Tally {
  renders: number;
  mounts: number;
  unchanged: number;
  stateChanged: number;
  readonly props: Map<string, number>;
}

/** A fiber as React's internals hold it, read field by field. */
type Fiber = Readonly<Record<string, unknown>>;

function isFiber(value: unknown): value is Fiber {
  return typeof value === "object" && value !== null;
}

/** A component's function or class's name, its `displayName` first, else `(anonymous)`. */
function nameOf(type: unknown): string {
  // A forward ref's type is an object holding its render function.
  const component: unknown =
    isFiber(type) && typeof type["render"] === "function" ? type["render"] : type;
  if (typeof component !== "function") {
    return "(anonymous)";
  }
  const displayName: unknown = Reflect.get(component, "displayName");
  if (typeof displayName === "string" && displayName.length > 0) {
    return displayName;
  }
  return component.name.length > 0 ? component.name : "(anonymous)";
}

/** The props of `after` whose values are not those of `before`, by `Object.is`, in key order. */
function changedProps(before: unknown, after: unknown): string[] {
  if (!isFiber(before) || !isFiber(after)) {
    return before === after ? [] : ["(props)"];
  }
  const keys = new Set([...Object.keys(before), ...Object.keys(after)]);
  return [...keys].filter((key) => !Object.is(before[key], after[key]));
}

/**
 * Whether a component's state changed between its last render's fiber and this one's: a class's
 * state object, or the value of any of a function's state hooks (those with an update queue,
 * `useState`, `useReducer` and `useSyncExternalStore`), pairwise in hook order.
 */
function stateChanged(tag: unknown, before: Fiber, after: Fiber): boolean {
  if (tag === CLASS_TAG) {
    return !Object.is(before["memoizedState"], after["memoizedState"]);
  }
  let was: unknown = before["memoizedState"];
  let now: unknown = after["memoizedState"];
  while (isFiber(was) && isFiber(now)) {
    const queue = now["queue"];
    if (
      queue !== null &&
      queue !== undefined &&
      !Object.is(was["memoizedState"], now["memoizedState"])
    ) {
      return true;
    }
    was = was["next"];
    now = now["next"];
  }
  return false;
}

/** Counts React's commits and its components' renders between its `begin` and its `take`. */
export class ReactCommitCounter {
  #counting = false;
  #commits = 0;
  #tallies = new Map<string, Tally>();

  /** Starts a window: everything counted before is forgotten. */
  begin(): void {
    this.#counting = true;
    this.#commits = 0;
    this.#tallies = new Map();
  }

  /**
   * The commits since {@link ReactCommitCounter.begin}, the components the most rendered first
   * (ties by name), at most {@link REACT_COMPONENTS_LISTED}; counting stops until the next begin.
   */
  take(): InspectionReactCommits {
    const tallies = [...this.#tallies].toSorted(
      ([an, a], [bn, b]) => b.renders - a.renders || (an < bn ? -1 : 1),
    );
    const components = tallies
      .slice(0, REACT_COMPONENTS_LISTED)
      .map(([name, tally]): InspectionReactComponent => ({
        name,
        renders: tally.renders,
        mounts: tally.mounts,
        unchanged: tally.unchanged,
        stateChanged: tally.stateChanged,
        propsChanged: [...tally.props]
          .toSorted(([an, a], [bn, b]) => b - a || (an < bn ? -1 : 1))
          .slice(0, REACT_PROPS_LISTED)
          .map(([prop, renders]) => ({ prop, renders })),
      }));
    const taken: InspectionReactCommits = {
      commits: this.#commits,
      renders: tallies.reduce((sum, [, tally]) => sum + tally.renders, 0),
      components,
    };
    this.#counting = false;
    this.#commits = 0;
    this.#tallies = new Map();
    return taken;
  }

  /** Counts one commit of `root`, a fiber root as React hands its hook one, while counting. */
  commit(root: unknown): void {
    if (!this.#counting) {
      return;
    }
    this.#commits += 1;
    const stack: unknown[] = [isFiber(root) ? root["current"] : undefined];
    while (stack.length > 0) {
      const fiber = stack.pop();
      if (!isFiber(fiber)) {
        continue;
      }
      const alternate = fiber["alternate"];
      const before = isFiber(alternate) ? alternate : null;
      const flags = fiber["flags"];
      if (
        COMPONENT_TAGS.has(fiber["tag"]) &&
        (before === null || (typeof flags === "number" && (flags & PERFORMED_WORK) !== 0))
      ) {
        this.#tally(fiber, before);
      }
      // Children that are the last commit's own were not rendered in this one, nor anything below.
      const child = fiber["child"];
      if (before !== null && child === before["child"]) {
        continue;
      }
      for (let each: unknown = child; isFiber(each); each = each["sibling"]) {
        stack.push(each);
      }
    }
  }

  #tally(fiber: Fiber, before: Fiber | null): void {
    const name = nameOf(fiber["type"]);
    let tally = this.#tallies.get(name);
    if (tally === undefined) {
      tally = { renders: 0, mounts: 0, unchanged: 0, stateChanged: 0, props: new Map() };
      this.#tallies.set(name, tally);
    }
    tally.renders += 1;
    if (before === null) {
      tally.mounts += 1;
      return;
    }
    const props = changedProps(before["memoizedProps"], fiber["memoizedProps"]);
    const state = stateChanged(fiber["tag"], before, fiber);
    for (const prop of props) {
      tally.props.set(prop, (tally.props.get(prop) ?? 0) + 1);
    }
    if (state) {
      tally.stateChanged += 1;
    } else if (props.length === 0) {
      tally.unchanged += 1;
    }
  }
}

/** The global React's DevTools hook is set on, and read from by `react-dom` as it loads. */
const HOOK = "__REACT_DEVTOOLS_GLOBAL_HOOK__";

/** The hook {@link installReactCommitHook} sets: React's DevTools hook's members it calls. */
class ReactCommitHook {
  readonly supportsFiber = true;
  readonly isDisabled = false;
  readonly counter = new ReactCommitCounter();
  /** Whether a renderer took the hook as it loaded. */
  injected = false;

  inject(): number {
    this.injected = true;
    return 1;
  }

  onCommitFiberRoot(_renderer: unknown, root: unknown): void {
    this.counter.commit(root);
  }

  onCommitFiberUnmount(): void {
    // Unmounts are not counted.
  }

  onPostCommitFiberRoot(): void {
    // Nothing is read after the passive effects.
  }

  checkDCE(): void {
    // A production build's dead-code check is not this hook's.
  }
}

/**
 * Sets the commit hook on `target`, the page's global, where none is there; it must be set before
 * `react-dom` loads, which takes it then or never.
 */
export function installReactCommitHook(target: object): void {
  if (Reflect.get(target, HOOK) === undefined) {
    Reflect.set(target, HOOK, new ReactCommitHook());
  }
}

/**
 * The counter of the hook set on `target` where React took it as it loaded, or `null`: no hook
 * set, another's, or one set after `react-dom` loaded.
 */
export function reactCommitCounterOf(target: object): ReactCommitCounter | null {
  const hook: unknown = Reflect.get(target, HOOK);
  return hook instanceof ReactCommitHook && hook.injected ? hook.counter : null;
}
