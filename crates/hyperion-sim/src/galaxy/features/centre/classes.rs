//! The centre's member classes by the class device of P09.T8: what the nuclear cluster holds and
//! on which distribution function each class lies (plan 09, P09.T26; Design note 14).
//!
//! # Ages
//!
//! The members are not coeval (the brainstorm, "Dense features"). Their initial mass follows
//! Schödel et al.'s (2020, A&A 641, A102, §6.1) star-formation history, as [`age_components`]
//! holds it (ruling 144.10; the old and intermediate spans are ours):
//!
//! - 80% of the mass formed 10–13 Gyr ago ("about 80% … 10 Gyr ago or longer");
//! - 15% 2.5–3.5 Gyr ago ("about 15% at 2–4 Gyr");
//! - 3% 150–500 Myr ago ("a few percent … in the past 150–500 Myr"), on the stars' profile;
//! - 1% at a constant rate from `H` after the epoch to 150 Myr ago ("≲ 1% of star formation
//!   happened in the past 100 Myr"), on the stars' profile, which keeps the members born within
//!   the clock's window;
//! - and a burst of [`BURST_SHARE`] of the mass formed, 2.5 × 10⁴ M☉ at Milky Way values (Lu et
//!   al. 2013, ApJ 764, 155: 1.4–3.7 × 10⁴ M☉ for the young cluster), uniformly 3–8 Myr ago
//!   (Lu et al. 2013's 2.5–5.8 Myr; Paumard et al. 2006's 6 ± 2 Myr): a third on the clockwise
//!   disc ([`CentreTracer::YoungDisc`]; Yelda et al. 2014 find about a fifth of the young stars
//!   on it, Lu et al. 2009 and Bartko et al. 2009 about half) and two thirds isotropic
//!   ([`CentreTracer::YoungIsotropic`]).
//!
//! The shares are relative, and divided by their sum (0.9906): what Schödel et al. leave out is
//! the near-zero 5–10 Gyr and the minimum near 1 Gyr. The classes are then scaled to the
//! cluster's present mass.
//!
//! # Classes and counts
//!
//! In each component, at four Gauss–Legendre ages, the galaxy's mass function gives per primary
//! formed the living stars of each band below the turn-off (plan 06's `turn_off_mass`), the white
//! dwarfs of each band from the turn-off to 8 M☉ (Kalirai et al. 2008's initial–final relation, as
//! the clusters' classes), and the neutron stars and black holes of plan 06's kick law **that stay
//! bound**. Every class is then scaled by one factor so that the classes' present mass is the
//! cluster's ([`CentreProfile::mass`]). The retained neutron stars and black holes are the
//! share of each progenitor's remnants kicked below the local escape speed `√(2Ψ(r))` (1,100
//! km/s at 0.1 ly, 210 at 10 ly), averaged over the stars' profile inside the reach, each
//! progenitor's share read from the clusters' retention table (ruling 139.5), which ends at 500
//! km/s and is held there above it: the 2% of the stars inside about 1 ly, where the escape speed
//! is higher, keep a little too few (about 0.01 on the neutron stars' share; the test holds the
//! table to plan 08's quadrature within 0.02). The low mode is judged on its pair's systemic
//! speed (ruling 126.3). About a third of the neutron stars and nine tenths of the black holes
//! stay (ruling 144.9): under Disberg and Mandel's (2025) log-normal ordinary kicks of plan 06,
//! Φ((ln v − 5.60) ÷ 0.68) keeps 0.36 at 210 km/s, and the low mode's pairs are all kept, where
//! the brainstorm's earlier "about a fifth" came from a Maxwellian of σ = 265 km/s. The rest are
//! the bulge's displaced remnants and are not generated here.
//!
//! # Profiles
//!
//! Four distribution functions (Design note 14 had three; ruling 144.6 splits the young), in the
//! one potential of the black hole and the stars:
//!
//! - **Stars** ([`CentreTracer::Stars`]): the cluster's profile, which the old, intermediate,
//!   recent and continuous stars, the white dwarfs and the neutron stars follow. The plan's
//!   "neutron stars on the stellar profile, widened" is built unwidened: a retained neutron star
//!   is heavier than the mean star, which segregation would narrow (a finding).
//! - **Black holes** ([`CentreTracer::BlackHoles`]): Bahcall and Wolf's (1976) 7⁄4 inside a break
//!   at half the stars' (5 ly), 3.5 outside ([`black_hole_shape`]).
//! - **The young disc** ([`CentreTracer::YoungDisc`], [`young_disc_shape`]): n ∝ r⁻³, the disc
//!   plane's Σ ∝ R⁻² for a thickness in proportion to radius (Paumard et al. 2006; Bartko et al.
//!   2009's r^−1.95±0.25; Lu et al. 2009), with a sharp inner edge at 0.1 ly (0.8–1″, Paumard et
//!   al. 2006 and Yelda et al. 2014) as the distribution function's energy cut, and a smooth
//!   break at 0.5 ly to r⁻⁵ (Yelda et al. find no disc beyond 0.42 ly, Paumard et al. no OB stars
//!   beyond 1.6 ly), under the disc's marks ([`OrbitMarks::young_disc`]: k = 25 about the Milky
//!   Way's disc normal, all turned its way).
//! - **The isotropic young** ([`CentreTracer::YoungIsotropic`], [`young_isotropic_shape`]): n ∝
//!   r^−2.1 (Do et al. 2013's Σ ∝ R^−1.14 beyond 1″, as Yelda et al. 2014 quote it; not
//!   re-checked) from the core radius, so that S2-like orbits exist, to a smooth break at 1.6 ly
//!   to r⁻⁵, with no flattening or rotation.

