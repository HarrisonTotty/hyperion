//! The class grid's tests (P09.T32), on toy classes that are not physics: one on a 512 ly class
//! and one on a 4,096 ly class, so that the alignment rule is exercised at both sizes, and in the
//! golden two more, a sparse one and an empty one.
//!
//! A toy expects `mean` candidates in every cell (its bound is `mean` ÷ the cell's volume, exact,
//! since the volume is a power of two). A candidate draws its position uniformly in the cell and is
//! accepted with probability its x fraction across the cell, so half are thinned; the toy's test
//! keeps a candidate that moves upwards (`v_z` > 0), half of the rest. So by the thinning theorem a
//! cell's entries are a Poisson number of mean `mean` ÷ 4, which can be checked by hand. Every
//! draw is on the foundation's `selftest.stream`, keyed by the candidate's ID; no production tag
//! is registered for them.

use std::sync::OnceLock;

use hyperion_testkit::float::bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;
use hyperion_testkit::lcg::Lcg;
use hyperion_testkit::order::assert_order_independent;
use hyperion_testkit::stats::{ALPHA, assert_poisson_count};

use super::*;
use crate::GENERATOR_VERSION;
use crate::coords::{GalacticPosition, GalacticVelocity, LyCell};
use crate::galaxy::catalogue_classes::{ALL_CLASSES, BuildCatalogueCellKeyError};
use crate::galaxy::placement;
use crate::id::{SystemId, SystemIdKind};
use crate::rng::Threshold;
use crate::time::UniverseTime;
use crate::units::LightYears;

/// The seed of the galaxy the toys are placed in.
const SEED: u64 = 0x0932_0000_6772_6964;

/// The galaxy most tests share; nothing in it changes once built.
fn galaxy() -> &'static Galaxy {
    static GALAXY: OnceLock<Galaxy> = OnceLock::new();
    GALAXY.get_or_init(|| Galaxy::new(Seed::new(SEED)))
}

/// A toy class process, not physics (module documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Toy {
    class: ClassId,
    /// Candidates expected per cell.
    mean: f64,
    /// Members of every entry.
    members: u8,
}

/// The toy on a 512 ly class, with a second member as P09.T34.a's survivor is.
const FINE: Toy = Toy {
    class: ClassId::CORE_COLLAPSE,
    mean: 12.0,
    members: 2,
};

/// The toy on a 4,096 ly class.
const COARSE: Toy = Toy {
    class: ClassId::STELLAR_MERGER,
    mean: 40.0,
    members: 1,
};

/// A sparse toy, as real classes are, under one candidate per 512 ly cell: the Poisson draw's
/// one-word path, which the golden pins on `class.cell` beside the two-word path of the others.
const SPARSE: Toy = Toy {
    class: ClassId::XRAY_BINARY,
    mean: 0.7,
    members: 1,
};

/// A toy whose bound is zero: no word is drawn, and the cell has no candidate.
const EMPTY: Toy = Toy {
    class: ClassId::ACCRETING_WHITE_DWARF,
    mean: 0.0,
    members: 1,
};

/// A toy candidate the thinning accepted.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ToyCandidate {
    position: GalacticPosition,
    velocity: GalacticVelocity,
}

/// A toy entry: its position and its velocity.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ToyEntry {
    position: GalacticPosition,
    velocity: GalacticVelocity,
}

/// `2⁻⁵²`.
const TWO_POW_MINUS_52: f64 = 1.0 / 4_503_599_627_370_496.0;

impl Toy {
    /// A candidate's one stream: words 0–2 its position, word 3 its acceptance, from word 4 its
    /// velocity.
    fn stream(galaxy: &Galaxy, id: CatalogueSystemId) -> Stream {
        Stream::open(
            galaxy.seed(),
            tags::SELFTEST_STREAM,
            ObjectKey::from(SystemId::from(id)),
        )
    }

    /// The one route to a candidate's velocity (the contract's rule 6): three normals of
    /// 100 km/s from word 4 of its stream, m/s.
    fn velocity(galaxy: &Galaxy, id: CatalogueSystemId) -> GalacticVelocity {
        let mut stream = Self::stream(galaxy, id);
        stream.seek(4);
        GalacticVelocity::new([(); 3].map(|()| stream.normal(0.0, 1e5)))
    }
}

impl ClassProcess for Toy {
    type Candidate = ToyCandidate;
    type Entry = ToyEntry;

    fn class(&self) -> ClassId {
        self.class
    }

    fn bound(&self, _: &Galaxy, cell: &CatalogueCellKey) -> PerCubicLightYear {
        PerCubicLightYear::new(self.mean / cell.volume_ly3())
    }

