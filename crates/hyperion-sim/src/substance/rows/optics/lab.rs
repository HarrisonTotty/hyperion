//! The laboratory end-members of the condensed optics and the mixtures made of them (P14.T49.i;
//! decision-p14-t49i-rock-mixtures): each splib07a spectrum the rows read, its sample as its
//! description gives it, its real index and grain size, and how its w is taken to a row's grains;
//! each mixture's parts at its cited pristine mode, with the mode's and the fine grains' sources.
//!
//! Every spectrum is of the USGS Spectral Library Version 7, splib07a (Kokaly et al. 2017, USGS
//! Data Series 1035, DOI 10.3133/ds1035; data release DOI 10.5066/F7RR1WDJ; U.S. public domain),
//! which `hyperion-fit`'s dataset `usgs_splib07a` commits unmodified. Each is inverted for w at its
//! own geometry by the reference form's model and taken to the form's grains as its end-member's
//! scaling says, or mixed: those are the uses, which no file's values change. A sieve fraction's
//! grain size is its description's measured size where it gives one, else its sieve's midpoint (the
//! .3B fraction's 74–250 µm taken as 162 µm), or half its upper bound for a fraction open below (a
//! "< 60 µm" fraction as 30 µm), the lane's convention, stated at each end-member.
//!
//! The real indices are the Handbook of Mineralogy's (J. W. Anthony, R. A. Bideaux, K. W. Bladh and
//! M. C. Nichols, Mineralogical Society of America), the mean of a mineral's principal indices,
//! each range taken at its midpoint, and for a solid solution linear in composition between its end
//! members' means (olivine between forsterite's 1.652 and fayalite's 1.858 in Fa; low-Ca pyroxene
//! between enstatite's Mg end, 1.653, and ferrosilite's 1.769 in Fs; plagioclase between albite's
//! 1.533 and anorthite's 1.582 in An). An opaque end-member's n is not used, since its w is held.

use super::super::super::optics::{
    EndMember, GrainClass, GrainScaling, LabSpectrum, MixturePart, OpaqueEndMember, Spectrometer,
};
use super::super::super::{Basis, Source};
use crate::units::Metres;

/// A Beckman spectrum of `file`.
#[must_use]
const fn beckman(file: &'static str) -> LabSpectrum {
    LabSpectrum::new(file, Spectrometer::Beckman)
}

/// An ASD spectrum of `file`.
#[must_use]
const fn asd(file: &'static str) -> LabSpectrum {
    LabSpectrum::new(file, Spectrometer::Asd)
}

/// The slab's scaling at real index `n` from laboratory grains of `um` µm.
#[must_use]
const fn slab(n: f64, um: f64) -> GrainScaling {
    GrainScaling::Slab {
        real_index: n,
        lab_grain_diameter: Metres::new(um * 1.0e-6),
    }
}

/// An end-member's or a mode's source: what `citation` describes, its basis `Reduced`.
#[must_use]
const fn described(covers: &'static str, citation: &'static str) -> Source {
    Source {
        covers,
        citation,
        basis: Basis::Reduced,
    }
}

/// A mode no source gives whole, its basis `Adopted`: `citation` names its sources and says which
/// part of it is an assumption.
#[must_use]
const fn assumed(citation: &'static str) -> Source {
    Source {
        covers: "mode",
        citation,
        basis: Basis::Adopted,
    }
}

/// A part at the form's grains.
#[must_use]
const fn form(end_member: &'static EndMember, fraction: f64) -> MixturePart {
    MixturePart::new(end_member, fraction, GrainClass::Form)
}

/// A part at the fine grains.
#[must_use]
const fn fine(end_member: &'static EndMember, fraction: f64) -> MixturePart {
    MixturePart::new(end_member, fraction, GrainClass::Fine)
}

/// An opaque index file's part at the fine grains (P14.T49.k).
#[must_use]
const fn fine_opaque(end_member: &'static OpaqueEndMember, fraction: f64) -> MixturePart {
    MixturePart::opaque(end_member, fraction, GrainClass::Fine)
}

// ---------------------------------------------------------------------------------------------
// Silicates, carbonates and salts, taken to the form's grains through the slab where sized.

/// Anorthite HS201.3B.
pub(in crate::substance) const ANORTHITE: EndMember = EndMember::new(
    "anorthite HS201.3B",
    &[beckman("splib07a_Anorthite_HS201.3B_Plagio_BECKa_AREF.txt")],
    slab(1.582, 162.0),
    described(
        "end-member anorthite HS201.3B",
        "splib07a's Anorthite_HS201.3B_Plagio_BECKa_AREF (Kokaly et al. 2017, U.S. public \
         domain): anorthite from Japan, purified with a Franz separator and hand-picked (its \
         description), the .3B sieve fraction, 74–250 µm, taken as 162 µm; n 1.582, the mean of \
         the Handbook of Mineralogy's anorthite α 1.573–1.577, β 1.580–1.585 and γ 1.585–1.590; \
         its Beckman spectrum inverted at its own geometry and taken to the form's grains through \
         the slab",
    ),
);

/// Hypersthene PYX02.f, Fs13.8.
pub(in crate::substance) const HYPERSTHENE_PYX02: EndMember = EndMember::new(
    "hypersthene PYX02.f",
    &[beckman("splib07a_Hypersthene_PYX02.f_60um_BECKc_AREF.txt")],
    slab(1.669, 60.0),
    described(
        "end-member hypersthene PYX02.f",
        "splib07a's Hypersthene_PYX02.f_60um_BECKc_AREF (Kokaly et al. 2017, U.S. public \
         domain): an orthopyroxene of FeO 9.38 and MgO 32.80 wt%, Fs13.8 (Mg# 86), with under 2% \
         tremolite (its description; purity 1c), its 60 µm fraction (named; the .f sieve 45–104 \
         µm); n 1.669, low-Ca pyroxene's Handbook of Mineralogy mean linear in Fs (enstatite's Mg \
         end 1.653, ferrosilite's 1.769) at Fs13.8; its Beckman spectrum inverted at its own \
         geometry and taken to the form's grains through the slab",
    ),
);

/// Hypersthene NMNHC2368, Fs26.
pub(in crate::substance) const HYPERSTHENE_C2368: EndMember = EndMember::new(
    "hypersthene NMNHC2368",
    &[beckman("splib07a_Hypersthene_NMNHC2368_BECKb_AREF.txt")],
    GrainScaling::Held,
    described(
        "end-member hypersthene NMNHC2368",
        "splib07a's Hypersthene_NMNHC2368_BECKb_AREF (Kokaly et al. 2017, U.S. public domain): \
         an orthopyroxene from western Greenland, FeO 17.07, MgO 26.09 and CaO 1.27 wt% (Fs26 \
         En71 Wo2.5, Mg# 73), mildly altered, with a little talc (its description; purity 1b); its \
         grain size is not stated, so its w is held as measured; its Beckman spectrum inverted at \
         its own geometry",
    ),
);

/// Enstatite NMNH128288, Fs7.
pub(in crate::substance) const ENSTATITE: EndMember = EndMember::new(
    "enstatite NMNH128288",
    &[beckman("splib07a_Enstatite_NMNH128288_BECKc_AREF.txt")],
    slab(1.661, 25.0),
    described(
        "end-member enstatite NMNH128288",
        "splib07a's Enstatite_NMNH128288_BECKc_AREF (Kokaly et al. 2017, U.S. public domain): \
         enstatite of Fs7 Wo1.6 by its analysis (its text says about 13% ferrosilite), about 85 \
         vol% enstatite and 15 vol% mica, moderately altered (its description; purity 1c), the \
         most Mg-rich low-Ca pyroxene in splib07a; average grain 25 µm (its description, another \
         estimate 47 µm); n 1.661, low-Ca pyroxene's Handbook of Mineralogy mean linear in Fs at \
         Fs7; its Beckman spectrum inverted at its own geometry and taken to the form's grains \
         through the slab",
    ),
);

/// Augite NMNH120049.
pub(in crate::substance) const AUGITE: EndMember = EndMember::new(
    "augite NMNH120049",
    &[beckman("splib07a_Augite_NMNH120049_BECKb_AREF.txt")],
    GrainScaling::Held,
    described(
        "end-member augite NMNH120049",
        "splib07a's Augite_NMNH120049_BECKb_AREF (Kokaly et al. 2017, U.S. public domain): augite \
         of Wo34 En45 Fs21, its dolomite removed with HCl (its description; purity 1b); its grain \
         size is not stated, so its w is held as measured; its Beckman spectrum inverted at its \
         own geometry",
    ),
);