use crate::galaxy::features::cluster::MEAN_BLACK_HOLE_MASS;
use crate::galaxy::features::interior::counts::{
    NEUTRON_STAR_MASS, WHITE_DWARF_PROGENITOR_MAX, white_dwarf_mass,
};
use crate::galaxy::features::interior::retention::{
    FE_H_NODES, MASS_NODES as MASS_NODES_USIZE, PAIR_SYSTEMIC_SIGMA, SPEED_NODES, SPEED_RANGE_KM_S,
    fe_h_nodes, mass_nodes, maxwell_cdf, monotone_cubic, table_row,
};
use crate::galaxy::imf::{MASS_BAND_EDGES, MassBand, MassFunction};
use crate::galaxy::quad::gl_log_panels;
use crate::math;
use crate::stellar::Composition;
use crate::stellar::composition::Z_SOLAR;
use crate::stellar::remnant::StandardKickLaw;
use crate::stellar::sse::turn_off_mass;
use crate::tables::cluster_retention;
use crate::tables::gauss_legendre::{GL4_NODES, GL4_WEIGHTS};
use crate::time::CLOCK_WINDOW_H;
use crate::units::{Dex, HeliumExcess, LightYears, SolarMasses, Years};

use super::marks::OrbitMarks;
use super::profile::{BREAK_SHARPNESS, CentreProfile, SlopeBreak, TracerProfile, TracerShape};

/// The centre's metallicity for the turn-off and the kick law, dex (provisional, ours: the
/// cluster is metal-rich, Schödel et al. 2020; plan 06's formulae clamp Z at 0.03).
pub const CENTRE_FE_H: Dex = Dex::new(0.3);

/// Progenitor-mass nodes of the retention quadrature, even in `ln m` over 8–100 M☉ (as the
/// clusters' `retention`).
const MASS_NODES: u32 = 16;

/// Which distribution function a class lies on (module documentation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CentreTracer {
    /// The cluster's own profile.
    Stars,
    /// The black holes' steeper cusp.
    BlackHoles,
    /// The young clockwise disc's.
    YoungDisc,
    /// The isotropic young stars' cusp.
    YoungIsotropic,
}

impl CentreTracer {
    /// Every tracer, in the order of the centre's distribution functions.
    pub const ALL: [Self; 4] = [
        Self::Stars,
        Self::BlackHoles,
        Self::YoungDisc,
        Self::YoungIsotropic,
    ];

    /// Its index in [`ALL`](Self::ALL).
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Stars => 0,
            Self::BlackHoles => 1,
            Self::YoungDisc => 2,
            Self::YoungIsotropic => 3,
        }
    }

    /// The marks its members pass.
    #[must_use]
    pub fn marks(self) -> OrbitMarks {
        match self {
            Self::Stars | Self::BlackHoles => OrbitMarks::old_stars(),
            Self::YoungDisc => OrbitMarks::young_disc(),
            Self::YoungIsotropic => OrbitMarks::isotropic(),
        }
    }

    /// Its profile: the stars' is `stars`, the others' their own shapes.
    #[must_use]
    pub fn profile(self, stars: &TracerProfile) -> TracerProfile {
        match self {
            Self::Stars => stars.clone(),
            Self::BlackHoles => TracerProfile::new(black_hole_shape()),
            Self::YoungDisc => TracerProfile::with_cut(young_disc_shape(), YOUNG_DISC_INNER_EDGE),
            Self::YoungIsotropic => TracerProfile::new(young_isotropic_shape()),
        }
    }
}

/// What a centre class is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CentreClassKind {
    /// Living stars below the turn-off.
    Living,
    /// White dwarfs, by their progenitors' band.
    WhiteDwarf,
    /// Retained neutron stars.
    NeutronStar,
    /// Retained black holes.
    BlackHole,
}

