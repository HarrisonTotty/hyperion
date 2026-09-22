//! The registry of event tags.
//!
//! An event tag is a 16-bit number in an event word, and each is backed by a domain tag of scope
//! `Event` from the single domain-tag registry, [`crate::rng::tags`], under which the event key of
//! its subjects is derived. The rules:
//!
//! - Each entry reads `number => CONST_NAME = tags::DOMAIN_TAG;`. Numbers are explicit and never
//!   reused, and 0 is never valid.
//! - Every domain tag named has scope `Event` and backs exactly one event tag.
//! - A `const` assertion enforces both, so a broken entry fails compilation.
//!
//! Number blocks: `0x0001` is plan 01's self-test tag; the stellar, feature, binary and body
//! stages allocate the blocks their plans set aside:
//!
//! | Block             | Owner                                      |
//! | ----------------- | ------------------------------------------ |
//! | `0x0001`          | Plan 01's self-test tag                    |
//! | `0x0100`–`0x01FF` | Single stars (plan 06)                     |
//! | `0x0200`–`0x02FF` | Features and the galactic centre (plan 09) |
//! | `0x0300`–`0x03FF` | Binaries (plan 11)                         |
//! | `0x0400`–`0x04FF` | Bodies (plan 14)                           |
//!
//! `0x0002`–`0x00FF` and everything from `0x0500` stay unallocated. Within a block numbers are
//! explicit, ascending in order of registration and never reused. Each tag's events are built by
//! one of `events`' two constructions only, Poisson bins or a monotone phase, never both, because
//! the two give the event key's slots different meanings.

use super::event::EventTag;
use crate::rng::{DomainTag, TagScope, tags};

/// Panics unless every number is non-zero and unique and every domain tag has scope `Event` and
/// appears once.
const fn assert_registry(entries: &[(u16, DomainTag)]) {
    let mut i = 0;
    while i < entries.len() {
        let (number, tag) = entries[i];
        assert!(number != 0, "event tag 0 is never valid");
        assert!(
            matches!(tag.scope(), TagScope::Event),
            "an event tag's domain tag has scope Event"
        );
        let mut j = 0;
        while j < i {
            assert!(number != entries[j].0, "duplicate event tag number");
            assert!(
                tag.hash() != entries[j].1.hash(),
                "a domain tag backs more than one event tag"
            );
            j += 1;
        }
        i += 1;
    }
}

