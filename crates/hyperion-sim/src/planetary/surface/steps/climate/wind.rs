//! The precipitation heuristic's 10 m winds (R09.T13.e; Design note 8, decision-r09-t13ge §2.5,
//! §2.10): "the three-cell pattern scaled by the Hadley width, centred each month on the month's
//! energy-flux equator, and converging on the substellar point for slow and locked rotators", each
//! month's orographic step carrying vapour along its component along the model's bands
//! ([`along_bands`]; decision-r09-t13e-revision §4.3).
//!
//! # The meridional wind
//!
//! - **The Hadley cells' lower branch** carries the partition's mass flux ψ
//!   ([`partition`](super::partition)), so its mean meridional wind is v = −ψ 2πR² g ÷ (2πR cos φ
//!   Δp) = −ψ R g ÷ (cos φ Δp), ψ per 2πR² and southward positive, Δp the branch's depth, a share
//!   of the surface pressure ([`LOWER_BRANCH_DEPTH_SHARE`]); ψ is read at a cell's sine latitude
//!   linearly between the band edges. In tidally locked coordinates the same ψ is the radial inflow
//!   towards the substellar point.
//! - **The Ferrel and polar cells**, on a world that turns under its sun, by a labelled template on
//!   NCEP/NCAR R1's zonal-mean 10 m wind (the NCEP/NCAR Reanalysis 1, E. Kalnay et al. 1996, Bull.
//!   Amer. Meteor. Soc. 77, 437–471; its 1991–2020 long-term monthly means from NOAA PSL;
//!   [`extratropical_m_s`]), at low confidence: poleward in the Ferrel cell and
//!   equatorward in the polar one, between each hemisphere's Hadley edge and its pole. The edge is
//!   the partition's `φ_H` in the hemisphere away from the centre and asin(|`x_c`| + sin `φ_H`) in
//!   the centre's, so the pattern moves with the centre as the Hadley cell's weighting does, and
//!   the cells narrow and weaken to nothing as the edge reaches the pole. A locked world's
//!   circulation is its substellar convergence alone.
//!
//! # The zonal wind
//!
//! From a quadratic surface drag balancing the Coriolis force on v, across the meridional
//! direction: 0 = f v − (`C_D` ÷ h) |V| u, so u = f v h ÷ (`C_D` |V|) and |V|² = u² + v², whence
//! |V|² = (v² + √(v⁴ + 4 (f v h ÷ `C_D`)²)) ÷ 2, with f = 2Ω sin φ at the geographic latitude, on a
//! locked world too, and `C_D` ÷ h ([`DRAG_OVER_DEPTH_PER_M`]). The deflection is along the
//! meridional direction × up, east in geographic coordinates, so the zonal sense follows the
//! geographic hemisphere wherever the meridional structure is centred (plan 09's Risks, "The
//! synthetic winds are not T13.e's").
//!
//! # The record
//!
//! Each climate cell's month holds the speed |V| and the azimuth of the resultant, towards and
//! clockwise from local north (north towards the body-fixed +z pole along the surface, east
//! +z × up, and at a pole north the +x meridian's direction: [`Wind`]'s convention), quantised to
//! the record's codes and decoded ([`recorded`]): what the orographic step reads is exactly what
//! the record holds. The heuristic has no transients, so the resultant's direction and the mean
//! scalar speed are the one wind's. The record is the full vector, for R11; the orographic step
//! reads its component along the model's bands, since the zonal budget already carries the
//! meridional transport the record's meridional part would carry again over land.

use core::f64::consts::TAU;

use hyperion_surface::field::Wind;

use super::lfpm::{CellWind, DOWNWIND_TOLERANCE};
use crate::math;
use crate::planetary::surface::steps::WindSample;
use crate::units::{MetresPerSecond, Radians};

