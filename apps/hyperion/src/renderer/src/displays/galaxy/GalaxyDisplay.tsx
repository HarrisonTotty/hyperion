/**
 * The `GALAXY` display: universe, parameters, galaxy map and local chart.
 *
 * @remarks
 * A placeholder panel until the request layer of plan 04 lands and plan 05's P05.T6.d builds the
 * display. It states only what is true: no universe can be open yet.
 */
export function GalaxyDisplay() {
  return (
    <section className="panel" aria-labelledby="galaxy-title">
      <h2 className="panel__title" id="galaxy-title">
        Galaxy
      </h2>
      <p className="panel__empty">NO UNIVERSE OPEN</p>
    </section>
  );
}
