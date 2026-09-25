//! Planet masses: a characteristic mass per group, members correlated about it ("peas in a pod"),
//! and every group held to what its disc can supply (plan 14, P14.T7; design notes 4 and 5;
//! rulings 38, 55, 60 and 68).
//!
//! # The model
//!
//! - **Characteristic mass** (P14.T7.a, as ruling 60 calibrates it). A group of the correlated law
//!   ([`MassLaw::Correlated`]) has one characteristic mass `m_c`, whose median follows how the
//!   group grew ([`is_drift_fed`], [`characteristic_mass`]):
//!   - a *drift-fed* group, a compact chain, a warm giant's companions or a substellar chain:
//!     `m_c` = 7.7 M⊕ × (M★ ÷ M☉) × 10^(`σ_b` z), with `σ_b` = 0.513 dex
//!     ([`BETWEEN_SYSTEM_SCATTER_DEX`]), M★ held at 0.35 M☉ for a star below it
//!     ([`MEDIAN_HOST_MASS_FLOOR`], ruling 94.5). The host sets it; the disc's solids and
//!     metallicity do not. It is the centre of its members' law, not a planet, and is not held to
//!     the template's range (ruling 73.2, below);
//!   - an *in-situ* rocky group: `m_c` = 0.5 M⊕ × (`M_s` ÷ [`REFERENCE_SOLID_MASS`]), as P14.T7.a
//!     wrote it, held to the template's range, `M_s` being the whole solid mass of the host's disc
//!     between its edges, already cut to the host's stable zone (P14.T9).
//! - **Members** (P14.T7.b). Member i of n, counted inside out, has log₁₀ mᵢ normal about
//!   log₁₀ `m_c` + s (i − (n − 1) ÷ 2) with the same scatter `σ_w` for every member, 0.17 dex for
//!   a drift-fed group ([`WITHIN_SYSTEM_SCATTER_DEX`]) and 0.2 for a rocky one
//!   ([`ROCKY_WITHIN_SYSTEM_SCATTER_DEX`]), truncated to the template's range (for a
//!   drift-fed group about a star, truncated at its host-scaled [`mass_floor`] and tapered as
//!   q^−2.9 above its ceiling to a giant's mass, [`taper_limit`], ruling 94.4): s more per step outward,
//!   centred on the group so that `m_c` stays its typical mass whatever its count, s = 0.21 dex for
//!   a drift-fed group ([`OUTWARD_STEP_DEX`]) and 0 for a rocky one ([`ROCKY_OUTWARD_STEP_DEX`]).
//!   The member's own variate εᵢ is taken at its rank Φ(εᵢ) in the truncated law
//!   ([`StandardNormal::truncated`]), so the same words draw it and no mass sits at a bound it did
//!   not draw (ruling 73.2).
//! - **The solid budget** (ruling 38, point 4; rulings 60 and 73.1). A group's members together
//!   hold at most [`solid_budget`], three times its disc's whole solid mass, and a drift-fed
//!   group's at most [`drift_budget`], that times (M★ ÷ M☉)^−0.67 (Mulders, Pascucci and Apai 2015,
//!   ApJ 814, 130). A correlated group whose drawn
//!   members hold more forms only its innermost members that fit, each at its drawn mass, and
//!   none if even the first does not fit ([`GroupCap::Truncated`]), so that no member lies under
//!   its floor (ruling 66: a chain of 1 M⊕ × M★ planets cannot be built from less; Lambrechts et
//!   al. 2019, a poor pebble flux grows no super-Earths). A group drawn from a law is scaled down,
//!   every member by one factor. A starved disc makes fewer or smaller planets, never more mass
//!   than it had.
//! - **Giants** (P14.T7.c). A group whose range reaches giants ([`PlanetGroup::places_giants`])
//!   takes each mass from its template's law, Cumming et al.'s dN ÷ d ln M ∝ M^−0.31, and is held
//!   together to the disc's gas mass ([`gas_budget`]) in the same way. The ice-rich bodies, the
//!   ice giants and the survivor draw from their laws (log-uniform) and are held to the solid
//!   budget. Whether a host can make a giant at all is [`giant_core`]: 10 M⊕ of solids beyond the
//!   snow line, and a core of 10 M⊕ grown there within the disc's lifetime.
//!
//! Each group's limit is its own, so that changing one group's template never moves another
//! group's masses (P14.T7.b's test). A system's groups together can therefore hold more than its
//! budget: a `CompactWithColdGiant` chain may take up to all of it while its giants' cores also
//! need 10 M⊕ of solids.
//!
//! # Why a drift-fed group's mass follows its host (ruling 60)
//!
//! P14.T7.a scaled every `m_c` as the disc's solids, `M_s` ∝ M★ f 10^\[Fe/H\] with the gas
//! fraction f's 0.5 dex of scatter. Placed (P14.T10.b), that put a third of an M dwarf's chain
//! planets under 1 M⊕, halved the small planets at \[Fe/H\] = −0.8, and passed the disc's scatter
//! into the adjacent planets' correlation (0.78 about FGK hosts against Weiss et al.'s 0.65).
//! Three measurements say the masses of close-in planets do not follow their discs' solids:
//!
//! - Pascucci et al. (2018, ApJ 856, L28, §2 and Table 1) find the occurrence of Kepler's planets
//!   inside 100 days a broken power law in the planet-to-star mass ratio, breaking at 2.9, 2.8
//!   and 2.8 × 10⁻⁵ for M, K and G hosts, so that "the most common planetary mass inside the
//!   snowline is not fixed but rather increases linearly with stellar mass, from ∼3.5-4.5 M⊕
//!   around M dwarfs up to ∼8-9 M⊕ around G stars", while dust discs' masses scale more steeply
//!   than linearly (Pascucci et al. 2016, ApJ 831, 125, abstract: `M_dust` ∝ M★^1.3–1.9); their §4
//!   notes that the pebble isolation mass scales linearly with the star's mass.
//! - Zhu (2019, ApJ 873, 8, §2) finds the hosts of 1–2 R⊕ and of 2–4 R⊕ planets with "statistically
//!   the same" metallicities (KS p = 0.75), so a planet's size does not follow its disc's metals;
//!   the occurrence does, weakly (§4: from 25% of stars at \[Fe/H\] ≈ −0.2 to 36% at +0.2), and
//!   P14.T4.b's class weights carry that.
//! - Lambrechts et al. (2019, A&A 627, A83, abstract): where the pebble flux is high, embryos
//!   migrate to the disc's inner edge and form "closely spaced super-Earths in the 5 to 20
//!   Earth-mass range, bounded by the pebble isolation mass"; a poorer disc makes fewer of them,
//!   or a terrestrial system instead. So a disc short of its chain's solids forms fewer planets
//!   here, not lighter ones.
//!
//! An in-situ rocky group keeps P14.T7.a's law: "in the standard scenario of terrestrial planet
//! formation, the average planet mass scales almost linearly with the local surface density of
//! the planetesimal disk" (Pascucci et al. 2018, §4, after Kokubo et al. 2006).
//!
//! # The normalisation (ruling 68.2)
//!
//! The median, 7.7 M⊕ × (M★ ÷ M☉), is the occurrence-weighted one: the peak of the distribution
//! of every close-in planet's mass, not of the planets whose masses have been measured.
//!
//! - Wu (2019, ApJ 874, 91, §3.1 and Table 1) fits the Gaia–Kepler sample's radius distribution,
//!   corrected for detection, with a population whose masses are log-normal about
//!   `M_c` = `M_0` (M★ ÷ M☉), the form used here, and finds `M_0` = 7.70 ± 1.5 M⊕ for an Earth-like
//!   core (her preferred model), the photo-evaporation valley's position across M to F hosts
//!   giving the host-mass slope β ≈ 1 (§4: "a single scaling law, `M_p` ∝ M★^β, with β ≈ 1" from
//!   0.2 to 2 M☉; her eq. 7 allows β of 0.95–1.35) and no dependence on the host's metallicity.
//! - Pascucci et al. (2018, ApJ 856, L28, §3) forward-model Kepler's G hosts with EPOS and find
//!   the break, where the occurrence per log mass ratio peaks, at 7.7 ± 1.7 M⊕, q = (2.5 ± 0.6) ×
//!   10⁻⁵, for the median host of 0.91 M☉; their binned Table 1 gives 2.8–2.9 × 10⁻⁵ for M, K
//!   and G hosts alike, "from ∼3.5-4.5 M⊕ around M dwarfs up to ∼8-9 M⊕ around G stars" (§2).
//!
//! Both put q near 2.5 × 10⁻⁵, 8.3 M⊕ at 1 M☉, and Wu's figure at 1 M☉ is taken. Plan 14's 4 M⊕,
//! which ruling 55.4 kept provisionally, is half of it; the literature's 5–6.5 M⊕ that ruling 55
//! set aside were medians of radial-velocity samples, and of the break-law extended below the
//! surveys' reach. The slope stays 1: both sources find it linear.
//!
//! Wu's width is 0.29 dex for her preferred Earth-like cores; her Table 1 trades it against the
//! cores' density, to 0.54 dex at 2 g cm⁻³. This module's is `σ_b` and `σ_w` together, 0.54 dex
//! (ruling 85.1), split as 0.513 and 0.17 (0.515 and 0.165 before ruling 94.4's taper) to meet Weiss et al.'s pair statistics compared like
//! with like (below). Wu's 0.29 does not meet them (ruling 73.2): split as `σ_b` 0.21 and `σ_w`
//! 0.2 it puts the adjacent log radii's correlation at 0.47, and split as 0.28–0.29 and 0.03–0.07
//! it reaches 0.58–0.61 only with a step that makes the outer planet the larger in 68–70% of pairs
//! and the heavier in 98% of this module's sample, against P14.T7.b's 0.55–0.75. Ruling 73.2's
//! re-fit to 0.74 dex is withdrawn (ruling 85.1): it lies beyond every row of Wu's table.
//!
//! # No mass at a bound it did not draw (ruling 73.2)
//!
//! Held to the template's 1–20 M⊕ by clamping, the 0.54 dex law of ruling 68.2 had put 14% of a
//! Sun's chain planets at exactly the 20 M⊕ ceiling, 7% at its 1 M⊕ floor, and 11% of the chain
//! planets about hosts of 0.2–0.4 M☉ at their host-scaled floor; rocky planets about FGK hosts sat
//! at their 2 M⊕ ceiling in 5% of cases and at their 0.05 M⊕ floor in 10%. Each member is now its
//! law's truncated quantile at its own rank, the distribution of redrawing it until it falls inside
//! the range, from the same words, and none sits at a bound. A drift-fed group's `m_c` is not
//! truncated or held itself: holding it, or truncating its own law to the range, narrowed the
//! scatter between systems that carries Weiss et al.'s correlation (0.52–0.57 against 0.60–0.70),
//! so a group whose `m_c` lies above the ceiling had members crowded just under it. The ceiling is
//! 20 M⊕ × (M★ ÷ M☉) since ruling 85.2 ([`mass_ceiling`]), where 4.8% of the chain planets about
//! hosts of 0.9–1.1 M☉ and 6.6% of those about 0.2–0.4 M☉ lay within 0.02 dex under it.
//!
//! # The taper (ruling 94.4)
//!
//! The hard ceiling excluded real planets: CD Cet b (3.95 M⊕ in m sin i at 0.161 M☉, against a
//! ceiling of 3.2), LP 819-052 b (7.4 at 0.178, against 3.6) and Ross 1020 b (8.0 at 0.272, against
//! 5.4), three of Sabotta et al.'s (2021, Table 1) ten late-M planets at 1–10 days, and it made
//! Kaminski et al.'s (2025, Table 7) 3–10 M⊕ bin about hosts under 0.16 M☉, 0.11 (+0.11 −0.06) at
//! 1–10 days, structurally empty. Above the ceiling a drift-fed member's law now falls as Pascucci
//! et al.'s (2018, Table 1) mass-ratio function does above its break, dN ÷ d log q ∝ q^−2.9
//! ([`MASS_TAPER_INDEX`]), never above the member's own law and continuous with it at the ceiling
//! ([`tapered_log_mass`]), and ends at a giant's
//! mass, so that a chain's planets stay small planets ([`taper_limit`]). The taper starts at the
//! ceiling, not at Pascucci et al.'s break (q ≈ 2.8 × 10⁻⁵, 9.3 M⊕ × M★): here the break is the
//! law's median, Wu's 7.7 M⊕ × M★, which their forward-modelled 7.7 ± 1.7 M⊕ confirms, and the
//! log-normal above it already falls; a wall of slope −2.9 from the break would crowd the 44% of
//! a Sun's members that the 0.54 dex law puts above 9.3 M⊕ into the next 0.15 dex. On
//! P14.T10.b's placed sample, of the drift-fed planets 11.4% about FGK primaries, 16.0% about
//! early M and 22.9% about late M primaries (0.08–0.337 M☉) lie above the old ceiling, and 2.2%,
//! 2.2% and 2.9% within 0.02 dex under it; none about hosts under 1.59 M☉ reaches a giant's mass.
//! The taper let the outer members of chains centred near the ceiling grow past it, so the outer
//! planet became the heavier in 0.752 of P14.T7.b's sample, over its 0.55–0.75; `σ_b` and `σ_w`
//! were re-split, 0.515 and 0.165 to 0.513 and 0.17, the same 0.54 dex, which gives 0.746.
//!
//! # The solid budget (ruling 38, point 4; ruling 60)
//!
//! P14.T7.b capped each mass at 10 times the local isolation mass ([`DiscProfile::isolation_mass`],
//! Lissauer 1987). In plan 14's median solar disc that is 2 × 10⁻³ M⊕ at 0.1 au (the isolation
//! mass there is 2.3 × 10⁻⁴ M⊕), where the compact classes place planets of 1–20 M⊕: planets that
//! close in cannot be made from the solids of their own annulus. Chiang and Laughlin (2013, MNRAS
//! 431, 3444, eq. 4) turn the Kepler planets into a minimum-mass extrasolar nebula of solids,
//! 620 g cm⁻² (a ÷ 0.2 au)^−1.6, "a factor of 5 larger than the solid surface density of the MMSN"
//! (their §2.2); between 0.05 and 0.5 au it holds 12.7 M⊕ for a typical Kepler planet's host,
//! where the median solar disc here has 0.23 M⊕. The solids must be carried inward, as pebbles
//! drifting through the disc (Lambrechts et al. 2019), so the budget is drawn from the whole disc.
//!
//! Ruling 55 set its efficiency at 1, after Mulders et al. (2021, ApJ 920, 66, abstract), who find
//! the solid masses of the surveys' planetary systems about Sun-like stars matching those of Class
//! II discs "only when the planet formation efficiency is below 100%"; they conclude (§5) that
//! "the reservoir from which exoplanets formed was likely larger", as the embedded discs are
//! "significantly more massive". Tychoniec et al. (2020, A&A 640, A19, abstract) measure Class 0
//! discs at a median of 158 M⊕ of dust and Class I at 52 M⊕ (47 and 12 M⊕ as lower limits), "larger
//! by at least a factor of 10 and 3" than Class II discs, and find the observed planets' solids
//! explained "if planet formation starts in Class 0 phase with an efficiency of ∼15%". Plan 14's
//! median solar disc holds 32.2 M⊕, 3.2 times the ∼10 M⊕ of a Sun-like Class II disc (Mulders et
//! al. 2021, §1), which is a Class I disc; so the budget is the Class 0 reservoir, three times the
//! disc ([`SOLID_BUDGET_EFFICIENCY`]), of which the median Sun-like chain (3.5 planets of
//! 7.7 M⊕) takes 28%, twice Tychoniec et al.'s efficiency. At efficiency 1 the budget held M
//! dwarfs to about 1.7 small planets inside 200 days, under Dressing and Charbonneau's window,
//! however steep
//! P14.T4.b's compact exponent; Mulders, Pascucci and Apai (2015, ApJ 814, 130, abstract) find the
//! heavy-element mass of close-in planets rising "roughly inversely with stellar mass from 4 M⊕ in
//! F stars to 5 M⊕ in G and K stars to 7 M⊕ in M stars ... in stark contrast with observed
//! protoplanetary disk masses". It truncates 328 of 2,000 Sun-like chains (this module's tests;
//! 306 under ruling 73's wider law, 333 under ruling 68.2's clamped law, 246 under the 4 M⊕ median).
//!
//! The local isolation mass keeps what it describes: the core a giant starts from beyond the snow
//! line ([`giant_core`]).
//!
//! # The correlation (P14.T7.b)
//!
//! Weiss et al. (2018, AJ 155, 48) find in the California–Kepler Survey's multi-planet systems a
//! Pearson correlation of r = 0.65 between the sizes of adjacent planets (§3; their Fig. 2's
//! caption prints 0.62), 0.53 for planets over 1 R⊕, and the outer planet the larger in 65.4 ±
//! 0.4% of pairs (§5.3), the ratio of the outer to the inner size growing with the pair's
//! difference in temperature, which they suggest photo-evaporation causes. The paper does not say
//! whether its correlation is of radii or of their logarithms; its figures are logarithmic, and
//! plan 14 reads it as log radii.
//!
//! A drift-fed group's mass no longer carries its disc's scatter, so `σ_b` is the whole of the
//! scatter between systems. The width is 0.54 dex (ruling 85.1), and its split between `σ_b` and
//! `σ_w`, 0.513 and 0.17 dex, is fitted with the step kept at ruling 60's 0.21 dex, on
//! P14.T10.b's placed hosts through P14.T11's Chen and Kipping radius with each planet's own
//! quantile, compared like with like with Weiss et al.'s pairs: all pairs' log radii correlate at
//! 0.660 about FGK primaries of drawn \[Fe/H\] (0.60–0.70) and 0.654 about single Suns, and pairs
//! of planets above 1 R⊕ at 0.614 (0.53; ruling 87.1's window 0.45–0.62); in linear radius
//! 0.552 and 0.504. The outer planet is the larger in 0.651 of pairs against their 65.4 ± 2.1%
//! (ruling 87.4), which it meets since ruling 94.4's taper (0.638–0.640 before). In this module's
//! sample of 2,000 chains in solar discs the log radii correlate at 0.636, the log masses at 0.859,
//! and the outer planet is the heavier in 0.746 of pairs and the larger in 0.642.
//! Chen and Kipping's scatter, 0.146 dex in radius above 2.04 M⊕ and independent for each planet
//! (design note 8), is what separates the radius statistics from the mass ones; He, Ford and
//! Ragozzine (2019, MNRAS 490, 4575, §3.7) fit a within-system width of 0.31 ± 0.07 in ln R, 0.135
//! dex, which that scatter alone more than fills.
//!
//! # Draws
//!
//! All on [`tags::PLANET_MASS`], keyed by the system's ID, with the planet's slot in the draw
//! number (design notes 3 and 4): the planet in slot s reads words 8s to 8s + 7
//! ([`MASS_WORDS_PER_SLOT`]). Words 0–1 are its own within-system scatter εᵢ, words 2–3 its
//! group's between-system scatter z, read at the group's first member, and word 4 the rank of the
//! group's mass law; 5–7 are reserved. A planet's masses depend on nothing but its slot, its
//! group's first slot, its place in the group and the group's count, so a placer assigns each
//! member its slot before asking for the masses and never renumbers it, and inserting a group
//! moves no other group's draws.

