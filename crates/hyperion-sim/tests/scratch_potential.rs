use hyperion_sim::galaxy::params::GalaxyParams;
use hyperion_sim::galaxy::potential::MassModel;
use hyperion_sim::galaxy::potential::sigma;
use hyperion_sim::galaxy::potential::spherical::SphericalMass;
use hyperion_sim::galaxy::imf::MassFunctionKind;
use hyperion_sim::galaxy::quad::gl_panels;
use hyperion_sim::galaxy::consts::G;
use hyperion_sim::units::LightYears;
use hyperion_sim::Seed;
use hyperion_sim::math;

fn cyl(x: f64, xe: f64) -> f64 { let o = x*x - xe*xe; if o > 0.0 { x*x*x - o*o.sqrt() } else { x*x*x } }

fn sigma_with(p: &GalaxyParams, a: f64, monopole: bool) -> f64 {
    let m = MassModel::without_centre(p);
    let v2 = |r: f64| {
        let rr = LightYears::new(r);
        if monopole {
            G * (m.expanded_enclosed_mass(rr).value() + m.dark_halo().enclosed_mass(rr).value()) / r
        } else { m.v_circ_sq(rr) }
    };
    let xe = sigma::effective_radius_in_scales();
    let edges = [0.0, 0.5*xe, xe, 2.0*xe, 4.0*xe, 8.0*xe, 16.0*xe, 60.0];
    let w = gl_panels(|x| if x <= 0.0 {0.0} else { cyl(x, xe) * math::exp(-x) * v2(a*x) / x }, &edges);
    let d = gl_panels(|x| cyl(x, xe) * math::exp(-x), &edges);
    (w/d).sqrt()
}

fn scales(p: &GalaxyParams) -> (f64, f64) {
    let b = p.bulge();
    let (a, bb, c, pp) = (b.scale_x().value(), b.scale_y().value(), b.scale_z().value(), b.boxiness());
    let ln_beta = |x: f64, y: f64| math::ln_gamma(x) + math::ln_gamma(y) - math::ln_gamma(x + y);
    let vol_factor = 3.0 * math::exp(ln_beta(2.0/pp, 1.0/pp + 1.0)) / pp;
    (sigma::sphericalised_scale(p).value(), math::cbrt(vol_factor * a * bb * c))
}

#[test]
fn scratch() {
    let p = GalaxyParams::milky_way_like();
    let (am, av) = scales(&p);
    println!("fixture: plane/mom {:.1} plane/vol {:.1} mono/mom {:.1} mono/vol {:.1}", sigma_with(&p, am, false), sigma_with(&p, av, false), sigma_with(&p, am, true), sigma_with(&p, av, true));
    let mut rows = vec![];
    for n in 0..100u64 {
        let p = GalaxyParams::from_seed(Seed::new(0x1234_0000 + n), MassFunctionKind::Kroupa);
        let (am, av) = scales(&p);
        rows.push([sigma_with(&p, am, true), sigma_with(&p, av, true)]);
    }
    for k in 0..2 {
        let mut v: Vec<f64> = rows.iter().map(|r| r[k]).collect();
        v.sort_by(f64::total_cmp);
        let inside = v.iter().filter(|&&x| (90.0..=135.0).contains(&x)).count();
        println!("monopole variant {k}: 5% {:.1} median {:.1} 95% {:.1} inside {}/100", v[5], v[50], v[95], inside);
    }
}
