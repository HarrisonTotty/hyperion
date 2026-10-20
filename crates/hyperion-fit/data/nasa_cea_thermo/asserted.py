"""Print P14.T49.d's asserted values as Rust, from kf.py's independent evaluation of a CEA library.

For `substance::formation`'s test `substance_formation_reproduces_the_cea_records`: each registry
gas row's log10 K_f at three temperatures, from the records of the library given and NIST-JANAF's
reference states. A metal's atom, the gas form of an element whose reference state is condensed,
has K_f = 1 by definition above its switch, so its three values are taken below it: at 500 K, at
its switch rounded down to 100 K, and midway between the two. Every other row is taken at 500,
2,500 and 5,000 K. Each row's range follows the table.

Usage: python3 -I asserted.py <kf.py> <cea.py> <thermo.inp>
"""
import importlib.util
import math
import sys

kf_path, cea_path, inp = sys.argv[1:4]
sys.argv = [kf_path, cea_path, inp]
spec = importlib.util.spec_from_file_location("kf", kf_path)
kf = importlib.util.module_from_spec(spec)
spec.loader.exec_module(kf)
TEMPERATURES = (500.0, 2500.0, 5000.0)


def temperatures(name):
    """The three temperatures at which row record `name` is asserted, K."""
    for el, (_, gas_form, atoms) in kf.COND_REF.items():
        if gas_form == name and atoms == 1:
            top = math.floor(kf.REFS[el].switch / 100.0) * 100.0
            return (500.0, (500.0 + top) / 2.0, top)
    return TEMPERATURES


def rust(value):
    """`value` as a Rust literal, its digits grouped by three as Clippy's `unreadable_literal`
    asks."""
    if value == 0.0:
        return "0.0"
    text = repr(value)
    mantissa, _, exponent = text.partition("e")
    sign = "-" if mantissa.startswith("-") else ""
    whole, _, fraction = mantissa.lstrip("-").partition(".")
    if len(whole) > 3:
        head = len(whole) % 3 or 3
        whole = "_".join([whole[:head]] + [whole[k:k + 3] for k in range(head, len(whole), 3)])
    fraction = "_".join(fraction[k:k + 3] for k in range(0, len(fraction), 3))
    return f"{sign}{whole}.{fraction}" + (f"e{int(exponent)}" if exponent else "")


print("const CEA: [(&str, [f64; 3], [f64; 3]); 57] = [")
for key, name in kf.ROWS:
    s = kf.species_of(name)
    temps = temperatures(name)
    values = [s.log_kf(t) for t in temps]
    print(f'    ("{key}", [{", ".join(rust(t) for t in temps)}], '
          f'[{", ".join(rust(v) for v in values)}]),')
print("];")
print("ranges:")
for key, name in kf.ROWS:
    print(key, kf.species_of(name).range())
