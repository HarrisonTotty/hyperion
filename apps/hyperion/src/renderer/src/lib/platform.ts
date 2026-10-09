/**
 * The keyboard's primary modifier: Meta (⌘, Command) on macOS, and Ctrl on Linux and Windows, as
 * each platform's own shortcuts take it (plan R07, T19.f).
 */
export type PrimaryModifier = "meta" | "ctrl";

/**
 * The primary modifier on a platform.
 *
 * @param platform - `window.hyperion.platform`, Node's `process.platform`: `darwin` is macOS.
 */
export function primaryModifierOf(platform: string): PrimaryModifier {
  return platform === "darwin" ? "meta" : "ctrl";
}

/** `KeyboardEvent.key`'s name for each primary modifier's own key. */
export const MODIFIER_KEY: Readonly<Record<PrimaryModifier, string>> = {
  meta: "Meta",
  ctrl: "Control",
};
