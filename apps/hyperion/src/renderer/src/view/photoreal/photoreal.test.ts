import { describe, expect, it } from "vitest";

import { CLEAR_COLOUR } from "../engine/webgpu/drawing";
import { HDR_COLOUR_FORMAT } from "../photometry/toneCurve";
import { METER_CLASS } from "../post/meter";
import { QUALITY_SETTINGS } from "../quality/qualitySetting";
import { TERRAIN_PASS_LABEL, TERRAIN_SHADOW_PASS_LABEL } from "../terrain/gpu/material";
import { PHOTOREAL_PASS_LABELS, photorealisticPasses } from "./passes";
import { sceneTargetSpec } from "./sceneTarget";
import { aCameraScene } from "../../test/viewFixtures";
import { newCameraState } from "../camera/state";
import { otherStyle, styleName, styleRefusal, withStyle } from "./style";

describe("photorealisticPasses", () => {
  it.each(QUALITY_SETTINGS)("labels every pass once on the %s setting", (setting) => {
    const labels = photorealisticPasses(setting).passes.map((pass) => pass.label);
    expect(new Set(labels).size).toBe(labels.length);
  });

  it.each(QUALITY_SETTINGS)("takes this plan's labels for its own passes on %s", (setting) => {
    const own = photorealisticPasses(setting).passes.filter((pass) => pass.owner === "R07");
    expect(own.map((pass) => pass.label)).toEqual(Object.values(PHOTOREAL_PASS_LABELS));
  });

  it("orders the sky first, the bodies and discs before the post passes, symbology last", () => {
    const labels = photorealisticPasses("high").passes.map((pass) => pass.label);
    expect(labels[0]).toBe("sky");
    expect(labels.indexOf("bodies")).toBeLessThan(labels.indexOf("discs"));
    expect(labels.indexOf("discs")).toBeLessThan(labels.indexOf("histogram"));
    expect(labels.slice(-4)).toEqual(["histogram", "bloom", "tonemap", "symbology"]);
  });

  it("puts the disc cells between the sky and the discs, before the mesh bodies that read them (R07.T8.d)", () => {
    const labels = photorealisticPasses("high").passes.map((pass) => pass.label);
    expect(labels.slice(0, 2)).toEqual(["sky", "disc cells"]);
    expect(labels.indexOf("disc cells")).toBeLessThan(labels.indexOf("bodies"));
    expect(labels.indexOf("disc cells")).toBeLessThan(labels.indexOf("discs"));
  });

  it.each(QUALITY_SETTINGS)(
    "puts R10's terrain shadows, built, just before the bodies and the terrain on %s (R10.T9.b)",
    (setting) => {
      const passes = photorealisticPasses(setting).passes;
      const labels = passes.map((pass) => pass.label);
      const at = labels.indexOf(TERRAIN_SHADOW_PASS_LABEL);
      expect(passes[at]).toEqual({ label: "terrain shadows", owner: "R10", built: true });
      expect(labels.slice(at, at + 2)).toEqual([TERRAIN_SHADOW_PASS_LABEL, "bodies"]);
      expect(at).toBeLessThan(labels.indexOf(TERRAIN_PASS_LABEL));
    },
  );

  it("leaves the slots of plans not yet built empty, R06's sky built (R07.T8.a)", () => {
    const empty = photorealisticPasses("high").passes.filter((pass) => !pass.built);
    expect(new Set(empty.map((pass) => pass.owner))).toEqual(new Set(["R11"]));
  });

  it.each(QUALITY_SETTINGS)(
    "builds R08's atmosphere after the discs and before R11's rings, with its passes' labels, on %s (R08.T10.b)",
    (setting) => {
      const passes = photorealisticPasses(setting).passes;
      const labels = passes.map((pass) => pass.label);
      const at = labels.indexOf("atmosphere");
      expect(passes[at]).toEqual({
        label: "atmosphere",
        owner: "R08",
        built: true,
        labels: ["atmosphere tables", "atmosphere view", "atmosphere composite"],
      });
      expect(labels.slice(at - 1, at + 2)).toEqual(["discs", "atmosphere", "rings"]);
    },
  );
});

describe("sceneTargetSpec", () => {
  it("makes the view's HDR target in R02's format with its own depth and one mip", () => {
    expect(sceneTargetSpec("view 1", { widthPx: 640, heightPx: 360 })).toEqual({
      name: "view 1:hdr",
      size: { widthPx: 640, heightPx: 360 },
      format: HDR_COLOUR_FORMAT,
      mips: 1,
      depth: true,
      category: "render-targets",
    });
  });

  it("clears to the alpha of METER_CLASS.other, so an unwritten pixel meters as other", () => {
    expect(CLEAR_COLOUR).toMatchObject({ a: METER_CLASS.other });
  });
});

describe("the style switch", () => {
  it("names each style as the label block shows it", () => {
    expect([styleName("wireframe"), styleName("photorealistic")]).toEqual([
      "WIREFRAME",
      "PHOTOREALISTIC",
    ]);
  });

  it("refuses the photorealistic style on a software adapter only", () => {
    expect(styleRefusal("photorealistic", { wireframe: true, photorealistic: false })).toBe(
      "GRAPHICS SOFTWARE ADAPTER: photorealistic style not available",
    );
    expect(styleRefusal("photorealistic", { wireframe: true, photorealistic: true })).toBeNull();
    expect(styleRefusal("wireframe", { wireframe: true, photorealistic: false })).toBeNull();
  });

  it("changes the style and nothing else: camera, pose and field of view stay", () => {
    const camera = newCameraState(aCameraScene(), "eye");
    const styled = withStyle(camera, "photorealistic", { wireframe: true, photorealistic: true });
    expect(styled.style).toBe("photorealistic");
    expect({ ...styled, style: camera.style }).toEqual(camera);
    // The projection is built from the pose and the field of view alone (ViewDisplay's
    // `{ pose, fovXRad }`), so every body projects to the same pixel in both styles.
    expect([styled.pose, styled.fovDeg]).toEqual([camera.pose, camera.fovDeg]);
  });

  it("leaves the camera unchanged where the style is refused", () => {
    const camera = newCameraState(aCameraScene(), "eye");
    expect(withStyle(camera, "photorealistic", { wireframe: true, photorealistic: false })).toBe(
      camera,
    );
  });

  it("toggles between the two styles", () => {
    expect(otherStyle("wireframe")).toBe("photorealistic");
    expect(otherStyle("photorealistic")).toBe("wireframe");
  });
});
