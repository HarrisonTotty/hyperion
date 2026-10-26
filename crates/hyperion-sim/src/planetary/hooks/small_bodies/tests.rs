//! Tests of a small body's surface type (plan 14, P14.T51.e; decision-p14-t51e-contract §5.1
//! and decision-p14-t51e-qs §3.2): the closed set and its names, the constants, every lithology
//! placed, the planet, the Kuiper-belt colours, the veneer, what is not given, the anchor of the
//! wind's clock, the clock at the anchor, its edges, the inputs' ranges, determinism, and the
//! tripwire that keeps the module off the record until its wiring.
//!
//! The anchor and scaling tests here hold by construction, since their wind terms are stated by
//! hand: `w_ref` is the ruling's 0.15998, and a body's w₀ about the same host is scaled from it as
//! T51.d's wind mass flux, Ṁ ÷ (4π a² √(1 − e²)) ([`wind_term_at`]). They check the class's
//! arithmetic, not T51.d's rule. The tests by T51.d's own rule (decision-p14-t51e-qs §3.2: the
//! anchor about `ModifierHost::sun()`, w₀ to the bit, the moon inside its planet's magnetopause,
//! and the census) come with the wiring's tests, in `planetary/system/small_bodies`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use hyperion_testkit::float::assert_same_bits;

use super::*;
use crate::math;
use crate::units::Gigayears;

/// T51.d's wind term on a circular 2.5 au orbit about its reference Sun, `w_ref`: 0.15998
/// (decision-p14-t51e-qs §1.2 and its table, from its port of T51.d's rule), stated by hand.
const REFERENCE_WIND_TERM: f64 = 0.159_98;

/// T51.d's weathering rate k on that orbit: 0.11842 (decision-p14-t51e-qs's table), stated by
/// hand.
const ANCHOR_RATE: f64 = 0.118_42;

/// The surface crate's lunar maturation time `τ_L`, Gyr: 0.3277, stated by hand (the modifiers'
/// tripwire keeps its name out of this file).
const LUNAR_MATURATION_GYR: f64 = 0.3277;

/// The Solar System's age, Gyr: 4.567, the age E4 names.
const SOLAR_SYSTEM_GYR: f64 = 4.567;

/// The ordinary chondrites.
const ORDINARY: [CrustLithology; 3] = [
    CrustLithology::ChondriteH,
    CrustLithology::ChondriteL,
    CrustLithology::ChondriteLl,
];

/// Every kind.
const KINDS: [TaxonomyBodyKind; 4] = [
    TaxonomyBodyKind::Planet,
    TaxonomyBodyKind::AsteroidBeltMember,
    TaxonomyBodyKind::KuiperBody,
    TaxonomyBodyKind::OtherMoon,
];

/// The two kinds classed by their veneer, then their lid.
const BY_LID: [TaxonomyBodyKind; 2] = [
    TaxonomyBodyKind::AsteroidBeltMember,
    TaxonomyBodyKind::OtherMoon,
];

/// The wind term of a body at `a_au` au and eccentricity `e` about the reference Sun: `w_ref` ×
/// (2.5 ÷ a)² ÷ √(1 − e²), T51.d's flux Ṁ ÷ (4π a² √(1 − e²)) relative to the anchor's.
fn wind_term_at(a_au: f64, e: f64) -> f64 {
    let ratio = Q_TYPE_ANCHOR_DISTANCE.value() / a_au;
    REFERENCE_WIND_TERM * ratio * ratio / (1.0 - e * e).sqrt()
}

/// T51.d's maturity m = 1 − exp(−k t ÷ `τ_L`) at the anchor's k after `age`: about 0.30 at 1 Gyr
/// and 0.81 at 4.567 Gyr (decision-p14-t51e-qs §1.3).
fn anchor_maturity(age: Years) -> f64 {
    -math::exp_m1(-ANCHOR_RATE * Gigayears::from(age).value() / LUNAR_MATURATION_GYR)
}

/// The wind terms of a body of wind term `w0`, against the anchor's `w_ref`.
fn winds(w0: f64) -> WindTerms {
    WindTerms::new(w0, REFERENCE_WIND_TERM).unwrap()
}

