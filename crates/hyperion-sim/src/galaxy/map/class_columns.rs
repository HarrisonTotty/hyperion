//! The displaced classes' part of an edge-on band: each class's mean column over every pixel of the
//! band, bit for bit what the direct quadrature (the tests' `displaced_pixel`) gives a pixel alone,
//! with the work its pixels share done once (plan 08, P08.T12.a; P08.T15.b's remnants map).
//!
//! A class's pixel is a two-dimensional quadrature: [`gl4`](super::gl4) on panels of the pixel's
//! height, and at each of those heights the line of sight along `+y` by [`gl16`](super::gl16) on
//! the class's panels. The line's nodes are the same at every column, every height and every class
//! of the same panels, so a column of the band finds:
//!
//! - each node's cylindrical radius once, and its arm point once on the young disc's arm geometry,
//!   which every class's arm shares (as the census's
//!   [`FormPoint`](crate::galaxy::displaced::forms::FormPoint) does);
//! - per class, what its density reads of a node alone (the arm factor, a layer's radial envelope
//!   and scale height, the ballistic layer's radial exponent), which every height of every row of
//!   the band then reads;
//! - per class, the line's integral once at each distinct `|z|` among the band's height nodes:
//!   every form is even in `z`, bit for bit, so a row and its mirror in the plane share their lines
//!   where a band holds both ([`render_row_ranges`](super::render_row_ranges));
//! - at each height, the density's radial part once at each distinct `|y|`: a line with arms takes
//!   `y` and `−y` at one radius, where only the arm factor differs.
//!
//! Each pixel is then summed as the direct quadrature sums it: the same panels, the same nodes and
//! the same order, by replaying [`along_line`](super::along_line) over the stored values of its
//! nodes and [`mean_over_height`](super::mean_over_height) over the stored lines.
//!
//! Under every 26 hold the edge-on remnants map took about 50 CPU-ms a pixel, 72% of it the
//! ballistic classes' arm points and arm factors found again at every node of every height; it now
//! takes about 1.5, with a band of rows and their mirrors, and a 1,024-pixel map 50 s on 16 workers
//! (pod, release; lane `perf-remnants-map`, 2026-11-01).

use super::{NO_KINK, PANEL_SCALES, Panels, ROOT_HALF, Symmetry, along_line, mean_over_height};
use crate::galaxy::displaced::DisplacedForm;
use crate::galaxy::displaced::forms::{BallisticLayer, CoredPowerLaw, DiscBornForm, FlaredLayer};
use crate::galaxy::fields::arms::{ArmGeometry, ArmPoint, SharpArm};
use crate::math;

/// The nodes of one panel scheme's line of sight, in the order [`along_line`] takes them, with the
/// distinct `|y|` among them.
#[derive(Debug)]
struct LineNodes {
    panels: Panels,
    symmetry: Symmetry,
    /// Each node's `y`, ly, in [`along_line`]'s order.
    ys: Vec<f64>,
    /// The distinct `|y|` among the nodes, ly.
    distinct: Vec<f64>,
    /// For each node, the index of its `|y|` in `distinct`.
    slot: Vec<usize>,
}

impl LineNodes {
    /// The nodes of `panels` on the sides `symmetry` takes.
    #[must_use]
    fn new(panels: Panels, symmetry: Symmetry) -> Self {
        let mut ys = Vec::new();
        let _: f64 = along_line(panels, symmetry, NO_KINK, |y| {
            ys.push(y);
            0.0
        });
        let (distinct, slot) = distinct_abs(&ys);
        Self {
            panels,
            symmetry,
            ys,
            distinct,
            slot,
        }
    }

    /// The line's integral from the value `value` gives each node, by its index in [`along_line`]'s
    /// order: its own sums, bit for bit, replayed.
    #[must_use]
    fn integral_with(&self, mut value: impl FnMut(usize) -> f64) -> f64 {
        let mut node = 0;
        along_line(self.panels, self.symmetry, NO_KINK, |_| {
            let at = value(node);
            node += 1;
            at
        })
    }
}

