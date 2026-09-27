//! The per-class histograms the orbits are recorded into (plan 15, P15.T6.b), which T6.c–f fit.
//!
//! Each class keeps, at the epoch:
//!
//! - counts of its orbits bound and inside the root cube, unbound and inside, and outside it;
//! - the bound and the unbound inside the cube on 48 × 40 logarithmic cells in R and |z|
//!   ([`r_cell`], [`z_cell`]): R from 16 ly to 2¹⁸ ly, |z| from 1 ly to 2¹⁶ ly, each with a first
//!   cell reaching down to 0;
//! - the moments of the bound members' cylindrical velocities in units of the class's speed scale
//!   (`Σ v_R`, `Σ v_φ`, `Σ v_R²`, `Σ v_φ²`, `Σ v_z²`), with `v_φ` spinward, and how many move away
//!   from the plane (`z v_z > 0`), for T6.e's kinematics;
//! - for the barred sources, the bound members on a 24 × 24 × 20 logarithmic histogram of `|x|`,
//!   `|y|` and `|z|` in the bar's frame (the galaxy's at the epoch), for T6.d's own-form fit;
//! - the leapfrog steps its orbits took, for the cost of a larger run, how many were integrated
//!   again at a smaller step, how many were kept with their Jacobi integral still over the run's
//!   tolerance, and the worst drift kept, for T6.c–e to judge the histograms by.
//!
//! Counts are integers, so merging two histograms is exact in any order; the moments are summed
//! in the run's chunk order, which the thread count cannot change.

use std::fmt::Write as _;

use hyperion_sim::math;

/// Cells in R.
pub const R_CELLS: usize = 48;
/// Cells in |z|.
pub const Z_CELLS: usize = 40;
/// Cells in |x|, |y| and |z| of the bar-frame histogram.
pub const BAR_CELLS: [usize; 3] = [24, 24, 20];

/// The cell of `v` on a logarithmic axis whose first log edge is `first` (ly), which spans
/// `octaves` doublings over `cells − 1` cells after a first cell from 0 to `first`.
fn log_cell(v: f64, first: f64, octaves: f64, cells: usize) -> usize {
    if v < first {
        return 0;
    }
    let last = cells - 1;
    #[expect(clippy::cast_precision_loss, reason = "a cell count below 64")]
    let per_octave = last as f64 / octaves;
    let position = math::log2(v / first) * per_octave;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a non-negative position clamped below the cell count"
    )]
    let cell = 1 + position.floor().min(f64::from(u8::MAX)) as usize;
    cell.min(last)
}

/// The R cell of `r` (ly): 0 below 16 ly, then 47 cells over 14 octaves to 2¹⁸ ly.
#[must_use]
pub fn r_cell(r: f64) -> usize {
    log_cell(r, 16.0, 14.0, R_CELLS)
}

/// The |z| cell of `z` (ly): 0 below 1 ly, then 39 cells over 16 octaves to 2¹⁶ ly.
#[must_use]
pub fn z_cell(z: f64) -> usize {
    log_cell(z.abs(), 1.0, 16.0, Z_CELLS)
}

/// The bar-frame cell of `x` (ly): |x| and |y| from 16 ly over 12 octaves, |z| from 1 ly over 16.
#[must_use]
pub fn bar_cell(x: [f64; 3]) -> usize {
    let i = log_cell(x[0].abs(), 16.0, 12.0, BAR_CELLS[0]);
    let j = log_cell(x[1].abs(), 16.0, 12.0, BAR_CELLS[1]);
    let k = log_cell(x[2].abs(), 1.0, 16.0, BAR_CELLS[2]);
    (i * BAR_CELLS[1] + j) * BAR_CELLS[2] + k
}

/// How an orbit's integration went: its steps over every pass, whether it was refined, whether it
/// was kept over the tolerance, and the drift of its Jacobi integral kept.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Integration {
    /// Leapfrog steps over every pass.
    pub steps: u64,
    /// Whether the step was halved at least once.
    pub refined: bool,
    /// Whether the drift kept is still over the run's tolerance.
    pub over_tolerance: bool,
    /// The relative drift of the Jacobi integral kept.
    pub drift: f64,
}

/// One class's record at the epoch (module documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct ClassHistogram {
    /// Orbits integrated.
    pub orbits: u64,
    /// Leapfrog steps taken.
    pub steps: u64,
    /// Orbits integrated again at a smaller step.
    pub refined: u64,
    /// Orbits kept with their Jacobi integral still over the run's tolerance after the last
    /// halving.
    pub over_tolerance: u64,
    /// The largest relative drift of the Jacobi integral kept.
    pub worst_drift: f64,
    /// Bound and inside the cube.
    pub in_cube_bound: u64,
    /// Unbound and inside the cube.
    pub in_cube_unbound: u64,
    /// Outside the cube, bound or not.
    pub outside: u64,
    /// The bound inside the cube, `[r_cell × Z_CELLS + z_cell]`.
    pub bound: Vec<u64>,
    /// The unbound inside the cube, likewise.
    pub unbound: Vec<u64>,
    /// `Σ v_R`, `Σ v_φ`, `Σ v_R²`, `Σ v_φ²`, `Σ v_z²` over the bound inside the cube, in the
    /// class's speed scale.
    pub moments: [f64; 5],
    /// The bound inside the cube moving away from the plane.
    pub outbound: u64,
    /// The bar-frame histogram of the bound inside the cube, for the barred sources.
    pub bar_frame: Option<Vec<u64>>,
}