/// Open clocks of exposure age `age`, wind term `wind`, the anchor's `w_ref` and maturity `m`.
fn open(age: Years, wind: f64, m: f64) -> WeatheringClocks {
    WeatheringClocks::new(age, winds(wind), m, AirShield::Open).unwrap()
}

/// A fresh, windless surface under air above the ion floor.
fn shielded_fresh() -> WeatheringClocks {
    WeatheringClocks::new(Years::ZERO, winds(0.0), 0.0, AirShield::AboveIonFloor).unwrap()
}

/// Compiles only for an error type the rules accept from a library crate.
fn sendable<E: Error + Send + Sync + 'static>(_: &E) {}

/// A veneer of tholin share `s_t` and uncovered ice share `s_i`.
fn veneer(s_t: f64, s_i: f64) -> Veneer {
    Veneer::new(s_t, s_i).unwrap()
}

/// A surface with no veneer.
fn bare() -> Veneer {
    veneer(0.0, 0.0)
}

/// The class of a body of kind `kind`, lid `lid`, veneer `veneer` and clocks `clocks`.
fn class_of(
    kind: TaxonomyBodyKind,
    lid: Option<CrustLithology>,
    veneer: Option<Veneer>,
    clocks: Option<WeatheringClocks>,
) -> Result<Option<TaxonomicClass>, ClassifyTaxonomyError> {
    TaxonomicClass::of(&TaxonomyInputs::new(kind, lid, veneer, clocks))
}

/// An ordinary chondrite belt member's class, bare, under `clocks`.
fn chondrite(lid: CrustLithology, clocks: WeatheringClocks) -> Option<TaxonomicClass> {
    class_of(
        TaxonomyBodyKind::AsteroidBeltMember,
        Some(lid),
        Some(bare()),
        Some(clocks),
    )
    .unwrap()
}

/// A surface at the anchor of the Solar System's age, weathered by both clocks.
fn old_at_anchor() -> WeatheringClocks {
    let age = Years::from(Gigayears::new(SOLAR_SYSTEM_GYR));
    open(age, REFERENCE_WIND_TERM, anchor_maturity(age))
}

/// A fresh surface under no wind: neither clock has run.
fn fresh_windless() -> WeatheringClocks {
    open(Years::ZERO, 0.0, 0.0)
}

/// Every input the rules can tell apart: each kind, each lid and none, a bare, a tholin-covered and
/// an icy veneer and none, and fresh, old, shielded and no clocks.
fn grid() -> Vec<TaxonomyInputs> {
    let lids = CrustLithology::ALL.map(Some).into_iter().chain([None]);
    let veneers = [
        None,
        Some(bare()),
        Some(veneer(0.9, 0.1)),
        Some(veneer(0.0, 0.9)),
    ];
    let clocks = [
        None,
        Some(fresh_windless()),
        Some(old_at_anchor()),
        Some(shielded_fresh()),
    ];
    let mut all = Vec::new();
    for kind in KINDS {
        for lid in lids.clone() {
            for veneer in veneers {
                for clocks in clocks {
                    all.push(TaxonomyInputs::new(kind, lid, veneer, clocks));
                }
            }
        }
    }
    all
}