/// Diopside HS15.3B.
pub(in crate::substance) const DIOPSIDE: EndMember = EndMember::new(
    "diopside HS15.3B",
    &[beckman("splib07a_Diopside_HS15.3B_Pyroxene_BECKc_AREF.txt")],
    slab(1.677, 295.0),
    described(
        "end-member diopside HS15.3B",
        "splib07a's Diopside_HS15.3B_Pyroxene_BECKc_AREF (Kokaly et al. 2017, U.S. public \
         domain): diopside with 8–10 vol% fluid inclusions and a trace of phlogopite (its \
         description; purity 1c), its .3B fraction of measured average grain 295 µm; n 1.677, \
         the mean of the Handbook of Mineralogy's diopside α 1.664, β 1.672 and γ 1.694; its \
         Beckman spectrum inverted at its own geometry and taken to the form's grains through \
         the slab",
    ),
);

/// Pigeonite HS199.3B.
pub(in crate::substance) const PIGEONITE: EndMember = EndMember::new(
    "pigeonite HS199.3B",
    &[beckman("splib07a_Pigeonite_HS199.3B_BECKc_AREF.txt")],
    slab(1.715, 162.0),
    described(
        "end-member pigeonite HS199.3B",
        "splib07a's Pigeonite_HS199.3B_BECKc_AREF (Kokaly et al. 2017, U.S. public domain): \
         named pigeonite, whose XRD finds high albite, clinopyroxene, minor quartz and possibly \
         ilmenite, and whose listed composition reads Wo34, an augite's (its description; purity \
         1c), the .3B sieve fraction, 74–250 µm, taken as 162 µm; n 1.715, the mean of the \
         Handbook of Mineralogy's pigeonite midpoints 1.707, 1.708 and 1.731; its Beckman \
         spectrum inverted at its own geometry and taken to the form's grains through the slab",
    ),
);

/// Olivine KI3054, Fo65.5.
pub(in crate::substance) const OLIVINE_FO66: EndMember = EndMember::new(
    "olivine KI3054 Fo66",
    &[beckman(
        "splib07a_Olivine_KI3054_Fo66_lt60um_BECKb_AREF.txt",
    )],
    slab(1.723, 30.0),
    described(
        "end-member olivine KI3054 Fo66",
        "splib07a's Olivine_KI3054_Fo66_lt60um_BECKb_AREF (Kokaly et al. 2017, U.S. public \
         domain): Kiglapait olivine of Fo65.5, with weak bands of hydrous alteration (its \
         description), the < 60 µm fraction, taken as 30 µm; n 1.723, olivine's Handbook of \
         Mineralogy mean linear in Fa (forsterite α 1.635, β 1.651, γ 1.670; fayalite 1.827, \
         1.869, 1.879) at Fa34.5; its Beckman spectrum inverted at its own geometry and taken to \
         the form's grains through the slab",
    ),
);

/// Olivine GDS70.c, Fo90.5.
pub(in crate::substance) const OLIVINE_FO89: EndMember = EndMember::new(
    "olivine GDS70.c Fo89",
    &[beckman("splib07a_Olivine_GDS70.c_Fo89_70um_BECKb_AREF.txt")],
    slab(1.672, 70.0),
    described(
        "end-member olivine GDS70.c Fo89",
        "splib07a's Olivine_GDS70.c_Fo89_70um_BECKb_AREF (Kokaly et al. 2017, U.S. public \
         domain): olivine named Fo89, of Fo90.5 by its analysis (its description), the .c \
         fraction, 60–104 µm, median 70 µm; n 1.672, olivine's Handbook of Mineralogy mean linear \
         in Fa at Fa9.5; its Beckman spectrum inverted at its own geometry and taken to the \
         form's grains through the slab",
    ),
);

/// Olivine GDS71.a, Fo91.9.
pub(in crate::substance) const OLIVINE_FO91: EndMember = EndMember::new(
    "olivine GDS71.a Fo91",
    &[beckman("splib07a_Olivine_GDS71.a_Fo91_65um_BECKb_AREF.txt")],
    slab(1.669, 65.0),
    described(
        "end-member olivine GDS71.a Fo91",
        "splib07a's Olivine_GDS71.a_Fo91_65um_BECKb_AREF (Kokaly et al. 2017, U.S. public \
         domain): forsterite of the Twin Sisters dunite, FeO 7.93 and MgO 50.70 wt% (Fo91.9), a \
         pure separate with weak 2.3 µm alteration bands (its description), the .a fraction, \
         60–104 µm, named 65 µm; n 1.669, olivine's Handbook of Mineralogy mean linear in Fa at \
         Fa8.1; the most Mg-rich sized olivine in splib07a; its Beckman spectrum inverted at its \
         own geometry and taken to the form's grains through the slab",
    ),
);

/// Olivine HS285.2B, Fo80.
pub(in crate::substance) const OLIVINE_FO80: EndMember = EndMember::new(
    "olivine HS285.2B Fo80",
    &[asd("splib07a_Olivine_HS285.2B_Fo80_ASDFRb_AREF.txt")],
    slab(1.693, 37.0),
    described(
        "end-member olivine HS285.2B Fo80",
        "splib07a's Olivine_HS285.2B_Fo80_ASDFRb_AREF (Kokaly et al. 2017, U.S. public domain): \
         olivine of Fo80 Fa20, with trace spinel and iron staining (its description), the .2B \
         fraction, < 74 µm, taken as 37 µm; n 1.693, olivine's Handbook of Mineralogy mean linear \
         in Fa at Fa20; its ASD spectrum inverted at its own geometry and taken to the form's \
         grains through the slab",
    ),
);

/// Olivine KI3189, Fo60.3.
pub(in crate::substance) const OLIVINE_FO60: EndMember = EndMember::new(
    "olivine KI3189 Fo60",
    &[beckman(
        "splib07a_Olivine_KI3189_Fo60_lt60um_BECKb_AREF.txt",
    )],
    slab(1.734, 30.0),
    described(
        "end-member olivine KI3189 Fo60",
        "splib07a's Olivine_KI3189_Fo60_lt60um_BECKb_AREF (Kokaly et al. 2017, U.S. public \
         domain): Kiglapait olivine of Fo60.3 (its description), the < 60 µm fraction, taken as \
         30 µm; n 1.734, olivine's Handbook of Mineralogy mean linear in Fa at Fa39.7; its \
         Beckman spectrum inverted at its own geometry and taken to the form's grains through the \
         slab",
    ),
);

/// Olivine KI3188, Fo50.6.
pub(in crate::substance) const OLIVINE_FO51: EndMember = EndMember::new(
    "olivine KI3188 Fo51",
    &[beckman(
        "splib07a_Olivine_KI3188_Fo51_lt60um_BECKb_AREF.txt",
    )],
    slab(1.754, 30.0),
    described(
        "end-member olivine KI3188 Fo51",
        "splib07a's Olivine_KI3188_Fo51_lt60um_BECKb_AREF (Kokaly et al. 2017, U.S. public \
         domain): Kiglapait olivine of Fo50.6 (its description), the < 60 µm fraction, taken as \
         30 µm, its channel at 445.8 nm deleted and interpolated across; n 1.754, olivine's \
         Handbook of Mineralogy mean linear in Fa at Fa49.4; its Beckman spectrum inverted at its \
         own geometry and taken to the form's grains through the slab",
    ),
);

/// Olivine KI3291, Fo29.5.
pub(in crate::substance) const OLIVINE_FO29: EndMember = EndMember::new(
    "olivine KI3291 Fo29",
    &[beckman(
        "splib07a_Olivine_KI3291_Fo29_lt60um_BECKb_AREF.txt",
    )],
    slab(1.797, 30.0),
    described(
        "end-member olivine KI3291 Fo29",
        "splib07a's Olivine_KI3291_Fo29_lt60um_BECKb_AREF (Kokaly et al. 2017, U.S. public \
         domain): Kiglapait olivine of Fo29.5 (its description), the < 60 µm fraction, taken as \
         30 µm; n 1.797, olivine's Handbook of Mineralogy mean linear in Fa at Fa70.5; its \
         Beckman spectrum inverted at its own geometry and taken to the form's grains through the \
         slab",
    ),
);

/// Quartz GDS31.
pub(in crate::substance) const QUARTZ: EndMember = EndMember::new(
    "quartz GDS31",
    &[beckman("splib07a_Quartz_GDS31_0-74um_fr_BECKa_AREF.txt")],
    slab(1.547, 37.0),
    described(
        "end-member quartz GDS31",
        "splib07a's Quartz_GDS31_0-74um_fr_BECKa_AREF (Kokaly et al. 2017, U.S. public domain): \
         pure quartz (its description; purity 1a), the 0–74 µm fraction, taken as 37 µm; n 1.547, \
         the mean of the Handbook of Mineralogy's ω 1.544 (twice) and ε 1.553; its Beckman \
         spectrum inverted at its own geometry and taken to the form's grains through the slab",
    ),
);