use crate::id::SystemId;
use crate::math;
use crate::planetary::architecture::template::{
    GroupRole, MassLaw, MassRange, PlanetGroup, ROCKY_MASS_FLOOR,
};
use crate::planetary::architecture::{
    ClassConstraints, DiscCapacity, HostMultiplicity, SUBSTELLAR_LIMIT, ZoneLimit,
};
use crate::planetary::disc::{Disc, DiscProfile};
use crate::planetary::index::{BodyIndex, BodySlot, BodySub};
use crate::planetary::params::SPACING_GIANT_MASS;
use crate::rng::{ObjectKey, Seed, Stream, tags};
use crate::stellar::draws::{StandardNormal, UnitUniform};
use crate::units::consts::METRES_PER_AU;
use crate::units::{EarthMasses, Megayears, Metres, SolarMasses};

/// Words of the [`tags::PLANET_MASS`] stream that one planet slot owns: the planet in slot s reads
/// words 8s to 8s + 7.
///
/// Changing it moves every planet but the first slot's, which is a generator-version change.
pub const MASS_WORDS_PER_SLOT: u64 = 8;

/// The first word of a planet's within-system scatter in its slot's block.
const SCATTER_WORD: u64 = 0;

/// The first word of a group's between-system scatter in its first member's block.
const GROUP_WORD: u64 = 2;

/// The word of a group law's rank in a planet's block.
const RANK_WORD: u64 = 4;

/// The characteristic mass of a drift-fed group about a host of 1 M☉: 7.7 M⊕ (P14.T7.a; ruling
/// 68.2).
///
/// Wu's (2019, ApJ 874, 91, Table 1) `M_0` = 7.70 ± 1.5 M⊕, the peak of the occurrence-weighted
/// log-normal mass distribution of Kepler's close-in planets at 1 M☉, which Pascucci et al.'s
/// (2018, §3) forward-modelled break, 7.7 ± 1.7 M⊕ at 0.91 M☉, confirms; plan 14's 4 M⊕ is half
/// of it (see the [module documentation](self)). It is the median of a compact chain's, a warm
/// giant's companions' and a substellar chain's characteristic mass about a solar-mass host, and
/// scales in proportion to the host's mass
/// (Pascucci et al. 2018; ruling 60).
pub const COMPACT_CHARACTERISTIC_MASS: EarthMasses = EarthMasses::new(7.7);

/// The characteristic mass of a rocky group in the median solar disc: 0.5 M⊕.
///
/// This module's figure, where plan 14 gives none: the median solar disc makes a group like the
/// Solar System's, whose four terrestrial planets hold 1.98 M⊕, 0.5 M⊕ each on average. Kokubo and
/// Genda (2010, ApJ 714, L21, Table 1) grow 3.6 ± 0.8 planets from 2.3 M⊕ of protoplanets
/// between 0.5 and 1.5 au, the largest 1.18 M⊕ and the second 0.72 M⊕, a mean of 0.64 M⊕. It sets
/// how many rocky planets reach Earth's mass (ruling 48, point f). It is not what carried η⊕'s
/// miss (ruling 60): η⊕ counts 0.5–1.5 R⊕, 0.08–2.9 M⊕, and Kokubo and Genda's mean of 0.64 M⊕
/// raises it by 0.005; the rocky groups' reach was the carrier (P14.T5, [`CountLaw::Fill`]).
///
/// [`CountLaw::Fill`]: crate::planetary::architecture::template::CountLaw::Fill
pub const ROCKY_CHARACTERISTIC_MASS: EarthMasses = EarthMasses::new(0.5);

/// The solid mass of the median solar disc, the disc of a 1 M☉ star of \[Fe/H\] = 0 at plan 06's
/// zero-age state with every variate at its median ([`DiscDraws::MEDIAN`]): 32.20 M⊕.
///
/// A rocky group's median characteristic mass is its [`reference_mass`] in a disc of this much
/// solid mass, and scales in proportion to the solid mass (P14.T7.a). A test holds it to the disc
/// model it stands for.
///
/// [`DiscDraws::MEDIAN`]: crate::planetary::disc::DiscDraws::MEDIAN
pub const REFERENCE_SOLID_MASS: EarthMasses = EarthMasses::new(32.20);

/// The between-system scatter `σ_b` of a drift-fed group's characteristic mass, in dex: 0.513
/// (P14.T7.a–b; rulings 60, 85.1 and 94.4; 0.515 before the taper).
///
/// Plan 14 leaves `σ_b` to be chosen, with `σ_w`, so that adjacent planets' log radii correlate at
/// 0.65 (Weiss et al. 2018). A drift-fed group's mass no longer follows its disc, so this is the
/// whole of the scatter between systems, fitted with [`OUTWARD_STEP_DEX`] on P14.T10.b's placed
/// hosts (see the [module documentation](self)): with [`WITHIN_SYSTEM_SCATTER_DEX`] it makes the
/// 0.54 dex of ruling 85.1 (0.5 and 0.2 when members were clamped). It is drawn on words 2–3 of the group's first
/// member's block. A rocky group takes none: its disc's own scatter carries it.
pub const BETWEEN_SYSTEM_SCATTER_DEX: f64 = 0.513;

/// The within-system scatter `σ_w` of a drift-fed group's member about its characteristic mass,
/// in dex: 0.17 (P14.T7.b; rulings 85.1 and 94.4; 0.165 before the taper, 0.2 before its members
/// were truncated to their range).
///
/// Chosen so that the adjacent log radii of compact systems in solar discs, after P14.T11's Chen
/// and Kipping radius, correlate at Weiss et al.'s (2018) 0.65; see the [module
/// documentation](self) for what it gives and what it cannot.
pub const WITHIN_SYSTEM_SCATTER_DEX: f64 = 0.17;

/// The within-system scatter `σ_w` of a rocky group's members, in dex: 0.2 (P14.T7.b; ruling
/// 55.1).
///
/// Ruling 55.1's 0.2 dex, kept for the in-situ groups when the drift-fed groups' scatter was
/// re-fitted on Weiss et al.'s (2018) pairs of Kepler's compact multis (ruling 73.2), which say
/// nothing of terrestrial groups.
pub const ROCKY_WITHIN_SYSTEM_SCATTER_DEX: f64 = 0.2;

/// The within-system scatter of `group`'s members, in dex: [`WITHIN_SYSTEM_SCATTER_DEX`] for a
/// drift-fed group, [`ROCKY_WITHIN_SYSTEM_SCATTER_DEX`] for any other.
#[must_use]
pub const fn within_scatter(group: &PlanetGroup) -> f64 {
    if is_drift_fed(group.role()) {
        WITHIN_SYSTEM_SCATTER_DEX
    } else {
        ROCKY_WITHIN_SYSTEM_SCATTER_DEX
    }
}

/// How much heavier each member of a drift-fed group's law is than the one inside it, in dex:
/// 0.21 (P14.T7.b; rulings 60 and 85.1).
///
/// Plan 14 had 0.1, for Weiss et al.'s (2018) outer planets being the larger in most pairs; 0.21
/// made the outer planet the larger in their 65.4% of pairs on P14.T10.b's placed hosts with
/// clamped members (ruling 55.1 proposed it: "a larger outward step with the scatter re-fitted").
/// With truncated members it gave 0.638, which ruling 85.1 recorded rather than tuned, and with
/// ruling 94.4's taper 0.651, inside the ±2.1% of their 504 pairs (ruling 87.4). It is
/// centred on the group's middle member.
pub const OUTWARD_STEP_DEX: f64 = 0.21;

/// How much heavier each member of a rocky group is than the one inside it, in dex: 0 (ruling 60).
///
/// Weiss et al.'s (2018) step is a property of Kepler's compact multis. An in-situ terrestrial
/// group has none: the Solar System's are Mercury 0.055, Venus 0.815, Earth 1 and Mars 0.107 M⊕,
/// and Kokubo and Genda (2010, ApJ 714, L21, §3.2 and Table 1) grow the largest planet near the
/// middle of their protoplanets' region. Since a rocky group reserves [`ROCKY_MAX_COUNT`]
/// members and places as many as reach its snow line, a step centred on the reserved count would
/// also lighten every group that stops early.
///
/// [`ROCKY_MAX_COUNT`]: crate::planetary::architecture::template::ROCKY_MAX_COUNT
pub const ROCKY_OUTWARD_STEP_DEX: f64 = 0.0;

/// How many times the disc's whole solid mass one group's members may hold: 3 (ruling 38, point
/// 4; ruling 60).
///
/// Planet formation starts in the embedded, Class 0 phase, whose discs hold at least ten times the
/// dust of Class II discs and Class I discs at least three times (Tychoniec et al. 2020, A&A 640,
/// A19, abstract). Plan 14's disc, 32.2 M⊕ of solids about the Sun, is 3.2 times the ∼10 M⊕ of a
/// Sun-like Class II disc (Mulders et al. 2021, ApJ 920, 66, §1), so it stands for a Class I disc
/// and the Class 0 reservoir is three times it. See the [module documentation](self).
pub const SOLID_BUDGET_EFFICIENCY: f64 = 3.0;

