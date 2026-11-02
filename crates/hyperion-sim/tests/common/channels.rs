//! Helpers the channels' world tests share (rendering plan R09, T6.f to T6.i): a point of a coarse
//! cell by its face coordinates, and a sorted list's quantile.

use hyperion_surface::cube::{FaceUv, PatchKey, face_uv_to_xyz, st_to_uv, unit_dir};

/// The unit direction at face coordinates (`x`, `y`) of `cell`, each in [0, 1].
#[must_use]
pub fn point_in(cell: PatchKey, x: f64, y: f64) -> [f64; 3] {
    let side = f64::from(1_u32 << cell.level());
    let s = (f64::from(cell.i()) + x) / side;
    let t = (f64::from(cell.j()) + y) / side;
    unit_dir(face_uv_to_xyz(FaceUv {
        face: cell.face(),
        u: st_to_uv(s),
        v: st_to_uv(t),
    }))
}

/// The `q` quantile of `sorted`, the nearest rank's.
///
/// # Panics
///
/// If `sorted` is empty.
#[must_use]
pub fn quantile(sorted: &[f64], q: f64) -> f64 {
    assert!(!sorted.is_empty(), "a quantile of nothing");
    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a rank of a list of cells or joins, exact in f64 and back"
    )]
    let rank = ((sorted.len() - 1) as f64 * q).round() as usize;
    sorted[rank]
}
