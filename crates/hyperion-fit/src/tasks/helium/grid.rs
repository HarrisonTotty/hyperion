//! `BaSTI`'s helium-enhanced grid as `data/basti_helium/` holds it, reduced by its `reduce.py`
//! (plan 15, P15.T7.b).
//!
//! It holds the ages at the end of the main sequence of the 0.6–0.9 M☉ tracks and the zero-age
//! horizontal branches by envelope mass, at each (Z, Y) of the grid, and carries each family of
//! sets (the canonical one, or the helium-enhanced sets of one Y) to another Z, which the fit
//! compares each helium-enhanced set with.
//!
//! Each set is placed at its own total Z (the lean of P15.T7.b's brief): an α-enhanced mixture
//! tracks a scaled-solar one of the same Z (Salaris, Chieffi and Straniero 1993, ApJ 414, 580), so
//! the set stands for a star of the sim's \[Fe/H\] = log₁₀(Z ÷ 0.02). The equivalence holds to Z ≈
//! 0.002 (Pietrinferni et al. 2006, ApJ 642, 797, §1); the rows at Z = 0.004 and 0.008, and the
//! node at \[Fe/H\] −0.5, extrapolate it, though both models of each ratio share the mixture. At one \[M/H\] the grid holds
//! Z ÷ X fixed, so a helium-enhanced set's Z is below its canonical set's; a family is carried to
//! another Z by linear interpolation in log₁₀ Z between its two neighbouring sets, or linear
//! extrapolation from the nearest two beyond them, so that every ratio is taken at one Z.
//!
//! The sets are told apart by their three numbers exactly, as the reduction writes them: those
//! comparisons are of values read from the same file. A mass a caller gives is matched to within
//! [`MASS_MATCH_MSUN`].

use std::fmt;

use hyperion_sim::math;

/// How close a caller's mass must be to a track's to name it, M☉: the files give masses to two
/// decimals.
pub const MASS_MATCH_MSUN: f64 = 1e-9;

/// One track's age at the end of its main sequence: a row of `turnoff.csv`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TurnOff {
    /// The canonical Z of the set's \[M/H\] row.
    pub z_row: f64,
    /// The set's metal fraction Z.
    pub z: f64,
    /// The set's helium fraction Y.
    pub y: f64,
    /// The initial mass, M☉.
    pub mass_msun: f64,
    /// log₁₀ of the age at the end of the main sequence, years from the zero-age main sequence
    /// (the readme's key point 4: central hydrogen exhausted).
    pub log_age_yr: f64,
}

/// One point of a zero-age horizontal branch: a row of `zahb.csv`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZahbPoint {
    /// The canonical Z of the set's \[M/H\] row.
    pub z_row: f64,
    /// The set's metal fraction Z.
    pub z: f64,
    /// The set's helium fraction Y.
    pub y: f64,
    /// The helium core mass that sets the envelope, M☉.
    pub core_mass_msun: f64,
    /// The star's mass on the branch, M☉.
    pub hb_mass_msun: f64,
    /// Its envelope mass, `hb_mass_msun` − `core_mass_msun`, M☉.
    pub envelope_msun: f64,
    /// log₁₀ L ÷ L☉.
    pub log_l: f64,
    /// log₁₀ `T_eff` ÷ K.
    pub log_teff: f64,
}

/// The reduced grid: its turn-offs and its branches, each in its file's order, as
/// [`Grid::parse`] checked them.
#[derive(Debug, Clone, PartialEq)]
pub struct Grid {
    turn_offs: Vec<TurnOff>,
    zahb: Vec<ZahbPoint>,
}

/// One (Z, Y) set of the grid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Set {
    /// The canonical Z of its \[M/H\] row.
    pub z_row: f64,
    /// Its metal fraction.
    pub z: f64,
    /// Its helium fraction.
    pub y: f64,
}

impl Set {
    /// Whether it is its row's canonical set, of the row's own Z.
    #[must_use]
    pub fn is_canonical(&self) -> bool {
        self.z.total_cmp(&self.z_row).is_eq()
    }

    /// The sim's \[Fe/H\] for a star of the set's Z: log₁₀(Z ÷ 0.02), the sim's scaled-solar Z☉
    /// (`stellar::composition`).
    #[must_use]
    pub fn fe_h(&self) -> f64 {
        math::log10(self.z / SIM_Z_SOLAR)
    }

