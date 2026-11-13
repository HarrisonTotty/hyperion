//! A pinned primary that its binary would make a white dwarf before its pin (plan 11, P11.T4's
//! fix).
//!
//! Design note 16 makes plan 06 own a massive primary's death: it collapses at plan 06's death age
//! with plan 06's remnant and kick, and if the binary would end it sooner it is held at its last
//! living state until then, or, if the pair has merged, the product collapses. As first built the
//! hold covered a bare core that collapses at once (`Remains::Ended`), but not one below the
//! Chandrasekhar mass, which the engine made a white dwarf (`Remains::WhiteDwarf`). Primaries of
//! 8.0–9.5 M☉ are stripped so before plan 06's death age (30–42 Myr): by a common envelope on the
//! early or thermally pulsing AGB, then case-BB stable transfer from the helium giant down to
//! 1.32–1.34 M☉, or by the common envelope alone; or a common envelope's coalescence leaves a
//! product that no star of its type can hold, the white dwarf of its core. At the pin
//! `Engine::pinned_collapse` then finds nothing living and records no collapse, while the system's
//! death time and natal kick report plan 06's neutron star. In layer E's sample of P11.T10's slow
//! tests under every 27 hold (research-leans-26; measured 2026-11-01) 63 of 5,421 pinned primaries
//! dead by the epoch (1.2%; 59 systems, the sample drawing some cells twice) missed their pin so:
//! 55 white dwarfs at it, and 8 whose neutron star came from a later merger or accretion-induced
//! collapse, at 45–128 Myr.
//!
//! So no exit leaves a pinned primary whose pin is pending a white dwarf:
//!
//! - a strip that would (`Engine::stripped_member_at`, through which `Engine::hold` strips a bare
//!   core too) holds the primary as a bare core that collapses is held: frozen at the core's last
//!   living state, the track's structure there at the core's mass, with that structure's core
//!   radius (`Remains::WhiteDwarf`'s `core` and `core_radius`). A bare core of the thermally
//!   pulsing AGB or of the post-AGB crossing is held at its white dwarf's luminosity and radius at
//!   formation (HPT section 6.3's limit, `Track::bare_agb_core`), since there the AGB's own closed
//!   forms keep a giant's radius to the loss of its envelope;
//! - a merger whose product in the primary's place no star of its kind can hold, which
//!   `Engine::new_star` makes the white dwarf of its core, holds the product the same way, frozen
//!   at that core's mass with its white dwarf's luminosity and radius at formation
//!   (`Engine::held_product`; a lean of 2026-11-01, recorded in plan 11).
//!
//! A held core at or above the Chandrasekhar mass, which has no white dwarf, shows the zero-age
//! helium star of its mass instead (`sse::bare_core_light`, a lean of 2026-11-01).
//!
//! The pin then collapses the held star from its mass with plan 06's remnant and kick
//! (`Engine::pinned_collapse`, `held_to`). A companion, a primary with no pin, and a primary whose
//! pin has passed take their white dwarf.