/// The mass a giant's core must reach for runaway gas accretion: 10 M⊕ (P14.T7.c).
///
/// Lambrechts and Johansen (2014, A&A 572, A107, §3.2): the mass at which an embryo's envelope
/// can no longer be supported "is standardly identified as the critical core mass, which is
/// typically of the order of 10 ME" (after Mizuno 1980). It is the same figure as the solids a
/// giant needs beyond the snow line,
/// [`GIANT_SOLID_BUDGET`](crate::planetary::architecture::GIANT_SOLID_BUDGET).
pub const CRITICAL_CORE_MASS: EarthMasses = EarthMasses::new(10.0);

/// The core mass of Lambrechts and Johansen's (2014) eq. 35 at their reference: 11 M⊕ at 1 Myr and
/// 10 au, for a dust-to-gas ratio of 0.01, a solar-mass star and a gas surface density of
/// 500 g cm⁻² × (r ÷ 1 au)^−1.
const PEBBLE_CORE_MASS_AT_REFERENCE: EarthMasses = EarthMasses::new(11.0);

/// Lambrechts and Johansen's (2014) reference dust-to-gas ratio, Z₀ = 0.01.
const PEBBLE_REFERENCE_DUST_TO_GAS: f64 = 0.01;

/// Lambrechts and Johansen's (2014, eq. 1) reference gas normalisation β₀, in g cm⁻²:
/// `Σ_g` = β₀ (r ÷ 1 au)^−1 with β₀ = 500 g cm⁻².
const PEBBLE_REFERENCE_GAS_SCALE_G_CM2: f64 = 500.0;

/// The orbital radius of Lambrechts and Johansen's (2014) eq. 35 at its reference, in au.
const PEBBLE_REFERENCE_RADIUS_AU: f64 = 10.0;

/// A surface density in kg m⁻² in g cm⁻².
const G_CM2_PER_KG_M2: f64 = 0.1;

/// The random variates of one planet's mass, as drawn from [`tags::PLANET_MASS`] by
/// [`MassDraws::for_planet`], or given explicitly by a test or a tool.
///
/// The fields are plain variates with no invariant between them, so they are public, as the
/// disc's [`DiscDraws`](crate::planetary::disc::DiscDraws) are.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MassDraws {
    /// The planet's within-system scatter εᵢ about its group's characteristic mass. Words 0–1 of
    /// its slot's block.
    pub scatter: StandardNormal,
    /// Its group's between-system scatter z, read only when the planet is its group's first
    /// member. Words 2–3.
    pub group: StandardNormal,
    /// The rank of its group's mass law, for the groups that draw from a law. Word 4.
    pub rank: UnitUniform,
}

impl MassDraws {
    /// Every variate at its median.
    pub const MEDIAN: Self = Self {
        scatter: StandardNormal::ZERO,
        group: StandardNormal::ZERO,
        rank: UnitUniform::HALF,
    };

    /// The draws of `planet` of `system`, in the universe of `seed`: words 8s onwards of
    /// `system`'s [`tags::PLANET_MASS`] stream, s being the planet's slot (the high byte of its
    /// index).
    ///
    /// # Panics
    ///
    /// In debug builds, if `planet` is not a planet: a primordial or second-generation planet's
    /// own index, not a moon's, a ring's, a belt's or a star's.
    #[must_use]
    pub fn for_planet(seed: Seed, system: SystemId, planet: BodyIndex) -> Self {
        debug_assert!(is_planet(planet), "a mass is a planet's, got {planet:?}");
        let mut stream = Stream::open(seed, tags::PLANET_MASS, ObjectKey::from(system));
        let start = u64::from(planet.get() >> 8) * MASS_WORDS_PER_SLOT;
        stream.seek(start + SCATTER_WORD);
        let scatter = draw_normal(&mut stream);
        stream.seek(start + GROUP_WORD);
        let group = draw_normal(&mut stream);
        stream.seek(start + RANK_WORD);
        let rank = UnitUniform::new(stream.uniform_open())
            .expect("an open uniform lies strictly between 0 and 1");
        Self {
            scatter,
            group,
            rank,
        }
    }
}

/// Whether `index` is a planet's own index.
#[must_use]
fn is_planet(index: BodyIndex) -> bool {
    matches!(
        (index.slot(), index.sub()),
        (
            BodySlot::Planet(_) | BodySlot::SecondGeneration(_),
            BodySub::Primary
        )
    )
}

/// The next two words of `stream` as a standard normal.
#[must_use]
fn draw_normal(stream: &mut Stream) -> StandardNormal {
    StandardNormal::new(stream.standard_normal()).expect("a Box–Muller variate is finite")
}

/// The characteristic mass a correlated group of `group`'s role has about a host of 1 M☉ in the
/// median solar disc, one of [`REFERENCE_SOLID_MASS`]: its median characteristic mass there.
///
/// A drift-fed group ([`is_drift_fed`]), a compact chain, a warm giant's companions or a
/// substellar chain, takes [`COMPACT_CHARACTERISTIC_MASS`], and a rocky group
/// [`ROCKY_CHARACTERISTIC_MASS`]. The other roles draw from their laws in every template; should
/// one take the correlated law, its reference is the geometric middle of its range.
#[must_use]
pub fn reference_mass(group: &PlanetGroup) -> EarthMasses {
    match group.role() {
        GroupRole::Chain | GroupRole::Companions => COMPACT_CHARACTERISTIC_MASS,
        GroupRole::Rocky => ROCKY_CHARACTERISTIC_MASS,
        GroupRole::IceRich | GroupRole::Giant | GroupRole::IceGiant | GroupRole::Survivor => {
            let range = group.masses();
            EarthMasses::new((range.min().value() * range.max().value()).sqrt())
        }
    }
}

/// Whether a group of `role` is drift-fed: grown from solids carried inward to the disc's inner
/// edge up to a mass its host sets (a compact chain, a warm giant's companions, a substellar
/// chain), rather than assembled in situ from the local solids (every other role).
///
/// Lambrechts et al. (2019, A&A 627, A83, abstract): where the pebble flux is high, embryos
/// migrate, "concentrate at the inner edge of the disc", and form "closely spaced super-Earths in
/// the 5 to 20 Earth-mass range, bounded by the pebble isolation mass"; where it is low, a few
/// terrestrial planets grow by collisions in place.
#[must_use]
pub const fn is_drift_fed(role: GroupRole) -> bool {
    match role {
        GroupRole::Chain | GroupRole::Companions => true,
        GroupRole::Rocky
        | GroupRole::IceRich
        | GroupRole::Giant
        | GroupRole::IceGiant
        | GroupRole::Survivor => false,
    }
}

/// The characteristic mass of `group` about the host of `disc`, at the group's between-system
/// variate `between` (P14.T7.a, as ruling 60 calibrates it): the centre of its members' law.
///
/// - A drift-fed group ([`is_drift_fed`]): [`reference_mass`] × (M★ ÷ M☉) × 10^(`σ_b` z), with
///   [`BETWEEN_SYSTEM_SCATTER_DEX`], M★ being the disc's host mass (a pair's total, for a
///   circumbinary disc), held at [`MEDIAN_HOST_MASS_FLOOR`] for a star below it
///   ([`median_host_mass`]). Neither the disc's solids nor its metallicity enter. It is not held to
///   the group's range, which holds the members instead (ruling 73.2): it may lie beyond it.
/// - Any other: [`reference_mass`] × (`M_s` ÷ [`REFERENCE_SOLID_MASS`]), `M_s` being the disc's
///   whole solid mass between its edges, as P14.T7.a wrote it, with no added scatter, held to the
///   group's range as P14.T7.a holds it (no variate enters it).
///
/// See the [module documentation](self) for the sources of each.
#[must_use]
pub fn characteristic_mass(
    group: &PlanetGroup,
    disc: &DiscProfile,
    between: StandardNormal,
) -> EarthMasses {
    if is_drift_fed(group.role()) {
        let median = reference_mass(group) * median_host_mass(disc.host_mass()).value();
        median * math::exp10(BETWEEN_SYSTEM_SCATTER_DEX * between.value())
    } else {
        held_between(
            reference_mass(group) * (disc.solid_mass() / REFERENCE_SOLID_MASS),
            mass_floor(group, disc),
            group.masses().max(),
        )
    }
}

/// The least host mass a drift-fed group's median follows: 0.35 M☉ (ruling 94.5), below which a
/// star's chains keep the median of a 0.35 M☉ host, 2.7 M⊕.
///
/// A calibration, not a law. Wu's (2019, ApJ 874, 91, §2) linear scaling is calibrated on hosts
/// from about 0.65 M☉ up, and Pascucci et al.'s (2018, §2) M bin has a median of 0.42 M☉ with
/// "very few exoplanet candidates around M dwarfs"; below about 0.4 M☉ nothing constrains it,
/// and its extrapolation misses the late M dwarfs' planets. With ruling 94's closer first period
/// and taper, single stars of 0.14–0.337 M☉ (median 0.24, Ribas et al.'s) held 0.400 planets of
/// 1–10 M⊕ in m sin i at 1–10 days against Ribas et al.'s (2023, A&A 670, A139, §5)
/// 0.56 (+0.15 −0.14), under ruling 94.5's 0.42, and hosts under 0.16 M☉ 0.41 of 0.5–3 M⊕
/// against Kaminski et al.'s (2025, Table 7) 0.88 (+0.36 −0.28). Holding the median at 0.35 M☉,
/// as ruling 94.5 directs, gives 0.50 and 0.63, inside both windows (P14.T10.b), and Kaminski et
/// al.'s 3–10 M⊕ bin 0.065 (0.03–0.3). The chains' inward mass step was separated first: the
/// chain planets at 1–10 days are 1–10 M⊕ in m sin i in 31% of cases against 49% of the chains'
/// planets at 1–1,000 days, and the host masses, drawn uniform to Ribas et al.'s median, give
/// 0.23, 0.39 and 0.44 at 0.08–0.16, 0.16–0.24 and 0.24–0.337 M☉ before the hold. A substellar
/// host's chain keeps the linear law, its template's 0.01–2 M⊕ being its hosts' range already.
pub const MEDIAN_HOST_MASS_FLOOR: SolarMasses = SolarMasses::new(0.35);

/// The host mass a drift-fed group's median scales with (ruling 94.5): the host's own, held at
/// [`MEDIAN_HOST_MASS_FLOOR`] for a star below it.
#[must_use]
pub fn median_host_mass(host: SolarMasses) -> SolarMasses {
    if host >= SUBSTELLAR_LIMIT && host < MEDIAN_HOST_MASS_FLOOR {
        MEDIAN_HOST_MASS_FLOOR
    } else {
        host
    }
}

/// The mass above which a member of `group` about the host of `disc` is tapered, or under which
/// it is truncated (rulings 85.2 and 94.4; [`taper_limit`]).
///
/// A drift-fed group about a star scales its template's ceiling with its host, as its floor and
/// its masses do: 20 M⊕ × (M★ ÷ M☉) for a compact chain or a warm giant's companions. Any other
/// group's, and a substellar host's chain (whose 0.01–2 M⊕ is already its hosts'), is the
/// template's.
#[must_use]
pub fn mass_ceiling(group: &PlanetGroup, disc: &DiscProfile) -> EarthMasses {
    let ceiling = group.masses().max();
    if is_drift_fed(group.role()) && disc.host_mass() >= SUBSTELLAR_LIMIT {
        ceiling * disc.host_mass().value()
    } else {
        ceiling
    }
}

/// The least mass a member of `group` about the host of `disc` is held to (ruling 66).
///
/// A drift-fed group's floor follows the same law as its masses, the template's floor times
/// M★ ÷ M☉, held no lower than the smaller of the template's floor and a rocky planet's,
/// [`ROCKY_MASS_FLOOR`]: 1 M⊕ about a Sun, 0.3 M⊕ about a 0.3 M☉ host. A floor fixed at 1 M⊕
/// held 39% of the chain planets about hosts of 0.2–0.4 M☉ at exactly 1 M⊕. It never rises
/// above the template's ceiling, which it would reach only about a host of over 20 M☉. Any other
/// group's is the template's.
///
/// [`ROCKY_MASS_FLOOR`]: crate::planetary::architecture::template::ROCKY_MASS_FLOOR
#[must_use]
pub fn mass_floor(group: &PlanetGroup, disc: &DiscProfile) -> EarthMasses {
    let floor = group.masses().min();
    if is_drift_fed(group.role()) {
        let lowest = floor.value().min(ROCKY_MASS_FLOOR.value());
        let scaled = (floor.value() * disc.host_mass().value()).max(lowest);
        EarthMasses::new(scaled.min(mass_ceiling(group, disc).value()))
    } else {
        floor
    }
}

/// How many 0.1 dex steps member `position` of `count` lies outside the group's middle:
/// position − (count − 1) ÷ 2.
#[must_use]
fn steps_from_middle(position: usize, count: usize) -> f64 {
    let position = u32::try_from(position).expect("no group's members approach 2³² entries");
    let count = u32::try_from(count).expect("no group's members approach 2³² entries");
    (f64::from(position) * 2.0 + 1.0 - f64::from(count)) / 2.0
}

/// How much heavier each member of `group` is than the one inside it, in dex:
/// [`OUTWARD_STEP_DEX`] for a drift-fed group, [`ROCKY_OUTWARD_STEP_DEX`] for any other.
#[must_use]
pub const fn outward_step(group: &PlanetGroup) -> f64 {
    if is_drift_fed(group.role()) {
        OUTWARD_STEP_DEX
    } else {
        ROCKY_OUTWARD_STEP_DEX
    }
}

/// How steeply the occurrence of a drift-fed group's members falls above its ceiling: dN ÷
/// d log q ∝ q^−2.9 (ruling 94.4).
///
/// Pascucci et al. (2018, ApJ 856, L28, §2, eq. 1 and Table 1) fit the occurrence of Kepler's
/// planets inside 100 days against the planet-to-star mass ratio q with a broken power law whose
/// index above the break is −2.7 ± 0.7, −2.9 ± 0.4 and −2.9 ± 0.4 for M, K and G hosts
/// (−1.9 ± 0.2 for F), and "n ∼ −2.9 (q > `q_br`)" for the universal law of hosts under 1 M☉.
pub const MASS_TAPER_INDEX: f64 = 2.9;