/// The distinct absolute values of `values`, ascending, and for each value the index of its
/// absolute value among them.
///
/// Two values are one when [`f64::total_cmp`] finds them equal, which is when their bits are:
/// `|−0|` and `0` are one, since `abs` clears the sign.
#[must_use]
fn distinct_abs(values: &[f64]) -> (Vec<f64>, Vec<usize>) {
    let mut order: Vec<usize> = (0..values.len()).collect();
    order.sort_unstable_by(|&a, &b| values[a].abs().total_cmp(&values[b].abs()));
    let mut distinct: Vec<f64> = Vec::new();
    let mut slot = vec![0; values.len()];
    for at in order {
        let value = values[at].abs();
        if distinct
            .last()
            .is_none_or(|last| last.total_cmp(&value).is_ne())
        {
            distinct.push(value);
        }
        slot[at] = distinct.len() - 1;
    }
    (distinct, slot)
}

/// Which line of sight a class integrates along.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineChoice {
    /// Equal panels on both sides, for a class with arms, whose density is not even in `y`.
    Arm,
    /// Logarithmic panels on one side, doubled.
    Plain,
}

/// A class's density, as the band reads it: what it reads of a node of the line alone, then of a
/// height.
#[derive(Debug, Clone, Copy)]
enum Kernel<'a> {
    /// The ballistic layer: `norm × squeeze × n0 exp(−(radial + height))`, times its arm factor.
    Ballistic(&'a BallisticLayer),
    /// The fitted form: `w × layer × arm + w × spheroid`, either part possibly absent; the layer
    /// with its [`vanishing_ratio`].
    Fitted {
        layer: Option<(f64, &'a FlaredLayer, f64)>,
        spheroid: Option<(f64, &'a CoredPowerLaw)>,
    },
    /// An old source's spheroid.
    Spheroid(&'a CoredPowerLaw),
}

impl<'a> Kernel<'a> {
    /// The kernel of `form`.
    #[must_use]
    fn of(form: &'a DisplacedForm) -> Self {
        match form {
            DisplacedForm::Spheroid(spheroid) => Self::Spheroid(spheroid),
            DisplacedForm::Disc(form) => match &**form {
                DiscBornForm::Ballistic(layer) => Self::Ballistic(layer),
                DiscBornForm::Fitted {
                    layer, spheroid, ..
                } => Self::Fitted {
                    layer: layer.as_ref().map(|(w, l)| (*w, l, vanishing_ratio(l))),
                    spheroid: spheroid.as_ref().map(|(w, s)| (*w, s)),
                },
            },
        }
    }
}

/// How a class's density at a node of its line is made from its height parts at the node's radius,
/// `core` and `extra`, and the node's arm factor: [`DiscBornForm`]'s fitted form takes `w × layer ×
/// arm + w × spheroid`, the ballistic layer `envelope × arm`, a spheroid its density.
///
/// A fitted form without a spheroid adds `0` to its layer, and one without a layer adds its
/// spheroid to `0`; a form without an arm takes no factor where the fitted form multiplies by 1.
/// `x + 0`, `0 + x` and `x × 1` are `x` exactly for an `x` that is not `−0`, and no density is, so
/// those operations are left out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Combine {
    /// `core`.
    Core,
    /// `core × factor`.
    Armed,
    /// `core + extra`.
    CorePlus,
    /// `core × factor + extra`.
    ArmedPlus,
}

impl Combine {
    /// How `kernel` makes its density, with an arm factor if `has_arm`.
    #[must_use]
    fn of(kernel: Kernel<'_>, has_arm: bool) -> Self {
        let both = matches!(
            kernel,
            Kernel::Fitted {
                layer: Some(_),
                spheroid: Some(_)
            }
        );
        let armed = has_arm
            && !matches!(
                kernel,
                Kernel::Fitted { layer: None, .. } | Kernel::Spheroid(_)
            );
        match (armed, both) {
            (false, false) => Self::Core,
            (true, false) => Self::Armed,
            (false, true) => Self::CorePlus,
            (true, true) => Self::ArmedPlus,
        }
    }
}

/// The power past which `exp(−power)` is exactly 0: the pinned `libm`'s `exp` returns 0 below
/// −745.133, where `e^x` is under half the least subnormal, and 750 lies far enough past that bound
/// that the few units in the last place of rounding in the power cannot carry it back.
const VANISHING_POWER: f64 = 750.0;

/// The ratio `|z| ÷ h` past which `layer`'s vertical factor `exp(−(|z| ÷ h)^β) ÷ h` is exactly 0:
/// `750^(1 ÷ β)` ([`VANISHING_POWER`]), past which the power is above 750 to within its rounding.
#[must_use]
fn vanishing_ratio(layer: &FlaredLayer) -> f64 {
    math::powf_positive(VANISHING_POWER, 1.0 / layer.beta())
}

/// One counted class of the plan: its weight, its density and its line.
#[derive(Debug)]
struct Class<'a> {
    /// Its systems inside the cube that the selection counts, above 0.
    weight: f64,
    /// Its form, which the tests hold the band to.
    #[cfg(test)]
    form: &'a DisplacedForm,
    kernel: Kernel<'a>,
    combine: Combine,
    arm: Option<&'a SharpArm>,
    line: LineChoice,
    /// The widest panel of a pixel's height, ly: [`PANEL_SCALES`] of the class's vertical scale.
    widest: f64,
}

/// The displaced classes of an edge-on plan, as the band reads them: the classes the selection
/// counts with a weight above 0, in class order, and the two lines of sight's nodes.
#[derive(Debug)]
pub(super) struct ClassPlan<'a> {
    classes: Vec<Class<'a>>,
    arm_line: LineNodes,
    plain_line: LineNodes,
    /// The geometry of the first class's arm, which the line's arm points are found on once.
    geometry: Option<ArmGeometry>,
}

