//! Architecture classes and their frequencies: what kind of planetary system an orbit host has,
//! drawn from its mass and metallicity (plan 14, P14.T4; brainstorm, "Planetary systems",
//! approach C, step 2).
//!
//! The brainstorm calls the classes and their frequencies "ours to define and defend". This module
//! is where they are defined and defended: its documentation and [`ARCHITECTURE_TABLE`] are the
//! single written definition of the frequencies, [`template::TEMPLATES`] the single definition of
//! what each class places, and the tests read the same two tables. The disc never decides the
//! class (design note 5): it only rules out giants that cannot form, and a close binary moves
//! weight to [`Barren`](ArchitectureClass::Barren) (design note 10). Both enter
//! [`ClassConstraints`] as plain arguments, since the stable zones of P14.T9 do not exist yet.
//!
//! # The classes (P14.T4.a)
//!
//! What each class holds is the "Contents" column of plan 14's table as re-checked; the numbers
//! behind each entry are [`template`]'s. Masses are in M⊕ unless marked M♃, and "au × √L" is a
//! distance scaled by the square root of the host's zero-age luminosity in L☉, as the snow line
//! is (design note 6).
//!
//! | Class | What it holds | The observation behind it |
//! | ----- | ------------- | ------------------------- |
//! | [`Barren`](ArchitectureClass::Barren) | No body above 0.02 M⊕; belts allowed | Stars with no detected planets, and disc failures. Indistinguishable from `TerrestrialOnly` in every survey, so the split between them is a judgement (plan 14, Risks) |
//! | [`TerrestrialOnly`](ArchitectureClass::TerrestrialOnly) | Rocky planets of 0.05–2 M⊕ from 0.2–0.5 au × √L to the snow line, as many as the spacing fits there (at most 10; plan 14 had 2–6); 0–3 ice-rich bodies of 0.02–5 M⊕ beyond it | The population below survey limits. Buchhave et al. (2012, Nature 486, 375, abstract): planets under 4 R⊕ form around hosts with a wide range of metallicities, so small planets need no enhanced metallicity |
//! | [`CompactMulti`](ArchitectureClass::CompactMulti) | A chain of 1–20 M⊕ planets whose first period follows Mulders et al.'s broken power law about 12 days (1–50 days), cold with a zero-truncated Poisson count (mean 3.5 at 1 M☉, 6.1 for M dwarfs, at most 10); in 40% of systems the dynamically hot variant, 1–2 planets with larger e and i | Kepler's multis. Weiss et al. (2018, AJ 155, 48): adjacent planets alike in size and regularly spaced. Pu and Wu (2015, ApJ 807, 44): spacing near the stability limit. Mulders et al. (2018, AJ 156, 24, Table 2): innermost planets at 12 (+3 −2) days, 38 ± 8% of systems isotropic (the Kepler dichotomy), 10 planets per system. Ballard and Johnson (2016, ApJ 816, 66, abstract): an M dwarf's coplanar system holds 6.1 ± 1.9 planets |
//! | [`CompactWithColdGiant`](ArchitectureClass::CompactWithColdGiant) | A `CompactMulti` chain plus 1–2 giants of 0.3–10 M♃ at 1–3 snow-line radii, with Kipping's Betas by period | Zhu and Wu (2018, AJ 156, 92, §4): 32 ± 8% of super-Earth hosts have a cold Jupiter, and 90 ± 20% of cold-Jupiter hosts have super-Earths. Bryan et al. (2019, AJ 157, 52, abstract): 39 ± 7% have a 0.5–20 M♃ companion at 1–20 au |
//! | [`SolarLike`](ArchitectureClass::SolarLike) | Rocky planets as `TerrestrialOnly`'s up to the giants' chaotic zones, 1–3 low-eccentricity giants from 1–2 snow-line radii, 0–2 ice giants of 10–30 M⊕ beyond them, and both belts | Solar System analogues: cold Jupiters with no super-Earths, which Zhu and Wu (2018, eq. 2) put at about 1% of stars. Jupiter analogues themselves, 6.2 (+2.8 −1.6)% at 3–7 au (Wittenmyer et al. 2016, ApJ 819, 28, abstract), come mostly from `CompactWithColdGiant` |
//! | [`EccentricGiant`](ArchitectureClass::EccentricGiant) | 1–2 giants at 0.5–5 au × √L, scattered in from beyond the snow line, with eccentricities from Kipping's Beta(0.867, 3.03); 0–1 small survivor | The radial-velocity giants, and planet–planet scattering. Kipping (2013, MNRAS 434, L51, abstract): the eccentricities of 396 radial-velocity planets |
//! | [`WarmGiant`](ArchitectureClass::WarmGiant) | A giant at 10–200 days with moderate e (Kipping's Betas by period); in half of systems 1–2 small companions flanking it | Disc migration, as plan 14 reads it. Huang, Wu and Triaud (2016, ApJ 825, 98, abstract): half of warm Jupiters (10–200 days) are closely flanked by small planets, and those, they propose, formed in situ |
//! | [`HotJupiter`](ArchitectureClass::HotJupiter) | A giant at 1–10 days, log-normal about 3.5 days; nothing else inside 100 days; in 60% of systems an outer giant of 1–10 M♃ at 2–8 snow-line radii | Wright et al. (2012, ApJ 753, 160) and Howard et al. (2012, ApJS 201, 15) for the rate; the pile-up near 3 days (Cumming et al. 2008, PASP 120, 531, §3.3.2 and Fig. 12). Huang et al. (2016): no companion inside 50 days (the 100 days is plan 14's). Bryan et al. (2016, ApJ 821, 89, abstract): 52 ± 5% of giant hosts have a 1–20 M♃ companion at 5–20 au, hot giants more often |
//! | [`SubstellarCompact`](ArchitectureClass::SubstellarCompact) | For hosts under 0.08 M☉: a compact chain of 0.01–2 M⊕ bodies | Design note 13; TRAPPIST-1 as the limiting case |
//!
//! The migrated groups are the giants of `WarmGiant`, `HotJupiter` and `EccentricGiant`, and the
//! chains of the compact classes. A migrated body keeps the composition of where it formed
//! ([`template::Origin`]): a migrated giant formed beyond the snow line, and each planet of a
//! chain has its origin set where it is placed (P14.T8, T11).
//!
//! # The frequency model (P14.T4.b)
//!
//! Each class has a weight w(M★, \[Fe/H\]) = w₀ × (mass scaling) × (metallicity scaling), and its
//! probability is its weight over the sum of all nine ([`ClassWeights::probabilities`]). At 1 M☉
//! and solar metallicity the weights sum to 1.102, so w₀ reads nearly as a probability there.
//! The anchors are observed rates, which see the outcome and not the table, so the table is set
//! to meet them after both of design note 5's fallbacks (ruling 48, point c; ruling 55.3): the
//! giant classes' w₀ are plan 14's draft times [`GIANT_WEIGHT_SCALE`], 1.701, and `HotJupiter`'s
//! is [`HOT_JUPITER_WEIGHT`].
//!
//! | Class | w₀ | Mass scaling | Metallicity scaling |
//! | ----- | -- | ------------ | ------------------- |
//! | `Barren` | 0.24 | 1 | 1, plus the weight the other classes lose in the halo |
//! | `TerrestrialOnly` | 0.35 | 1 | s(\[Fe/H\]) |
//! | `CompactMulti` | 0.21 | (M ÷ M☉)^−3.0 | s(\[Fe/H\]) |
//! | `CompactWithColdGiant` | 0.1701 (0.10 × 1.701) | g(M) | z(\[Fe/H\]) |
//! | `SolarLike` | 0.01701 (0.01 × 1.701) | g(M) | z(\[Fe/H\]) |
//! | `EccentricGiant` | 0.06804 (0.04 × 1.701) | g(M) | z(\[Fe/H\]) |
//! | `WarmGiant` | 0.03402 (0.02 × 1.701) | g(M) | z(\[Fe/H\]) |
//! | `HotJupiter` | 0.0127 | g(M) | z(\[Fe/H\]) |
//! | `SubstellarCompact` | 1 | `TerrestrialOnly` + `CompactMulti` at 0.08 M☉ | s(\[Fe/H\]) |
//!
//! The sources of each row:
//!
//! - `Barren` and `TerrestrialOnly`, 0.24 and 0.35: plan 14's split of what no survey sees, a
//!   judgement (Risks). Their sum, with the compact classes at 35.7% and the giants at 19.5%
//!   after the fallback, is what is left at 1 M☉.
//! - `CompactMulti`, 0.21, and `CompactWithColdGiant`, 0.1701: together, after the fallbacks,
//!   35.7% of Sun-like stars with 0.97 planets per star, against Zhu et al.'s (2018, ApJ 860,
//!   101, abstract) 30 ± 3% with Kepler-like planets (at least 1 R⊕, inside 400 days) and about
//!   0.9 per star; a cold giant in 30.7% of those, Zhu and Wu's (2018, AJ 156, 92, §4) 32 ± 8%.
//!   Plan 14's draft had 0.24 and 0.07, which gave 23% a cold giant. `TerrestrialOnly`'s rocky
//!   planets of an Earth mass or more, inside 400 days around a Sun-like star, are Kepler-like
//!   too, so η rises to roughly 40–50% and the planets per star from 0.97 to about 1.2, as
//!   P14.T7's masses decide: between Zhu et al.'s 30 ± 3% and 0.9, and Yang, Xie and Zhou's
//!   (2020, AJ 159, 164, abstract) 73 ± 13% or Mulders et al.'s (2018, AJ 156, 24, abstract) "at
//!   least 42%" (ruling 48, point f).
//! - The early M dwarfs (ruling 85.4): about hosts of 0.35–0.6 M☉, blended over 0.30–0.35 and
//!   0.60–0.70 M☉ ([`early_m_dwarf_share`]), `Barren` and `TerrestrialOnly` take no weight, so
//!   every such host draws a system (Hsu, Ford and Terrien 2020, MNRAS 498, 2249, §5), 55% of its
//!   chains the hot variant and the rest cold chains of about five planets inside 200 days
//!   (Ballard and Johnson 2016, ApJ 816, 66, §3.3), the hot variant with the cold chain's count,
//!   since Ballard and Johnson's single-transiting mode is a transiting multiplicity (ruling
//!   87.2), their first periods calibrated to Dressing and Charbonneau's (2015, Table 5) 19%
//!   inside 10 days (P14.T5's constants).
//! - `CompactMulti`'s (M ÷ M☉)^−3.0: re-fitted by ruling 60 on P14.T10.b's placed M dwarfs (plan
//!   14 had −0.9, P14.T4 −1.5, which placed gave 0.98 small planets per M dwarf), to Dressing and
//!   Charbonneau's (2015, ApJ 807, 45, abstract) 2.5 ± 0.2 small planets per M dwarf (1–4 R⊕,
//!   under 200 days), hosts of median 0.47 R☉ (§2): placed primaries of 0.35–0.6 M☉ with their
//!   companions had 1.90, inside T10.b's 1.8–3.2, together with P14.T7's host-scaled masses and
//!   Class 0 budget, before ruling 66's floor (2.83 now, after ruling 87.2's hot variant);
//!   −2.5 gave 1.75. Planets of 1–8 M⊕ (about 1–2.8 R⊕) at 2–50 days are then 2.4
//!   times as common about those M dwarfs as about FGK stars, between Mulders, Pascucci and Apai's
//!   (2015, ApJ 798, 112, abstract) "twice as frequently as around G stars, and thrice as
//!   frequently as around F stars" and their (2015, ApJ 814, 130, abstract) 3.5 times more
//!   1.0–2.8 R⊕ planets. The weight is 1 at 1 M☉, so no Sun-like anchor moves; a 0.3 M☉ host is
//!   93% compact and a 0.1 M☉ host over 99%.
//! - The giant classes' w₀ (ruling 48, point c): plan 14's 0.10, 0.01, 0.04 and 0.02, in their
//!   ratios, times 1.701, and `HotJupiter`'s 0.0127, set together over the solar-disc sample of
//!   this module's tests (83.4% of whose discs have the solids for a giant, and 71.1% grow its core
//!   within their star's own disc lifetime) so that after both fallbacks, at 1 M☉ and
//!   \[Fe/H\] = 0: giants of 0.3–10 M♃ at 2–2,000 days lie around 10.5% of hosts, with the
//!   templates' orbits, Cumming et al.'s (2008, PASP 120, 531, abstract) figure; giants in all
//!   around 19.5%, inside their extrapolated 17–20% within 20 au; and hot Jupiters around 0.82%,
//!   between Howard et al.'s (2012, ApJS 201, 15, §3.2 and Table 4) 0.4–0.5% of Kepler's Sun-like
//!   stars and Wright et al.'s (2012, ApJ 753, 160, abstract) 1.2 ± 0.38% of the solar
//!   neighbourhood's. `SolarLike`'s share is bounded by Zhu and Wu's (2018, eq. 2) P(no SE, CJ) =
//!   (1 − P(SE ∣ CJ)) P(CJ) ≈ 1%, which counts every cold Jupiter without super-Earths: with
//!   `EccentricGiant`'s cold giants and the hot Jupiters' outer giants, about 4% of Sun-like stars
//!   have such a system after the fallbacks, and P(SE ∣ CJ) is about 75%, inside their 90 ± 20%
//!   (plan 14's `SolarLike` of 0.03 gave about 6% and 64%). `EccentricGiant`'s and `WarmGiant`'s
//!   shares of the giants are plan 14's.
//! - g(M), the giant classes' mass scaling: M ÷ M☉ up to 1.9 M☉, Johnson et al.'s (2010, PASP
//!   122, 905, eq. 8 and Table 1) f ∝ M★^(1.0 ± 0.3), fitted over 0.2–1.9 M☉; above it
//!   1.9 exp(−½ ((M − 1.9 M☉) ÷ 0.5 M☉)²), the fall of Reffert et al.'s (2015, A&A 574, A116,
//!   eq. 3 and §5) Gaussian, peak µ = 1.9 (+0.1 −0.5) M☉ and width σ = 0.5 (+0.5 −0.2) M☉.
//! - z(\[Fe/H\]), the giant classes' metallicity scaling: 10^(2 \[Fe/H\]) for \[Fe/H\] in −0.5 to
//!   +0.5, Fischer and Valenti's (2005, ApJ 622, 1102, abstract) law, "the formation probability
//!   for gas giant planets [scales as] the square of the number of metal atoms", which the
//!   brainstorm adopts ("roughly as 10^(2\[Fe/H\])"); held at 10 above +0.5; and below −0.5 held at
//!   0.1 and thinned as the small planets are, 0.1 s(\[Fe/H\]) ÷ s(−0.5). The hold below −0.5
//!   follows Sozzetti et al. (2009, ApJ 697, 544, abstract), whose data for −1.0 to 0.0 are
//!   "compatible ... with a constant occurrence rate fp ≃ 1%", and Mortier et al. (2013, A&A 551,
//!   A112, abstract), who find "no statistical difference between a constant or an exponential
//!   function" below solar metallicity; the thinning keeps giants from outnumbering small planets
//!   in the halo, where held they would (at −2.5, 3.0% of hosts with a giant against 0.7% with
//!   small planets alone), against every survey's order
//!   (giants favour metal-rich hosts and small planets do not: Buchhave et al. 2012, 2014) and
//!   Mortier et al.'s (2012, A&A 543, A45, abstract) "strong function of metallicity, even in the
//!   low-metallicity tail". Sozzetti et al.'s fp < 0.67% for −2.0 to −0.6 (planets above about 3
//!   M♃ (P ÷ yr)^⅓ inside 3 years) is met either way, since about a tenth of the giants fall in
//!   that window.
//! - s(\[Fe/H\]) = 1 ÷ (1 + 10^(−2 (\[Fe/H\] + 1.5))), the small planets' metallicity scaling: flat
//!   through the discs, 0.96 of solar at −0.8, and a tenth of it at −2, as the brainstorm asks
//!   ("Small rocky planets depend on it only weakly ... they should thin out only at the very low
//!   metallicities of the halo"). The flat part is Buchhave et al. (2012, Nature 486, 375,
//!   abstract) and Petigura et al. (2018, AJ 155, 89, abstract: 20 warm super-Earths per 100 stars
//!   "regardless of metallicity" over −0.4 to +0.4). It is flat only below 1.7 R⊕: Petigura et
//!   al.'s warm sub-Neptunes (1.7–4.0 R⊕) double over the same range, and Buchhave et al. (2014,
//!   Nature 509, 593, abstract) find three metallicity regimes, divided at 1.7 and 3.9 R⊕. The
//!   class frequency stays flat. P14.T7 no longer carries that trend in the masses: Zhu (2019,
//!   ApJ 873, 8, §2) finds the hosts of 1–2 and 2–4 R⊕ planets of "statistically the same"
//!   metallicity (ruling 60). The knee at −1.5 is plan 14's, from the brainstorm's
//!   halo; no survey measures it.
//! - The halo: what s and z take from the other classes goes to `Barren`, so that thinning the
//!   small planets in the halo does not raise every other class's share by normalisation.
//! - `SubstellarCompact`: for a host under 0.08 M☉, the lower limit of the stellar mass function
//!   ([`galaxy::imf::MASS_LIMIT_LO`](crate::galaxy::imf::MASS_LIMIT_LO)), the two small-planet
//!   classes' weights at 0.08 M☉ go to it, and the giant classes are zero (design note 13: "no
//!   giants, a compact chain or nothing"). No survey has measured how often a brown dwarf has
//!   planets; the weight is continuous with the stars' at the limit.
//!
//! The ratio form keeps each giant class proportional to 10^(2 \[Fe/H\]) while giants are rare and
//! saturates their total near four fifths at +0.5 (79% at 1 M☉, before the fallbacks). The
//! share of giants at 1 M☉ has a log-slope of 1.85 between −0.5 and −0.2, and bends as it
//! saturates.
//!
//! # The anchors, re-checked
//!
//! Plan 14 cited its literature from memory. Each anchor was re-checked against the paper
//! (downloaded from arXiv, or the publisher's abstract where there is no preprint).
//!
//! | Anchor | What the paper says | Re-checked | What the table gives, after the fallback |
//! | ------ | ------------------- | ---------- | -------------------- |
//! | Cumming et al. (2008, PASP 120, 531) | "10.5% of solar type stars have a planet with mass in the range 0.3–10 M♃ and orbital period 2–2000 days"; "17–20% of stars having gas giant planets within 20 AU"; dN ∝ M^(−0.31 ± 0.2) P^(0.26 ± 0.1) d ln M d ln P (abstract); M dwarfs 1.0%, under 5.4% at 2σ (§3.4) | Yes, full text | 10.5% at 2–2,000 days with the templates' orbits; 19.5% in all at 1 M☉ |
//! | Hot Jupiters (Wright et al. 2012, ApJ 753, 160; Howard et al. 2012, ApJS 201, 15) | 1.2 ± 0.38% of FGK dwarfs (Wright, abstract); 0.004 ± 0.001 per star for P < 10 days and 8–32 R⊕, 0.005 ± 0.001 to Kp < 16 (Howard, §3.2 and Table 4) | Yes, full text | 0.82% at 1 M☉ (1.15% in the table before the fallbacks) |
//! | Zhu et al. (2018, ApJ 860, 101) | "the fraction of Sun-like stars with Kepler-like planets ... is 30 ± 3%", with 3.0 ± 0.3 planets within 400 days per system and about 0.9 per star (abstract). Yang, Xie and Zhou (2020, AJ 159, 164, abstract and §5.1) find 73 ± 13% and 2.3 ± 0.4 with DR25 and efficiency corrections | Yes, full text | 35.7% at 1 M☉, 2.7 planets per system, 0.97 per star from the compact classes; roughly 40–50% and 1.2 counting `TerrestrialOnly`'s Earth-mass planets |
//! | Zhu and Wu (2018, AJ 156, 92) | P(CJ ∣ SE) = 32 ± 8%, rising to 60% or more for \[Fe/H\] > 0.1; P(SE ∣ CJ) = 90 ± 20%; cold Jupiters without super-Earths ∼1% of stars (abstract, eq. 2, §4) | Yes, full text | P(CJ ∣ SE) 30.7% at 1 M☉, and 67% at +0.2 before the fallbacks; P(SE ∣ CJ) about 75%, and about 4% of stars with a cold Jupiter and no super-Earth |
//! | Dressing and Charbonneau (2015, ApJ 807, 45) | "2.5 ± 0.2 planets per M dwarf with radii 1–4 R⊕ and periods shorter than 200 days" (abstract), hosts under 4,000 K, median 3,746 K and 0.47 R☉ (§2) | Yes, full text | Placed, 1.75 about primaries of 0.35–0.6 M☉ with their companions (P14.T10.b, ruling 85.4), a finding under the 1.8–3.2 window |
//! | Giants around about 3% of M dwarfs | Johnson et al. (2010, abstract): "3% around M dwarfs (0.5 M☉)" inside 2.5 au; Cumming et al. (2008, §3.4): 1.0%; Bonfils et al. (2013, A&A 549, A109, abstract): ≲1% at 1–10 days and 2 (+3 −1)% at 10–100 days; Montet et al. (2014, ApJ 781, 28, abstract): 6.5 ± 3.0% for 1–13 M♃ within 20 au | Yes, full text | 2.3% at 0.3 M☉, where half the discs cannot form a giant (the median disc has just under 10 M⊕ of solids beyond its snow line), and 7.2% at 0.5 M☉, about 5.6% inside Johnson et al.'s window; 4.6% and 11.3% in the table before the fallbacks |
//! | Johnson et al. (2010, PASP 122, 905) | f(M★, \[Fe/H\]) = 0.07 ± 0.01 (M★ ÷ M☉)^(1.0 ± 0.3) 10^((1.2 ± 0.2) \[Fe/H\]), for K > 20 m s⁻¹ and a < 2.5 au, over 0.2–1.9 M☉ (eq. 8, Table 1) | Yes, full text | g(M) = M ÷ M☉ to 1.9 M☉. Their metallicity exponent is 1.2, and 1.7 ± 0.3 on Fischer and Valenti's stars alone (§6.1); the brainstorm keeps 2 |
//! | Reffert et al. (2015, A&A 574, A116) | A Gaussian in mass with µ = 1.9 (+0.1 −0.5) M☉ and σ = 0.5 (+0.5 −0.2) M☉, half its peak at 1.2 and 2.6 M☉; no planet above 2.7 M☉, under 1.6% for 2.7–5 M☉ (abstract, eq. 3, §5) | Yes, full text | g falls with σ = 0.5 M☉ above 1.9 M☉ (plan 14 had 0.8) |
//! | Fischer and Valenti (2005, ApJ 622, 1102) | Giant occurrence rises as "the square of the number of metal atoms"; under 3% for −0.5 < \[Fe/H\] < 0.0 and 25% above +0.3, for K > 30 m s⁻¹ and P < 4 yr (abstract) | Abstract only: the paper has no preprint and the publisher's full text was not reachable, so the fitted interval of ±0.5 is plan 14's reading, consistent with the abstract's bins | z = 10^(2 \[Fe/H\]) on −0.5 to +0.5 exactly |
//! | Buchhave et al. (2012, Nature 486, 375) | "planets with radii less than four Earth radii form around host stars with a wide range of metallicities (but on average a metallicity close to that of the Sun), whereas large planets preferentially form around stars with higher metallicities" (abstract) | Abstract only (no preprint); Buchhave et al. (2014, Nature 509, 593, full text) and Petigura et al. (2018, full text) agree | s is flat to within 4% down to −0.8 |
//!
//! The table gives a host's frequencies where its disc can form giants. With plan 14's disc
//! (P14.T3's draws) that is 83.4% of solar-mass, solar-metallicity hosts, 67% at 0.5 M☉, 50% at
//! 0.3 M☉ and 48% of solar-mass hosts at \[Fe/H\] = −0.5, and the rest fall back (design note 5).
//! Of discs living their star's own lifetime, a core grows in time (P14.T7.c, the second fallback,
//! which P14.T8's placer applies; ruling 55.3) in 71.1%, 63.8%, 49.2% and 28.5% of them. The last
//! column is measured after both fallbacks, as P14.T10.b measures, and the test
//! `the_anchors_hold_after_the_disc_fallback` asserts it at 1 M☉. The fallbacks also steepen the
//! giants' fall below solar metallicity.
//!
//! # Where the table departs from plan 14's first draft
//!
//! - `CompactWithColdGiant` 0.07 → 0.10 and `CompactMulti` 0.24 → 0.21: the draft gave 23% of
//!   compact systems a cold giant, below Zhu and Wu's 32 ± 8% and Bryan et al.'s 39 ± 7%.
//! - The giant classes then × 1.701, and `HotJupiter` 0.008 → 0.0127, so that the anchors hold
//!   after both fallbacks, which remove 28.9% of Sun-like hosts' giants (ruling 48, point c, which
//!   gave × 1.373 and 0.0103 after the first alone; ruling 55.3).
//! - `SolarLike` 0.03 → 0.01: the draft's 3% counted Wittenmyer et al.'s Jupiter analogues, most of
//!   which have super-Earths (Zhu and Wu: 90 ± 20% of cold-Jupiter hosts do) and so belong to
//!   `CompactWithColdGiant`; Zhu and Wu's ∼1% of cold Jupiters without super-Earths bounds what is
//!   left, which the eccentric and outer giants already fill.
//! - `CompactMulti`'s mass exponent −0.9 → −1.5 → −3.0: with −0.9 and the draft's counts the
//!   table gave 1.4 small planets per M dwarf against Dressing and Charbonneau's 2.5 ± 0.2,
//!   counting every chain planet, and placed, −1.5 gave 0.98 (ruling 60).
//! - Reffert et al.'s width 0.8 → 0.5 M☉: their σ is 0.5 (+0.5 −0.2) M☉; 0.8 kept 60% of the peak
//!   at 2.7 M☉, where they find no planets.
//! - Below \[Fe/H\] = −0.5 the giant weight is thinned by s and not only held: held, it made giants
//!   more common than small planets in the halo (at −2.5, 3.0% of hosts with a giant against 0.7%
//!   with small planets alone).
//! - The halo's loss goes to `Barren` instead of to every class by normalisation.
//! - `SubstellarCompact` has a weight: the draft left it without one.
//!
//! # Constraints (design notes 5 and 10)
//!
//! [`ClassWeights::constrained`] applies, from plain arguments ([`ClassConstraints::new`]: the
//! disc, the stable zone's end as a [`ZoneLimit`], and a [`HostMultiplicity`]):
//!
//! - **No disc**: everything is `Barren`, since nothing can form.
//! - **No giants** (design note 5), when the snow line is not inside the reach of the disc (the
//!   nearer of its outer edge and the stable zone's outer limit) or when the solids between them
//!   are under [`GIANT_SOLID_BUDGET`], 10 M⊕: each giant class's weight moves to its giant-free
//!   sibling ([`ArchitectureClass::giant_free_sibling`]), the in-situ giants' (`SolarLike`,
//!   `EccentricGiant`) to `TerrestrialOnly` and the migrating ones' (`CompactWithColdGiant`,
//!   `WarmGiant`, `HotJupiter`) to `CompactMulti`. The disc's lifetime is the second fallback,
//!   P14.T7.c's core grown in time, which P14.T8's placer applies after the draw
//!   ([`core_fallback`](crate::planetary::placement::classes::core_fallback); ruling 55.3), to the
//!   same sibling.
//! - **A close binary** (design note 10), inside Kraus et al.'s cut-off: every class but `Barren`
//!   keeps [`CLOSE_BINARY_SUPPRESSION`], 0.34, of its weight and `Barren` takes the rest.
//!
//! Plan 14's class frequencies in tests (P14.T10.b) are measured after these.
//!
//! # The draw
//!
//! One [`Mark`] per orbit host on [`tags::PLANET_CLASS`], keyed by the system's ID with the host's
//! number in the draw number (design note 4): host h reads word 4h ([`CLASS_WORDS_PER_HOST`]),
//! and words 4h + 1 to 4h + 3 are reserved. The mark picks a class by integer thresholds on the
//! constrained weights, with their own total as the bound, so that no mark is rejected
//! ([`Mark::pick_weighted`], the same answer as
//! [`Thresholds::from_weights`](crate::rng::Thresholds::from_weights) and [`Mark::pick`]). The
//! order of [`ArchitectureClass::ALL`] is the order of the thresholds, and so part of the output.