/// Where the taper above `group`'s [`mass_ceiling`] ends about the host of `disc`, if its members
/// are tapered rather than truncated at the ceiling (ruling 94.4): for a drift-fed group about a
/// star whose ceiling, ruling 85.2's 20 M⊕ × (M★ ÷ M☉), lies under a giant's mass, at that mass,
/// [`SPACING_GIANT_MASS`] (0.1 Jupiter masses, about 32 M⊕).
///
/// A chain's planets stay small planets: one of a giant's mass would be spaced, gapped and
/// placed by the placer's rules for giants, and counted as formed at the giants' core site
/// (P14.T8). The taper is truncated there, as ruling 85.2 truncated the law at its ceiling, so
/// nothing piles at it; about a Sun it has fallen to 10^(−2.9 × 0.2) ≈ 0.26 of its value at the
/// ceiling, about an M dwarf of 0.3 M☉ to 0.8%. Hosts above about 1.6 M☉, whose ceiling lies
/// above the giant's mass, keep the truncated law, and so does a substellar host's chain (its
/// template's hard 2 M⊕) and every other group (its template's range).
///
/// [`SPACING_GIANT_MASS`]: crate::planetary::params::SPACING_GIANT_MASS
#[must_use]
pub fn taper_limit(group: &PlanetGroup, disc: &DiscProfile) -> Option<EarthMasses> {
    let giant = EarthMasses::from(SPACING_GIANT_MASS);
    (is_drift_fed(group.role())
        && disc.host_mass() >= SUBSTELLAR_LIMIT
        && mass_ceiling(group, disc) < giant)
        .then_some(giant)
}

/// A member's mass about `characteristic`, at its scatter `scatter` and `steps` outside the
/// middle of a group whose members step outward by `step` dex with a scatter of `sigma` dex,
/// drawn above `floor` and under `ceiling`, or tapered above the ceiling to `taper` (P14.T7.b;
/// rulings 66, 73.2 and 94.4): the log-normal law about `characteristic` shifted by the steps,
/// truncated to the range ([`StandardNormal::truncated`]) or tapered ([`tapered_log_mass`]), at
/// the scatter's rank.
#[must_use]
fn member_mass(
    characteristic: EarthMasses,
    scatter: StandardNormal,
    steps: f64,
    (step, sigma): (f64, f64),
    (floor, ceiling, taper): (EarthMasses, EarthMasses, Option<EarthMasses>),
) -> EarthMasses {
    let (low, high) = (math::log10(floor.value()), math::log10(ceiling.value()));
    let mean = math::log10(characteristic.value()) + step * steps;
    let log_mass = match taper {
        Some(limit) => tapered_log_mass(
            scatter,
            mean,
            sigma,
            (low, high, math::log10(limit.value())),
        ),
        None => scatter.truncated(mean, sigma, low, high),
    };
    EarthMasses::new(math::exp10(log_mass))
}

/// The upper tail of the standard normal, Q(x) = 1 − Φ(x), accurate in both tails.
#[must_use]
fn upper_tail(x: f64) -> f64 {
    0.5 * math::erfc(x * core::f64::consts::FRAC_1_SQRT_2)
}

/// The standard normal's density.
#[must_use]
fn density(x: f64) -> f64 {
    math::exp(-0.5 * x * x) / (2.0 * core::f64::consts::PI).sqrt()
}

/// The log₁₀ mass at `scatter`'s rank Φ(z) of a normal law about `mean` of width `sigma` dex,
/// truncated below at `low`, tapered above `high` and truncated at `limit` (ruling 94.4).
///
/// Above `high` the density in x = log₁₀ m is the smaller of the member's own normal law and a
/// wall falling from the law's value at `high` as 10^(−[`MASS_TAPER_INDEX`] (x − `high`)),
/// Pascucci et al.'s mass-ratio function above its break. So the taper never adds mass the
/// member's law would not have: a member centred well under the ceiling keeps its whole law,
/// whose own fall there is already the steeper, and one centred above it falls as the wall does
/// instead of crowding under it, as the truncated law made it.
///
/// In units of σ, with a, b and c the floor, the ceiling and the limit less `mean`, over σ, and
/// λ = 2.9 ln 10 σ, the wall meets the normal again at 2λ − b (solving −t²/2 = −b²/2 − λ(t − b)),
/// and binds only between b and that, so for b ≥ λ the law is the normal truncated to a–c. The
/// law's parts, inside out: the body Φ(b) − Φ(a); the wall (φ(b) ÷ λ)(1 − e^(−λ(e − b))) to
/// e = min(2λ − b, c); the normal's tail Q(e) − Q(c). A rank is inverted in whichever part holds
/// it, from the top down for Q(z) W of the whole W above it. One word, the member's own, as
/// before.
#[must_use]
fn tapered_log_mass(
    scatter: StandardNormal,
    mean: f64,
    sigma: f64,
    (low, high, limit): (f64, f64, f64),
) -> f64 {
    debug_assert!(
        sigma > 0.0 && low <= high && high <= limit,
        "a tapered law needs σ > 0 and floor ≤ ceiling ≤ limit: σ = {sigma}, [{low}, {high}, \
         {limit}]"
    );
    let (lo, hi, top) = (
        (low - mean) / sigma,
        (high - mean) / sigma,
        (limit - mean) / sigma,
    );
    let lambda = MASS_TAPER_INDEX * core::f64::consts::LN_10 * sigma;
    // Where the wall gives way to the normal again, at or beyond the ceiling.
    let rejoin = (2.0 * lambda - hi).max(hi).min(top);
    // The body's weight, taken from the side that keeps its digits.
    let body = if lo >= 0.0 {
        upper_tail(lo) - upper_tail(hi)
    } else if hi <= 0.0 {
        upper_tail(-hi) - upper_tail(-lo)
    } else {
        1.0 - upper_tail(-lo) - upper_tail(hi)
    }
    .max(0.0);
    let wall = density(hi) / lambda;
    let beyond = math::exp(-lambda * (rejoin - hi));
    let walled = -wall * math::exp_m1(-lambda * (rejoin - hi));
    let tail = (upper_tail(rejoin) - upper_tail(top)).max(0.0);
    let whole = body + walled + tail;
    if !(whole > 0.0 && whole.is_finite()) {
        // Unreachable for any drawn centre: the ceiling would need to lie 38 σ from it.
        return mean.clamp(low, high);
    }
    let variate = scatter.value();
    let (below, above) = (upper_tail(-variate) * whole, upper_tail(variate) * whole);
    let t = if above < tail {
        // In the normal's tail beyond the wall: Q(t) = Q(c) + above.
        let q = upper_tail(top) + above;
        if q > 0.0 {
            -math::normal_quantile(q)
        } else {
            top
        }
        .max(rejoin)
    } else if above < tail + walled {
        // On the wall: (φ(b) ÷ λ)(e^(−λ(t − b)) − e^(−λ(e − b))) = above − tail.
        (hi - math::ln((above - tail) / wall + beyond) / lambda).clamp(hi, rejoin)
    } else if lo < 0.0 {
        // In the body, `below` of the whole lies under the rank: Φ(t) = Φ(a) + below.
        let p = upper_tail(-lo) + below;
        if p > 0.0 {
            math::normal_quantile(p)
        } else {
            lo
        }
        .min(hi)
    } else {
        // Both ends above the centre: Q(t) = Q(a) − below.
        let q = upper_tail(lo) - below;
        if q > 0.0 {
            -math::normal_quantile(q)
        } else {
            hi
        }
        .min(hi)
    };
    (mean + sigma * t).clamp(low, limit)
}

/// The mass at `rank` of a group law over `range`: the inverse of its cumulative distribution
/// (P14.T7.c for giants).
///
/// dN ÷ d ln M ∝ M^α gives M = (lo^α + u (hi^α − lo^α))^(1 ÷ α), and α = 0 or a log-uniform law
/// M = lo (hi ÷ lo)^u. The correlated law has no rank; it is not a law of one planet.
///
/// # Panics
///
/// If `range`'s law is [`MassLaw::Correlated`], in every build: a bug in the caller.
#[must_use]
fn law_mass(range: MassRange, rank: UnitUniform) -> EarthMasses {
    let (lo, hi, u) = (range.min().value(), range.max().value(), rank.value());
    let log_uniform = || math::exp(math::ln(lo) + u * (math::ln(hi) - math::ln(lo)));
    let mass = match range.law() {
        MassLaw::PowerLaw { index } if index.abs() > 1e-12 => {
            let (low, high) = (math::powf(lo, index), math::powf(hi, index));
            math::powf(low + u * (high - low), 1.0 / index)
        }
        MassLaw::PowerLaw { .. } | MassLaw::LogUniform => log_uniform(),
        MassLaw::Correlated => panic!("the correlated law draws a group, not one planet"),
    };
    held(EarthMasses::new(mass), range)
}

/// `mass` held to `range`.
#[must_use]
fn held(mass: EarthMasses, range: MassRange) -> EarthMasses {
    held_between(mass, range.min(), range.max())
}

/// `mass` held to `floor`–`ceiling`.
#[must_use]
fn held_between(mass: EarthMasses, floor: EarthMasses, ceiling: EarthMasses) -> EarthMasses {
    EarthMasses::new(mass.value().clamp(floor.value(), ceiling.value()))
}

/// The most that one group's members may hold of `disc`'s solids: [`SOLID_BUDGET_EFFICIENCY`]
/// times its whole solid mass between its edges (ruling 38, point 4; ruling 60).
#[must_use]
pub fn solid_budget(disc: &DiscProfile) -> EarthMasses {
    disc.solid_mass() * SOLID_BUDGET_EFFICIENCY
}

/// The host masses [`drift_budget`]'s factor is held to: 0.42–1.08 M☉, the medians of Mulders,
/// Pascucci and Apai's (2015, ApJ 814, 130, Table 2) M and F bins (ruling 85.3).
pub const DRIFT_BUDGET_HOST_RANGE: (SolarMasses, SolarMasses) =
    (SolarMasses::new(0.42), SolarMasses::new(1.08));

/// The exponent of [`drift_budget`]'s factor in the host's mass: −0.67 (ruling 85.3).
///
/// The least-squares slope of log₁₀ of Mulders, Pascucci and Apai's (2015, ApJ 814, 130, Table 2)
/// heavy-element masses, 7.3, 5.4, 5.0 and 3.6 M⊕, against log₁₀ of their bins' median host masses,
/// 0.42, 0.73, 0.91 and 1.08 M☉: −0.6705, unweighted.
pub const DRIFT_BUDGET_EXPONENT: f64 = -0.67;

/// The most that one drift-fed group's members may hold of `disc`'s solids: [`solid_budget`]
/// times (M★ ÷ M☉)^[`DRIFT_BUDGET_EXPONENT`] (rulings 73.1 and 85.3), M★ being the disc's host
/// mass (a pair's total, for a circumbinary disc) held to [`DRIFT_BUDGET_HOST_RANGE`].
///
/// Mulders, Pascucci and Apai (2015, ApJ 814, 130, §3.3 and Table 2) find the heavy-element mass
/// in Kepler's planets inside 150 days, where the survey is complete for every spectral type,
/// rising "roughly inversely with stellar mass": 3.6 ± 0.1 M⊕ about F stars (median 1.08 M☉),
/// 5.0 ± 0.1 about G (0.91), 5.4 ± 0.2 about K (0.73) and 7.3 ± 0.7 about M dwarfs (0.42), "in
/// stark contrast" with discs' dust masses, which fall with the star's, and conclude that inward
/// drift of planetary building blocks is "more efficient for lower mass stars" (abstract). A
/// budget in proportion to the disc, as a rocky group's is, bound 32% of the chains about M dwarfs
/// of 0.35–0.6 M☉ against 23% about FGK stars. The factor follows their inventory's own slope,
/// held at the ends of their bins, since below 0.42 M☉ nothing measures it; the solid budget itself
/// still falls with the disc there. With ruling 85.4's systems it binds 22% of the early M dwarfs'
/// chains and puts their inventory at 7.6 M⊕, Mulders et al.'s 7.3 ± 0.7 (0.7–150 days,
/// 0.5–16 R⊕). At 1 M☉ it is [`solid_budget`] itself, so no Sun-like anchor moves.
///
/// # Examples
///
/// ```
/// use hyperion_sim::planetary::disc::{self, DiscDraws, DiscHost, Truncation};
/// use hyperion_sim::planetary::placement::masses::{drift_budget, solid_budget};
/// use hyperion_sim::units::{Dex, Megayears, SolarLuminosities, SolarMasses, SolarRadii};
///
/// let host = DiscHost::new(
///     SolarMasses::new(0.4),
///     Dex::new(0.0),
///     SolarLuminosities::new(0.025),
///     SolarRadii::new(0.37),
/// )?;
/// let disc = disc::derive(&host, Megayears::new(3.0), &DiscDraws::MEDIAN, Truncation::NONE);
/// let profile = disc.profile().expect("an untruncated disc");
/// // Held at 0.42 M☉, the lowest of Mulders et al.'s bins: 0.42^−0.67 = 1.79.
/// let factor = hyperion_sim::math::powf(0.42, -0.67);
/// assert!((drift_budget(profile) / solid_budget(profile) - factor).abs() < 1e-12);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn drift_budget(disc: &DiscProfile) -> EarthMasses {
    let (low, high) = DRIFT_BUDGET_HOST_RANGE;
    let m = disc.host_mass().value().clamp(low.value(), high.value());
    solid_budget(disc) * math::powf(m, DRIFT_BUDGET_EXPONENT)
}

/// The most that `group`'s members together may hold of `disc` ([`group_masses_from`]): the gas
/// mass for a group that places giants ([`gas_budget`]), [`drift_budget`] for a drift-fed group
/// ([`is_drift_fed`]) and [`solid_budget`] for any other.
#[must_use]
pub fn group_budget(group: &PlanetGroup, disc: &DiscProfile) -> EarthMasses {
    if group.places_giants() {
        gas_budget(disc)
    } else if is_drift_fed(group.role()) {
        drift_budget(disc)
    } else {
        solid_budget(disc)
    }
}

/// The most that one group of giants may hold: `disc`'s gas mass between its edges (P14.T7.c).
#[must_use]
pub fn gas_budget(disc: &DiscProfile) -> EarthMasses {
    EarthMasses::from(disc.gas_mass())
}

