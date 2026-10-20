use hyperion_sim::substance::optics::{FRESH_SNOW, LUNAR_REGOLITH};

use super::super::h_function;
use super::*;

/// The fast H is the task's H: [`HAtMu`] agrees with [`h_function`] to 10⁻¹³, relative, at every
/// geometry the inversions take and across w, conservative scattering included.
#[test]
fn substance_albedos_the_fast_h_is_the_task_s_h() {
    let (asd0, asd, _) = cosines(ASD_GEOMETRY);
    let beckman = math::cos(BECKMAN_INCIDENCE_DEG.to_radians());
    for mu in [asd0, asd, beckman, 0.5, 1.0, 0.05] {
        let fast = HAtMu::new(mu);
        for w in [0.0, 0.05, 0.3, 0.9, 0.999, 0.999_95, 1.0] {
            let (a, b) = (fast.at(w), h_function(w, mu));
            assert!(
                (a / b - 1.0).abs() < 1e-13,
                "mu {mu}, w {w}: {a} against {b}"
            );
        }
    }
}

/// The geometries' cosines: the ASD's phase is 35.5° (DS 1035, p. 7), and source and detector on
/// opposite sides give a phase of i + e.
#[test]
fn substance_albedos_the_geometries_give_their_phase() {
    let (_, _, cos_g) = cosines(ASD_GEOMETRY);
    let phase = math::acos(cos_g).to_degrees();
    assert!((phase - 35.53).abs() < 0.01, "{phase}");
    let opposite = Geometry::new(20.0, 60.0, 180.0).unwrap();
    let (_, _, cos_g) = cosines(opposite);
    assert!((math::acos(cos_g).to_degrees() - 80.0).abs() < 1e-9);
}

/// Each forward model rises strictly with w from 0 at w = 0, the Beckman's to 1 at w = 1, so that
/// an inversion's root is unique.
#[test]
fn substance_albedos_the_forward_models_rise_with_w() {
    let asymmetry = LUNAR_REGOLITH.phase_function().asymmetry();
    let snow = Geometry::new(20.0, 60.0, 180.0).unwrap();
    for measurement in [
        Measurement::beckman(asymmetry),
        Measurement::asd(&LUNAR_REGOLITH),
        Measurement::reflectance_factor(snow, &FRESH_SNOW),
    ] {
        let table = InversionTable::new(measurement.clone());
        assert!(
            table.values.windows(2).all(|p| p[0] > p[1]),
            "{measurement:?}"
        );
        assert!(measurement.forward(0.0).abs() < 1e-15);
    }
    assert!((Measurement::beckman(asymmetry).forward(1.0) - 1.0).abs() < 1e-15);
}

/// Each inversion reproduces its value at its own geometry: across the range, at each of the three
/// measurements, |model(w) − value| below 10⁻¹³, a value above the model's at w = 1 clipped bright
/// and one at 0 or below clipped dark.
#[test]
fn substance_albedos_the_inversion_reproduces_its_value() {
    let asymmetry = LUNAR_REGOLITH.phase_function().asymmetry();
    let snow = Geometry::new(20.0, 60.0, 180.0).unwrap();
    for measurement in [
        Measurement::beckman(asymmetry),
        Measurement::asd(&LUNAR_REGOLITH),
        Measurement::reflectance_factor(snow, &FRESH_SNOW),
    ] {
        let table = InversionTable::new(measurement.clone());
        let top = measurement.forward(1.0);
        for j in 1..200_u32 {
            let value = top * f64::from(j) / 200.0;
            let (w, clip) = invert(&table, value);
            assert_eq!(clip, None);
            let residual = (measurement.forward(w) - value).abs();
            assert!(
                residual < 1e-13,
                "{measurement:?}: {value} gives {w}, off {residual}"
            );
        }
        assert_eq!(invert(&table, top * 1.001), (1.0, Some(Clip::Bright)));
        assert_eq!(invert(&table, 0.0), (0.0, Some(Clip::Dark)));
        assert_eq!(invert(&table, -0.01), (0.0, Some(Clip::Dark)));
    }
}

