import { useEffect, useState } from "react";

const TICK_INTERVAL_MS = 200;

function currentSecond(): number {
  return Math.floor(Date.now() / 1000);
}

function formatUtc(epochSeconds: number): string {
  return new Date(epochSeconds * 1000).toISOString().slice(11, 19);
}

/**
 * Labelled wall-clock time in UTC, for the header strip.
 *
 * @remarks
 * Polls faster than once a second so the display never skips a second, but only re-renders when
 * the second changes.
 */
export function UtcClock() {
  const [epochSeconds, setEpochSeconds] = useState(currentSecond);

  useEffect(() => {
    const timer = setInterval(() => {
      setEpochSeconds(currentSecond());
    }, TICK_INTERVAL_MS);
    return () => {
      clearInterval(timer);
    };
  }, []);

  return (
    <p className="field">
      <span className="field__label">UTC</span>
      <time className="field__value" dateTime={new Date(epochSeconds * 1000).toISOString()}>
        {formatUtc(epochSeconds)}
      </time>
    </p>
  );
}