pub mod template;

use super::disc::Disc;
use crate::galaxy::imf::MASS_LIMIT_LO;
use crate::id::SystemId;
use crate::math;
use crate::rng::{Mark, ObjectKey, Seed, Stream, tags};
use crate::units::{Dex, EarthMasses, Metres, SolarMasses};

/// The number of architecture classes.
pub const CLASS_COUNT: usize = 9;

/// The kind of planetary system an orbit host has (P14.T4.a); see the [module documentation](self)
/// for what each holds and why.
///
/// The declaration order is [`ALL`](Self::ALL)'s, which is the order of the pick's thresholds and
/// so part of the output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ArchitectureClass {
    /// No body above 0.02 M⊕; belts allowed.
    Barren,
    /// Rocky planets inside the snow line and ice-rich bodies beyond it, with no giant.
    TerrestrialOnly,
    /// A compact chain of super-Earths and sub-Neptunes, or its dynamically hot variant.
    CompactMulti,
    /// A compact chain with one or two cold giants beyond the snow line.
    CompactWithColdGiant,
    /// A Solar System analogue: rocky planets, cold giants, ice giants and both belts.
    SolarLike,
    /// One or two eccentric giants scattered inward, and at most one small survivor.
    EccentricGiant,
    /// A giant at 0.1–1 au × √L, with small companions in half of systems.
    WarmGiant,
    /// A hot Jupiter with nothing else inside 100 days, and often an outer giant.
    HotJupiter,
    /// For a host under 0.08 M☉: a compact chain of small bodies.
    SubstellarCompact,
}

