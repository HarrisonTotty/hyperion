interface ImportMetaEnv {
  /** WebSocket URL of the hyperion-server to connect to. */
  readonly VITE_HYPERION_SERVER_URL?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}

/** Injected at build time from package.json. */
declare const __APP_VERSION__: string;
