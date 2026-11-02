//! The accreting white dwarfs' rows by donor kind (plan 11, P11.T5's fix; research-p15-t10b-rates):
//! each kind of donor scaled to its own observed count, each kind's fast and slow split the
//! engine's.
//!
//! [`share_rows`](super::share_rows) scales the two classes together to one total, so the engine's
//! ratio of donor kinds sets the class's split: helium donors, nearly all helium white dwarfs (AM
//! Canum Venaticorum stars), are 61% of the engine's members, about a tenth observed. Under plan
//! 11's design note 11 the counts are observed, so [`awd_share_rows`] scales each kind to its own
//! figure ([`OBSERVED_DONOR_KINDS`]): a kind's hosts of both classes at the Sun-like point
//! ([`SUN_LY`]) to its observed density, or its hosts in the galaxy to its observed count. A
//! kind's rates, by population and band, are its census members a record, as `share_rows`' are;
//! a class's row is the sum of its kinds', and its donor kinds' shares are its hosts' in the
//! galaxy, which its sampler's pick should draw. No task calls it yet: the refit at 27 switches
//! `fit_awd` to it.

use hyperion_sim::galaxy::consts::LIGHT_YEARS_PER_PARSEC;
use hyperion_sim::galaxy::fields::Component;
use hyperion_sim::galaxy::imf::BandShares;
use hyperion_sim::galaxy::{Galaxy, POPULATIONS, PointLy};
use hyperion_sim::tables::binary::AwdDonorKind;

use super::{AWD_CLASSES, Censuses, ShareRow};

/// What a donor kind's hosts, both classes together, are scaled to: a positive figure.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KindTarget {
    /// Hosts per cubic parsec at the Sun-like point, [`SUN_LY`].
    DensityPerCubicParsec(f64),
    /// Hosts in the galaxy.
    GalacticCount(f64),
}

/// The observed figures, in [`AwdDonorKind`]'s order:
///
/// - a main-sequence or subgiant donor, a cataclysmic variable: 4.8 (+0.6 −0.9) × 10⁻⁶ pc⁻³,
///   Pala et al. (2020, MNRAS 494, 3799), the volume-limited sample within 150 pc;
/// - a helium donor, an AM Canum Venaticorum star: (5 ± 3) × 10⁻⁷ pc⁻³, Carter et al. (2013,
///   MNRAS 429, 2143), from the SDSS spectroscopic sample;
/// - a giant donor, a symbiotic star: 1.0 × 10⁴ in the Galaxy, as the engine counts the wind-fed
///   ones (the white dwarf fed by the giant's wind at 10 L☉ or more, `classify`'s
///   `SYMBIOTIC_MIN_LUMINOSITY`; Lü et al. omit the Roche-lobe-overflow ones the engine also
///   counts): Lü, Yungelson and Han's (2006, MNRAS 372, 1389) population synthesis of the same
///   stars, their accretion model's 7,100 in all (table 2), of which 37 are brighter than `V_c` =
///   12, scaled to the 52 that Belczyński et al.'s (2000, A&AS 146, 407) catalogue holds there, a
///   sample Lü et al. (section 2.4) take as complete: 7,100 × 52 ÷ 37 ≈ 1.0 × 10⁴; their standard
///   and accretion models together (56 of about 11,400) give 1.06 × 10⁴, and each case of table 2
///   scaled alike gives 0.8–1.7 × 10⁴. The observational estimates run "from several thousands
///   (Boyarchuk 1970) to about 30,000 (Kenyon 1994)", as Lü et al.'s section 3.2 quotes them, and
///   to 3 × 10⁵ on an assumed discovery probability (Munari and Renzini 1992, ApJ 397, L87);
///   Magrini, Corradi and Munari's (2003, ASP Conf. Ser. 303, 539) 4 × 10⁵, 0.5% of the Galaxy's
///   red giants, counts quiescent systems too: "only a fraction of them can be observed in the
///   'active' phase" (their section 4).
///
/// The two densities are recalled, not read at source (the science check of 2026-11-01 could
/// reach neither), and are taken as mid-plane densities at [`SUN_LY`], as the sim's tests take
/// Pala et al.'s; the symbiotic count is research-leans-23's, its sources read there.
pub const OBSERVED_DONOR_KINDS: [KindTarget; 3] = [
    KindTarget::DensityPerCubicParsec(4.8e-6),
    KindTarget::DensityPerCubicParsec(5.0e-7),
    KindTarget::GalacticCount(1.0e4),
];