    /// Whether `(z_row, z, y)` read from the same file are this set's.
    fn holds(&self, z_row: f64, z: f64, y: f64) -> bool {
        self.z_row.total_cmp(&z_row).is_eq()
            && self.z.total_cmp(&z).is_eq()
            && self.y.total_cmp(&y).is_eq()
    }
}

/// Which family of sets a branch is carried in: the canonical sets, whose Y rises a little with
/// Z, or the helium-enhanced sets of one Y.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Family {
    /// The canonical sets.
    Canonical,
    /// The helium-enhanced sets of helium fraction `y`.
    Enhanced {
        /// Their Y.
        y: f64,
    },
}

/// The sim's solar metal fraction, which its \[Fe/H\] is reckoned from: Z = 0.02 × 10^\[Fe/H\]
/// (`Composition::from_fe_h`).
pub const SIM_Z_SOLAR: f64 = 0.02;

/// Why a line of `turnoff.csv` or `zahb.csv` is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridLineFault {
    /// The first line is not the reduction's header.
    Header,
    /// A cell does not parse as a number.
    NotANumber,
    /// A line has another number of cells than the header.
    Width,
    /// A cell is infinite or not a number.
    NotFinite,
    /// A branch's envelope mass is not its mass less its core mass.
    Envelope,
}

impl fmt::Display for GridLineFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Header => "the header is not the reduction's",
            Self::NotANumber => "a cell is not a number",
            Self::Width => "the line has the wrong number of cells",
            Self::NotFinite => "a cell is not finite",
            Self::Envelope => "the envelope mass is not the mass less the core mass",
        })
    }
}

/// `turnoff.csv` or `zahb.csv` could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadGridError {
    /// The file.
    pub file: &'static str,
    /// The line, counted from 1.
    pub line: usize,
    /// What is wrong with it.
    pub fault: GridLineFault,
}

impl fmt::Display for ReadGridError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} line {}: {}", self.file, self.line, self.fault)
    }
}

impl std::error::Error for ReadGridError {}

/// The numbers of every line of `text` after its header, which must be `header`, each line as
/// many as the header has names, with its line number.
fn rows(
    file: &'static str,
    text: &str,
    header: &str,
) -> Result<Vec<(usize, Vec<f64>)>, ReadGridError> {
    let error = |line: usize, fault: GridLineFault| ReadGridError { file, line, fault };
    let mut lines = text.lines();
    if lines.next() != Some(header) {
        return Err(error(1, GridLineFault::Header));
    }
    let width = header.split(',').count();
    let mut out = Vec::new();
    for (i, line) in lines.enumerate() {
        let cells: Result<Vec<f64>, _> = line.split(',').map(str::parse::<f64>).collect();
        let cells = cells.map_err(|_| error(i + 2, GridLineFault::NotANumber))?;
        if cells.len() != width {
            return Err(error(i + 2, GridLineFault::Width));
        }
        if !cells.iter().all(|c| c.is_finite()) {
            return Err(error(i + 2, GridLineFault::NotFinite));
        }
        out.push((i + 2, cells));
    }
    Ok(out)
}

/// A family's branch at one Z: the two sets' branches that carry it and the second's share.
#[derive(Debug, Clone, PartialEq)]
pub struct CarriedBranch {
    below: Vec<ZahbPoint>,
    above: Vec<ZahbPoint>,
    share: f64,
}

impl CarriedBranch {
    /// log₁₀ `T_eff` at envelope mass `envelope_msun` (M☉), or `None` outside either branch.
    #[must_use]
    pub fn log_teff(&self, envelope_msun: f64) -> Option<f64> {
        let a = branch_log_teff(&self.below, envelope_msun)?;
        let b = branch_log_teff(&self.above, envelope_msun)?;
        Some((1.0 - self.share) * a + self.share * b)
    }

    /// The core mass that sets the carried branch's envelopes, M☉, or `None` for a branch with no
    /// points.
    #[must_use]
    pub fn core_mass_msun(&self) -> Option<f64> {
        let (a, b) = (self.below.first()?, self.above.first()?);
        Some((1.0 - self.share) * a.core_mass_msun + self.share * b.core_mass_msun)
    }

