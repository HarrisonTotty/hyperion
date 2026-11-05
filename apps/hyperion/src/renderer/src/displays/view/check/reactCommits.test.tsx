import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it } from "vitest";

import { installReactCommitHook, ReactCommitCounter, reactCommitCounterOf } from "./reactCommits";

/** A fiber as React's internals hold it, the fields the counter reads. */
interface FakeFiber {
  tag: number;
  type: unknown;
  flags: number;
  child: FakeFiber | null;
  sibling: FakeFiber | null;
  alternate: FakeFiber | null;
  memoizedProps: Record<string, unknown>;
  memoizedState: unknown;
}

function Plate(): null {
  return null;
}

function Readout(): null {
  return null;
}

/** A label, as a component that renders its parent's text. */
function Shown({ label }: { readonly label: string }) {
  return <p>{label}</p>;
}

/** A count its button raises, shown beside a fixed label. */
function Stage() {
  const [count, setCount] = useState(0);
  return (
    <div>
      <button
        type="button"
        onClick={() => {
          setCount((value) => value + 1);
        }}
      >
        MORE
      </button>
      <Shown label="FIXED" />
      <Shown label={String(count)} />
    </div>
  );
}

/** A function component's fiber with `props`, its children as given. */
function fiber(
  type: unknown,
  props: Record<string, unknown>,
  children: FakeFiber[] = [],
  flags = 1,
): FakeFiber {
  const made: FakeFiber = {
    tag: 0,
    type,
    flags,
    child: children[0] ?? null,
    sibling: null,
    alternate: null,
    memoizedProps: props,
    memoizedState: null,
  };
  children.forEach((each, i) => {
    each.sibling = children[i + 1] ?? null;
  });
  return made;
}

/** A root whose host fiber holds `child`, as a commit hands it over. */
function rootOf(child: FakeFiber, alternate: FakeFiber | null): { current: FakeFiber } {
  const host = fiber(null, {}, [child], 0);
  host.tag = 3;
  host.alternate = alternate;
  return { current: host };
}

describe("the frame profile's React commits (lane perf-render-thread-2)", () => {
  it("counts a component's renders by what changed, and nothing below a subtree left as it was", () => {
    const counter = new ReactCommitCounter();
    counter.begin();
    const readoutBefore = fiber(Readout, { value: 1 });
    const plateBefore = fiber(Plate, { lines: "a", faults: "b" }, [readoutBefore]);
    const hostBefore = rootOf(plateBefore, null).current;
    // The plate rendered again with a new `lines`; its readout's subtree is the last commit's own.
    const plate = fiber(Plate, { lines: "c", faults: "b" }, []);
    plate.child = readoutBefore;
    plate.alternate = plateBefore;
    plateBefore.alternate = plate;
    counter.commit(rootOf(plate, hostBefore));
    // Again with nothing changed, and then cloned but not rendered (no PerformedWork).
    const same = fiber(Plate, { lines: "c", faults: "b" }, [], 1);
    same.child = readoutBefore;
    same.alternate = plate;
    counter.commit(rootOf(same, hostBefore));
    const skipped = fiber(Plate, { lines: "c", faults: "b" }, [], 0);
    skipped.child = readoutBefore;
    skipped.alternate = same;
    counter.commit(rootOf(skipped, hostBefore));
    expect(counter.take()).toEqual({
      commits: 3,
      renders: 2,
      components: [
        {
          name: "Plate",
          renders: 2,
          mounts: 0,
          unchanged: 1,
          stateChanged: 0,
          propsChanged: [{ prop: "lines", renders: 1 }],
        },
      ],
    });
  });

  it("counts a mount's whole subtree, and a state hook's change apart from its effects'", () => {
    const counter = new ReactCommitCounter();
    counter.begin();
    counter.commit(rootOf(fiber(Plate, {}, [fiber(Readout, {})]), null));
    const before = fiber(Readout, { value: 1 });
    // A state hook, then an effect hook, whose object is new at every render.
    before.memoizedState = {
      memoizedState: 1,
      queue: {},
      next: { memoizedState: {}, queue: null, next: null },
    };
    const after = fiber(Readout, { value: 1 });
    after.memoizedState = {
      memoizedState: 1,
      queue: {},
      next: { memoizedState: {}, queue: null, next: null },
    };
    after.alternate = before;
    counter.commit(rootOf(after, fiber(null, {}, [before], 0)));
    const changed = fiber(Readout, { value: 1 });
    changed.memoizedState = { memoizedState: 2, queue: {}, next: null };
    changed.alternate = after;
    counter.commit(rootOf(changed, fiber(null, {}, [after], 0)));
    const taken = counter.take();
    expect([
      taken.commits,
      taken.components.map((each) => [
        each.name,
        each.renders,
        each.mounts,
        each.unchanged,
        each.stateChanged,
      ]),
    ]).toEqual([
      3,
      [
        ["Readout", 3, 1, 1, 1],
        ["Plate", 1, 1, 0, 0],
      ],
    ]);
  });

  it("counts nothing before its begin or after its take", () => {
    const counter = new ReactCommitCounter();
    counter.commit(rootOf(fiber(Plate, {}), null));
    counter.begin();
    expect(counter.take()).toEqual({ commits: 0, renders: 0, components: [] });
    counter.commit(rootOf(fiber(Plate, {}), null));
    expect(counter.take().commits).toBe(0);
  });

  it("is taken by React's renderer and counts its commits, a parent's render apart from a state's", async () => {
    const page: object = {};
    expect(reactCommitCounterOf(page)).toBeNull();
    installReactCommitHook(page);
    expect(reactCommitCounterOf(page)).toBeNull();
    // React takes the hook as it loads; this test's has loaded, so it is handed over as React does.
    const hook: unknown = Reflect.get(page, "__REACT_DEVTOOLS_GLOBAL_HOOK__");
    if (
      typeof hook !== "object" ||
      hook === null ||
      !("inject" in hook) ||
      typeof hook.inject !== "function" ||
      !("onCommitFiberRoot" in hook) ||
      typeof hook.onCommitFiberRoot !== "function"
    ) {
      throw new Error("the hook is set");
    }
    Reflect.apply(hook.inject, hook, [{}]);
    const counter = reactCommitCounterOf(page);
    if (counter === null) {
      throw new Error("the hook is taken");
    }
    const user = userEvent.setup();
    const { container } = render(<Stage />);
    const more = screen.getByRole("button", { name: "MORE" });
    await user.click(more);
    counter.begin();
    await user.click(more);
    // The fiber root, as React hands its hook one at each commit: the container's host fiber's.
    const key = Object.keys(container).find((each) => each.startsWith("__reactContainer$"));
    const host: unknown = key === undefined ? undefined : Reflect.get(container, key);
    const fiberRoot: unknown =
      typeof host === "object" && host !== null ? Reflect.get(host, "stateNode") : undefined;
    Reflect.apply(hook.onCommitFiberRoot, hook, [1, fiberRoot]);
    const taken = counter.take();
    expect(
      taken.components.map((each) => [
        each.name,
        each.renders,
        each.unchanged,
        each.stateChanged,
        each.propsChanged,
      ]),
    ).toEqual([
      ["Shown", 2, 1, 0, [{ prop: "label", renders: 1 }]],
      ["Stage", 1, 0, 1, []],
    ]);
  });
});
