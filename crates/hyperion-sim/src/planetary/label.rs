//! Bodies' labels for people: `A b`, `AB c`, `A d II`, `BELT 1` (plan 14, design note 22,
//! P14.T30.c).
//!
//! Plan 01's designation of a body, its system's designation followed by ` /<body index>`, stays
//! the designation of record, because it parses back to the ID. A label is what the consoles show
//! beside it, in the exoplanet catalogues' manner:
//!
//! - **The host** is named by the letters of the components its bodies orbit, each component its
//!   body index's letter from `A` (the primary) to `P`: `A` for a planet of the primary, `B` for
//!   one of its first companion, `AB` for a planet about both (Kepler-16 (AB) b's convention, the
//!   parentheses dropped), `ABC` for one about a whole triple.
//! - **Planets** are lettered from `b` by their primordial semi-major axis about their host, the
//!   innermost first, ties broken by body index: `A b`, `A c`, … After `z` the letters run on as
//!   `aa`, `ab`, … (bijective base 26), which no catalogue has needed and a host of this generator
//!   version does not reach.
//! - **Moons** follow their planet's label with a Roman numeral from `I`: `A d II` (phase D).
//! - **Belts** are numbered from 1 through the system: `BELT 1` (phase D).
//!
//! Labels are derived, never stored ([`labels`]): each is a function of the generated system's
//! primordial state alone, so a label never changes with time, is the same at every detail level
//! that shows one (`MassAndOrbit` and above; a contact withholds it, P14.T34), and a planet that is
//! destroyed or not yet formed keeps its letter. Proper names are an overlay, as the brainstorm
//! has them.

use std::fmt;
use std::num::NonZeroU8;

use crate::planetary::index::{BodyIndex, STELLAR_SUB_END};
use crate::planetary::system::PlanetarySystem;

/// A body's label for people: host letters, planets lettered from `b` by semi-major axis, moons in
/// Roman numerals, belts numbered (design note 22), such as `A b` or `A d II`.
///
/// Labels are derived for a whole system ([`labels`]); the designation of record stays plan 01's,
/// which parses back to the ID.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BodyLabel(String);

impl BodyLabel {
    /// The label `text`, or `None` if it is empty.
    #[must_use]
    pub fn new(text: String) -> Option<Self> {
        (!text.is_empty()).then_some(Self(text))
    }

    /// The label of the planet at `ordinal` (0 the innermost) about the host named `host`:
    /// `A b` for the innermost planet of the primary.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::planetary::label::BodyLabel;
    ///
    /// assert_eq!(BodyLabel::planet("A", 0).as_str(), "A b");
    /// assert_eq!(BodyLabel::planet("AB", 7).as_str(), "AB i");
    /// ```
    #[must_use]
    pub fn planet(host: &str, ordinal: usize) -> Self {
        Self(format!("{host} {}", planet_letters(ordinal)))
    }

    /// The label of moon `number` (1 the first) of the planet labelled `planet`: `A d II` for the
    /// second moon of `A d`.
    ///
    /// # Examples
    ///
    /// ```
    /// use hyperion_sim::planetary::label::BodyLabel;
    ///
    /// use std::num::NonZeroU8;
    ///
    /// let planet = BodyLabel::planet("A", 2);
    /// let second = NonZeroU8::new(2).ok_or("two is not zero")?;
    /// assert_eq!(BodyLabel::moon(&planet, second).as_str(), "A d II");
    /// # Ok::<(), &'static str>(())
    /// ```
    #[must_use]
    pub fn moon(planet: &Self, number: NonZeroU8) -> Self {
        Self(format!("{} {}", planet.0, roman_numeral(number)))
    }

    /// The label of belt `number` (1 the first) of a system: `BELT 1`.
    #[must_use]
    pub fn belt(number: NonZeroU8) -> Self {
        Self(format!("BELT {number}"))
    }

