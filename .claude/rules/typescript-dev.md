---
paths:
  - "**/*.{ts,tsx,mts,cts}"
  - "**/package.json"
  - "**/tsconfig*.json"
  - ".oxlintrc.json"
---

# TypeScript Development

Applies to `apps/hyperion` (Electron + React) and `packages/*`. Formatting is owned by Prettier;
never hand-format or argue with its output.

## Workflow

- After changing TypeScript run `pnpm typecheck`, `pnpm lint` and `pnpm test`. Iterate on one file
  with `pnpm --filter hyperion exec vitest run <path>`.
- `oxlint --type-aware --deny-warnings` makes every warning a failure. Fix the code rather than
  silencing the rule.
- When a rule is genuinely wrong, disable it for one line with
  `// oxlint-disable-next-line <rule>` and put the reason on the comment line above it. Never
  disable a rule for a whole file or in `.oxlintrc.json` without asking.
- Never use `@ts-ignore`. Use `@ts-expect-error` with a reason, and only in tests.
- Do not loosen `tsconfig.base.json`. Code must pass under `strict`, `noUncheckedIndexedAccess`,
  `exactOptionalPropertyTypes` and `noPropertyAccessFromIndexSignature`.
- TypeScript 7 (native compiler) has no JavaScript compiler API. Do not add tools that need one:
  ESLint with typescript-eslint, ts-node, ts-jest, or compiler-API transformers.
- Add dependencies with `pnpm add --filter <package>`. Tooling shared by the whole repo goes in
  the root `package.json`.

## Package and process boundaries

- `packages/protocol/src/generated` is ts-rs output. Never edit it: change the Rust types in
  `crates/hyperion-protocol` and run `just gen-protocol`.
- All wire encoding and decoding goes through `@hyperion/protocol`. No `JSON.parse` on server
  data anywhere else.
- `src/main` and `src/preload` are Node/Electron code built as CommonJS: use `__dirname`, not
  `import.meta`. `src/renderer` is browser code and never imports `electron`, `node:*` modules, or
  anything from `src/main`.
- The renderer reaches the OS only through `window.hyperion`. Its type lives in
  `src/preload/api.ts`, the one preload file the renderer may import, and as a type only.

## Types

- Never use `any`. Use `unknown` for values of unknown shape and narrow before use.
- Never use the boxed types `Number`, `String`, `Boolean`, `Symbol` or `Object`.
- Model variants as discriminated unions with a literal `type` or `kind` field, matching the wire
  protocol, instead of bags of optional properties.
- Handle unions with an exhaustive `switch` and no `default` branch, so that adding a variant is a
  compile error.
- No non-null assertions (`!`). Narrow, or throw an error that names the broken invariant.
- Avoid `as`. Narrow with `typeof`, `in`, `instanceof` or a type predicate. The allowed forms are
  `as const` and a documented cast at a trust boundary, as in `decodeServerMessage`.
- Conditions must be real booleans, because truthiness checks mishandle `0` and `""`. Write
  `value !== undefined`, `text.length > 0`, `count > 0`.
- Use `??` rather than `||` for defaults, and `===` always.
- With `exactOptionalPropertyTypes`, `prop?: T` means "may be absent". Write `prop: T | undefined`
  when callers must pass the key explicitly.
- Indexed access returns `T | undefined`. Handle the missing case instead of asserting it away.
- Use string-literal unions or `as const` objects. No `enum` and no `namespace`.
- Declare explicit return types on exported functions and hooks. Components and locals may be
  inferred.
- Describe object shapes with `interface` and use `type` for unions, tuples and mapped types.
- Use `import type` for type-only imports.
- Mark data that is not mutated as `readonly` or `ReadonlyArray<T>`, particularly props and
  state.
- Every generic type parameter must appear in the signature at least twice; otherwise remove it.
- Prefer a union parameter to overloads that differ in one argument, and optional parameters to
  overloads that differ only in trailing arguments. Callback parameters are never optional.
- Give physical quantities a unit in the name or a branded type (`rangeKm`, `bearingDeg`). Never
  pass a bare `number` whose unit is ambiguous.

## Async and errors

- Every promise is awaited, returned, or explicitly discarded with `void` plus a `.catch` that
  reports the failure. Never pass an async function where a `void` callback is expected.
- Throw only `Error` objects, with `{ cause }` when rethrowing. A caught value is `unknown`:
  narrow it with `instanceof Error` before reading it.
- Model expected failures, such as connection loss or a rejected command, as state or as a
  result union. Reserve exceptions for bugs.
- Anything that starts work must stop it: effects and classes release their timers, listeners,
  sockets and `AbortController`s on cleanup.
- No `console.log`. Use `console.warn` and `console.error` for real diagnostics only.

## React

- Components and hooks are pure: the same props, state and context give the same output. Never
  mutate props, state, or hook arguments and return values. No side effects during render.
- Call hooks only at the top level of components and custom hooks, before any early return.
  Never call a component as a function.