    fn candidate(
        &self,
        galaxy: &Galaxy,
        cell: &CatalogueCellKey,
        id: CatalogueSystemId,
        bound: PerCubicLightYear,
    ) -> Option<ToyCandidate> {
        let mut stream = Self::stream(galaxy, id);
        let log2 = cell.edge_ly().trailing_zeros();
        let origin = cell.origin_ly();
        let mut ly = [0_i32; 3];
        let mut fraction = [0.0; 3];
        for axis in 0..3 {
            // The top bits are the whole light-year in the cell, the next 52 its fraction.
            let word = stream.next_u64();
            ly[axis] = origin[axis] + i32::try_from(word >> (64 - log2)).unwrap();
            #[expect(
                clippy::cast_precision_loss,
                reason = "a 52-bit fraction is exact in f64"
            )]
            let f = ((word << log2) >> 12) as f64 * TWO_POW_MINUS_52;
            fraction[axis] = f;
        }
        let position =
            GalacticPosition::new(LyCell::new(ly), fraction.map(|f| f * METRES_PER_LIGHT_YEAR))
                .unwrap();
        let across = (f64::from(ly[0] - origin[0]) + fraction[0]) / f64::from(cell.edge_ly());
        let density = bound.value() * across;
        stream
            .decide(Threshold::from_ratio(density, bound.value()))
            .then(|| ToyCandidate {
                position,
                velocity: Self::velocity(galaxy, id),
            })
    }

    fn test(&self, _: &Galaxy, candidate: ToyCandidate) -> Option<ToyEntry> {
        let [_, _, vz] = candidate.velocity.metres_per_second();
        (vz > 0.0).then_some(ToyEntry {
            position: candidate.position,
            velocity: candidate.velocity,
        })
    }

    fn members(&self, _: &ToyEntry) -> u8 {
        self.members
    }
}

/// A test lookup over toy processes.
struct Toys(Vec<Toy>);

impl ClassLookup for Toys {
    fn visit_class<V: ClassVisitor>(
        &self,
        _: &Galaxy,
        class: ClassId,
        visitor: V,
    ) -> Option<V::Output> {
        self.0
            .iter()
            .find(|toy| toy.class == class)
            .map(|toy| visitor.visit(toy))
    }
}

/// The two toys' lookup.
fn toys() -> Toys {
    Toys(vec![FINE, COARSE])
}

/// What the tests keep of a member [`resolve`] found: its ID and its entry's member count.
struct Found;

impl MemberVisitor for Found {
    type Output = (CatalogueSystemId, u8);

    fn visit<P: ClassProcess>(
        self,
        _: &P,
        member: CatalogueMember<P::Entry>,
    ) -> (CatalogueSystemId, u8) {
        (member.id(), member.entry().members())
    }
}

/// An entry's bits: the position's light-year cell and offsets, and the velocity.
fn entry_bits(entry: &ToyEntry) -> ([i32; 3], [u64; 3], [u64; 3]) {
    (
        entry.position.cell().to_array(),
        entry.position.offset_metres().map(bits),
        entry.velocity.metres_per_second().map(bits),
    )
}

/// 48 cells of the fine toy about the Sun-like point, and 18 of the coarse one.
fn keys(toy: Toy) -> Vec<CatalogueCellKey> {
    let mut keys = Vec::new();
    if toy.class.cell_log2_ly() == Some(9) {
        for x in -2..2 {
            for y in 49..53 {
                for z in -2..1 {
                    keys.push(CatalogueCellKey::new(toy.class, [x, y, z]).unwrap());
                }
            }
        }
    } else {
        for x in -1..2 {
            for y in 5..8 {
                for z in -1..1 {
                    keys.push(CatalogueCellKey::new(toy.class, [x, y, z]).unwrap());
                }
            }
        }
    }
    keys
}

/// Every cell of both toys' key sets, generated.
fn cells(galaxy: &Galaxy, toy: Toy) -> Vec<CatalogueClassCell<ToyEntry>> {
    keys(toy)
        .into_iter()
        .map(|key| CatalogueClassCell::generate(galaxy, &toy, key))
        .collect()
}

// 1. Order independence.

#[test]
fn cells_do_not_depend_on_what_was_generated_or_resolved_before() {
    let galaxy = galaxy();
    for toy in [FINE, COARSE] {
        let set = keys(toy);
        let forwards: Vec<_> = set
            .iter()
            .map(|&key| CatalogueClassCell::generate(galaxy, &toy, key))
            .collect();
        // Backwards, each cell after resolving every entry of the one before and an ID of the
        // other toy's class.
        let other = if toy == FINE { COARSE } else { FINE };
        let stray = keys(other)[0].candidate_id(0, 0).unwrap();
        let mut backwards: Vec<CatalogueClassCell<ToyEntry>> = Vec::new();
        for &key in set.iter().rev() {
            for entry in backwards
                .last()
                .map_or(&[][..], CatalogueClassCell::entries)
            {
                resolve_entry(galaxy, &toy, entry.id()).unwrap();
            }
            // Whether the stray ID names a system is beside the point: only asking it could
            // disturb what follows.
            let _ = resolve(galaxy, &toys(), stray, Found);
            backwards.push(CatalogueClassCell::generate(galaxy, &toy, key));
        }
        backwards.reverse();
        assert_eq!(forwards, backwards, "{}", toy.class);
        for (a, b) in forwards.iter().zip(&backwards) {
            for (x, y) in a.entries().iter().zip(b.entries()) {
                assert_eq!(entry_bits(x.value()), entry_bits(y.value()));
            }
        }
    }
}