/// The Sun-like point a density target is read at, light-years in the galactic frame (x along the
/// bar, y in the plane, z to galactic north, as `PointLy`'s): the solar circle's census slab's
/// centre (`CensusSite::SOLAR_CIRCLE`), in the mid-plane, where the sim's tests read the class's
/// density.
pub const SUN_LY: [f64; 3] = [0.0, 26_000.0, 0.0];

/// The accreting white dwarfs' rows scaled by donor kind.
#[derive(Debug, Clone, PartialEq)]
pub struct DonorKindRows {
    /// The fast and the slow classes' rows, each the sum of its kinds'; its rate a system is the
    /// census's own, the sum of its kinds' rates (the rate [`share_rows`](super::share_rows)
    /// gives, to rounding).
    pub rows: [ShareRow; 2],
    /// Each kind's fast and slow rows, in [`AwdDonorKind`]'s order.
    pub by_kind: [[ShareRow; 2]; 3],
    /// Each kind's scale, hosts in the galaxy per census member a record and system born, or `None`
    /// for a kind no census found a member of, whose rows are zero.
    pub scales: [Option<f64>; 3],
    /// Each class's hosts of each kind over its hosts, in the galaxy, in [`AwdDonorKind`]'s order.
    pub donor_kinds: [[f64; 3]; 2],
}

/// What the rows read of the galaxy: the band shares of the systems, each population's systems
/// born, its born systems a cubic light-year at the Sun-like point, and the mass formed a system.
#[derive(Debug, Clone, Copy, PartialEq)]
struct RowTerms {
    shares: [f64; 5],
    born: [f64; 7],
    born_per_cubic_ly_at_sun: [f64; 7],
    formed_solar_masses: f64,
}

impl RowTerms {
    /// `galaxy`'s terms. A component's hosts a system are its population's rate × its born
    /// fraction, as `AwdProcess::new` builds them, so its born systems at the point are its
    /// density × its born fraction.
    #[must_use]
    fn of(galaxy: &Galaxy) -> Self {
        let fields = galaxy.fields();
        let [x, y, z] = SUN_LY;
        let sun = PointLy::new(x, y, z);
        let of_population = |term: &dyn Fn(&Component) -> f64| {
            POPULATIONS.map(|population| {
                fields
                    .components()
                    .iter()
                    .filter(|c| c.population() == population)
                    .fold(0.0, |sum, c| sum + term(c))
            })
        };
        Self {
            shares: BandShares::of(galaxy.mass_function()).as_array(),
            born: of_population(&|c| c.count()),
            born_per_cubic_ly_at_sun: of_population(&|c| {
                c.ages().born_fraction() * c.density(&sun)
            }),
            formed_solar_masses: galaxy.mean_formed_mass().value(),
        }
    }
}

/// One kind's and class's terms: its rate a system by population, the census's own, and its hosts
/// in the galaxy under its kind's scale, in each layer and in all.
#[derive(Debug, Clone, Copy, PartialEq)]
struct KindTerms {
    rate_per_system: [f64; 7],
    layer_hosts: [f64; 5],
    hosts: f64,
}

/// Each class's members of each donor kind a record, by population and band: `[class][kind][p][b]`.
type KindRates = [[[[f64; 5]; 7]; 3]; 2];