impl ClassHistogram {
    /// An empty record, with the bar-frame histogram if `barred`.
    #[must_use]
    pub fn new(barred: bool) -> Self {
        Self {
            orbits: 0,
            steps: 0,
            refined: 0,
            over_tolerance: 0,
            worst_drift: 0.0,
            in_cube_bound: 0,
            in_cube_unbound: 0,
            outside: 0,
            bound: vec![0; R_CELLS * Z_CELLS],
            unbound: vec![0; R_CELLS * Z_CELLS],
            moments: [0.0; 5],
            outbound: 0,
            bar_frame: barred.then(|| vec![0; BAR_CELLS.iter().product()]),
        }
    }

    /// Records an orbit at the epoch at `x` (ly) with cylindrical velocity `[v_R, v_φ, v_z]` in
    /// the class's speed scale, bound or not, after `integration`.
    pub fn record(&mut self, x: [f64; 3], v: [f64; 3], bound: bool, integration: Integration) {
        self.orbits += 1;
        self.steps += integration.steps;
        self.refined += u64::from(integration.refined);
        self.over_tolerance += u64::from(integration.over_tolerance);
        self.worst_drift = self.worst_drift.max(integration.drift);
        let half = f64::from(hyperion_sim::coords::ROOT_HALF_WIDTH_LY);
        if x.iter().any(|c| c.abs() > half) {
            self.outside += 1;
            return;
        }
        let cell = r_cell(math::hypot(x[0], x[1])) * Z_CELLS + z_cell(x[2]);
        if !bound {
            self.in_cube_unbound += 1;
            self.unbound[cell] += 1;
            return;
        }
        self.in_cube_bound += 1;
        self.bound[cell] += 1;
        let [vr, vphi, vz] = v;
        for (m, add) in self
            .moments
            .iter_mut()
            .zip([vr, vphi, vr * vr, vphi * vphi, vz * vz])
        {
            *m += add;
        }
        if x[2] * vz > 0.0 {
            self.outbound += 1;
        }
        if let Some(bar) = &mut self.bar_frame {
            bar[bar_cell(x)] += 1;
        }
    }

    /// Adds `other`'s record to this one.
    pub fn merge(&mut self, other: &Self) {
        self.orbits += other.orbits;
        self.steps += other.steps;
        self.refined += other.refined;
        self.over_tolerance += other.over_tolerance;
        self.worst_drift = self.worst_drift.max(other.worst_drift);
        self.in_cube_bound += other.in_cube_bound;
        self.in_cube_unbound += other.in_cube_unbound;
        self.outside += other.outside;
        for (a, b) in self.bound.iter_mut().zip(&other.bound) {
            *a += b;
        }
        for (a, b) in self.unbound.iter_mut().zip(&other.unbound) {
            *a += b;
        }
        for (a, b) in self.moments.iter_mut().zip(other.moments) {
            *a += b;
        }
        self.outbound += other.outbound;
        if let (Some(a), Some(b)) = (&mut self.bar_frame, &other.bar_frame) {
            for (x, y) in a.iter_mut().zip(b) {
                *x += y;
            }
        }
    }

    /// Appends the record to `out` as text, under the class name `name`: a header line of the
    /// counts and moments (floats in Rust's shortest round-trip form), then one line per histogram.
    pub fn write(&self, name: &str, out: &mut String) {
        let join = |v: &[u64]| {
            let mut s = String::with_capacity(v.len() * 2);
            for (i, n) in v.iter().enumerate() {
                if i > 0 {
                    s.push(' ');
                }
                write!(s, "{n}").expect("writing to a String cannot fail");
            }
            s
        };
        writeln!(
            out,
            "class {name} orbits {} steps {} refined {} over_tolerance {} worst_drift {:?} \
             in_cube_bound {} in_cube_unbound {} outside {} outbound {} moments {:?} {:?} {:?} \
             {:?} {:?}",
            self.orbits,
            self.steps,
            self.refined,
            self.over_tolerance,
            self.worst_drift,
            self.in_cube_bound,
            self.in_cube_unbound,
            self.outside,
            self.outbound,
            self.moments[0],
            self.moments[1],
            self.moments[2],
            self.moments[3],
            self.moments[4],
        )
        .expect("writing to a String cannot fail");
        writeln!(out, "bound {}", join(&self.bound)).expect("writing to a String cannot fail");
        writeln!(out, "unbound {}", join(&self.unbound)).expect("writing to a String cannot fail");
        if let Some(bar) = &self.bar_frame {
            writeln!(out, "bar_frame {}", join(bar)).expect("writing to a String cannot fail");
        }
    }
}

/// A record written by [`ClassHistogram::write`] could not be read back.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("a class record cannot be read: {0}")]
pub struct ReadRecordError(pub String);