/// One of the centre's star-formation episodes (module documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AgeComponent {
    /// Its share of the mass formed.
    pub share: f64,
    /// Its youngest age at the epoch, years; negative for members born after it.
    pub youngest: Years,
    /// Its oldest age, years.
    pub oldest: Years,
    /// The distribution function its living stars lie on.
    pub tracer: CentreTracer,
}

/// `H` in years.
fn clock_window_years() -> f64 {
    CLOCK_WINDOW_H.as_julian_years_f64()
}

/// The burst's share of the mass formed, before the shares are divided by their sum: 2.5 × 10⁴ M☉
/// at Milky Way values (module documentation; ruling 144.6).
pub const BURST_SHARE: f64 = 6e-4;

/// The share of the burst on the clockwise disc (ruling 144.6).
pub const BURST_DISC_SHARE: f64 = 1.0 / 3.0;

/// The young disc's inner edge, ly: the distribution function's energy cut (ruling 144.6).
pub const YOUNG_DISC_INNER_EDGE: LightYears = LightYears::new(0.1);

/// The centre's star-formation history (module documentation): old, intermediate, recent,
/// continuous, and the burst on the disc and off it.
#[must_use]
pub fn age_components() -> [AgeComponent; 6] {
    let h = clock_window_years();
    let burst = |share: f64, tracer: CentreTracer| AgeComponent {
        share: BURST_SHARE * share,
        youngest: Years::new(3e6),
        oldest: Years::new(8e6),
        tracer,
    };
    [
        AgeComponent {
            share: 0.80,
            youngest: Years::new(1.0e10),
            oldest: Years::new(1.3e10),
            tracer: CentreTracer::Stars,
        },
        AgeComponent {
            share: 0.15,
            youngest: Years::new(2.5e9),
            oldest: Years::new(3.5e9),
            tracer: CentreTracer::Stars,
        },
        AgeComponent {
            share: 0.03,
            youngest: Years::new(1.5e8),
            oldest: Years::new(5e8),
            tracer: CentreTracer::Stars,
        },
        AgeComponent {
            share: 0.01,
            youngest: Years::new(-h),
            oldest: Years::new(1.5e8),
            tracer: CentreTracer::Stars,
        },
        burst(BURST_DISC_SHARE, CentreTracer::YoungDisc),
        burst(1.0 - BURST_DISC_SHARE, CentreTracer::YoungIsotropic),
    ]
}

/// The black holes' shape: 7⁄4 inside 5 ly, 3.5 outside (module documentation).
///
/// # Panics
///
/// Never: the slopes are constants inside the ranges.
#[must_use]
pub fn black_hole_shape() -> TracerShape {
    TracerShape::new(
        1.75,
        [SlopeBreak {
            radius: 5.0,
            sharpness: BREAK_SHARPNESS,
            rise: 1.75,
        }],
    )
    .expect("7/4 inside and 3.5 outside")
}

/// The young disc's shape: r⁻³ from its inner edge, a smooth break at 0.5 ly to r⁻⁵ (module
/// documentation). Inside the edge, which its distribution function's cut sets, the shape turns
/// to r⁻² about 0.01 ly only so that its mass converges; that part sets nothing but the
/// normalisation the cut replaces. The sharpness of both breaks is 4 (ours).
///
/// # Panics
///
/// Never: the slopes are constants inside the ranges.
#[must_use]
pub fn young_disc_shape() -> TracerShape {
    TracerShape::new(
        2.0,
        [
            SlopeBreak {
                radius: 0.01,
                sharpness: 4.0,
                rise: 1.0,
            },
            SlopeBreak {
                radius: 0.5,
                sharpness: 4.0,
                rise: 2.0,
            },
        ],
    )
    .expect("2, then 3, then 5")
}

/// The isotropic young stars' shape: r^−2.1 to a smooth break at 1.6 ly to r⁻⁵, of sharpness 4
/// (module documentation; the break and its sharpness are ours).
///
/// # Panics
///
/// Never: the slopes are constants inside the ranges.
#[must_use]
pub fn young_isotropic_shape() -> TracerShape {
    TracerShape::new(
        2.1,
        [SlopeBreak {
            radius: 1.6,
            sharpness: 4.0,
            rise: 2.9,
        }],
    )
    .expect("2.1 inside and 5 outside")
}

/// One class of the centre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CentreClass {
    /// What it is.
    pub kind: CentreClassKind,
    /// The band its primaries' initial masses fall in.
    pub band: MassBand,
    /// Its age component, an index into [`age_components`].
    pub component: u8,
    /// The distribution function it lies on.
    pub tracer: CentreTracer,
    /// The range its primaries' initial masses are drawn from, M☉.
    pub initial_mass_range: (SolarMasses, SolarMasses),
    /// The mean present mass of its systems, M☉.
    pub mean_mass: SolarMasses,
    /// Its expected count in the whole cluster, inside the reach and beyond it.
    pub expected: f64,
}