/// The Hadley cells' lower branch's depth over the surface pressure, Δp ÷ `p_s`: 0.07, calibrated
/// on the reference Earth's trades (R09.T13.e's second part), as the brief asked. The lane's
/// prototype took 0.1, Siler, Roe and Armour's (2018, Fig. 2d's caption, read) "layer below 900
/// hPa, where most atmospheric water vapor is concentrated", which is their ERA-Interim mass
/// transport's layer; there the reference Earth's annual zonal-mean v in the 5° bins at 12.5° and
/// 17.5° of both hemispheres (R09.T13.g's synthetic sea, each band's land then in one block, level
/// 7) averaged 1.05 m/s in magnitude, against the NCEP/NCAR Reanalysis 1's 1.48 over 1991–2020
/// (Kalnay et al. 1996, Bull. Amer. Meteor. Soc. 77, 437–471; the lane's scratch
/// `e/zonal-winds-ncep-r1.txt`), so 0.1 × 1.05 ÷ 1.48 = 0.071, held to two figures. It is an
/// effective depth for the 10 m wind, the column's equatorward mass flux over the surface wind
/// (about 70 hPa, or 600 m, inside the trade-wind boundary layer below its inversion at about
/// 1–2 km, from memory), not the vapour layer's depth, and is not re-fitted on later set-ups: the
/// four bins' mean |v| is 1.50 m/s on the calibrating run, 1.53 on Natural Earth's land by box
/// with its heights by box median (−1.57, −1.41, +1.61 and +1.53 m/s) and 1.57 on the coarse
/// pass's Earth, against NCEP's 1.48 (decision-r09-t13e-asbuilt §3). The
/// model's trades are symmetric about the equator where NCEP's southern ones are the stronger
/// (−1.05 and −1.27 m/s north, +1.83 and +1.77 south). Low-to-medium confidence.
pub(super) const LOWER_BRANCH_DEPTH_SHARE: f64 = 0.07;

/// The surface drag coefficient over the boundary layer's depth, `C_D` ÷ h, m⁻¹: 4 × 10⁻⁶, the
/// lane's fit to NCEP/NCAR R1's 1991–2020 zonal-mean 10 m winds (scratch
/// `e/zonal-winds-ncep-r1.txt`): with NCEP's own v it gives the trades −3.6 m/s against −3.1 at
/// 17.5° N and the westerlies +7.3 against +6.9 at 52.5° S (about `C_D` 1.2 × 10⁻³ over 300 m).
/// Low-to-medium confidence; one value for every world.
pub(super) const DRAG_OVER_DEPTH_PER_M: f64 = 4.0e-6;

/// The Ferrel cell's greatest poleward 10 m wind on the template, m/s: 1.21, the mean of the two
/// hemispheres' annual zonal means at 52.5° in NCEP/NCAR R1 over 1991–2020, all longitudes (+0.54
/// north, −1.88 south; Kalnay et al. 1996, the dataset's citation; the lane's scratch
/// `e/zonal-winds-ncep-r1.txt`). Low confidence: a zonal mean of land and sea, one amplitude for
/// both hemispheres where NCEP's differ 3.5-fold, the template's own on every world.
pub(super) const FERREL_POLEWARD_M_S: f64 = 1.21;

/// The polar cell's greatest equatorward 10 m wind on the template, m/s: 1.14, the hemispheres'
/// mean at 72.5° in the same means (−0.21 north, +2.06 south). Low confidence, as
/// [`FERREL_POLEWARD_M_S`]: it is mostly Antarctica's katabatic outflow, and a world with no polar
/// ice dome likely has the north's weak polar wind (plan 09's deferred finding E-P7).
pub(super) const POLAR_EQUATORWARD_M_S: f64 = 1.14;

