# Maneuvering Thrusters

The following directory contains YAML files for all of the thrusters in the game. Maneuvering thrusters provide angular thrust, affecting how quickly spacecraft turn. Each file has the following format:

```yaml
# (str) The name, model, kind, and manufacturer of the maneuvering thruster. The
# `name` is usually the same as the model, and is the only value that needs to
# be unique. The rest are for lore/rp purposes and will be displayed on the
# science screen when scanning a vessel.
name: Ion Thrusters
model: IV8X Ion Thrusters
kind: Ion Engine
manufacturer: Azul Deep Space Industries

# (str) A breif description of the thruster.
desc: >-
  Uses a series of small ion engines to produce a small amount of angular thrust with good energy efficiency.

# (int) The build cost of the thruster.
cost: 100

# (int) The weight of the thruster, in kg.
weight: 100

# (int) The amount of angular thrust provided by the thruster.
thrust: 400

# (int) The amount of energy consumed per second when provided 100% power.
energy_consumption: 10
```