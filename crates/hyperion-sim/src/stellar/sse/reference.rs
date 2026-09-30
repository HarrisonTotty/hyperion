//! Reference values for the backbone (plan 06, P06.T4–T6): a golden of its closed forms, bit for
//! bit, and a comparison of the giant branches' landmarks with the published SSE code.
//!
//! The landmark test exercises `gb.rs` alone and belongs in its tests; it is here, beside the
//! golden, only while P06.T7–T9 are being built in `gb.rs` in parallel (round 6), and moves there
//! once they merge.
//!
//! The comparison tests hold the formulae to the published SSE code only to 10⁻⁹, which a
//! rewrite that changes the last bits, or another architecture's arithmetic, would pass. This
//! pins every coefficient of [`ZCoeffs`] and the critical masses at the five metallicities of the
//! SSE run, and luminosity, radius, core mass and timescales at fixed points of the main
//! sequence, the Hertzsprung gap and the first giant branch, so that any change to their form is
//! seen, and is a generator-version change once the track integrator (P06.T10) reads them.

use hyperion_testkit::float::assert_same_bits;
use hyperion_testkit::golden;
use hyperion_testkit::golden::GoldenWriter;

use super::coeffs::ZCoeffs;
use super::gb::{self, FirstGiantBranch};
use super::hg::HertzsprungGap;
use super::ms::{self, MainSequence};
use super::{PhasePoint, zams};
use crate::GENERATOR_VERSION;
use crate::units::{Megayears, MetalFraction, SolarMasses};

/// The five metallicities of the published SSE comparison.
const METALLICITIES: [f64; 5] = [1e-4, 1e-3, 4e-3, 0.02, 0.03];

/// Masses across every regime: fully convective, the hook, the helium flash, `M_FGB` at each Z.
const MASSES: [f64; 12] = [
    0.1, 0.3, 0.8, 1.0, 1.5, 2.0, 2.5, 5.0, 12.0, 20.0, 60.0, 100.0,
];

fn point(w: &mut GoldenWriter, label: &str, p: PhasePoint) {
    w.f64(&format!("{label} L"), p.luminosity.value());
    w.f64(&format!("{label} R"), p.radius.value());
    w.f64(&format!("{label} Mc"), p.core_mass.value());
}

#[test]
fn backbone_closed_forms_are_pinned() {
    let mut w = GoldenWriter::new();
    w.header(GENERATOR_VERSION.get());
    for z in METALLICITIES {
        let c = ZCoeffs::new(MetalFraction::new(z));
        w.line("");
        w.line(&format!("Z = {z}"));
        w.f64("zeta", c.zeta());
        w.f64("m_hook", c.m_hook().value());
        w.f64("m_hef", c.m_hef().value());
        w.f64("m_fgb", c.m_fgb().value());
        for n in 1..=81 {
            w.f64(&format!("a{n}"), c.a(n));
        }
        for n in 1..=57 {
            w.f64(&format!("b{n}"), c.b(n));
        }
        for (i, v) in c.zams_l().iter().enumerate() {
            w.f64(&format!("zams_l[{i}]"), *v);
        }
        for (i, v) in c.zams_r().iter().enumerate() {
            w.f64(&format!("zams_r[{i}]"), *v);
        }
        for m in MASSES {
            let mass = SolarMasses::new(m);
            w.line(&format!("M = {m}"));
            w.f64("zams L", zams::luminosity(mass, &c).value());
            w.f64("zams R", zams::radius(mass, &c).value());
            w.f64("t_bgb", ms::t_bgb(mass, &c).value());
            w.f64("t_hook", ms::t_hook(mass, &c).value());
            w.f64("t_ms", ms::t_ms(mass, &c).value());
            w.f64("l_tms", ms::l_tms(mass, &c).value());
            w.f64("r_tms", ms::r_tms(mass, &c).value());
            w.f64("l_bgb", ms::l_bgb(mass, &c).value());
            w.f64("l_hei", gb::l_hei(mass, &c).value());
            w.f64("mc_hei", gb::mc_hei(mass, &c).value());
            w.f64("mc_bgb", gb::mc_bgb(mass, &c).value());
            w.f64("mc_bagb", gb::mc_bagb(mass, &c).value());
            let main = MainSequence::new(mass, &c);
            let t_ms = main.t_ms().value();
            for tau in [0.25, 0.5, 0.9, 0.99, 1.0] {
                point(
                    &mut w,
                    &format!("ms {tau}"),
                    main.at(Megayears::new(tau * t_ms)),
                );
            }
            let gap = HertzsprungGap::new(mass, &c);
            let (start, end) = (gap.t_start().value(), gap.t_end().value());
            for tau in [0.5, 1.0] {
                let t = Megayears::new(start + tau * (end - start));
                point(&mut w, &format!("hg {tau}"), gap.at(t));
            }
            if m < c.m_fgb().value() {
                let branch = FirstGiantBranch::new(mass, &c);
                let t_hei = branch.t_hei().value();
                w.f64("t_hei", t_hei);
                for tau in [0.5, 0.9, 1.0] {
                    let t = Megayears::new(end + tau * (t_hei - end));
                    point(&mut w, &format!("gb {tau}"), branch.at(t));
                }
            }
        }
    }
    golden!("stellar/sse", w.as_str());
}