/// Microcline HS151.3B.
pub(in crate::substance) const MICROCLINE: EndMember = EndMember::new(
    "microcline HS151.3B",
    &[beckman(
        "splib07a_Microcline_HS151.3B_Feldspar_BECKc_AREF.txt",
    )],
    slab(1.526, 162.0),
    described(
        "end-member microcline HS151.3B",
        "splib07a's Microcline_HS151.3B_Feldspar_BECKc_AREF (Kokaly et al. 2017, U.S. public \
         domain): microcline with much albite and a little mica by its XRD, iron-stained (its \
         description; purity 1c), the .3B sieve fraction, 74–250 µm, taken as 162 µm; n 1.526, \
         the mean of the Handbook of Mineralogy's microcline midpoints 1.5215, 1.5255 and 1.530; \
         its Beckman spectrum inverted at its own geometry and taken to the form's grains through \
         the slab",
    ),
);

/// Oligoclase HS110.3B.
pub(in crate::substance) const OLIGOCLASE: EndMember = EndMember::new(
    "oligoclase HS110.3B",
    &[beckman("splib07a_Oligoclase_HS110.3B_BECKc_AREF.txt")],
    slab(1.542, 162.0),
    described(
        "end-member oligoclase HS110.3B",
        "splib07a's Oligoclase_HS110.3B_BECKc_AREF (Kokaly et al. 2017, U.S. public domain): \
         oligoclase of An < 30 by its cell parameters, with minor quartz (its description), \
         sieved to 74–250 µm, taken as 162 µm; n 1.542, plagioclase's Handbook of Mineralogy \
         mean linear in An between albite (midpoints 1.528, 1.532, 1.5375) and anorthite at An20; \
         its Beckman spectrum inverted at its own geometry and taken to the form's grains through \
         the slab",
    ),
);

/// Biotite HS28.3B.
pub(in crate::substance) const BIOTITE: EndMember = EndMember::new(
    "biotite HS28.3B",
    &[beckman("splib07a_Biotite_HS28.3B_BECKa_AREF.txt")],
    slab(1.625, 162.0),
    described(
        "end-member biotite HS28.3B",
        "splib07a's Biotite_HS28.3B_BECKa_AREF (Kokaly et al. 2017, U.S. public domain): a pure \
         Ti-biotite of Mg# 59 (its description; purity 1a), sieved to 74–250 µm, taken as 162 µm, \
         its channels over 372.6–378.6 and 397.1–451.3 nm deleted and interpolated across, B's \
         band among them; n 1.625, the mean of webmineral's biotite α 1.595 and β = γ 1.64; its \
         Beckman spectrum inverted at its own geometry and taken to the form's grains through \
         the slab",
    ),
);

/// Lizardite NMNHR4687.c.
pub(in crate::substance) const LIZARDITE: EndMember = EndMember::new(
    "lizardite NMNHR4687.c",
    &[beckman(
        "splib07a_Lizardite_NMNHR4687.c_70um_BECKa_AREF.txt",
    )],
    slab(1.551, 70.0),
    described(
        "end-member lizardite NMNHR4687.c",
        "splib07a's Lizardite_NMNHR4687.c_70um_BECKa_AREF (Kokaly et al. 2017, U.S. public \
         domain): pure lizardite, a serpentine (its description; purity 1a), the .c fraction, \
         60–104 µm, named 70 µm, its channels over 843–859 nm deleted and interpolated across; n \
         1.551, the mean of the Handbook of Mineralogy's lizardite midpoints 1.546, 1.553 and \
         1.553; its Beckman spectrum inverted at its own geometry and taken to the form's grains \
         through the slab",
    ),
);

/// Saponite SapCa-1.
pub(in crate::substance) const SAPONITE: EndMember = EndMember::new(
    "saponite SapCa-1",
    &[beckman("splib07a_Saponite_SapCa-1_BECKb_AREF.txt")],
    GrainScaling::Held,
    described(
        "end-member saponite SapCa-1",
        "splib07a's Saponite_SapCa-1_BECKb_AREF (Kokaly et al. 2017, U.S. public domain): the \
         clay SapCa-1, its < 2 µm fraction saponite and its coarser fraction carrying quartz (its \
         description); its grain size is not stated, so its w is \
         held as measured; its Beckman spectrum inverted at its own geometry",
    ),
);

/// Cronstedtite M3542.
pub(in crate::substance) const CRONSTEDTITE: EndMember = EndMember::new(
    "cronstedtite M3542",
    &[beckman("splib07a_Cronstedtite_M3542_BECKb_AREF.txt")],
    GrainScaling::Held,
    described(
        "end-member cronstedtite M3542",
        "splib07a's Cronstedtite_M3542_BECKb_AREF (Kokaly et al. 2017, U.S. public domain): \
         cronstedtite, an Fe-serpentine, with up to 5% quartz (its description); its grain size is \
         not stated, so its w is held as measured; its Beckman spectrum inverted at its own \
         geometry",
    ),
);

/// Calcite GDS304.
pub(in crate::substance) const CALCITE: EndMember = EndMember::new(
    "calcite GDS304",
    &[asd("splib07a_Calcite_GDS304_75-150um_ASDFRb_AREF.txt")],
    slab(1.601, 112.5),
    described(
        "end-member calcite GDS304",
        "splib07a's Calcite_GDS304_75-150um_ASDFRb_AREF (Kokaly et al. 2017, U.S. public domain): \
         ground Iceland spar (its description; purity 1b), sieved to 75–150 µm, taken as 112.5 µm, \
         in place of the Beckman spectrum of Calcite WS272, whose w passes 0.9995 at 363 of its \
         channels (the lane's prototype); n 1.601, the mean of the Handbook of Mineralogy's ω \
         1.658 (twice) and ε 1.486; its ASD spectrum inverted at its own geometry and taken to the \
         form's grains through the slab",
    ),
);

/// Halite HS433.3B.
pub(in crate::substance) const HALITE: EndMember = EndMember::new(
    "halite HS433.3B",
    &[beckman("splib07a_Halite_HS433.3B_BECKa_AREF.txt")],
    slab(1.5443, 230.0),
    described(
        "end-member halite HS433.3B",
        "splib07a's Halite_HS433.3B_BECKa_AREF (Kokaly et al. 2017, U.S. public domain): halite \
         with 8 vol% inclusions (its description; purity 1a), its .3B fraction of measured \
         average grain 230 µm; n 1.5443, the Handbook of Mineralogy's; its Beckman spectrum \
         inverted at its own geometry and taken to the form's grains through the slab",
    ),
);

/// Kieserite KIEDE1.b.
pub(in crate::substance) const KIESERITE: EndMember = EndMember::new(
    "kieserite KIEDE1.b",
    &[asd("splib07a_Kieserite_KIEDE1.b_fine_gr_ASDFRc_AREF.txt")],
    GrainScaling::Held,
    described(
        "end-member kieserite KIEDE1.b",
        "splib07a's Kieserite_KIEDE1.b_fine_gr_ASDFRc_AREF (Kokaly et al. 2017, U.S. public \
         domain): kieserite, MgSO4·H2O, with minor langbeinite and a trace of halite (its \
         description; purity 1c), described as fine grained with no size, so its w is held as \
         measured; its ASD spectrum inverted at its own geometry",
    ),
);

/// Trona GDS148.
pub(in crate::substance) const TRONA: EndMember = EndMember::new(
    "trona GDS148",
    &[beckman("splib07a_Trona_GDS148_BECKa_AREF.txt")],
    GrainScaling::Held,
    described(
        "end-member trona GDS148",
        "splib07a's Trona_GDS148_BECKa_AREF (Kokaly et al. 2017, U.S. public domain): trona, \
         Na3(CO3)(HCO3)·2H2O, whose description was not reached, so its grain size is not known \
         and its w is held as measured; its Beckman spectrum inverted at its own geometry",
    ),
);

/// Corundum HS283.3B.
pub(in crate::substance) const CORUNDUM: EndMember = EndMember::new(
    "corundum HS283.3B",
    &[beckman("splib07a_Corundum_HS283.3B_BECKc_AREF.txt")],
    slab(1.767, 162.0),
    described(
        "end-member corundum HS283.3B",
        "splib07a's Corundum_HS283.3B_BECKc_AREF (Kokaly et al. 2017, U.S. public domain): natural \
         corundum of 0.5% Fe and 0.01% Cr with minor diaspore (its description; purity 1c), the \
         .3B sieve fraction, 74–250 µm, taken as 162 µm; n 1.767, the mean of the Handbook of \
         Mineralogy's ω 1.767–1.772 (twice) and ε 1.759–1.763; its Beckman spectrum inverted at \
         its own geometry and taken to the form's grains through the slab",
    ),
);