impl ArchitectureClass {
    /// Every class, in declaration order.
    pub const ALL: [Self; CLASS_COUNT] = [
        Self::Barren,
        Self::TerrestrialOnly,
        Self::CompactMulti,
        Self::CompactWithColdGiant,
        Self::SolarLike,
        Self::EccentricGiant,
        Self::WarmGiant,
        Self::HotJupiter,
        Self::SubstellarCompact,
    ];

    /// The class's position in [`ALL`](Self::ALL), which indexes [`ClassWeights`] and
    /// [`ClassProbabilities`].
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Barren => 0,
            Self::TerrestrialOnly => 1,
            Self::CompactMulti => 2,
            Self::CompactWithColdGiant => 3,
            Self::SolarLike => 4,
            Self::EccentricGiant => 5,
            Self::WarmGiant => 6,
            Self::HotJupiter => 7,
            Self::SubstellarCompact => 8,
        }
    }

    /// A lower-case name, for golden files and logs.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Barren => "barren",
            Self::TerrestrialOnly => "terrestrial_only",
            Self::CompactMulti => "compact_multi",
            Self::CompactWithColdGiant => "compact_with_cold_giant",
            Self::SolarLike => "solar_like",
            Self::EccentricGiant => "eccentric_giant",
            Self::WarmGiant => "warm_giant",
            Self::HotJupiter => "hot_jupiter",
            Self::SubstellarCompact => "substellar_compact",
        }
    }

    /// Whether the class places a giant of 0.3 M♃ or more, and so needs a disc that can form one
    /// (design note 5).
    #[must_use]
    pub const fn has_giants(self) -> bool {
        match self {
            Self::CompactWithColdGiant
            | Self::SolarLike
            | Self::EccentricGiant
            | Self::WarmGiant
            | Self::HotJupiter => true,
            Self::Barren | Self::TerrestrialOnly | Self::CompactMulti | Self::SubstellarCompact => {
                false
            }
        }
    }

    /// The class a host falls back to when its disc cannot form giants (design note 5); a class
    /// without giants is its own.
    ///
    /// The classes whose giants form where they stay (`SolarLike`, and `EccentricGiant`, whose
    /// giants scatter only after forming) fall back to `TerrestrialOnly`, the system their small
    /// planets make alone. The classes whose planets migrate (`CompactWithColdGiant`, `WarmGiant`,
    /// `HotJupiter`) fall back to `CompactMulti`: the chain without the giant, or the migrating
    /// small planets that would have flanked it (Huang et al. 2016).
    #[must_use]
    pub const fn giant_free_sibling(self) -> Self {
        match self {
            Self::CompactWithColdGiant | Self::WarmGiant | Self::HotJupiter => Self::CompactMulti,
            Self::SolarLike | Self::EccentricGiant => Self::TerrestrialOnly,
            Self::Barren | Self::TerrestrialOnly | Self::CompactMulti | Self::SubstellarCompact => {
                self
            }
        }
    }
}

/// The host mass below which a host is substellar and draws on the `SubstellarCompact` row: 0.08
/// M☉, the lower limit of the stellar mass function
/// ([`MASS_LIMIT_LO`]) and near the hydrogen-burning limit.
pub const SUBSTELLAR_LIMIT: SolarMasses = SolarMasses::new(MASS_LIMIT_LO);

/// The stellar mass at which giant occurrence peaks: 1.9 M☉ (g(M)).
///
/// Johnson et al. (2010, PASP 122, 905, eq. 8) fit f ∝ M★ over 0.2–1.9 M☉; Reffert et al. (2015,
/// A&A 574, A116, §5) find the peak of their Gaussian at µ = 1.9 (+0.1 −0.5) M☉.
pub const GIANT_HOST_PEAK: SolarMasses = SolarMasses::new(1.9);

/// The width of the fall of giant occurrence above [`GIANT_HOST_PEAK`]: σ = 0.5 M☉.
///
/// Reffert et al. (2015, §5): σ = 0.5 (+0.5 −0.2) M☉, with occurrence at half its peak at 2.6 M☉
/// and no planet found above 2.7 M☉ (under 1.6% at 68.3% confidence for 2.7–5 M☉, abstract).
pub const GIANT_HOST_FALL_WIDTH: SolarMasses = SolarMasses::new(0.5);

/// The exponent of the giant classes' metallicity law, 10^(2 \[Fe/H\]): Fischer and Valenti
/// (2005, ApJ 622, 1102, abstract), "the square of the number of metal atoms".
pub const GIANT_METALLICITY_EXPONENT: f64 = 2.0;

/// The lower end of the giant metallicity law's range, \[Fe/H\] = −0.5, below which the law is
/// held and thinned with the small planets (Fischer and Valenti 2005; Sozzetti et al. 2009, ApJ
/// 697, 544; Mortier et al. 2013, A&A 551, A112).
pub const GIANT_METALLICITY_FLOOR: Dex = Dex::new(-0.5);

/// The upper end of the giant metallicity law's range, \[Fe/H\] = +0.5, above which it is held.
pub const GIANT_METALLICITY_CEILING: Dex = Dex::new(0.5);

/// The \[Fe/H\] at which the small planets' metallicity scaling s is one half: −1.5, plan 14's
/// knee for the brainstorm's halo.
pub const SMALL_PLANET_METALLICITY_KNEE: Dex = Dex::new(-1.5);

/// How steeply s falls below its knee: s = 1 ÷ (1 + 10^(−2 (\[Fe/H\] + 1.5))), so that it falls as
/// 10^(2 \[Fe/H\]) far below the knee, as the giants' law does above −0.5 (plan 14).
pub const SMALL_PLANET_METALLICITY_SLOPE: f64 = 2.0;

/// `CompactMulti`'s mass exponent: (M ÷ M☉)^−3.0.
///
/// Re-fitted by ruling 60 on P14.T10.b's placed M dwarfs, as ruling 48 (b) and (e) asked, to
/// Dressing and Charbonneau's (2015, ApJ 807, 45, abstract) 2.5 ± 0.2 small planets per M dwarf
/// inside 200 days, the T10.b window 1.8–3.2; see the [module documentation](self). Plan 14 had
/// −0.9 and P14.T4 −1.5.
pub const COMPACT_MASS_EXPONENT: f64 = -3.0;

/// The factor by which the giant classes' w₀ are raised over plan 14's draft (0.10, 0.01, 0.04 and
/// 0.02 for `CompactWithColdGiant`, `SolarLike`, `EccentricGiant` and `WarmGiant`), so that the
/// anchors hold after both of design note 5's fallbacks (ruling 48, point c; ruling 55.3): 1.701.
///
/// Observed rates see the outcome, not the table. Over the solar-disc sample of this module's
/// tests (20,000 discs of a 1 M☉ host at \[Fe/H\] = 0, each living its star's own drawn lifetime),
/// 83.4% of discs have the solids for a giant (P14.T4.c) and 71.1% grow its core in time
/// (P14.T7.c), and the rest fall back to their giant-free siblings. With this factor, and
/// [`HOT_JUPITER_WEIGHT`], giants of 0.3–10 M♃ at 2–2,000 days come out around 10.5% of those
/// hosts after both fallbacks, Cumming et al.'s (2008, PASP 120, 531, abstract) figure, and a cold
/// giant in 30.7% of compact systems, Zhu and Wu's (2018, AJ 156, 92, §4) 32 ± 8%. The ratios
/// between these four classes are the draft's. After the first fallback alone the factor was
/// 1.373 (ruling 48); the second raised it by 24%.
pub const GIANT_WEIGHT_SCALE: f64 = 1.701;