/// The closed set: `DeMeo` et al. 2009's 24 classes, Tholen's P and the two colours, each with its
/// name; and over every input the rules tell apart they produce S, Q, C, Ch, K, Xc, V, A, D, P, red
/// and neutral, and no other, as each variant's doc says, each refusal naming an input the body
/// was not given.
#[test]
fn the_classes_are_closed_with_their_names_and_twelve_are_produced() {
    use TaxonomicClass as T;
    let names: Vec<&str> = TaxonomicClass::ALL.iter().map(|c| c.name()).collect();
    assert_eq!(
        names,
        [
            "A", "B", "C", "Cb", "Cg", "Cgh", "Ch", "D", "K", "L", "O", "Q", "R", "S", "Sa", "Sq",
            "Sr", "Sv", "T", "V", "X", "Xc", "Xe", "Xk", "P", "red", "neutral",
        ]
    );
    assert!(
        TaxonomicClass::ALL.windows(2).all(|pair| pair[0] < pair[1]),
        "ALL is in declaration order, distinct"
    );
    let mut produced = BTreeSet::new();
    let mut refused = 0_usize;
    for inputs in grid() {
        // The match is exhaustive, so an error other than not given fails to compile here.
        match TaxonomicClass::of(&inputs) {
            Ok(Some(class)) => {
                produced.insert(class);
            }
            Ok(None) => {}
            Err(ClassifyTaxonomyError::NotGiven(missing)) => {
                let absent = match missing {
                    MissingTaxonomyInput::Lid => inputs.lid().is_none(),
                    MissingTaxonomyInput::Veneer => inputs.veneer().is_none(),
                    MissingTaxonomyInput::Clocks => inputs.clocks().is_none(),
                };
                assert!(
                    absent,
                    "{inputs:?} refused for its {missing:?}, which it was given"
                );
                refused += 1;
            }
        }
    }
    assert!(
        refused > 0,
        "the grid holds bodies not given what they read"
    );
    let expected: BTreeSet<TaxonomicClass> = [
        T::S,
        T::Q,
        T::C,
        T::Ch,
        T::K,
        T::Xc,
        T::V,
        T::A,
        T::D,
        T::P,
        T::Red,
        T::Neutral,
    ]
    .into();
    assert_eq!(produced, expected);
}

#[test]
fn the_constants_are_the_rulings() {
    // decision-p14-t51e-qs §3.1: T_Q 1 Myr at 2.5 au, Morris's 30 ÷ 75; decision-p14-t51e-contract
    // §3.3: the veneer's majority, labelled.
    assert_same_bits(Q_TYPE_WEATHERING_TIME.value(), 1.0e6);
    assert_same_bits(Q_TYPE_ANCHOR_DISTANCE.value(), 2.5);
    assert_same_bits(IMMATURE_MATURITY, 0.4);
    assert_same_bits(VENEER_SHARE, 0.5);
}

/// Every `CrustLithology` is placed (rule 4), for an asteroid belt's member and another moon alike,
/// bare, on an old surface at the anchor; the table names every lithology, in order.
#[test]
fn every_lithology_is_placed() {
    use TaxonomicClass as T;
    let table = [
        (CrustLithology::Water, None),
        (CrustLithology::Graphite, None),
        (CrustLithology::Feldspathic, None),
        (CrustLithology::Basalt, Some(T::V)),
        (CrustLithology::IronRichBasalt, Some(T::V)),
        (CrustLithology::Komatiite, None),
        (CrustLithology::ReducedBasalt, None),
        (CrustLithology::Granite, None),
        (CrustLithology::Ultramafic, Some(T::A)),
        (CrustLithology::Phyllosilicate, Some(T::C)),
        (CrustLithology::ChondriteCi, Some(T::C)),
        (CrustLithology::ChondriteCm, Some(T::Ch)),
        (CrustLithology::ChondriteCv, Some(T::K)),
        (CrustLithology::ChondriteH, Some(T::S)),
        (CrustLithology::ChondriteL, Some(T::S)),
        (CrustLithology::ChondriteLl, Some(T::S)),
        (CrustLithology::ChondriteE, Some(T::Xc)),
        (CrustLithology::Cometary, Some(T::P)),
    ];
    assert_eq!(
        table.map(|(lid, _)| lid),
        CrustLithology::ALL,
        "the table names every lithology, in order"
    );
    for kind in BY_LID {
        for (lid, expected) in table {
            assert_eq!(
                class_of(kind, Some(lid), Some(bare()), Some(old_at_anchor())),
                Ok(expected),
                "{} as {kind:?}",
                lid.key()
            );
        }
    }
}

/// A planet is not classed, whatever it holds or lacks.
#[test]
fn a_planet_is_not_classed_whatever_it_holds() {
    for inputs in grid()
        .iter()
        .filter(|inputs| inputs.kind() == TaxonomyBodyKind::Planet)
    {
        assert_eq!(TaxonomicClass::of(inputs), Ok(None), "{inputs:?}");
    }
}