/// Rutile HS126.3B.
pub(in crate::substance) const RUTILE: EndMember = EndMember::new(
    "rutile HS126.3B",
    &[beckman("splib07a_Rutile_HS126.3B_BECKc_AREF.txt")],
    slab(2.706, 162.0),
    described(
        "end-member rutile HS126.3B",
        "splib07a's Rutile_HS126.3B_BECKc_AREF (Kokaly et al. 2017, U.S. public domain): natural \
         reddish-brown rutile from Oaxaca of 0.3% Fe, nearly pure (its description; purity 1c), \
         the .3B fraction by the series' convention, 74–250 µm, taken as 162 µm; chosen over \
         Titanium_dioxide_GDS798, one of splib07a's artist's pigments, whose polymorph, coating \
         and particle size are not recorded; n 2.706, the mean of the Handbook of Mineralogy's ω \
         2.605–2.613 (twice) and ε 2.899–2.901; its Beckman spectrum inverted at its own geometry \
         and taken to the form's grains through the slab",
    ),
);

/// Sulfur GDS94.
pub(in crate::substance) const SULFUR: EndMember = EndMember::new(
    "sulfur GDS94",
    &[beckman("splib07a_Sulfur_GDS94_Reagent_BECKa_AREF.txt")],
    GrainScaling::Held,
    described(
        "end-member sulfur GDS94",
        "splib07a's Sulfur_GDS94_Reagent_BECKa_AREF (Kokaly et al. 2017, U.S. public domain): \
         reagent sulphur powder at room temperature, its grain size not stated, so its w is held \
         as measured; its Beckman spectrum inverted at its own geometry. Its absorption edge is a \
         room-temperature one: cooled to Io's about 120 K it retreats about 28 nm to the blue \
         (Gradie and Veverka 1984, Icarus 58, 227, abstract: about 1.6 Å K⁻¹ in reflected light, \
         over 295–120 K), which is not drawn",
    ),
);

// ---------------------------------------------------------------------------------------------
// Opaque and dark phases, held as measured at every grain size.

/// Ilmenite HS231.3B.
pub(in crate::substance) const ILMENITE: EndMember = EndMember::new(
    "ilmenite HS231.3B",
    &[beckman("splib07a_Ilmenite_HS231.3B_BECKb_AREF.txt")],
    GrainScaling::Held,
    described(
        "end-member ilmenite HS231.3B",
        "splib07a's Ilmenite_HS231.3B_BECKb_AREF (Kokaly et al. 2017, U.S. public domain): \
         ilmenite with minor hematite and orthopyroxene (its description), the .3B sieve \
         fraction, 74–250 µm; opaque, so its w is held as measured; its Beckman spectrum \
         inverted at its own geometry",
    ),
);

/// Magnetite HS195.3B, the coarse fraction.
pub(in crate::substance) const MAGNETITE: EndMember = EndMember::new(
    "magnetite HS195.3B",
    &[beckman("splib07a_Magnetite_HS195.3B_BECKb_AREF.txt")],
    GrainScaling::Held,
    described(
        "end-member magnetite HS195.3B",
        "splib07a's Magnetite_HS195.3B_BECKb_AREF (Kokaly et al. 2017, U.S. public domain): \
         magnetite slightly contaminated with spectrally neutral quartz (its description), the \
         .3B sieve fraction, 74–250 µm; opaque, so its w is held as measured (its description \
         quotes Hunt, Salisbury and Lenhoff 1971: \"typically opaque behavior, decreasing in \
         reflectivity with decreasing particle size\"); its Beckman spectrum inverted at its own \
         geometry",
    ),
);

/// Magnetite's finest fractions, HS195.1B and HS78.1B, their w averaged.
pub(in crate::substance) const MAGNETITE_FINE: EndMember = EndMember::new(
    "magnetite HS195.1B and HS78.1B",
    &[
        asd("splib07a_Magnetite_HS195.1B_ASDFRb_AREF.txt"),
        asd("splib07a_Magnetite_HS78.1B_ASDFRb_AREF.txt"),
    ],
    GrainScaling::Held,
    described(
        "end-member magnetite HS195.1B and HS78.1B",
        "the mean of the w of splib07a's Magnetite_HS195.1B_ASDFRb_AREF and \
         Magnetite_HS78.1B_ASDFRb_AREF (Kokaly et al. 2017, U.S. public domain), magnetite's \
         finest measured fractions, each the .1B sieve fraction, < 5 µm (HS78 titaniferous, with \
         plagioclase and traces of ilmenite, Al-spinel and mica by its XRD; its description), \
         decision-p14-t49i-rock-mixtures §2.4's rule; opaque, so their w is held as measured; \
         their ASD spectra each inverted at their own geometry, then averaged",
    ),
);

/// Iron powder SA-209309, 325 mesh.
pub(in crate::substance) const IRON: EndMember = EndMember::new(
    "iron powder SA-209309",
    &[asd("splib07a_IronPowder_SA-209309_325mesh_ASDFRa_AREF.txt")],
    GrainScaling::Held,
    described(
        "end-member iron powder SA-209309",
        "splib07a's IronPowder_SA-209309_325mesh_ASDFRa_AREF (Kokaly et al. 2017, U.S. public \
         domain): Sigma-Aldrich iron powder of 97% Fe, −325 mesh, < 44 µm (its description); \
         opaque, so its w is held as measured; its ASD spectrum inverted at its own geometry",
    ),
);

/// Iron powder SA-267953, < 10 µm.
pub(in crate::substance) const IRON_FINE: EndMember = EndMember::new(
    "iron powder SA-267953",
    &[asd(
        "splib07a_Iron_Powder_SA-267953_lt10_um_ASDHRa_AREF.txt",
    )],
    GrainScaling::Held,
    described(
        "end-member iron powder SA-267953",
        "splib07a's Iron_Powder_SA-267953_lt10_um_ASDHRa_AREF (Kokaly et al. 2017, U.S. public \
         domain): Sigma-Aldrich iron powder of at least 99.9% Fe, < 10 µm (its description), \
         iron's finest measured fraction; opaque, so its w is held as measured; its ASD spectrum \
         inverted at its own geometry",
    ),
);

/// Iron sulphide, Cerac I-1047.
pub(in crate::substance) const IRON_SULPHIDE: EndMember = EndMember::new(
    "iron sulphide Cerac I-1047",
    &[asd("splib07a_Iron_Sulfide_Cerac-I-1047_ASDHRa_AREF.txt")],
    GrainScaling::Held,
    described(
        "end-member iron sulphide Cerac I-1047",
        "splib07a's Iron_Sulfide_Cerac-I-1047_ASDHRa_AREF (Kokaly et al. 2017, U.S. public \
         domain): Cerac's synthetic FeS of about 99.9%, −100 mesh, < 149 µm (its description), in \
         place of Pyrrhotite HS269, whose XRD of 2005 finds mostly szomolnokite and \
         hydroniumjarosite; opaque, so its w is held as measured; at a mixture's fine grains this \
         coarse powder's w is an upper bound on the fine phase's, opaque powders darkening as \
         they get finer; its ASD spectrum inverted at its own geometry",
    ),
);

/// Chromite HS281.3B.
pub(in crate::substance) const CHROMITE: EndMember = EndMember::new(
    "chromite HS281.3B",
    &[asd("splib07a_Chromite_HS281.3B_ASDFRc_AREF.txt")],
    GrainScaling::Held,
    described(
        "end-member chromite HS281.3B",
        "splib07a's Chromite_HS281.3B_ASDFRc_AREF (Kokaly et al. 2017, U.S. public domain): a \
         sample of 50 vol% chromite, 30 vol% iron-stained feldspar and 20 vol% white mica (its \
         description), the .3B sieve fraction; held as measured, chromite being opaque; its ASD \
         spectrum inverted at its own geometry",
    ),
);

/// Hematite GDS27.
pub(in crate::substance) const HEMATITE: EndMember = EndMember::new(
    "hematite GDS27",
    &[beckman("splib07a_Hematite_GDS27_BECKa_AREF.txt")],
    GrainScaling::Held,
    described(
        "end-member hematite GDS27",
        "splib07a's Hematite_GDS27_BECKa_AREF (Kokaly et al. 2017, U.S. public domain): a pure \
         reagent hematite of 200 µm aggregates of 2–3 µm grains (its description); opaque, so its \
         w is held as measured; its Beckman spectrum inverted at its own geometry",
    ),
);

/// Ferrihydrite GDS75.
pub(in crate::substance) const FERRIHYDRITE: EndMember = EndMember::new(
    "ferrihydrite GDS75",
    &[beckman("splib07a_Ferrihydrite_GDS75_Syn_F6_BECKb_AREF.txt")],
    GrainScaling::Held,
    described(
        "end-member ferrihydrite GDS75",
        "splib07a's Ferrihydrite_GDS75_Syn_F6_BECKb_AREF (Kokaly et al. 2017, U.S. public domain): \
         a synthetic, X-ray amorphous ferrihydrite (its description), its grain size not stated, \
         so its w is held as measured; its Beckman spectrum inverted at its own geometry",
    ),
);

