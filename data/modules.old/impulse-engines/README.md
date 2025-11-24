# Impulse Engines

The following directory contains the YAML files for impulse engines. These have following form:

```yaml
# (str) The name, model, kind, and manufacturer of the impulse engine. The
# `name` is usually the same as the model, and is the only value that needs to
# be unique. The rest are for lore/rp purposes and will be displayed on the
# science screen when scanning a vessel.
name: Ion Engines
model: IE83J Ion Engine
kind: Ion Engine
manufacturer: Azul Deep Space Industries

# (str) A breif description of the engine.
desc: >-
  Ion impulse engines provide little forward thrust but are extremely energy efficient.

# (int) The build cost of the engine.
cost: 100

# (int) The weight of the engine, in kg.
weight: 100

# (int) The amount of thrust provided by the engine.
thrust: 400

# (int) The amount of energy consumed per second when provided 100% power.
energy_consumption: 10
```