impl<'a> ClassPlan<'a> {
    /// The plan of the classes `counted`, each with its weight, in class order.
    ///
    /// A class of weight 0 is left out: it adds `0 × m`, exactly 0, to every pixel's sum, since its
    /// mean column `m` is finite and never below 0, and a sum of columns never holds `−0`.
    #[must_use]
    pub(super) fn new(counted: impl IntoIterator<Item = (f64, &'a DisplacedForm)>) -> Self {
        let classes: Vec<Class<'a>> = counted
            .into_iter()
            .inspect(|&(weight, _)| {
                debug_assert!(
                    weight.is_finite() && weight >= 0.0,
                    "a class's systems inside the cube are {weight}"
                );
            })
            .filter(|&(weight, _)| weight > 0.0)
            .map(|(weight, form)| {
                let arm = match form {
                    DisplacedForm::Disc(disc) => disc.arm(),
                    DisplacedForm::Spheroid(_) => None,
                };
                let kernel = Kernel::of(form);
                Class {
                    weight,
                    #[cfg(test)]
                    form,
                    kernel,
                    combine: Combine::of(kernel, arm.is_some()),
                    arm,
                    line: if form.has_arm() {
                        LineChoice::Arm
                    } else {
                        LineChoice::Plain
                    },
                    widest: PANEL_SCALES * form.vertical_scale(),
                }
            })
            .collect();
        let geometry = classes
            .iter()
            .find_map(|class| class.arm.map(|arm| *arm.geometry()));
        Self {
            classes,
            arm_line: LineNodes::new(Panels::uniform(ROOT_HALF), Symmetry::Any),
            plain_line: LineNodes::new(Panels::logarithmic(ROOT_HALF), Symmetry::EvenInY),
            geometry,
        }
    }

    /// How many classes the plan counts: each pixel's values hold one a class.
    #[must_use]
    pub(super) fn len(&self) -> usize {
        self.classes.len()
    }

    /// Each class's weight, in the plan's order.
    pub(super) fn weights(&self) -> impl Iterator<Item = f64> + '_ {
        self.classes.iter().map(|class| class.weight)
    }