/// Kerogen BK-Cornell.
pub(in crate::substance) const KEROGEN: EndMember = EndMember::new(
    "kerogen BK-Cornell",
    &[beckman("splib07a_Kerogen_BK-Cornell_BECKa_AREF.txt")],
    GrainScaling::Held,
    described(
        "end-member kerogen BK-Cornell",
        "splib07a's Kerogen_BK-Cornell_BECKa_AREF (Kokaly et al. 2017, U.S. public domain): an \
         insoluble coal-tar residue (its description), its grain size not stated, so its w is \
         held as measured, its channel at 851 nm deleted and interpolated across; the named \
         analogue of chondritic and cometary insoluble organic matter, of which no open index or \
         spectrum was found (decision-p14-t49i-rock-mixtures §2.4); its Beckman spectrum \
         inverted at its own geometry",
    ),
);

/// Carbon black GDS68.
pub(in crate::substance) const CARBON_BLACK: EndMember = EndMember::new(
    "carbon black GDS68",
    &[beckman("splib07a_Carbon_Black_GDS68_sm.ap._BECKa_AREF.txt")],
    GrainScaling::Held,
    described(
        "end-member carbon black GDS68",
        "splib07a's Carbon_Black_GDS68_sm.ap._BECKa_AREF (Kokaly et al. 2017, U.S. public domain): \
         carbon black of 99.0% C, 0.17 µm spheres in clumps (its description), its channels at \
         2,720 and 2,944–2,976 nm deleted; opaque, so its w is held as measured; its Beckman \
         spectrum inverted at its own geometry",
    ),
);

/// Graphite, `graphite.json`'s opaque slab: an index file's end-member (P14.T49.k;
/// decision-p14-t49k Q2).
pub(in crate::substance) const GRAPHITE_SLAB: OpaqueEndMember = OpaqueEndMember::new(
    "graphite.json opaque slab",
    "graphite.json",
    described(
        "end-member graphite.json",
        "graphite.json, the `graphite` row's index file (randomly oriented graphite at 20 K, from \
         Draine 2003's dielectric functions for E ∥ c and E ⊥ c, ApJ 598, 1026, through Draine's \
         site), as an opaque slab: w the hemispherically averaged complex Fresnel reflectance \
         S_e of its n and k, the slab's own limit for a grain of 1 µm, graphite's absorption \
         length λ ÷ (4πk), 1/α, being about 0.04 µm at 550 nm (decision-p14-t49k Q2); the file's \
         data cover \
         380–780 nm, and the part's w holds its 780 nm value beyond 780 nm and its 380 nm value \
         below 380 nm, as the `graphite` row's bins hold its last covered bin's, labelled, so \
         that it does not cut the mixture's reach, its spectra's; Draine's own infrared from the \
         same two files, which would move a row's Bond albedo by about +0.007 (`basalt_reduced`, \
         decision-p14-t49k Q2), is not read: a deferred finding",
    ),
);

// ---------------------------------------------------------------------------------------------
// The mixtures: each row's parts at its cited pristine mode, and the mode's source.

/// `anorthosite`'s parts.
pub(in crate::substance) const ANORTHOSITE: &[MixturePart] =
    &[form(&ANORTHITE, 0.95), form(&HYPERSTHENE_C2368, 0.05)];

/// `anorthosite`'s mode.
pub(in crate::substance) const ANORTHOSITE_MODE: Source = assumed(
    "the registry's anorthosite (its grain density's source, after science-composition-crusts \
     §7.2): 95 per cent An95 plagioclase and 5 per cent low-calcium pyroxene, by volume (its 2,790 \
     kg m⁻³ is their volume-weighted mean at 2,753 and about 3,500), a lunar ferroan \
     anorthosite's proportions as that ruling gives them from memory, not read (its §7.1), so an \
     assumption here; the pyroxene as hypersthene NMNHC2368 (Fs26), the nearest of splib07a's \
     low-Ca pyroxenes to a lunar anorthosite's",
);

/// `feldspathic`'s parts.
pub(in crate::substance) const FELDSPATHIC: &[MixturePart] = &[
    form(&ANORTHITE, 0.8099),
    form(&HYPERSTHENE_C2368, 0.1441),
    form(&OLIVINE_FO66, 0.0461),
];

/// `feldspathic`'s mode.
pub(in crate::substance) const FELDSPATHIC_MODE: Source = described(
    "mode",
    "science-composition-crusts §10.1's lunar highlands: 77 wt% An95 plagioclase, 17.25 wt% \
     low-Ca pyroxene and 5.75 wt% olivine of Mg# 68, at 2,753, 3,467 and 3,614 kg m⁻³, to vol%: \
     0.8099, 0.1441 and 0.0461; the pyroxene as hypersthene NMNHC2368 (Fs26) and the olivine as \
     KI3054 (Fo65.5), splib07a's nearest to Mg# 68",
);

/// `komatiite`'s parts.
pub(in crate::substance) const KOMATIITE: &[MixturePart] =
    &[form(&OLIVINE_FO89, 0.40), form(&AUGITE, 0.60)];

/// `komatiite`'s mode.
pub(in crate::substance) const KOMATIITE_MODE: Source = assumed(
    "the spinifex-textured komatiite SA 3091 of Munro Township, by volume: skeletal olivine 40, \
     subcalcic augite 45 and devitrified glass 15 per cent (Arndt 1976, Carnegie Inst. Washington \
     Year Book 75, 555, p. 555, the registry's grain density's source), the olivine Fo90–92 (the \
     registry's assumption); the glass counted as augite, an assumption no source gives; the \
     olivine as GDS70.c (Fo90.5) and the augite as NMNH120049",
);

/// `ultramafic`'s parts.
pub(in crate::substance) const ULTRAMAFIC: &[MixturePart] =
    &[form(&OLIVINE_FO89, 0.60), form(&ENSTATITE, 0.40)];

/// `ultramafic`'s mode.
pub(in crate::substance) const ULTRAMAFIC_MODE: Source = assumed(
    "Hu, Ehlmann and Seager 2012's ultramafic type, \"60% Olivine, 40% Enstatite\" (ApJ 752, 7, \
     Table 2, the registry's grain density's source), of Mg# 90 (the registry's assumption); the \
     paper does not say that its shares are by volume, so taking them as volume fractions is an \
     assumption; as olivine GDS70.c (Fo90.5) and enstatite NMNH128288 (Fs7)",
);

/// `basalt_fe`'s parts.
pub(in crate::substance) const BASALT_FE: &[MixturePart] = &[
    form(&AUGITE, 0.3089),
    form(&PIGEONITE, 0.3089),
    form(&OLIVINE_FO66, 0.0644),
    form(&ANORTHITE, 0.2483),
    form(&ILMENITE, 0.0694),
];

/// `basalt_fe`'s mode.
pub(in crate::substance) const BASALT_FE_MODE: Source = assumed(
    "the low-Ti mare basalts' mode, the mean of the Lunar Sourcebook's (Heiken, Vaniman and French \
     1991, ch. 6, Table 6.1, p. 188) Apollo 12 ilmenite, pigeonite and olivine and Apollo 15 \
     olivine and pigeonite basalts: 61.76 vol% pyroxene, 24.82 plagioclase, 6.44 olivine and 6.94 \
     opaques, to fractions; the pyroxene split 1:1 between augite NMNH120049 (Wo34 En45 Fs21) and \
     pigeonite HS199.3B, an assumption no source gives; the opaques (ilmenite, armalcolite, spinel \
     and troilite, the table's footnote) as ilmenite HS231.3B at the form's grains, no fine \
     fraction being cited (decision-p14-t49i-rock-mixtures §2.2 and §2.7)",
);

/// `basalt_reduced`'s parts.
pub(in crate::substance) const BASALT_REDUCED: &[MixturePart] = &[
    form(&ANORTHITE, 0.5400),
    form(&ENSTATITE, 0.2804),
    form(&DIOPSIDE, 0.0909),
    form(&OLIVINE_FO91, 0.0886),
];

/// `basalt_reduced`'s mode.
pub(in crate::substance) const BASALT_REDUCED_MODE: Source = assumed(
    "an assumption inside Charlier and Namur 2017's bounds for Mercury's Intermediate Plains \
     (IcP-HCT), whose solidus mineralogy has the most enstatite (above 25%) and much plagioclase \
     (above 45%) (LPSC XLVIII, #1173, p. 2): 50 wt% plagioclase, 30 enstatite, 10 diopside and 10 \
     forsterite, at the Handbook of Mineralogy's 2,760, 3,189 and 3,278 kg m⁻³ and 3,362 for the \
     forsterite, olivine at GDS71.a's Fa8.1, linear between forsterite's D(calc.) 3.271 and \
     fayalite's 4.40 g cm⁻³, to vol%: 0.5400, 0.2804, 0.0909 and 0.0886; the plagioclase as \
     anorthite, the Northern Volcanic Plains' sodic melts not drawn; sulphides not drawn",
);