/// Asserts `ours` is within a relative `tolerance` of the published SSE code's `theirs`.
#[track_caller]
fn assert_near(what: &str, ours: f64, theirs: f64, tolerance: f64) {
    assert!(
        (ours / theirs - 1.0).abs() < tolerance,
        "{what}: {ours} against the SSE code's {theirs}"
    );
}

/// The landmarks of the giant branches that the gap's end and later phases start from, against
/// the published SSE code's own functions (see [`sse`](super) for the run; its `lHeIf`, `rminf`,
/// `ragbf`, `rgbf`, `tblf`, `mcagbf` and `mcheif`): `L_HeI`, the giant and asymptotic-giant radii
/// at it, `Mc,BAGB` and, from `M_HeF` up, `R_mHe` to 10⁻¹²; `Mc,BGB` and `Mc,HeI` to 10⁻⁷ (equation
/// 44's printed c₁); above `M_FGB` the blue-phase fraction to 2 × 10⁻³ (it is normalised at
/// `M_FGB`, whose printed constants are rounded) and the radius at ignition to 10⁻¹², which is
/// `R_AGB`(`L_HeI`) where the blue phase vanishes and `R_mHe` from 12 M☉ where it does not. The
/// gap's own tests compare its end with these functions, so this is what checks them.
#[test]
fn giant_branch_landmarks_match_the_published_sse_code() {
    let mut red_supergiant_ignitions = 0;
    for &[
        z,
        m,
        l_hei,
        r_mhe,
        r_agb,
        r_gb,
        tau_bl,
        mc_bagb,
        mc_bgb,
        mc_hei,
    ] in SSE_LANDMARKS
    {
        let c = ZCoeffs::new(MetalFraction::new(z));
        let mass = SolarMasses::new(m);
        let what = |name: &str| format!("{name} at Z = {z}, M = {m}");
        let ours = gb::l_hei(mass, &c);
        assert_near(&what("L_HeI"), ours.value(), l_hei, 1e-12);
        assert_near(
            &what("R_GB(L_HeI)"),
            gb::radius(mass, ours, &c).value(),
            r_gb,
            1e-12,
        );
        let agb = gb::agb_radius(mass, ours, &c).value();
        assert_near(&what("R_AGB(L_HeI)"), agb, r_agb, 1e-12);
        assert_near(
            &what("Mc,BAGB"),
            gb::mc_bagb(mass, &c).value(),
            mc_bagb,
            1e-12,
        );
        if m >= c.m_hef().value() {
            let rmin = gb::r_mhe_intermediate(mass, &c).value();
            assert_near(&what("R_mHe"), rmin, r_mhe, 1e-12);
            assert_near(&what("Mc,BGB"), gb::mc_bgb(mass, &c).value(), mc_bgb, 1e-7);
            assert_near(&what("Mc,HeI"), gb::mc_hei(mass, &c).value(), mc_hei, 1e-7);
        }
        if m > c.m_fgb().value() {
            let ours = gb::blue_fraction_massive(mass, &c);
            if tau_bl <= 0.0 {
                assert_same_bits(ours, 0.0);
                red_supergiant_ignitions += 1;
                assert_near(&what("R_HeI"), gb::r_hei(mass, &c).value(), r_agb, 1e-12);
            } else {
                assert_near(&what("tau_bl"), ours, tau_bl, 2e-3);
                if m >= 12.0 {
                    assert_near(&what("R_HeI"), gb::r_hei(mass, &c).value(), r_mhe, 1e-12);
                }
            }
        }
    }
    assert!(
        red_supergiant_ignitions >= 3,
        "{red_supergiant_ignitions} cases"
    );
}

