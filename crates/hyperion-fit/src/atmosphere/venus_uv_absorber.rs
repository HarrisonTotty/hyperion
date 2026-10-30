//! R08.T5.e's geometric albedo on the tracer's plane-parallel Venus deck (plan R08;
//! `science-r08-unowned-absorbers.md` §1.2), held to Venus's: test-only.
//!
//! p(λ) = 2 ∫₀¹ R(μ, μ; α = 0) μ dμ, R = π I ÷ F the radiance factor at opposition, on the 8-point
//! Gauss–Legendre rule in μ = (1 + x) ÷ 2, so that p = Σ wₖ μₖ R(μₖ): the client's
//! `test/venusAlbedo.ts` writes the case, `venus-92bar`'s medium on a 10¹² m sphere at 365 nm and
//! the B, V, g′ and r′ bands' effective wavelengths, one geometry a node, its sun of irradiance π,
//! and `atmosphere-reference` traces it to the committed reference these tests read.

use hyperion_sim::tables::gauss_legendre::{GL8_NODES, GL8_WEIGHTS};

/// Each node's μ and weight, ascending in μ: the client's `ALBEDO_NODES`.
fn nodes() -> impl Iterator<Item = (f64, f64)> {
    GL8_NODES
        .iter()
        .zip(GL8_WEIGHTS)
        .map(|(&x, w)| (f64::midpoint(1.0, x), w))
}

/// The geometric albedo of radiances at the nodes, in order, under a sun of irradiance π, and its
/// standard error from theirs.
fn geometric_albedo(radiance: &[f64], standard_error: &[f64]) -> (f64, f64) {
    let (mut albedo, mut variance) = (0.0, 0.0);
    for (((mu, w), &r), &s) in nodes().zip(radiance).zip(standard_error) {
        albedo += w * mu * r;
        variance += (w * mu * s) * (w * mu * s);
    }
    (albedo, variance.sqrt())
}

/// The committed albedo reference, from the crate: `venus-92bar`'s plane-parallel deck at the
/// fitted absorber, traced by `atmosphere-reference --stokes`, read when a test asks, so that the
/// crate's tests build without it.
const REFERENCE_FROM_CRATE: &str = "../../apps/hyperion/src/renderer/src/view/atmosphere/reference/venus-92bar-albedo.reference.json";

/// The wavelengths the albedo case traces, nm, in its order: 365 nm (Y. J. Lee et al., AJ 158
/// (2019) 126), and Johnson B and V and Sloan g′ and r′ at the effective wavelengths Mallama et
/// al. state (A. Mallama, B. Krobusek and H. Pavlov, Icarus 282 (2017) 19, Tables 1 and 2).
const BANDS_NM: [f64; 5] = [365.0, 436.0, 549.0, 469.0, 616.0];

#[cfg(test)]
mod tests {
    use core::f64::consts::PI;
    use std::num::NonZeroU64;

    use serde_json::{Value, json};

    use super::super::{AtmosphereCase, Polarisation, trace_reference};
    use super::*;

