# Materials

A material describes what an optical component is made of. It bundles an **asset header** (UUID, version number, name, optional manufacturer, and description) with a set of **optical properties** — a refractive index model and an absorption model — as well as optional thermal and mechanical data. Materials are stored independently of any node so the same definition can be reused across components and models. A component that encloses a volume (a lens, a wedge, or a cylindric lens) refers to a material for both its refraction and, when defined, its absorption.

For details on catalog management, Git synchronization, and conflict resolution workflows, see [Material Catalogue & Asset Management](../concepts/material_catalogue.md). For the physical background on how absorption combines with gain, see [Absorption](../concepts/absorption.md).

## Catalog Storage, Identity, and Versioning

OPOSSUM organizes materials across a **dual-catalog registry** split into two independent repositories on disk:

* **Official Catalog (`official/`):** Contains curated standard optical materials synchronized from the upstream Git repository (`origin/main`). Official materials are strictly read-only — they cannot be overwritten or deleted in place.  
* **User Catalog (`user/`):** Stores custom materials, forked official materials, and materials imported from `.opm` documents, tracked locally on its own `main` branch.

### UUID and Version Rules

* **Unique Identity (`UUID`):** Every material is identified by a version-4 UUID. An entry in the `user` catalog is never permitted to share a UUID with an entry in the `official` catalog.  
* **Forking Official Materials:** Editing an `Official` material creates a copy in the `user` catalog with a **new UUID** and resets the working draft to `v0`, publishing as `v1` in `user/` while leaving the `Official` entry untouched.  
* **Incremental User Versioning:** Editing an existing `User` material preserves its **existing UUID** and publishes the changes as the next sequential version (`v2`, `v3`, …) without overwriting earlier versions on disk.  
* **Deletion:** Only versions in the `user` catalog can be deleted; deletion removes the latest version (`vN`) and reverts the active catalog entry to `vN-1` (or removes the material completely if `v1` was deleted).

### Storage in Optical Models (`.opm`) and Synchronization

Inside an `.opm` document, volume components embed their assigned material inline (`AssetRef::Inline`) together with its catalog UUID and version number (`v0` for local uncataloged ad-hoc drafts, v \>= 1 for cataloged versions). This keeps .opm files self-contained across systems while enabling **Asset Catalog Synchronization (`Sync with Model`)** to compare model materials against both `official/` and user/ indices, update outdated model nodes, import unlisted materials into user/, or resolve user catalog conflicts (`Overwrite`, `Save as New`, or `Cancel`).

## Absorption models

A material's optical properties carry one absorption model. It is applied during ray tracing over the geometric path length L a ray travels through the component's medium, at the ray's own wavelength λ, and yields a transmittance T ∈ \[0, 1\] that scales the ray's energy. The default is None (perfectly transparent), so a material with no absorption defined leaves rays unchanged.

| Model | Parameter(s) | Transmittance T(λ, L) |
| :---- | :---- | :---- |
| **None** | — | `1` (transparent; the default) |
| **Constant attenuation** | a flat factor `f ∈ [0, 1]` | `f`, independent of wavelength and path length |
| **Lambert–Beer, constant** | absorption coefficient `α` (in 1/m) | `exp(−α·L)` |
| **Lambert–Beer, spectrum** | `α(λ)` as a spectrum (values in 1/m) | `exp(−α(λ)·L)` |
| **Catalog transmittance** | tabulated internal transmittance `τ(λ)` at a reference thickness `d_ref` | `τ(λ)^(L / d_ref)` |
| **Extinction coefficient** | `k`, the imaginary part of the complex index `n + ik` | `exp(−α·L)` with `α = 4π·k / λ` |

Notes on the individual models:

* **Constant attenuation** is a path-length-independent factor — a flat fraction of energy removed on transit. It is a bookkeeping model, not a bulk absorption, and does not scale with how far the ray travels through the medium.  
* **Lambert–Beer, constant / spectrum** are the physical bulk-absorption models. The constant form uses one coefficient for all wavelengths; the spectrum form looks the coefficient up per wavelength, so different lines of a polychromatic beam are attenuated differently.  
* **Catalog transmittance** takes an internal transmittance quoted by a glass catalog at a reference thickness and scales it to the ray's actual path length by the power law above.  
* **Extinction coefficient** derives the Beer–Lambert coefficient from the imaginary part of the complex refractive index, `α = 4π·k / λ`.

A wavelength or path length outside a model's valid range (for instance a wavelength not covered by a spectrum) is reported as an error rather than silently extrapolated.
