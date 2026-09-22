//! The discs' vertical profiles for one galaxy: the old thin disc's five sub-discs by age, the
//! young disc, the thick disc and the nuclear disc (brainstorm, "Fields" and "Orbits and time";
//! plan 02, Design note 9 and P02.T7.b).
//!
//! One scale height for the whole old thin disc would contradict the observed heating of stars
//! with age, so the old thin disc is five discs by age, as in the Besançon model. A sub-disc's age
//! `τ` is the mean age of the formation history inside its bin, and its vertical dispersion
//! follows the heating law of Sharma et al. (2021, MNRAS 506, 1761, eqs. 4 and 7, Table 2), which
//! the brainstorm rounds to `22 km/s × (τ ÷ 10 Gyr)^0.44`:
//!
//! `σ_z(τ, z) = s × 21.1 km/s × ((τ ÷ Gyr + 0.1) ÷ 10.1)^0.441 × (1 + 0.20 |z| ÷ kpc)`,
//!
//! up to 2.4 kpc from the plane, the reach of the heights it was fitted to, and level above,
//! at the Sun's angular momentum and metallicity, which the reference radius, three thin-disc
//! scale lengths, stands for. Each sub-disc's vertical profile is the vertical Jeans equation's
//! solution for that dispersion in the galaxy's potential at the reference radius
//! ([`vertical`](super::vertical)): cored at the plane, with no free height. `s` is the galaxy's
//! dispersion scale, one number that the drawn mean height fixes: the sub-discs' effective heights
//! `hᵢ = Σᵢ ÷ 2ρ₀,ᵢ`, weighted by their shares `wᵢ` of the old thin disc, have the harmonic mean
//! `1 ÷ Σ wᵢ ÷ hᵢ` of the drawn 850–1,150 ly. That mean is the whole old thin disc's effective
//! height, `Σ ÷ 2ρ₀`, since the sub-discs' mid-plane densities add. The heights are met by
//! scaling the dispersions, not the heights, so heights, dispersions and profiles satisfy one
//! Jeans equation together (brainstorm, Decisions, "2026-09-21: local density rulings", 1).
//!
//! The other discs are cored the same way, their dispersions rising with height by the same
//! 0.20 per kpc, as Sharma et al. find the high-α stars' do too ("no special provision is needed to
//! accommodate the thick disc stars", in their summary and conclusions), each with a mid-plane
//! dispersion of its own that meets its drawn effective height:
//! the young disc (130–200 ly, drawn apart from the old disc's) and the thick disc at the same
//! reference radius, and the nuclear disc at two of its own scale lengths, the mass-weighted mean
//! radius of an exponential disc. An isothermal thick disc, as mono-abundance populations are
//! measured to be over 0.5–2 kpc ("nearly isothermal", Bovy et al. 2012, ApJ 755, 115) and as the
//! brainstorm's single dispersion for it might be read, falls off too fast far from the plane: at
//! a given effective height a cored profile e-folds faster far out than an exponential (sech²
//! in half its effective height), and in the model's potential `K_z` keeps rising above the thin
//! disc. 1.5 kpc up the Milky Way fixture's discs then e-fold in 420 pc, and a double exponential
//! fitted to them at the Sun's radius finds its thick disc at the fit's floor of 500 pc, against
//! Bland-Hawthorn and Gerhard's (2016, ARA&A 54, 529, §5.1.3) 900 ± 180 pc, an exponential fit's
//! far-field height; with the gradient it finds 985 pc (plan 02, P02.T7.b). For the
//! nuclear disc, a hundred light-years thick, the gradient changes its dispersion by under 1%
//! across it.
//!
//! Solving costs two tables of `K_z`, 48 evaluations of the mass model each, at the two reference
//! radii, which is most of the fields' build; then bisections of the dispersion, each a pass over
//! the profiles' knots. All of it is part of the generator version.

use super::vertical::{BISECTIONS, JeansIntegral, VerticalForce, VerticalProfile};
use crate::galaxy::ages::{AgeDistribution, SubDisc};
use crate::galaxy::consts::{LIGHT_YEARS_PER_KILOPARSEC, YEARS_PER_GIGAYEAR};
use crate::galaxy::params::GalaxyParams;
use crate::galaxy::potential::MassModel;
use crate::galaxy::quad::bisect;
use crate::math;
use crate::units::{KilometresPerSecond, LightYears, Years};