#[test]
fn cells_are_order_independent() {
    let galaxy = galaxy();
    for toy in [FINE, COARSE] {
        assert_order_independent(&keys(toy), |&key| {
            CatalogueClassCell::generate(galaxy, &toy, key)
        });
        // And a resolve of every candidate index, kept or not, whatever was resolved before.
        let ids: Vec<_> = keys(toy)
            .into_iter()
            .flat_map(|key| (0..16).map(move |index| key.candidate_id(index, 0).unwrap()))
            .collect();
        assert_order_independent(&ids, |&id| resolve_entry(galaxy, &toy, id));
    }
}

#[test]
fn the_same_seed_gives_the_same_bits_twice() {
    let fresh = Galaxy::new(Seed::new(SEED));
    for toy in [FINE, COARSE] {
        let (a, b) = (cells(galaxy(), toy), cells(&fresh, toy));
        assert_eq!(a, b);
        for (x, y) in a.iter().zip(&b) {
            assert_eq!(x.candidates(), y.candidates());
            for (p, q) in x.entries().iter().zip(y.entries()) {
                assert_eq!(p.id(), q.id());
                assert_eq!(entry_bits(p.value()), entry_bits(q.value()));
            }
        }
    }
    // Another seed draws other counts.
    let other = Galaxy::new(Seed::new(SEED + 1));
    let counts = |galaxy: &Galaxy| -> Vec<u32> {
        cells(galaxy, FINE)
            .iter()
            .map(CatalogueClassCell::candidates)
            .collect()
    };
    assert_ne!(counts(galaxy()), counts(&other));
}

/// The toys' counts and entries are the Poisson numbers they should be: candidates of mean
/// `mean` per cell, and entries of mean `mean` ÷ 4 (a half thinned, a half of the rest failing the
/// test).
#[test]
fn the_toys_counts_and_entries_are_poisson_by_hand() {
    let galaxy = galaxy();
    for toy in [FINE, COARSE] {
        let mut candidates = 0_u64;
        let mut entries = 0_u64;
        let mut n = 0_u32;
        for x in -6..6 {
            for y in -6..6 {
                for z in -2..2 {
                    let key = CatalogueCellKey::new(toy.class, [x, y, z]).unwrap();
                    let cell = CatalogueClassCell::generate(galaxy, &toy, key);
                    candidates += u64::from(cell.candidates());
                    entries += u64::try_from(cell.entries().len()).unwrap();
                    n += 1;
                }
            }
        }
        let cells = f64::from(n);
        assert_poisson_count("candidates", candidates, toy.mean * cells, ALPHA);
        assert_poisson_count("entries", entries, toy.mean * cells / 4.0, ALPHA);
    }
}

// 2. Resolution.

#[test]
fn every_entry_resolves_to_itself_bit_for_bit() {
    let galaxy = galaxy();
    let lookup = toys();
    let mut resolved = 0;
    for toy in [FINE, COARSE] {
        for cell in cells(galaxy, toy) {
            for entry in cell.entries() {
                for member in 0..entry.members() {
                    let id = entry.member_id(member).unwrap();
                    let found = resolve_entry(galaxy, &toy, id).unwrap();
                    assert_eq!(found.id(), id);
                    assert_eq!(found.member(), member);
                    assert_eq!(found.entry(), entry);
                    assert_eq!(entry_bits(found.entry().value()), entry_bits(entry.value()));
                    assert_eq!(resolve(galaxy, &lookup, id, Found), Ok((id, toy.members)));
                    resolved += 1;
                }
            }
        }
    }
    assert!(resolved > 100, "only {resolved} members resolved");
}

#[test]
fn a_free_class_value_names_no_system() {
    let galaxy = galaxy();
    for value in [9, 10, 40, 63] {
        let id = CatalogueSystemId::new(value, [0, 50, 0], 0, 0).unwrap();
        assert_eq!(
            resolve(galaxy, &toys(), id, Found),
            Err(ResolveSystemError::NoSuchSystem),
            "class value {value}"
        );
    }
}

#[test]
fn the_tidal_disruption_victims_name_no_system() {
    let galaxy = galaxy();
    let victims = Toy {
        class: ClassId::TIDAL_DISRUPTION_VICTIM,
        mean: 12.0,
        members: 1,
    };
    // A lookup that holds a process for the class anyway: it is refused, since the class has no
    // cell size and so no cell.
    let lookup = Toys(vec![FINE, COARSE, victims]);
    for index in 0..8 {
        let id = CatalogueSystemId::new(7, [0, 50, 0], index, 0).unwrap();
        assert_eq!(
            resolve(galaxy, &lookup, id, Found),
            Err(ResolveSystemError::NoSuchSystem)
        );
        assert_eq!(
            resolve_entry(galaxy, &victims, id),
            Err(ResolveSystemError::NoSuchSystem)
        );
    }
    assert_eq!(
        CatalogueCellKey::new(ClassId::TIDAL_DISRUPTION_VICTIM, [0, 0, 0]),
        Err(BuildCatalogueCellKeyError::NotPlaced(
            ClassId::TIDAL_DISRUPTION_VICTIM
        ))
    );
    assert_eq!(
        check_class_headroom(
            ClassId::TIDAL_DISRUPTION_VICTIM,
            PerCubicLightYear::new(1e-9)
        ),
        Err(CheckClassHeadroomError::NotPlaced(
            ClassId::TIDAL_DISRUPTION_VICTIM
        ))
    );
}

