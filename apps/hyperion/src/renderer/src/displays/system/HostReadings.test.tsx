import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { KM_PER_AU } from "../../lib/format";
import type { HabitableZone, HostBody, Zone } from "../../lib/system/model";
import { HostReadings } from "./HostReadings";
import type { HostZone } from "./useSystemView";

/** Metres in an astronomical unit. */
const M_PER_AU = KM_PER_AU * 1_000;

/** A Sun-like primary, alive, with nothing modelled beyond its light. */
const SUN: HostBody = {
  id: "42002cb20000000d.0000",
  bodyIndex: 0,
  designation: "H7K 4C0RFZ D-7 /0",
  kind: "dwarf",
  phase: "main_sequence",
  spectralClass: "G2V",
  initialMassMsun: 1,
  massMsun: 1,
  luminosityLsun: 1,
  radiusRsun: 1,
  teffK: 5_772,
  absoluteVMag: 4.83,
  remnant: null,
  deathTime: null,
  rotationPeriodD: { kind: "not_modelled" },
  activityLogLxLbol: { kind: "not_modelled" },
  variability: { kind: "not_modelled" },
  planetaryNebula: { kind: "not_modelled" },
  activeEvents: { kind: "not_modelled" },
};

/** Kopparapu et al.'s limits about the Sun, in AU, each limit given. */
const SUN_HZ: HabitableZone = {
  recentVenusM: 0.75 * M_PER_AU,
  runawayGreenhouseM: 0.95 * M_PER_AU,
  moistGreenhouseM: 0.99 * M_PER_AU,
  maximumGreenhouseM: 1.69 * M_PER_AU,
  earlyMarsM: 1.77 * M_PER_AU,
  extrapolated: false,
};

function aZone(overrides: Partial<Zone> = {}): HostZone {
  return {
    about: "H7K 4C0RFZ D-7 /0",
    zone: {
      host: { kind: "star", bodyIndex: 0 },
      innerM: null,
      outerM: null,
      snowLineM: 2.26 * M_PER_AU,
      plane: { inclinationRad: 0, ascendingNodeRad: 0 },
      architecture: "solar_like",
      habitableZone: SUN_HZ,
      ...overrides,
    },
  };
}

function renderHost(zone: HostZone) {
  const view = (next: HostZone) => (
    <dl>
      <HostReadings host={SUN} zones={[next]} />
    </dl>
  );
  const { rerender } = render(view(zone));
  return { rerender: (next: HostZone) => rerender(view(next)) };
}

/** The value read beside `label`, and the part of it in the row's unit slot. */
function reading(label: string): { readonly text: string; readonly unit: string | null } {
  const dd = screen.getByText(label, { selector: "dt" }).nextElementSibling;
  return {
    text: dd?.textContent ?? "",
    unit: dd?.querySelector(".body-readout__unit")?.textContent ?? null,
  };
}

describe("HostReadings' zone spans (ruling 113.3)", () => {
  it("reads a habitable zone's two limits as one reading, its unit in the unit slot", () => {
    renderHost(aZone());

    expect(reading("HABITABLE ZONE")).toEqual({ text: "0.990 – 1.69 AU", unit: "AU" });
    expect(reading("OPTIMISTIC")).toEqual({ text: "0.750 – 1.77 AU", unit: "AU" });
  });

  it("marks each end of an extrapolated zone estimated", () => {
    renderHost(aZone({ habitableZone: { ...SUN_HZ, extrapolated: true } }));

    expect(reading("HABITABLE ZONE")).toEqual({ text: "~0.990 – ~1.69 AU", unit: "AU" });
  });

  it("reads FROM an inner limit whose outer limit lies beyond every orbit", () => {
    renderHost(
      aZone({ habitableZone: { ...SUN_HZ, maximumGreenhouseM: null, extrapolated: true } }),
    );

    expect(reading("HABITABLE ZONE")).toEqual({ text: "FROM ~0.990 AU", unit: "AU" });
  });

  it("reads NONE where a host has no habitable zone", () => {
    renderHost(aZone({ habitableZone: null }));

    expect(reading("HABITABLE ZONE")).toEqual({ text: "NONE", unit: null });
    expect(reading("OPTIMISTIC")).toEqual({ text: "NONE", unit: null });
  });

  it("reads a stable zone's two limits as one reading in the outer limit's unit", () => {
    // An inner limit of 12.0 Gm reads in the outer limit's AU, to three figures (ruling 113.3).
    renderHost(aZone({ innerM: 12e9, outerM: 0.15 * M_PER_AU }));

    expect(reading("STABLE ZONE")).toEqual({ text: "0.0802 – 0.150 AU", unit: "AU" });
  });

  it("keeps the unit beside the one number where a stable limit is NONE", () => {
    renderHost(aZone({ outerM: 1.69 * M_PER_AU }));
    expect(reading("STABLE ZONE")).toEqual({ text: "NONE – 1.69 AU", unit: null });
  });

  it("writes an inner stable limit alone before its NONE", () => {
    renderHost(aZone({ innerM: 0.989 * M_PER_AU }));
    expect(reading("STABLE ZONE")).toEqual({ text: "0.989 AU – NONE", unit: null });
  });

  it("leaves the stable zone out where no companion sets it", () => {
    renderHost(aZone());

    expect(screen.queryByText("STABLE ZONE")).not.toBeInTheDocument();
  });

  it("holds a span's unit across the boundary with hysteresis, as a belt's edges do", () => {
    // 0.1 AU is where Gm gives way to AU; an outer limit just inside it keeps the AU it was in.
    const { rerender } = renderHost(aZone({ innerM: 5e9, outerM: 0.2 * M_PER_AU }));
    expect(reading("STABLE ZONE").unit).toBe("AU");

    rerender(aZone({ innerM: 5e9, outerM: 0.098 * M_PER_AU }));
    expect(reading("STABLE ZONE")).toEqual({ text: "0.0334 – 0.0980 AU", unit: "AU" });

    // Beyond the margin, the unit changes.
    rerender(aZone({ innerM: 5e9, outerM: 0.05 * M_PER_AU }));
    expect(reading("STABLE ZONE")).toEqual({ text: "5.00 – 7.48 Gm", unit: "Gm" });
  });
});

describe("HostReadings' ID row", () => {
  it("may break only after the full stop, so that a narrow readout never scrolls sideways", () => {
    renderHost(aZone());

    const dd = screen.getByText("ID", { selector: "dt" }).nextElementSibling;
    expect(dd).toHaveTextContent("42002CB20000000D.0000");
    const value = dd?.querySelector(".body-readout__value");
    expect(value?.innerHTML).toBe("42002CB20000000D.<wbr>0000");
  });
});
