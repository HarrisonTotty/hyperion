//! The velocities of plan 08's displaced classes (P08.T12.d; plan 08, Design notes 7, 22 and 23).
//!
//! A member of a displaced class moves as its class does, not as the field component it was born
//! in:
//!
//! - **The ballistic bins** (the thin disc's two youngest age bins, under 0.3 `R_d ÷ v_c` since
//!   the kick): the circular velocity at the site plus the kick. The kick's speed and azimuth are
//!   a remnant's natal kick, plan 06's draw through the stellar stage's kick loop, or a runaway's
//!   ejection speed in its speed bin and an azimuth, drawn on `runaway.speed`; its vertical
//!   component is z ÷ s, the height over the time since the kick, held to the kick's speed. So the
//!   site x − v s lands in the midplane. The circular velocity is the one at the epoch position's
//!   radius, from which the site lies at most some hundreds of light-years.
//! - **Every other bin:** the class's mean rotation and three dispersions times its speed scale
//!   (`v_c`, or the nuclear disc's own circular speed for its classes), Gaussian in local
//!   cylindrical axes, on `system.velocity` as the field's are drawn. In the first-excursion bins
//!   (0.3–8 `R_d ÷ v_c`) the vertical component takes the sign of z with the class's `outbound`
//!   share, so a fast pulsar high above the disc is also moving away from it.
//!
//! The escape cut of Design note 7 holds every class but the fastest of each age bin, which hold
//! the unbound (Design note 23): those are cut at their record's layer's
//! [`pad_speed`](crate::galaxy::query::pad_speed) instead. A Gaussian draw at or above its cut is
//! drawn again on the next words, and the last scaled to 0.99 of the cut; a ballistic velocity,
//! which no draw makes, is scaled.

use super::EllipsoidAxes;
use super::draw::{ESCAPE_CUT_ATTEMPTS, VelocityDraw, WORDS_PER_ATTEMPT, basis, norm, scaled_to};
use crate::galaxy::displaced::SPEED_EDGES;
use crate::galaxy::displaced::class_table::ClassKey;
use crate::galaxy::displaced::runaway::{Ejected, RUNAWAY_CUT_OF_V_C, WALKAWAY_CUT};
use crate::galaxy::displaced::{DisplacedClassId, DisplacedFields, DisplacedKind, SPEED_BINS};
use crate::galaxy::placement::{SystemRecord, ballistic_motion};
use crate::galaxy::query::{PAD_SPEED, pad_speed};
use crate::galaxy::{Galaxy, PointLy};
use crate::math;
use crate::rng::{ObjectKey, Stream, Threshold, tags};
use crate::stellar::system::{draw_metallicity, primary_model};
use crate::units::KilometresPerSecond;
use crate::units::consts::SPEED_OF_LIGHT;

/// The first word of `system.velocity` the outbound marks are read from: the outbound mark of
/// attempt `k` is word `OUTBOUND_WORD + k`, past every attempt's normals.
const OUTBOUND_WORD: u64 = WORDS_PER_ATTEMPT * 16;

const _: () = assert!(
    ESCAPE_CUT_ATTEMPTS == 16,
    "the outbound marks follow every attempt"
);

/// The first and last age bins of the first excursion: 0.3–8 `R_d ÷ v_c`.
const FIRST_EXCURSION_BINS: (usize, usize) = (2, 5);

/// Bisection steps in a runaway's speed quantile: 2⁻⁴⁰ of its bin.
const SPEED_BISECTIONS: u32 = 40;

/// Whether the class `key` is exempt from the escape cut: the fastest class of each age bin, and
/// of each old source, which holds the unbound still inside the cube (Design notes 7 and 23), and
/// the hypervelocity survivors.
#[must_use]
pub(crate) fn is_exempt(key: ClassKey) -> bool {
    match key {
        ClassKey::Thin { speed, .. } | ClassKey::Old { speed, .. } => {
            speed.index() == SPEED_BINS - 1
        }
        ClassKey::Hypervelocity => true,
    }
}