#[test]
fn a_class_with_no_process_names_no_system() {
    let galaxy = galaxy();
    // A toy on the Type Ia class finds entries; the toys' lookup has no Type Ia process, so the
    // same IDs name nothing there.
    let type_ia = Toy {
        class: ClassId::TYPE_IA,
        ..FINE
    };
    let with = Toys(vec![FINE, COARSE, type_ia]);
    let key = CatalogueCellKey::new(ClassId::TYPE_IA, [0, 50, 0]).unwrap();
    let cell = CatalogueClassCell::generate(galaxy, &type_ia, key);
    assert_ne!(cell.entries(), []);
    for entry in cell.entries() {
        assert_eq!(
            resolve(galaxy, &with, entry.id(), Found),
            Ok((entry.id(), 2))
        );
        assert_eq!(
            resolve(galaxy, &toys(), entry.id(), Found),
            Err(ResolveSystemError::NoSuchSystem)
        );
        // And a process of another class does not answer for it.
        assert_eq!(
            resolve_entry(galaxy, &FINE, entry.id()),
            Err(ResolveSystemError::NoSuchSystem)
        );
    }
}

#[test]
fn a_misaligned_cell_names_no_system() {
    let galaxy = galaxy();
    for cell in cells(galaxy, COARSE) {
        for entry in cell.entries() {
            let id = entry.id();
            let mut fine = id.cell();
            fine[1] += 4;
            let misaligned = CatalogueSystemId::new(id.class(), fine, id.index(), 0).unwrap();
            assert!(!misaligned.is_aligned_to(12));
            assert_eq!(
                resolve_entry(galaxy, &COARSE, misaligned),
                Err(ResolveSystemError::NoSuchSystem)
            );
            assert_eq!(
                resolve(galaxy, &toys(), misaligned, Found),
                Err(ResolveSystemError::NoSuchSystem)
            );
        }
    }
}

#[test]
fn an_index_at_the_count_names_no_system() {
    let galaxy = galaxy();
    for toy in [FINE, COARSE] {
        for cell in cells(galaxy, toy) {
            for index in [cell.candidates(), CatalogueSystemId::INDEX_LIMIT - 1] {
                let id = cell.key().candidate_id(index, 0).unwrap();
                assert_eq!(
                    resolve_entry(galaxy, &toy, id),
                    Err(ResolveSystemError::NoSuchSystem),
                    "{} {:?} at index {index}",
                    toy.class,
                    cell.key().cell()
                );
                assert_eq!(
                    resolve(galaxy, &toys(), id, Found),
                    Err(ResolveSystemError::NoSuchSystem)
                );
            }
        }
    }
}

/// The IDs of candidates below the count that the thinning rejects (`thinned`) or the test
/// rejects (`failed`), over the toy's key set.
fn rejected(galaxy: &Galaxy, toy: Toy) -> (Vec<CatalogueSystemId>, Vec<CatalogueSystemId>) {
    let (mut thinned, mut failed) = (Vec::new(), Vec::new());
    for cell in cells(galaxy, toy) {
        let key = cell.key();
        let bound = toy.bound(galaxy, &key);
        for index in 0..cell.candidates() {
            let id = key.candidate_id(index, 0).unwrap();
            match toy.candidate(galaxy, &key, id, bound) {
                None => thinned.push(id),
                Some(candidate) if toy.test(galaxy, candidate).is_none() => failed.push(id),
                Some(_) => {
                    assert!(cell.entries().iter().any(|entry| entry.id() == id));
                }
            }
        }
    }
    (thinned, failed)
}

#[test]
fn a_thinned_candidate_names_no_system() {
    let galaxy = galaxy();
    for toy in [FINE, COARSE] {
        let (thinned, _) = rejected(galaxy, toy);
        assert!(thinned.len() > 10, "{}: {}", toy.class, thinned.len());
        for id in thinned {
            assert_eq!(
                resolve_entry(galaxy, &toy, id),
                Err(ResolveSystemError::NoSuchSystem)
            );
        }
    }
}

#[test]
fn a_candidate_failing_the_test_names_no_system() {
    let galaxy = galaxy();
    for toy in [FINE, COARSE] {
        let (_, failed) = rejected(galaxy, toy);
        assert!(failed.len() > 10, "{}: {}", toy.class, failed.len());
        for id in failed {
            assert_eq!(
                resolve_entry(galaxy, &toy, id),
                Err(ResolveSystemError::NoSuchSystem)
            );
            assert_eq!(
                resolve(galaxy, &toys(), id, Found),
                Err(ResolveSystemError::NoSuchSystem)
            );
        }
    }
}