/// A Kuiper-belt body, a member or a moon of one alike (one kind), is neutral at a tholin share of
/// 0 and 0.49 and red at 0.5, 0.9 (perennial CH₄ ice over 0.9 of its surface under a saturated
/// cover, 0.9 × 1) and 1, whatever its lid, given none, and whatever its ice and its clocks.
#[test]
fn a_kuiper_body_is_red_or_neutral_by_its_cover_whatever_its_lid() {
    let expected = [
        (0.0, TaxonomicClass::Neutral),
        (0.49, TaxonomicClass::Neutral),
        (0.5, TaxonomicClass::Red),
        (0.9, TaxonomicClass::Red),
        (1.0, TaxonomicClass::Red),
    ];
    let lids = CrustLithology::ALL.map(Some).into_iter().chain([None]);
    for lid in lids {
        for clocks in [None, Some(fresh_windless()), Some(old_at_anchor())] {
            for (s_t, colour) in expected {
                for s_i in [0.0, 1.0 - s_t] {
                    assert_eq!(
                        class_of(
                            TaxonomyBodyKind::KuiperBody,
                            lid,
                            Some(veneer(s_t, s_i)),
                            clocks
                        ),
                        Ok(Some(colour)),
                        "{lid:?} at s_t {s_t}, s_i {s_i}"
                    );
                }
            }
        }
    }
}

/// A cometary moon under a water-ice cover (`s_i` 0.9, and 0.5 itself) is not classed, and a dry
/// one (`s_i` 0.49 or 0, `s_t` 0) P; a belt member alike.
#[test]
fn a_cometary_moon_under_water_ice_is_not_classed_and_a_dry_one_p() {
    for kind in BY_LID {
        let cometary = |v: Veneer| class_of(kind, Some(CrustLithology::Cometary), Some(v), None);
        assert_eq!(cometary(veneer(0.0, 0.9)), Ok(None), "{kind:?}");
        assert_eq!(cometary(veneer(0.0, 0.5)), Ok(None), "{kind:?}");
        assert_eq!(cometary(veneer(0.0, 0.49)), Ok(Some(TaxonomicClass::P)));
        assert_eq!(cometary(bare()), Ok(Some(TaxonomicClass::P)), "{kind:?}");
    }
}

/// A member or moon whose lid is classed is D under a tholin cover of 0.5 and over, before its ice
/// and its clocks are read, and its lid's class at 0.49; a lid of the closed not-classed set stays
/// unclassed under any veneer.
#[test]
fn a_classed_lid_under_a_majority_tholin_cover_is_d() {
    for kind in BY_LID {
        for lid in CrustLithology::ALL {
            let at = |v: Veneer, clocks| class_of(kind, Some(lid), Some(v), clocks);
            let bare_class = at(bare(), Some(old_at_anchor())).unwrap();
            let Some(own) = bare_class else {
                for v in [veneer(1.0, 0.0), veneer(0.0, 1.0), veneer(0.5, 0.5)] {
                    assert_eq!(at(v, None), Ok(None), "{} as {kind:?}", lid.key());
                }
                continue;
            };
            for v in [veneer(0.5, 0.0), veneer(0.5, 0.5), veneer(1.0, 0.0)] {
                assert_eq!(
                    at(v, None),
                    Ok(Some(TaxonomicClass::D)),
                    "{} as {kind:?}",
                    lid.key()
                );
            }
            assert_eq!(
                at(veneer(0.49, 0.0), Some(old_at_anchor())),
                Ok(Some(own)),
                "{} as {kind:?}",
                lid.key()
            );
        }
    }
}

/// What a body's rule reads and is not given is refused, never none: a member or moon with no lid;
/// a Kuiper-belt body, or a classed lid, with no veneer; an ordinary chondrite left to its lid with
/// no clocks. A not-classed lid reads no veneer, and an icy or covered ordinary chondrite no
/// clocks.
#[test]
fn what_is_not_given_is_refused_not_none() {
    let not_given = |input| Err(ClassifyTaxonomyError::NotGiven(input));
    for kind in BY_LID {
        for veneer in [None, Some(bare())] {
            assert_eq!(
                class_of(kind, None, veneer, Some(old_at_anchor())),
                not_given(MissingTaxonomyInput::Lid),
                "{kind:?}"
            );
        }
        let cv = Some(CrustLithology::ChondriteCv);
        assert_eq!(
            class_of(kind, cv, None, None),
            not_given(MissingTaxonomyInput::Veneer)
        );
        let ice = Some(CrustLithology::Water);
        assert_eq!(class_of(kind, ice, None, None), Ok(None), "{kind:?}");
        for lid in ORDINARY {
            let lid = Some(lid);
            assert_eq!(
                class_of(kind, lid, Some(bare()), None),
                not_given(MissingTaxonomyInput::Clocks)
            );
            assert_eq!(class_of(kind, lid, Some(veneer(0.0, 0.9)), None), Ok(None));
            assert_eq!(
                class_of(kind, lid, Some(veneer(0.9, 0.0)), None),
                Ok(Some(TaxonomicClass::D))
            );
        }
    }
    assert_eq!(
        class_of(TaxonomyBodyKind::KuiperBody, None, None, None),
        not_given(MissingTaxonomyInput::Veneer)
    );
    let error = ClassifyTaxonomyError::NotGiven(MissingTaxonomyInput::Lid);
    sendable(&error);
    assert_eq!(
        error.to_string(),
        "the body's lid lithology is not given, and its class reads it"
    );
}

