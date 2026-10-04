import { describe, expect, it } from "vitest";

import { type RenderStyle, viewId } from "../camera/state";
import { QUALITY_SETTINGS, SETTINGS } from "../quality/qualitySetting";
import { ResolutionController } from "./resolutionController";
import {
  frameGpuBudgetMs,
  GPU_FRAME_SHARE,
  ONE_PHOTOREALISTIC_VIEW,
  PER_CANVAS_OVERHEAD_MS,
  photorealisticAllowed,
  type ViewBudget,
  viewBudgets,
  type ViewSpec,
} from "./viewBudget";

const COCKPIT = viewId("cockpit");
const LEFT = viewId("instrument 1");
const RIGHT = viewId("instrument 2");

function primary(style: RenderStyle): ViewSpec {
  return { id: COCKPIT, slot: "primary", style };
}

function instrument(id: ViewSpec["id"], style: RenderStyle = "wireframe"): ViewSpec {
  return { id, slot: "instrument", style };
}

function budgetOf(
  budgets: ReadonlyMap<ViewSpec["id"], ViewBudget>,
  id: ViewSpec["id"],
): ViewBudget {
  const budget = budgets.get(id);
  if (budget === undefined) {
    throw new Error(`no budget for ${id}`);
  }
  return budget;
}

/** A photorealistic primary's GPU time at a scale, ms: a fixed part and its pixels. */
function primaryMs(scale: number): number {
  return 2 + 10 * scale * scale;
}

describe("photorealisticAllowed", () => {
  it("refuses a second photorealistic view on the low setting, saying why", () => {
    const views = [primary("photorealistic"), instrument(LEFT)];
    expect(photorealisticAllowed(views, "low", LEFT)).toEqual({
      allowed: false,
      reason: "ONE PHOTOREALISTIC VIEW ON LOW SETTING",
    });
    expect(ONE_PHOTOREALISTIC_VIEW).toBe("ONE PHOTOREALISTIC VIEW ON LOW SETTING");
  });

  it("allows the view that holds the low setting's one photorealistic view", () => {
    const views = [primary("photorealistic"), instrument(LEFT)];
    expect(photorealisticAllowed(views, "low", COCKPIT)).toEqual({ allowed: true });
  });

  it("refuses it to the primary while an instrument holds it on the low setting", () => {
    const views = [primary("wireframe"), instrument(LEFT, "photorealistic")];
    expect(photorealisticAllowed(views, "low", COCKPIT)).toMatchObject({ allowed: false });
    expect(photorealisticAllowed(views, "low", LEFT)).toEqual({ allowed: true });
  });

  it("allows every view on the high setting", () => {
    const views = [
      primary("photorealistic"),
      instrument(LEFT, "photorealistic"),
      instrument(RIGHT),
    ];
    for (const view of views) {
      expect(photorealisticAllowed(views, "high", view.id)).toEqual({ allowed: true });
    }
  });

  it("refuses a view that is not among the views", () => {
    expect(() => photorealisticAllowed([primary("wireframe")], "low", LEFT)).toThrow(RangeError);
  });
});