/// The retained shares of the remnants, averaged over the progenitors and the profile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CentreRetention {
    /// The share of neutron stars retained.
    pub neutron_stars: f64,
    /// The share of black holes retained.
    pub black_holes: f64,
}

/// One progenitor node of the retention quadrature.
#[derive(Debug, Clone, Copy)]
struct Progenitor {
    mass: f64,
    /// Primaries per primary formed at this node: `pdf × m × Δ ln m ÷ ∫ pdf`, trapezoid ends.
    weight: f64,
    neutron_stars: f64,
    black_holes: f64,
    kept_neutron_stars: f64,
    kept_black_holes: f64,
}

/// The centre's classes (module documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct CentreClasses {
    classes: Vec<CentreClass>,
    retention: CentreRetention,
    primaries: f64,
    formed: (f64, f64),
}

/// `∫ φ` and `∫ m φ` over `[lo, hi]` of the normalised mass function, by 32-node panels in `ln m`
/// split at its breaks.
fn moments(f: &dyn MassFunction, lo: f64, hi: f64) -> (f64, f64) {
    if hi <= lo {
        return (0.0, 0.0);
    }
    let mut edges = vec![lo];
    edges.extend(f.breaks().iter().copied().filter(|&b| b > lo && b < hi));
    edges.push(hi);
    let norm = f.integral(0.0, MASS_BAND_EDGES[5]);
    (
        gl_log_panels(|m| f.pdf(m), &edges) / norm,
        gl_log_panels(|m| m * f.pdf(m), &edges) / norm,
    )
}

/// An average over the stars' profile of a function of the local escape speed (km/s).
type ProfileAverage<'a> = dyn Fn(&dyn Fn(f64) -> f64) -> f64 + 'a;

/// `∫ w kept(v_esc) ÷ ∫ w` over the stars' profile inside the reach, `w = 4πr³ n★` in `d ln r`,
/// two 16-node panels a decade from 10⁻⁴ ly.
fn profile_average(profile: &CentreProfile, kept: &dyn Fn(f64) -> f64) -> f64 {
    let reach = profile.reach().value();
    let step = math::exp10(0.5);
    let mut edges = vec![1e-4];
    let mut edge = 1e-4;
    while edge * step < reach {
        edge *= step;
        edges.push(edge);
    }
    edges.push(reach);
    let four_pi = 4.0 * core::f64::consts::PI;
    let w = |r: f64| four_pi * r * r * r * profile.stars().density(r);
    gl_log_panels(|r| w(r) * kept(profile.escape_speed(r).value()), &edges)
        / gl_log_panels(w, &edges)
}

/// The retained shares over the progenitor nodes.
fn retention_of(progenitors: &[Progenitor]) -> CentreRetention {
    let total = |f: &dyn Fn(&Progenitor) -> f64| progenitors.iter().map(f).sum::<f64>();
    CentreRetention {
        neutron_stars: total(&|p| p.weight * p.kept_neutron_stars)
            / total(&|p| p.weight * p.neutron_stars),
        black_holes: total(&|p| p.weight * p.kept_black_holes)
            / total(&|p| p.weight * p.black_holes),
    }
}