/// The anchor (decision-p14-t51e-qs §3.2): `τ_Q` on the circular 2.5 au orbit is `T_Q` to 10⁻¹²
/// relative, and about one host it scales as a² √(1 − e²): a 1 au circular twin's is 0.16 of it,
/// (1 ÷ 2.5)², an orbit of e = 0.6 at 2.5 au 0.8 of it, and one at 1 au 0.128; a host with no
/// wind's is infinite. w₀ by hand ([`wind_term_at`]).
#[test]
fn at_the_anchor_the_q_type_time_is_t_q_and_scales_as_a_squared_root_one_minus_e_squared() {
    let t_q = Q_TYPE_WEATHERING_TIME.value();
    for (a, e, share) in [
        (2.5, 0.0, 1.0),
        (1.0, 0.0, 0.16),
        (2.5, 0.6, 0.8),
        (1.0, 0.6, 0.128),
    ] {
        let tau = open(Years::ZERO, wind_term_at(a, e), 0.0)
            .q_type_time()
            .value();
        let expected = share * t_q;
        assert!(
            ((tau - expected) / expected).abs() < 1e-12,
            "a {a} au, e {e}: τ_Q {tau} yr against {expected}"
        );
    }
    let still = fresh_windless().q_type_time().value();
    assert!(still.is_infinite() && still > 0.0, "no wind: τ_Q {still}");
}

/// The clock (decision-p14-t51e-qs §3.2): an ordinary chondrite on the anchor's orbit, with the
/// maturity T51.d gives it there, is Q at `t_x` = 0 (m = 0) and at half of `τ_Q` (m 1.8 × 10⁻⁴),
/// and S at twice `τ_Q` (m 7.2 × 10⁻⁴, the wind's clock alone), at 1 Gyr (m 0.303, where m alone
/// would say Q) and at 4.567 Gyr (m 0.808).
#[test]
fn an_ordinary_chondrite_at_the_anchor_is_q_while_fresh_and_s_once_either_clock_has_run() {
    let w = wind_term_at(2.5, 0.0);
    let tau = open(Years::ZERO, w, 0.0).q_type_time();
    let at = |age: Years| open(age, w, anchor_maturity(age));
    let gyr = |t: f64| Years::from(Gigayears::new(t));
    let m_1 = anchor_maturity(gyr(1.0));
    let m_old = anchor_maturity(gyr(SOLAR_SYSTEM_GYR));
    assert!((m_1 - 0.30).abs() < 0.01, "m at 1 Gyr {m_1}");
    assert!((m_old - 0.81).abs() < 0.01, "m at 4.567 Gyr {m_old}");
    assert!(anchor_maturity(tau * 2.0) < 1e-3);
    for lid in ORDINARY {
        let name = lid.key();
        assert_eq!(
            chondrite(lid, at(Years::ZERO)),
            Some(TaxonomicClass::Q),
            "{name}"
        );
        assert_eq!(
            chondrite(lid, at(tau * 0.5)),
            Some(TaxonomicClass::Q),
            "{name}"
        );
        assert_eq!(
            chondrite(lid, at(tau * 2.0)),
            Some(TaxonomicClass::S),
            "{name}"
        );
        assert_eq!(
            chondrite(lid, at(gyr(1.0))),
            Some(TaxonomicClass::S),
            "{name}"
        );
        assert_eq!(
            chondrite(lid, open(gyr(1.0), 0.0, m_1)),
            Some(TaxonomicClass::Q),
            "{name}: m alone at 1 Gyr"
        );
        assert_eq!(
            chondrite(lid, at(gyr(SOLAR_SYSTEM_GYR))),
            Some(TaxonomicClass::S),
            "{name}"
        );
    }
}