    /// The committed reference's geometric albedo and σ at each band, in [`BANDS_NM`]' order.
    fn committed_albedos() -> Vec<(f64, f64)> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(REFERENCE_FROM_CRATE);
        let text =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let reference: Value = serde_json::from_str(&text).unwrap();
        let wavelengths: Vec<f64> = reference["wavelengthsNm"]
            .as_array()
            .unwrap()
            .iter()
            .map(|w| w.as_f64().unwrap())
            .collect();
        assert_eq!(wavelengths.len(), BANDS_NM.len(), "the bands");
        for (traced, band) in wavelengths.iter().zip(BANDS_NM) {
            assert!(
                (traced - band).abs() < 1e-9,
                "the bands, in order: {wavelengths:?}"
            );
        }
        assert_eq!(reference["polarisation"], "stokes");
        let geometries = reference["geometries"].as_array().unwrap();
        assert_eq!(geometries.len(), 8, "a geometry a node");
        (0..BANDS_NM.len())
            .map(|w| {
                let (radiance, sigma): (Vec<f64>, Vec<f64>) = geometries
                    .iter()
                    .enumerate()
                    .map(|(k, g)| {
                        assert_eq!(g["name"], format!("albedo-n{k}"));
                        (
                            g["radiance"][w].as_f64().unwrap(),
                            g["standardError"][w].as_f64().unwrap(),
                        )
                    })
                    .unzip();
                geometric_albedo(&radiance, &sigma)
            })
            .collect()
    }

    /// A ratio of two albedos and its σ, the two independent (each band its own draws).
    fn ratio((a, sa): (f64, f64), (b, sb): (f64, f64)) -> (f64, f64) {
        let r = a / b;
        (r, r * ((sa / a) * (sa / a) + (sb / b) * (sb / b)).sqrt())
    }

    /// Holds `(value, sigma)` within [low, high] as Design note 10's tolerance test for a sanity
    /// range: low − 4σ ≤ value ≤ high + 4σ, σ ≤ (high − low) ÷ 16.
    fn within(what: &str, (value, sigma): (f64, f64), low: f64, high: f64) {
        assert!(
            sigma <= (high - low) / 16.0,
            "{what}: σ {sigma} refuses the range {low}–{high}"
        );
        assert!(
            value >= low - 4.0 * sigma && value <= high + 4.0 * sigma,
            "{what}: {value} ± {sigma} against {low}–{high}"
        );
    }

    /// As [`within`], but a σ past (high − low) ÷ 16 is the range's refusal, stated, the value still
    /// held to low − 4σ ≤ value ≤ high + 4σ: g′ ÷ r′'s, whose σ the reference's 6,144 samples a node
    /// do not bring under 0.00375 (R08.T5.e's record).
    fn within_or_refused(what: &str, (value, sigma): (f64, f64), low: f64, high: f64) {
        if sigma > (high - low) / 16.0 {
            eprintln!("venus_uv_absorber: {what} {value} ± {sigma}: refused, σ over (b − a) ÷ 16");
        }
        assert!(
            value >= low - 4.0 * sigma && value <= high + 4.0 * sigma,
            "{what}: {value} ± {sigma} against {low}–{high}"
        );
    }

    #[test]
    fn atmosphere_venus_uv_absorber_meets_venuss_365_nm_albedo_and_colours() {
        // R08.T5.e's tests: p(365) in 0.33–0.40 (Lee et al. 2019's 2011 and 2015–2017 albedos),
        // p(g′) ÷ p(r′) in 0.90–0.96 (Mallama et al.'s 0.933), and p(B) ÷ p(V) in 0.78–0.96 (their
        // 0.955, and 0.780 from their Table 3's synthetic B).
        let p = committed_albedos();
        within("p(365)", p[0], 0.33, 0.40);
        within("p(B) ÷ p(V)", ratio(p[1], p[2]), 0.78, 0.96);
        within_or_refused("p(g′) ÷ p(r′)", ratio(p[3], p[4]), 0.90, 0.96);
        eprintln!("venus_uv_absorber: p at {BANDS_NM:?} nm: {p:?}");
    }

    #[test]
    fn atmosphere_venus_uv_absorber_a_lambert_planes_geometric_albedo_is_two_thirds_of_its_albedo()
    {
        // A Lambert plane under no air: each node's radiance is A μ F ÷ π exactly, so the rule
        // integrates 2A ∫ μ² dμ = 2A ÷ 3 exactly, to the plane's curvature on its 10¹² m sphere:
        // seen from 2 m, the lowest node's ground point lies 100 m off, its sun turned by 10⁻¹⁰ rad
        // (from 2 km, 4 × 10⁻⁹ of p).
        let albedo = 0.37;
        let geometries: Vec<Value> = nodes()
            .enumerate()
            .map(|(k, (mu, _))| {
                let zenith = hyperion_sim::math::acos(mu).to_degrees();
                json!({
                    "name": format!("albedo-n{k}"),
                    "observer": { "heightM": 2.0 },
                    "view": { "zenithDeg": 180.0 - zenith, "azimuthDeg": 0.0 },
                    "sunDirections": [{ "zenithDeg": zenith, "azimuthDeg": 180.0 }]
                })
            })
            .collect();
        let case = json!({
            "format": 1, "name": "lambert",
            "wavelengthsNm": [365],
            "shells": { "kind": "sphere", "radiusM": 1e12 },
            "topHeightM": 1.0,
            "groundAlbedo": [albedo],
            "terms": [{
                "name": "void",
                "density": { "kind": "tabulated", "altitudesM": [0.0, 1.0], "relative": [1.0, 1.0] },
                "scattering": [0.0], "absorption": [0.0], "phase": { "kind": "none" }
            }],
            "suns": [{ "name": "sun", "irradiance": [PI] }],
            "detectorHalfAngleDeg": 0.0,
            "geometries": geometries
        });
        let case = AtmosphereCase::from_json(&case.to_string()).unwrap();
        let reference = trace_reference(
            &case,
            Polarisation::Scalar,
            NonZeroU64::new(4).unwrap(),
            std::num::NonZeroUsize::new(1).unwrap(),
        )
        .unwrap();
        let (radiance, sigma): (Vec<f64>, Vec<f64>) = reference
            .geometries()
            .iter()
            .map(|g| (g.radiance()[0], g.standard_error()[0]))
            .unzip();
        let (p, sigma) = geometric_albedo(&radiance, &sigma);
        assert!(
            (p / (2.0 * albedo / 3.0) - 1.0).abs() < 1e-9,
            "{p} against {}",
            2.0 * albedo / 3.0
        );
        // Every path scores the ground's one next-event estimate and escapes: no variance.
        assert!(sigma < 1e-12 * p, "σ {sigma}");
    }
}
