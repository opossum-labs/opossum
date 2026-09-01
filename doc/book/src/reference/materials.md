# Materials

A material describes what an optical component is made of. It bundles a set of **optical properties**
— a refractive index model and an absorption model — with optional thermal and mechanical data, and
it is stored independently of any node so the same material can be reused across components and
models. A component that encloses a volume (a lens, a wedge or a cylindric lens) refers to a material
for both its refraction and, when defined, its absorption.

This page documents the **absorption models**. For the physical picture — why absorption is a
property of the material rather than of an analysis, and how it combines with gain — see
[Absorption](../concepts/absorption.md).

## Absorption models

A material's optical properties carry one absorption model. It is applied during ray tracing over the
geometric path length `L` a ray travels through the component's medium, at the ray's own wavelength
`λ`, and yields a transmittance `T ∈ [0, 1]` that scales the ray's energy. The default is `None`
(perfectly transparent), so a material with no absorption defined leaves rays unchanged.

| Model | Parameter(s) | Transmittance `T(λ, L)` |
|---|---|---|
| **None** | — | `1` (transparent; the default) |
| **Constant attenuation** | a flat factor `f ∈ [0, 1]` | `f`, independent of wavelength and path length |
| **Lambert–Beer, constant** | absorption coefficient `α` (in 1/m) | `exp(−α·L)` |
| **Lambert–Beer, spectrum** | `α(λ)` as a spectrum (values in 1/m) | `exp(−α(λ)·L)` |
| **Catalog transmittance** | tabulated internal transmittance `τ(λ)` at a reference thickness `d_ref` | `τ(λ)^(L / d_ref)` |
| **Extinction coefficient** | `k`, the imaginary part of the complex index `n + ik` | `exp(−α·L)` with `α = 4π·k / λ` |

Notes on the individual models:

- **Constant attenuation** is a path-length-independent factor — a flat fraction of energy removed
  on transit. It is a bookkeeping model, not a bulk absorption, and does not scale with how far the
  ray travels through the medium.
- **Lambert–Beer, constant / spectrum** are the physical bulk-absorption models. The constant form
  uses one coefficient for all wavelengths; the spectrum form looks the coefficient up per
  wavelength, so different lines of a polychromatic beam are attenuated differently.
- **Catalog transmittance** takes an internal transmittance quoted by a glass catalog at a reference
  thickness and scales it to the ray's actual path length by the power law above.
- **Extinction coefficient** derives the Beer–Lambert coefficient from the imaginary part of the
  complex refractive index, `α = 4π·k / λ`.

A wavelength or path length outside a model's valid range (for instance a wavelength not covered by a
spectrum) is reported as an error rather than silently extrapolated.