/// A tie on the wind's clock is S: with w₀ = `w_ref` = 0.25, whose products are exact, `t_x` =
/// `T_Q` is S and the year below it Q.
#[test]
fn the_wind_clock_ties_to_s() {
    let wind = WindTerms::new(0.25, 0.25).unwrap();
    let clocks =
        |age: f64| WeatheringClocks::new(Years::new(age), wind, 0.0, AirShield::Open).unwrap();
    let t_q = Q_TYPE_WEATHERING_TIME.value();
    for lid in ORDINARY {
        assert_eq!(chondrite(lid, clocks(t_q)), Some(TaxonomicClass::S));
        assert_eq!(
            chondrite(lid, clocks(t_q.next_down())),
            Some(TaxonomicClass::Q)
        );
    }
}

/// The edges (decision-p14-t51e-qs §3.2): with no wind, an ordinary chondrite is S exactly where
/// m ≥ 0.4, 0.4 itself S and 0.3999 Q, whatever its age: the ruling's dust-only host at 2.5 au
/// (m 0.415 at 4.567 Gyr) is S, and at 5.2 au (m 0.067) Q, labelled, since no agent weathers it.
#[test]
fn with_no_wind_the_class_is_s_exactly_where_the_maturity_reaches_0_4() {
    let old = Years::from(Gigayears::new(SOLAR_SYSTEM_GYR));
    for lid in ORDINARY {
        for age in [Years::ZERO, old] {
            let at = |m: f64| chondrite(lid, open(age, 0.0, m));
            assert_eq!(at(IMMATURE_MATURITY), Some(TaxonomicClass::S));
            assert_eq!(at(0.4), Some(TaxonomicClass::S));
            assert_eq!(at(0.4_f64.next_down()), Some(TaxonomicClass::Q));
            assert_eq!(at(0.3999), Some(TaxonomicClass::Q));
            assert_eq!(at(1.0), Some(TaxonomicClass::S));
        }
        assert_eq!(
            chondrite(lid, open(old, 0.0, 0.415)),
            Some(TaxonomicClass::S)
        );
        assert_eq!(
            chondrite(lid, open(old, 0.0, 0.067)),
            Some(TaxonomicClass::Q)
        );
    }
}

/// Under air above the ion floor an ordinary chondrite is S even on a fresh, windless surface; the
/// same clocks open are Q.
#[test]
fn under_air_an_ordinary_chondrite_is_s() {
    for lid in ORDINARY {
        assert_eq!(chondrite(lid, shielded_fresh()), Some(TaxonomicClass::S));
        assert_eq!(chondrite(lid, fresh_windless()), Some(TaxonomicClass::Q));
    }
}

/// No lithology but an ordinary chondrite is ever Q, and only a bare member's or moon's: every
/// kind, every lid and none, every veneer, on a fresh surface with no wind and with the anchor's.
#[test]
fn no_lithology_but_an_ordinary_chondrite_is_ever_q() {
    let fresh = [
        fresh_windless(),
        open(Years::ZERO, REFERENCE_WIND_TERM, 0.0),
    ];
    // Each veneer with whether it is bare.
    let veneers = [
        (bare(), true),
        (veneer(0.9, 0.1), false),
        (veneer(0.0, 0.9), false),
    ];
    let lids = CrustLithology::ALL.map(Some).into_iter().chain([None]);
    for kind in KINDS {
        for lid in lids.clone() {
            for (v, is_bare) in veneers {
                for clocks in fresh {
                    let q =
                        class_of(kind, lid, Some(v), Some(clocks)) == Ok(Some(TaxonomicClass::Q));
                    let ordinary = lid.is_some_and(|lid| ORDINARY.contains(&lid))
                        && BY_LID.contains(&kind)
                        && is_bare;
                    assert_eq!(q, ordinary, "{lid:?} as {kind:?} under {v:?}");
                }
            }
        }
    }
}