/// The template's nodes at the Earth's Hadley edge, 25.1°, degrees of latitude: the Hadley and
/// Ferrel cells' meeting at the surface, 35.4°, the hemispheres' mean of NCEP R1's annual v's
/// zeros on its native Gaussian rows (about 1.9°), 35.95° N and 34.88° S: the read used, since a
/// zero is located at the data's own resolution. Its 5° bins give 36.95° N and 34.99° S (mean
/// 35.97°); linear interpolation between bin means moves a crossing across a curved profile
/// towards its flatter side, as the "36.9° N" of the as-built ruling's read did (−0.23 m/s at
/// 32.5° N, +0.03 at 37.5° N). Decision-r09-t13e-revision, "Review points (follow-up 2)", keeps
/// 35.4° as built and withdraws plan 09's E-P7 node item, a difference of method, not of NCEP.
/// Then the Ferrel cell's greatest wind at 52.5°, its meeting with the polar cell at 62.75°
/// (61.0° N and 64.5° S; 61.3° N and 64.1° S, mean 62.7°, by linear interpolation between the
/// bins) and the polar cell's greatest at 72.5° (the same means as [`FERREL_POLEWARD_M_S`]). Off
/// the Earth's rotation each lies at the same share of the way from the hemisphere's edge to its
/// pole. Low confidence; the amplitudes stay as the first read gave them (E-P7, with E-P3).
const TEMPLATE_NODES_DEG: [f64; 4] = [35.4, 52.5, 62.75, 72.5];

/// The Hadley edge the template's nodes are read against, degrees: the partition's at Ω⊕.
const TEMPLATE_EDGE_DEG: f64 = super::partition::HADLEY_EDGE_AT_EARTH_DEG;

/// The Ferrel and polar cells' poleward 10 m wind at the latitude `latitude_deg` from the equator
/// (its magnitude, degrees) in a hemisphere whose Hadley edge lies at `edge_deg`, m/s, negative
/// where it blows equatorward: zero from the equator to the edge and on to the template's first
/// node, then linear between the nodes ([`TEMPLATE_NODES_DEG`]) through the Ferrel cell's
/// [`FERREL_POLEWARD_M_S`] and the polar cell's −[`POLAR_EQUATORWARD_M_S`] to zero at the pole,
/// each node at its Earth share s = (φ − 25.1°) ÷ 64.9° of the way from the edge to the pole, and
/// the whole scaled by (90° − edge) ÷ 64.9°, so that the cells vanish as the edge reaches the pole.
#[must_use]
pub(super) fn extratropical_m_s(latitude_deg: f64, edge_deg: f64) -> f64 {
    if edge_deg >= 90.0 || latitude_deg <= edge_deg {
        return 0.0;
    }
    let earth_span = 90.0 - TEMPLATE_EDGE_DEG;
    let span = 90.0 - edge_deg;
    let s = (latitude_deg - edge_deg) / span;
    let node = |k: usize| (TEMPLATE_NODES_DEG[k] - TEMPLATE_EDGE_DEG) / earth_span;
    let nodes = [
        (0.0, 0.0),
        (node(0), 0.0),
        (node(1), FERREL_POLEWARD_M_S),
        (node(2), 0.0),
        (node(3), -POLAR_EQUATORWARD_M_S),
        (1.0, 0.0),
    ];
    let shape = nodes
        .windows(2)
        .find(|pair| s <= pair[1].0)
        .map_or(0.0, |pair| {
            let ((s0, v0), (s1, v1)) = (pair[0], pair[1]);
            v0 + (v1 - v0) * (s - s0) / (s1 - s0)
        });
    shape * span / earth_span
}

/// The latitude at which the Hadley and Ferrel cells meet at the surface in a hemisphere whose
/// Hadley edge lies at `edge_deg`, degrees: the template's first node at its Earth share of the
/// way from the edge to the pole ([`TEMPLATE_NODES_DEG`]; 35.4° at Earth's 25.1°), the pole where
/// the edge reaches it. The ocean's Ekman cells return their water at it (decision-r09-t13i
/// §2.3.1; Klinger and Marotzke 2000 take the contrast to the zero-wind latitude, about 30°).
#[must_use]
pub(super) fn return_latitude_deg(edge_deg: f64) -> f64 {
    if edge_deg >= 90.0 {
        return 90.0;
    }
    let share = (TEMPLATE_NODES_DEG[0] - TEMPLATE_EDGE_DEG) / (90.0 - TEMPLATE_EDGE_DEG);
    edge_deg + share * (90.0 - edge_deg)
}

