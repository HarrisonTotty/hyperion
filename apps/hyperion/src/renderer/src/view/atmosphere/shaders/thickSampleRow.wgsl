// The thick read's altitude row once a sample of the ray march (plan R08, R08.T14.c's read;
// perf-thick-march, its step 4): `thickReadBands.wgsl`'s thickMultipleAt finds the row h* lies in by
// five dependent loads of the table's info row, then two more (`thickRead.wgsl`'s thickRowAt), for
// each source of each step. Every source of a step reads it at the step's own h*, so the step's first
// source that reads the table finds it and the rest take it, the same row bit for bit. Appended after
// THICK_READ_WGSL by `hillaire.ts`'s withThickRowOnce, which clears it at each step
// (`rayMarch.wgsl`'s rayMarchStep) and points thickMultipleAt here; the thick read's march per step
// (`RAY_MARCH_THICK_KERNEL`), the sky view and the aerial perspective keep the search a source.

var<private> thickSampleRow : ThickRow;
var<private> thickSampleRowFound : bool;

// The row h* `scaledHeightM` lies in, held at 0 as thickMultipleAt holds it (thickRowAt): found at
// the step's first call and held for its other sources. A loop of at most one pass, not an `if`, as
// the composite's reads are (`composite.wgsl`): SwiftShader runs both sides of a branch under its
// lanes' mask.
fn thickSampleRowAt(scaledHeightM : f32) -> ThickRow {
  for (var find = !thickSampleRowFound; find; find = false) {
    thickSampleRow = thickRowAt(max(scaledHeightM, 0.0));
    thickSampleRowFound = true;
  }
  return thickSampleRow;
}