    /// Each class's form, in the plan's order.
    #[cfg(test)]
    pub(super) fn forms(&self) -> impl Iterator<Item = &'a DisplacedForm> + '_ {
        self.classes.iter().map(|class| class.form)
    }

    /// The band of rows whose heights are `heights`, `[z_lo, z_hi]` each (ly, from the plane):
    /// every class's height nodes over them.
    #[must_use]
    pub(super) fn band(&self, heights: &[[f64; 2]]) -> ClassBand {
        let classes = self
            .classes
            .iter()
            .map(|class| {
                let mut zs = Vec::new();
                let mut row_ends = Vec::with_capacity(heights.len());
                for &[z_lo, z_hi] in heights {
                    let _: f64 = mean_over_height(class.widest, z_lo, z_hi, |z| {
                        zs.push(z);
                        0.0
                    });
                    row_ends.push(zs.len());
                }
                let (distinct, slot) = distinct_abs(&zs);
                BandHeights {
                    distinct,
                    slot,
                    row_ends,
                }
            })
            .collect();
        ClassBand {
            heights: heights.to_vec(),
            classes,
        }
    }

    /// Fills `out` with each class's mean column, in the plan's order, over every row of `band` at
    /// the column `x` (ly): `out[row × len + class]`, bit for bit the direct quadrature's.
    ///
    /// # Panics
    ///
    /// If `out` does not hold a value for each class of each row of the band.
    pub(super) fn column(&self, band: &ClassBand, x: f64, scratch: &mut Scratch, out: &mut [f64]) {
        let count = self.classes.len();
        assert_eq!(
            out.len(),
            band.heights.len() * count,
            "one value a class a row"
        );
        let Scratch {
            radii,
            points,
            factors,
            first,
            second,
            core,
            extra,
            lines,
        } = scratch;
        // The radii at each distinct |y|: `hypot` reads |y|, so a node and its mirror share one.
        for (line, radii) in [&self.arm_line, &self.plain_line]
            .into_iter()
            .zip(radii.iter_mut())
        {
            radii.clear();
            radii.extend(line.distinct.iter().map(|&y| math::hypot(x, y)));
        }
        points.clear();
        if let Some(geometry) = &self.geometry {
            points.extend(self.arm_line.ys.iter().map(|&y| geometry.point(x, y)));
        }
        for (at, class) in self.classes.iter().enumerate() {
            let kernel = class.kernel;
            let (line, radii) = match class.line {
                LineChoice::Arm => (&self.arm_line, &radii[0]),
                LineChoice::Plain => (&self.plain_line, &radii[1]),
            };
            // The arm factor at every node, on the shared points where the arm's geometry is the
            // first's, as `FormPoint` shares them.
            factors.clear();
            if let Some(arm) = class.arm {
                if self.geometry.as_ref() == Some(arm.geometry()) {
                    factors.extend(points.iter().map(|point| arm.factor(point)));
                } else {
                    factors.extend(
                        line.ys
                            .iter()
                            .map(|&y| arm.factor(&arm.geometry().point(x, y))),
                    );
                }
            }
            radial_parts(kernel, radii, first, second);
            let combine = class.combine;
            let heights = &band.classes[at];
            lines.clear();
            for &z in &heights.distinct {
                height_parts(kernel, radii, first, second, z, core, extra);
                lines.push(line_integral(line, combine, core, extra, factors));
            }
            let mut start = 0;
            for (row, (&[z_lo, z_hi], &end)) in
                band.heights.iter().zip(&heights.row_ends).enumerate()
            {
                let mut next = heights.slot[start..end].iter();
                out[row * count + at] = mean_over_height(class.widest, z_lo, z_hi, |_| {
                    lines[*next
                        .next()
                        .expect("one line for each height node of the row")]
                });
                start = end;
            }
        }
    }
}