/// (Z, M, `L_HeI`, `R_mHe`, `R_AGB`(`L_HeI`), `R_GB`(`L_HeI`), `τ_bl`, `Mc,BAGB`, `Mc,BGB`,
/// `Mc,HeI`) from the published SSE code's functions at 24 of its run's (Z, M) points.
#[expect(
    clippy::unreadable_literal,
    reason = "the SSE run's output digits, compared by eye with its listing"
)]
const SSE_LANDMARKS: &[[f64; 10]] = &[
    [
        0.0001,
        0.8,
        2009.3683120177216,
        3.2002327244104514,
        81.25587275979711,
        84.3977684711494,
        1.0,
        0.5407806075010706,
        0.26349410064984596,
        0.3178298465589933,
    ],
    [
        0.0001,
        1.75,
        760.7123306605883,
        6.1091937176326665,
        42.26793857788969,
        42.7663002521758,
        1.0,
        0.6117181799088687,
        0.28594987016319867,
        0.3314545455689449,
    ],
    [
        0.0001,
        2.25,
        244.42023491207812,
        7.320045803200934,
        21.154184505103903,
        21.106245130639593,
        0.8984907380625107,
        0.7124341676551791,
        0.33201369502803063,
        0.36389593732696224,
    ],
    [
        0.0001,
        5.0,
        2871.873619375666,
        11.564597427934233,
        78.41770984369785,
        78.37321035514587,
        1.0,
        1.5886211858361794,
        0.8629689009633087,
        0.8650556375858395,
    ],
    [
        0.0001,
        10.0,
        25632.039533603103,
        15.263966178163118,
        258.7464230787315,
        260.9674525445041,
        1.0,
        3.4501045398568935,
        2.196000454264692,
        2.1961275388933075,
    ],
    [
        0.0001,
        25.0,
        244592.41756642883,
        28.44000945979663,
        884.4128785047444,
        903.0488605414221,
        1.0,
        9.656670243897805,
        7.569194692127538,
        7.569197795820166,
    ],
    [
        0.001,
        1.0,
        2288.7669680728127,
        6.674290437971021,
        96.73727904938387,
        98.74830429316405,
        1.0,
        0.5323707265627428,
        0.218216839534992,
        0.3324237781665589,
    ],
    [
        0.001,
        2.0,
        300.08546290176565,
        12.748934402176245,
        24.85389130094731,
        24.643225839856676,
        0.9154041666741792,
        0.6337749794711711,
        0.27912343589999233,
        0.35573311166342814,
    ],
    [
        0.001,
        12.0,
        41890.32156314098,
        31.912191054296486,
        333.29813833146756,
        387.80724056419007,
        1.0,
        4.23980542741964,
        2.8090042059243014,
        2.809116360188,
    ],
    [
        0.001,
        100.0,
        3422386.1717354925,
        5558.625696291807,
        3689.344021815392,
        6970.165415194206,
        0.0,
        48.65711620062879,
        46.22426039059735,
        46.22426039059735,
    ],
    [
        0.004,
        1.75,
        1014.3633559780104,
        14.84084584056519,
        58.898989103858355,
        60.83060327187856,
        1.0,
        0.5563050999530501,
        0.2146823921315776,
        0.33357827534359963,
    ],
    [
        0.004,
        4.0,
        1481.662482537179,
        30.078525870576932,
        58.67981612285074,
        61.75220218213428,
        0.5475200563185041,
        1.074655587024853,
        0.6372690162398043,
        0.6469549065273724,
    ],
    [
        0.004,
        20.0,
        142735.73743645093,
        64.5419186151276,
        739.5524854364701,
        841.4560413608415,
        0.658331221325251,
        7.451764816378301,
        5.599733225028668,
        5.599747829734661,
    ],
    [
        0.004,
        100.0,
        3243113.103266819,
        54406.399252062656,
        3338.315291041472,
        4582.5095135922875,
        0.0,
        52.401339645648605,
        49.22254301220445,
        49.22254303370771,
    ],
    [
        0.02,
        1.0,
        2751.62190311728,
        14.15809959206737,
        166.22141058256423,
        173.82984541181068,
        1.0,
        0.5122283201423554,
        0.4866169041352376,
        0.2838723455910695,
    ],
    [
        0.02,
        1.75,
        2147.579443523305,
        24.65462860299371,
        122.14267894988275,
        126.45436864619654,
        1.0,
        0.5258947172370024,
        0.49959998137515227,
        0.30178662575026777,
    ],
    [
        0.02,
        2.0,
        244.08256777464635,
        28.08033189053526,
        27.86450437796565,
        27.66113501857422,
        0.9985940985750132,
        0.5393568476363269,
        0.21123850226181123,
        0.31854326883544526,
    ],
    [
        0.02,
        3.0,
        630.558699557009,
        40.93959053812598,
        45.22738101746098,
        46.62533136555486,
        0.8015541478181719,
        0.6711356537383218,
        0.4259211659436387,
        0.45056940864830985,
    ],
    [
        0.02,
        20.0,
        133189.3938402057,
        321.17340657951337,
        901.4421735791698,
        1019.5932700266435,
        0.029577094961967924,
        7.18863593360428,
        5.599730186011905,
        5.599742010391341,
    ],
    [
        0.02,
        60.0,
        1139606.7369552066,
        51332.8650989572,
        2585.6145962135856,
        3248.931294231287,
        0.0,
        30.121138430944406,
        24.691378129051785,
        24.691378266977438,
    ],
    [
        0.03,
        1.0,
        2814.263267328014,
        23.87035938568998,
        186.03033972408065,
        194.3161715197944,
        1.0,
        0.509992047831467,
        0.4844924454398936,
        0.26969662926183224,
    ],
    [
        0.03,
        2.25,
        282.24051335463014,
        53.174837257612616,
        31.566047120657,
        31.041833742478882,
        0.9366826130892836,
        0.5504684280847469,
        0.26287222629154533,
        0.3347506076627229,
    ],
    [
        0.03,
        15.0,
        68659.84030295668,
        136.11751811890426,
        675.4449161233209,
        761.9832484212551,
        0.0,
        4.9215303439428295,
        3.796902558729687,
        3.796938099988299,
    ],
    [
        0.03,
        100.0,
        2791735.649074465,
        824044.9770249121,
        4062.6253401847143,
        5973.6248115262815,
        0.0,
        61.218892302366896,
        49.22254300630442,
        49.222543022617465,
    ],
];