#[test]
fn a_missing_member_names_no_system() {
    let galaxy = galaxy();
    for toy in [FINE, COARSE] {
        for cell in cells(galaxy, toy) {
            for entry in cell.entries() {
                assert_eq!(entry.member_id(toy.members), None);
                let id = entry.id();
                for member in [toy.members, 15] {
                    let missing =
                        CatalogueSystemId::new(id.class(), id.cell(), id.index(), member).unwrap();
                    assert_eq!(
                        resolve_entry(galaxy, &toy, missing),
                        Err(ResolveSystemError::NoSuchSystem)
                    );
                    assert_eq!(
                        resolve(galaxy, &toys(), missing, Found),
                        Err(ResolveSystemError::NoSuchSystem)
                    );
                }
            }
        }
    }
}

/// The contract's rule 6 on the toy, which carries a velocity: the cell, the resolve and the
/// toy's one velocity function see the same bits, and its test read those.
#[test]
fn the_velocity_has_one_route() {
    let galaxy = galaxy();
    for toy in [FINE, COARSE] {
        for cell in cells(galaxy, toy) {
            for entry in cell.entries() {
                let drawn = Toy::velocity(galaxy, entry.id()).metres_per_second();
                let resolved = resolve_entry(galaxy, &toy, entry.id()).unwrap();
                for velocity in [entry.value().velocity, resolved.entry().value().velocity] {
                    assert_eq!(velocity.metres_per_second().map(bits), drawn.map(bits));
                }
                assert!(drawn[2] > 0.0, "the test kept an entry moving upwards");
            }
        }
    }
}

// 3. Canonical IDs.

#[test]
fn every_generated_id_is_canonical() {
    let galaxy = galaxy();
    for toy in [FINE, COARSE] {
        let log2 = toy.class.cell_log2_ly().unwrap();
        for cell in cells(galaxy, toy) {
            for entry in cell.entries() {
                let id = entry.id();
                assert!(id.is_aligned_to(log2));
                assert_eq!(id.member(), 0);
                assert_eq!(id.class(), toy.class.value());
                assert_eq!(CatalogueCellKey::of(id), Ok(cell.key()));
                let raw = SystemId::from(id).raw();
                let back = SystemId::from_raw(raw).unwrap();
                assert_eq!(back.kind(), SystemIdKind::Catalogue(id));
                assert_eq!(back.cell_word(), cell.key().word());
            }
            // Entries are in index order, every index below the count.
            let indices: Vec<u32> = cell.entries().iter().map(|e| e.id().index()).collect();
            assert!(indices.windows(2).all(|pair| pair[0] < pair[1]));
            assert!(indices.iter().all(|&i| i < cell.candidates()));
        }
    }
}

#[test]
fn a_coarse_id_with_a_low_cell_bit_never_resolves() {
    let galaxy = galaxy();
    let mut twins = 0;
    for cell in cells(galaxy, COARSE) {
        for entry in cell.entries() {
            let id = entry.id();
            assert!(resolve_entry(galaxy, &COARSE, id).is_ok());
            for axis in 0..3 {
                for bit in 0..3 {
                    let mut fine = id.cell();
                    fine[axis] |= 1 << bit;
                    let twin = CatalogueSystemId::new(id.class(), fine, id.index(), 0).unwrap();
                    assert_eq!(
                        resolve_entry(galaxy, &COARSE, twin),
                        Err(ResolveSystemError::NoSuchSystem)
                    );
                    twins += 1;
                }
            }
        }
    }
    assert!(twins > 50, "only {twins} twins");
}

// 4. No cell above 2²⁴ candidates.

#[test]
fn the_count_is_held_under_the_index() {
    let limit = CatalogueSystemId::INDEX_LIMIT;
    assert_eq!(clamp_to_index(0), 0);
    assert_eq!(clamp_to_index(7), 7);
    assert_eq!(clamp_to_index(u64::from(limit)), limit);
    assert_eq!(clamp_to_index(u64::from(limit) + 5), limit);
    assert_eq!(clamp_to_index(u64::MAX), limit);
    // Every candidate of a full cell has an ID: indices 0 to 2²⁴ − 1.
    let key = CatalogueCellKey::new(ClassId::CORE_COLLAPSE, [0, 0, 0]).unwrap();
    assert!(key.candidate_id(limit - 1, 0).is_ok());
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "past the 2^24 index")]
fn a_draw_past_the_index_is_a_debug_assertion() {
    // A mean 10⁵ above the index, 24 standard deviations: the draw is past it.
    let over = Toy {
        mean: f64::from(CatalogueSystemId::INDEX_LIMIT) + 1e5,
        ..FINE
    };
    let key = CatalogueCellKey::new(over.class, [0, 0, 0]).unwrap();
    // The count's debug assertion panics before any candidate is drawn.
    let _ = CatalogueClassCell::generate(galaxy(), &over, key);
}

#[test]
#[should_panic(expected = "a class cell is generated by its own class's process")]
fn a_cell_of_another_class_is_refused() {
    let key = CatalogueCellKey::new(COARSE.class, [0, 6, 0]).unwrap();
    // The assertion fires before any draw.
    let _ = CatalogueClassCell::generate(galaxy(), &FINE, key);
}

