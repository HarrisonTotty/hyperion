//! The hold on a pinned primary that its binary strips to a white dwarf's core before its pin
//! (plan 11, P11.T4's fix, at [`PINNED_WHITE_DWARFS_VERSION`]).
//!
//! Design note 16 makes plan 06 own a massive primary's death: it collapses at plan 06's death age
//! with plan 06's remnant and kick, and if the binary would end it sooner it is held at its last
//! living state until then, or, if the pair has merged, the product collapses. As built the hold
//! covers a bare core that collapses at once (`Remains::Ended`), but not one below the
//! Chandrasekhar mass, which the engine makes a white dwarf (`Remains::WhiteDwarf`). Primaries of
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
//! From 27 ([`PinnedWhiteDwarfRules::Held`]) no strip leaves a pinned primary whose pin is pending
//! a white dwarf. A strip that would (`Engine::stripped_member_at`, through which `Engine::hold`
//! strips a bare core too) holds the primary as a bare core that collapses is held: frozen at the
//! core's last living state, the track's structure there at the core's mass, with that structure's
//! core radius (`Remains::WhiteDwarf`'s `core` and `core_radius`). A bare core of the thermally
//! pulsing AGB or of the post-AGB crossing is held at its white dwarf's luminosity and radius at
//! formation (HPT section 6.3's limit, `Track::bare_agb_core`), since there the AGB's own closed
//! forms keep a giant's radius to the loss of its envelope.
//!
//! The pin then collapses the held star from its mass with plan 06's remnant and kick
//! (`Engine::pinned_collapse`, `held_to`). A companion, a primary with no pin, and a primary whose
//! pin has passed take the held arm's white dwarf under either arm.

use crate::GeneratorVersion;

/// The generator version whose stellar rules hold a pinned primary that its binary would make a
/// white dwarf before its pin until the pin (P11.T4's fix): `Engine::stripped_member_at` takes
/// [`PinnedWhiteDwarfRules::Held`].
///
/// Below it every pair takes the held arm, [`PinnedWhiteDwarfRules::Born`], the code of version 24
/// verbatim, so every generated value keeps its bits; the tests reach the new arm through
/// `StellarRules::every_27_hold()`. The bump to 27 deletes this hold: this module with
/// [`PinnedWhiteDwarfRules`], the stellar rules' `pinned_white_dwarfs` field, its reader's call of
/// [`PinnedWhiteDwarfRules::holds`] (`stripped_member_at`'s white-dwarf arm), and the tripwire
/// (`the_pinned_white_dwarfs_are_held_only_until_the_27_bump`).
pub(crate) const PINNED_WHITE_DWARFS_VERSION: GeneratorVersion = GeneratorVersion::new(27);

/// What becomes of a pinned primary, its pin pending, that its binary would make a white dwarf
/// (the stellar rules' `pinned_white_dwarfs` field, the hold of [`PINNED_WHITE_DWARFS_VERSION`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum PinnedWhiteDwarfRules {
    /// Version 26's and before: it is born a white dwarf, and its pin finds nothing living.
    Born,
    /// Version 27's: it is held at its core's last living state until its pin collapses it.
    Held,
}

impl PinnedWhiteDwarfRules {
    /// Whether a pinned primary, its pin pending, that its binary would make a white dwarf is held
    /// until its pin under these rules: the hold's one `match`, which the strip
    /// (`Engine::stripped_member_at`) goes through.
    #[must_use]
    pub(crate) const fn holds(self) -> bool {
        match self {
            Self::Born => false,
            Self::Held => true,
        }
    }
}