/// The Hadley cells' lower-branch meridional wind, m/s, towards the model's north pole: −ψ R g ÷
/// (cos φ Δp) for the mass flux `mass_flux_kg_m2_s` over 2πR² (southward positive) at the sine
/// latitude `sine_latitude` in the model's frame, on a body of radius `radius_m` and gravity
/// `gravity_m_s2` under a branch of depth `depth_pa`; zero at a pole.
#[must_use]
pub(super) fn lower_branch_m_s(
    mass_flux_kg_m2_s: f64,
    sine_latitude: f64,
    radius_m: f64,
    gravity_m_s2: f64,
    depth_pa: f64,
) -> f64 {
    let cosine = (1.0 - sine_latitude * sine_latitude).sqrt();
    if cosine > 0.0 && depth_pa > 0.0 {
        -mass_flux_kg_m2_s * radius_m * gravity_m_s2 / (cosine * depth_pa)
    } else {
        0.0
    }
}

/// The drag balance's deflected wind and speed for the meridional wind `v_m_s` under the Coriolis
/// parameter `coriolis_per_s` (the [module documentation](self)): u = f v ÷ ((`C_D` ÷ h) |V|),
/// m/s, along the meridional direction × up, and |V|, m/s; both zero in still air.
#[must_use]
pub(super) fn drag_balance(v_m_s: f64, coriolis_per_s: f64) -> (f64, f64) {
    let forcing = coriolis_per_s * v_m_s / DRAG_OVER_DEPTH_PER_M;
    let v2 = v_m_s * v_m_s;
    let speed = f64::midpoint(v2, (v2 * v2 + 4.0 * forcing * forcing).sqrt()).sqrt();
    if speed > 0.0 {
        (forcing / speed, speed)
    } else {
        (0.0, 0.0)
    }
}

/// Local east and north at the unit vector `up`, in the body frame, as [`Wind`] takes them: east
/// = +z × up ÷ |+z × up| and north = up × east; at a pole, where north has no direction, north the
/// +x meridian's direction, +x, and east north × up.
#[must_use]
pub(super) fn east_north(up: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    let across = (up[0] * up[0] + up[1] * up[1]).sqrt();
    if across > 0.0 {
        let east = [-up[1] / across, up[0] / across, 0.0];
        (east, cross(up, east))
    } else {
        let north = [1.0, 0.0, 0.0];
        (cross(north, up), north)
    }
}

/// The wind of `velocity_m_s` (a vector along the surface at `up`, m/s, in the body frame) as its
/// record holds it: quantised to [`Wind`]'s codes and decoded, the sample and the code; its speed
/// held below the last code's, so that every wind the heuristic gives has a code.
///
/// # Panics
///
/// If a component of the velocity is not finite.
#[must_use]
pub(super) fn recorded(velocity_m_s: [f64; 3], up: [f64; 3]) -> (WindSample, Wind) {
    assert!(
        velocity_m_s.iter().all(|v| v.is_finite()),
        "a wind's velocity must be finite, got {velocity_m_s:?}"
    );
    let speed = dot(velocity_m_s, velocity_m_s).sqrt();
    let (east, north) = east_north(up);
    let azimuth = math::atan2(dot(velocity_m_s, east), dot(velocity_m_s, north));
    let fastest = Wind {
        azimuth: 0,
        speed: u8::MAX,
    }
    .speed()
    .value();
    let held = if speed < fastest { speed } else { fastest };
    let code = Wind::from_velocity(MetresPerSecond::new(held), Radians::new(azimuth))
        .expect("a held, finite wind has a code");
    (
        WindSample {
            speed: code.speed(),
            azimuth: code.azimuth_angle(),
        },
        code,
    )
}

