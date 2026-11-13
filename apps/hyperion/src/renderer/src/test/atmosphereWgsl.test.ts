import { describe, expect, it } from "vitest";

import { arraysByValueFrom, wgslCode, wgslFunctions, wgslStructs } from "./atmosphereWgsl";

describe("the atmosphere's static WGSL checks", () => {
  it("strips a block comment's opener inside a line comment as part of that comment", () => {
    expect(wgslCode("let a = 1; // shaders/*.wgsl\nlet b = 2; /* c */ let d = 3;")).toBe(
      "let a = 1;  \nlet b = 2;   let d = 3;",
    );
  });

  it("reads each function's parameters, return type and body, and each struct's members", () => {
    const wgsl = `
      struct Pair { a : ptr<function, array<f32, 4>>, b : vec3f, }
      fn f(x : f32, y : array<vec3f, 6>) -> vec2f { return vec2f(x, y[0].x); }
      fn g() { }
    `;
    expect(wgslFunctions(wgsl).get("f")).toEqual({
      params: [
        { name: "x", type: "f32" },
        { name: "y", type: "array<vec3f, 6>" },
      ],
      returns: "vec2f",
      body: " return vec2f(x, y[0].x); ",
    });
    expect(wgslFunctions(wgsl).get("g")?.returns).toBe("");
    expect(wgslStructs(wgsl).get("Pair")).toEqual(["ptr<function, array<f32, 4>>", "vec3f"]);
  });

  it("finds an array taken or returned by value, and a struct that holds one, through the calls", () => {
    const wgsl = `
      struct Inner { a : array<f32, 4>, }
      struct Outer { inner : Inner, x : f32, }
      struct Plain { x : f32, }
      fn leaf(values : array<vec3f, 6>, plain : Plain) -> f32 { return values[0].x; }
      fn held(o : Outer) -> array<f32, 2> {
        return array<f32, 2>(leaf(array<vec3f, 6>(), Plain(0.0)), o.x);
      }
      fn fine(p : ptr<function, array<f32, 4>>, plain : Plain) -> f32 { return (*p)[0] + plain.x; }
      fn step(p : ptr<function, Outer>) { let unused = held(*p); }
      fn unreached(values : array<f32, 3>) {}
    `;
    expect(arraysByValueFrom(wgsl, ["step", "fine"])).toEqual([
      "held o : Outer",
      "held -> array<f32, 2>",
      "leaf values : array<vec3f, 6>",
    ]);
  });

  it("finds a struct that holds an array returned by value", () => {
    const wgsl = `
      struct Inner { a : array<f32, 4>, }
      struct Outer { inner : Inner, }
      fn made() -> Outer { var o : Outer; return o; }
      fn step() { let o = made(); }
    `;
    expect(arraysByValueFrom(wgsl, ["step"])).toEqual(["made -> Outer"]);
  });

  it("follows no call that stands only in a comment", () => {
    const wgsl = `
      fn taken(values : array<f32, 3>) {}
      fn step() {
        // taken(array<f32, 3>());
        /* taken(array<f32, 3>()); */
      }
    `;
    expect(arraysByValueFrom(wgsl, ["step"])).toEqual([]);
  });
});