#[test]
fn the_headroom_check_passes_the_toys_and_refuses_a_bound_at_the_index() {
    let galaxy = galaxy();
    for toy in [FINE, COARSE] {
        // Every toy cell expects the same, so any cell is the fullest.
        let key = CatalogueCellKey::new(toy.class, [0, 0, 0]).unwrap();
        assert_eq!(
            check_class_headroom(toy.class, toy.bound(galaxy, &key)),
            Ok(())
        );
        let per_cell = |mean: f64| PerCubicLightYear::new(mean / key.volume_ly3());
        let limit = f64::from(CatalogueSystemId::INDEX_LIMIT);
        // A bound at the index's edge expects 2²⁴ candidates exactly.
        assert_eq!(
            check_class_headroom(toy.class, per_cell(limit)),
            Err(CheckClassHeadroomError::ClassTooDense {
                class: toy.class,
                largest_mean: limit,
                capacity: CatalogueSystemId::INDEX_LIMIT,
            })
        );
        // The largest mean with eight standard deviations of room: (√(16 + 2²⁴) − 4)².
        let root = (16.0 + limit).sqrt() - 4.0;
        let largest = root * root;
        assert!((16_744_479.0..16_744_481.0).contains(&largest), "{largest}");
        assert_eq!(
            check_class_headroom(toy.class, per_cell(largest - 1.0)),
            Ok(())
        );
        assert!(matches!(
            check_class_headroom(toy.class, per_cell(largest + 1.0)),
            Err(CheckClassHeadroomError::ClassTooDense { class, .. }) if class == toy.class
        ));
        for bad in [f64::NAN, f64::INFINITY, -1.0] {
            assert!(matches!(
                check_class_headroom(toy.class, per_cell(bad)),
                Err(CheckClassHeadroomError::ClassTooDense { .. })
            ));
        }
    }
}

// 5. The lookup.

/// The squared distance from `centre` to a cell's closed box `[low, low + edge]` on one axis, ly²,
/// from the centre's whole light-year and its fraction: a direct statement of the walk's test.
fn axis_gap_squared(centre_ly: i64, fraction: f64, low: i64, edge: i64) -> f64 {
    let high = low + edge;
    #[expect(
        clippy::cast_precision_loss,
        reason = "gaps inside a test's range are far under 2^53"
    )]
    let gap = if centre_ly >= high {
        (centre_ly - high) as f64 + fraction
    } else if low > centre_ly {
        (low - centre_ly) as f64 - fraction
    } else {
        0.0
    };
    gap * gap
}

/// Every cell of `class` within `radius` + 4,096 ly of `centre`, by testing each cell of a box
/// that holds them, in ascending order.
fn brute_force(class: ClassId, centre: &GalacticPosition, radius: f64) -> Vec<CatalogueCellKey> {
    let log2 = class.cell_log2_ly().unwrap();
    let edge = 1_i64 << log2;
    let half = i64::from(ROOT_HALF_WIDTH_LY) >> log2;
    let reach = radius + LIFETIME_REACH.value();
    let c = centre.cell().to_array().map(i64::from);
    let f = centre.offset_metres().map(|m| m / METRES_PER_LIGHT_YEAR);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a test's reach is a few thousand light-years"
    )]
    let span = (reach / 512.0).ceil() as i64 + 2;
    let range = |axis: usize| {
        let at = c[axis].div_euclid(edge);
        (at - span).max(-half)..=(at + span).min(half - 1)
    };
    let mut found = Vec::new();
    for x in range(0) {
        for y in range(1) {
            for z in range(2) {
                let d2 = axis_gap_squared(c[0], f[0], x * edge, edge)
                    + axis_gap_squared(c[1], f[1], y * edge, edge)
                    + axis_gap_squared(c[2], f[2], z * edge, edge);
                if d2 <= reach * reach {
                    let cell = [x, y, z].map(|v| i32::try_from(v).unwrap());
                    found.push(CatalogueCellKey::new(class, cell).unwrap());
                }
            }
        }
    }
    found
}

/// The walk's cells against a plain float distance from `centre` to each cell's box, checked
/// independently of the walk's whole-light-year test: every cell of the bounding box clearly
/// inside the padded radius is walked, and every walked cell is not clearly outside it. "Clearly"
/// is a millionth of a light-year, far above a float light-year's rounding inside the cube.
fn assert_plain_distances(
    walked: &[CatalogueCellKey],
    class: ClassId,
    centre: [f64; 3],
    radius: f64,
) {
    let log2 = class.cell_log2_ly().unwrap();
    let edge = f64::from(1_u32 << log2);
    let half = ROOT_HALF_WIDTH_LY >> log2;
    let reach = radius + LIFETIME_REACH.value();
    let distance = |cell: [i32; 3]| {
        let squares = [0, 1, 2].map(|axis| {
            let low = f64::from(cell[axis]) * edge;
            let gap = (low - centre[axis])
                .max(centre[axis] - (low + edge))
                .max(0.0);
            gap * gap
        });
        (squares[0] + squares[1] + squares[2]).sqrt()
    };
    for key in walked {
        assert!(distance(key.cell()) <= reach + 1e-6, "{key:?} is outside");
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "cell coordinates of the root cube, whole numbers well inside i32"
    )]
    let around = |axis: usize| {
        let at = (centre[axis] / edge).floor() as i32;
        let span = (reach / edge).ceil() as i32 + 1;
        (at - span).max(-half)..=(at + span).min(half - 1)
    };
    for x in around(0) {
        for y in around(1) {
            for z in around(2) {
                if distance([x, y, z]) < reach - 1e-6 {
                    let key = CatalogueCellKey::new(class, [x, y, z]).unwrap();
                    assert!(walked.binary_search(&key).is_ok(), "{key:?} is missing");
                }
            }
        }
    }
}