/// `basalt_reduced`'s parts from `ROW_DARKENING_VERSION` (P14.T49.k): 23's scaled by 0.9817, and
/// Mercury's global carbon as graphite at the fine grains.
pub(in crate::substance) const BASALT_REDUCED_DARKENED: &[MixturePart] = &[
    form(&ANORTHITE, 0.5301),
    form(&ENSTATITE, 0.2753),
    form(&DIOPSIDE, 0.0892),
    form(&OLIVINE_FO91, 0.0870),
    fine_opaque(&GRAPHITE_SLAB, 0.0183),
];

/// `basalt_reduced`'s mode from `ROW_DARKENING_VERSION` (P14.T49.k).
pub(in crate::substance) const BASALT_REDUCED_DARKENED_MODE: Source = assumed(
    "`basalt_reduced`'s silicate mode, an assumption inside Charlier and Namur 2017's bounds for \
     Mercury's Intermediate Plains (IcP-HCT), whose solidus mineralogy has the most enstatite \
     (above 25%) and much plagioclase (above 45%) (LPSC XLVIII, #1173, p. 2): 50 wt% plagioclase, \
     30 enstatite, 10 diopside and 10 forsterite, to vol% 0.5400, 0.2804, 0.0909 and 0.0886 (23's \
     row's), the plagioclase as anorthite, the Northern Volcanic Plains' sodic melts and the \
     sulphides not drawn; with Mercury's global near-surface carbon, 1.4 ± 0.9 wt% (Peplowski et \
     al. 2015, PSS 108, 98, from MESSENGER's gamma-ray spectrometer: \"consistent with 0-4.1 wt% \
     C at the three-standard-deviation level and therefore does not represent a definitive \
     detection\", its abstract as decision-p14-t49k relays it, not re-read here), all of it as \
     graphite, an assumption (Xu et al. 2024, Nat. Astron. \
     8, 280, find under 1 wt% as intergrain graphite; Trang, Lucey and Izenberg 2017, Icarus 293, \
     206, model 1.9 wt% submicroscopic carbon); at the registry's grain densities, graphite's \
     2,261 and the rock's 2,970 kg m⁻³, (1.4 ÷ 2,261) ÷ (1.4 ÷ 2,261 + 98.6 ÷ 2,970) = 1.831 vol% \
     graphite, to 0.0183, and the silicates scaled by 0.9817, to 0.5301, 0.2753, 0.0892 and \
     0.0870; the low-reflectance material's 1–3 wt% (Peplowski et al. 2016, Nat. Geosci. 9, 273) \
     and Klima et al. 2018's 2.5 and 4 wt% (GRL 45, 2945) are excesses over the local mean, not \
     used, and carbon black is not used (decision-p14-t49k Q2 and Q3)",
);

/// `basalt_reduced`'s fine grains from `ROW_DARKENING_VERSION` (P14.T49.k).
pub(in crate::substance) const BASALT_REDUCED_DARKENED_FINE: Source = Source {
    covers: "fine grains",
    citation: "the graphite at 1 µm, all of it (decision-p14-t49i-rock-mixtures §2.2 and §2.3, \
               `Adopted`, never fitted): Murchie et al. 2015 (Icarus 254, 287) model Mercury's \
               dark phases as intimate mixtures \"under the assumption that the grain size is 1 \
               μm or larger\"; at the form's 70 µm the same carbon would leave the row at about \
               0.50 about a Sun, the carbon darkening only when fine (decision-p14-t49k Q2); \
               beyond about 1.5 µm a 1 µm grain is below geometric optics (πD ÷ λ < 2), and the \
               weights stay geometric there: an approximation in the row's infrared bins (§2.3)",
    basis: Basis::Adopted,
};

/// `granite`'s parts.
pub(in crate::substance) const GRANITE: &[MixturePart] = &[
    form(&QUARTZ, 0.2761),
    form(&MICROCLINE, 0.3685),
    form(&OLIGOCLASE, 0.3153),
    form(&BIOTITE, 0.0321),
    form(&MAGNETITE, 0.0080),
];

/// `granite`'s mode.
pub(in crate::substance) const GRANITE_MODE: Source = assumed(
    "Chayes's mode of the granite G-1 (Fairbairn et al. 1951, USGS Bulletin 980, Table 26, p. \
     61): 27.5 vol% quartz, 35.4 K-feldspar, 31.4 plagioclase (oligoclase, p. 59), 3.2 biotite, \
     1.3 muscovite, 0.8 opaques and 0.4 non-opaque accessories; the muscovite counted with the \
     K-feldspar as microcline, an assumption no source gives, the non-opaque accessories left out \
     and the rest normalised; the opaques as magnetite HS195.3B",
);

/// `phyllosilicate`'s parts.
pub(in crate::substance) const PHYLLOSILICATE: &[MixturePart] =
    &[form(&LIZARDITE, 0.82), form(&SAPONITE, 0.18)];

/// `phyllosilicate`'s mode.
pub(in crate::substance) const PHYLLOSILICATE_MODE: Source = assumed(
    "an assumption, inferred and not cited: the registry's grain density, 2,500 kg m⁻³, between \
     saponite's 2,270 (the Handbook of Mineralogy's 2.24–2.30) and lizardite's 2,550 (its \
     D(meas.)), as a volume-weighted mean, gives 82 vol% lizardite and 18 vol% saponite",
);

/// `ferric`'s parts.
pub(in crate::substance) const FERRIC: &[MixturePart] = &[
    form(&AUGITE, 0.2842),
    form(&PIGEONITE, 0.2842),
    form(&OLIVINE_FO66, 0.0593),
    form(&ANORTHITE, 0.2284),
    form(&ILMENITE, 0.0639),
    form(&FERRIHYDRITE, 0.0619),
    form(&HEMATITE, 0.0064),
    form(&MAGNETITE, 0.0117),
];

/// `ferric`'s mode.
pub(in crate::substance) const FERRIC_MODE: Source = assumed(
    "the registry's ferric soil (its grain density's source): Rocknest's hematite 1.0 and \
     magnetite 1.8 wt% (Achilles et al. 2017, JGR Planets 122, 2344, Table 1) and nanophase ferric \
     oxide 6.8 wt% (Morris et al. 2013, LPSC 44, 1653, Table 3), in 90.4 wt% `basalt_fe`, at \
     5,260 and 5,175 kg m⁻³ (the Handbook of Mineralogy's), ferrihydrite's 3,700 (the midpoint \
     of Hiemstra and Van Riemsdijk 2009's 3.5–3.9 g cm⁻³) and `basalt_fe`'s 3,310, to vol%: \
     0.0064, 0.0117, 0.0619 and 0.9200, `basalt_fe`'s parts scaled by 0.9200, its pyroxene's 1:1 \
     split among them, an assumption no source gives; the nanophase oxide as ferrihydrite GDS75, \
     the rest at the form's grains",
);

/// `ferric`'s parts from `ROW_DARKENING_VERSION` (P14.T49.k): Rocknest's mode with its nanophase
/// ferric oxide as ferrihydrite's own mass ([`FERRIC_DARKENED_MODE`]), the oxide at the fine
/// grains.
pub(in crate::substance) const FERRIC_DARKENED: &[MixturePart] = &[
    form(&AUGITE, 0.2807),
    form(&PIGEONITE, 0.2807),
    form(&OLIVINE_FO66, 0.0585),
    form(&ANORTHITE, 0.2256),
    form(&ILMENITE, 0.0631),
    fine(&FERRIHYDRITE, 0.0736),
    form(&HEMATITE, 0.0063),
    form(&MAGNETITE, 0.0116),
];

/// `ferric`'s mode from `ROW_DARKENING_VERSION` (P14.T49.k; decision-p14-t49k, its addendum's
/// item 3).
pub(in crate::substance) const FERRIC_DARKENED_MODE: Source = assumed(
    "the registry's ferric soil (its grain density's source): Rocknest's hematite 1.0 and \
     magnetite 1.8 wt% (Achilles et al. 2017, JGR Planets 122, 2344, Table 1) and nanophase ferric \
     oxide 6.8 wt% as Fe2O3 (Morris et al. 2013, LPSC 44, 1653, Table 3, \"Fe2O3-npOx 6.8\"), in \
     90.4 wt% `basalt_fe`; the nanophase oxide as ferrihydrite Fe5HO8·4H2O, whose Fe2O3 is 2.5 × \
     159.687 ÷ 480.285 = 0.8312 of its mass, so 6.8 ÷ 0.8312 = 8.181 wt% of ferrihydrite; at 5,260 \
     and 5,175 kg m⁻³ (the Handbook of Mineralogy's), ferrihydrite's 3,700 (the midpoint of \
     Hiemstra and Van Riemsdijk 2009's 3.5–3.9 g cm⁻³) and `basalt_fe`'s 3,310, the volumes 1.0 ÷ \
     5,260, 1.8 ÷ 5,175, 8.181 ÷ 3,700 and 90.4 ÷ 3,310 give, to vol%, 0.0063, 0.0116, 0.0736 and \
     0.9086 (23's row and decision-p14-t49k Q5 took the 6.8 wt% Fe2O3 as ferrihydrite's own mass, \
     0.0619, which this corrects), `basalt_fe`'s parts scaled by 0.9086, its pyroxene's 1:1 split \
     among them, an assumption no source gives; the nanophase oxide as ferrihydrite GDS75 at the \
     fine grains, the rest at the form's",
);