/// Which limit, if any, a group's masses were held to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GroupCap {
    /// The masses as drawn fit inside the group's limit.
    AsDrawn,
    /// The drawn masses of a group drawn from a law held more than [`solid_budget`], and were
    /// scaled down to it.
    SolidBudget,
    /// The drawn masses of a correlated group held more than its [`group_budget`], so only its
    /// innermost members that fit formed, each at its drawn mass; where even the first did not
    /// fit, none formed (ruling 66).
    Truncated,
    /// The drawn giants held more than [`gas_budget`], and were scaled down to it.
    GasMass,
}

/// The masses of one group's members, inside out, and what held them ([`group_masses`]).
#[derive(Debug, Clone, PartialEq)]
pub struct GroupMasses {
    masses: Vec<EarthMasses>,
    characteristic: Option<EarthMasses>,
    cap: GroupCap,
}

impl GroupMasses {
    /// The masses of the members that formed, in the order of the members given: every member,
    /// unless the group was [`Truncated`](GroupCap::Truncated), when they are its innermost.
    #[must_use]
    pub fn masses(&self) -> &[EarthMasses] {
        &self.masses
    }

    /// The group's characteristic mass, for a group of the correlated law with at least one
    /// member; `None` otherwise.
    #[must_use]
    pub const fn characteristic(&self) -> Option<EarthMasses> {
        self.characteristic
    }

    /// Which limit held the masses.
    #[must_use]
    pub const fn cap(&self) -> GroupCap {
        self.cap
    }

    /// The members' total mass, summed inside out.
    #[must_use]
    pub fn total(&self) -> EarthMasses {
        total(&self.masses)
    }
}

/// The sum of `masses` in their order.
#[must_use]
fn total(masses: &[EarthMasses]) -> EarthMasses {
    masses
        .iter()
        .fold(EarthMasses::ZERO, |sum, &mass| sum + mass)
}

/// How many of `masses`, from the first, fit inside `limit` together, summed in their order.
#[must_use]
fn formed_within(masses: &[EarthMasses], limit: EarthMasses) -> usize {
    let mut sum = EarthMasses::ZERO;
    let mut formed = 0;
    for &mass in masses {
        sum = sum + mass;
        if sum > limit {
            break;
        }
        formed += 1;
    }
    formed
}

/// The masses of `group`'s members in `disc`, from each member's variates `draws`, inside out
/// (P14.T7): the pure function behind [`group_masses`].
///
/// Member i of the group is `draws[i]`, the group's count is `draws.len()`, and the group's
/// between-system variate is the first member's. A correlated group scatters its members about
/// [`characteristic_mass`]; any other draws each mass from its law at the member's rank. The
/// members are then held together to their [`group_budget`]: a correlated group over it forms its
/// innermost members that fit ([`GroupCap::Truncated`]), any other is scaled down by one factor for
/// every member.
///
/// # Examples
///
/// Every variate at its median shows the law's shape: a chain of five about a Sun centred on its
/// characteristic mass, each planet's law 0.21 dex heavier than the one inside it, and the
/// outermost, whose law is centred above the range's 20 M⊕, drawn inside it at its law's
/// truncated median; and in a disc with a tenth of the metals, the same chain's two inner planets,
/// all that its budget can build.
///
/// ```
/// use hyperion_sim::planetary::architecture::ArchitectureClass;
/// use hyperion_sim::planetary::architecture::template::template;
/// use hyperion_sim::planetary::disc::{self, DiscDraws, DiscHost, Truncation};
/// use hyperion_sim::planetary::placement::masses::{
///     GroupCap, MassDraws, group_masses_from, solid_budget,
/// };
/// use hyperion_sim::units::{Dex, Megayears, SolarLuminosities, SolarMasses, SolarRadii};
///
/// let disc_at = |fe_h: f64| -> Result<_, Box<dyn std::error::Error>> {
///     let host = DiscHost::new(
///         SolarMasses::new(1.0),
///         Dex::new(fe_h),
///         SolarLuminosities::new(0.70),
///         SolarRadii::new(0.89),
///     )?;
///     let disc = disc::derive(&host, Megayears::new(2.0), &DiscDraws::MEDIAN, Truncation::NONE);
///     Ok(*disc.profile().expect("an untruncated disc"))
/// };
/// let chain = &template(ArchitectureClass::CompactMulti).groups()[0];
/// let draws = [MassDraws::MEDIAN; 5];
///
/// let solar = group_masses_from(chain, &disc_at(0.0)?, &draws);
/// let m = solar.masses();
/// assert_eq!(solar.cap(), GroupCap::AsDrawn);
/// assert!((7.0..7.7).contains(&m[2].value()));
/// assert!(m[0] < m[1] && m[3] < m[4] && m[4].value() < 20.0);
///
/// let poor = disc_at(-1.0)?;
/// let held = group_masses_from(chain, &poor, &draws);
/// assert_eq!(held.cap(), GroupCap::Truncated);
/// assert_eq!(held.masses(), &m[..2]);
/// assert!(held.total() <= solid_budget(&poor));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn group_masses_from(
    group: &PlanetGroup,
    disc: &DiscProfile,
    draws: &[MassDraws],
) -> GroupMasses {
    let range = group.masses();
    let (mut masses, characteristic): (Vec<EarthMasses>, _) = match range.law() {
        MassLaw::Correlated => match draws.first() {
            None => (Vec::new(), None),
            Some(first) => {
                let characteristic = characteristic_mass(group, disc, first.group);
                let (step, sigma) = (outward_step(group), within_scatter(group));
                let limits = (
                    mass_floor(group, disc),
                    mass_ceiling(group, disc),
                    taper_limit(group, disc),
                );
                let masses = draws
                    .iter()
                    .enumerate()
                    .map(|(i, d)| {
                        let steps = steps_from_middle(i, draws.len());
                        member_mass(characteristic, d.scatter, steps, (step, sigma), limits)
                    })
                    .collect();
                (masses, Some(characteristic))
            }
        },
        MassLaw::PowerLaw { .. } | MassLaw::LogUniform => (
            draws.iter().map(|d| law_mass(range, d.rank)).collect(),
            None,
        ),
    };
    let limit = group_budget(group, disc);
    let reached = if group.places_giants() {
        GroupCap::GasMass
    } else {
        GroupCap::SolidBudget
    };
    let unscaled = total(&masses);
    let cap = if unscaled <= limit {
        GroupCap::AsDrawn
    } else if matches!(range.law(), MassLaw::Correlated) {
        masses.truncate(formed_within(&masses, limit));
        GroupCap::Truncated
    } else {
        let scale = limit / unscaled;
        for mass in &mut masses {
            *mass = *mass * scale;
        }
        reached
    };
    GroupMasses {
        masses,
        characteristic,
        cap,
    }
}

/// The masses of `group`'s members `members`, inside out, about a host whose disc is `disc`, in
/// system `system` of the universe of `seed` (P14.T7).
///
/// `members` are the planets' indices, in the order the placer (P14.T8) sets them down from the
/// host outward, for the count it drew; a placer that stops early keeps the masses of those it
/// placed. Each member's variates are its own slot's ([`MassDraws::for_planet`]), so the masses
/// of one group never depend on another's. See [`group_masses_from`] for the laws and the limits.
///
/// # Panics
///
/// In debug builds, if a member is not a planet's own index, or two members share a slot.
///
/// # Examples
///
/// A compact chain of four planets about a Sun-like star: similar masses about a characteristic
/// mass drawn about 7.7 M⊕, and never more than the disc's budget.
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::id::SystemId;
/// use hyperion_sim::planetary::architecture::ArchitectureClass;
/// use hyperion_sim::planetary::architecture::template::template;
/// use hyperion_sim::planetary::disc::{self, DiscDraws, DiscHost, Truncation};
/// use hyperion_sim::planetary::placement::masses::{
///     BETWEEN_SYSTEM_SCATTER_DEX, MassDraws, group_masses, solid_budget,
/// };
/// use hyperion_sim::planetary::{BodyIndex, BodySlot, BodySub};
/// use hyperion_sim::stellar::Composition;
/// use hyperion_sim::stellar::sse::{ZCoeffs, zams};
/// use hyperion_sim::units::{Megayears, SolarMasses};
///
/// let (seed, system) = (Seed::new(7), SystemId::from_raw(0x0200_0800_2000_0000)?);
/// let (mass, composition) = (SolarMasses::new(1.0), Composition::SOLAR);
/// let coeffs = ZCoeffs::new(composition.z_fit());
/// let host = DiscHost::new(
///     mass,
///     composition.fe_h(),
///     zams::luminosity(mass, &coeffs),
///     zams::radius(mass, &coeffs),
/// )?;
/// let disc = disc::derive(&host, Megayears::new(2.0), &DiscDraws::MEDIAN, Truncation::NONE);
/// let profile = disc.profile().expect("an untruncated disc");
///
/// let chain = &template(ArchitectureClass::CompactMulti).groups()[0];
/// let members = (1..=4)
///     .map(|n| BodyIndex::new(BodySlot::Planet(n), BodySub::Primary))
///     .collect::<Result<Vec<_>, _>>()?;
/// let group = group_masses(seed, system, chain, profile, &members);
/// assert_eq!(group.masses().len(), 4);
/// // A Sun's chains are 7.7 M⊕ times the group's own between-system scatter.
/// let z = MassDraws::for_planet(seed, system, members[0]).group.value();
/// let typical = group.characteristic().expect("a correlated group").value();
/// let expected = 7.7 * hyperion_sim::math::exp10(BETWEEN_SYSTEM_SCATTER_DEX * z);
/// assert!((typical / expected - 1.0).abs() < 1e-9);
/// // Every member is drawn above the chain's 1 M⊕ floor and under a giant's mass, about 32 M⊕,
/// // wherever its characteristic mass lies: the law tapers above 20 M⊕ (ruling 94.4).
/// assert!(group.masses().iter().all(|m| (1.0..31.8).contains(&m.value())));
/// assert!(group.total() <= solid_budget(profile));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn group_masses(
    seed: Seed,
    system: SystemId,
    group: &PlanetGroup,
    disc: &DiscProfile,
    members: &[BodyIndex],
) -> GroupMasses {
    debug_assert!(
        members
            .iter()
            .enumerate()
            .all(|(i, a)| members[..i].iter().all(|b| a.slot() != b.slot())),
        "a group's members are in distinct slots, got {members:?}"
    );
    let draws: Vec<MassDraws> = members
        .iter()
        .map(|&member| MassDraws::for_planet(seed, system, member))
        .collect();
    group_masses_from(group, disc, &draws)
}

/// Whether a host's disc can make a giant, and if so by what age its core reaches
/// [`CRITICAL_CORE_MASS`] (P14.T7.c; design note 5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GiantCore {
    /// There is no disc.
    NoDisc,
    /// The snow line lies beyond the disc's reach, or there are under 10 M⊕ of solids between
    /// them: D5's condition, as P14.T4.c applies it
    /// ([`ClassConstraints::capacity`] is [`DiscCapacity::SmallPlanets`]).
    TooFewSolids,
    /// The core would reach 10 M⊕ only at age `by`, after the disc has gone.
    TooSlow {
        /// The time from the disc's formation at which the core would reach the critical mass,
        /// beyond the disc's lifetime; infinite where no pebbles reach the core's site.
        by: Megayears,
    },
    /// The core reaches 10 M⊕ at age `by`, within the disc's lifetime.
    Forms {
        /// The time from the disc's formation at which the core reaches the critical mass, from
        /// zero (a seed already that heavy) to the disc's lifetime.
        by: Megayears,
    },
}

impl GiantCore {
    /// Whether the disc can make a giant.
    #[must_use]
    pub const fn forms(&self) -> bool {
        match self {
            Self::Forms { .. } => true,
            Self::NoDisc | Self::TooFewSolids | Self::TooSlow { .. } => false,
        }
    }
}

/// Whether `disc`, in a stable zone ending at `zone`, can make a giant (P14.T7.c): 10 M⊕ of solids
/// beyond the snow line, and a core of [`CRITICAL_CORE_MASS`] grown there within the disc's
/// lifetime.
///
/// The solids are P14.T4.c's condition ([`ClassConstraints::new`]), unchanged. The core starts as
/// the local isolation mass at the snow line, or at the inner edge where that lies beyond it
/// ([`DiscProfile::isolation_mass`], Lissauer 1987): ruling 38 keeps the isolation mass for this,
/// the core a giant starts from. In plan 14's median solar disc it is 0.07 M⊕ there, and
/// planetesimals alone grow no 10 M⊕ core anywhere in the disc (0.2 M⊕ at 5 au). The core then
/// grows by pebble accretion, Lambrechts and Johansen's (2014, A&A 572, A107) eq. 35,
///
/// `M_c`(t)^⅓ = `M_c,0`^⅓ + [11 M⊕ (Z ÷ 0.01)^(25/6) (M★ ÷ M☉)^(−11/12) (β ÷ 500 g cm⁻²)³
/// (r ÷ 10 au)^(−5/4)]^⅓ (t ÷ 1 Myr)^(13/18),
///
/// with the disc's own dust-to-gas ratio Z and gas surface density `Σ_g` = β (r ÷ 1 au)^−1 at the
/// site. Their disc is untapered and their model runs from 5 to 100 au, so applying it at a snow
/// line of 2–3 au in a disc with a 30 au taper is an extrapolation; there the growth is fastest.
/// It reproduces their summary, a core "within about 1 Myr at 5 AU", in the median solar disc
/// (0.50 Myr; 0.32 Myr at its snow line).
///
/// # Panics
///
/// In debug builds, if the zone's end is not positive.
///
/// # Examples
///
/// A Sun-like star's median disc grows a giant's core in a third of a Myr, unless a companion
/// cuts its disc inside the snow line, or the disc is gone before the core is grown.
///
/// ```
/// use hyperion_sim::planetary::architecture::ZoneLimit;
/// use hyperion_sim::planetary::disc::{self, DiscDraws, DiscHost, Truncation};
/// use hyperion_sim::planetary::placement::masses::{GiantCore, giant_core};
/// use hyperion_sim::stellar::draws::StandardNormal;
/// use hyperion_sim::units::{
///     AstronomicalUnits, Dex, Megayears, Metres, SolarLuminosities, SolarMasses, SolarRadii,
/// };
///
/// let sun = DiscHost::new(
///     SolarMasses::new(1.0),
///     Dex::new(0.0),
///     SolarLuminosities::new(0.70),
///     SolarRadii::new(0.89),
/// )?;
/// let median = disc::derive(&sun, Megayears::new(1.7), &DiscDraws::MEDIAN, Truncation::NONE);
/// let GiantCore::Forms { by } = giant_core(&median, ZoneLimit::Unbounded) else {
///     panic!("the median solar disc grows a core");
/// };
/// assert!((0.3..0.35).contains(&by.value()));
///
/// let two_au = Metres::from(AstronomicalUnits::new(2.0));
/// let cut = giant_core(&median, ZoneLimit::Outer(two_au));
/// assert_eq!(cut, GiantCore::TooFewSolids);
///
/// // A lighter disc that lasts only 0.3 Myr has the solids, but not the time.
/// let light = DiscDraws {
///     gas_fraction: StandardNormal::new(-0.5).expect("finite"),
///     ..DiscDraws::MEDIAN
/// };
/// let brief = disc::derive(&sun, Megayears::new(0.3), &light, Truncation::NONE);
/// assert!(matches!(giant_core(&brief, ZoneLimit::Unbounded), GiantCore::TooSlow { .. }));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn giant_core(disc: &Disc, zone: ZoneLimit) -> GiantCore {
    let capacity = ClassConstraints::new(disc, zone, HostMultiplicity::SingleOrWide).capacity();
    let profile = match (capacity, disc) {
        (DiscCapacity::None, _) | (_, Disc::None) => return GiantCore::NoDisc,
        (DiscCapacity::SmallPlanets, _) => return GiantCore::TooFewSolids,
        (DiscCapacity::Giants, Disc::Present(profile)) => profile,
    };
    let site = if profile.snow_line() > profile.inner_edge() {
        profile.snow_line()
    } else {
        profile.inner_edge()
    };
    let by = critical_core_age(profile, site);
    if by <= profile.lifetime() {
        GiantCore::Forms { by }
    } else {
        GiantCore::TooSlow { by }
    }
}