/// The heating law's vertical dispersion at 10 Gyr in the mid-plane, `σ_0,z` (Sharma et al. 2021,
/// Table 2: 21.1 ± 0.2 km/s; the brainstorm rounds it to 22).
pub const SIGMA_Z_AT_TEN_GYR: KilometresPerSecond = KilometresPerSecond::new(21.1);

/// The heating law's exponent `β_z`: `σ_z ∝ (τ + 0.1 Gyr)^0.441` (Sharma et al. 2021, Table 2:
/// 0.441 ± 0.007; the brainstorm rounds it to 0.44).
pub const HEATING_EXPONENT: f64 = 0.441;

/// The heating law's age offset, Gyr: a birth dispersion for stars younger than 0.1 Gyr (Sharma et
/// al. 2021, eq. 4).
pub const HEATING_AGE_OFFSET_GYR: f64 = 0.1;

/// How the vertical dispersion grows with height, `γ_z` per kpc: `σ_z ∝ 1 + γ_z |z|` (Sharma et al.
/// 2021, eq. 7 and Table 2: 0.20 ± 0.01 per kpc).
pub const DISPERSION_HEIGHT_GRADIENT_PER_KPC: f64 = 0.20;

/// The height, kpc, up to which the dispersion rises by [`DISPERSION_HEIGHT_GRADIENT_PER_KPC`] and
/// above which it stays level: the reach of the heights Sharma et al. (2021, Figs. 1 and 15)
/// fitted their law to. They say nothing of extrapolating it, and carried on to the root cube's
/// edge it would give every disc a tail falling as `z⁻²`: the Milky Way fixture's thick disc would
/// hold a tenth of the halo's density 10 kpc above the Sun.
pub const DISPERSION_GRADIENT_REACH_KPC: f64 = 2.4;

/// The heating law in the mid-plane, `σ_z(τ, 0) = 21.1 km/s × ((τ ÷ Gyr + 0.1) ÷ 10.1)^0.441`
/// (module documentation).
#[must_use]
pub fn heating_law(age: Years) -> KilometresPerSecond {
    let gyr = age.value() / YEARS_PER_GIGAYEAR;
    SIGMA_Z_AT_TEN_GYR
        * math::powf(
            (gyr + HEATING_AGE_OFFSET_GYR) / (10.0 + HEATING_AGE_OFFSET_GYR),
            HEATING_EXPONENT,
        )
}

/// Where the thin and thick discs' profiles are solved, in thin-disc scale lengths (plan 02,
/// Design note 9).
pub const REFERENCE_RADIUS_LENGTHS: f64 = 3.0;

/// Where the nuclear disc's profile is solved, in its own scale lengths: the mass-weighted mean
/// radius of an exponential disc.
pub const NUCLEAR_REFERENCE_RADIUS_LENGTHS: f64 = 2.0;

/// The bracket of the dispersion scale `s` in which the old thin disc's is sought.
const SCALE_BRACKET: (f64, f64) = (1.0 / 16.0, 16.0);

/// The old thin disc's sub-discs as the vertical Jeans equation shapes them (module
/// documentation): each one's age, share, dispersion and effective height, and the dispersion
/// scale `s` that gives the five together the drawn mean height.
///
/// # Examples
///
/// ```
/// use hyperion_sim::galaxy::fields::Fields;
/// use hyperion_sim::galaxy::params::GalaxyParams;
/// use hyperion_sim::galaxy::potential::MassModel;
///
/// let params = GalaxyParams::milky_way_like();
/// let fields = Fields::new(&params, &MassModel::new(&params));
/// let sub = fields.sub_disc_heights();
/// // Older sub-discs are hotter and thicker, and together they have the effective height, and so
/// // the mid-plane density, of one disc of the drawn mean height.
/// let h = sub.heights();
/// assert!(h.windows(2).all(|w| w[0] < w[1]));
/// assert!((sub.mean_height() / params.thin_disc().height() - 1.0).abs() < 1e-12);
/// // The dispersions are the heating law's times one scale.
/// let (law, scaled) = (sub.dispersions(), sub.scaled_dispersions());
/// assert!((scaled[4] / law[4] - sub.scale()).abs() < 1e-15);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SubDiscHeights {
    reference_radius: LightYears,
    mean_ages: [Years; 5],
    dispersions: [KilometresPerSecond; 5],
    shares: [f64; 5],
    unscaled: [LightYears; 5],
    scale: f64,
    heights: [LightYears; 5],
}