/// `HotJupiter`'s w₀: 0.0127, so that hot Jupiters come out around 0.82% of Sun-like hosts after
/// both of design note 5's fallbacks (ruling 48, point c; ruling 55.3), between Howard et al.'s
/// (2012, ApJS 201, 15, §3.2 and Table 4) 0.4–0.5% of Kepler's stars and Wright et al.'s (2012,
/// ApJ 753, 160, abstract) 1.2 ± 0.38% of the solar neighbourhood's.
///
/// It is the draft's 0.008 raised by 1.59 rather than by [`GIANT_WEIGHT_SCALE`], since the hot
/// Jupiters' rate is an anchor of its own; after the first fallback alone it was 0.0103.
pub const HOT_JUPITER_WEIGHT: f64 = 0.0127;

/// The solids a giant needs beyond the snow line: 10 M⊕ (design note 5; P14.T7.c).
///
/// Plan 14's figure, the core mass above which a core's envelope can no longer stay in
/// hydrostatic balance and runaway gas accretion begins (Pollack et al. 1996, Icarus 124, 62, cited
/// from memory, not re-checked).
pub const GIANT_SOLID_BUDGET: EarthMasses = EarthMasses::new(10.0);

/// The share of its weight that each planet-bearing class keeps around a host in a close binary:
/// 0.34 (design note 10).
///
/// Kraus et al. (2016, AJ 152, 8, abstract): inside a semi-major axis of 47 (+59 −23) au "the
/// planet occurrence rate in binary systems is only `S_bin` = 0.34 (+0.14 −0.15) times that of
/// wider binaries or single stars".
pub const CLOSE_BINARY_SUPPRESSION: f64 = 0.34;

/// The binary semi-major axis inside which a host counts as in a close binary: 47 au, plan 14's
/// "about 50 au" (Kraus et al. 2016, abstract: `a_cut` = 47 (+59 −23) au).
///
/// The caller compares against it (P14.T9.c); [`ClassConstraints`] takes the answer.
pub const CLOSE_BINARY_CUTOFF_AU: f64 = 47.0;

/// Words of the [`tags::PLANET_CLASS`] stream that one orbit host owns.
///
/// Host h reads word 4h, and words 4h + 1 to 4h + 3 are reserved. Changing it moves every host
/// after the first, which is a generator-version change.
pub const CLASS_WORDS_PER_HOST: u64 = 4;

/// How a class's weight scales with its host's mass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MassScaling {
    /// Independent of the host's mass.
    Flat,
    /// (M ÷ M☉)^exponent.
    PowerLaw {
        /// The exponent.
        exponent: f64,
    },
    /// g(M): M ÷ M☉ up to [`GIANT_HOST_PEAK`], then its value there times Reffert et al.'s
    /// Gaussian fall of width [`GIANT_HOST_FALL_WIDTH`] ([`giant_host_mass_scaling`]).
    GiantHost,
    /// The summed weights, before metallicity, of the stellar rows with small-planet metallicity
    /// scaling, at [`SUBSTELLAR_LIMIT`]: `SubstellarCompact`'s, continuous with the stars'.
    SmallPlanetsAtSubstellarLimit,
}

/// How a class's weight scales with its host's \[Fe/H\].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MetallicityScaling {
    /// Independent of \[Fe/H\], but taking up what the other classes lose in the halo: `Barren`'s.
    TakesHaloLoss,
    /// s(\[Fe/H\]) ([`small_planet_metallicity_scaling`]).
    SmallPlanet,
    /// z(\[Fe/H\]) ([`giant_metallicity_scaling`]).
    GiantPlanet,
}

/// Which hosts a row applies to; its weight is zero for the others.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Hosts {
    /// Every host.
    All,
    /// Hosts of [`SUBSTELLAR_LIMIT`] and above.
    Stellar,
    /// Hosts below [`SUBSTELLAR_LIMIT`].
    Substellar,
}

impl Hosts {
    /// Whether the row applies to a host of mass `mass`.
    #[must_use]
    pub fn admit(self, mass: SolarMasses) -> bool {
        let substellar = mass < SUBSTELLAR_LIMIT;
        match self {
            Self::All => true,
            Self::Stellar => !substellar,
            Self::Substellar => substellar,
        }
    }
}

/// One row of [`ARCHITECTURE_TABLE`]: a class's weight w₀ × (mass scaling) × (metallicity
/// scaling), for the hosts it applies to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClassRow {
    class: ArchitectureClass,
    base_weight: f64,
    mass_scaling: MassScaling,
    metallicity_scaling: MetallicityScaling,
    hosts: Hosts,
}

impl ClassRow {
    /// The class.
    #[must_use]
    pub const fn class(&self) -> ArchitectureClass {
        self.class
    }

    /// w₀, the weight at 1 M☉ and \[Fe/H\] = 0 before the halo's transfer.
    #[must_use]
    pub const fn base_weight(&self) -> f64 {
        self.base_weight
    }

    /// How the weight scales with the host's mass.
    #[must_use]
    pub const fn mass_scaling(&self) -> MassScaling {
        self.mass_scaling
    }

    /// How the weight scales with the host's \[Fe/H\].
    #[must_use]
    pub const fn metallicity_scaling(&self) -> MetallicityScaling {
        self.metallicity_scaling
    }

    /// Which hosts the row applies to.
    #[must_use]
    pub const fn hosts(&self) -> Hosts {
        self.hosts
    }
}

/// The written frequency model (P14.T4.b), one row per class in [`ArchitectureClass::ALL`]'s
/// order; the sources of each row are in the [module documentation](self).
pub const ARCHITECTURE_TABLE: &[ClassRow; CLASS_COUNT] = &[
    ClassRow {
        class: ArchitectureClass::Barren,
        base_weight: 0.24,
        mass_scaling: MassScaling::Flat,
        metallicity_scaling: MetallicityScaling::TakesHaloLoss,
        hosts: Hosts::All,
    },
    ClassRow {
        class: ArchitectureClass::TerrestrialOnly,
        base_weight: 0.35,
        mass_scaling: MassScaling::Flat,
        metallicity_scaling: MetallicityScaling::SmallPlanet,
        hosts: Hosts::Stellar,
    },
    ClassRow {
        class: ArchitectureClass::CompactMulti,
        base_weight: 0.21,
        mass_scaling: MassScaling::PowerLaw {
            exponent: COMPACT_MASS_EXPONENT,
        },
        metallicity_scaling: MetallicityScaling::SmallPlanet,
        hosts: Hosts::Stellar,
    },
    ClassRow {
        class: ArchitectureClass::CompactWithColdGiant,
        base_weight: 0.10 * GIANT_WEIGHT_SCALE,
        mass_scaling: MassScaling::GiantHost,
        metallicity_scaling: MetallicityScaling::GiantPlanet,
        hosts: Hosts::Stellar,
    },
    ClassRow {
        class: ArchitectureClass::SolarLike,
        base_weight: 0.01 * GIANT_WEIGHT_SCALE,
        mass_scaling: MassScaling::GiantHost,
        metallicity_scaling: MetallicityScaling::GiantPlanet,
        hosts: Hosts::Stellar,
    },
    ClassRow {
        class: ArchitectureClass::EccentricGiant,
        base_weight: 0.04 * GIANT_WEIGHT_SCALE,
        mass_scaling: MassScaling::GiantHost,
        metallicity_scaling: MetallicityScaling::GiantPlanet,
        hosts: Hosts::Stellar,
    },
    ClassRow {
        class: ArchitectureClass::WarmGiant,
        base_weight: 0.02 * GIANT_WEIGHT_SCALE,
        mass_scaling: MassScaling::GiantHost,
        metallicity_scaling: MetallicityScaling::GiantPlanet,
        hosts: Hosts::Stellar,
    },
    ClassRow {
        class: ArchitectureClass::HotJupiter,
        base_weight: HOT_JUPITER_WEIGHT,
        mass_scaling: MassScaling::GiantHost,
        metallicity_scaling: MetallicityScaling::GiantPlanet,
        hosts: Hosts::Stellar,
    },
    ClassRow {
        class: ArchitectureClass::SubstellarCompact,
        base_weight: 1.0,
        mass_scaling: MassScaling::SmallPlanetsAtSubstellarLimit,
        metallicity_scaling: MetallicityScaling::SmallPlanet,
        hosts: Hosts::Substellar,
    },
];

/// g(M), the giant classes' mass scaling: M ÷ M☉ up to [`GIANT_HOST_PEAK`] (Johnson et al. 2010,
/// PASP 122, 905, eq. 8), and above it 1.9 exp(−½ ((M − 1.9 M☉) ÷ 0.5 M☉)²) (Reffert et al. 2015,
/// A&A 574, A116, eq. 3).
///
/// # Panics
///
/// In debug builds, if `mass` is not positive and finite.
#[must_use]
pub fn giant_host_mass_scaling(mass: SolarMasses) -> f64 {
    let m = mass.value();
    debug_assert!(
        m.is_finite() && m > 0.0,
        "a host mass is positive and finite, got {m}"
    );
    let peak = GIANT_HOST_PEAK.value();
    if m <= peak {
        m
    } else {
        let x = (m - peak) / GIANT_HOST_FALL_WIDTH.value();
        peak * math::exp(-0.5 * x * x)
    }
}

/// s(\[Fe/H\]) = 1 ÷ (1 + 10^(−2 (\[Fe/H\] + 1.5))), the small-planet classes' metallicity scaling:
/// flat through the discs, a half at −1.5 and a tenth at −2.
///
/// # Panics
///
/// In debug builds, if `fe_h` is not finite.
#[must_use]
pub fn small_planet_metallicity_scaling(fe_h: Dex) -> f64 {
    let x = fe_h.value();
    debug_assert!(x.is_finite(), "[Fe/H] is finite, got {x}");
    let below_knee = -SMALL_PLANET_METALLICITY_SLOPE * (x - SMALL_PLANET_METALLICITY_KNEE.value());
    1.0 / (1.0 + math::exp10(below_knee))
}

/// z(\[Fe/H\]) before the halo's thinning: 10^(2 \[Fe/H\]) with \[Fe/H\] held to −0.5 to +0.5.
#[must_use]
fn giant_metallicity_unthinned(fe_h: Dex) -> f64 {
    let x = fe_h.value().clamp(
        GIANT_METALLICITY_FLOOR.value(),
        GIANT_METALLICITY_CEILING.value(),
    );
    math::exp10(GIANT_METALLICITY_EXPONENT * x)
}

/// z(\[Fe/H\]), the giant classes' metallicity scaling: 10^(2 \[Fe/H\]) on −0.5 to +0.5
/// (Fischer and Valenti 2005, ApJ 622, 1102), held at 10 above, and below −0.5 held at 0.1 and
/// thinned with the small planets, 0.1 s(\[Fe/H\]) ÷ s(−0.5).
///
/// # Panics
///
/// In debug builds, if `fe_h` is not finite.
#[must_use]
pub fn giant_metallicity_scaling(fe_h: Dex) -> f64 {
    debug_assert!(
        fe_h.value().is_finite(),
        "[Fe/H] is finite, got {}",
        fe_h.value()
    );
    let held = giant_metallicity_unthinned(fe_h);
    if fe_h < GIANT_METALLICITY_FLOOR {
        held * small_planet_metallicity_scaling(fe_h)
            / small_planet_metallicity_scaling(GIANT_METALLICITY_FLOOR)
    } else {
        held
    }
}

