// The shading of a lit body's surface (plan R07, Design notes 5, 6 and 24): the body BRDF, a
// lunar-Lambert disc term times a phase factor f read from a table, as reflectance I/F.
//
// A library composed by concatenation: it declares no binding of its own. Its includer declares
// `phase_factor_table : texture_2d<f32>` at the includer's own group and binding (an
// `rgba32float` texture read with textureLoad, so unfilterable-float): one row per tabulated law,
// PHASE_TABLE_SAMPLES texels from 0 to π every 0.5°, f in r, g and b (alpha unused). The
// TypeScript twin is `view/appearance/brdf.ts`, which reads the same table the same way.

const LIT_PI : f32 = 3.14159265358979;

// The phase-factor table's samples and step, rad: 0° to 180° every 0.5° (`law.ts`).
const PHASE_TABLE_SAMPLES : u32 = 361u;
const PHASE_TABLE_STEP_RAD : f32 = 0.00872664625997165;

// A lunar-Lambert law, which a caller may build per texel (R10): the albedo scale A per channel,
// the Lommel–Seeliger share L, the exponents s per channel that its table row was made with, and
// `table_row`, the row of `phase_factor_table` holding its f (one row per tabulated law: template,
// L and s). The shader reads f from the row; s rides along for callers that rebuild rows.
struct LunarLambert {
  a : vec3f,
  l : f32,
  s : vec3f,
  table_row : u32,
}

// f per channel at phase α, by linear interpolation between the row's two nearest samples.
fn phase_factor(row : u32, alpha : f32) -> vec3f {
  let x = clamp(alpha, 0.0, LIT_PI) / PHASE_TABLE_STEP_RAD;
  let i = min(u32(floor(x)), PHASE_TABLE_SAMPLES - 2u);
  let t = x - f32(i);
  let below = textureLoad(phase_factor_table, vec2u(i, row), 0).rgb;
  let above = textureLoad(phase_factor_table, vec2u(i + 1u, row), 0).rgb;
  return below + t * (above - below);
}

// I/F = A · f(α) · [L · 2μ₀ ÷ (μ₀ + μ) + (1 − L) μ₀]; dark where μ₀ ≤ 0 or μ ≤ 0.
fn body_brdf(law : LunarLambert, mu0 : f32, mu : f32, alpha : f32) -> vec3f {
  if (mu0 <= 0.0 || mu <= 0.0) {
    return vec3f(0.0);
  }
  let disc = law.l * 2.0 * mu0 / (mu0 + mu) + (1.0 - law.l) * mu0;
  return law.a * phase_factor(law.table_row, alpha) * disc;
}