/// The pebble-accretion growth rate of Lambrechts and Johansen's (2014) eq. 35 at `at` in `disc`:
/// the cube root of the core mass gained in the first Myr, in M⊕^⅓, so that `M_c`(t)^⅓ =
/// `M_c,0`^⅓ + this × (t ÷ 1 Myr)^(13/18).
#[must_use]
fn pebble_growth(disc: &DiscProfile, at: Metres) -> f64 {
    let gas = disc.gas_surface_density(at).value();
    if gas <= 0.0 {
        return 0.0;
    }
    let dust_to_gas = disc.surface_density(at).value() / gas;
    let r_au = at.value() / METRES_PER_AU;
    let beta = gas * r_au * G_CM2_PER_KG_M2;
    let ratio = beta / PEBBLE_REFERENCE_GAS_SCALE_G_CM2;
    let at_one_myr = PEBBLE_CORE_MASS_AT_REFERENCE.value()
        * math::powf(dust_to_gas / PEBBLE_REFERENCE_DUST_TO_GAS, 25.0 / 6.0)
        * math::powf(disc.host_mass().value(), -11.0 / 12.0)
        * (ratio * ratio * ratio)
        * math::powf(r_au / PEBBLE_REFERENCE_RADIUS_AU, -5.0 / 4.0);
    math::cbrt(at_one_myr)
}

