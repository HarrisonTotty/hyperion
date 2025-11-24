# Power Cores

A power core YAML file has the following specification.

```yaml
# (str) The name, model, kind, and manufacturer of the power core. The `name` is usually the same as the model, and is the only value that needs to be unique. The rest are for lore/rp purposes and will be displayed on the science screen when scanning a vessel.
name: Mark III Fission Reactor
model: Mark III Fission Reactor
kind: Fission Reactor
manufacturer: MagniCo Industries

# (str) A brief description of the power core.
desc: >-
  A basic nuclear fission reactor core capable of slowly restoring its energy
  reserves if power is cut from other ship systems.

# (int) The cost of the power core.
cost: 100

# (int) The maximum amount of energy capable of being stored in the power core.
max_energy: 1000

# (int) The amount of energy produced every second by the power core.
production: 20

# (int) The weight of the power core in kg.
weight: 100
```