/// `ferric`'s fine grains from `ROW_DARKENING_VERSION` (P14.T49.k).
pub(in crate::substance) const FERRIC_DARKENED_FINE: Source = Source {
    covers: "fine grains",
    citation: "the nanophase ferric oxide at 1 µm, all of it (decision-p14-t49i-rock-mixtures \
               §2.2 and §2.3, `Adopted`, never fitted; decision-p14-t49k Q5): Morris et al. \
               2013's nanophase ferric oxide (LPSC 44, 1653, Table 3), fine by definition, as \
               ferrihydrite GDS75, a synthetic, X-ray amorphous ferrihydrite, whose measured w is \
               a fine phase's; its hematite and magnetite, their sizes not cited, and \
               `basalt_fe`'s parts stay at the form's grains; beyond about 1.5 µm a 1 µm grain is \
               below geometric optics (πD ÷ λ < 2), and the weights stay geometric there: an \
               approximation in the row's infrared bins (§2.3)",
    basis: Basis::Adopted,
};

/// `chondrite_ci`'s parts.
pub(in crate::substance) const CHONDRITE_CI: &[MixturePart] = &[
    form(&SAPONITE, 0.3905),
    form(&LIZARDITE, 0.3905),
    fine(&IRON_SULPHIDE, 0.0536),
    fine(&MAGNETITE_FINE, 0.0630),
    form(&OLIVINE_FO80, 0.0094),
    fine(&KEROGEN, 0.0930),
];

/// `chondrite_ci`'s mode.
pub(in crate::substance) const CHONDRITE_CI_MODE: Source = assumed(
    "Orgueil's mean mode (King et al. 2015, GCA 165, 148, Table 1, p. 154, read in Open Research \
     Online's CC BY 4.0 copy of the version of record): 83.0 vol% \
     phyllosilicate, 5.7 sulphide, 6.7 magnetite and 1.0 olivine (Fo80), its ferrihydrite (3.3) \
     and gypsum (0.3) left out as terrestrial weathering products (Amano et al. 2023, Sci. Adv. 9, \
     eadi3789: CI's Fe hydroxides and Ca and Mg sulphates formed after the fall), normalised, then \
     scaled by 0.907 for 9.3 vol% organic matter (C 3.5 wt%, Alexander et al. 2012 via Kurokawa et \
     al. 2020, as about 5 wt% organic matter at 1,400 kg m⁻³ in a bulk of 2,590, not read here); \
     the phyllosilicate split 1:1 between saponite and serpentine (lizardite), an assumption no \
     source gives; the olivine as HS285.2B (Fo80), the organics as kerogen",
);

/// `chondrite_ci`'s fine grains.
pub(in crate::substance) const CHONDRITE_CI_FINE: Source = Source {
    covers: "fine grains",
    citation: "the organics, magnetite and sulphides at 1 µm (decision-p14-t49i-rock-mixtures §2.2 \
               and §2.3, `Adopted`, never fitted), all of their mode, CI's matrix being above 99 \
               vol% (Weisberg, McCoy and Krot 2006, MESS II, Table 1, p. 26); beside Orgueil's \
               magnetite framboids' crystals of 0.1–1 µm (Hua and Buseck 1998, MAPS 33, A215, p. \
               A217), Ryugu's magnetite of 0.10–1.5 µm, mean 0.64 µm (Kimura et al. 2023, Sci. \
               Rep. 13, 14096), and its sulphides under 1 µm and sub-µm organic matter (Nakamura \
               et al. 2022, PJA-B 98, 227); the magnetite as the mean of its finest fractions, the \
               sulphide as Cerac's −100 mesh FeS, an upper bound on its w; beyond about 1.5 µm a \
               1 µm grain is below geometric optics (πD ÷ λ < 2), and the weights stay geometric \
               there: an approximation in the row's infrared bins (§2.3)",
    basis: Basis::Adopted,
};

/// `chondrite_cm`'s parts.
pub(in crate::substance) const CHONDRITE_CM: &[MixturePart] = &[
    form(&OLIVINE_FO91, 0.0637),
    form(&OLIVINE_FO80, 0.0178),
    form(&OLIVINE_FO51, 0.0150),
    form(&ENSTATITE, 0.0178),
    form(&LIZARDITE, 0.2455),
    form(&CRONSTEDTITE, 0.5444),
    form(&CALCITE, 0.0112),
    fine(&KEROGEN, 0.0630),
    fine(&IRON_SULPHIDE, 0.0138),
    form(&IRON_SULPHIDE, 0.0059),
    fine(&MAGNETITE_FINE, 0.0013),
    form(&MAGNETITE, 0.0006),
];

/// `chondrite_cm`'s mode.
pub(in crate::substance) const CHONDRITE_CM_MODE: Source = described(
    "mode",
    "Murchison's mode (Bland et al. 2004, MAPS 39, 3, Table 2, p. 12): olivine Fo100 6.8, Fo80 \
     1.9 and Fo50 1.6 vol%, clinoenstatite (En98) 1.9, pyrrhotite 1.8 and pentlandite 0.3, \
     magnetite 0.2, serpentine 26.2, cronstedtite 58.1 and calcite 1.2, scaled by 0.937 for 6.3 \
     vol% organic matter (C 2.08 wt% as about 3 wt% organic matter at 1,400 kg m⁻³ in a bulk of \
     2,920, its carbonate's carbon not subtracted; not read here); the olivines as GDS71.a \
     (Fo91.9, the most Mg-rich), HS285.2B (Fo80) and KI3188 (Fo50.6), the clinoenstatite as \
     NMNH128288 (Fs7), the sulphides as FeS, the serpentine as lizardite, the calcite as \
     GDS304 and the organics as kerogen; the opaques' matrix share 0.70 fine (below) and the \
     rest at the form's grains, the magnetite there as HS195.3B",
);

/// `chondrite_cm`'s fine grains.
pub(in crate::substance) const CHONDRITE_CM_FINE: Source = Source {
    covers: "fine grains",
    citation: "the organics, all fine, and the sulphides and magnetite at their matrix share, \
               CM's matrix being 70 vol% (Weisberg, McCoy and Krot 2006, MESS II, Table 1, p. \
               26; \"highly variable\"), at 1 µm (decision-p14-t49i-rock-mixtures §2.2 and §2.3, \
               `Adopted`, never fitted); beside the pristine CM Asuka 12169's matrix of sub-µm \
               silicates and metal and abundant Fe sulphides under 100 nm (Nittler et al. 2021, \
               MAPS 56, 260); the magnetite as the mean of its finest fractions, the sulphide as \
               Cerac's −100 mesh FeS, an upper bound on its w; beyond about 1.5 µm a 1 µm grain \
               is below geometric optics (πD ÷ λ < 2), and the weights stay geometric there: an \
               approximation in the row's infrared bins (§2.3)",
    basis: Basis::Adopted,
};

/// `chondrite_cv`'s parts.
pub(in crate::substance) const CHONDRITE_CV: &[MixturePart] = &[
    form(&OLIVINE_FO91, 0.2340),
    form(&OLIVINE_FO80, 0.1540),
    form(&OLIVINE_FO60, 0.2060),
    form(&OLIVINE_FO51, 0.2060),
    form(&OLIVINE_FO29, 0.0390),
    form(&ENSTATITE, 0.0660),
    form(&ANORTHITE, 0.0120),
    fine(&IRON_SULPHIDE, 0.0320),
    form(&IRON_SULPHIDE, 0.0480),
    fine(&MAGNETITE_FINE, 0.0008),
    form(&MAGNETITE, 0.0012),
    fine(&IRON_FINE, 0.0004),
    form(&IRON, 0.0006),
];

/// `chondrite_cv`'s mode.
pub(in crate::substance) const CHONDRITE_CV_MODE: Source = described(
    "mode",
    "Allende's mode (Bland et al. 2004, MAPS 39, 3, Table 1, p. 10): olivine Fo100 23.4, Fo80 \
     15.4, Fo60 20.6, Fo50 20.6 and Fo25 3.9 vol%, pentlandite 8.0, clinoenstatite (En90) 6.6, \
     anorthite 1.2, magnetite 0.2 and metal 0.1; the olivines as GDS71.a (Fo91.9), HS285.2B \
     (Fo80), KI3189 (Fo60.3), KI3188 (Fo50.6) and KI3291 (Fo29.5), the clinoenstatite as \
     NMNH128288 (Fs7), the pentlandite as FeS; the opaques' matrix share 0.40 fine (below) and the \
     rest at the form's grains, as HS195.3B magnetite and 325-mesh iron there",
);