/// The progenitor nodes from plan 09's retention table (ruling 139.5): at each of its sixteen
/// masses, each kind's share and its retained share at speed `v`, `ordinary(v) + fallback +
/// envelope × M(v, 1 km/s) + low × M(v, 12 km/s)` with `M` the Maxwellian's distribution
/// function, linear in \[Fe/H\] of `z_fit` and monotone cubic in `log v`, held at the table's
/// 500 km/s above it; `average` takes it over the profile.
fn tabulated_progenitors(
    mass_function: &dyn MassFunction,
    composition: &Composition,
    average: &ProfileAverage<'_>,
) -> Vec<Progenitor> {
    let law = StandardKickLaw::default();
    let nodes = fe_h_nodes();
    let x = math::log10(composition.z_fit().value() / Z_SOLAR.value())
        .clamp(nodes[0], nodes[FE_H_NODES - 1]);
    let f = nodes
        .windows(2)
        .position(|w| x <= w[1])
        .unwrap_or(FE_H_NODES - 2);
    let frac = (x - nodes[f]) / (nodes[f + 1] - nodes[f]);
    let (log_lo, log_hi) = (
        math::log10(SPEED_RANGE_KM_S.0),
        math::log10(SPEED_RANGE_KM_S.1),
    );
    let last = f64::from(u32::try_from(SPEED_NODES - 1).expect("32 speed nodes"));
    let (lo, hi) = (math::ln(8.0), math::ln(100.0));
    let step = (hi - lo) / f64::from(MASS_NODES - 1);
    let norm = mass_function.integral(0.0, MASS_BAND_EDGES[5]);
    mass_nodes()
        .iter()
        .enumerate()
        .map(|(k, &m)| {
            let end = k == 0 || k == MASS_NODES_USIZE - 1;
            let weight = mass_function.pdf(m) * m * step / norm * if end { 0.5 } else { 1.0 };
            let kind = |kind: usize| {
                let at = |ff: usize, v: f64| {
                    let row = table_row(k, ff, kind);
                    let [share, fallback, low, envelope] = cluster_retention::SHARES[row];
                    let place = (math::log10(v) - log_lo) / (log_hi - log_lo) * last;
                    let ordinary = monotone_cubic(&cluster_retention::ORDINARY_BELOW[row], place)
                        .clamp(0.0, share);
                    (
                        share,
                        ordinary
                            + fallback
                            + envelope * maxwell_cdf(v, law.params().wd_sigma_km_s)
                            + low * maxwell_cdf(v, PAIR_SYSTEMIC_SIGMA),
                    )
                };
                let share = at(f, 1.0).0 + frac * (at(f + 1, 1.0).0 - at(f, 1.0).0);
                let kept = average(&|v: f64| {
                    let (k0, k1) = (at(f, v).1, at(f + 1, v).1);
                    k0 + frac * (k1 - k0)
                });
                (share, kept)
            };
            let ((ns, kept_ns), (bh, kept_bh)) = (kind(0), kind(1));
            Progenitor {
                mass: m,
                weight,
                neutron_stars: ns,
                black_holes: bh,
                kept_neutron_stars: kept_ns,
                kept_black_holes: kept_bh,
            }
        })
        .collect()
}