- Use function components only, one exported component per file, with the file named after it.
  Props are an interface named `<Component>Props` with `readonly` members. Do not use `React.FC`.
- Effects exist to synchronise with external systems: WebSockets, timers, DOM APIs, Electron.
  Do not use an effect to derive state, to react to a user event, or to notify a parent.
- Compute derived values during render. Use `useMemo` only once a calculation is measurably slow.
- To reset a subtree's state, give it a different `key`. Never copy props into state.
- Every effect that subscribes, connects or schedules returns a cleanup. Async work inside an
  effect checks an `ignore` flag or an `AbortSignal` before it sets state.
- Effects run twice in development under `StrictMode`. Fix the missing cleanup instead of
  guarding against the second run.
- Dependency arrays list every reactive value. Never suppress `react/exhaustive-deps`; restructure
  the code instead.
- Subscribe to external mutable sources with `useSyncExternalStore` or a custom hook that owns
  the subscription, as `useServerConnection` does.
- Keep state minimal and local. Lift it only to the nearest common parent that needs it.
- List keys are stable identifiers from the data, never array indexes or random values.
- High-frequency simulation data must not re-render the whole tree. Subscribe at the component
  that displays it, and draw fast instruments on a canvas driven by `requestAnimationFrame`.

## Accessibility and markup

- Use the semantic element before ARIA: `button`, `output`, `meter`, `progress`, `nav`, `dialog`.
  Add `role` or `aria-*` only where no native element fits.
- Every control is reachable and operable by keyboard and has an accessible name.
- Never signal state by colour alone. Pair it with text or an icon, as the connection panel does
  with its status label.
- Visual and interaction decisions follow `docs/frontend/ux-guidelines.md`.

## Electron security

- Every `BrowserWindow` keeps `sandbox: true`, `contextIsolation: true` and
  `nodeIntegration: false`. Never set `webSecurity: false`, `allowRunningInsecureContent`,
  `experimentalFeatures` or `enableBlinkFeatures`, and never add a `<webview>`.
- The preload exposes narrow, purpose-built functions through `contextBridge`, one per operation.
  Never expose `ipcRenderer`, `send`, `invoke`, `on`, or any function that takes a channel name.
- Every `ipcMain.handle` or `ipcMain.on` handler validates `event.senderFrame` and treats its
  arguments as `unknown` until they are validated. Prefer `invoke`/`handle` to `send`/`on`.
- Keep the Content Security Policy in `src/renderer/index.html` strict. Do not add
  `'unsafe-inline'`, `'unsafe-eval'` or a new origin without asking.
- Deny new windows with `setWindowOpenHandler` and block unexpected navigation in
  `will-navigate`. Pass a URL to `shell.openExternal` only after parsing it with `new URL` and
  checking that the protocol is `https:` or `http:`.
- Never load remote content into a window, and deny permission requests by default.

## Documentation

- Every exported function, hook, component, type and constant has a `/** */` TSDoc comment.
- The first sentence is a summary of what the item is for. Put further detail after `@remarks`.
- Do not repeat types in prose. Use `@param` and `@returns` only to add meaning the types lack:
  units, valid ranges, ownership, side effects.
- Document `@throws` for every error a caller is expected to handle, and `@example` for APIs
  whose use is not obvious. Use `{@link Name}` for cross-references and `@deprecated` with the
  replacement.
- Ordinary `//` comments explain why, never what. Do not leave commented-out code or a `TODO`
  without an issue reference.

## Tests

- Tests sit beside the code as `<name>.test.ts` or `<name>.test.tsx`. Shared fakes and setup live
  in `src/renderer/src/test/`.
- Every behaviour change comes with a test, and every bug fix with a regression test that fails
  before the fix.
- Test what the user sees, not the implementation. Query through `screen`, preferring
  `getByRole` with a `name`, then `getByLabelText`, then `getByText`. Use `getByTestId` only as a
  last resort.
- Use `getBy*` for elements that must be present, `queryBy*` only to assert absence, and
  `await findBy*` for elements that appear asynchronously. Never `sleep` in a test.
- Simulate input with `const user = userEvent.setup()` created before `render`, and await every
  call on it. Use `fireEvent` only for events `user-event` cannot produce.
- Name tests as behaviour in plain words: `it("shows NO CARRIER when the socket closes")`. Each
  test has one reason to fail.
- Fake the boundary, not our own modules: stub `WebSocket` with `FakeWebSocket` through
  `vi.stubGlobal`, and stub `window.hyperion`. Reserve `vi.mock` for third-party modules, and
  remember that it is hoisted above the imports.
- Control time with `vi.useFakeTimers()` and `vi.advanceTimersByTime`, and call
  `vi.useRealTimers()` in `afterEach`. Mocks and stubbed globals reset automatically through
  `restoreMocks` and `unstubGlobals`.
- Tests are deterministic and independent: no real network, no real clock, no shared mutable
  module state, no dependence on order.
- Do not use snapshot tests for components. Assert on specific roles and text.