/// A sphere of `radius` ly about `centre`, at the epoch with no pad.
fn sphere(centre: [f64; 3], radius: f64) -> QuerySphere {
    QuerySphere::new(
        GalacticPosition::from_light_years(centre).unwrap(),
        LightYears::new(radius),
        UniverseTime::EPOCH,
        LightYears::ZERO,
    )
    .unwrap()
}

#[test]
fn the_lookup_equals_a_brute_force_scan() {
    let mut lcg = Lcg::new(0x0932_0005);
    let face = f64::from(ROOT_HALF_WIDTH_LY);
    let mut centres = vec![
        [0.0, 26_000.0, 20.0],
        [0.0, 0.0, 0.0],
        [-0.25, 511.75, -4_096.0],
        [face, 0.0, 0.0],
        [-face, -face, face - 1e-6],
        [face + 3_000.0, 100.0, -200.0],
        [65_000.0, -65_400.0, 0.5],
    ];
    for _ in 0..40 {
        centres.push([0, 1, 2].map(|_| (lcg.next_f64() * 2.0 - 1.0) * (face + 2_000.0)));
    }
    let mut compared = 0;
    for (i, &centre) in centres.iter().enumerate() {
        let radius = [0.001, 50.0, 300.5, 2_000.0][i % 4];
        let sphere = sphere(centre, radius);
        for class in [ClassId::CORE_COLLAPSE, ClassId::STELLAR_MERGER] {
            let walked: Vec<_> = cells_in_sphere(class, &sphere).collect();
            let expected = brute_force(class, sphere.centre(), radius);
            assert_eq!(
                walked, expected,
                "{class} about {centre:?}, radius {radius}"
            );
            assert_plain_distances(&walked, class, centre, radius);
            assert!(walked.windows(2).all(|pair| pair[0] < pair[1]));
            compared += walked.len();
        }
    }
    assert!(compared > 10_000, "only {compared} cells compared");
}

/// The lookup's figures (P09.T32, issue #127): a sphere of a thousandth of a light-year at a cell's
/// middle reaches eight rings of 512 ly cells about its own and one of 4,096 ly cells.
#[test]
fn the_lookup_reaches_eight_rings_at_512_ly_and_one_at_4096_ly() {
    for (class, middle, rings) in [
        (ClassId::CORE_COLLAPSE, 256.0, 8),
        (ClassId::STELLAR_MERGER, 2_048.0, 1),
    ] {
        let sphere = sphere([middle, middle, middle], 0.001);
        let keys: Vec<_> = cells_in_sphere(class, &sphere).collect();
        let farthest = keys
            .iter()
            .map(|key| key.cell().map(i32::abs).into_iter().max().unwrap())
            .max()
            .unwrap();
        assert_eq!(farthest, rings, "{class}");
        assert!(keys.contains(&CatalogueCellKey::new(class, [rings, 0, 0]).unwrap()));
        assert!(!keys.contains(&CatalogueCellKey::new(class, [rings + 1, 0, 0]).unwrap()));
    }
    assert_eq!(LIFETIME_REACH.value() / 512.0, 8.0);
    assert_eq!(LIFETIME_REACH.value() / 4_096.0, 1.0);
}

/// The lookup reads the sphere's radius, not its padded radius: the grid layers' drift pad is
/// not the class's.
#[test]
fn the_lookup_pads_by_the_reach_not_the_sphere_s_pad() {
    let centre = GalacticPosition::from_light_years([100.0, 26_000.0, 20.0]).unwrap();
    let plain = QuerySphere::new(
        centre,
        LightYears::new(50.0),
        UniverseTime::EPOCH,
        LightYears::ZERO,
    )
    .unwrap();
    let padded = QuerySphere::new(
        centre,
        LightYears::new(50.0),
        UniverseTime::EPOCH,
        LightYears::new(3_000.0),
    )
    .unwrap();
    for class in [ClassId::CORE_COLLAPSE, ClassId::STELLAR_MERGER] {
        let a: Vec<_> = cells_in_sphere(class, &plain).collect();
        let b: Vec<_> = cells_in_sphere(class, &padded).collect();
        assert_eq!(a, b);
    }
}

/// The 50 ly query's cell counts, which P09.T37's 5 ms budget reads, pinned.
#[test]
fn a_50_ly_sphere_visits_these_cells() {
    let sun = sphere([0.0, 26_000.0, 20.0], 50.0);
    let off_plane = sphere([1_234.4, 26_000.6, 17.2], 50.0);
    let counts = |s: &QuerySphere| {
        [ClassId::CORE_COLLAPSE, ClassId::STELLAR_MERGER].map(|c| cells_in_sphere(c, s).count())
    };
    assert_eq!(counts(&sun), FIFTY_LY_COUNTS[0]);
    assert_eq!(counts(&off_plane), FIFTY_LY_COUNTS[1]);
}