/// The wind `sample` (a record's, decoded) at the field cell whose centre is the unit vector `up`,
/// as the orographic step reads it: still air at speed 0, and otherwise the direction of its
/// azimuth from the cell's own local north ([`east_north`]). The step computes it as
/// [`cell_wind_in`] with each cell's frame and each record's heading computed once; this is the
/// same to the bit, for the tests.
#[cfg(test)]
#[must_use]
pub(super) fn cell_wind(sample: &WindSample, up: [f64; 3]) -> CellWind {
    cell_wind_in(&Heading::of(sample), east_north(up))
}

/// A decoded wind's heading, as the orographic step reads it at each of its field cells: whether
/// the air moves, and the sine and cosine of its azimuth.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Heading {
    /// Whether the record's speed is above calm.
    moves: bool,
    /// The sine and cosine of the azimuth.
    sin_cos: (f64, f64),
}

impl Heading {
    /// The heading of `sample`.
    #[must_use]
    pub(super) fn of(sample: &WindSample) -> Self {
        Self {
            moves: sample.speed.value() > 0.0,
            sin_cos: math::sin_cos(sample.azimuth.value()),
        }
    }
}

/// [`cell_wind`] of the heading `heading` at a field cell whose local east and north are `frame`
/// ([`east_north`]), to the bit: the frame and the heading computed once, as the step reads every
/// field cell's every month.
#[must_use]
pub(super) fn cell_wind_in(heading: &Heading, frame: ([f64; 3], [f64; 3])) -> CellWind {
    if !heading.moves {
        return CellWind::Calm;
    }
    let ((east, north), (sin, cos)) = (frame, heading.sin_cos);
    CellWind::Towards([
        cos * north[0] + sin * east[0],
        cos * north[1] + sin * east[1],
        cos * north[2] + sin * east[2],
    ])
}

/// The local east about the model frame's pole `pole` (a unit vector in the body frame) at the
/// unit vector `up`: towards the pole × up, the direction along the model's bands, geographic east
/// in geographic coordinates and the direction about the substellar axis in tidally locked ones;
/// `None` at the pole or its antipode, where the bands have no direction.
#[must_use]
pub(super) fn model_east(pole: [f64; 3], up: [f64; 3]) -> Option<[f64; 3]> {
    towards_pole(pole, up).map(|north| cross(north, up))
}

/// The orographic step's wind from a field cell's record's wind `wind` ([`cell_wind_in`]), along
/// the model's bands (decision-r09-t13e-revision §4.3): towards `east` (the cell's
/// [`model_east`]) where the wind's unit direction ŵ has ŵ · ê above the orographic model's
/// downwind tolerance, towards −ê where it is below its negative, and calm otherwise, where the
/// record is calm, or at the model's poles (`east` `None`). The zonal budget's E − P is the
/// divergence of the meridional water flux, Hadley and eddy (Siler, Roe and Armour 2018, J.
/// Climate 31, 7481, the text after their eq. 9, and their Fig. 2f), which the record's meridional
/// part would carry again over land (E-L7's trap); what the orographic model uniquely supplies is
/// the exchange between sea and land along the bands.
#[must_use]
pub(super) fn along_bands(wind: CellWind, east: Option<[f64; 3]>) -> CellWind {
    let (CellWind::Towards(towards), Some(east)) = (wind, east) else {
        return CellWind::Calm;
    };
    let length = dot(towards, towards).sqrt();
    let along = if length > 0.0 {
        dot(towards, east) / length
    } else {
        0.0
    };
    if along > DOWNWIND_TOLERANCE {
        CellWind::Towards(east)
    } else if along < -DOWNWIND_TOLERANCE {
        CellWind::Towards([-east[0], -east[1], -east[2]])
    } else {
        CellWind::Calm
    }
}

