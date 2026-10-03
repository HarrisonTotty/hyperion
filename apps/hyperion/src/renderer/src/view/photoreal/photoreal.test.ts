import { describe, expect, it } from "vitest";

import { CLEAR_COLOUR } from "../engine/webgpu/drawing";
import { HDR_COLOUR_FORMAT } from "../photometry/toneCurve";
import { METER_CLASS } from "../post/meter";
import { QUALITY_SETTINGS } from "../quality/qualitySetting";
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

  it("leaves the slots of plans not yet built empty", () => {
    const empty = photorealisticPasses("high").passes.filter((pass) => !pass.built);
    expect(new Set(empty.map((pass) => pass.owner))).toEqual(new Set(["R06", "R08", "R11"]));
  });
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