    /// [`log_teff`](Self::log_teff) with the envelope held at each branch's ends beyond them, or
    /// `None` for a branch with no points.
    #[must_use]
    pub fn log_teff_held(&self, envelope_msun: f64) -> Option<f64> {
        let held = |branch: &[ZahbPoint]| {
            let (lo, hi) = (branch.first()?.envelope_msun, branch.last()?.envelope_msun);
            branch_log_teff(branch, envelope_msun.clamp(lo, hi))
        };
        let (a, b) = (held(&self.below)?, held(&self.above)?);
        Some((1.0 - self.share) * a + self.share * b)
    }
}

impl Grid {
    /// The header line of `turnoff.csv`.
    pub const TURNOFF_HEADER: &'static str = "z_row,z,y,mass,log_age_ms_end";

    /// The header line of `zahb.csv`.
    pub const ZAHB_HEADER: &'static str =
        "z_row,z,y,core_mass,hb_mass,envelope_mass,log_l,log_teff";

    /// Reads the grid from the texts of `turnoff.csv` and `zahb.csv`.
    ///
    /// # Errors
    ///
    /// [`ReadGridError`] if a file's header is not the reduction's, a line does not hold that
    /// many finite numbers, or a branch's envelope mass is not its mass less its core mass.
    pub fn parse(turnoff: &str, zahb: &str) -> Result<Self, ReadGridError> {
        let turn_offs = rows("turnoff.csv", turnoff, Self::TURNOFF_HEADER)?
            .into_iter()
            .map(|(_, c)| TurnOff {
                z_row: c[0],
                z: c[1],
                y: c[2],
                mass_msun: c[3],
                log_age_yr: c[4],
            })
            .collect();
        let mut points = Vec::new();
        for (line, c) in rows("zahb.csv", zahb, Self::ZAHB_HEADER)? {
            // The reduction subtracts in decimal to four places.
            if (c[4] - c[3] - c[5]).abs() > 1e-9 {
                return Err(ReadGridError {
                    file: "zahb.csv",
                    line,
                    fault: GridLineFault::Envelope,
                });
            }
            points.push(ZahbPoint {
                z_row: c[0],
                z: c[1],
                y: c[2],
                core_mass_msun: c[3],
                hb_mass_msun: c[4],
                envelope_msun: c[5],
                log_l: c[6],
                log_teff: c[7],
            });
        }
        Ok(Self {
            turn_offs,
            zahb: points,
        })
    }

    /// Every row of `turnoff.csv`, in its order.
    #[must_use]
    pub fn turn_offs(&self) -> &[TurnOff] {
        &self.turn_offs
    }

    /// Every row of `zahb.csv`, in its order.
    #[must_use]
    pub fn zahb(&self) -> &[ZahbPoint] {
        &self.zahb
    }

    /// The grid's sets, in `turnoff.csv`'s order (by row, then Z, then Y), each once.
    #[must_use]
    pub fn sets(&self) -> Vec<Set> {
        let mut sets: Vec<Set> = Vec::new();
        for t in &self.turn_offs {
            if !sets.iter().any(|s| s.holds(t.z_row, t.z, t.y)) {
                sets.push(Set {
                    z_row: t.z_row,
                    z: t.z,
                    y: t.y,
                });
            }
        }
        sets
    }

    /// The sets of `family`, by increasing Z.
    #[must_use]
    pub fn family(&self, family: Family) -> Vec<Set> {
        let mut sets: Vec<Set> = self
            .sets()
            .into_iter()
            .filter(|s| match family {
                Family::Canonical => s.is_canonical(),
                Family::Enhanced { y } => !s.is_canonical() && s.y.total_cmp(&y).is_eq(),
            })
            .collect();
        sets.sort_by(|a, b| a.z.total_cmp(&b.z));
        sets
    }

    /// The canonical sets, by increasing Z.
    #[must_use]
    pub fn canonical(&self) -> Vec<Set> {
        self.family(Family::Canonical)
    }

    /// log₁₀ of the age (years) at the end of the main sequence of `set`'s track of initial mass
    /// `mass_msun` (M☉), if it has one.
    #[must_use]
    pub fn log_age_yr(&self, set: &Set, mass_msun: f64) -> Option<f64> {
        self.turn_offs
            .iter()
            .find(|t| {
                set.holds(t.z_row, t.z, t.y) && (t.mass_msun - mass_msun).abs() < MASS_MATCH_MSUN
            })
            .map(|t| t.log_age_yr)
    }