impl ClassHistogram {
    /// Reads back one record [`write`](Self::write) wrote, from the next lines of `lines`, with
    /// the bar-frame histogram if `barred`: its class name and the record, bit for bit.
    ///
    /// # Errors
    ///
    /// [`ReadRecordError`] if a line is missing, out of order or malformed.
    pub fn read<'a>(
        lines: &mut impl Iterator<Item = &'a str>,
        barred: bool,
    ) -> Result<(String, Self), ReadRecordError> {
        let bad = |what: &str| ReadRecordError(what.to_owned());
        let head = lines.next().ok_or_else(|| bad("no class line"))?;
        let tokens: Vec<&str> = head.split_whitespace().collect();
        if tokens.first() != Some(&"class") || tokens.len() != 26 {
            return Err(bad(head));
        }
        let int = |key: &str, at: usize| -> Result<u64, ReadRecordError> {
            if tokens[at] != key {
                return Err(bad(key));
            }
            tokens[at + 1].parse().map_err(|_| bad(key))
        };
        let float =
            |text: &str| -> Result<f64, ReadRecordError> { text.parse().map_err(|_| bad(text)) };
        let mut record = Self::new(barred);
        record.orbits = int("orbits", 2)?;
        record.steps = int("steps", 4)?;
        record.refined = int("refined", 6)?;
        record.over_tolerance = int("over_tolerance", 8)?;
        if tokens[10] != "worst_drift" {
            return Err(bad("worst_drift"));
        }
        record.worst_drift = float(tokens[11])?;
        record.in_cube_bound = int("in_cube_bound", 12)?;
        record.in_cube_unbound = int("in_cube_unbound", 14)?;
        record.outside = int("outside", 16)?;
        record.outbound = int("outbound", 18)?;
        if tokens[20] != "moments" {
            return Err(bad("moments"));
        }
        for (m, text) in record.moments.iter_mut().zip(&tokens[21..]) {
            *m = float(text)?;
        }
        let mut counts = |key: &str, into: &mut Vec<u64>| -> Result<(), ReadRecordError> {
            let line = lines.next().ok_or_else(|| bad(key))?;
            let mut words = line.split_whitespace();
            if words.next() != Some(key) {
                return Err(bad(key));
            }
            let values: Result<Vec<u64>, _> = words.map(str::parse).collect();
            let values = values.map_err(|_| bad(key))?;
            if values.len() != into.len() {
                return Err(bad(key));
            }
            *into = values;
            Ok(())
        };
        counts("bound", &mut record.bound)?;
        counts("unbound", &mut record.unbound)?;
        if let Some(bar) = &mut record.bar_frame {
            counts("bar_frame", bar)?;
        }
        Ok((tokens[1].to_owned(), record))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_cover_their_ranges() {
        assert_eq!(r_cell(0.0), 0);
        assert_eq!(r_cell(15.9), 0);
        assert_eq!(r_cell(16.0), 1);
        assert_eq!(r_cell(262_143.0), R_CELLS - 1);
        assert_eq!(r_cell(1e9), R_CELLS - 1);
        assert_eq!(z_cell(-0.5), 0);
        assert_eq!(z_cell(-65_535.0), Z_CELLS - 1);
        assert!((1..R_CELLS).all(|i| {
            let k = f64::from(u8::try_from(i - 1).unwrap());
            let edge = 16.0 * math::exp2(14.0 * k / 47.0) * 1.000_001;
            r_cell(edge) == i
        }));
        assert!(bar_cell([65_535.0, 65_535.0, 65_535.0]) < BAR_CELLS.iter().product());
    }

    #[test]
    fn merging_adds_and_records_split_by_where_they_end() {
        let five = Integration {
            steps: 5,
            ..Integration::default()
        };
        let mut a = ClassHistogram::new(true);
        a.record([100.0, 0.0, 10.0], [0.1, 0.9, 0.2], true, five);
        a.record([100.0, 0.0, 10.0], [0.0, 0.0, 0.0], false, five);
        a.record(
            [70_000.0, 0.0, 0.0],
            [0.0; 3],
            true,
            Integration {
                over_tolerance: true,
                drift: 0.01,
                ..five
            },
        );
        let mut b = ClassHistogram::new(true);
        b.merge(&a);
        b.merge(&a);
        assert_eq!(
            (b.orbits, b.in_cube_bound, b.in_cube_unbound, b.outside),
            (6, 2, 2, 2)
        );
        assert_eq!(b.outbound, 2);
        assert_eq!(b.steps, 30);
        assert_eq!(b.over_tolerance, 2);
        assert!((b.worst_drift - 0.01).abs() < 1e-15);
        assert_eq!(b.bar_frame.as_ref().unwrap().iter().sum::<u64>(), 2);
        let mut text = String::new();
        b.write("toy", &mut text);
        let (name, back) = ClassHistogram::read(&mut text.lines(), true).unwrap();
        assert_eq!(name, "toy");
        assert_eq!(back, b);
        assert!(text.starts_with("class toy orbits 6 "));
        assert_eq!(text.lines().count(), 4);
    }
}
