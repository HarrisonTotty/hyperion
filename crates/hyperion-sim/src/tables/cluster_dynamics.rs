//! The constants of star clusters' dynamics (plan 09, P09.T7 and P09.T9): the black holes' loss,
//! the relaxation time's prefactor, the equipartition exponent and the pulsars' encounter-rate law.
//!
//! Three fits of plan 15 write this file, each its own block under its own header (P15.T8.a
//! `cluster_bh`, P15.T8.b `equipartition`, P15.T8.c `pulsars`). Plan 09 committed the constants as
//! scratch values; P15.T8 moved them here unchanged, and each block stays `@provisional`, holding
//! plan 09's values, while its fit misses its acceptance (the block's header gives the fitted
//! values and the misses).
//!
//! - `BH_LOSS_BETA`, `BH_LOSS_PSI_SLOPE`: Breen and Heggie (2013, MNRAS 432, 2779) as
//!   parametrised by Antonini and Gieles (2020, MNRAS 492, 2936): black-hole mass is lost at β
//!   cluster masses per relaxation time, which shortens by `1 + ψ₁ f` while black holes remain.
//! - `BH_CLOCK_FACTOR`: the black-hole law reads its clock at 2.5 × age ÷ t★, standing for the
//!   cluster's denser past (ruling 126.4); it multiplies the clock of the black-hole law only.
//! - `BH_RELAXATION_PREFACTOR`: the half-mass relaxation time `t★ = c √(M r_h³ ÷ G) ÷ (⟨m⟩ ln Λ)`
//!   (Spitzer 1987's 0.138).
//! - `EQUIPARTITION_EXPONENT`: η in `q′ = q^η` below the turn-off; 1 is δ = ½ mass segregation
//!   (η = 2δ), the standard multimass King law (plan 09, Design note 9; ruling 139.2). It is not
//!   velocity equipartition: δ = ½ models are not equipartitioned (Gieles and Zocchi 2015, MNRAS
//!   454, 576, §3.2.1), so it agrees with the members' partial-equipartition velocities (ruling
//!   126.7). Peuten et al. 2017 (MNRAS 470, 2736) find δ ≃ 0.5 in N-body models, Hénault-Brunet et
//!   al. 2019 (MNRAS 491, 113) fit 0.44 at 47 Tucanae.
//! - `PULSARS_AT_47_TUC_GAMMA`, `PULSAR_GAMMA_EXPONENT`, `PULSAR_CORE_COLLAPSE_CAP`: about 40
//!   millisecond pulsars at 47 Tucanae's encounter rate, rising as Γ^0.7, capped for
//!   core-collapsed clusters at the count at 47 Tucanae's Γ (the brainstorm, "What is inside a
//!   cluster today").

// @begin-table cluster_bh
// The black holes' loss from clusters (plan 15, P15.T8.a): Breen and Heggie's (2013) law
// as parametrised by Antonini and Gieles (2020), against the CMC Cluster Catalog.
//
// @provisional by hyperion-fit 0.1.0, task `cluster_bh` revision 0 (P15.T8.a). Do not edit.
// inputs-sha256: 4efee92684d31ad81aac4f53fdfc763c4b5d402a75211fe60edac1bf384906d6
// manifest: crates/hyperion-fit/manifests/cluster_bh.toml
// data: `cmc@cc49168e45c3`
// sim-fingerprint: 5ce8c09fc8aef170dd77d9c55908f7a3290284a190a7dc8977abd18d714ffe11
// since-generator-version: 15
// source: Kremer et al. (2020, ApJS 247, 48), Table A1; Breen and Heggie (2013, MNRAS 432,
//   2779); Antonini and Gieles (2020, MNRAS 492, 2936); Spitzer (1987); Kroupa (2001); the
//   Baumgardt–Hilker catalogue as plan 09's tests hold it
// acceptance: fitted β = 3.8937e-21, ψ₁ = 25949798996716978176.00 (β ψ₁ = 0.1010) at k = 2.5 and
//   c = 0.138 (β and c enter only as β ÷ c) over 124 CMC models at 14 Gyr (1 of them dissolved
//   to under 500 M☉ and left out), 97 retaining any: rms 0.476 dex in log₁₀(1 + N) over those
//   (under 0.3; FAILS; 1.716 at the scratch constants); 19 of 34 predicted empty are empty
//   (FAILS); on the Baumgardt–Hilker catalogue at 12 Gyr, the most in a cluster past 14 t★ 25.86
//   (FAILS), the median over 3 × 10⁵ M☉ 693 (tens to a few hundred; FAILS), ω Centauri 12100
//   (thousands; FAILS), 0.006 past the core-collapse line with none (0.15–0.25; FAILS); the
//   table keeps plan 09's scratch β 0.0028, ψ₁ 147 until a ruling