/// The slab's rescale: at the laboratory grain it is the identity; a transparent grain brightens as
/// it gets finer and darkens as it coarsens; a w below `S_e` (no Θ gives it) and a held end-member
/// are held at any size; and Θ = 1 stays lossless.
#[test]
fn substance_albedos_the_slab_rescales_by_its_transmission() {
    let scaling = GrainScaling::Slab {
        real_index: 1.58,
        lab_grain_diameter: Metres::new(162.0e-6),
    };
    let (s_e, s_i) = surface_terms(1.58);
    for w in [0.2, 0.6, 0.9, 0.99] {
        let same = rescale(w, scaling, Metres::new(162.0e-6));
        assert!((same - w).abs() < 1e-14, "{w}: {same}");
        let finer = rescale(w, scaling, Metres::new(70.0e-6));
        let coarser = rescale(w, scaling, Metres::new(500.0e-6));
        assert!(finer > w && coarser < w, "{w}: {finer}, {coarser}");
        let theta = slab_theta(w, s_e, s_i);
        assert!((slab_w(theta, s_e, s_i) - w).abs() < 1e-14);
    }
    assert!((rescale(1.0, scaling, Metres::new(70.0e-6)) - 1.0).abs() < 1e-15);
    let dark = s_e * 0.8;
    assert!(
        rescale(dark, scaling, Metres::new(70.0e-6))
            .total_cmp(&dark)
            .is_eq()
    );
    let held = rescale(0.3, GrainScaling::Held, Metres::new(1.0e-6));
    assert!(held.total_cmp(&0.3).is_eq());
}

/// The cosine-weighted hemispherical Fresnel reflectance of a dielectric, Martinez-Anton 2026's
/// `R_H` (Sensors 26, 4031, CC BY 4.0, "integrating for all angles in a weighted manner and for
/// unpolarized light"): 0.08 at n = 1.42, 0.0918 at 1.5 and 0.12 at 1.7, each to its printed
/// digits.
const HEMISPHERICAL_REFLECTANCES: [(f64, f64, f64); 3] =
    [(1.42, 0.08, 0.005), (1.5, 0.0918, 5e-5), (1.7, 0.12, 0.005)];

/// The complex Fresnel surface term: a dielectric's meets Martinez-Anton 2026's values; at k ≪ 1 it
/// lies within 4% of Hapke 2012's fit `S_e` = 0.0587 + 0.8543 R(0) + 0.0870 R(0)² over n 1.5–2.8,
/// the fit's range for silicates (0.0918 against 0.0930 at n 1.5, the prototype's check); normal
/// incidence gives R(0), grazing incidence reflects all; an opaque metal's is above its R(0)'s
/// fit would say at grazing angles, and the reflectance rises with k.
#[test]
fn substance_albedos_the_complex_fresnel_term_is_the_hemisphere_s() {
    for (n, expected, tolerance) in HEMISPHERICAL_REFLECTANCES {
        let s_e = hemispherical_fresnel_reflectance(n, 0.0);
        assert!(
            (s_e - expected).abs() <= tolerance,
            "n {n}: {s_e} against {expected}"
        );
    }
    for n in [1.5, 1.6, 1.8, 2.0, 2.4, 2.8] {
        let exact = hemispherical_fresnel_reflectance(n, 1e-6);
        let (fit, _) = surface_terms(n);
        assert!(
            (fit / exact - 1.0).abs() < 0.04,
            "n {n}: {exact} against the fit {fit}"
        );
    }
    for (n, k) in [(1.5, 0.0), (2.95, 2.93), (1.57, 0.53)] {
        let r0 = ((n - 1.0) * (n - 1.0) + k * k) / ((n + 1.0) * (n + 1.0) + k * k);
        assert!(
            (fresnel_reflectance(n, k, 1.0) - r0).abs() < 1e-14,
            "{n} {k}"
        );
        assert!(
            (fresnel_reflectance(n, k, 0.0) - 1.0).abs() < 1e-12,
            "{n} {k}"
        );
    }
    let mut last = 0.0;
    for k in [0.0, 0.5, 1.0, 2.0, 3.0] {
        let s_e = hemispherical_fresnel_reflectance(2.9, k);
        assert!(s_e > last, "k {k}: {s_e}");
        last = s_e;
    }
}

/// The quadrature of the hemisphere is converged: the surface term at 128 nodes is within 10⁻⁹ of
/// the midpoint rule's at 200,000 intervals, for a dielectric and a metal.
#[test]
fn substance_albedos_the_fresnel_quadrature_converges() {
    for (n, k) in [(1.5, 0.0), (2.95, 2.93)] {
        let intervals = 200_000_u32;
        let h = 1.0 / f64::from(intervals);
        let midpoint: f64 = (0..intervals)
            .map(|j| {
                let mu = (f64::from(j) + 0.5) * h;
                2.0 * mu * fresnel_reflectance(n, k, mu) * h
            })
            .sum();
        let gauss = hemispherical_fresnel_reflectance(n, k);
        assert!(
            (gauss - midpoint).abs() < 1e-9,
            "{n} {k}: {gauss} against {midpoint}"
        );
    }
}