/// What a class's density reads of each distinct radius `radii` alone: `first` and `second`.
///
/// The ballistic layer's radial exponent in `first`; a fitted layer's radial envelope in `first`
/// and its scale height in `second`; nothing for a spheroid, which reads the radius itself.
fn radial_parts(kernel: Kernel<'_>, radii: &[f64], first: &mut Vec<f64>, second: &mut Vec<f64>) {
    first.clear();
    second.clear();
    match kernel {
        Kernel::Ballistic(layer) => first.extend(radii.iter().map(|&r| layer.radial_exponent(r))),
        Kernel::Fitted {
            layer: Some((_, layer, _)),
            ..
        } => {
            first.extend(radii.iter().map(|&r| layer.radial(r)));
            second.extend(radii.iter().map(|&r| layer.scale_height(r)));
        }
        Kernel::Fitted { layer: None, .. } | Kernel::Spheroid(_) => {}
    }
}

/// A class's density at the height `z` (ly) over each distinct radius `radii`, from its radial
/// parts: `core`, the part the arm factor multiplies, and `extra`, the part added after it
/// ([`Combine`]).
///
/// The ballistic layer's normalised envelope in `core`; a fitted form's weighted layer in `core`
/// and its weighted spheroid in `extra`, or the spheroid in `core` where it has no layer; a
/// spheroid's density in `core`. Each is the arithmetic of the form's own density, in its order.
/// A layer's density past its [`vanishing_ratio`] is the 0 it computes to.
fn height_parts(
    kernel: Kernel<'_>,
    radii: &[f64],
    first: &[f64],
    second: &[f64],
    z: f64,
    core: &mut Vec<f64>,
    extra: &mut Vec<f64>,
) {
    core.clear();
    extra.clear();
    let spheroid_at =
        |weight: f64, spheroid: &CoredPowerLaw, r: f64| weight * spheroid.density_at_radius(r, z);
    match kernel {
        Kernel::Ballistic(layer) => {
            let height = layer.height_exponent(z);
            core.extend(
                first
                    .iter()
                    .map(|&radial| layer.envelope_from(radial, height)),
            );
        }
        Kernel::Fitted { layer, spheroid } => {
            let (into, after) = match layer {
                Some((weight, layer, vanishing)) => {
                    core.extend(first.iter().zip(second).map(|(&radial, &h)| {
                        if z.abs() / h > vanishing {
                            0.0
                        } else {
                            weight * layer.density_from(radial, h, z)
                        }
                    }));
                    (&mut *extra, true)
                }
                None => (&mut *core, false),
            };
            if let Some((weight, spheroid)) = spheroid {
                into.extend(radii.iter().map(|&r| spheroid_at(weight, spheroid, r)));
            } else if !after {
                into.resize(radii.len(), 0.0);
            }
        }
        Kernel::Spheroid(spheroid) => {
            core.extend(radii.iter().map(|&r| spheroid.density_at_radius(r, z)));
        }
    }
}

/// A class's line integral at one height, from its height parts at each distinct radius, `core`
/// and `extra`, and its arm factor at each node, `factors`, made into each node's density as
/// `combine` says: [`along_line`]'s sums, bit for bit.
///
/// A line whose densities are all 0, a thin layer's far above the plane, is exactly 0 without the
/// sums: each is `0 + w × 0`.
#[must_use]
fn line_integral(
    line: &LineNodes,
    combine: Combine,
    core: &[f64],
    extra: &[f64],
    factors: &[f64],
) -> f64 {
    let vanished = |values: &[f64]| values.iter().all(|&value| value <= 0.0);
    if vanished(core) && (matches!(combine, Combine::Core | Combine::Armed) || vanished(extra)) {
        return 0.0;
    }
    let slot = &line.slot;
    match combine {
        Combine::Core => line.integral_with(|node| core[slot[node]]),
        Combine::Armed => line.integral_with(|node| core[slot[node]] * factors[node]),
        Combine::CorePlus => line.integral_with(|node| core[slot[node]] + extra[slot[node]]),
        Combine::ArmedPlus => {
            line.integral_with(|node| core[slot[node]] * factors[node] + extra[slot[node]])
        }
    }
}

