//! The crate's numeric conversions, each in one place with the reason it cannot lose anything.

/// A `u32` index as a `usize`.
#[must_use]
pub(crate) fn ix(i: u32) -> usize {
    usize::try_from(i).expect("the tool builds for 64-bit hosts only, where u32 fits usize")
}

/// A count or index as a `u32`.
///
/// # Panics
///
/// If it exceeds `u32::MAX`: a broken invariant, since [`crate::mesh::SourceMesh::new`] refuses a
/// mesh past [`crate::mesh::MAX_ELEMENTS`] vertices or triangles, which keeps every count the
/// build makes (corners, clusters, positions) below 2³², and [`crate::page::write_mesh`] refuses a
/// file whose offsets would pass it.
#[must_use]
pub(crate) fn u32_of(i: usize) -> u32 {
    u32::try_from(i).expect("the mesh's counts were checked to fit u32")
}

/// An `f64` rounded to the nearest `f32`.
#[expect(
    clippy::cast_possible_truncation,
    reason = "rounding to the stored f32 is the intent; callers round up where it must bound"
)]
#[must_use]
pub(crate) fn f32_nearest(x: f64) -> f32 {
    assert!(
        !x.is_nan(),
        "no NaN reaches the pages: its sign is not portable"
    );
    x as f32
}

/// The least `f32` not below `x`, which is finite and non-negative.
#[must_use]
pub(crate) fn f32_up(x: f64) -> f32 {
    let y = f32_nearest(x);
    if f64::from(y) < x { y.next_up() } else { y }
}

/// The greatest `f32` not above `x`, which is finite.
#[must_use]
pub(crate) fn f32_down(x: f64) -> f32 {
    let y = f32_nearest(x);
    if f64::from(y) > x { y.next_down() } else { y }
}

/// An integral `f64` of magnitude below 2⁵³ as an `i64`.
#[expect(
    clippy::cast_possible_truncation,
    reason = "callers pass rounded or floored values far below 2⁵³ in magnitude"
)]
#[must_use]
pub(crate) fn i64_of_integral(x: f64) -> i64 {
    x as i64
}

/// An `i64` below 2⁵³ in magnitude as an `f64`, exactly.
#[expect(
    clippy::cast_precision_loss,
    reason = "grid coordinates and their sums stay far below 2⁵³, where f64 is exact"
)]
#[must_use]
pub(crate) fn f64_of(x: i64) -> f64 {
    x as f64
}

/// An `i128` as the nearest `f64`.
#[expect(
    clippy::cast_precision_loss,
    reason = "a magnitude compared against an f64 bound"
)]
#[must_use]
pub(crate) fn f64_of_i128(x: i128) -> f64 {
    x as f64
}

/// A count as an `f64`, exactly below 2⁵³.
#[expect(clippy::cast_precision_loss, reason = "counts stay far below 2⁵³")]
#[must_use]
pub(crate) fn f64_of_count(x: usize) -> f64 {
    x as f64
}

/// The lesser of two finite values, the same on every platform (unlike `f64::min` at signed
/// zeros, whose result its documentation leaves to the platform).
#[must_use]
pub(crate) fn min(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

/// The greater of two finite values, the same on every platform.
#[must_use]
pub(crate) fn max(a: f64, b: f64) -> f64 {
    if b > a { b } else { a }
}