/// The velocity of `record`, a member of class `class` of kind `kind`, drawn on `stream` (its
/// `system.velocity` stream): this module's law, with its cut.
///
/// # Panics
///
/// If `galaxy` has no kinematic tables, or the class has no row of kinematics (the hypervelocity
/// survivors, which have no weight).
#[must_use]
pub(super) fn draw_displaced(
    galaxy: &Galaxy,
    displaced: &DisplacedFields,
    record: &SystemRecord,
    (class, kind): (DisplacedClassId, DisplacedKind),
    stream: &mut Stream,
) -> VelocityDraw {
    assert!(
        galaxy.kinematics().is_some(),
        "velocities need the kinematic tables, which Galaxy::with_full_potential builds"
    );
    let entry = displaced.class(class);
    let key = entry.key();
    let table = displaced.class_table();
    let source = key
        .source()
        .expect("only the hypervelocity survivors have no source, and they have no weight");
    let v_ref = table.speed_scale(source).value();
    let position = record.epoch_position();
    let p = PointLy::from(position);
    let axes = basis(position, &p, EllipsoidAxes::Cylindrical);
    let cut = if is_exempt(key) {
        pad_speed(record.layer()).value()
    } else {
        let r_cyl = math::hypot(p.x, p.y);
        let escape = galaxy
            .potential()
            .escape_speed(
                crate::units::LightYears::new(r_cyl),
                crate::units::LightYears::new(p.z),
            )
            .map_or(f64::INFINITY, KilometresPerSecond::value);
        if escape.is_nan() {
            PAD_SPEED.value()
        } else {
            escape.min(PAD_SPEED.value())
        }
    };
    if let Some((height, since)) = ballistic_motion(galaxy, record) {
        let (speed, azimuth) = kick(galaxy, displaced, record, (key, kind), v_ref);
        let vertical = if since.value() > 0.0 {
            (height.value() / since.value() * SPEED_OF_LIGHT / 1_000.0).clamp(-speed, speed)
        } else {
            0.0
        };
        let across = (speed * speed - vertical * vertical).max(0.0).sqrt();
        let (sin, cos) = math::sin_cos(azimuth);
        let v_c = galaxy
            .potential()
            .v_circ(crate::units::LightYears::new(math::hypot(p.x, p.y)))
            .value();
        let spinward = axes[1];
        let v = [
            v_c * spinward[0] + across * cos,
            v_c * spinward[1] + across * sin,
            v_c * spinward[2] + vertical,
        ];
        return scaled_to(v, cut, 1);
    }
    let kinematics = entry
        .kinematics()
        .expect("a class with weight has its row of kinematics");
    let first_excursion = matches!(key, ClassKey::Thin { age, .. }
        if (FIRST_EXCURSION_BINS.0..=FIRST_EXCURSION_BINS.1).contains(&age.index()));
    let mean = [0.0, kinematics.mean_phi * v_ref, 0.0];
    let sigma = [
        kinematics.sigma_r * v_ref,
        kinematics.sigma_phi * v_ref,
        kinematics.sigma_z * v_ref,
    ];
    let outbound =
        first_excursion.then(|| (kinematics.outbound, if p.z < 0.0 { -1.0 } else { 1.0 }));
    gaussian(stream, (mean, sigma), &axes, outbound, cut)
}

/// A Gaussian draw on `stream` of `mean` and `sigma` (km/s) along the local axes `axes`, cut at
/// `cut` km/s (Design note 7): attempt `k` from word `4k`, drawn again while it reaches the cut,
/// and the last scaled to 0.99 of it. Where `outbound` gives a share and the sign of z, the
/// vertical component takes that sign with that share, on word `OUTBOUND_WORD + k`.
fn gaussian(
    stream: &mut Stream,
    (mean, sigma): ([f64; 3], [f64; 3]),
    axes: &[[f64; 3]; 3],
    outbound: Option<(f64, f64)>,
    cut: f64,
) -> VelocityDraw {
    let mut last = [0.0; 3];
    for k in 0..ESCAPE_CUT_ATTEMPTS {
        stream.seek(WORDS_PER_ATTEMPT * u64::from(k));
        let (n0, n1) = stream.standard_normal_pair();
        let n2 = stream.standard_normal();
        let mut local = [
            mean[0] + sigma[0] * n0,
            mean[1] + sigma[1] * n1,
            mean[2] + sigma[2] * n2,
        ];
        if let Some((share, outward)) = outbound {
            stream.seek(OUTBOUND_WORD + u64::from(k));
            let away = stream.mark().is_below(Threshold::from_probability(share));
            local[2] = local[2].abs() * if away { outward } else { -outward };
        }
        let mut v = [0.0; 3];
        for (axis, &component) in axes.iter().zip(&local) {
            for (sum, &unit) in v.iter_mut().zip(axis) {
                *sum += component * unit;
            }
        }
        if norm(v) < cut {
            return scaled_to(v, cut, k + 1);
        }
        last = v;
    }
    scaled_to(last, cut, ESCAPE_CUT_ATTEMPTS)
}