/// [`a_50_ly_sphere_visits_these_cells`]'s counts: at (0, 26,000, 20) ly and at (1,234.4,
/// 26,000.6, 17.2) ly, of 512 ly and of 4,096 ly cells.
const FIFTY_LY_COUNTS: [[usize; 2]; 2] = [[2_902, 20], [2_890, 20]];

// 6. The production lookup.

#[test]
fn the_production_lookup_resolves_no_catalogue_id() {
    struct Visited;
    impl ClassVisitor for Visited {
        type Output = ();
        fn visit<P: ClassProcess>(self, _: &P) {}
    }
    let galaxy = galaxy();
    for class in ALL_CLASSES {
        assert_eq!(ProductionClasses.visit_class(galaxy, class, Visited), None);
    }
    let mut lcg = Lcg::new(0x0932_0006);
    for _ in 0..2_000 {
        let class = u8::try_from(lcg.next_below(64)).unwrap();
        // Aligned to the class's cell size where it has one, so that only the missing process
        // refuses most of them.
        let log2 = ClassId::from_value(class)
            .and_then(ClassId::cell_log2_ly)
            .unwrap_or(9);
        let cell = [(); 3].map(|()| {
            let c = i32::try_from(lcg.next_below(256)).unwrap() - 128;
            c & !((1 << (log2 - 9)) - 1)
        });
        let index = u32::try_from(lcg.next_below(64)).unwrap();
        let member = u8::try_from(lcg.next_below(16)).unwrap();
        let id = CatalogueSystemId::new(class, cell, index, member).unwrap();
        assert_eq!(
            resolve(galaxy, &ProductionClasses, id, Found),
            Err(ResolveSystemError::NoSuchSystem),
            "{id:?}"
        );
        // Plan 03's resolve is unchanged: it does not generate catalogue systems yet (lean (a)).
        assert_eq!(
            placement::resolve(galaxy, SystemId::from(id)),
            Err(ResolveSystemError::KindNotGenerated)
        );
    }
}

// 7. The golden.

/// The grid pinned on the toys: each listed cell's key word, its candidate count from
/// `class.cell` and its entries' IDs; and one lookup's cell list, in the walk's order. The toys'
/// own draws are test code; the count stream, the key words, the ID layout and the walk's order
/// are what the classes of P09.T33 on ride on.
#[test]
fn grid_golden() {
    let galaxy = galaxy();
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    let listed = [
        (
            FINE,
            vec![
                [0, 50, 0],
                [0, 50, -1],
                [-1, 50, 0],
                [3, -60, 1],
                [-128, -128, -128],
                [127, 127, 127],
            ],
        ),
        (
            COARSE,
            vec![[0, 6, 0], [-1, 6, -1], [-16, -16, -16], [15, 15, 15]],
        ),
        (
            SPARSE,
            vec![
                [0, 50, 0],
                [1, 50, 0],
                [2, 50, 0],
                [3, 50, 0],
                [0, 51, 0],
                [-5, 20, 3],
                [10, -10, -1],
                [-128, 127, 0],
            ],
        ),
        (EMPTY, vec![[0, 50, 0], [-128, -128, -128]]),
    ];
    for (toy, cells) in listed {
        w.line(&format!(
            "toy {}: {} ly cells, {} candidates expected per cell, entries of {} member(s)",
            toy.class,
            1_u32 << toy.class.cell_log2_ly().unwrap(),
            toy.mean,
            toy.members
        ));
        for cell in cells {
            let key = CatalogueCellKey::new(toy.class, cell).unwrap();
            let generated = CatalogueClassCell::generate(galaxy, &toy, key);
            let [x, y, z] = cell;
            w.u64_hex(&format!("cell ({x}, {y}, {z}) word"), key.word());
            w.line(&format!(
                "cell ({x}, {y}, {z}) candidates = {}",
                generated.candidates()
            ));
            for entry in generated.entries() {
                w.u64_hex(
                    &format!("  entry {}", entry.id().index()),
                    SystemId::from(entry.id()).raw(),
                );
            }
        }
    }
    let sun = sphere([0.0, 26_000.0, 20.0], 50.0);
    let coarse: Vec<_> = cells_in_sphere(ClassId::STELLAR_MERGER, &sun).collect();
    w.line(&format!(
        "lookup STELLAR_MERGER, 50 ly about (0, 26000, 20) ly: {} cells",
        coarse.len()
    ));
    for key in coarse {
        let [x, y, z] = key.cell();
        w.line(&format!("  ({x}, {y}, {z})"));
    }
    let fine: Vec<_> = cells_in_sphere(ClassId::CORE_COLLAPSE, &sun).collect();
    w.line(&format!(
        "lookup CORE_COLLAPSE, 50 ly about (0, 26000, 20) ly: {} cells",
        fine.len()
    ));
    for key in fine.iter().take(3).chain(fine.iter().rev().take(3).rev()) {
        let [x, y, z] = key.cell();
        w.line(&format!("  ({x}, {y}, {z})"));
    }
    golden!("catalogue_classes/grid", w.as_str());
}