/// `chondrite_cv`'s fine grains.
pub(in crate::substance) const CHONDRITE_CV_FINE: Source = Source {
    covers: "fine grains",
    citation: "the matrix-hosted opaques (sulphide, magnetite and metal) at their matrix share, \
               CV's matrix being 40 vol% (Weisberg, McCoy and Krot 2006, MESS II, Table 1, p. \
               26), at 1 µm (decision-p14-t49i-rock-mixtures §2.2 and §2.3, `Adopted`, never \
               fitted), the row standing for CO and CK too; the magnetite as the mean of its \
               finest fractions, the metal as iron < 10 µm, the sulphide as Cerac's −100 mesh \
               FeS, an upper bound on its w; beyond about 1.5 µm a 1 µm grain is below geometric \
               optics (πD ÷ λ < 2), and the weights stay geometric there: an approximation in the \
               row's infrared bins (§2.3)",
    basis: Basis::Adopted,
};

/// `chondrite_h`'s parts.
pub(in crate::substance) const CHONDRITE_H: &[MixturePart] = &[
    form(&OLIVINE_FO80, 0.3558),
    form(&HYPERSTHENE_PYX02, 0.2340),
    form(&HYPERSTHENE_C2368, 0.0576),
    form(&DIOPSIDE, 0.0769),
    form(&OLIGOCLASE, 0.1282),
    form(&IRON_SULPHIDE, 0.0450),
    form(&IRON, 0.0867),
    form(&CHROMITE, 0.0157),
];

/// `chondrite_l`'s parts.
pub(in crate::substance) const CHONDRITE_L: &[MixturePart] = &[
    form(&OLIVINE_FO80, 0.2733),
    form(&OLIVINE_FO66, 0.1528),
    form(&HYPERSTHENE_PYX02, 0.1151),
    form(&HYPERSTHENE_C2368, 0.1354),
    form(&DIOPSIDE, 0.0890),
    form(&OLIGOCLASE, 0.1286),
    form(&IRON_SULPHIDE, 0.0535),
    form(&IRON, 0.0382),
    form(&CHROMITE, 0.0140),
];

/// `chondrite_ll`'s parts.
pub(in crate::substance) const CHONDRITE_LL: &[MixturePart] = &[
    form(&OLIVINE_FO80, 0.1098),
    form(&OLIVINE_FO66, 0.3877),
    form(&HYPERSTHENE_PYX02, 0.0394),
    form(&HYPERSTHENE_C2368, 0.1828),
    form(&DIOPSIDE, 0.0794),
    form(&OLIGOCLASE, 0.1303),
    form(&IRON_SULPHIDE, 0.0412),
    form(&IRON, 0.0155),
    form(&CHROMITE, 0.0139),
];

/// The ordinary chondrites' mode's source, for group `group` of `rows` meteorites in Dunn et al.
/// 2010's Table 4, its olivine Fa and pyroxene Fs.
macro_rules! ordinary_mode {
    ($group:literal, $rows:literal, $mode:literal, $fa:literal, $fs:literal, $split:literal) => {
        described(
            "mode",
            concat!(
                "the ",
                $group,
                " group's mean mode, the mean of its ",
                $rows,
                " rows in Dunn et al. 2010's Table 4 (MAPS 45, 123, read in the Internet \
                 Archive's copy of the publisher's free-access page, 2020-06-02): ",
                $mode,
                " wt% olivine, low-Ca pyroxene, high-Ca pyroxene, plagioclase, troilite, metal and \
                 other phases, to vol% at the Handbook of Mineralogy's D(calc.) (olivine and \
                 low-Ca pyroxene linear in Fa and Fs, plagioclase at An12, troilite 4,850, \
                 kamacite 7,900, the other phases, chromite and phosphates, about 4,000 kg m⁻³, \
                 assumed); the olivine matched to the group's equilibrated mean, Fa ",
                $fa,
                " (Weisberg, McCoy and Krot 2006, MESS II, Table 1, p. 26), and the low-Ca \
                 pyroxene to its range's midpoint, Fs ",
                $fs,
                " (Dunn et al. 2010, after Brearley and Jones 1998), each by splitting it \
                 between two of splib07a's samples, linear in composition: ",
                $split,
                "; the high-Ca pyroxene as diopside HS15.3B, the plagioclase as oligoclase \
                 HS110.3B, the troilite as FeS, the metal as 325-mesh iron and the other phases as \
                 chromite HS281.3B, all at the form's grains: the equilibrated (types 4–6) \
                 chondrites' metal and troilite are coarse (decision-p14-t49i-rock-mixtures §2.2)"
            ),
        )
    };
}

/// `chondrite_h`'s mode.
pub(in crate::substance) const CHONDRITE_H_MODE: Source = ordinary_mode!(
    "H",
    "18",
    "33.04, 25.61, 6.71, 8.98, 5.81, 18.23 and 1.67",
    "19.3",
    "16.25",
    "the olivine all HS285.2B (Fo80), the pyroxene 0.80 PYX02.f (Fs13.8) and 0.20 NMNHC2368 \
     (Fs26.2)"
);

/// `chondrite_l`'s mode.
pub(in crate::substance) const CHONDRITE_L_MODE: Source = ordinary_mode!(
    "L",
    "17",
    "42.12, 23.18, 8.11, 9.41, 7.22, 8.39 and 1.56",
    "25.2",
    "20.5",
    "the olivine 0.64 HS285.2B (Fo80) and 0.36 KI3054 (Fo65.5), the pyroxene 0.46 PYX02.f and \
     0.54 NMNHC2368"
);

/// `chondrite_ll`'s mode.
pub(in crate::substance) const CHONDRITE_LL_MODE: Source = ordinary_mode!(
    "LL",
    "13",
    "51.11, 21.12, 7.38, 9.72, 5.66, 3.46 and 1.58",
    "31.3",
    "24",
    "the olivine 0.22 HS285.2B and 0.78 KI3054, the pyroxene 0.18 PYX02.f and 0.82 NMNHC2368"
);

/// `chondrite_e`'s parts.
pub(in crate::substance) const CHONDRITE_E: &[MixturePart] = &[
    form(&ENSTATITE, 0.7231),
    form(&OLIGOCLASE, 0.1262),
    form(&IRON, 0.0953),
    form(&IRON_SULPHIDE, 0.0555),
];

/// `chondrite_e`'s mode.
pub(in crate::substance) const CHONDRITE_E_MODE: Source = described(
    "mode",
    "Atlanta's EL6 matrix (Rubin 1983, Meteoritics 18, 113, Table 1, p. 114): 63 wt% enstatite, 9 \
     plagioclase, 20.5 metal (kamacite, martensite and schreibersite) and 7.1 sulphides (troilite, \
     alabandite and daubréelite), at 3,200, 2,620, 7,900 and 4,700 kg m⁻³, to vol%: 0.7231, \
     0.1262, 0.0953 and 0.0555 (Rubin measured vol% and gave wt% by estimated densities); the \
     enstatite as NMNH128288 (Fs7, the most Mg-rich in splib07a, where E chondrites' is nearly \
     FeO-free), the metal as 325-mesh iron and the sulphides as FeS, all at the form's grains \
     (decision-p14-t49i-rock-mixtures §2.2)",
);

/// `cometary`'s parts.
pub(in crate::substance) const COMETARY: &[MixturePart] = &[
    fine(&KEROGEN, 0.6685),
    form(&OLIVINE_FO89, 0.1356),
    form(&ENSTATITE, 0.1356),
    fine(&IRON_SULPHIDE, 0.0603),
];

/// `cometary`'s mode.
pub(in crate::substance) const COMETARY_MODE: Source = assumed(
    "the registry's cometary refractory (its grain density's source): 45 wt% organics (Bardyn et \
     al. 2017, MNRAS 469, S712, §3.4) at 1,400 kg m⁻³ and 55 wt% minerals at 3,450 (the midpoint \
     of 3,430–3,480), so 66.85 vol% organics, and the minerals silicate to sulphide 4.5:1 by \
     volume (the midpoint of Fulle et al. 2016's 4–5 to 1, MNRAS 462, S132); the silicate split \
     equally between olivine GDS70.c (Fo90.5) and enstatite NMNH128288, an assumption no source \
     gives; the organics as kerogen",
);

/// `cometary`'s fine grains.
pub(in crate::substance) const COMETARY_FINE: Source = Source {
    covers: "fine grains",
    citation: "the organics and the sulphides at 1 µm (decision-p14-t49i-rock-mixtures §2.2 and \
               §2.3, `Adopted`, never fitted), all of their mode; the sulphide as Cerac's −100 \
               mesh FeS, an upper bound on its w; beyond about 1.5 µm a 1 µm grain is below \
               geometric optics (πD ÷ λ < 2), and the weights stay geometric there: an \
               approximation in the row's infrared bins (§2.3)",
    basis: Basis::Adopted,
};