impl CentreClasses {
    /// The classes of the cluster of `profile` for primaries on `mass_function` under the default
    /// kick law, whose retention table (ruling 139.5) it reads (module documentation).
    ///
    /// # Panics
    ///
    /// Never: the retention's panel edges are never empty.
    #[must_use]
    #[expect(
        clippy::too_many_lines,
        reason = "the retention quadrature, the ages and the classes in the documented order"
    )]
    pub fn new(profile: &CentreProfile, mass_function: &dyn MassFunction) -> Self {
        let composition = Composition::from_fe_h(CENTRE_FE_H, HeliumExcess::ZERO);
        let progenitors = tabulated_progenitors(mass_function, &composition, &|kept| {
            profile_average(profile, kept)
        });
        let retention = retention_of(&progenitors);
        let formed = (
            progenitors.iter().map(|p| p.weight * p.neutron_stars).sum(),
            progenitors.iter().map(|p| p.weight * p.black_holes).sum(),
        );

        // Per primary formed: (component, kind, band) → (count, mass, highest initial mass).
        let components = age_components();
        let shares: f64 = components.iter().map(|c| c.share).sum();
        let mut raw: Vec<(CentreClass, f64)> = Vec::new();
        let mut add = |class: CentreClass, count: f64, mass: f64| {
            if count <= 0.0 {
                return;
            }
            if let Some((c, m)) = raw.iter_mut().find(|(c, _)| {
                (c.component, c.kind, c.band) == (class.component, class.kind, class.band)
            }) {
                c.expected += count;
                c.initial_mass_range.1 = SolarMasses::new(
                    c.initial_mass_range
                        .1
                        .value()
                        .max(class.initial_mass_range.1.value()),
                );
                c.initial_mass_range.0 = SolarMasses::new(
                    c.initial_mass_range
                        .0
                        .value()
                        .min(class.initial_mass_range.0.value()),
                );
                *m += mass;
            } else {
                raw.push((
                    CentreClass {
                        expected: count,
                        ..class
                    },
                    mass,
                ));
            }
        };
        for (index, component) in (0_u8..).zip(components.iter()) {
            let (a, b) = (component.youngest.value(), component.oldest.value());
            let half = 0.5 * (b - a);
            for (&x, &w) in GL4_NODES.iter().zip(&GL4_WEIGHTS) {
                let age = (a + half + half * x).max(1e4);
                let share = component.share / shares * 0.5 * w;
                let m_to = turn_off_mass(Years::new(age), &composition).value();
                for band in MassBand::ALL {
                    let (lo, hi) = (
                        MASS_BAND_EDGES[band.index()],
                        MASS_BAND_EDGES[band.index() + 1],
                    );
                    let live_hi = hi.min(m_to);
                    let (n, mass) = moments(mass_function, lo, live_hi);
                    let base = CentreClass {
                        kind: CentreClassKind::Living,
                        band,
                        component: index,
                        tracer: component.tracer,
                        initial_mass_range: (SolarMasses::new(lo), SolarMasses::new(live_hi)),
                        mean_mass: SolarMasses::ZERO,
                        expected: 0.0,
                    };
                    add(base, share * n, share * mass);
                    let (wd_lo, wd_hi) = (lo.max(m_to), hi.min(WHITE_DWARF_PROGENITOR_MAX));
                    let (n_wd, m_wd) = moments(mass_function, wd_lo, wd_hi);
                    if n_wd > 0.0 {
                        let each = white_dwarf_mass(m_wd / n_wd);
                        add(
                            CentreClass {
                                kind: CentreClassKind::WhiteDwarf,
                                tracer: CentreTracer::Stars,
                                initial_mass_range: (
                                    SolarMasses::new(wd_lo),
                                    SolarMasses::new(wd_hi),
                                ),
                                ..base
                            },
                            share * n_wd,
                            share * n_wd * each,
                        );
                    }
                }
                let dead: Vec<&Progenitor> = progenitors.iter().filter(|p| p.mass > m_to).collect();
                let ns: f64 = dead.iter().map(|p| p.weight * p.kept_neutron_stars).sum();
                let bh: f64 = dead.iter().map(|p| p.weight * p.kept_black_holes).sum();
                let e = CentreClass {
                    kind: CentreClassKind::NeutronStar,
                    band: MassBand::E,
                    component: index,
                    tracer: CentreTracer::Stars,
                    initial_mass_range: (SolarMasses::new(8.0), SolarMasses::new(100.0)),
                    mean_mass: SolarMasses::ZERO,
                    expected: 0.0,
                };
                add(e, share * ns, share * ns * NEUTRON_STAR_MASS);
                add(
                    CentreClass {
                        kind: CentreClassKind::BlackHole,
                        tracer: CentreTracer::BlackHoles,
                        ..e
                    },
                    share * bh,
                    share * bh * MEAN_BLACK_HOLE_MASS,
                );
            }
        }
        let present: f64 = raw.iter().map(|(_, m)| m).sum();
        let primaries = profile.mass().value() / present;
        let mut classes: Vec<CentreClass> = raw
            .into_iter()
            .map(|(c, m)| CentreClass {
                mean_mass: SolarMasses::new(m / c.expected),
                expected: c.expected * primaries,
                ..c
            })
            .collect();
        classes.sort_by_key(|c| (c.band, c.kind, c.component));
        Self {
            classes,
            retention,
            primaries,
            formed,
        }
    }

    /// Every class, by band, kind and component.
    #[must_use]
    pub fn classes(&self) -> &[CentreClass] {
        &self.classes
    }

    /// The retained shares of the remnants.
    #[must_use]
    pub fn retention(&self) -> CentreRetention {
        self.retention
    }

    /// The neutron stars and the black holes formed per primary formed, retained or not.
    #[must_use]
    pub fn remnants_per_primary(&self) -> (f64, f64) {
        self.formed
    }

    /// The primaries the cluster formed: its present mass over the classes' present mass per
    /// primary.
    #[must_use]
    pub fn primaries_formed(&self) -> f64 {
        self.primaries
    }

    /// The expected systems of every class together.
    #[must_use]
    pub fn systems(&self) -> f64 {
        self.classes.iter().map(|c| c.expected).sum()
    }

    /// The mean present mass per system over every class, M☉: the centre's own mean system mass
    /// (Design note 5).
    #[must_use]
    pub fn mean_system_mass(&self) -> SolarMasses {
        let mass: f64 = self
            .classes
            .iter()
            .map(|c| c.expected * c.mean_mass.value())
            .sum();
        SolarMasses::new(mass / self.systems())
    }

    /// The expected count of `kind`, every band and component together.
    #[must_use]
    pub fn count(&self, kind: CentreClassKind) -> f64 {
        self.classes
            .iter()
            .filter(|c| c.kind == kind)
            .map(|c| c.expected)
            .sum()
    }

    /// The expected systems on `tracer`.
    #[must_use]
    pub fn on_tracer(&self, tracer: CentreTracer) -> f64 {
        self.classes
            .iter()
            .filter(|c| c.tracer == tracer)
            .map(|c| c.expected)
            .sum()
    }
}