    /// The label's text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BodyLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The letters of a host that is the components `members` (body indices, 0–15, in ascending
/// order): `A` for the primary, `B` for its first companion, `AB` for both.
///
/// # Panics
///
/// In debug builds, if a member is not a component's index (16 or more).
///
/// # Examples
///
/// ```
/// use hyperion_sim::planetary::label::host_letters;
///
/// assert_eq!(host_letters([0]), "A");
/// assert_eq!(host_letters([1, 2]), "BC");
/// ```
#[must_use]
pub fn host_letters(members: impl IntoIterator<Item = u8>) -> String {
    members
        .into_iter()
        .map(|member| {
            debug_assert!(
                member < STELLAR_SUB_END,
                "component {member} is not in slot 0"
            );
            char::from(b'A' + member.min(STELLAR_SUB_END - 1))
        })
        .collect()
}

/// The letters of the planet at `ordinal` (0 the innermost): `b` to `z`, then `aa`, `ab`, … as a
/// bijective base-26 numeral of `ordinal` + 2.
#[must_use]
fn planet_letters(ordinal: usize) -> String {
    let mut n = ordinal + 2;
    let mut letters = Vec::new();
    while n > 0 {
        let digit = (n - 1) % 26;
        letters.push(b'a' + u8::try_from(digit).expect("a digit below 26 fits a byte"));
        n = (n - 1) / 26;
    }
    letters.iter().rev().map(|&b| char::from(b)).collect()
}

/// `number` (1–255) in Roman numerals: `I`, `II`, … `CCLV`.
#[must_use]
fn roman_numeral(number: NonZeroU8) -> String {
    const NUMERALS: [(u8, &str); 9] = [
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut left = number.get();
    let mut text = String::new();
    for (value, numeral) in NUMERALS {
        while left >= value {
            text.push_str(numeral);
            left -= value;
        }
    }
    text
}

/// Every body of `system` with its label, in index order.
///
/// Each planet is lettered among its host's planets by primordial semi-major axis
/// ([`Body::orbit`](crate::planetary::system::Body::orbit)), ties broken by body index, after its
/// host's letters ([`host_letters`] of its zone's members). Labels are unique within a system:
/// two hosts' letters differ, since their members do, and a host's planets take distinct letters.
#[must_use]
pub fn labels(system: &PlanetarySystem) -> Vec<(BodyIndex, BodyLabel)> {
    let mut labelled: Vec<(BodyIndex, BodyLabel)> = Vec::with_capacity(system.bodies().len());
    let mut order: Vec<(f64, BodyIndex)> = Vec::new();
    for zone in system.zones() {
        let letters = host_letters(zone.members());
        order.clear();
        order.extend(
            system
                .bodies()
                .iter()
                .filter(|body| body.host() == zone.host())
                .map(|body| (body.orbit().semi_major_axis().value(), body.index())),
        );
        order.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        labelled.extend(
            order
                .iter()
                .enumerate()
                .map(|(ordinal, &(_, index))| (index, BodyLabel::planet(&letters, ordinal))),
        );
    }
    labelled.sort_by_key(|(index, _)| *index);
    labelled
}

/// The label of body `index` of `system`, or `None` if the system holds no such body: the one
/// [`labels`] gives it, found among its own host's planets alone.
#[must_use]
pub fn label(system: &PlanetarySystem, index: BodyIndex) -> Option<BodyLabel> {
    let body = system.body(index)?;
    let zone = system.zone(body.host())?;
    let key = |b: &crate::planetary::system::Body| (b.orbit().semi_major_axis(), b.index());
    let (a, _) = key(body);
    let ordinal = system
        .bodies()
        .iter()
        .filter(|other| other.host() == body.host())
        .filter(|other| {
            let (b, other_index) = key(other);
            b.value()
                .total_cmp(&a.value())
                .then(other_index.cmp(&index))
                .is_lt()
        })
        .count();
    Some(BodyLabel::planet(&host_letters(zone.members()), ordinal))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planetary::record::{DetailLevel, Section, SectionState};
    use crate::planetary::system::Body;
    use crate::planetary::system::generate_planets;
    use crate::planetary::system::tests::{SEED, generated, sun};
    use crate::time::UniverseTime;

    #[test]
    fn planets_are_lettered_from_b_and_run_on_past_z() {
        let letters: Vec<String> = (0..30).map(planet_letters).collect();
        assert_eq!(letters[0], "b");
        assert_eq!(letters[7], "i");
        assert_eq!(letters[24], "z");
        assert_eq!(letters[25], "aa");
        assert_eq!(letters[26], "ab");
        let mut unique = letters.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), letters.len());
        assert_eq!(planet_letters(24 + 26 * 26), "zz");
        assert_eq!(planet_letters(25 + 26 * 26), "aaa");
    }

    #[test]
    fn moons_take_roman_numerals_and_belts_numbers() {
        let d = BodyLabel::planet("A", 2);
        assert_eq!(d.as_str(), "A d");
        let moons: Vec<String> = [1, 2, 4, 9, 14, 40, 49, 90, 127, 255]
            .map(|n| BodyLabel::moon(&d, NonZeroU8::new(n).unwrap()).to_string())
            .to_vec();
        assert_eq!(
            moons,
            [
                "A d I",
                "A d II",
                "A d IV",
                "A d IX",
                "A d XIV",
                "A d XL",
                "A d XLIX",
                "A d XC",
                "A d CXXVII",
                "A d CCLV"
            ]
        );
        assert_eq!(BodyLabel::belt(NonZeroU8::MIN).as_str(), "BELT 1");
    }