/// A row's mass scaling for a host of `mass`.
#[must_use]
fn mass_factor(scaling: MassScaling, mass: SolarMasses) -> f64 {
    match scaling {
        MassScaling::Flat => 1.0,
        MassScaling::PowerLaw { exponent } => math::powf(mass.value(), exponent),
        MassScaling::GiantHost => giant_host_mass_scaling(mass),
        MassScaling::SmallPlanetsAtSubstellarLimit => ARCHITECTURE_TABLE
            .iter()
            .filter(|row| {
                row.hosts == Hosts::Stellar
                    && row.metallicity_scaling == MetallicityScaling::SmallPlanet
            })
            .fold(0.0, |sum, row| {
                sum + row.base_weight * mass_factor(row.mass_scaling, SUBSTELLAR_LIMIT)
            }),
    }
}

/// The weight of each class, indexed by [`ArchitectureClass::index`].
///
/// Weights are relative: the probability of a class is its weight over their sum. They come from
/// [`class_weights`], and [`constrained`](Self::constrained) applies a host's disc and binarity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClassWeights([f64; CLASS_COUNT]);

impl ClassWeights {
    /// The weights, in [`ArchitectureClass::ALL`]'s order.
    #[must_use]
    pub const fn as_array(&self) -> &[f64; CLASS_COUNT] {
        &self.0
    }

    /// One class's weight.
    #[must_use]
    pub const fn get(&self, class: ArchitectureClass) -> f64 {
        self.0[class.index()]
    }

    /// The sum of the weights, in class order: the bound of the pick.
    #[must_use]
    pub fn total(&self) -> f64 {
        self.0.iter().fold(0.0, |sum, &w| sum + w)
    }

    /// The summed weight of the classes with giants, in class order.
    #[must_use]
    pub fn giant_total(&self) -> f64 {
        ArchitectureClass::ALL
            .iter()
            .filter(|class| class.has_giants())
            .fold(0.0, |sum, &class| sum + self.get(class))
    }

    /// Each class's probability: its weight over [`total`](Self::total).
    #[must_use]
    pub fn probabilities(&self) -> ClassProbabilities {
        let total = self.total();
        ClassProbabilities(self.0.map(|w| w / total))
    }

    /// The weights after a host's constraints (design notes 5 and 10): everything to `Barren`
    /// without a disc; each giant class to its [giant-free
    /// sibling](ArchitectureClass::giant_free_sibling) when the disc cannot form giants; and, in a
    /// close binary, [`CLOSE_BINARY_SUPPRESSION`] of every other class's weight kept and the rest
    /// given to `Barren`.
    ///
    /// The total is unchanged but for rounding.
    #[must_use]
    pub fn constrained(&self, constraints: &ClassConstraints) -> Self {
        let barren = ArchitectureClass::Barren.index();
        let mut weights = self.0;
        match constraints.capacity {
            DiscCapacity::None => {
                let mut weights = [0.0; CLASS_COUNT];
                weights[barren] = self.total();
                return Self(weights);
            }
            DiscCapacity::Giants => {}
            DiscCapacity::SmallPlanets => {
                for class in ArchitectureClass::ALL {
                    if class.has_giants() {
                        let moved = weights[class.index()];
                        weights[class.giant_free_sibling().index()] += moved;
                        weights[class.index()] = 0.0;
                    }
                }
            }
        }
        if constraints.multiplicity == HostMultiplicity::CloseBinary {
            let mut lost = 0.0;
            for class in ArchitectureClass::ALL {
                if class != ArchitectureClass::Barren {
                    let i = class.index();
                    let kept = weights[i] * CLOSE_BINARY_SUPPRESSION;
                    lost += weights[i] - kept;
                    weights[i] = kept;
                }
            }
            weights[barren] += lost;
        }
        Self(weights)
    }
}

/// Each class's probability, indexed by [`ArchitectureClass::index`]; they sum to 1 to rounding.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClassProbabilities([f64; CLASS_COUNT]);

impl ClassProbabilities {
    /// The probabilities, in [`ArchitectureClass::ALL`]'s order.
    #[must_use]
    pub const fn as_array(&self) -> &[f64; CLASS_COUNT] {
        &self.0
    }

    /// One class's probability.
    #[must_use]
    pub const fn get(&self, class: ArchitectureClass) -> f64 {
        self.0[class.index()]
    }

    /// The probability of a class with giants.
    #[must_use]
    pub fn giant_share(&self) -> f64 {
        ArchitectureClass::ALL
            .iter()
            .filter(|class| class.has_giants())
            .fold(0.0, |sum, &class| sum + self.get(class))
    }

    /// The probability of one of the two compact classes of stellar hosts, `CompactMulti` and
    /// `CompactWithColdGiant`.
    #[must_use]
    pub fn compact_share(&self) -> f64 {
        self.get(ArchitectureClass::CompactMulti)
            + self.get(ArchitectureClass::CompactWithColdGiant)
    }
}

/// The early M dwarfs of Dressing and Charbonneau's (2015) sample whose systems ruling 85.4 sets:
/// 0.35–0.6 M☉, blended to the ordinary law over 0.30–0.35 and 0.60–0.70 M☉.
pub const EARLY_M_DWARF_MASSES: (SolarMasses, SolarMasses) =
    (SolarMasses::new(0.35), SolarMasses::new(0.6));

/// The ends of the blends about [`EARLY_M_DWARF_MASSES`]: 0.30 and 0.70 M☉, this module's choice,
/// so that no statistic steps at a host mass.
pub const EARLY_M_DWARF_BLEND: (SolarMasses, SolarMasses) =
    (SolarMasses::new(0.30), SolarMasses::new(0.70));

/// How far a host of `mass` takes the early M dwarfs' systems (ruling 85.4): 1 inside
/// [`EARLY_M_DWARF_MASSES`], 0 outside [`EARLY_M_DWARF_BLEND`], and linear in ln M between.
///
/// Ballard and Johnson (2016, ApJ 816, 66, §3.3) find 45% of Kepler's planet-hosting M dwarfs with
/// about five coplanar planets inside 200 days and 55% single or inclined, and Hsu, Ford and
/// Terrien (2020, MNRAS 498, 2249, §5) the planets consistent with every early M dwarf hosting a
/// system. Inside the range, then, `Barren` and `TerrestrialOnly` take no weight (the table's
/// giant classes keep theirs), the compact chain's hot variant takes 55% of chains
/// ([`EARLY_M_DWARF_HOT_VARIANT_PROBABILITY`]) with the cold chain's count
/// ([`EARLY_M_DWARF_HOT_VARIANT_COUNT`]; ruling 87.2), and its cold chain places about five
/// planets inside 200 days ([`CHAIN_COUNT`]'s factor), starting closer in
/// ([`EARLY_M_DWARF_FIRST_PERIOD_SCALE`]).
///
/// [`EARLY_M_DWARF_HOT_VARIANT_PROBABILITY`]: template::EARLY_M_DWARF_HOT_VARIANT_PROBABILITY
/// [`EARLY_M_DWARF_HOT_VARIANT_COUNT`]: template::EARLY_M_DWARF_HOT_VARIANT_COUNT
/// [`CHAIN_COUNT`]: template::CHAIN_COUNT
/// [`EARLY_M_DWARF_FIRST_PERIOD_SCALE`]: template::EARLY_M_DWARF_FIRST_PERIOD_SCALE
///
/// # Examples
///
/// ```
/// use hyperion_sim::planetary::architecture::early_m_dwarf_share;
/// use hyperion_sim::units::SolarMasses;
///
/// assert_eq!(early_m_dwarf_share(SolarMasses::new(0.45)), 1.0);
/// assert_eq!(early_m_dwarf_share(SolarMasses::new(1.0)), 0.0);
/// let edge = early_m_dwarf_share(SolarMasses::new(0.65));
/// assert!(edge > 0.0 && edge < 1.0);
/// ```
#[must_use]
pub fn early_m_dwarf_share(mass: SolarMasses) -> f64 {
    let (m, (inner, outer), (low, high)) =
        (mass.value(), EARLY_M_DWARF_MASSES, EARLY_M_DWARF_BLEND);
    let across = |from: SolarMasses, to: SolarMasses| {
        ((math::ln(m) - math::ln(from.value())) / (math::ln(to.value()) - math::ln(from.value())))
            .clamp(0.0, 1.0)
    };
    if m < inner.value() {
        across(low, inner)
    } else if m <= outer.value() {
        1.0
    } else {
        1.0 - across(outer, high)
    }
}

/// The class weights of a host of initial mass `mass` and metallicity `fe_h` (P14.T4.b): the rows
/// of [`ARCHITECTURE_TABLE`] that apply to the host, with `Barren` taking what the halo takes from
/// the others.
///
/// # Panics
///
/// In debug builds, if `mass` is not positive and finite or `fe_h` is not finite.
///
/// # Examples
///
/// A Sun-like star whose disc can form giants has a compact inner system about a third of the
/// time and a giant more than a quarter of the time (a fifth after design note 5's fallbacks); an
/// M dwarf is dominated by compact systems and seldom has a giant.
///
/// ```
/// use hyperion_sim::planetary::architecture::{ArchitectureClass, class_weights};
/// use hyperion_sim::units::{Dex, SolarMasses};
///
/// let sun = class_weights(SolarMasses::new(1.0), Dex::new(0.0)).probabilities();
/// assert!((0.27..0.35).contains(&sun.compact_share()));
/// assert!((0.25..0.30).contains(&sun.giant_share()));
///
/// let m_dwarf = class_weights(SolarMasses::new(0.3), Dex::new(0.0)).probabilities();
/// assert!(m_dwarf.compact_share() > 0.45 && m_dwarf.giant_share() < 0.05);
/// assert!(m_dwarf.get(ArchitectureClass::SubstellarCompact) <= 0.0);
/// ```
#[must_use]
pub fn class_weights(mass: SolarMasses, fe_h: Dex) -> ClassWeights {
    debug_assert!(
        mass.value().is_finite() && mass.value() > 0.0,
        "a host mass is positive and finite, got {}",
        mass.value()
    );
    let small = small_planet_metallicity_scaling(fe_h);
    let giant = giant_metallicity_scaling(fe_h);
    let giant_unthinned = giant_metallicity_unthinned(fe_h);
    let mut weights = [0.0; CLASS_COUNT];
    let mut halo_loss = 0.0;
    let mut barren_rows = 0.0;
    for row in ARCHITECTURE_TABLE {
        if !row.hosts.admit(mass) {
            continue;
        }
        let mut raw = row.base_weight * mass_factor(row.mass_scaling, mass);
        // Ruling 85.4: every early M dwarf hosts a system.
        if matches!(
            row.class,
            ArchitectureClass::Barren | ArchitectureClass::TerrestrialOnly
        ) {
            raw *= 1.0 - early_m_dwarf_share(mass);
        }
        let i = row.class.index();
        match row.metallicity_scaling {
            MetallicityScaling::TakesHaloLoss => barren_rows += raw,
            MetallicityScaling::SmallPlanet => {
                weights[i] = raw * small;
                halo_loss += raw * (1.0 - small);
            }
            MetallicityScaling::GiantPlanet => {
                weights[i] = raw * giant;
                halo_loss += raw * (giant_unthinned - giant);
            }
        }
    }
    weights[ArchitectureClass::Barren.index()] = barren_rows + halo_loss;
    ClassWeights(weights)
}

/// Whether an orbit host is in a close binary, which suppresses its planets (design note 10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HostMultiplicity {
    /// A single star, or a component of a binary wider than [`CLOSE_BINARY_CUTOFF_AU`], or a
    /// circumbinary host.
    SingleOrWide,
    /// A component of a binary closer than [`CLOSE_BINARY_CUTOFF_AU`].
    CloseBinary,
}

/// Where the stable zone an orbit host's disc lies in ends (P14.T9), which may cut the disc's
/// reach inside its own outer edge.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum ZoneLimit {
    /// No zone ends inside the disc: a single star's disc reaches its own edge.
    Unbounded,
    /// The zone ends at this radius.
    Outer(Metres),
}

/// What a host's disc can make (design note 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiscCapacity {
    /// No disc: nothing forms.
    None,
    /// A disc that cannot form a giant: its snow line lies beyond its reach, or it has under
    /// [`GIANT_SOLID_BUDGET`] of solids between them.
    SmallPlanets,
    /// A disc that can form giants.
    Giants,
}

/// What a host's disc, zone and binarity allow of its class (design notes 5 and 10): the plain
/// arguments of [`ClassWeights::constrained`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClassConstraints {
    capacity: DiscCapacity,
    multiplicity: HostMultiplicity,
}

