# Directed Energy Weapons

The following directory contains the YAML files for the directed energy weapons. These are files with the following specification:

```yaml
# (str) The name, model, kind, and manufacturer of the weapon. The
# `name` is usually the same as the model, and is the only value that needs to
# be unique. The rest are for lore/rp purposes and will be displayed on the
# science screen when scanning a vessel.
name: Laser Beam
model: LLV7 FocalPoint 9
kind: Laser Beam
manufacturer: SunScore Systems Inc.

# (str) A breif description of the weapon.
desc: >-
  A high-wattage laser beam capable of traversing incredibly long ranges at the
  speed of light. Deals a small amount of continuous damage at very long
  ranges.

# (int) The cost of the weapon.
cost: 10

# (int) The weight of the weapon, in kg.
weight: 100

# (int) A delay between when the weapon is ordered to fire and when firing occurs, in seconds.
fire_delay: 1

# (int) The amount of time it takes to recharge the weapon at 100% power, in seconds.
recharge_time: 5

# (int) The speed of the beam or particle (m/s). If set to `-1`, the weapon instantly reaches the target location.
speed: -1

# (int) The maximum range of the weapon (m).
max_range: 10000

# (list) A list of weapon tags associated with the weapon.
tags: ["Photon", "Beam", "Manual", "Toggle"]

# (int) The damage of the weapon. For beam weapons, this damage is per second.
damage: 5

# (int) The amount of energy consumed during the activation of the weapon.
energy_consumption: 10
```

## Design Guidelines

1. See `doc/game.md` for a list of weapon tags and their effects.