/// A splib07a file reads: its header names its record, its values follow, and a malformed one is
/// refused.
#[test]
fn substance_albedos_a_splib_file_reads() {
    let text = " splib07a Record=1403: Anorthite HS201.3B Plagio     BECKa AREF\n 3.1e-002\n\
                -1.2300000e+034\n 0.5\n";
    let (record, values) = splib_values("x.txt", text).unwrap();
    assert_eq!(record, "Anorthite_HS201.3B_Plagio_BECKa_AREF");
    assert_eq!(values.len(), 3);
    assert!(values[1] < DELETED_BELOW);
    assert!(matches!(
        splib_values("x.txt", "not a header\n1\n"),
        Err(FitAlbedoError::Spectrum { .. })
    ));
    assert!(matches!(
        splib_values("x.txt", " splib07a Record=1: A B\n one\n"),
        Err(FitAlbedoError::Spectrum { .. })
    ));
}

/// A mixture's w is the cross-section weighting of its parts: two held end-members of constant w
/// give Σ (f ÷ D) w ÷ Σ (f ÷ D), a fine part counting 70 times its volume at the form's 70 µm.
#[test]
fn substance_albedos_a_mixture_weights_by_cross_section() {
    let wavelengths = [(400.0, 0.5), (900.0, 0.5)];
    let grain = |w: f64| EndMemberGrain {
        scaling: GrainScaling::Held,
        spectra: vec![InvertedSpectrum {
            channels: wavelengths.iter().map(|&(nm, _)| (nm, w)).collect(),
            bright: 0,
            dark: 0,
            residual: 0.0,
        }],
        reach: [400.0, 900.0],
    };
    let mixture = MixtureGrain {
        parts: vec![
            (Arc::new(grain(0.9)), 0.9, GrainClass::Form),
            (Arc::new(grain(0.1)), 0.1, GrainClass::Fine),
        ],
        reach: [400.0, 900.0],
    };
    let d = Metres::new(70.0e-6);
    let (a, b) = (0.9 / 70.0e-6, 0.1 / 1.0e-6);
    let expected = (a * 0.9 + b * 0.1) / (a + b);
    let w = mixture.single_scattering_albedo(550.0, d);
    assert!((w - expected).abs() < 1e-15, "{w} against {expected}");
    let weights = mixture.weights(d);
    assert!((weights[1] / weights[0] - 70.0 / 9.0).abs() < 1e-12);
}

/// A splib07a file of record `record` holding `values`, one a line.
fn splib_file(record: &str, values: &[f64]) -> String {
    use std::fmt::Write as _;
    let mut text = format!(" splib07a Record=1: {record}\n");
    for value in values {
        writeln!(text, " {value}").unwrap();
    }
    text
}

/// A library of the Beckman's wavelengths, 400, 450 and 500 nm, the ASD's, 600 and 700 nm, and
/// `files`, each (file name, record, values).
fn library(files: &[(&str, &str, &[f64])]) -> LabLibrary {
    let mut texts = std::collections::BTreeMap::from([
        (
            BECKMAN_WAVELENGTHS.to_owned(),
            splib_file("Wavelengths BECK", &[0.4, 0.45, 0.5]),
        ),
        (
            ASD_WAVELENGTHS.to_owned(),
            splib_file("Wavelengths ASD", &[0.6, 0.7]),
        ),
    ]);
    for &(file, record, values) in files {
        texts.insert(file.to_owned(), splib_file(record, values));
    }
    LabLibrary::from_texts(texts)
}

/// The spectrum error of `file` for `why`.
fn spectrum_refusal(file: &'static str, why: &str) -> FitAlbedoError {
    FitAlbedoError::Spectrum {
        file,
        why: why.to_owned(),
    }
}