/// Declares the registry of event tags. Used once, below.
///
/// Emits one `pub const` [`EventTag`] per entry, the slice `ALL`, `EventTag::from_number`,
/// `EventTag::domain_tag`, `EventTag::name`, and the `const` assertion of [`assert_registry`].
macro_rules! event_tags {
    ($( $(#[$meta:meta])* $number:literal => $name:ident = $tag:path ; )*) => {
        $(
            $(#[$meta])*
            pub const $name: EventTag = EventTag::registered($number);
        )*

        /// Every registered event tag, in registry order.
        pub const ALL: &[EventTag] = &[$($name),*];

        const _: () = assert_registry(&[$(($number, $tag)),*]);

        impl EventTag {
            /// The registered tag with this number, or `None`.
            #[must_use]
            pub const fn from_number(number: u16) -> Option<Self> {
                match number {
                    $($number => Some($name),)*
                    _ => None,
                }
            }

            /// The domain tag of scope `Event` that keys this tag's events.
            ///
            /// # Panics
            ///
            /// Never: every `EventTag` is an entry of this registry.
            #[must_use]
            pub const fn domain_tag(self) -> DomainTag {
                match self.number() {
                    $($number => $tag,)*
                    _ => panic!("every EventTag is registered"),
                }
            }

            /// The domain tag's name.
            #[must_use]
            pub const fn name(self) -> &'static str {
                self.domain_tag().name()
            }
        }
    };
}

event_tags! {
    // Plan 01: the determinism foundation.

    /// A tag for tests and golden files, never emitted by a generator.
    0x0001 => SELF_TEST = tags::EVENT_SELFTEST;

    // Plan 06: single stars, block 0x0100–0x01FF (P06.T27.a). Re-exported as `events::tags`.

    /// Flares of stars with convective envelopes (Poisson bins).
    0x0100 => STAR_FLARE = tags::STAR_EV_FLARE;
    /// Glitches of Crab-like pulsars (Poisson bins).
    0x0101 => STAR_GLITCH = tags::STAR_EV_GLITCH;
    /// Glitches of Vela-like pulsars (monotone phase).
    0x0102 => STAR_GLITCH_CYCLE = tags::STAR_EV_GLITCH_CYCLE;
    /// A magnetar's active episodes (Poisson bins).
    0x0103 => STAR_MAGNETAR_EPISODE = tags::STAR_EV_MAGNETAR_EPISODE;
    /// A magnetar's short bursts within an episode (Poisson bins).
    0x0104 => STAR_MAGNETAR_BURST = tags::STAR_EV_MAGNETAR_BURST;
    /// A magnetar's giant flares (Poisson bins).
    0x0105 => STAR_MAGNETAR_GIANT = tags::STAR_EV_MAGNETAR_GIANT;
    /// FU Orionis outbursts of young stars (Poisson bins).
    0x0106 => STAR_FU_ORIONIS = tags::STAR_EV_FU_ORIONIS;
    /// Giant eruptions of luminous blue variables (Poisson bins).
    0x0107 => STAR_LBV_ERUPTION = tags::STAR_EV_LBV_ERUPTION;
    /// Thermal pulses on the asymptotic giant branch (monotone phase).
    0x0108 => STAR_THERMAL_PULSE = tags::STAR_EV_THERMAL_PULSE;
    /// The cycle-keyed irregularity of pulsating variables, S Doradus cycles included (monotone
    /// phase).
    0x0109 => STAR_VARIABILITY_CYCLE = tags::STAR_VAR_CYCLE;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_self_test_tag_is_registered() {
        assert_eq!(SELF_TEST.number(), 1);
        assert_eq!(SELF_TEST.name(), "event.selftest");
        assert_eq!(SELF_TEST.domain_tag(), tags::EVENT_SELFTEST);
        assert_eq!(EventTag::from_number(1), Some(SELF_TEST));
        assert_eq!(EventTag::from_number(0), None);
        assert_eq!(EventTag::from_number(2), None);
        assert_eq!(ALL.first(), Some(&SELF_TEST));
    }

    /// Plan 06's ten tags take `0x0100`–`0x0109` in order, each backed by its `star.` domain tag.
    #[test]
    fn plan_06_star_events_hold_0x0100_to_0x0109_in_order() {
        let plan_06 = [
            (STAR_FLARE, "star.ev.flare"),
            (STAR_GLITCH, "star.ev.glitch"),
            (STAR_GLITCH_CYCLE, "star.ev.glitch_cycle"),
            (STAR_MAGNETAR_EPISODE, "star.ev.magnetar_episode"),
            (STAR_MAGNETAR_BURST, "star.ev.magnetar_burst"),
            (STAR_MAGNETAR_GIANT, "star.ev.magnetar_giant"),
            (STAR_FU_ORIONIS, "star.ev.fu_orionis"),
            (STAR_LBV_ERUPTION, "star.ev.lbv_eruption"),
            (STAR_THERMAL_PULSE, "star.ev.thermal_pulse"),
            (STAR_VARIABILITY_CYCLE, "star.var.cycle"),
        ];
        for ((tag, name), number) in plan_06.into_iter().zip(0x0100_u16..) {
            assert_eq!(tag.number(), number, "{name}");
            assert_eq!(tag.name(), name);
            assert_eq!(EventTag::from_number(number), Some(tag));
        }
        assert_eq!(EventTag::from_number(0x010A), None);
        let in_block = ALL
            .iter()
            .filter(|t| (0x0100..=0x01FF).contains(&t.number()))
            .count();
        assert_eq!(in_block, plan_06.len());
    }

    #[test]
    fn every_entry_is_backed_by_a_distinct_event_scoped_domain_tag() {
        for (i, tag) in ALL.iter().enumerate() {
            assert_ne!(tag.number(), 0);
            assert_eq!(tag.domain_tag().scope(), TagScope::Event);
            assert!(tags::ALL.contains(&tag.domain_tag()));
            for other in &ALL[..i] {
                assert_ne!(tag.number(), other.number());
                assert_ne!(tag.domain_tag(), other.domain_tag());
            }
        }
    }

    #[test]
    #[should_panic(expected = "duplicate event tag number")]
    fn a_duplicate_number_panics() {
        assert_registry(&[(1, tags::EVENT_SELFTEST), (1, tags::EVENT_SELFTEST)]);
    }

    #[test]
    #[should_panic(expected = "scope Event")]
    fn a_domain_tag_of_another_scope_panics() {
        assert_registry(&[(1, tags::SELFTEST_STREAM)]);
    }

    #[test]
    #[should_panic(expected = "never valid")]
    fn number_zero_panics() {
        assert_registry(&[(0, tags::EVENT_SELFTEST)]);
    }

    #[test]
    #[should_panic(expected = "more than one event tag")]
    fn a_domain_tag_named_twice_panics() {
        assert_registry(&[(1, tags::EVENT_SELFTEST), (2, tags::EVENT_SELFTEST)]);
    }
}
