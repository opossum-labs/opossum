# Pump scenarios

A pump scenario is a named operating point of a model: it states, for every amplifying component,
how strongly that component amplifies during one analysis. Scenarios belong to the document, not to
any node, and are stored in the `.opm` file. For the reasoning behind that split see
[Amplification](../concepts/amplification.md); for the steps to set one up, see
[Model an amplifier](../howto%20guides/model_an_amplifier.md).

A scenario consists of a name and, per node, a gain model. A node no scenario mentions is analyzed
exactly like the passive component it is.

## Gain models

`None`
: No amplification. The default, and what every node not named by the scenario falls back to. A node
  set to `None` behaves like the passive optic it is.

`Const`
: A constant energy gain. The energy of the light passing the medium is multiplied by the gain
  factor, once per traversal of the medium.

    `gain factor`
    : The factor the energy is multiplied by. Must be finite and non-negative; `1.0` is neutral, so
      selecting `Const` does not change a result until the factor is set. The factor does not depend
      on wavelength, on the path length through the medium, or on how much energy was already
      extracted — see [Amplification](../concepts/amplification.md) for what this model is and is not
      good for.

## Which components can amplify

Only nodes that enclose a volume of material can carry a gain model:

- [Spherical lens](./nodes/spherical_lens.md)
- [Wedge](./nodes/wedge.md)
- [Cylindric lens](./nodes/cylindric_lens.md)

This follows from the node type itself, not from a list maintained in the user interface, so a node
type gaining a volume in the future becomes usable as an amplifier without anything else being
changed. Other node types do not offer the setting.

**Amplifier candidate.** Marking a node as an amplifier is bookkeeping for the user interface: it
determines which nodes the scenario editor offers a row for and what the canvas shows on the node.
It is not what makes a component amplify. The analysis reads the gain model in the scenario and
nothing else — worth knowing when a model is built in code, where the marking is optional.

## Running an analysis in a scenario

Each analyzer carries a list of the scenarios it is run in, alongside its own configuration (see
[Analyzers](./analyzers.md)). The list behaves as follows:

- **One report per scenario**, produced in the order the scenarios are listed. The report's analysis
  type is extended by the scenario's name, e.g. `Energy Analysis - full power`, so the reports of one
  model at different operating points are told apart by their title.
- **An empty list means a single passive run** — the behaviour of every analyzer before scenarios
  existed. This is not an error and needs no scenario to be defined.
- Every run starts from the same model state, so scenarios do not influence one another regardless of
  the order they are listed in.
- Deleting a scenario removes it from the selection of every analyzer that referred to it. An
  analyzer pointing at a scenario that does not exist is reported as an error before the analysis
  starts, rather than silently running passively.

## Effect on the individual analyses

`Const` is evaluated in all three analyses:

Energy Analysis
: The spectrum passing the component is scaled by the gain factor.

Ray Tracing Analysis
: Every ray of the bundle inside the medium is scaled by the gain factor, between the entrance and
  the exit surface of the component.

Ghost Focus Analysis
: As in ray tracing, and applied on every pass — including the reflected paths. A ghost that crosses
  a pumped medium twice more than the main beam is amplified twice more.