impl ClassConstraints {
    /// No constraint: a disc that can form giants, around a single host.
    pub const NONE: Self = Self {
        capacity: DiscCapacity::Giants,
        multiplicity: HostMultiplicity::SingleOrWide,
    };

    /// The constraints of a host with disc `disc`, in a stable zone ending at `zone`, and of
    /// binarity `multiplicity`.
    ///
    /// Giants are possible when the snow line lies inside the disc's reach, the nearer of its
    /// outer edge and the zone's end, and the solids between them are at least
    /// [`GIANT_SOLID_BUDGET`].
    ///
    /// # Panics
    ///
    /// In debug builds, if the zone's end is not positive.
    #[must_use]
    pub fn new(disc: &Disc, zone: ZoneLimit, multiplicity: HostMultiplicity) -> Self {
        let capacity = match disc {
            Disc::None => DiscCapacity::None,
            Disc::Present(profile) => {
                let edge = profile.outer_edge();
                let reach = match zone {
                    ZoneLimit::Unbounded => edge,
                    ZoneLimit::Outer(end) => {
                        debug_assert!(end.value() > 0.0, "a zone's end is positive, got {end:?}");
                        if end < edge { end } else { edge }
                    }
                };
                let snow = profile.snow_line();
                if snow < reach && profile.solid_mass_between(snow, reach) >= GIANT_SOLID_BUDGET {
                    DiscCapacity::Giants
                } else {
                    DiscCapacity::SmallPlanets
                }
            }
        };
        Self {
            capacity,
            multiplicity,
        }
    }

    /// What the host's disc can make.
    #[must_use]
    pub const fn capacity(&self) -> DiscCapacity {
        self.capacity
    }

    /// The host's binarity.
    #[must_use]
    pub const fn multiplicity(&self) -> HostMultiplicity {
        self.multiplicity
    }
}

/// An orbit host's class draw: one [`Mark`] on [`tags::PLANET_CLASS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClassDraw {
    mark: Mark,
}

impl ClassDraw {
    /// The draw of orbit host number `host` of `system`, in the universe of `seed`: word
    /// [`CLASS_WORDS_PER_HOST`] × `host` of `system`'s [`tags::PLANET_CLASS`] stream.
    ///
    /// A single star is host 0, as for its disc; the numbering of a multiple system's hosts is
    /// P14.T9's.
    #[must_use]
    pub fn for_host(seed: Seed, system: SystemId, host: u8) -> Self {
        let stream = Stream::open(seed, tags::PLANET_CLASS, ObjectKey::from(system));
        Self {
            mark: Mark::from_word(stream.word_at(u64::from(host) * CLASS_WORDS_PER_HOST)),
        }
    }

    /// A draw of a given mark, for tests and tools.
    #[must_use]
    pub const fn from_mark(mark: Mark) -> Self {
        Self { mark }
    }

    /// The mark.
    #[must_use]
    pub const fn mark(&self) -> Mark {
        self.mark
    }

    /// The class this draw picks against `weights`: the first class whose running share of the
    /// weights lies above the mark, with the weights' own total as the bound, so that no mark is
    /// rejected.
    ///
    /// # Panics
    ///
    /// If the weights' total is not positive and finite, in every build. [`class_weights`] gives
    /// such weights only for a host mass that is not positive and finite or an \[Fe/H\] that is
    /// not finite, which it asserts against in debug builds; otherwise `Barren`'s weight is at
    /// least 0.24, and [`ClassWeights::constrained`] keeps the total.
    #[must_use]
    pub fn class(&self, weights: &ClassWeights) -> ArchitectureClass {
        let index = self
            .mark
            .pick_weighted(weights.as_array(), weights.total())
            .expect(
                "the bound is the weights' own positive total (Barren's weight is at least 0.24), \
                 so the last threshold accepts every mark",
            );
        ArchitectureClass::ALL[index]
    }
}

/// The architecture class of orbit host `host` of `system` (P14.T4.c): its [`ClassDraw`] picked
/// against `weights` after `constraints`.
///
/// # Panics
///
/// As [`ClassDraw::class`]: if the weights' total is not positive and finite.
///
/// # Examples
///
/// A Sun-like star with no disc is barren, whatever its mark.
///
/// ```
/// use hyperion_sim::Seed;
/// use hyperion_sim::id::SystemId;
/// use hyperion_sim::planetary::architecture::{
///     ArchitectureClass, ClassConstraints, HostMultiplicity, ZoneLimit, class_weights, draw_class,
/// };
/// use hyperion_sim::planetary::disc::Disc;
/// use hyperion_sim::units::{Dex, SolarMasses};
///
/// let system = SystemId::from_raw(0x0200_0800_2000_0000)?;
/// let weights = class_weights(SolarMasses::new(1.0), Dex::new(0.0));
/// let no_disc =
///     ClassConstraints::new(&Disc::None, ZoneLimit::Unbounded, HostMultiplicity::SingleOrWide);
/// let class = draw_class(Seed::new(3), system, 0, &weights, &no_disc);
/// assert_eq!(class, ArchitectureClass::Barren);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn draw_class(
    seed: Seed,
    system: SystemId,
    host: u8,
    weights: &ClassWeights,
    constraints: &ClassConstraints,
) -> ArchitectureClass {
    ClassDraw::for_host(seed, system, host).class(&weights.constrained(constraints))
}

#[cfg(test)]
mod tests {
    use hyperion_testkit::float::assert_same_bits;
    use hyperion_testkit::order::assert_order_independent;
    use hyperion_testkit::stats::{ALPHA, assert_p_value, chi_square_gof, normal_cdf};

    use super::template::{Location, PeriodLaw};
    use super::*;
    use crate::coords::{CellSize, GenCell};
    use crate::id::BodyId;
    use crate::id::Layer;
    use crate::planetary::disc::{self, DiscDraws, DiscHost, Truncation, snow_line};
    use crate::planetary::placement::masses::giant_core;
    use crate::rng::Thresholds;
    use crate::stellar::Composition;
    use crate::stellar::draws::StarDraws;
    use crate::stellar::premain::disc_lifetime;
    use crate::stellar::sse::{ZCoeffs, zams};
    use crate::units::consts::{GM_SUN, SECONDS_PER_DAY};
    use crate::units::{AstronomicalUnits, HeliumExcess, Megayears, SolarLuminosities, SolarRadii};

    const SEED: Seed = Seed::new(0x5eed_0000_0014_0004);

    fn system(index: u32) -> SystemId {
        let cell = GenCell::new(CellSize::Ly8, [-7, 22, 1]).unwrap();
        SystemId::from_parts(Layer::A, cell, index).unwrap()
    }

    fn weights(mass: f64, fe_h: f64) -> ClassWeights {
        class_weights(SolarMasses::new(mass), Dex::new(fe_h))
    }

    fn probabilities(mass: f64, fe_h: f64) -> ClassProbabilities {
        weights(mass, fe_h).probabilities()
    }

    /// Masses from 0.08 to 150 M☉, geometric, both ends included.
    fn stellar_masses() -> Vec<f64> {
        let (lo, hi, n) = (math::ln(0.08), math::ln(150.0), 60);
        (0..=n)
            .map(|i| math::exp(lo + (hi - lo) * f64::from(i) / f64::from(n)))
            .collect()
    }

    /// \[Fe/H\] from −2.5 to +0.5 in steps of 0.05.
    fn metallicities() -> Vec<f64> {
        (0..=60).map(|i| -2.5 + 0.05 * f64::from(i)).collect()
    }

    fn relative(a: f64, b: f64) -> f64 {
        ((a - b) / b).abs()
    }

    #[test]
    fn early_m_dwarfs_all_host_a_system() {
        use crate::planetary::architecture::template::{
            EARLY_M_DWARF_HOT_VARIANT_PROBABILITY, PeriodLaw, template,
        };
        use crate::units::Days;
        let share = |m: f64| early_m_dwarf_share(SolarMasses::new(m));
        assert_same_bits(share(0.30), 0.0);
        assert_same_bits(share(0.35), 1.0);
        assert_same_bits(share(0.6), 1.0);
        assert_same_bits(share(0.70), 0.0);
        assert!(share(0.32) > 0.0 && share(0.32) < 1.0);
        for fe_h in [-0.5, 0.0, 0.3] {
            let p = class_weights(SolarMasses::new(0.45), Dex::new(fe_h)).probabilities();
            assert_same_bits(p.get(ArchitectureClass::TerrestrialOnly), 0.0);
            let barren = p.get(ArchitectureClass::Barren);
            assert!(
                barren < 0.03,
                "only the metallicity losses are barren: {barren}"
            );
            assert!(p.compact_share() > 0.9, "{}", p.compact_share());
        }
        // A Sun is untouched.
        let sun = class_weights(SolarMasses::new(1.0), Dex::ZERO).probabilities();
        assert!(sun.get(ArchitectureClass::Barren) > 0.1);
        let hot = template(ArchitectureClass::CompactMulti).groups()[0]
            .hot_variant()
            .expect("a chain has a hot variant");
        assert_same_bits(
            hot.probability_about(SolarMasses::new(1.0)),
            hot.probability(),
        );
        assert_same_bits(
            hot.probability_about(SolarMasses::new(0.45)),
            EARLY_M_DWARF_HOT_VARIANT_PROBABILITY,
        );
        let law = PeriodLaw::BrokenPowerLaw {
            break_period: Days::new(12.0),
            rising: 1.6,
            falling: -0.9,
            min: Days::new(1.0),
            max: Days::new(50.0),
        };
        let PeriodLaw::BrokenPowerLaw {
            break_period,
            min,
            max,
            ..
        } = law.with_break_scaled(0.45)
        else {
            panic!("a broken power law stays one");
        };
        assert!((break_period.value() - 5.4).abs() < 1e-12);
        assert_same_bits(min.value(), 1.0);
        assert_same_bits(max.value(), 50.0);
    }

    #[test]
    fn the_table_has_one_row_per_class_in_class_order() {
        assert_eq!(ARCHITECTURE_TABLE.len(), CLASS_COUNT);
        for (i, (row, class)) in ARCHITECTURE_TABLE
            .iter()
            .zip(ArchitectureClass::ALL)
            .enumerate()
        {
            assert_eq!(row.class(), class);
            assert_eq!(class.index(), i);
        }
        // At 1 M☉ and solar metallicity the stellar rows' w₀ sum to 1.102.
        let stellar: f64 = ARCHITECTURE_TABLE
            .iter()
            .filter(|row| row.hosts() != Hosts::Substellar)
            .map(ClassRow::base_weight)
            .sum();
        assert!((stellar - 1.101_87).abs() < 1e-12, "{stellar}");
    }

    /// P14.T4.c: the probabilities sum to 1 over masses 0.08–150 M☉ and \[Fe/H\] −2.5 to +0.5.
    #[test]
    fn probabilities_sum_to_one_over_the_grid() {
        for m in stellar_masses().into_iter().chain([0.01, 0.05, 0.079]) {
            for x in metallicities() {
                let p = probabilities(m, x);
                let sum: f64 = p.as_array().iter().sum();
                assert!((sum - 1.0).abs() < 1e-12, "m = {m}, [Fe/H] = {x}: {sum}");
                assert!(p.as_array().iter().all(|&q| (0.0..=1.0).contains(&q)));
            }
        }
    }

    /// P14.T4.c, as the corrected table gives it: the giant classes' summed weight is
    /// 10^(2 Δ\[Fe/H\]) on −0.5 to +0.5, constant above +0.5, and thinned by s below −0.5.
    #[test]
    fn the_giant_weight_scales_as_ten_to_twice_the_metallicity_in_the_fitted_range() {
        for m in [0.3, 1.0, 1.9, 2.6] {
            let solar = weights(m, 0.0).giant_total();
            for i in 0..=40 {
                let x = -0.5 + 0.025 * f64::from(i);
                let ratio = weights(m, x).giant_total() / solar;
                assert!(
                    relative(ratio, math::exp10(2.0 * x)) < 1e-12,
                    "m = {m}, [Fe/H] = {x}: {ratio}"
                );
            }
            let top = weights(m, 0.5).giant_total();
            for x in [0.51, 0.7, 1.5] {
                assert!(relative(weights(m, x).giant_total(), top) < 1e-15);
            }
            let floor = weights(m, -0.5).giant_total();
            let s_floor = small_planet_metallicity_scaling(Dex::new(-0.5));
            let mut previous = floor;
            for x in [-0.55, -0.8, -1.2, -1.5, -2.0, -2.5] {
                let giants = weights(m, x).giant_total();
                let thinned = floor * small_planet_metallicity_scaling(Dex::new(x)) / s_floor;
                assert!(relative(giants, thinned) < 1e-12, "m = {m}, [Fe/H] = {x}");
                assert!(giants < previous);
                previous = giants;
            }
        }
    }