/// A spectrum's channels are refused, by their specific reason, under a file name not of the
/// release's form, when its header names another record, when its channels are not its
/// wavelength file's, and when its wavelengths do not rise; a well-formed one reads, deleted
/// channels dropped.
#[test]
fn substance_albedos_a_malformed_spectrum_is_refused() {
    let lib = library(&[
        (
            "splib07a_Good_BECKa_AREF.txt",
            "Good BECKa AREF",
            &[0.3, -1.23e34, 0.5],
        ),
        (
            "splib07a_Other_BECKa_AREF.txt",
            "Good BECKa AREF",
            &[0.3, 0.4, 0.5],
        ),
        (
            "splib07a_Short_BECKa_AREF.txt",
            "Short BECKa AREF",
            &[0.3, 0.4],
        ),
        ("Plain.txt", "Plain", &[0.3, 0.4, 0.5]),
    ]);
    let beckman = |file| LabSpectrum::new(file, Spectrometer::Beckman);
    let good = lib
        .channels(beckman("splib07a_Good_BECKa_AREF.txt"))
        .unwrap();
    assert_eq!(good.len(), 2, "the deleted channel is dropped");
    for (file, why) in [
        (
            "splib07a_Other_BECKa_AREF.txt",
            "its header names the record `Good_BECKa_AREF`",
        ),
        (
            "splib07a_Short_BECKa_AREF.txt",
            "it has 2 channels and its wavelength file 3",
        ),
        ("Plain.txt", "its name is not splib07a_<record>.txt"),
        (
            "splib07a_Missing_BECKa_AREF.txt",
            "it is not one of the dataset's files",
        ),
    ] {
        assert_eq!(
            lib.channels(beckman(file)),
            Err(spectrum_refusal(file, why))
        );
    }
    let mut texts = std::collections::BTreeMap::from([(
        ASD_WAVELENGTHS.to_owned(),
        splib_file("Wavelengths ASD", &[0.7, 0.6]),
    )]);
    let down = "splib07a_Down_ASDFRa_AREF.txt";
    texts.insert(down.to_owned(), splib_file("Down ASDFRa AREF", &[0.3, 0.4]));
    assert_eq!(
        LabLibrary::from_texts(texts).channels(LabSpectrum::new(down, Spectrometer::Asd)),
        Err(spectrum_refusal(
            down,
            "its channels are fewer than two or do not rise"
        ))
    );
}

/// Spectra that share no wavelength are refused: an end-member's two spectra over 400–500 and
/// 600–700 nm, and a mixture of a Beckman end-member and an ASD one over the same; a mixture of
/// no part is refused as one.
#[test]
fn substance_albedos_spectra_that_share_no_wavelength_are_refused() {
    static SPECTRA: [LabSpectrum; 2] = [
        LabSpectrum::new("splib07a_Blue_BECKa_AREF.txt", Spectrometer::Beckman),
        LabSpectrum::new("splib07a_Red_ASDFRa_AREF.txt", Spectrometer::Asd),
    ];
    let lib = library(&[
        (
            "splib07a_Blue_BECKa_AREF.txt",
            "Blue BECKa AREF",
            &[0.3, 0.3, 0.3],
        ),
        (
            "splib07a_Red_ASDFRa_AREF.txt",
            "Red ASDFRa AREF",
            &[0.3, 0.3],
        ),
    ]);
    let tables = SpectrometerTables::new(&LUNAR_REGOLITH);
    let source = hyperion_sim::substance::by_key("S8")
        .and_then(|id| id.substance().condensed_optics())
        .unwrap()
        .sources()[0];
    // The cache knows an end-member by its name, so each is named.
    let end_member = |name: &'static str, spectra: &'static [LabSpectrum]| -> &'static EndMember {
        Box::leak(Box::new(EndMember::new(
            name,
            spectra,
            GrainScaling::Held,
            source,
        )))
    };
    let both = end_member("both", &SPECTRA);
    assert_eq!(
        EndMemberGrain::new(both, &lib, &tables),
        Err(spectrum_refusal(
            "splib07a_Blue_BECKa_AREF.txt",
            "its end-member's spectra share no wavelength"
        ))
    );
    let parts = [
        MixturePart::new(end_member("blue", &SPECTRA[..1]), 0.5, GrainClass::Form),
        MixturePart::new(end_member("red", &SPECTRA[1..]), 0.5, GrainClass::Form),
    ];
    let mut cache = EndMemberCache::new();
    assert_eq!(
        MixtureGrain::new(&parts, &lib, &tables, &mut cache),
        Err(spectrum_refusal(
            "splib07a_Blue_BECKa_AREF.txt",
            "a mixture's spectra share no wavelength"
        ))
    );
    assert_eq!(
        MixtureGrain::new(&[], &lib, &tables, &mut cache),
        Err(FitAlbedoError::EmptyMixture)
    );
}
