# Energy Distribution

In OPOSSUM, the energy distribution defines how energy values are assigned to rays at the source. While the position distribution determines where rays originate, the energy distribution determines how the energy is distributed among those rays.

In a real optical beam, the energy is generally not distributed uniformly across the beam cross section. Depending on the beam profile, different regions of the beam may contain different amounts of energy. The energy distribution models this spatial variation by assigning an energy value to each ray according to its position within the source.

The following energy distributions are currently available:

- `Uniform`
- `Generalized Gaussian`

## General Concepts

The energy distribution describes how energy varies across the source aperture. During source generation, each ray is assigned an energy value according to its position and the selected energy distribution.

The energy distribution is independent of the position distribution. The position distribution determines where rays are placed, whereas the energy distribution determines the energy assigned to those rays.

Different energy distributions therefore produce different spatial energy profiles while using the same ray positions.

## Uniform Distribution

The uniform distribution assigns the same energy value to every ray, regardless of its position within the source.

Since every ray receives the same energy value, the distribution contains no spatial variation in energy across the source. This distribution is suitable when a constant energy profile across the entire source is required.

## Generalized Gaussian Distribution

The generalized Gaussian distribution assigns energy values according to a two-dimensional generalized Gaussian function. The energy assigned to each ray depends on its position within the source and the selected distribution parameters.

Compared to a standard Gaussian distribution, the generalized Gaussian introduces an additional `Power` parameter that controls the shape of the distribution. A `Power` value of `1` corresponds to a standard Gaussian distribution. Increasing the `Power` produces super-Gaussian distributions with progressively flatter central regions and steeper edges.

The generalized Gaussian distribution is defined by the following parameters:

- `μx`, `μy` – define the center of the distribution. Changing these values shifts the center of the distribution along the x and y directions.
- `σx`, `σy` – define the distribution widths in the x and y directions. Larger values produce a wider distribution, while smaller values produce a narrower distribution.
- `Power` – controls the shape of the distribution. A value of `1` produces a standard Gaussian distribution, while larger values produce super-Gaussian distributions.
- `θ` – defines the rotation angle of the distribution. Positive values rotate the distribution counter-clockwise.
- `Shape` – specifies whether the generalized Gaussian is evaluated using an elliptical or a rectangular formulation.

OPOSSUM supports both elliptical and rectangular generalized Gaussian formulations. For a `Power` value of `1`, both formulations correspond to a standard two-dimensional Gaussian distribution. For larger `Power` values, the elliptical and rectangular formulations produce different generalized Gaussian profiles.

Conceptually, the generalized Gaussian distribution first evaluates the position of each ray relative to the center of the distribution. The distribution widths define how rapidly the energy decreases along the x and y directions, the `Power` parameter controls the overall beam profile, and the rotation angle `θ` changes the orientation of the distribution. The resulting value determines the relative energy assigned to each ray.

The generalized Gaussian distribution provides relative energy weights for the rays based on their spatial positions. The final energy carried by each ray depends on the selected source energy and the calculated distribution value. By changing the distribution parameters, different spatial energy profiles can be represented while keeping the same ray positions.