/// Deterministic, with no draw: the class reads no seed and no stream, so the same inputs give the
/// same results twice, and in the reverse order.
#[test]
fn the_class_is_deterministic_with_no_draw() {
    let inputs = grid();
    let forward: Vec<_> = inputs.iter().map(TaxonomicClass::of).collect();
    let again: Vec<_> = inputs.iter().map(TaxonomicClass::of).collect();
    let mut backward: Vec<_> = inputs.iter().rev().map(TaxonomicClass::of).collect();
    backward.reverse();
    assert_eq!(forward, again);
    assert_eq!(forward, backward);
}

#[test]
fn the_inputs_keep_their_values() {
    let wind = winds(0.3);
    assert_same_bits(wind.wind_term(), 0.3);
    assert_same_bits(wind.reference_wind_term(), REFERENCE_WIND_TERM);
    let clocks =
        WeatheringClocks::new(Years::new(2.0e6), wind, 0.25, AirShield::AboveIonFloor).unwrap();
    assert_same_bits(clocks.exposure_age().value(), 2.0e6);
    assert_same_bits(clocks.wind_term(), 0.3);
    assert_same_bits(clocks.reference_wind_term(), REFERENCE_WIND_TERM);
    assert_same_bits(clocks.maturity(), 0.25);
    assert_eq!(clocks.air(), AirShield::AboveIonFloor);
    let v = veneer(0.75, 0.125);
    assert_same_bits(v.tholin_share(), 0.75);
    assert_same_bits(v.ice_share(), 0.125);
    let lid = Some(CrustLithology::ChondriteLl);
    let inputs = TaxonomyInputs::new(TaxonomyBodyKind::OtherMoon, lid, Some(v), Some(clocks));
    assert_eq!(inputs.kind(), TaxonomyBodyKind::OtherMoon);
    assert_eq!(inputs.lid(), lid);
    assert_eq!(inputs.veneer(), Some(v));
    assert_eq!(inputs.clocks(), Some(clocks));
}

/// An input not finite or out of its range is refused, naming it: the exposure age and the wind
/// term below 0, the reference wind term at or below 0, the maturity and the shares outside
/// [0, 1], and two shares summing above 1 + 10⁻¹²; `s_t` with 1 − `s_t` is accepted.
#[test]
fn the_inputs_out_of_range_are_refused() {
    fn refused<T>(input: TaxonomyInput) -> Result<T, BuildTaxonomyInputsError> {
        Err(BuildTaxonomyInputsError::OutOfRange(input))
    }
    let w = REFERENCE_WIND_TERM;
    let clocks =
        |age: f64, m: f64| WeatheringClocks::new(Years::new(age), winds(w), m, AirShield::Open);
    for bad in [f64::NAN, -1.0, f64::INFINITY] {
        assert_eq!(clocks(bad, 0.0), refused(TaxonomyInput::ExposureAge));
        assert_eq!(WindTerms::new(bad, w), refused(TaxonomyInput::WindTerm));
        assert_eq!(
            WindTerms::new(w, bad),
            refused(TaxonomyInput::ReferenceWindTerm)
        );
    }
    assert_eq!(
        WindTerms::new(w, 0.0),
        refused(TaxonomyInput::ReferenceWindTerm)
    );
    for bad in [f64::NAN, -0.01, 1.01, f64::INFINITY] {
        assert_eq!(clocks(0.0, bad), refused(TaxonomyInput::Maturity));
        assert_eq!(Veneer::new(bad, 0.0), refused(TaxonomyInput::TholinShare));
        assert_eq!(Veneer::new(0.0, bad), refused(TaxonomyInput::IceShare));
    }
    for (s_t, s_i) in [(1.0, 1.0), (0.9, 0.2), (0.5, 0.5 + 1.0e-11)] {
        assert_eq!(
            Veneer::new(s_t, s_i),
            refused(TaxonomyInput::VeneerSum),
            "s_t {s_t}, s_i {s_i}"
        );
    }
    assert!(WindTerms::new(0.0, w).is_ok());
    assert!(clocks(0.0, 1.0).is_ok());
    for s_t in [0.0, 0.1, 1.0 / 3.0, 0.5, 0.7, 0.9, 1.0] {
        assert!(Veneer::new(s_t, 1.0 - s_t).is_ok(), "s_t {s_t}");
    }
    assert!(Veneer::new(0.5, 0.5 + 1.0e-13).is_ok());
    let error = clocks(f64::NAN, 0.0).unwrap_err();
    sendable(&error);
    assert_eq!(
        error.to_string(),
        "the body's exposure age is not finite or out of its range"
    );
    assert_eq!(
        Veneer::new(0.0, 2.0).unwrap_err().to_string(),
        "the body's uncovered ice share is not finite or out of its range"
    );
    assert_eq!(
        Veneer::new(1.0, 1.0).unwrap_err().to_string(),
        "the body's tholin and ice shares' sum is not finite or out of its range"
    );
}