/// The rates of `censuses`: each class's members of each kind of its population over the records
/// its band's census ran of it.
///
/// # Panics
///
/// If a census ran no record of a population.
#[must_use]
fn kind_rates(censuses: &Censuses) -> KindRates {
    let mut rates = [[[[0.0; 5]; 7]; 3]; 2];
    for population in POPULATIONS {
        for census in censuses.of(population) {
            let records = census.scanned[population.index()];
            assert!(records > 0, "a census ran no record of {population:?}");
            let mut counts = [[0_u32; 3]; 2];
            for m in census
                .members
                .iter()
                .filter(|m| m.record().population() == population)
            {
                let Some(class) = AWD_CLASSES.iter().position(|&c| c == m.class()) else {
                    continue;
                };
                // `carved_class` puts a pair in an accreting class only at an age its white dwarf
                // is fed, and the marks read the same ages.
                let kind = m
                    .accreting()
                    .expect("an accreting white dwarf member has its marks")
                    .donor_kind();
                let k = AwdDonorKind::ALL
                    .iter()
                    .position(|&d| d == kind)
                    .expect("every kind is in `AwdDonorKind::ALL`");
                counts[class][k] += 1;
            }
            for (class, by_kind) in counts.iter().enumerate() {
                for (k, &count) in by_kind.iter().enumerate() {
                    #[expect(clippy::cast_precision_loss, reason = "a record count under 2⁵³")]
                    let rate = f64::from(count) / records as f64;
                    rates[class][k][population.index()][census.band.index()] = rate;
                }
            }
        }
    }
    rates
}

/// The rows of the two accreting white dwarf classes from `censuses` in `galaxy`, each donor kind
/// scaled to its target in `targets` (module documentation).
///
/// # Panics
///
/// If a target is not a positive figure, a census ran no record of a population, or no census
/// found a member of any kind.
#[must_use]
pub fn awd_share_rows(
    galaxy: &Galaxy,
    censuses: &Censuses,
    targets: &[KindTarget; 3],
) -> DonorKindRows {
    scaled_rows(&RowTerms::of(galaxy), &kind_rates(censuses), targets)
}

/// One population's rate a system of `rate`, a census's members a record by band: its bands'
/// rates weighted by their shares of the systems.
#[must_use]
fn per_system(terms: &RowTerms, rate: &[[f64; 5]; 7], p: usize) -> f64 {
    rate[p]
        .iter()
        .zip(terms.shares)
        .fold(0.0, |sum, (r, s)| sum + r * s)
}

/// Each kind's scale: its target over its census figure, both classes, its density at the point
/// per cubic parsec or its hosts in the galaxy at one host a member a record and system born;
/// `None` for a kind no census found.
///
/// # Panics
///
/// If a target is not a positive figure, or no kind has a member.
#[must_use]
fn kind_scales(terms: &RowTerms, rates: &KindRates, targets: &[KindTarget; 3]) -> [Option<f64>; 3] {
    let scales = std::array::from_fn(|k| {
        let target = targets[k];
        let (wanted, weights) = match target {
            KindTarget::DensityPerCubicParsec(n) => (
                n,
                terms.born_per_cubic_ly_at_sun.map(|born| {
                    born * LIGHT_YEARS_PER_PARSEC * LIGHT_YEARS_PER_PARSEC * LIGHT_YEARS_PER_PARSEC
                }),
            ),
            KindTarget::GalacticCount(n) => (n, terms.born),
        };
        assert!(
            wanted.is_finite() && wanted > 0.0,
            "a kind's target is a positive figure: {target:?}"
        );
        let figure = rates.iter().fold(0.0, |sum, by_kind| {
            (0..7).fold(sum, |sum, p| {
                sum + per_system(terms, &by_kind[k], p) * weights[p]
            })
        });
        (figure > 0.0).then(|| wanted / figure)
    });
    assert!(
        scales.iter().any(Option::is_some),
        "the censuses found members of the accreting white dwarfs"
    );
    scales
}

