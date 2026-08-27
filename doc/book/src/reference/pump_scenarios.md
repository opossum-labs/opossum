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

`Small Signal Gain`
: A monochromatic, unsaturated gain model. A pump source fills the component's volume with a
  spatial gain field; each ray accumulates gain along the actual path it takes through the medium.
  The gain coefficient g₀ is wavelength-independent — all wavelengths in the beam receive the same
  gain. For the underlying concepts see [Amplification](../concepts/amplification.md).

    `integration steps`
    : Number of steps along a ray's path through the medium used to integrate the gain. More steps
      give a more accurate result; fewer are faster. Must be at least 1. Default: 20.

    `grid` (n_x × n_y × n_z)
    : Number of cells in the discretized gain field, along x, y (transversal) and z (longitudinal).
      Finer grids resolve spatial structure in the pump profile more accurately.
      All three must be at least 1. Default: 10 × 10 × 10.

    **Pump source**

    The pump source defines the spatial distribution of the small-signal gain coefficient g₀ across
    the medium's volume.

    `None`
    : No pumping. The gain field is zero everywhere; the component behaves passively.

    `Const`
    : A spatially uniform g₀.

        `peak gain coefficient` (g₀)
        : The small-signal gain coefficient, uniform across the volume. Must be finite. Negative
          values model an absorbing medium. Unit: m⁻¹ (or cm⁻¹).

    `Analytic`
    : A spatially profiled g₀, composed of independent transversal and longitudinal factors. The
      coefficient at each point is g₀ × T(x, y) × L(z), where T and L are each peak-normalized
      to 1 at the profile center or inlet.

        `peak gain coefficient` (g₀)
        : The peak value of the profile, at the center of the transversal profile and the inlet of
          the longitudinal one. Same rules as for `Const`.

        Transversal profile — across the aperture (x–y plane):

        `Flat`
        : Uniform across the aperture; T(x, y) = 1 everywhere.

        `Super-Gaussian`
        : A generalized Gaussian, peak-normalized to 1 at the center.

            `center` (x, y)
            : Center of the profile in the component's local frame. Unit: length. Default: (0, 0).

            `sigma` (x, y)
            : 1/e² half-widths along x and y. Must be positive.

            `power`
            : Gaussian order (≥ 1). 1 gives an ordinary Gaussian; higher values give a flatter
              top with steeper edges (top-hat limit).

            `theta`
            : Rotation of the ellipse in the transversal plane.

            `rectangular`
            : If enabled, the profile is the product T_x(x) × T_y(y) instead of an elliptical
              function of the radial distance — useful for rectangular pump beams.

        Longitudinal profile — along the propagation direction (z):

        `Flat`
        : Uniform along z; L(z) = 1 everywhere.

        `Beer-Lambert`
        : Exponential absorption of the pump: L(z) = exp(−α × z), where z is measured from the
          surface the pump enters.

            `absorption coefficient` (α)
            : Absorption coefficient of the pump light in the medium. Must be positive and finite.
              Unit: m⁻¹ (or cm⁻¹).

            `direction`
            : `Forward` — pump enters from the input surface (z = 0 there). `Backward` — pump
              enters from the output surface (z = 0 at the rear face).

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

`Small Signal Gain` is evaluated in ray tracing and ghost focus analysis only:

Energy Analysis
: **Not supported.** The gain depends on the path each ray takes through the medium; an energy
  flow analysis carries no spatial or geometric information about individual rays and cannot
  determine it. A model that assigns Small Signal Gain to a component returns an error when an
  Energy Analysis is run on it.

Ray Tracing Analysis
: The gain factor for each ray is computed by integrating g₀ along the ray's actual path through
  the medium. Rays that do not intersect the medium's volume are unaffected.

Ghost Focus Analysis
: As in ray tracing, computed independently on every pass — including reflected ghost paths. A
  ghost that traverses a pumped component picks up gain on each crossing.