    /// `set`'s zero-age horizontal branch, by increasing envelope mass, each point once: the
    /// files repeat a few points exactly.
    #[must_use]
    pub fn branch(&self, set: &Set) -> Vec<ZahbPoint> {
        let mut points: Vec<ZahbPoint> = self
            .zahb
            .iter()
            .filter(|p| set.holds(p.z_row, p.z, p.y))
            .copied()
            .collect();
        points.sort_by(|a, b| a.envelope_msun.total_cmp(&b.envelope_msun));
        points.dedup_by(|a, b| {
            [a.envelope_msun, a.log_l, a.log_teff]
                .iter()
                .zip([b.envelope_msun, b.log_l, b.log_teff])
                .all(|(x, y)| x.total_cmp(&y).is_eq())
        });
        points
    }

    /// The two sets of `family` that carry it to log₁₀ Z `log_z`, and the share of the second: the
    /// neighbours of `log_z`, or the nearest two beyond the ends, whose share then lies outside
    /// 0–1.
    fn pair(&self, family: Family, log_z: f64) -> Option<(Set, Set, f64)> {
        let sets = self.family(family);
        if sets.len() < 2 {
            return None;
        }
        let logs: Vec<f64> = sets.iter().map(|s| math::log10(s.z)).collect();
        let last = sets.len() - 2;
        let i = logs[1..=last].iter().take_while(|&&l| log_z >= l).count();
        let share = (log_z - logs[i]) / (logs[i + 1] - logs[i]);
        Some((sets[i], sets[i + 1], share))
    }

    /// The canonical sequence's helium fraction Y₀ at metal fraction `z`.
    #[must_use]
    pub fn canonical_y(&self, z: f64) -> Option<f64> {
        let (a, b, share) = self.pair(Family::Canonical, math::log10(z))?;
        Some((1.0 - share) * a.y + share * b.y)
    }

    /// The canonical sequence's ln of the age (years) at the end of the main sequence of a star of
    /// initial mass `mass_msun` (M☉) at metal fraction `z`, if both its sets have the mass.
    #[must_use]
    pub fn canonical_ln_age_yr(&self, z: f64, mass_msun: f64) -> Option<f64> {
        let (a, b, share) = self.pair(Family::Canonical, math::log10(z))?;
        let (ta, tb) = (
            self.log_age_yr(&a, mass_msun)?,
            self.log_age_yr(&b, mass_msun)?,
        );
        Some(std::f64::consts::LN_10 * ((1.0 - share) * ta + share * tb))
    }

    /// `family`'s zero-age horizontal branch carried to metal fraction `z`.
    #[must_use]
    pub fn carried_branch(&self, family: Family, z: f64) -> Option<CarriedBranch> {
        let (a, b, share) = self.pair(family, math::log10(z))?;
        Some(CarriedBranch {
            below: self.branch(&a),
            above: self.branch(&b),
            share,
        })
    }
}

