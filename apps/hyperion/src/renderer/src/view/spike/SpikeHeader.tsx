import { LinkStatus } from "../../components/LinkStatus";
import type { ConnectionStatus } from "../../lib/connection";

/** Props of {@link SpikeHeader}. */
export interface SpikeHeaderProps {
  /**
   * The server link's state over a served world (plan R10, R10.T15.b), which the status area shows
   * beside the banner; absent over every other world, which links to no server.
   */
  readonly link?: ConnectionStatus | undefined;
}

/**
 * The descent spike's header strip: the ship's name, the display's title and the `MEASUREMENT`
 * banner, which stands the whole run (the guide's "Layout"), and over a served world the server
 * link's annunciator (`LinkStatus`), the state of the link its terrain comes over.
 */
export function SpikeHeader({ link }: SpikeHeaderProps) {
  return (
    <header className="console__header">
      <div className="console__identity">
        <span className="console__ship">HYPERION</span>
        <h1 className="console__title">DESCENT SPIKE</h1>
      </div>
      <div className="console__status">
        <output className="console__banner" aria-label="Mode">
          MEASUREMENT
        </output>
        {link === undefined ? null : <LinkStatus status={link} />}
      </div>
    </header>
  );
}