/// β: black-hole mass lost per relaxation time, in cluster masses (Antonini and Gieles 2020).
pub const BH_LOSS_BETA: f64 = 0.002_8;

/// ψ₁: how fast the relaxation time shortens with the black holes' mass fraction f,
/// `1 + ψ₁ f` (Antonini and Gieles 2020).
pub const BH_LOSS_PSI_SLOPE: f64 = 147.0;

/// The factor on age ÷ t★ in the black-hole law only (ruling 126.4).
pub const BH_CLOCK_FACTOR: f64 = 2.5;

/// The prefactor of the half-mass relaxation time, 0.138 (Spitzer 1987, eq. 2-63).
pub const BH_RELAXATION_PREFACTOR: f64 = 0.138;
// @end-table cluster_bh

// @begin-table equipartition
// The mass-segregation exponent η of the cluster classes' profiles (plan 15, P15.T8.b).
//
// @provisional by hyperion-fit 0.1.0, task `equipartition` revision 0 (P15.T8.b). Do not edit.
// inputs-sha256: f46e73a140d825480a77e18cd79ab37ad3ce3531a665c870ff0184ffb20d0402
// manifest: crates/hyperion-fit/manifests/equipartition.toml
// data: none
// sim-fingerprint: af683cf5f15ed5942cf366d4441bd2ad5073eae14fdccd0e3126dbfcf85219f8
// since-generator-version: 15
// source: multimass lowered-isothermal models (Da Costa and Freeman 1976, ApJ 206, 128; Gunn and
//   Griffin 1979, AJ 84, 752) in the parametrisation of Gieles and Zocchi (2015, MNRAS 454,
//   576), integrated by the task; plan 09's class profiles and counts
// acceptance: η = 0.1675 at δ = 0.5 over 40 multimass King models (W₀ [3.0, 4.0, 5.0, 6.0, 7.0,
//   8.0, 9.0, 10.0], slopes [-1.5, -1.0, -0.5, 0.0, 0.5]; concentrations 0.81–2.08; mass
//   fractions within 8.7e-12): in 0.8–1.0 (ruling 139.2; FAILS); the living classes' half-mass
//   radii below the turn-off within 0.282 (FAILS; 10%); at plan 09's η = 1 within 0.921; the
//   table keeps plan 09's η until a ruling

/// η of the class profiles' `q′ = q^η` below the turn-off: 1 is δ = ½ mass segregation
/// (η = 2δ; ruling 139.2; Gieles and Zocchi 2015, MNRAS 454, 576), which P15.T8.b fits
/// to multimass King models (the block's acceptance).
pub const EQUIPARTITION_EXPONENT: f64 = 1.0;
// @end-table equipartition

// @begin-table pulsars
// The millisecond pulsars' encounter-rate law (plan 15, P15.T8.c), against the census of
// pulsars in globular clusters and Bahramian et al.'s (2013) encounter rates.
//
// @provisional by hyperion-fit 0.1.0, task `pulsars` revision 0 (P15.T8.c). Do not edit.
// inputs-sha256: a93eaf6f23b593829cdae6a4c0bc45d69ae4a52714681b68893ac086cd3e960a
// manifest: crates/hyperion-fit/manifests/pulsars.toml
// data: `bahramian_2013@bdaae66c2c4e`, `gc_pulsars@24351a852f1c`
// sim-fingerprint: none
// since-generator-version: 15
// source: P. C. C. Freire's table of pulsars in globular clusters (retrieved 2026-09-30);
//   Bahramian et al. (2013, ApJ 766, 136); Hessels et al. (2007, ApJ 670, 363) for the
//   completeness's form
// acceptance: fitted over 126 clusters (17 left out for want of a distance; 363 known pulsars):
//   γ = 0.632 (0.5–0.9; passes), A = 38.0 at 47 Tucanae's Γ (30–50; passes), completeness
//   distance 4.8 kpc; the implied total over the catalogue 1539 (2,000–8,000; FAILS); the table
//   keeps plan 09's scratch A 40, γ 0.7 and cap 40 until a ruling

/// The millisecond pulsars a cluster of 47 Tucanae's encounter rate holds.
pub const PULSARS_AT_47_TUC_GAMMA: f64 = 40.0;

/// The exponent of the pulsars' encounter-rate law.
pub const PULSAR_GAMMA_EXPONENT: f64 = 0.7;

/// The most millisecond pulsars a core-collapsed cluster holds: the count at 47 Tucanae's Γ.
pub const PULSAR_CORE_COLLAPSE_CAP: f64 = 40.0;
// @end-table pulsars
