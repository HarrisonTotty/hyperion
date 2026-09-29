//! The constants of star clusters' dynamics (plan 09, P09.T7 and P09.T9): the black holes' loss,
//! the relaxation time's prefactor, the equipartition exponent and the pulsars' encounter-rate law.
//!
//! **Provisional: scratch values, hand-entered by plan 09** (it runs before plan 15's P15.T8, as
//! plan 09's Consumes allows). They are not a fitted table yet, so no fit header and no
//! [`MANIFEST`](super::MANIFEST) entry: P15.T8.a–c take this module over unchanged first and then
//! fit each constant, each fit a generator-version bump.
//!
//! - [`BH_LOSS_BETA`], [`BH_LOSS_PSI_SLOPE`]: Breen and Heggie (2013, MNRAS 432, 2779) as
//!   parametrised by Antonini and Gieles (2020, MNRAS 492, 2936): black-hole mass is lost at β
//!   cluster masses per relaxation time, which shortens by `1 + ψ₁ f` while black holes remain.
//! - [`BH_CLOCK_FACTOR`]: the black-hole law reads its clock at 2.5 × age ÷ t★, standing for the
//!   cluster's denser past (ruling 126.4); it multiplies the clock of the black-hole law only.
//! - [`BH_RELAXATION_PREFACTOR`]: the half-mass relaxation time `t★ = c √(M r_h³ ÷ G) ÷ (⟨m⟩ ln Λ)`
//!   (Spitzer 1987's 0.138).
//! - [`EQUIPARTITION_EXPONENT`]: η in `q′ = q^η` below the turn-off; 1 is δ = ½ mass segregation
//!   (η = 2δ), the standard multimass King law (plan 09, Design note 9; ruling 139.2). It is not
//!   velocity equipartition: δ = ½ models are not equipartitioned (Gieles and Zocchi 2015, MNRAS
//!   454, 576, §3.2.1), so it agrees with the members' partial-equipartition velocities (ruling
//!   126.7). Peuten et al. 2017 (MNRAS 470, 2736) find δ ≃ 0.5 in N-body models, Hénault-Brunet et
//!   al. 2019 (MNRAS 491, 113) fit 0.44 at 47 Tucanae; P15.T8.b fits η in 0.8–1.0.
//! - [`PULSARS_AT_47_TUC_GAMMA`], [`PULSAR_GAMMA_EXPONENT`], [`PULSAR_CORE_COLLAPSE_CAP`]: about 40
//!   millisecond pulsars at 47 Tucanae's encounter rate, rising as Γ^0.7, capped for
//!   core-collapsed clusters at the count at 47 Tucanae's Γ (the brainstorm, "What is inside a
//!   cluster today"; P15.T8.c's format).

/// β: black-hole mass lost per relaxation time, in cluster masses (Antonini and Gieles 2020).
pub const BH_LOSS_BETA: f64 = 2.8e-3;

/// ψ₁: how fast the relaxation time shortens with the black holes' mass fraction f,
/// `1 + ψ₁ f` (Antonini and Gieles 2020).
pub const BH_LOSS_PSI_SLOPE: f64 = 147.0;

/// The factor on age ÷ t★ in the black-hole law only (ruling 126.4; scratch, P15.T8.a fits it with
/// the three constants above and below).
pub const BH_CLOCK_FACTOR: f64 = 2.5;

/// The prefactor of the half-mass relaxation time, 0.138 (Spitzer 1987, eq. 2-63).
pub const BH_RELAXATION_PREFACTOR: f64 = 0.138;

/// η of the class profiles' `q′ = q^η` below the turn-off: 1, δ = ½ mass segregation (η = 2δ;
/// ruling 139.2).
pub const EQUIPARTITION_EXPONENT: f64 = 1.0;

/// The millisecond pulsars a cluster of 47 Tucanae's encounter rate holds: 40.
pub const PULSARS_AT_47_TUC_GAMMA: f64 = 40.0;

/// The exponent of the pulsars' encounter-rate law, 0.7.
pub const PULSAR_GAMMA_EXPONENT: f64 = 0.7;

/// The most millisecond pulsars a core-collapsed cluster holds: the count at 47 Tucanae's Γ.
pub const PULSAR_CORE_COLLAPSE_CAP: f64 = PULSARS_AT_47_TUC_GAMMA;
