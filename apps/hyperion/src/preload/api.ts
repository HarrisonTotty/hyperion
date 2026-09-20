/** The API the preload script exposes to the renderer as `window.hyperion`. */
export interface HyperionApi {
  readonly platform: string;
  readonly versions: {
    readonly electron: string;
    readonly chrome: string;
    readonly node: string;
  };
}

declare global {
  interface Window {
    readonly hyperion: HyperionApi;
  }
}