/// The age at which a core seeded with the local isolation mass at `at` reaches
/// [`CRITICAL_CORE_MASS`] by pebble accretion (Lambrechts and Johansen 2014, eq. 35): zero if the
/// seed is already that heavy, and infinite if no pebbles reach it.
#[must_use]
fn critical_core_age(disc: &DiscProfile, at: Metres) -> Megayears {
    let seed = math::cbrt(disc.isolation_mass(at).value());
    let needed = math::cbrt(CRITICAL_CORE_MASS.value()) - seed;
    if needed <= 0.0 {
        return Megayears::ZERO;
    }
    let growth = pebble_growth(disc, at);
    if growth <= 0.0 {
        return Megayears::new(f64::INFINITY);
    }
    Megayears::new(math::powf(needed / growth, 18.0 / 13.0))
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::lcg::Lcg;
    use hyperion_testkit::order::assert_order_independent;
    use hyperion_testkit::stats::{ALPHA, assert_p_value, ks_one_sample};

    use super::*;
    use crate::coords::{CellSize, GenCell};
    use crate::id::BodyId;
    use crate::id::Layer;
    use crate::planetary::architecture::template::{
        CHAIN_COUNT, ClassTemplate, CountLaw, TEMPLATES, template,
    };
    use crate::planetary::architecture::{ArchitectureClass, GIANT_SOLID_BUDGET, class_weights};
    use crate::planetary::derive::radius::radius_chen_kipping;
    use crate::planetary::disc::{self, DiscDraws, DiscHost, Truncation};
    use crate::stellar::Composition;
    use crate::stellar::draws::StarDraws;
    use crate::stellar::premain::disc_lifetime;
    use crate::stellar::sse::{ZCoeffs, zams};
    use crate::units::{AstronomicalUnits, Dex, HeliumExcess, SolarMasses};

    const SEED: Seed = Seed::new(0x5eed_0000_0014_0007);

    fn au(x: f64) -> Metres {
        Metres::from(AstronomicalUnits::new(x))
    }

    fn system(index: u32) -> SystemId {
        let cell = GenCell::new(CellSize::Ly8, [-7, 31, 2]).unwrap();
        SystemId::from_parts(Layer::A, cell, index).unwrap()
    }

    fn planet(slot: u8) -> BodyIndex {
        BodyIndex::new(BodySlot::Planet(slot), BodySub::Primary).unwrap()
    }

    fn slots(first: u8, count: u8) -> Vec<BodyIndex> {
        (first..first + count).map(planet).collect()
    }

    /// A host of plan 06's zero-age state at `mass` and `fe_h`.
    fn zams_host(mass: f64, fe_h: f64) -> DiscHost {
        let mass = SolarMasses::new(mass);
        let composition = Composition::from_fe_h(Dex::new(fe_h), HeliumExcess::ZERO);
        let coeffs = ZCoeffs::new(composition.z_fit());
        DiscHost::new(
            mass,
            composition.fe_h(),
            zams::luminosity(mass, &coeffs),
            zams::radius(mass, &coeffs),
        )
        .unwrap()
    }

    fn profile(disc: Disc) -> DiscProfile {
        *disc.profile().expect("a disc")
    }

    /// The disc of `host` at the median variates, living `lifetime` Myr.
    fn median_disc(host: &DiscHost, lifetime: f64) -> DiscProfile {
        profile(disc::derive(
            host,
            Megayears::new(lifetime),
            &DiscDraws::MEDIAN,
            Truncation::NONE,
        ))
    }

    /// The disc of `host` at \[Fe/H\] `fe_h` (its zero-age luminosity and radius kept) and gas
    /// fraction variate `gas`, the others at their medians, living 2 Myr.
    fn median_disc_with(host: &DiscHost, fe_h: f64, gas: f64) -> DiscProfile {
        let host = DiscHost::new(
            host.mass(),
            Dex::new(fe_h),
            host.zams_luminosity(),
            host.zams_radius(),
        )
        .unwrap();
        let draws = DiscDraws {
            gas_fraction: StandardNormal::new(gas).unwrap(),
            ..DiscDraws::MEDIAN
        };
        profile(disc::derive(
            &host,
            Megayears::new(2.0),
            &draws,
            Truncation::NONE,
        ))
    }

    /// Host 0's disc of system `index`, as drawn, living 2.5 Myr.
    fn drawn_disc(host: &DiscHost, index: u32) -> DiscProfile {
        let draws = DiscDraws::for_host(SEED, system(index), 0);
        profile(disc::derive(
            host,
            Megayears::new(2.5),
            &draws,
            Truncation::NONE,
        ))
    }

    fn chain() -> &'static PlanetGroup {
        &template(ArchitectureClass::CompactMulti).groups()[0]
    }

    fn median(values: &mut [f64]) -> f64 {
        values.sort_by(f64::total_cmp);
        values[values.len() / 2]
    }

    fn pearson(pairs: &[(f64, f64)]) -> f64 {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a sample of pairs is far below 2⁵³"
        )]
        let n = pairs.len() as f64;
        let (mx, my) = pairs
            .iter()
            .fold((0.0, 0.0), |(x, y), &(a, b)| (x + a / n, y + b / n));
        let (sxy, sxx, syy) = pairs.iter().fold((0.0, 0.0, 0.0), |(xy, xx, yy), &(a, b)| {
            let (dx, dy) = (a - mx, b - my);
            (xy + dx * dy, xx + dx * dx, yy + dy * dy)
        });
        sxy / (sxx * syy).sqrt()
    }

    /// A zero-truncated Poisson count of `law` for a host of `mass`, held at its cap, by
    /// inversion on `rng`: the placer's count (P14.T8), which this module does not draw.
    fn chain_count(law: CountLaw, mass: f64, rng: &mut Lcg) -> u8 {
        let lambda = law.poisson_rate(SolarMasses::new(mass)).unwrap();
        let (_, max) = law.range();
        loop {
            let u = rng.next_f64();
            let (mut k, mut p) = (0_u8, math::exp(-lambda));
            let mut cumulative = p;
            while u > cumulative && k < max {
                k += 1;
                p *= lambda / f64::from(k);
                cumulative += p;
            }
            if k >= 1 {
                return k;
            }
        }
    }

    #[test]
    fn draws_are_words_eight_s_onwards_of_the_system_stream() {
        let id = system(5);
        for slot in [1_u8, 2, 9, 191] {
            let mut stream = Stream::open(SEED, tags::PLANET_MASS, ObjectKey::from(id));
            stream.seek(8 * u64::from(slot));
            let draws = MassDraws::for_planet(SEED, id, planet(slot));
            assert_same_bits(draws.scatter.value(), stream.standard_normal());
            assert_same_bits(draws.group.value(), stream.standard_normal());
            assert_same_bits(draws.rank.value(), stream.uniform_open());
            assert!(stream.position() <= 8 * u64::from(slot) + MASS_WORDS_PER_SLOT);
        }
        let second = BodyIndex::new(BodySlot::SecondGeneration(0), BodySub::Primary).unwrap();
        let mut stream = Stream::open(SEED, tags::PLANET_MASS, ObjectKey::from(id));
        stream.seek(8 * 0xC0);
        assert_same_bits(
            MassDraws::for_planet(SEED, id, second).scatter.value(),
            stream.standard_normal(),
        );
    }

    #[test]
    fn a_planet_s_draws_do_not_depend_on_what_was_drawn_before() {
        let keys: Vec<(u32, u8)> = (0..5)
            .flat_map(|s| [1_u8, 2, 3, 40].map(move |p| (s, p)))
            .collect();
        assert_order_independent(&keys, |&(s, p)| {
            MassDraws::for_planet(SEED, system(s), planet(p))
        });
        assert_ne!(
            MassDraws::for_planet(SEED, system(0), planet(1)),
            MassDraws::for_planet(SEED, system(0), planet(2))
        );
        assert_ne!(
            MassDraws::for_planet(SEED, system(0), planet(1)),
            MassDraws::for_planet(SEED, system(1), planet(1))
        );
    }

    #[test]
    fn the_same_inputs_give_the_same_masses_twice() {
        let disc = drawn_disc(&zams_host(0.9, -0.1), 3);
        for group in TEMPLATES.iter().flat_map(ClassTemplate::groups) {
            let run = || group_masses(SEED, system(3), group, &disc, &slots(2, 5));
            assert_eq!(run(), run());
        }
    }

    #[test]
    fn the_reference_solid_mass_is_the_median_solar_disc_s() {
        let disc = median_disc(&zams_host(1.0, 0.0), 2.0);
        let solids = disc.solid_mass() / REFERENCE_SOLID_MASS;
        assert!((solids - 1.0).abs() < 2e-4, "{}", disc.solid_mass().value());
        // So its rocky groups are 0.5 M⊕, exactly as far as the constant's four figures allow,
        // and its compact groups, which follow the host, 7.7 M⊕.
        let rocky = &template(ArchitectureClass::TerrestrialOnly).groups()[0];
        for (group, expected) in [(chain(), 7.7), (rocky, 0.5)] {
            let m = characteristic_mass(group, &disc, StandardNormal::ZERO);
            assert!((m.value() / expected - 1.0).abs() < 2e-4, "{m:?}");
        }
    }

    /// Ruling 38, point 4: the local isolation mass cannot make the compact classes' planets, and
    /// the whole disc holds Chiang and Laughlin's minimum-mass extrasolar nebula.
    #[test]
    fn close_in_planets_need_the_whole_disc_s_solids() {
        let disc = median_disc(&zams_host(1.0, 0.0), 2.0);
        // Ten times the isolation mass at 0.1 au is 2.3 × 10⁻³ M⊕, against a chain's 1–20 M⊕.
        let local = 10.0 * disc.isolation_mass(au(0.1)).value();
        assert!((1e-3..1e-2).contains(&local), "{local} M⊕");
        // Chiang and Laughlin's (2013) eq. 4, 620 g cm⁻² (a ÷ 0.2 au)^−1.6, holds 12.7 M⊕ between
        // 0.05 and 0.5 au; the median solar disc has a fiftieth of that there.
        let sigma0 = 620.0 / G_CM2_PER_KG_M2;
        let r0 = au(0.2).value();
        let ring = |a: f64| math::powf(au(a).value() / r0, 0.4);
        let mmen = core::f64::consts::TAU * sigma0 * r0 * r0 / 0.4 * (ring(0.5) - ring(0.05))
            / crate::units::consts::EARTH_MASS_KG;
        assert!((mmen - 12.7).abs() < 0.1, "{mmen} M⊕");
        let in_situ = disc.solid_mass_between(au(0.05), au(0.5)).value();
        assert!(in_situ < mmen / 30.0, "{in_situ} M⊕ in situ");
        assert!(solid_budget(&disc).value() > 2.0 * mmen);
    }

    /// P14.T7.a, as rulings 60 and 68.2 calibrate it: the median characteristic mass of 10⁴
    /// compact groups about Suns is 6.9–8.5 M⊕, Wu's (2019) 7.7 M⊕ within 10%, as the plan held
    /// its 4 M⊕ to 3.6–4.4. It does not move when the discs' solids double, and halves with
    /// the host's mass (Pascucci et al. 2018); a rocky group's still doubles with its disc's
    /// solids.
    #[test]
    fn compact_groups_about_suns_have_wu_s_median_mass() {
        let sun = zams_host(1.0, 0.0);
        // The Sun's zero-age luminosity and radius at every mass and [Fe/H], so that only the
        // mass or the metals change.
        let host = |mass: f64, fe_h: f64| {
            DiscHost::new(
                SolarMasses::new(mass),
                Dex::new(fe_h),
                sun.zams_luminosity(),
                sun.zams_radius(),
            )
            .unwrap()
        };
        let median_of = |mass: f64, fe_h: f64| {
            let host = host(mass, fe_h);
            let mut masses: Vec<f64> = (0..10_000)
                .map(|i| {
                    let disc = drawn_disc(&host, i);
                    let group = MassDraws::for_planet(SEED, system(i), planet(1)).group;
                    characteristic_mass(chain(), &disc, group).value()
                })
                .collect();
            median(&mut masses)
        };
        let solar = median_of(1.0, 0.0);
        assert!((6.9..8.5).contains(&solar), "{solar} M⊕");
        // [Fe/H] = log₁₀ 2 doubles every disc's solids, and the gas does not change.
        let doubled = median_of(1.0, core::f64::consts::LOG10_2);
        assert_same_bits(doubled, solar);
        let half = median_of(0.5, 0.0);
        assert!((half / solar - 0.5).abs() < 1e-12, "{half} against {solar}");
        // A rocky group is proportional to its disc's solid mass.
        let rocky = &template(ArchitectureClass::TerrestrialOnly).groups()[0];
        let one = characteristic_mass(
            rocky,
            &median_disc_with(&sun, 0.0, 0.3),
            StandardNormal::ZERO,
        );
        let two = characteristic_mass(
            rocky,
            &median_disc_with(&sun, core::f64::consts::LOG10_2, 0.3),
            StandardNormal::ZERO,
        );
        assert!((two / one - 2.0).abs() < 1e-12, "{one:?} {two:?}");
    }

    /// The compact sample of P14.T7.b: 2,000 systems of solar discs, each a cold chain of the
    /// template's count at slots 1 onwards; for each adjacent pair, the log masses and the log
    /// radii of P14.T11 (each planet's own quantile, here from an LCG, as P14.T16 will draw it on
    /// `planet.radius`).
    struct CompactSample {
        masses: Vec<(f64, f64)>,
        radii: Vec<(f64, f64)>,
        groups: Vec<(GroupMasses, DiscProfile)>,
    }

    fn compact_sample() -> CompactSample {
        let host = zams_host(1.0, 0.0);
        let mut rng = Lcg::new(0x5eed_0014_0007);
        let mut sample = CompactSample {
            masses: Vec::new(),
            radii: Vec::new(),
            groups: Vec::new(),
        };
        for i in 0..2_000 {
            let disc = drawn_disc(&host, i);
            let count = chain_count(CHAIN_COUNT, 1.0, &mut rng);
            let group = group_masses(SEED, system(i), chain(), &disc, &slots(1, count));
            let radii: Vec<f64> = group
                .masses()
                .iter()
                .map(|&m| {
                    let quantile = loop {
                        if let Some(q) = UnitUniform::new(rng.next_f64()) {
                            break q;
                        }
                    };
                    math::log10(radius_chen_kipping(m, quantile).value())
                })
                .collect();
            for (pair, radius) in group.masses().windows(2).zip(radii.windows(2)) {
                sample
                    .masses
                    .push((math::log10(pair[0].value()), math::log10(pair[1].value())));
                sample.radii.push((radius[0], radius[1]));
            }
            sample.groups.push((group, disc));
        }
        sample
    }

    /// P14.T7.b's correlation, calibrated on the radii and measured on the masses.
    #[test]
    fn adjacent_planets_are_peas_in_a_pod() {
        let sample = compact_sample();
        assert!(sample.masses.len() > 4_000, "{} pairs", sample.masses.len());
        let radius_r = pearson(&sample.radii);
        let mass_r = pearson(&sample.masses);
        let outer_heavier = share(&sample.masses);
        let outer_larger = share(&sample.radii);
        // Weiss et al.'s (2018) 0.65 in log radius, the calibration's target.
        assert!((0.60..0.70).contains(&radius_r), "{radius_r}");
        // P14.T7.b's window for the log masses, corrected for Chen and Kipping's scatter.
        assert!((0.80..0.90).contains(&mass_r), "{mass_r}");
        assert!((0.55..0.75).contains(&outer_heavier), "{outer_heavier}");
        // Weiss et al.'s 65.4%, to three binomial standard errors of this sample's 4,259 pairs
        // (ruling 60's step); P14.T10.b's placed hosts give 0.651 since ruling 94.4's taper.
        assert!((0.633..0.675).contains(&outer_larger), "{outer_larger}");
    }

    /// The share of pairs whose outer member is the larger.
    fn share(pairs: &[(f64, f64)]) -> f64 {
        let outer = pairs.iter().filter(|(inner, outer)| outer > inner).count();
        #[expect(
            clippy::cast_precision_loss,
            reason = "counts of pairs are far below 2⁵³"
        )]
        let share = outer as f64 / pairs.len() as f64;
        share
    }

    /// P14.T7.b, with ruling 60's budget: no group exceeds its budget, three times its disc's
    /// solids.
    #[test]
    fn no_group_exceeds_its_budget() {
        let sample = compact_sample();
        let mut truncated = 0;
        for (group, disc) in &sample.groups {
            let budget = solid_budget(disc).value();
            assert!(group.total().value() <= budget * (1.0 + 1e-12));
            assert!((budget / disc.solid_mass().value() - 3.0).abs() < 1e-15);
            if group.cap() == GroupCap::Truncated {
                truncated += 1;
            }
        }
        // A sixth of Sun-like chains are short of their reservoir (328 of 2,000 before ruling
        // 94.4's taper): the heavy
        // chains, and the poorest discs.
        assert!(
            (240..430).contains(&truncated),
            "{truncated} of 2,000 truncated to the budget"
        );
        // Every template's groups in an M dwarf's poor disc, where the budget binds.
        let poor = profile(disc::derive(
            &zams_host(0.3, -0.5),
            Megayears::new(2.5),
            &DiscDraws {
                gas_fraction: StandardNormal::new(-1.5).unwrap(),
                ..DiscDraws::MEDIAN
            },
            Truncation::NONE,
        ));
        for group in TEMPLATES.iter().flat_map(ClassTemplate::groups) {
            let masses = group_masses(SEED, system(9), group, &poor, &slots(1, 10));
            let limit = group_budget(group, &poor);
            if is_drift_fed(group.role()) && !group.places_giants() {
                // Held at Mulders et al.'s lowest bin, 0.42 M☉ (ruling 85.3).
                let factor = math::powf(0.42, DRIFT_BUDGET_EXPONENT);
                assert!((limit / solid_budget(&poor) - factor).abs() < 1e-12);
            }
            assert!(masses.total().value() <= limit.value() * (1.0 + 1e-12));
            assert!(masses.masses().iter().all(|m| m.value() > 0.0));
        }
    }

    /// Ruling 60: a correlated group over its budget forms its innermost members, each at its
    /// drawn mass, and where even the first does not fit it forms none, so that no member lies under
    /// its floor (ruling 66). A group drawn from a law is still scaled by one factor.
    #[test]
    fn a_group_over_its_budget_forms_its_innermost_members() {
        let poor = |gas: f64| {
            profile(disc::derive(
                &zams_host(1.0, -1.0),
                Megayears::new(2.5),
                &DiscDraws {
                    gas_fraction: StandardNormal::new(gas).unwrap(),
                    ..DiscDraws::MEDIAN
                },
                Truncation::NONE,
            ))
        };
        let draws = [MassDraws::MEDIAN; 6];
        // The median solar disc forms all six; a chain's masses do not depend on its disc.
        let rich = median_disc(&zams_host(1.0, 0.0), 2.0);
        let whole = group_masses_from(chain(), &rich, &draws);
        assert_eq!(whole.cap(), GroupCap::AsDrawn);
        assert_eq!(whole.masses().len(), 6);
        // A metal-poor disc with a thirtieth of the solids forms the inner ones, bit for bit.
        let some = group_masses_from(chain(), &poor(-1.0), &draws);
        assert_eq!(some.cap(), GroupCap::Truncated);
        let formed = some.masses().len();
        assert!((1..6).contains(&formed), "{formed}");
        for (a, b) in some.masses().iter().zip(whole.masses()) {
            assert_same_bits(a.value(), b.value());
        }
        assert!(some.total() <= solid_budget(&poor(-1.0)));
        let next = some.total() + whole.masses()[formed];
        assert!(next > solid_budget(&poor(-1.0)));
        // One still poorer forms none: its budget is under even the innermost member.
        let none = group_masses_from(chain(), &poor(-3.0), &draws);
        assert_eq!(none.cap(), GroupCap::Truncated);
        assert!(none.masses().is_empty());
        assert!(solid_budget(&poor(-3.0)) < whole.masses()[0]);
        // An ice giant group, drawn from its law, is scaled down whole.
        let ice = &template(ArchitectureClass::SolarLike).groups()[2];
        let scaled = group_masses_from(ice, &poor(-3.0), &draws[..2]);
        assert_eq!(scaled.cap(), GroupCap::SolidBudget);
        assert_eq!(scaled.masses().len(), 2);
        assert!((scaled.total() / solid_budget(&poor(-3.0)) - 1.0).abs() < 1e-12);
    }

    /// P14.T7.b: changing the template of one group leaves the masses of the others
    /// bit-identical.
    #[test]
    fn changing_one_group_s_template_leaves_the_others_bit_identical() {
        let disc = drawn_disc(&zams_host(1.1, 0.2), 4);
        let cold = template(ArchitectureClass::CompactWithColdGiant).groups();
        let (chain_group, giants) = (&cold[0], &cold[1]);
        let (chain_slots, giant_slots) = (slots(1, 5), slots(6, 2));
        let chain_masses = group_masses(SEED, system(4), chain_group, &disc, &chain_slots);
        let giant_masses = group_masses(SEED, system(4), giants, &disc, &giant_slots);
        // The giants' group becomes a Solar-like system's ice giants, and then a rocky group:
        // the chain does not move.
        let ice = &template(ArchitectureClass::SolarLike).groups()[2];
        let rocky = &template(ArchitectureClass::SolarLike).groups()[0];
        for other in [ice, rocky] {
            let _ = group_masses(SEED, system(4), other, &disc, &giant_slots);
            let again = group_masses(SEED, system(4), chain_group, &disc, &chain_slots);
            for (a, b) in again.masses().iter().zip(chain_masses.masses()) {
                assert_same_bits(a.value(), b.value());
            }
        }
        // The chain becomes a substellar chain or a rocky group: the giants do not move.
        let substellar = &template(ArchitectureClass::SubstellarCompact).groups()[0];
        for other in [substellar, rocky] {
            let changed = group_masses(SEED, system(4), other, &disc, &chain_slots);
            assert_ne!(changed.masses(), chain_masses.masses());
            let again = group_masses(SEED, system(4), giants, &disc, &giant_slots);
            for (a, b) in again.masses().iter().zip(giant_masses.masses()) {
                assert_same_bits(a.value(), b.value());
            }
        }
    }

    /// Ruling 66: a drift-fed group's floor follows its host's mass, never under a rocky planet's,
    /// and a rocky group's is its template's.
    #[test]
    fn a_chain_s_floor_follows_its_host() {
        let rocky = &template(ArchitectureClass::TerrestrialOnly).groups()[0];
        let substellar = &template(ArchitectureClass::SubstellarCompact).groups()[0];
        for (mass, chain_floor) in [(1.0, 1.0), (0.3, 0.3), (0.02, 0.05)] {
            let disc = median_disc(&zams_host(mass, 0.0), 2.0);
            assert!((mass_floor(chain(), &disc).value() - chain_floor).abs() < 1e-15);
            assert_same_bits(mass_floor(rocky, &disc).value(), 0.05);
            assert!(mass_floor(substellar, &disc).value() >= 0.01);
            // A far-scattered chain member about an M dwarf lies just above its own floor, not at
            // 1 M⊕, and not at the floor itself (ruling 73.2).
            let low = [MassDraws {
                scatter: StandardNormal::new(-8.0).unwrap(),
                ..MassDraws::MEDIAN
            }];
            let drawn = group_masses_from(chain(), &disc, &low).masses()[0].value();
            let floor = mass_floor(chain(), &disc).value();
            assert!(
                drawn > floor && drawn < floor * 1.001,
                "{drawn} against {floor}"
            );
        }
    }

    /// Ruling 94.4: the tapered law's ranks follow its density, the smaller of the member's
    /// normal law and the wall falling from it at the ceiling, and its mass rises with the rank,
    /// for centres well under the ceiling (where the wall never binds), about it and far above it.
    #[test]
    fn the_taper_follows_its_density() {
        let sigma = WITHIN_SYSTEM_SCATTER_DEX;
        let (low, high, limit) = (0.0, 1.0, 1.5);
        let kappa = MASS_TAPER_INDEX * core::f64::consts::LN_10;
        for mean in [0.5, 0.9, 1.0, 1.1, 1.4, 2.0] {
            let normal = |x: f64| math::exp(-0.5 * ((x - mean) / sigma) * ((x - mean) / sigma));
            let density = |x: f64| {
                if x <= high {
                    normal(x)
                } else {
                    normal(x).min(normal(high) * math::exp(-kappa * (x - high)))
                }
            };
            // The cumulative distribution by the trapezoid rule on a fine grid.
            let steps = 60_000_u32;
            let width = (limit - low) / f64::from(steps);
            let grid: Vec<f64> = (0..=steps).map(|k| low + width * f64::from(k)).collect();
            let mut cumulative = vec![0.0];
            for pair in grid.windows(2) {
                let last = cumulative[cumulative.len() - 1];
                cumulative.push(last + 0.5 * width * (density(pair[0]) + density(pair[1])));
            }
            let total = cumulative[cumulative.len() - 1];
            let cdf = |x: f64| {
                let k = grid.partition_point(|&g| g <= x).clamp(1, grid.len() - 1);
                let frac = (x - grid[k - 1]) / width;
                (cumulative[k - 1] + frac * (cumulative[k] - cumulative[k - 1])) / total
            };
            let mut last = low;
            for k in 1..400_u32 {
                let u = f64::from(k) / 400.0;
                let z = StandardNormal::new(math::normal_quantile(u)).unwrap();
                let x = tapered_log_mass(z, mean, sigma, (low, high, limit));
                assert!(
                    x >= last && (low..=limit).contains(&x),
                    "{mean}: {x} after {last}"
                );
                last = x;
                assert!(
                    (cdf(x) - u).abs() < 1e-3,
                    "{mean}: F({x}) = {} at {u}",
                    cdf(x)
                );
            }
        }
    }

    #[test]
    fn members_step_outward_about_the_characteristic_mass() {
        assert_same_bits(steps_from_middle(0, 1), 0.0);
        assert_same_bits(steps_from_middle(0, 4), -1.5);
        assert_same_bits(steps_from_middle(3, 4), 1.5);
        assert_same_bits(steps_from_middle(2, 5), 0.0);
        // With no scatter each chain member's law is 0.21 dex heavier than the one inside it, and
        // the middle one's is centred on the characteristic mass. The inner two laws lie far
        // inside the range, and each sits within 2% of its centre; the outermost's centre lies
        // 0.2 dex under the ceiling of 20 M⊕ × M★ ÷ M☉ at every host mass (ruling 85.2), so it is
        // drawn below its centre, inside the range (ruling 73.2). A rocky group's members are all
        // alike, at its truncated law's median.
        let draws = [MassDraws::MEDIAN; 3];
        let step = math::exp10(OUTWARD_STEP_DEX);
        for host in [0.3, 1.0] {
            let disc = median_disc(&zams_host(host, 0.0), 2.0);
            let group = group_masses_from(chain(), &disc, &draws);
            let m_c = group.characteristic().unwrap().value();
            let masses = group.masses();
            assert!((masses[1].value() / m_c - 1.0).abs() < 0.02, "{masses:?}");
            assert!(
                (masses[1] / masses[0] / step - 1.0).abs() < 0.02,
                "{masses:?}"
            );
            let ceiling = mass_ceiling(chain(), &disc).value();
            assert!((ceiling / (20.0 * host) - 1.0).abs() < 1e-12);
            assert!(masses[2].value() < m_c * step && masses[2].value() < ceiling);
            assert!(masses[1] < masses[2], "{masses:?}");
        }
        let disc = median_disc(&zams_host(1.0, 0.0), 2.0);
        let rocky = &template(ArchitectureClass::TerrestrialOnly).groups()[0];
        let flat = group_masses_from(rocky, &disc, &draws);
        let m_c = flat.characteristic().unwrap().value();
        let first = flat.masses()[0].value();
        for m in flat.masses() {
            assert_same_bits(m.value(), first);
        }
        assert!((first / m_c - 1.0).abs() < 0.02, "{first} against {m_c}");
        // Drawn inside the template's range at both ends, never on it.
        let far = [MassDraws {
            scatter: StandardNormal::new(8.0).unwrap(),
            ..MassDraws::MEDIAN
        }];
        // Above the ceiling the law tapers to a giant's mass (ruling 94.4).
        let high = group_masses_from(chain(), &disc, &far).masses()[0].value();
        let giant = EarthMasses::from(SPACING_GIANT_MASS).value();
        assert!(high < giant && high > giant * 0.999, "{high}");
        let near = [MassDraws {
            scatter: StandardNormal::new(-8.0).unwrap(),
            ..MassDraws::MEDIAN
        }];
        let low = group_masses_from(chain(), &disc, &near).masses()[0].value();
        assert!(low > 1.0 && low < 1.001, "{low}");
        assert!(group_masses_from(chain(), &disc, &[]).masses().is_empty());
        assert_eq!(
            group_masses_from(chain(), &disc, &[]).characteristic(),
            None
        );
    }

    #[test]
    fn every_template_group_has_a_reference_mass_inside_its_range() {
        let substellar = &template(ArchitectureClass::SubstellarCompact).groups()[0];
        for group in TEMPLATES.iter().flat_map(ClassTemplate::groups) {
            let range = group.masses();
            // A substellar chain's hosts have discs of a few hundredths of the solar one: 7.7 M⊕
            // at 0.08 M☉ is 0.62 M⊕, inside its range.
            let reference = if group == substellar {
                reference_mass(group) * 0.08
            } else {
                reference_mass(group)
            };
            assert!(
                range.min() <= reference && reference <= range.max(),
                "{group:?}"
            );
        }
        let rocky = &template(ArchitectureClass::SolarLike).groups()[0];
        assert_same_bits(reference_mass(rocky).value(), 0.5);
        let companions = &template(ArchitectureClass::WarmGiant).groups()[1];
        assert_same_bits(reference_mass(companions).value(), 7.7);
        let ice = &template(ArchitectureClass::SolarLike).groups()[2];
        assert!((reference_mass(ice).value() - 300.0_f64.sqrt()).abs() < 1e-12);
    }

    /// P14.T7.c and the other laws: each mass is its law's quantile, so the masses of many
    /// planets follow the law.
    #[test]
    fn law_masses_follow_their_laws() {
        // A disc rich enough that no limit holds one planet of either law.
        let disc = profile(disc::derive(
            &zams_host(1.0, 0.0),
            Megayears::new(2.0),
            &DiscDraws {
                gas_fraction: StandardNormal::new(1.0).unwrap(),
                ..DiscDraws::MEDIAN
            },
            Truncation::NONE,
        ));
        let giants = &template(ArchitectureClass::EccentricGiant).groups()[1];
        let ice = &template(ArchitectureClass::SolarLike).groups()[2];
        assert_eq!(giants.masses().law(), MassLaw::PowerLaw { index: -0.31 });
        assert_eq!(ice.masses().law(), MassLaw::LogUniform);
        for group in [giants, ice] {
            let range = group.masses();
            let (lo, hi) = (range.min().value(), range.max().value());
            let law = range.law();
            let cdf = |m: f64| {
                match law {
                    MassLaw::PowerLaw { index } => {
                        (math::powf(m, index) - math::powf(lo, index))
                            / (math::powf(hi, index) - math::powf(lo, index))
                    }
                    MassLaw::LogUniform => {
                        (math::ln(m) - math::ln(lo)) / (math::ln(hi) - math::ln(lo))
                    }
                    MassLaw::Correlated => unreachable!("both groups draw from a law"),
                }
                .clamp(0.0, 1.0)
            };
            let mut masses: Vec<f64> = (0..10_000)
                .map(|i| {
                    let one = group_masses(SEED, system(i), group, &disc, &slots(1, 1));
                    assert_eq!(one.cap(), GroupCap::AsDrawn);
                    assert!(one.characteristic().is_none());
                    one.masses()[0].value()
                })
                .collect();
            assert!(masses.iter().all(|&m| (lo..=hi).contains(&m)));
            let ks = ks_one_sample(&mut masses, cdf);
            assert_p_value(&format!("{:?}", group.role()), ks.p_value, ALPHA);
        }
        // The ranks' ends map to the range's ends.
        let range = giants.masses();
        let low = UnitUniform::new(1e-15).unwrap();
        let high = UnitUniform::new(1.0 - 1e-15).unwrap();
        assert!((law_mass(range, low) / range.min() - 1.0).abs() < 1e-12);
        assert!((law_mass(range, high) / range.max() - 1.0).abs() < 1e-12);
    }

    /// P14.T7.c: no giant exceeds its disc's gas mass.
    #[test]
    fn no_giant_exceeds_its_disc_s_gas_mass() {
        let mut capped = 0;
        for (mass, fe_h) in [(1.0, 0.0), (0.5, 0.0), (0.3, -0.3), (2.0, 0.2)] {
            let host = zams_host(mass, fe_h);
            for i in 0..500 {
                let disc = drawn_disc(&host, i);
                for class in ArchitectureClass::ALL {
                    for group in template(class)
                        .groups()
                        .iter()
                        .filter(|g| g.places_giants())
                    {
                        let giants = group_masses(SEED, system(i), group, &disc, &slots(1, 3));
                        let gas = gas_budget(&disc).value();
                        assert!(giants.total().value() <= gas * (1.0 + 1e-12));
                        assert!(giants.masses().iter().all(|m| m.value() <= gas));
                        if giants.cap() == GroupCap::GasMass {
                            capped += 1;
                        }
                    }
                }
            }
        }
        assert!(capped > 0, "some poor discs hold their giants to their gas");
    }

    /// P14.T7.c: a disc with under 10 M⊕ of solids beyond the snow line yields D5's fallback.
    #[test]
    fn a_disc_with_too_few_solids_beyond_the_snow_line_falls_back() {
        // An M dwarf's median disc has just under 10 M⊕ beyond its snow line.
        let m_dwarf = median_disc(&zams_host(0.3, 0.0), 2.5);
        let beyond = m_dwarf.solid_mass_between(m_dwarf.snow_line(), m_dwarf.outer_edge());
        assert!(beyond < GIANT_SOLID_BUDGET, "{beyond:?}");
        let disc = Disc::Present(m_dwarf);
        assert_eq!(
            giant_core(&disc, ZoneLimit::Unbounded),
            GiantCore::TooFewSolids
        );
        let constraints =
            ClassConstraints::new(&disc, ZoneLimit::Unbounded, HostMultiplicity::SingleOrWide);
        let weights = class_weights(SolarMasses::new(0.3), Dex::new(0.0));
        assert_same_bits(weights.constrained(&constraints).giant_total(), 0.0);
        // A solar disc cut inside its snow line likewise, and no disc at all.
        let sun = disc::derive(
            &zams_host(1.0, 0.0),
            Megayears::new(2.5),
            &DiscDraws::MEDIAN,
            Truncation::NONE,
        );
        assert_eq!(
            giant_core(&sun, ZoneLimit::Outer(au(2.0))),
            GiantCore::TooFewSolids
        );
        assert_eq!(
            giant_core(&Disc::None, ZoneLimit::Unbounded),
            GiantCore::NoDisc
        );
        // The median solar disc grows its core in a third of a Myr.
        let core = giant_core(&sun, ZoneLimit::Unbounded);
        assert!(core.forms(), "{core:?}");
    }

    /// Lambrechts and Johansen's (2014) eq. 35 at their own reference, and their summary: a core
    /// within about 1 Myr at 5 au. A planetesimal core alone never reaches 10 M⊕ in the median
    /// solar disc.
    #[test]
    fn pebble_cores_grow_as_lambrechts_and_johansen_find() {
        let disc = median_disc(&zams_host(1.0, 0.0), 2.0);
        // The disc's own figures at 10 au, scaled to their reference by eq. 35's powers.
        let at = au(10.0);
        let gas = disc.gas_surface_density(at).value() * 10.0 * G_CM2_PER_KG_M2;
        let z = disc.surface_density(at).value() / disc.gas_surface_density(at).value();
        let growth = pebble_growth(&disc, at);
        let reference = growth * growth * growth
            / (math::powf(z / 0.01, 25.0 / 6.0) * math::powf(gas / 500.0, 3.0));
        assert!((reference - 11.0).abs() < 1e-9, "{reference} M⊕");
        let at_five = critical_core_age(&disc, au(5.0)).value();
        assert!((0.3..1.0).contains(&at_five), "{at_five} Myr at 5 au");
        // The planetesimal isolation mass peaks near r_c, under 1 M⊕.
        let most = (1..=90)
            .map(|a| disc.isolation_mass(au(f64::from(a))).value())
            .fold(0.0, f64::max);
        assert!(most < 1.0, "{most} M⊕");
        assert!((0.1..0.3).contains(&disc.isolation_mass(au(5.0)).value()));
        // Outside the disc there is no gas and no growth.
        assert_same_bits(critical_core_age(&disc, au(200.0)).value(), f64::INFINITY);
        // In the heaviest, most metal-rich disc a planetesimal core is already critical near r_c.
        let heavy = median_disc_with(&zams_host(1.0, 0.0), 0.5, 2.0);
        assert!(heavy.isolation_mass(au(30.0)) > CRITICAL_CORE_MASS);
        assert_same_bits(critical_core_age(&heavy, au(30.0)).value(), 0.0);
        let core = giant_core(&Disc::Present(disc), ZoneLimit::Unbounded);
        match core {
            GiantCore::Forms { by } => assert!((0.3..0.35).contains(&by.value()), "{by:?}"),
            other => panic!("{other:?}"),
        }
    }

    /// The share of discs that can make a giant, by the solids alone (P14.T4.c) and with the core
    /// grown within the disc's lifetime (P14.T7.c), each disc living its star's own lifetime: the
    /// figures the orchestrator rules on, pinned.
    #[test]
    fn the_core_s_lifetime_condition_removes_some_giant_capable_discs() {
        let cases = [
            (1.0, 0.0, 0.80..0.87, 0.66..0.76),
            (1.0, -0.5, 0.44..0.53, 0.25..0.33),
            (1.0, 0.3, 0.92..0.96, 0.85..0.92),
            (0.5, 0.0, 0.62..0.70, 0.60..0.68),
            (1.5, 0.0, 0.85..0.92, 0.58..0.66),
        ];
        for (mass, fe_h, solids_window, core_window) in cases {
            let host = zams_host(mass, fe_h);
            let (mut solids, mut both) = (0_u32, 0_u32);
            for i in 0..4_000 {
                let draws = DiscDraws::for_host(SEED, system(i), 0);
                let star = StarDraws::for_star(SEED, BodyId::new(system(i), 0));
                let lifetime = disc_lifetime(host.mass(), star.disc_lifetime());
                let disc = disc::derive(&host, lifetime, &draws, Truncation::NONE);
                match giant_core(&disc, ZoneLimit::Unbounded) {
                    GiantCore::Forms { .. } => (solids, both) = (solids + 1, both + 1),
                    GiantCore::TooSlow { .. } => solids += 1,
                    GiantCore::NoDisc | GiantCore::TooFewSolids => {}
                }
            }
            let (solids, both) = (f64::from(solids) / 4_000.0, f64::from(both) / 4_000.0);
            assert!(
                solids_window.contains(&solids) && core_window.contains(&both),
                "{mass} M☉, [Fe/H] {fe_h}: solids {solids}, core in time {both}"
            );
        }
    }
}