/// Every `.rs` file under `dir`, recursively, in sorted order.
fn source_files(dir: &Path, found: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            source_files(&path, found);
        } else if path.extension().is_some_and(|e| e == "rs") {
            found.push(path);
        }
    }
}

/// The code of a source file as the tripwire scans it: its lines up to its inline test module,
/// the first `#[cfg(test)]` whose next line opens `mod tests {`, its comment lines left out (a doc
/// may name the module).
fn scanned_code(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let end = lines
        .windows(2)
        .position(|pair| {
            pair[0].trim() == "#[cfg(test)]" && pair[1].trim_start().starts_with("mod tests {")
        })
        .unwrap_or(lines.len());
    lines[..end]
        .iter()
        .filter(|line| !line.trim_start().starts_with("//"))
        .copied()
        .collect::<Vec<_>>()
        .join("\n")
}

/// Nothing on the record reads a small body's taxonomic class at generator version 23
/// (decision-p14-t51e-contract §3.7). P14.T51.e's wiring, `planetary/system/small_bodies.rs` behind
/// `SMALL_BODY_TYPES_VERSION` = 24, joins the exempt files below with its tests, and P14.T54.b.l
/// deletes this test with the other source scans.
///
/// It walks the sim's `src/` outside `planetary/hooks/small_bodies*`, in sorted order, and fails
/// on a path or public name of the module in each file's code (the file less its comment lines
/// and its inline test module). `hooks/mod.rs`'s `pub mod small_bodies;` names neither. The bare
/// `small_bodies::` is not matched, since the galaxy's `substellar` module has a private module of
/// that name. `TaxonomyInput` also matches `TaxonomyInputs`, `MissingTaxonomyInput` and
/// `BuildTaxonomyInputsError`.
#[test]
fn small_bodies_nothing_on_the_record_reads_the_types_yet() {
    const PATTERNS: [&str; 14] = [
        "hooks::small_bodies",
        "super::small_bodies",
        "TaxonomicClass",
        "TaxonomyInput",
        "TaxonomyBodyKind",
        "WindTerms",
        "WeatheringClocks",
        "ClassifyTaxonomyError",
        "Veneer",
        "AirShield",
        "Q_TYPE_WEATHERING_TIME",
        "Q_TYPE_ANCHOR_DISTANCE",
        "IMMATURE_MATURITY",
        "VENEER_SHARE",
    ];
    // Files allowed to match: the wiring and its tests, once they exist; none yet.
    const EXEMPT: [&str; 1] = ["planetary/hooks/small_bodies"];
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    source_files(&src, &mut files);
    assert!(files.len() > 100, "the walk found the sim's sources");
    let mut readers = Vec::new();
    for path in files {
        let relative = path
            .strip_prefix(&src)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if EXEMPT.iter().any(|prefix| relative.starts_with(prefix)) {
            continue;
        }
        let code = scanned_code(&std::fs::read_to_string(&path).unwrap());
        for line in code.lines() {
            for pattern in PATTERNS {
                if line.contains(pattern) {
                    readers.push(format!("{relative}: {pattern}"));
                }
            }
        }
    }
    assert_eq!(
        readers,
        Vec::<String>::new(),
        "a small body's taxonomic class is read outside its module before P14.T51.e's wiring, \
         which joins this scan's exempt files"
    );
    let hooks = std::fs::read_to_string(src.join("planetary/hooks/mod.rs")).unwrap();
    assert_eq!(hooks.matches("pub mod small_bodies;").count(), 1);
}