/// One class's height nodes over a band's rows.
#[derive(Debug)]
struct BandHeights {
    /// The distinct `|z|` among the nodes, ly.
    distinct: Vec<f64>,
    /// For each node, in the order the rows take them, the index of its `|z|` in `distinct`.
    slot: Vec<usize>,
    /// Where each row's nodes end in `slot`.
    row_ends: Vec<usize>,
}

/// A band of an edge-on raster, as the classes read it: its rows' heights and each class's height
/// nodes over them.
#[derive(Debug)]
pub(super) struct ClassBand {
    /// Each row's heights, `[z_lo, z_hi]`, ly from the plane.
    heights: Vec<[f64; 2]>,
    /// Each class's height nodes over the rows, in the plan's order.
    classes: Vec<BandHeights>,
}

/// The buffers one column fills, kept from column to column so that a band allocates them once.
#[derive(Debug, Default)]
pub(super) struct Scratch {
    /// The radius at each distinct `|y|` of the arm line and of the plain line, ly.
    radii: [Vec<f64>; 2],
    /// The arm point at each node of the arm line, on the plan's geometry.
    points: Vec<ArmPoint>,
    /// One class's arm factor at each node of its line.
    factors: Vec<f64>,
    /// One class's radial parts at each distinct radius.
    first: Vec<f64>,
    second: Vec<f64>,
    /// One class's height parts at each distinct radius, at one height.
    core: Vec<f64>,
    extra: Vec<f64>,
    /// One class's line integral at each distinct height of the band.
    lines: Vec<f64>,
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;

    /// Past its vanishing ratio every flared layer's vertical factor is exactly 0, as the band
    /// takes it: at the ratio's next `f64` up and at ratios beyond it, for the scale heights of
    /// every class of the Milky Way fixture under every 26 hold. It rests on the pinned `libm`'s
    /// `exp`, which returns 0 below −745.133, so a change of `libm` that moved that bound fails
    /// here first.
    #[test]
    fn past_its_vanishing_ratio_a_layer_s_vertical_factor_is_exactly_0() {
        let fields = crate::galaxy::displaced::hold::displaced().fields();
        let displaced = fields
            .displaced()
            .expect("every 26 hold installs the classes");
        let mut layers = 0;
        for form in displaced.classes().iter().filter_map(|class| class.form()) {
            let DisplacedForm::Disc(disc) = form else {
                continue;
            };
            let DiscBornForm::Fitted {
                layer: Some((_, layer)),
                ..
            } = &**disc
            else {
                continue;
            };
            layers += 1;
            let ratio = vanishing_ratio(layer);
            assert!(ratio.is_finite() && ratio > 1.0, "{ratio}");
            for r in [0.0, 8_000.0, 26_000.0, 90_000.0] {
                let h = layer.scale_height(r);
                for beyond in [ratio.next_up(), 1.000_001 * ratio, 2.0 * ratio, 1e6 * ratio] {
                    let z = beyond * h;
                    // The band's test, `|z| ÷ h` past the ratio, holds wherever the factor is 0.
                    if z.abs() / h > ratio {
                        assert_same_bits(layer.vertical(h, z), 0.0);
                        assert_same_bits(layer.vertical(h, -z), 0.0);
                    }
                }
            }
        }
        assert!(layers > 30, "{layers} layers");
    }
}