    #[test]
    fn hosts_are_named_by_their_components_letters() {
        assert_eq!(host_letters([0]), "A");
        assert_eq!(host_letters([1]), "B");
        assert_eq!(host_letters([0, 1]), "AB");
        assert_eq!(host_letters([0, 1, 2]), "ABC");
        assert_eq!(host_letters([15]), "P");
    }

    #[test]
    fn a_label_is_never_empty() {
        assert!(BodyLabel::new(String::new()).is_none());
        let label = BodyLabel::new("A b".to_owned()).unwrap();
        assert_eq!(label.as_str(), "A b");
        assert_eq!(label.to_string(), "A b");
    }

    #[test]
    fn labels_are_unique_within_a_system_and_lettered_by_semi_major_axis() {
        for (_, system) in generated() {
            let labels = labels(system);
            assert_eq!(labels.len(), system.bodies().len());
            for (index, text) in &labels {
                assert_eq!(label(system, *index).as_ref(), Some(text));
            }
            let mut texts: Vec<&str> = labels.iter().map(|(_, l)| l.as_str()).collect();
            texts.sort_unstable();
            let before = texts.len();
            texts.dedup();
            assert_eq!(texts.len(), before, "{:?}", system.system());
            for zone in system.zones() {
                let letters = host_letters(zone.members());
                let mut hosted: Vec<(&Body, &BodyLabel)> = system
                    .bodies()
                    .iter()
                    .zip(labels.iter().map(|(_, l)| l))
                    .filter(|(body, _)| body.host() == zone.host())
                    .collect();
                hosted.sort_by(|a, b| {
                    a.0.orbit()
                        .semi_major_axis()
                        .total_cmp(&b.0.orbit().semi_major_axis())
                });
                for (ordinal, (_, label)) in hosted.iter().enumerate() {
                    assert_eq!(*label, &BodyLabel::planet(&letters, ordinal));
                }
            }
        }
    }

    /// A host of eight planets or more is lettered `A b` to `A i` from the inside out, as the
    /// Solar-like golden system will be.
    #[test]
    fn a_host_of_eight_planets_is_lettered_a_b_to_a_i() {
        let (ctx, system) = (0..2_000)
            .map(|i| {
                let ctx = sun(500 + i);
                let system = generate_planets(SEED, &ctx);
                (ctx, system)
            })
            .find(|(_, s)| s.bodies().len() >= 8)
            .expect("a Sun with eight planets among 2,000");
        let mut bodies: Vec<&Body> = system.bodies().iter().collect();
        bodies.sort_by(|a, b| {
            a.orbit()
                .semi_major_axis()
                .total_cmp(&b.orbit().semi_major_axis())
        });
        let expected = ["A b", "A c", "A d", "A e", "A f", "A g", "A h", "A i"];
        for (body, text) in bodies.iter().zip(expected) {
            let record = system
                .body_at(&ctx, body.index(), UniverseTime::EPOCH)
                .unwrap();
            assert_eq!(
                record.identity().label().ok().map(BodyLabel::as_str),
                Some(text)
            );
        }
    }

    #[test]
    fn a_circumbinary_planet_is_lettered_after_both_its_stars() {
        let mut found = 0;
        for (_, system) in generated() {
            for (index, label) in labels(system) {
                let host = system.body(index).unwrap().host();
                let letters = host_letters(system.zone(host).unwrap().members());
                assert!(label.as_str().starts_with(&format!("{letters} ")));
                if letters.len() > 1 {
                    found += 1;
                }
            }
        }
        assert!(found > 0, "no circumbinary planet in the sample");
    }

    #[test]
    fn a_label_is_stable_under_degrade_down_to_mass_and_orbit() {
        for (ctx, system) in generated().iter().take(60) {
            for record in system.snapshot_at(ctx, UniverseTime::EPOCH).bodies() {
                let label = record.identity().label().clone();
                assert_eq!(label.state(), SectionState::Ok);
                for level in [
                    DetailLevel::MassAndOrbit,
                    DetailLevel::Bulk,
                    DetailLevel::Surface,
                    DetailLevel::Full,
                ] {
                    assert_eq!(record.degrade(level).identity().label(), &label);
                }
                assert_eq!(
                    record.degrade(DetailLevel::Contact).identity().label(),
                    &Section::NotResolved
                );
            }
        }
    }
}
