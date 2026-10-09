/**
 * The app's version from `package.json`, injected at build time (`electron.vite.config.mts`).
 *
 * @remarks
 * `app.getVersion()` gives Electron's own version when the client is started on its built
 * `out/main/index.js`, as the descent spike's launch does, since no `package.json` is beside it.
 */
declare const __APP_VERSION__: string;