describe("viewBudgets", () => {
  it("draws only one photorealistic view on the low setting, the primary first", () => {
    const views = [
      primary("photorealistic"),
      instrument(LEFT, "photorealistic"),
      instrument(RIGHT, "photorealistic"),
    ];
    const low = viewBudgets(views, "low");
    expect([COCKPIT, LEFT, RIGHT].map((id) => budgetOf(low, id).style)).toEqual([
      "photorealistic",
      "wireframe",
      "wireframe",
    ]);
    const high = viewBudgets(views, "high");
    expect([COCKPIT, LEFT, RIGHT].map((id) => budgetOf(high, id).style)).toEqual([
      "photorealistic",
      "photorealistic",
      "photorealistic",
    ]);
  });

  it.each(QUALITY_SETTINGS)(
    "renders each secondary view at a lower scale or 30 Hz on the %s setting",
    (setting) => {
      const views = [
        primary("photorealistic"),
        instrument(LEFT),
        instrument(RIGHT, "photorealistic"),
      ];
      const budgets = viewBudgets(views, setting);
      const main = budgetOf(budgets, COCKPIT);
      for (const id of [LEFT, RIGHT]) {
        const budget = budgetOf(budgets, id);
        expect(budget.rateHz === 30 || budget.renderScale < main.renderScale).toBe(true);
      }
    },
  );

  it("renders the instruments below a photorealistic primary's rate on the high setting", () => {
    const budgets = viewBudgets([primary("photorealistic"), instrument(LEFT)], "high");
    expect(budgetOf(budgets, LEFT).rateHz).toBeLessThan(budgetOf(budgets, COCKPIT).rateHz);
  });

  it("renders the instruments below a wireframe primary's rate on the low setting", () => {
    const budgets = viewBudgets([primary("wireframe"), instrument(LEFT)], "low");
    expect(budgetOf(budgets, LEFT).rateHz).toBeLessThan(budgetOf(budgets, COCKPIT).rateHz);
  });

  it("streams the instruments' terrain at the secondary priority", () => {
    const budgets = viewBudgets([primary("photorealistic"), instrument(LEFT)], "high");
    expect(budgetOf(budgets, COCKPIT).streamPriority).toBe("primary");
    expect(budgetOf(budgets, LEFT).streamPriority).toBe("secondary");
  });

  it("never controls an instrument's scale", () => {
    const views = [
      primary("photorealistic"),
      instrument(LEFT, "photorealistic"),
      instrument(RIGHT),
    ];
    const budgets = viewBudgets(views, "high");
    expect([LEFT, RIGHT].map((id) => budgetOf(budgets, id).control)).toEqual([null, null]);
  });

  it("budgets a photorealistic primary at 60 Hz on the high setting", () => {
    expect(budgetOf(viewBudgets([primary("photorealistic")], "high"), COCKPIT).rateHz).toBe(60);
  });

  it("budgets a photorealistic primary at 30 Hz on the low setting", () => {
    expect(budgetOf(viewBudgets([primary("photorealistic")], "low"), COCKPIT).rateHz).toBe(30);
  });

  it("keeps a wireframe primary at 60 Hz on the low setting", () => {
    expect(budgetOf(viewBudgets([primary("wireframe")], "low"), COCKPIT).rateHz).toBe(60);
  });

  it("renders a wireframe view at full scale and leaves it uncontrolled", () => {
    const budgets = viewBudgets([primary("wireframe"), instrument(LEFT)], "high");
    expect(budgetOf(budgets, COCKPIT)).toMatchObject({ renderScale: 1, control: null });
    expect(budgetOf(budgets, LEFT)).toMatchObject({ renderScale: 1, control: null });
  });

  it.each(QUALITY_SETTINGS)(
    "renders a photorealistic primary alone at the bounds' max, uncontrolled, on %s",
    (setting) => {
      const alone = budgetOf(viewBudgets([primary("photorealistic")], setting), COCKPIT);
      expect(alone).toMatchObject({
        renderScale: SETTINGS[setting].internalScaleBounds[1],
        control: null,
      });
    },
  );

  it.each(QUALITY_SETTINGS)(
    "controls a photorealistic primary's scale within %s's bounds while instruments are open",
    (setting) => {
      const open = viewBudgets([primary("photorealistic"), instrument(LEFT)], setting);
      expect(budgetOf(open, COCKPIT).control?.bounds).toEqual(
        SETTINGS[setting].internalScaleBounds,
      );
    },
  );

  it("drops the primary's scale while instruments are open", () => {
    // The primary alone fits its frame at full scale; two instruments add 1.5 ms each.
    expect(primaryMs(1)).toBeLessThanOrEqual(frameGpuBudgetMs(60, 0));
    const budgets = viewBudgets(
      [primary("photorealistic"), instrument(LEFT), instrument(RIGHT)],
      "high",
    );
    const control = budgetOf(budgets, COCKPIT).control;
    if (control === null) {
      throw new Error("the primary should be controlled while instruments are open");
    }
    const controller = new ResolutionController(control.bounds, control.target);
    for (let frame = 0; frame < 600; frame += 1) {
      controller.update([primaryMs(controller.scale) + 2 * 1.5], control.target.periodMs);
    }
    expect(controller.scale).toBeLessThan(1);
    expect(controller.scale).toBeGreaterThanOrEqual(control.bounds[0]);
    expect(primaryMs(controller.scale) + 3).toBeLessThanOrEqual(control.target.gpuBudgetMs);
  });

  it.each(QUALITY_SETTINGS)(
    "takes PER_CANVAS_OVERHEAD_MS from the frame's budget for each instrument on %s",
    (setting) => {
      const rateHz = SETTINGS[setting].budget.photorealisticRateHz;
      const periodMs = 1000 / rateHz;
      const one = viewBudgets([primary("photorealistic"), instrument(LEFT)], setting);
      const two = viewBudgets(
        [primary("photorealistic"), instrument(LEFT), instrument(RIGHT)],
        setting,
      );
      expect(budgetOf(one, COCKPIT).control?.target.periodMs).toBe(periodMs);
      expect(budgetOf(one, COCKPIT).control?.target.gpuBudgetMs).toBeCloseTo(
        GPU_FRAME_SHARE * periodMs - PER_CANVAS_OVERHEAD_MS,
        12,
      );
      expect(budgetOf(two, COCKPIT).control?.target.gpuBudgetMs).toBeCloseTo(
        GPU_FRAME_SHARE * periodMs - 2 * PER_CANVAS_OVERHEAD_MS,
        12,
      );
    },
  );

  it("starts PER_CANVAS_OVERHEAD_MS at the provisional 0.3 ms", () => {
    expect(PER_CANVAS_OVERHEAD_MS).toBe(0.3);
  });

  it("refuses views without exactly one primary, or with an identity listed twice", () => {
    expect(() => viewBudgets([instrument(LEFT)], "high")).toThrow(RangeError);
    expect(() =>
      viewBudgets([primary("wireframe"), { ...primary("wireframe"), id: LEFT }], "high"),
    ).toThrow(RangeError);
    expect(() =>
      viewBudgets([primary("wireframe"), instrument(LEFT), instrument(LEFT)], "high"),
    ).toThrow(RangeError);
  });
});