/// The Coriolis parameter f = 2Ω sin φ at the body-frame unit vector `up` of a body of sidereal
/// period `rotation_period_s`, s⁻¹: its third component is the sine of its geographic latitude.
#[must_use]
pub(super) fn coriolis_per_s(rotation_period_s: f64, up: [f64; 3]) -> f64 {
    2.0 * (TAU / rotation_period_s) * up[2]
}

/// The unit vector along the surface at `up` towards the model frame's pole `pole` (a unit
/// vector in the body frame), or `None` at the pole or its antipode.
#[must_use]
pub(super) fn towards_pole(pole: [f64; 3], up: [f64; 3]) -> Option<[f64; 3]> {
    let along = dot(pole, up);
    let tangent = [
        pole[0] - along * up[0],
        pole[1] - along * up[1],
        pole[2] - along * up[2],
    ];
    let length = dot(tangent, tangent).sqrt();
    (length > 0.0).then(|| {
        [
            tangent[0] / length,
            tangent[1] / length,
            tangent[2] / length,
        ]
    })
}

/// a × b.
#[must_use]
pub(super) fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// a · b.
#[must_use]
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyperion_testkit::float::assert_same_bits;

    /// The research's check of `C_D` ÷ h with NCEP's own v: the trades −3.6 m/s at 17.5° N (v −1.27
    /// m/s) and the westerlies +7.3 m/s at 52.5° S (v −1.88 m/s), on the Earth's rotation.
    #[test]
    fn the_drag_balance_gives_the_research_s_trades_and_westerlies() {
        let day = 86_164.098_9;
        let at = |latitude_deg: f64| {
            let (s, c) = math::sin_cos(latitude_deg.to_radians());
            coriolis_per_s(day, [c, 0.0, s])
        };
        let (trades, speed) = drag_balance(-1.27, at(17.5));
        assert!((trades + 3.62).abs() < 0.02, "{trades}");
        assert!((speed * speed - trades * trades - 1.27 * 1.27).abs() < 1e-9);
        let (westerlies, _) = drag_balance(-1.88, at(-52.5));
        assert!((westerlies - 7.25).abs() < 0.02, "{westerlies}");
        assert_same_bits(drag_balance(0.0, at(30.0)).1, 0.0);
        let (u, speed) = drag_balance(0.7, 0.0);
        assert_same_bits(u, 0.0);
        assert_same_bits(speed, 0.7);
    }

    /// The template is zero inside the edge and at the pole, poleward through the Ferrel cell and
    /// equatorward through the polar one, NCEP's anchors at the Earth's edge, and nothing at an
    /// edge on the pole.
    #[test]
    fn the_extratropical_template_takes_ncep_s_anchors_at_the_earth_s_edge() {
        assert_same_bits(extratropical_m_s(20.0, 25.1), 0.0);
        assert_same_bits(extratropical_m_s(30.0, 25.1), 0.0);
        assert!((extratropical_m_s(52.5, 25.1) - 1.21).abs() < 1e-12);
        assert!(extratropical_m_s(62.75, 25.1).abs() < 1e-12);
        assert!((extratropical_m_s(72.5, 25.1) + 1.14).abs() < 1e-12);
        assert!(extratropical_m_s(90.0, 25.1).abs() < 1e-12);
        assert!(extratropical_m_s(45.0, 25.1) > 0.0);
        assert!(extratropical_m_s(80.0, 25.1) < 0.0);
        // A wider cell compresses the pattern and weakens it.
        let wide = extratropical_m_s(36.0 + 0.4222 * 54.0, 36.0);
        assert!((wide - 1.21 * 54.0 / 64.9).abs() < 1e-3, "{wide}");
        assert_same_bits(extratropical_m_s(89.0, 90.0), 0.0);
    }

    /// A recorded wind decodes to its code and re-quantises to it; its direction at the cell is
    /// the azimuth from local north, east at a quarter turn; a calm one is still air; and at a
    /// pole north is +x.
    #[test]
    fn a_recorded_wind_is_its_code_and_blows_towards_its_azimuth() {
        let up = [0.6, 0.0, 0.8];
        let (east, north) = east_north(up);
        let velocity = [
            3.0 * east[0] + 4.0 * north[0],
            3.0 * east[1] + 4.0 * north[1],
            3.0 * east[2] + 4.0 * north[2],
        ];
        let (sample, code) = recorded(velocity, up);
        assert!((sample.speed.value() / 5.0 - 1.0).abs() < 0.03);
        assert_eq!(
            Wind::from_velocity(sample.speed, sample.azimuth).unwrap(),
            code
        );
        let CellWind::Towards(towards) = cell_wind(&sample, up) else {
            panic!("a 5 m/s wind is not calm");
        };
        let angle = math::atan2(dot(towards, east), dot(towards, north));
        assert!((angle - sample.azimuth.value()).abs() < 1e-12);
        let (calm, code) = recorded([0.0, 0.0, 0.0], up);
        assert_eq!(code, Wind::CALM);
        assert_eq!(cell_wind(&calm, up), CellWind::Calm);
        let (east, north) = east_north([0.0, 0.0, 1.0]);
        assert_eq!(north, [1.0, 0.0, 0.0]);
        assert_eq!(east, [0.0, -1.0, 0.0]);
    }

    /// `a` ÷ |`a`|.
    fn unit(a: [f64; 3]) -> [f64; 3] {
        let length = dot(a, a).sqrt();
        [a[0] / length, a[1] / length, a[2] / length]
    }

    /// Asserts that two vectors agree within 10⁻¹⁵ in each component.
    #[track_caller]
    fn assert_near(actual: [f64; 3], expected: [f64; 3]) {
        for (a, e) in actual.iter().zip(&expected) {
            assert!((a - e).abs() <= 1e-15, "{actual:?} against {expected:?}");
        }
    }

    /// Asserts that `wind` blows towards `expected`, to the bit.
    #[track_caller]
    fn assert_towards(wind: CellWind, expected: [f64; 3]) {
        let CellWind::Towards(towards) = wind else {
            panic!("a calm wind, not one towards {expected:?}");
        };
        for (&a, &e) in towards.iter().zip(&expected) {
            assert_same_bits(a, e);
        }
    }

    /// Asserts that `wind` is still air.
    #[track_caller]
    fn assert_calm(wind: CellWind) {
        assert!(matches!(wind, CellWind::Calm), "{wind:?} is not calm");
    }

    /// Decision-r09-t13e-revision §4.3: the model's east, ê = towards the pole × up, is geographic
    /// east, +z × up normalised, about the spin pole +z, a unit vector along the surface, with no
    /// direction at either pole; and in tidally locked coordinates, about a substellar axis such
    /// as +x, it is the axis's tangent at the cell × up, the direction about that axis, axis × up
    /// normalised, with no direction at the substellar and antistellar points.
    #[test]
    fn the_model_s_east_is_geographic_east_about_the_spin_pole_and_turns_about_a_substellar_axis() {
        let ups = [
            [1.0, 0.0, 0.0],
            [0.6, 0.0, 0.8],
            [0.48, 0.6, 0.64],
            [-0.36, 0.48, -0.8],
            [0.0, 0.8, -0.6],
        ];
        let spin = [0.0, 0.0, 1.0];
        for up in ups {
            let east = model_east(spin, up).expect("off the poles the bands have a direction");
            assert_near(east, unit(cross(spin, up)));
            assert_near(east, east_north(up).0);
            assert!(
                dot(east, up).abs() <= 1e-15 && (dot(east, east) - 1.0).abs() <= 1e-15,
                "the east {east:?} at {up:?} is a unit vector along the surface"
            );
        }
        assert!(model_east(spin, [0.0, 0.0, 1.0]).is_none());
        assert!(model_east(spin, [0.0, 0.0, -1.0]).is_none());
        let axis = [1.0, 0.0, 0.0];
        for (up, by_hand) in [
            ([0.0, 0.0, 1.0], [0.0, -1.0, 0.0]),
            ([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
            ([0.6, 0.0, 0.8], [0.0, -1.0, 0.0]),
        ] {
            assert_near(model_east(axis, up).unwrap(), by_hand);
        }
        for up in &ups[1..] {
            let east = model_east(axis, *up).expect("off the axis the bands have a direction");
            let along = dot(axis, *up);
            let tangent = unit([
                axis[0] - along * up[0],
                axis[1] - along * up[1],
                axis[2] - along * up[2],
            ]);
            assert_near(east, cross(tangent, *up));
            assert_near(east, unit(cross(axis, *up)));
        }
        assert!(model_east(axis, [1.0, 0.0, 0.0]).is_none());
        assert!(model_east(axis, [-1.0, 0.0, 0.0]).is_none());
    }

    /// Decision-r09-t13e-revision §4.3: the orographic step's wind along the model's bands is
    /// towards its east ê where the record's wind has an eastward share ŵ · ê above the downwind
    /// tolerance (north-east, south-east, east, and ten times the tolerance), towards −ê where it
    /// has a westward one, and calm for a due-north or due-south wind, whose share lies within the
    /// tolerance, for one a tenth of the tolerance east, for a calm record, and with no east, at
    /// the model's poles; the same for a wind of any length; and in tidally locked coordinates
    /// along the east about the substellar axis.
    #[test]
    fn a_wind_along_the_bands_is_east_west_or_calm_by_the_sign_of_its_share_along_them() {
        let up = [0.48, 0.6, 0.64];
        let (east, north) = east_north(up);
        let model = model_east([0.0, 0.0, 1.0], up);
        let ahead = model.expect("off the poles the bands have a direction");
        let west = [-ahead[0], -ahead[1], -ahead[2]];
        let tiny = DOWNWIND_TOLERANCE;
        for length in [1e-6, 1.0, 7.5, 1e6] {
            let wind = |u: f64, v: f64| {
                CellWind::Towards([
                    length * (u * east[0] + v * north[0]),
                    length * (u * east[1] + v * north[1]),
                    length * (u * east[2] + v * north[2]),
                ])
            };
            for (u, v) in [(1.0, 1.0), (1.0, -1.0), (1.0, 0.0), (10.0 * tiny, 1.0)] {
                assert_towards(along_bands(wind(u, v), model), ahead);
                assert_towards(along_bands(wind(-u, v), model), west);
            }
            for (u, v) in [
                (0.0, 1.0),
                (0.0, -1.0),
                (0.1 * tiny, 1.0),
                (-0.1 * tiny, -1.0),
            ] {
                assert_calm(along_bands(wind(u, v), model));
            }
            assert_calm(along_bands(wind(1.0, 1.0), None));
        }
        assert_calm(along_bands(CellWind::Calm, model));
        assert_calm(along_bands(CellWind::Calm, None));
        let pole = [0.0, 0.0, 1.0];
        assert_calm(along_bands(
            CellWind::Towards([1.0, 0.0, 0.0]),
            model_east(pole, pole),
        ));
        // Locked about +x: at the north pole of the body frame, the bands run along −y.
        let locked = model_east([1.0, 0.0, 0.0], pole);
        let about = locked.expect("off the axis the bands have a direction");
        assert_towards(
            along_bands(CellWind::Towards([0.3, -1.0, 0.0]), locked),
            about,
        );
        assert_towards(
            along_bands(CellWind::Towards([0.3, 1.0, 0.0]), locked),
            [-about[0], -about[1], -about[2]],
        );
        assert_calm(along_bands(CellWind::Towards([1.0, 0.0, 0.0]), locked));
    }
}