/// A ballistic record's kick: its speed, km/s, and its azimuth, radians from the galactic +x axis.
/// A remnant's is its natal kick, the stellar stage's (through the kick loop, a track built: a
/// cost P08.T16 benches); a runaway's or a walkaway's its ejection speed in its class's speed bin
/// by Design note 24's law and an azimuth, on `runaway.speed` (words 0 and 1).
fn kick(
    galaxy: &Galaxy,
    displaced: &DisplacedFields,
    record: &SystemRecord,
    (key, kind): (ClassKey, DisplacedKind),
    v_ref: f64,
) -> (f64, f64) {
    let ejected = match kind {
        DisplacedKind::Remnant => {
            let (model, _) = primary_model(galaxy, record, draw_metallicity(galaxy, record));
            return model.natal_kick().map_or((0.0, 0.0), |kick| {
                let d = kick.direction().components();
                (
                    KilometresPerSecond::from(kick.speed()).value(),
                    math::atan2(d[1], d[0]),
                )
            });
        }
        DisplacedKind::Runaway => Ejected::Runaway,
        DisplacedKind::Walkaway => Ejected::Walkaway,
        DisplacedKind::HypervelocitySurvivor => {
            unreachable!("the hypervelocity survivors have no weight, and no ballistic class")
        }
    };
    let speed_bin = match key {
        ClassKey::Thin { speed, .. } | ClassKey::Old { speed, .. } => speed,
        ClassKey::Hypervelocity => {
            unreachable!("the hypervelocity survivors have no weight, and no ballistic class")
        }
    };
    let mut stream = Stream::open(
        galaxy.seed(),
        tags::RUNAWAY_SPEED,
        ObjectKey::from(record.id()),
    );
    let rank = stream.uniform();
    let azimuth = core::f64::consts::TAU * stream.uniform();
    let model = displaced.class_table().runaway_model();
    let scale = KilometresPerSecond::new(v_ref);
    let bin = speed_bin.index();
    let least = if bin == 0 {
        0.0
    } else {
        SPEED_EDGES[bin - 1] * v_ref
    };
    // The fastest bin has no upper edge: the law's own cut ends it, which its CDF reads as 1.
    let law_top = (RUNAWAY_CUT_OF_V_C * v_ref).max(WALKAWAY_CUT.value());
    let most = SPEED_EDGES
        .get(bin)
        .map_or(law_top.max(least), |edge| edge * v_ref);
    let cdf = |speed: f64| model.speed_cdf(ejected, KilometresPerSecond::new(speed), scale);
    let target = cdf(least) + rank * (cdf(most) - cdf(least));
    let (mut below, mut above) = (least, most);
    for _ in 0..SPEED_BISECTIONS {
        let mid = f64::midpoint(below, above);
        if cdf(mid) < target {
            below = mid;
        } else {
            above = mid;
        }
    }
    (f64::midpoint(below, above), azimuth)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Seed;

    /// A draw that reaches its cut on every attempt is drawn again each time and then scaled to
    /// 0.99 of the cut (Design note 7), as an exempt class's at its layer's pad speed is: here a
    /// law far wider than its cut, so that every attempt reaches it.
    #[test]
    fn a_draw_at_its_cut_on_every_attempt_is_scaled_to_0_99_of_it() {
        let axes = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let mut stream = Stream::open(Seed::new(5), tags::SELFTEST_STREAM, ObjectKey::galaxy());
        let d = gaussian(
            &mut stream,
            ([0.0, 9_000.0, 0.0], [10.0; 3]),
            &axes,
            None,
            3_000.0,
        );
        assert_eq!(d.attempts(), ESCAPE_CUT_ATTEMPTS);
        let speed = d.velocity().speed().value() / 1_000.0;
        assert!((speed - 0.99 * 3_000.0).abs() < 1e-9 * speed, "{speed}");
        let d = gaussian(
            &mut stream,
            ([0.0, 100.0, 0.0], [10.0; 3]),
            &axes,
            Some((1.0, -1.0)),
            3_000.0,
        );
        assert_eq!(d.attempts(), 1);
        assert!(
            d.velocity().metres_per_second()[2] <= 0.0,
            "outbound below the plane"
        );
    }
}
