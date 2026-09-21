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
//! stages allocate the blocks their plans set aside.

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
        assert_eq!(ALL, &[SELF_TEST]);
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