impl SubDiscHeights {
    /// `R_ref`, three thin-disc scale lengths, where the Jeans equation is solved.
    #[must_use]
    pub fn reference_radius(&self) -> LightYears {
        self.reference_radius
    }

    /// Each sub-disc's age: the mean age of the formation history inside its bin, youngest first.
    #[must_use]
    pub fn mean_ages(&self) -> [Years; 5] {
        self.mean_ages
    }

    /// Each sub-disc's mid-plane dispersion under Sharma et al.'s heating law at its mean age,
    /// before the galaxy's dispersion scale.
    #[must_use]
    pub fn dispersions(&self) -> [KilometresPerSecond; 5] {
        self.dispersions
    }

    /// Each sub-disc's mid-plane dispersion as its profile has it: the heating law's times the
    /// galaxy's dispersion scale.
    ///
    /// With the law's `1 + 0.20 |z| ÷ kpc` up to 2.4 kpc, the vertical Jeans equation on the
    /// sub-disc's own profile returns it (plan 08, Design note 3).
    #[must_use]
    pub fn scaled_dispersions(&self) -> [KilometresPerSecond; 5] {
        self.dispersions.map(|sigma| sigma * self.scale)
    }

    /// Each sub-disc's share of the old thin disc's systems; they sum to 1.
    #[must_use]
    pub fn shares(&self) -> [f64; 5] {
        self.shares
    }

    /// Each sub-disc's effective height under the heating law itself, a dispersion scale of 1.
    #[must_use]
    pub fn unscaled(&self) -> [LightYears; 5] {
        self.unscaled
    }

    /// The galaxy's dispersion scale `s`: the factor on the heating law that gives the sub-discs
    /// the drawn mean height.
    ///
    /// Far from 1, it says the model's disc mass and the measured heating law disagree with the
    /// drawn height (plan 02, Risks, R2).
    #[must_use]
    pub fn scale(&self) -> f64 {
        self.scale
    }

    /// The sub-discs' effective heights `Σ ÷ 2ρ₀`, youngest first.
    #[must_use]
    pub fn heights(&self) -> [LightYears; 5] {
        self.heights
    }

    /// The share-weighted harmonic mean of [`heights`](Self::heights), `1 ÷ Σ wᵢ ÷ hᵢ`: the old thin
    /// disc's effective height, which is the drawn mean height.
    #[must_use]
    pub fn mean_height(&self) -> LightYears {
        LightYears::new(harmonic_mean(
            &self.shares,
            &self.heights.map(LightYears::value),
        ))
    }
}

/// `1 ÷ Σ wᵢ ÷ hᵢ`, in order.
#[must_use]
fn harmonic_mean(weights: &[f64; 5], heights: &[f64; 5]) -> f64 {
    let inverse = weights
        .iter()
        .zip(heights)
        .fold(0.0, |sum, (w, h)| sum + w / h);
    1.0 / inverse
}

/// Every disc's vertical profile for one galaxy, and the sub-discs' summary.
#[derive(Debug)]
pub(crate) struct DiscProfiles {
    pub young: VerticalProfile,
    pub sub_discs: [VerticalProfile; 5],
    pub thick: VerticalProfile,
    pub nuclear: VerticalProfile,
    pub summary: SubDiscHeights,
}