    /// P14.T4.c: the log-slope of the giant share between −0.5 and −0.2 is 1.85–2.0 at 1 M☉
    /// (1.88 as built), and bends as the share saturates.
    #[test]
    fn the_giant_share_rises_with_a_log_slope_near_two() {
        let slope = |m: f64, a: f64, b: f64| {
            math::log10(probabilities(m, b).giant_share() / probabilities(m, a).giant_share())
                / (b - a)
        };
        let sun = slope(1.0, -0.5, -0.2);
        assert!((1.85..=2.0).contains(&sun), "{sun}");
        assert!((1.85..1.86).contains(&sun), "{sun}");
        assert!(slope(1.0, 0.2, 0.5) < sun, "the share saturates");
        let m_dwarf = slope(0.3, -0.5, -0.2);
        assert!((1.85..=2.0).contains(&m_dwarf), "{m_dwarf}");
    }

    /// P14.T4.c: at 0.3 M☉ the giants under 0.05 and the compact classes over 0.45; at 1 M☉ the
    /// compact classes 0.27–0.35. As built, in the table before design note 5's fallbacks: 0.011
    /// and 0.926 (with ruling 60's compact exponent of −3.0); at 1 M☉ giants 0.274 and compact
    /// 0.345. The plan's 0.14–0.20 for giants at 1 M☉
    /// is an observed rate, so it holds after the fallbacks, where the giants are 0.195
    /// (`the_anchors_hold_after_the_disc_fallback`, rulings 48 and 55.3).
    #[test]
    fn m_dwarfs_and_sun_like_stars_meet_their_brackets() {
        let m_dwarf = probabilities(0.3, 0.0);
        assert!(m_dwarf.giant_share() < 0.05, "{}", m_dwarf.giant_share());
        assert!(
            m_dwarf.compact_share() > 0.45,
            "{}",
            m_dwarf.compact_share()
        );
        assert!((0.010..0.012).contains(&m_dwarf.giant_share()));
        assert!((0.92..0.93).contains(&m_dwarf.compact_share()));
        let sun = probabilities(1.0, 0.0);
        assert!((0.27..=0.35).contains(&sun.compact_share()));
        assert!((0.27..0.28).contains(&sun.giant_share()));
        assert!((0.34..0.35).contains(&sun.compact_share()));
        // Before the fallbacks: hot Jupiters 1.15%, a cold giant in 45% of compact systems.
        let hot = sun.get(ArchitectureClass::HotJupiter);
        assert!((0.0114..0.0116).contains(&hot), "{hot}");
        let cold = sun.get(ArchitectureClass::CompactWithColdGiant) / sun.compact_share();
        assert!((0.44..0.45).contains(&cold), "{cold}");
    }

    /// P14.T4.c: the small-planet classes' weights at \[Fe/H\] = −0.8 are within 5% of solar and
    /// under a quarter of it at −2.
    #[test]
    fn small_planets_thin_only_in_the_halo() {
        let small = |m: f64, x: f64| {
            let w = weights(m, x);
            w.get(ArchitectureClass::TerrestrialOnly) + w.get(ArchitectureClass::CompactMulti)
        };
        for m in [0.2, 0.5, 1.0, 1.5] {
            let solar = small(m, 0.0);
            assert!(relative(small(m, -0.8), solar) < 0.05, "m = {m}");
            assert!(small(m, -2.0) < 0.25 * solar, "m = {m}");
        }
        let s = |x: f64| small_planet_metallicity_scaling(Dex::new(x));
        assert!((s(-1.5) - 0.5).abs() < 1e-15);
        assert!((s(-2.0) - 1.0 / 11.0).abs() < 1e-15);
    }

    /// What the halo takes from the other classes goes to `Barren`, so below −0.5 the total
    /// weight does not change with \[Fe/H\].
    #[test]
    fn the_halo_s_loss_goes_to_barren() {
        for m in [0.05, 0.3, 1.0, 1.9] {
            let total = weights(m, -0.5).total();
            for x in [-0.8, -1.5, -2.5, -4.0] {
                assert!(relative(weights(m, x).total(), total) < 1e-14, "m = {m}");
            }
            let barren = |x: f64| probabilities(m, x).get(ArchitectureClass::Barren);
            assert!(barren(-2.5) > barren(-1.5) && barren(-1.5) > barren(-0.5));
        }
        // In the halo giants thin with the small planets, keeping their ratio at −0.5 (0.134),
        // so they never outnumber them (held at −0.5 instead, they would: at −2.5, 3.0% of hosts
        // with a giant against 1.9% in the compact classes, before ruling 55.3's re-fit).
        let ratio = |x: f64| {
            let p = probabilities(1.0, x);
            p.giant_share() / p.compact_share()
        };
        assert!(ratio(-2.5) < 0.14);
        assert!(relative(ratio(-2.5), ratio(-0.5)) < 0.02);
    }

    #[test]
    fn g_rises_to_the_peak_and_falls_with_reffert_s_width() {
        let g = |m: f64| giant_host_mass_scaling(SolarMasses::new(m));
        assert_same_bits(g(0.5), 0.5);
        assert_same_bits(g(1.9), 1.9);
        // One width above the peak: exp(−½) of it.
        assert!(relative(g(2.4), 1.9 * math::exp(-0.5)) < 1e-15);
        // Reffert et al.'s half-peak at 2.6 M☉, to their 0.5 M☉.
        let half: f64 = 1.9 + 0.5 * (2.0 * math::ln(2.0)).sqrt();
        assert!((half - 2.6).abs() < 0.2, "{half}");
        assert!(g(3.5) < 0.01 * g(1.9));
        assert_same_bits(g(150.0), 0.0);
    }

    /// Design note 13: a host below 0.08 M☉ draws `Barren` or `SubstellarCompact`, with the
    /// small-planet classes' weight at the limit.
    #[test]
    fn substellar_hosts_draw_barren_or_a_substellar_chain() {
        for x in [-2.0, -0.5, 0.0, 0.4] {
            let w = weights(0.05, x);
            for class in ArchitectureClass::ALL {
                let expected_zero = !matches!(
                    class,
                    ArchitectureClass::Barren | ArchitectureClass::SubstellarCompact
                );
                assert_eq!(w.get(class) <= 0.0, expected_zero, "{class:?}");
            }
            let at_limit = weights(0.08, x);
            let small = at_limit.get(ArchitectureClass::TerrestrialOnly)
                + at_limit.get(ArchitectureClass::CompactMulti);
            assert!(relative(w.get(ArchitectureClass::SubstellarCompact), small) < 1e-14);
            assert!(relative(weights(0.0799, x).total(), w.total()) < 1e-15);
            // Stellar hosts never draw the substellar row.
            assert_same_bits(at_limit.get(ArchitectureClass::SubstellarCompact), 0.0);
        }
    }

    #[test]
    fn giant_free_siblings_have_no_giants() {
        for class in ArchitectureClass::ALL {
            let sibling = class.giant_free_sibling();
            assert!(!sibling.has_giants(), "{class:?}");
            assert_eq!(sibling == class, !class.has_giants(), "{class:?}");
            assert_eq!(sibling.giant_free_sibling(), sibling);
        }
    }

    /// A host with the zero-age state plan 06's fits give roughly, and the median draws.
    fn disc_of(mass: f64, fe_h: f64, truncation: Truncation) -> Disc {
        let host = DiscHost::new(
            SolarMasses::new(mass),
            Dex::new(fe_h),
            SolarLuminosities::new(0.70 * math::powi(mass, 4)),
            SolarRadii::new(0.89 * math::powf(mass, 0.8)),
        )
        .unwrap();
        disc::derive(&host, Megayears::new(2.5), &DiscDraws::MEDIAN, truncation)
    }

    fn au(x: f64) -> Metres {
        Metres::from(AstronomicalUnits::new(x))
    }

    fn capacity(disc: &Disc, zone: ZoneLimit) -> DiscCapacity {
        ClassConstraints::new(disc, zone, HostMultiplicity::SingleOrWide).capacity()
    }

    /// Design note 5: giants need the snow line inside the disc's reach and 10 M⊕ of solids
    /// between them.
    #[test]
    fn a_disc_that_cannot_form_giants_rules_them_out() {
        let open = ZoneLimit::Unbounded;
        let sun = disc_of(1.0, 0.0, Truncation::NONE);
        assert_eq!(capacity(&sun, open), DiscCapacity::Giants);
        // A zone that ends inside the 2.3 au snow line.
        let inside = ZoneLimit::Outer(au(2.0));
        assert_eq!(capacity(&sun, inside), DiscCapacity::SmallPlanets);
        // The same cut made by truncating the disc.
        let cut = disc_of(1.0, 0.0, Truncation::NONE.with_outer(au(2.0)));
        assert_eq!(capacity(&cut, open), DiscCapacity::SmallPlanets);
        // Just outside the snow line there are too few solids for a core.
        assert_eq!(
            capacity(&sun, ZoneLimit::Outer(au(3.0))),
            DiscCapacity::SmallPlanets
        );
        // A metal-poor M dwarf's disc has well under 10 M⊕ of solids, and the median disc of a
        // 0.3 M☉ star just under; a 0.5 M☉ star's median disc has enough.
        let poor = disc_of(0.2, -1.0, Truncation::NONE);
        assert_eq!(capacity(&poor, open), DiscCapacity::SmallPlanets);
        let m_dwarf = disc_of(0.3, 0.0, Truncation::NONE);
        assert_eq!(capacity(&m_dwarf, open), DiscCapacity::SmallPlanets);
        let early_m = disc_of(0.5, 0.0, Truncation::NONE);
        assert_eq!(capacity(&early_m, open), DiscCapacity::Giants);
        assert_eq!(capacity(&Disc::None, inside), DiscCapacity::None);
        assert_eq!(
            ClassConstraints::NONE,
            ClassConstraints::new(&sun, open, HostMultiplicity::SingleOrWide)
        );
    }

    /// Ruling 48's solar-disc sample: the discs of 20,000 systems' host 0, a 1 M☉ star of
    /// \[Fe/H\] = 0 at plan 06's zero-age state, as drawn on `planet.disc`, each living its star's
    /// own lifetime, P06.T15.c's law of its `star.disc_lifetime` rank (ruling 55.3); and the host's
    /// zero-age luminosity.
    fn solar_disc_sample() -> (Vec<Disc>, SolarLuminosities) {
        let mass = SolarMasses::new(1.0);
        let composition = Composition::from_fe_h(Dex::new(0.0), HeliumExcess::ZERO);
        let coeffs = ZCoeffs::new(composition.z_fit());
        let luminosity = zams::luminosity(mass, &coeffs);
        let host = DiscHost::new(
            mass,
            composition.fe_h(),
            luminosity,
            zams::radius(mass, &coeffs),
        )
        .unwrap();
        let cell = GenCell::new(CellSize::Ly8, [1, 2, 3]).unwrap();
        let discs = (0..20_000)
            .map(|i| {
                let id = SystemId::from_parts(Layer::A, cell, i).unwrap();
                let draws = DiscDraws::for_host(Seed::new(9), id, 0);
                let star = StarDraws::for_star(Seed::new(9), BodyId::new(id, 0));
                let lifetime = disc_lifetime(mass, star.disc_lifetime());
                disc::derive(&host, lifetime, &draws, Truncation::NONE)
            })
            .collect();
        (discs, luminosity)
    }

    /// `weights` with each giant class's weight moved to its giant-free sibling.
    fn without_giants(weights: &ClassWeights) -> ClassWeights {
        let mut moved = weights.0;
        for class in ArchitectureClass::ALL {
            if class.has_giants() {
                moved[class.giant_free_sibling().index()] += moved[class.index()];
                moved[class.index()] = 0.0;
            }
        }
        ClassWeights(moved)
    }