/// The progenitor nodes by plan 08's kick quadrature itself, one pass per node at 300 km/s, its
/// seven bin edges' speeds (75–840 km/s) interpolated in `ln v` and held at the ends: the
/// reference the table is checked against.
#[cfg(test)]
fn exact_progenitors(
    mass_function: &dyn MassFunction,
    composition: &Composition,
    average: &ProfileAverage<'_>,
) -> Vec<Progenitor> {
    use crate::galaxy::displaced::SPEED_EDGES;
    use crate::galaxy::displaced::kick_bins::speed_bin_shares_against;
    use crate::stellar::remnant::{KickMode, RemnantKind};
    use crate::units::KilometresPerSecond;
    let reference = KilometresPerSecond::new(300.0);
    let speeds = SPEED_EDGES.map(|e| e * reference.value());
    let kept_at = |v: f64, kept: &[f64; 7]| {
        if v <= speeds[0] {
            return kept[0];
        }
        if v >= speeds[6] {
            return kept[6];
        }
        let k = speeds.partition_point(|&s| s <= v) - 1;
        let t = math::ln(v / speeds[k]) / math::ln(speeds[k + 1] / speeds[k]);
        kept[k] + t * (kept[k + 1] - kept[k])
    };
    let low_kept = speeds.map(|v| maxwell_cdf(v, PAIR_SYSTEMIC_SIGMA));
    let law = StandardKickLaw::default();
    let (lo, hi) = (math::ln(8.0), math::ln(100.0));
    let step = (hi - lo) / f64::from(MASS_NODES - 1);
    let norm = mass_function.integral(0.0, MASS_BAND_EDGES[5]);
    mass_nodes()
        .iter()
        .enumerate()
        .map(|(k, &m)| {
            let end = k == 0 || k == MASS_NODES_USIZE - 1;
            let weight = mass_function.pdf(m) * m * step / norm * if end { 0.5 } else { 1.0 };
            let shares =
                speed_bin_shares_against(&law, SolarMasses::new(m), composition, reference);
            let kept = |kind: RemnantKind| -> [f64; 7] {
                std::array::from_fn(|s| {
                    let mut here = 0.0;
                    for mode in [
                        KickMode::Ordinary,
                        KickMode::FallbackNone,
                        KickMode::WhiteDwarf,
                    ] {
                        here += shares.mode_bins(kind, mode)[..=s].iter().sum::<f64>();
                    }
                    let low: f64 = shares.mode_bins(kind, KickMode::Low).iter().sum();
                    here + low * low_kept[s]
                })
            };
            let (ns_kept, bh_kept) = (kept(RemnantKind::NeutronStar), kept(RemnantKind::BlackHole));
            Progenitor {
                mass: m,
                weight,
                neutron_stars: shares.kind_share(RemnantKind::NeutronStar),
                black_holes: shares.kind_share(RemnantKind::BlackHole),
                kept_neutron_stars: average(&|v| kept_at(v, &ns_kept)),
                kept_black_holes: average(&|v| kept_at(v, &bh_kept)),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::consts::LIGHT_YEARS_PER_PARSEC;
    use crate::galaxy::features::centre::testing::milky_way_centre;

    /// P09.T26: 10⁴–4 × 10⁴ black holes inside the central parsec (Hailey et al. 2018); about a
    /// third of the neutron stars (ruling 144.9: 0.30–0.40) and nine tenths of the black holes
    /// retained, to a third (the brainstorm); the unretained are not generated.
    #[test]
    fn the_centre_keeps_a_third_of_its_neutron_stars_and_most_black_holes() {
        let model = milky_way_centre();
        let classes = model.classes();
        let kept = classes.retention();
        let holes = classes.count(CentreClassKind::BlackHole);
        let in_parsec = holes
            * model
                .distribution(CentreTracer::BlackHoles)
                .fraction_within(LIGHT_YEARS_PER_PARSEC);
        let stars_in_parsec = classes.on_tracer(CentreTracer::Stars)
            * model
                .distribution(CentreTracer::Stars)
                .fraction_within(LIGHT_YEARS_PER_PARSEC);
        let disc_mass: f64 = classes
            .classes()
            .iter()
            .filter(|c| c.tracer == CentreTracer::YoungDisc)
            .map(|c| c.expected * c.mean_mass.value())
            .sum();
        eprintln!(
            "retention: neutron stars {:.4}, black holes {:.4}; black holes {holes:.4e}, \
             {in_parsec:.4e} inside 1 pc (stars there {stars_in_parsec:.4e}); neutron stars \
             {:.4e}; white dwarfs {:.4e}; systems {:.4e}, mean mass {:.4} M☉; young disc \
             {:.4e} systems, {disc_mass:.4e} M☉; primaries formed {:.4e}",
            kept.neutron_stars,
            kept.black_holes,
            classes.count(CentreClassKind::NeutronStar),
            classes.count(CentreClassKind::WhiteDwarf),
            classes.systems(),
            classes.mean_system_mass().value(),
            classes.on_tracer(CentreTracer::YoungDisc),
            classes.primaries_formed(),
        );
        assert!((1e4..=4e4).contains(&in_parsec), "{in_parsec}");
        // Ruling 144.9: about a third under the adopted kick law, 0.30–0.40.
        assert!(
            (0.30..=0.40).contains(&kept.neutron_stars),
            "{}",
            kept.neutron_stars
        );
        assert!(
            (0.6..=1.0).contains(&kept.black_holes),
            "{}",
            kept.black_holes
        );
        // Only the retained are generated.
        let (ns, bh) = classes.remnants_per_primary();
        let formed_ns = ns * classes.primaries_formed();
        let formed_bh = bh * classes.primaries_formed();
        let ns_ratio = classes.count(CentreClassKind::NeutronStar) / formed_ns;
        let bh_ratio = holes / formed_bh;
        // The recent components' youngest progenitors are still alive, a per cent at most.
        for (ratio, share) in [(ns_ratio, kept.neutron_stars), (bh_ratio, kept.black_holes)] {
            assert!(
                (0.99..=1.0 + 1e-12).contains(&(ratio / share)),
                "{ratio} {share}"
            );
        }
    }

    /// The table's retention against plan 08's kick quadrature itself (seven speeds to 840 km/s):
    /// within 0.02 for both kinds, the table's hold at 500 km/s costing the neutron stars about
    /// 0.01 (the module documentation).
    #[test]
    fn the_table_s_retention_matches_the_kick_quadrature() {
        let model = milky_way_centre();
        let imf = crate::galaxy::imf::MassFunctionKind::default().to_mass_function();
        let composition = Composition::from_fe_h(CENTRE_FE_H, HeliumExcess::ZERO);
        let exact = retention_of(&exact_progenitors(imf.as_ref(), &composition, &|kept| {
            profile_average(model.profile(), kept)
        }));
        let table = model.classes().retention();
        eprintln!("exact {exact:?}, table {table:?}");
        assert!((exact.neutron_stars - table.neutron_stars).abs() < 0.02);
        assert!((exact.black_holes - table.black_holes).abs() < 0.02);
    }

    /// Ruling 144.6 and 144.10: Schödel et al.'s (2020) history, 80 / 15 / 3 / 1% of the mass
    /// formed, and the burst's 2.5 × 10⁴ M☉ formed 3–8 Myr ago at Milky Way values (1.4–3.7 ×
    /// 10⁴), a third of its systems on the disc. Its mass above 1 M☉ is printed: Lu et al.'s
    /// (2013) figure counts only that, of a top-heavy mass function.
    #[test]
    fn the_burst_is_the_measured_young_cluster() {
        let model = milky_way_centre();
        let classes = model.classes();
        let components = age_components();
        let shares: Vec<f64> = components.iter().map(|c| c.share).collect();
        assert_eq!(&shares[..4], &[0.80, 0.15, 0.03, 0.01]);
        let imf = crate::galaxy::imf::MassFunctionKind::default().to_mass_function();
        let (_, per_primary) = moments(imf.as_ref(), MASS_BAND_EDGES[0], MASS_BAND_EDGES[5]);
        let (_, above_one) = moments(imf.as_ref(), 1.0, MASS_BAND_EDGES[5]);
        let burst = BURST_SHARE / shares.iter().sum::<f64>();
        let formed = burst * classes.primaries_formed() * per_primary;
        let heavy = burst * classes.primaries_formed() * above_one;
        let systems = |tracer| classes.on_tracer(tracer);
        let (disc, isotropic) = (
            systems(CentreTracer::YoungDisc),
            systems(CentreTracer::YoungIsotropic),
        );
        let present: f64 = classes
            .classes()
            .iter()
            .filter(|c| c.component >= 4)
            .map(|c| c.expected * c.mean_mass.value())
            .sum();
        eprintln!(
            "burst: formed {formed:.4e} M☉ ({heavy:.4e} above 1 M☉), present {present:.4e} M☉; \
             systems {disc:.4e} on the disc and {isotropic:.4e} isotropic"
        );
        assert!((1.4e4..=3.7e4).contains(&formed), "{formed}");
        assert!((disc / (disc + isotropic) - BURST_DISC_SHARE).abs() < 1e-9);
        for c in &components[4..] {
            assert_eq!((c.youngest.value(), c.oldest.value()), (3e6, 8e6));
        }
    }

    #[test]
    fn the_classes_hold_the_cluster_s_mass_mostly_old() {
        let model = milky_way_centre();
        let classes = model.classes();
        let mass = |f: &dyn Fn(&CentreClass) -> bool| -> f64 {
            classes
                .classes()
                .iter()
                .filter(|c| f(c))
                .map(|c| c.expected * c.mean_mass.value())
                .sum()
        };
        let total = mass(&|_| true);
        assert!((total / model.profile().mass().value() - 1.0).abs() < 1e-12);
        let old = mass(&|c| c.component == 0) / total;
        eprintln!("old share of the present mass {old:.4}");
        assert!(old > 0.75, "{old}");
        // No living class lies above its turn-off: band E lives only in the young components.
        for c in classes.classes() {
            if c.kind == CentreClassKind::Living && c.band == MassBand::E {
                assert!(c.component >= 2, "{c:?}");
            }
        }
    }
}