impl DiscProfiles {
    /// Solves every disc's profile of the galaxy `params` in its mass model `model`.
    ///
    /// # Panics
    ///
    /// Never for built parameters, whose formation timescale and heights are positive.
    #[must_use]
    pub(crate) fn solve(params: &GalaxyParams, model: &MassModel) -> Self {
        const VALID: &str = "built parameters hold a positive timescale";
        let force_at = |r: f64| {
            VerticalForce::new(|z| model.vertical_force(LightYears::new(r), LightYears::new(z)))
        };
        let gamma = DISPERSION_HEIGHT_GRADIENT_PER_KPC;
        let reach = LightYears::new(DISPERSION_GRADIENT_REACH_KPC * LIGHT_YEARS_PER_KILOPARSEC);

        let reference = REFERENCE_RADIUS_LENGTHS * params.thin_disc().length().value();
        let disc_force = force_at(reference);
        let heated = JeansIntegral::new(&disc_force, gamma, reach, LightYears::new(reference));

        let tau = params.sfh_timescale();
        let weights = SubDisc::ALL.map(|bin| bin.share(tau));
        let total = weights.iter().fold(0.0, |sum, w| sum + w);
        let shares = weights.map(|w| w / total);
        let mean_ages = SubDisc::ALL.map(|bin| {
            AgeDistribution::old_thin_disc(tau, bin)
                .expect(VALID)
                .mean()
        });
        let dispersions = mean_ages.map(heating_law);
        let profiles = |scale: f64| dispersions.map(|sigma| heated.profile(sigma * scale));
        let mean_height = |profiles: &[VerticalProfile; 5]| {
            harmonic_mean(
                &shares,
                &profiles.each_ref().map(|p| p.effective_height().value()),
            )
        };
        let drawn = params.thin_disc().height().value();
        let ln_scale = bisect(
            |u| mean_height(&profiles(math::exp(u))) - drawn,
            math::ln(SCALE_BRACKET.0),
            math::ln(SCALE_BRACKET.1),
            BISECTIONS,
        );
        let scale = math::exp(ln_scale);
        let sub_discs = profiles(scale);
        let effective =
            |p: &[VerticalProfile; 5]| p.each_ref().map(VerticalProfile::effective_height);
        let summary = SubDiscHeights {
            reference_radius: LightYears::new(reference),
            mean_ages,
            dispersions,
            shares,
            unscaled: effective(&profiles(1.0)),
            scale,
            heights: effective(&sub_discs),
        };

        let young = heated.solve(params.young_disc().height());
        let thick = heated.solve(params.thick_disc().height());
        let nuclear_reference =
            NUCLEAR_REFERENCE_RADIUS_LENGTHS * params.nuclear_disc().length().value();
        let nuclear = JeansIntegral::new(
            &force_at(nuclear_reference),
            gamma,
            reach,
            LightYears::new(nuclear_reference),
        )
        .solve(params.nuclear_disc().height());
        Self {
            young,
            sub_discs,
            thick,
            nuclear,
            summary,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The heating law is Sharma et al.'s (2021, eq. 4 and Table 2): 21.1 km/s at 10 Gyr, a birth
    /// dispersion at zero age, the exponent 0.441 on the age plus 0.1 Gyr.
    #[test]
    fn the_heating_law_is_sharma_s() {
        let gyr = |t: f64| Years::new(t * YEARS_PER_GIGAYEAR);
        assert!((heating_law(gyr(10.0)).value() - 21.1).abs() < 1e-12);
        let birth = 21.1 * math::powf(0.1 / 10.1, 0.441);
        assert!((heating_law(Years::ZERO).value() / birth - 1.0).abs() < 1e-14);
        let ratio = heating_law(gyr(4.0)).value() / heating_law(gyr(1.0)).value();
        assert!((ratio / math::powf(4.1 / 1.1, 0.441) - 1.0).abs() < 1e-14);
    }

    /// Every profile of the fixture has the height it was solved for, and the sub-discs' profiles
    /// are the heating law's times the one scale; every disc takes the law's gradient.
    #[test]
    fn every_disc_meets_its_drawn_height() {
        let params = GalaxyParams::milky_way_like();
        let profiles = DiscProfiles::solve(&params, &MassModel::new(&params));
        let relative = |a: LightYears, b: LightYears| (a.value() / b.value() - 1.0).abs();
        assert!(
            relative(
                profiles.young.effective_height(),
                params.young_disc().height()
            ) < 1e-12
        );
        assert!(
            relative(
                profiles.thick.effective_height(),
                params.thick_disc().height()
            ) < 1e-12
        );
        assert!(
            relative(
                profiles.nuclear.effective_height(),
                params.nuclear_disc().height()
            ) < 1e-12
        );
        let summary = profiles.summary;
        assert!(relative(summary.mean_height(), params.thin_disc().height()) < 1e-12);
        let gamma = DISPERSION_HEIGHT_GRADIENT_PER_KPC;
        for (profile, sigma) in profiles.sub_discs.iter().zip(summary.scaled_dispersions()) {
            assert!((profile.dispersion().value() / sigma.value() - 1.0).abs() < 1e-15);
            assert!((profile.gradient_per_kpc() / gamma - 1.0).abs() < 1e-15);
        }
        for profile in [&profiles.young, &profiles.thick, &profiles.nuclear] {
            assert!((profile.gradient_per_kpc() / gamma - 1.0).abs() < 1e-15);
            let reach = profile.gradient_reach().value();
            assert!((reach / (2.4 * LIGHT_YEARS_PER_KILOPARSEC) - 1.0).abs() < 1e-15);
        }
    }
}
