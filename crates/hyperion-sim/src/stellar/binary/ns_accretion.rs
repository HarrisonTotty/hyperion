//! What a neutron star keeps of a companion's stable Roche-lobe overflow (plan 11, P11.T4's fix).
//!
//! As first built, a neutron star fed by stable transfer kept all of it up to its Eddington rate
//! (BSE section 2.6.6, equations 67 and 68; ruling 132.1), and collapsed into a black hole once it
//! passed `MAX_NEUTRON_STAR_MASS` (`Engine::accretor_events`). A 1.35 M☉ neutron star beside a
//! 1.5 M☉ main-sequence donor took the whole 0.46 M☉ of the donor's main-sequence transfer, over
//! 3.3 Gyr at about 10⁻¹⁰ M☉ yr⁻¹, passed 2.0 M☉ while the donor crossed its Hertzsprung gap, and
//! ended a 2.6 M☉ black hole. In the slab's census (P11.T4's fix, its research of 2026-11-01) most
//! of the black holes in X-ray binaries were born neutron stars that
//! grew so, from donors of 1.2–3.8 M☉, which made black holes most of the low-mass hosts and put
//! nearly all of them under 5 M☉.
//!
//! Recycled pulsars show that neutron stars keep little of what their donors give them: a
//! millisecond pulsar needs only about 0.1 M☉ to be spun up, and the pulsars' masses are about the
//! slow pulsars' (Tauris, Langer and Kramer 2012, MNRAS 425, 1601; Antoniadis et al. 2016,
//! arXiv:1605.01665). So a neutron star keeps a share [`NS_ACCRETION_EFFICIENCY`] of the transfer,
//! never more than its Eddington rate (`Engine::accretion`), and the rest leaves the pair. Its
//! accretion is marked as lost from the accretor, as the super-Eddington excess is; the transfer's
//! step books it so only under the heavy-donor hold, at 27 ([`heavy_donor`](super::heavy_donor)),
//! and with the donor's specific angular momentum otherwise, as it books every loss.

/// The share of a stable transfer a neutron star keeps, below its Eddington rate: 0.1.
///
/// It is the share recycled pulsars imply (a lean of 2026-11-01). Spinning a 1.4 M☉ neutron star up
/// to 3 ms takes 0.057 M☉, `ΔM_eq` = 0.22 M☉ (M ÷ M☉)^⅓ ÷ (P ÷ ms)^(4/3) (Tauris, Langer and
/// Kramer 2012, equation 14; `recycling::SPIN_UP_MASS_AT_1_MS`), and the millisecond pulsars'
/// masses lie about 0.1–0.2 M☉ above the slow pulsars' (Antoniadis et al. 2016, arXiv:1605.01665;
/// recalled, not read at source), while a low-mass donor gives its neutron star some 0.7–1 M☉ on
/// its way to a helium white dwarf. Such efficiencies, well under one, are the usual inference
/// (Tauris, Langer and Kramer 2012; recalled).
pub(crate) const NS_ACCRETION_EFFICIENCY: f64 = 0.1;