/// log₁₀ `T_eff` on `branch` (by increasing envelope mass, each point once) at envelope mass
/// `envelope_msun` (M☉), linear between its points; `None` outside it.
#[must_use]
pub fn branch_log_teff(branch: &[ZahbPoint], envelope_msun: f64) -> Option<f64> {
    let (first, last) = (branch.first()?, branch.last()?);
    if envelope_msun < first.envelope_msun || envelope_msun > last.envelope_msun {
        return None;
    }
    if branch.len() == 1 {
        return Some(first.log_teff);
    }
    branch.windows(2).find_map(|w| {
        let (p, q) = (w[0], w[1]);
        let width = q.envelope_msun - p.envelope_msun;
        (envelope_msun <= q.envelope_msun && width > 0.0).then(|| {
            let share = (envelope_msun - p.envelope_msun) / width;
            (1.0 - share) * p.log_teff + share * q.log_teff
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TURNOFF: &str = "z_row,z,y,mass,log_age_ms_end\n\
        0.001,0.0008,0.4000,0.80,9.6\n\
        0.001,0.001,0.2460,0.80,10.1\n\
        0.002,0.002,0.2480,0.80,10.2\n";
    const ZAHB: &str = "z_row,z,y,core_mass,hb_mass,envelope_mass,log_l,log_teff\n\
        0.001,0.001,0.2460,0.4906,0.4912,0.0006,1.3,4.5\n\
        0.001,0.001,0.2460,0.4906,0.5912,0.1006,1.5,4.1\n\
        0.002,0.002,0.2480,0.4877,0.4882,0.0005,1.3,4.4\n\
        0.002,0.002,0.2480,0.4877,0.5882,0.1005,1.5,4.0\n";

    #[test]
    fn the_grid_parses_and_finds_its_canonical_sets() {
        let grid = Grid::parse(TURNOFF, ZAHB).unwrap();
        assert_eq!(grid.sets().len(), 3);
        let canonical = grid.canonical();
        assert_eq!(canonical.len(), 2);
        assert!(canonical.iter().all(Set::is_canonical));
        assert!(canonical[0].z < canonical[1].z);
        assert_eq!(grid.log_age_yr(&canonical[0], 0.8), Some(10.1));
        assert_eq!(grid.log_age_yr(&canonical[0], 0.7), None);
    }

    #[test]
    fn the_canonical_sequence_is_carried_in_log_z() {
        let grid = Grid::parse(TURNOFF, ZAHB).unwrap();
        // Halfway in log Z between 0.001 and 0.002, and extrapolated below 0.001.
        let mid = (0.001_f64 * 0.002).sqrt();
        let ln_age = grid.canonical_ln_age_yr(mid, 0.8).unwrap();
        assert!(
            (ln_age - std::f64::consts::LN_10 * 10.15).abs() < 1e-12,
            "{ln_age}"
        );
        let y = grid.canonical_y(0.0005).unwrap();
        assert!((y - 0.244).abs() < 1e-12, "{y}");
        let branch = grid.carried_branch(Family::Canonical, 0.001).unwrap();
        let t = branch.log_teff(0.0506).unwrap();
        assert!((t - 4.3).abs() < 1e-12, "{t}");
        assert_eq!(branch.log_teff(0.2), None);
        let held = branch.log_teff_held(0.2).unwrap();
        assert!((held - 4.1).abs() < 1e-12, "{held}");
    }

    /// A point the files repeat exactly is kept once, and a branch with two points at one envelope
    /// mass gives no 0 ÷ 0 there.
    #[test]
    fn a_repeated_point_is_read_once() {
        let zahb = format!("{ZAHB}0.001,0.001,0.2460,0.4906,0.5912,0.1006,1.5,4.1\n");
        let grid = Grid::parse(TURNOFF, &zahb).unwrap();
        let set = grid.canonical()[0];
        assert_eq!(grid.branch(&set).len(), 2);
        let tied = [
            ZahbPoint {
                envelope_msun: 0.0,
                log_teff: 4.5,
                ..grid.branch(&set)[0]
            },
            ZahbPoint {
                envelope_msun: 0.0,
                log_teff: 4.4,
                ..grid.branch(&set)[0]
            },
            ZahbPoint {
                envelope_msun: 0.1,
                log_teff: 4.0,
                ..grid.branch(&set)[0]
            },
        ];
        let t = branch_log_teff(&tied, 0.0).unwrap();
        assert!(t.is_finite() && (t - 4.4).abs() < 1e-12, "{t}");
    }

    #[test]
    fn a_malformed_file_is_refused() {
        let fault = |turnoff: &str, zahb: &str| Grid::parse(turnoff, zahb).unwrap_err();
        assert_eq!(
            fault("z,y\n", ZAHB),
            ReadGridError {
                file: "turnoff.csv",
                line: 1,
                fault: GridLineFault::Header
            }
        );
        assert_eq!(
            fault("z,y\n", ZAHB).to_string(),
            "turnoff.csv line 1: the header is not the reduction's"
        );
        assert_eq!(
            fault(TURNOFF, &ZAHB.replace("0.1006", "0.1007")),
            ReadGridError {
                file: "zahb.csv",
                line: 3,
                fault: GridLineFault::Envelope
            }
        );
        assert_eq!(
            fault(&format!("{TURNOFF}0.001,0.001\n"), ZAHB),
            ReadGridError {
                file: "turnoff.csv",
                line: 5,
                fault: GridLineFault::Width
            }
        );
        assert_eq!(
            fault(&format!("{TURNOFF}0.001,0.001,0.25,0.8,ten\n"), ZAHB),
            ReadGridError {
                file: "turnoff.csv",
                line: 5,
                fault: GridLineFault::NotANumber
            }
        );
        assert_eq!(
            fault(&format!("{TURNOFF}0.001,0.001,0.25,0.8,inf\n"), ZAHB),
            ReadGridError {
                file: "turnoff.csv",
                line: 5,
                fault: GridLineFault::NotFinite
            }
        );
    }
}