/// [`awd_share_rows`] of the galaxy's `terms` and the census's `rates`.
///
/// # Panics
///
/// As [`kind_scales`].
#[must_use]
fn scaled_rows(terms: &RowTerms, rates: &KindRates, targets: &[KindTarget; 3]) -> DonorKindRows {
    let scales = kind_scales(terms, rates, targets);
    let kinds: [[KindTerms; 2]; 3] = std::array::from_fn(|k| {
        std::array::from_fn(|class| {
            let (rate, scale) = (&rates[class][k], scales[k].unwrap_or(0.0));
            let rate_per_system: [f64; 7] = std::array::from_fn(|p| per_system(terms, rate, p));
            KindTerms {
                layer_hosts: std::array::from_fn(|b| {
                    (0..7).fold(0.0, |sum, p| {
                        sum + scale * rate[p][b] * terms.shares[b] * terms.born[p]
                    })
                }),
                hosts: (0..7).fold(0.0, |sum, p| {
                    sum + scale * rate_per_system[p] * terms.born[p]
                }),
                rate_per_system,
            }
        })
    });
    let normalised = |hosts: [f64; 5]| {
        let sum = hosts.iter().sum::<f64>();
        if sum > 0.0 {
            hosts.map(|h| h / sum)
        } else {
            hosts
        }
    };
    let by_kind: [[ShareRow; 2]; 3] = std::array::from_fn(|k| {
        std::array::from_fn(|class| {
            let terms_k = &kinds[k][class];
            let scale = scales[k].unwrap_or(0.0);
            ShareRow {
                hosts_per_formed_mass: terms_k
                    .rate_per_system
                    .map(|r| scale * r / terms.formed_solar_masses),
                layer_shares: normalised(terms_k.layer_hosts),
                rate_per_system: terms_k.rate_per_system,
            }
        })
    });
    let rows: [ShareRow; 2] = std::array::from_fn(|class| {
        let (mut hosts_per_formed_mass, mut rate_per_system) = ([0.0; 7], [0.0; 7]);
        let mut layer_hosts = [0.0; 5];
        for (kind_rows, kind_terms) in by_kind.iter().zip(&kinds) {
            let terms_k = &kind_terms[class];
            for p in 0..7 {
                hosts_per_formed_mass[p] += kind_rows[class].hosts_per_formed_mass[p];
                rate_per_system[p] += terms_k.rate_per_system[p];
            }
            for (sum, h) in layer_hosts.iter_mut().zip(terms_k.layer_hosts) {
                *sum += h;
            }
        }
        ShareRow {
            hosts_per_formed_mass,
            layer_shares: normalised(layer_hosts),
            rate_per_system,
        }
    });
    let donor_kinds = std::array::from_fn(|class| {
        let hosts: [f64; 3] = std::array::from_fn(|k| kinds[k][class].hosts);
        let total = hosts.iter().sum::<f64>();
        if total > 0.0 {
            hosts.map(|h| h / total)
        } else {
            hosts
        }
    });
    DonorKindRows {
        rows,
        by_kind,
        scales,
        donor_kinds,
    }
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;

    use super::*;

    /// Terms of two populations, the rest empty, with bands D and E.
    fn terms() -> RowTerms {
        RowTerms {
            shares: [0.5, 0.3, 0.1, 0.06, 0.04],
            born: [2.0e9, 5.0e10, 0.0, 0.0, 0.0, 0.0, 0.0],
            born_per_cubic_ly_at_sun: [1.0e-4, 8.0e-3, 0.0, 0.0, 0.0, 0.0, 0.0],
            formed_solar_masses: 0.6,
        }
    }

    /// Rates of each class and kind, a different one for each, in the first two populations.
    fn rates() -> KindRates {
        let mut rates = [[[[0.0; 5]; 7]; 3]; 2];
        for (class, by_kind) in rates.iter_mut().enumerate() {
            for (k, rate) in by_kind.iter_mut().enumerate() {
                for (p, by_band) in rate.iter_mut().take(2).enumerate() {
                    let base = f64::from(u32::try_from(1 + class + 2 * k + 6 * p).unwrap());
                    by_band[3] = 1.0e-5 * base;
                    by_band[4] = 3.0e-5 * base;
                }
            }
        }
        rates
    }

    /// A row's density at the Sun-like point, pc⁻³, and its hosts in the galaxy, read as the sim
    /// reads a row: hosts a system its hosts per solar mass formed × the mass formed.
    fn figures(terms: &RowTerms, row: &ShareRow) -> (f64, f64) {
        let pc3 = LIGHT_YEARS_PER_PARSEC * LIGHT_YEARS_PER_PARSEC * LIGHT_YEARS_PER_PARSEC;
        (0..7).fold((0.0, 0.0), |(density, hosts), p| {
            let per_system = row.hosts_per_formed_mass[p] * terms.formed_solar_masses;
            (
                density + per_system * terms.born_per_cubic_ly_at_sun[p] * pc3,
                hosts + per_system * terms.born[p],
            )
        })
    }

    /// Each kind's rows give its target, a density at the point or a count in the galaxy; each
    /// kind keeps the engine's split between the classes by population; a class's row is the sum
    /// of its kinds', its rate the census's own; and the donor kinds' shares are each class's
    /// hosts'.
    #[test]
    fn each_kind_is_its_target_and_keeps_the_engine_s_split() {
        let (terms, rates) = (terms(), rates());
        let rows = scaled_rows(&terms, &rates, &OBSERVED_DONOR_KINDS);
        for (k, target) in OBSERVED_DONOR_KINDS.iter().enumerate() {
            let [fast, slow] = &rows.by_kind[k];
            let (density, hosts) = [fast, slow].iter().fold((0.0, 0.0), |(d, h), row| {
                let (dr, hr) = figures(&terms, row);
                (d + dr, h + hr)
            });
            let (got, wanted) = match *target {
                KindTarget::DensityPerCubicParsec(n) => (density, n),
                KindTarget::GalacticCount(n) => (hosts, n),
            };
            assert!(
                (got / wanted - 1.0).abs() < 1e-12,
                "{k}: {got} against {wanted}"
            );
            for (p, (fast_rate, slow_rate)) in rates[0][k].iter().zip(&rates[1][k]).enumerate() {
                if slow_rate[3] <= 0.0 {
                    continue;
                }
                let engine = fast_rate[3] / slow_rate[3];
                let row = fast.hosts_per_formed_mass[p] / slow.hosts_per_formed_mass[p];
                assert!((row / engine - 1.0).abs() < 1e-12, "{k}, {p}");
            }
        }
        for class in 0..2 {
            let row = &rows.rows[class];
            for p in 0..7 {
                let sum: f64 = rows
                    .by_kind
                    .iter()
                    .map(|kind| kind[class].hosts_per_formed_mass[p])
                    .sum();
                assert!(
                    (row.hosts_per_formed_mass[p] - sum).abs() <= 1e-12 * sum.abs(),
                    "{class}, {p}"
                );
                let rate: f64 = rows
                    .by_kind
                    .iter()
                    .map(|kind| kind[class].rate_per_system[p])
                    .sum();
                assert!((row.rate_per_system[p] - rate).abs() <= 1e-15);
            }
            assert!((row.layer_shares.iter().sum::<f64>() - 1.0).abs() < 1e-12);
            let hosts: Vec<f64> = rows
                .by_kind
                .iter()
                .map(|kind| figures(&terms, &kind[class]).1)
                .collect();
            let total: f64 = hosts.iter().sum();
            for (share, h) in rows.donor_kinds[class].iter().zip(&hosts) {
                assert!((share - h / total).abs() < 1e-12);
            }
        }
    }

    /// A kind no census found keeps zero rows and no scale, and the others still meet their
    /// targets.
    #[test]
    fn a_kind_with_no_member_is_left_at_zero() {
        let terms = terms();
        let mut rates = rates();
        for by_kind in &mut rates {
            by_kind[2] = [[0.0; 5]; 7];
        }
        let rows = scaled_rows(&terms, &rates, &OBSERVED_DONOR_KINDS);
        assert_eq!(rows.scales[2], None);
        for row in &rows.by_kind[2] {
            for h in row.hosts_per_formed_mass {
                assert_same_bits(h, 0.0);
            }
        }
        let density =
            figures(&terms, &rows.by_kind[1][0]).0 + figures(&terms, &rows.by_kind[1][1]).0;
        assert!((density / 5.0e-7 - 1.0).abs() < 1e-12);
        for shares in rows.donor_kinds {
            assert_same_bits(shares[2], 0.0);
        }
    }
}