    /// Each class's probability after both of design note 5's fallbacks, averaged over `discs`:
    /// the solids beyond the snow line (P14.T4.c), then a giant's core grown within the disc's
    /// lifetime (P14.T7.c, applied by P14.T8's placer; ruling 55.3).
    fn after_fallback(weights: &ClassWeights, discs: &[Disc]) -> [f64; CLASS_COUNT] {
        let mut sum = [0.0; CLASS_COUNT];
        for disc in discs {
            let c =
                ClassConstraints::new(disc, ZoneLimit::Unbounded, HostMultiplicity::SingleOrWide);
            let mut kept = weights.constrained(&c);
            if !giant_core(disc, ZoneLimit::Unbounded).forms() {
                kept = without_giants(&kept);
            }
            let p = kept.probabilities();
            for (total, q) in sum.iter_mut().zip(p.as_array()) {
                *total += q;
            }
        }
        #[expect(
            clippy::cast_precision_loss,
            reason = "a sample of 20,000 is exact in f64"
        )]
        let n = discs.len() as f64;
        sum.map(|total| total / n)
    }

    /// The share of a class's systems whose innermost giant lies at 2–2,000 days around a 1 M☉
    /// host of zero-age luminosity `l`, Cumming et al.'s window, from its template's location.
    fn share_in_cumming_window(class: ArchitectureClass, l: SolarLuminosities) -> f64 {
        let Some(group) = template::template(class)
            .groups()
            .iter()
            .find(|g| g.places_giants())
        else {
            return 0.0;
        };
        let (lo, hi) = (math::ln(2.0), math::ln(2_000.0));
        // ln P in days of a circular orbit of `a_au` about 1 M☉.
        let ln_period = |a_au: f64| {
            let a = Metres::from(AstronomicalUnits::new(a_au)).value();
            math::ln(2.0 * core::f64::consts::PI * (a * a * a / GM_SUN).sqrt() / SECONDS_PER_DAY)
        };
        // The share of a law uniform in ln P on [a, b] that lies in the window.
        let overlap = |a: f64, b: f64| (b.min(hi) - a.max(lo)).max(0.0) / (b - a);
        let share = match group.location() {
            Location::Period(PeriodLaw::LogUniform { min, max }) => {
                overlap(math::ln(min.value()), math::ln(max.value()))
            }
            Location::Period(PeriodLaw::LogNormal {
                median,
                sigma_dex,
                min,
                max,
            }) => {
                let cdf =
                    |p: f64| normal_cdf((math::log10(p) - math::log10(median.value())) / sigma_dex);
                let (min, max) = (min.value(), max.value());
                (cdf(max.min(2_000.0)) - cdf(min.max(2.0))) / (cdf(max) - cdf(min))
            }
            Location::ScaledAu { inner, outer } => {
                let r = l.value().sqrt();
                overlap(ln_period(inner * r), ln_period(outer * r))
            }
            Location::SnowLines { inner, outer } => {
                let snow = AstronomicalUnits::from(snow_line(l)).value();
                overlap(ln_period(inner * snow), ln_period(outer * snow))
            }
            Location::Period(PeriodLaw::BrokenPowerLaw { .. })
            | Location::Outward
            | Location::Flanking => panic!("no giant group of {class:?} starts so"),
        };
        group.presence() * share
    }

    /// Ruling 48 (c): the anchors are observed rates, so they hold after design note 5's
    /// fallbacks, at 1 M☉ and \[Fe/H\] = 0, over the solar-disc sample; ruling 55.3 adds the
    /// second, a giant's core grown within the disc's lifetime (P14.T7.c). As built: 83.4% of the
    /// discs have the solids for giants and 71.1% grow a core in time; then giants of 0.3–10 M♃ at
    /// 2–2,000 days around 10.5% of hosts (Cumming et al. 2008), hot Jupiters around 0.82%, a cold
    /// giant in 30.7% of compact systems (Zhu and Wu 2018), and giants in all around 19.5%
    /// (Cumming et al.'s 17–20% within 20 au).
    #[test]
    fn the_anchors_hold_after_the_disc_fallback() {
        let (discs, l) = solar_disc_sample();
        let capable = discs
            .iter()
            .filter(|d| {
                ClassConstraints::new(d, ZoneLimit::Unbounded, HostMultiplicity::SingleOrWide)
                    .capacity()
                    == DiscCapacity::Giants
            })
            .count();
        assert!((16_500..16_900).contains(&capable), "{capable}");
        let cores = discs
            .iter()
            .filter(|d| giant_core(d, ZoneLimit::Unbounded).forms())
            .count();
        assert!((14_000..14_400).contains(&cores), "{cores}");
        let p = after_fallback(&weights(1.0, 0.0), &discs);
        let get = |class: ArchitectureClass| p[class.index()];
        let cumming: f64 = ArchitectureClass::ALL
            .iter()
            .map(|&class| get(class) * share_in_cumming_window(class, l))
            .sum();
        assert!((0.103..0.107).contains(&cumming), "{cumming}");
        let hot = get(ArchitectureClass::HotJupiter);
        assert!((0.0080..0.0084).contains(&hot), "{hot}");
        let cold = get(ArchitectureClass::CompactWithColdGiant)
            / (get(ArchitectureClass::CompactMulti) + get(ArchitectureClass::CompactWithColdGiant));
        assert!((0.30..0.36).contains(&cold), "{cold}");
        let giants: f64 = ArchitectureClass::ALL
            .iter()
            .filter(|class| class.has_giants())
            .map(|&class| get(class))
            .sum();
        assert!((0.17..=0.20).contains(&giants), "{giants}");
    }

    #[test]
    fn without_a_disc_every_host_is_barren() {
        let none = ClassConstraints::new(
            &Disc::None,
            ZoneLimit::Unbounded,
            HostMultiplicity::CloseBinary,
        );
        let w = weights(1.0, 0.0);
        let p = w.constrained(&none).probabilities();
        assert_same_bits(p.get(ArchitectureClass::Barren), 1.0);
        assert_same_bits(w.constrained(&none).total(), w.total());
        for word in [0, u64::MAX, 0x1234_5678_9abc_def0] {
            let draw = ClassDraw::from_mark(Mark::from_word(word));
            assert_eq!(draw.class(&w.constrained(&none)), ArchitectureClass::Barren);
        }
    }

    #[test]
    fn giant_classes_fall_back_to_their_giant_free_siblings() {
        let sun = disc_of(1.0, 0.0, Truncation::NONE);
        let no_giants = ClassConstraints::new(
            &sun,
            ZoneLimit::Outer(au(2.0)),
            HostMultiplicity::SingleOrWide,
        );
        for (m, x) in [(1.0, 0.0), (0.4, 0.3), (1.9, -0.2)] {
            let w = weights(m, x);
            let fallen = w.constrained(&no_giants);
            assert_same_bits(fallen.giant_total(), 0.0);
            assert!(relative(fallen.total(), w.total()) < 1e-15);
            let migrating = w.get(ArchitectureClass::CompactMulti)
                + w.get(ArchitectureClass::CompactWithColdGiant)
                + w.get(ArchitectureClass::WarmGiant)
                + w.get(ArchitectureClass::HotJupiter);
            let in_situ = w.get(ArchitectureClass::TerrestrialOnly)
                + w.get(ArchitectureClass::SolarLike)
                + w.get(ArchitectureClass::EccentricGiant);
            assert!(relative(fallen.get(ArchitectureClass::CompactMulti), migrating) < 1e-15);
            assert!(relative(fallen.get(ArchitectureClass::TerrestrialOnly), in_situ) < 1e-15);
            assert_same_bits(
                fallen.get(ArchitectureClass::Barren),
                w.get(ArchitectureClass::Barren),
            );
        }
    }

    /// Design note 10: a host in a close binary has planets 0.34 times as often.
    #[test]
    fn a_close_binary_moves_two_thirds_of_the_planets_weight_to_barren() {
        let sun = disc_of(1.0, 0.0, Truncation::NONE);
        let close =
            ClassConstraints::new(&sun, ZoneLimit::Unbounded, HostMultiplicity::CloseBinary);
        for (m, x) in [(1.0, 0.0), (0.3, 0.0), (0.05, -0.3)] {
            let w = weights(m, x);
            let single = w.probabilities();
            let binary = w.constrained(&close).probabilities();
            let planets = |p: &ClassProbabilities| 1.0 - p.get(ArchitectureClass::Barren);
            assert!(relative(planets(&binary), 0.34 * planets(&single)) < 1e-12);
            for class in ArchitectureClass::ALL {
                if class != ArchitectureClass::Barren && single.get(class) > 0.0 {
                    assert!(relative(binary.get(class), 0.34 * single.get(class)) < 1e-12);
                }
            }
        }
    }

    #[test]
    fn a_host_s_draw_is_word_four_h_of_the_system_stream() {
        let id = system(3);
        for host in [0_u8, 1, 2, 255] {
            let stream = Stream::open(SEED, tags::PLANET_CLASS, ObjectKey::from(id));
            let word = stream.word_at(4 * u64::from(host));
            assert_eq!(
                ClassDraw::for_host(SEED, id, host).mark(),
                Mark::from_word(word)
            );
        }
    }

    #[test]
    fn the_same_seed_gives_the_same_class_twice_in_any_order() {
        let w = weights(0.8, 0.1);
        let run = || draw_class(SEED, system(9), 2, &w, &ClassConstraints::NONE);
        assert_eq!(run(), run());
        let keys: Vec<(u32, u8)> = (0..8).flat_map(|s| (0..4).map(move |h| (s, h))).collect();
        assert_order_independent(&keys, |&(s, h)| {
            draw_class(SEED, system(s), h, &w, &ClassConstraints::NONE)
        });
        assert_ne!(
            ClassDraw::for_host(SEED, system(0), 0),
            ClassDraw::for_host(SEED, system(0), 1)
        );
        assert_ne!(
            ClassDraw::for_host(SEED, system(0), 0),
            ClassDraw::for_host(SEED, system(1), 0)
        );
    }

    /// The pick uses the weights' own total as its bound: every mark picks a class, and the
    /// allocation-free pick agrees with the thresholds.
    #[test]
    fn every_mark_picks_a_class_and_both_picks_agree() {
        let mut marks: Vec<Mark> = [0, 1 << 11, u64::MAX - (1 << 11), u64::MAX]
            .into_iter()
            .map(Mark::from_word)
            .collect();
        let stream = Stream::open(SEED, tags::SELFTEST_STREAM, ObjectKey::galaxy());
        marks.extend((0..2_000).map(|n| Mark::from_word(stream.word_at(n))));
        let constrained = weights(1.0, 0.0).constrained(&ClassConstraints::new(
            &Disc::None,
            ZoneLimit::Unbounded,
            HostMultiplicity::SingleOrWide,
        ));
        for w in [
            weights(1.0, 0.0),
            weights(0.3, 0.4),
            weights(0.05, -1.0),
            weights(20.0, 0.0),
            constrained,
        ] {
            let thresholds = Thresholds::from_weights(w.as_array(), w.total());
            assert_eq!(
                thresholds.as_slice().last(),
                Some(&crate::rng::Threshold::ALWAYS)
            );
            for &mark in &marks {
                let class = ClassDraw::from_mark(mark).class(&w);
                assert_eq!(mark.pick(&thresholds), Some(class.index()));
                assert!(w.get(class) > 0.0, "a class of no weight is never drawn");
            }
        }
    }

    /// P14.T4.c: 10⁵ class draws against the probabilities, for a Sun-like star and for an M
    /// dwarf in a close binary whose disc cannot form giants.
    #[test]
    fn class_draws_follow_the_probabilities() {
        let sun = disc_of(1.0, 0.0, Truncation::NONE);
        let cases = [
            (weights(1.0, 0.0), ClassConstraints::NONE),
            (
                weights(0.3, 0.2),
                ClassConstraints::new(
                    &sun,
                    ZoneLimit::Outer(au(2.0)),
                    HostMultiplicity::CloseBinary,
                ),
            ),
        ];
        for (index, (w, constraints)) in cases.iter().enumerate() {
            let constrained = w.constrained(constraints);
            let p = constrained.probabilities();
            let n = 100_000_u32;
            let mut counts = [0_u64; CLASS_COUNT];
            for i in 0..n {
                let class = ClassDraw::for_host(SEED, system(i / 4), u8::try_from(i % 4).unwrap())
                    .class(&constrained);
                counts[class.index()] += 1;
            }
            let expected = p.as_array().map(|q| q * f64::from(n));
            let fit = chi_square_gof(&counts, &expected);
            assert_p_value(&format!("class draws, case {index}"), fit.p_value, ALPHA);
        }
    }
}
