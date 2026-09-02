# Amplification

Laser systems do not only transport light, they make more of it. This section explains how an
amplifying component is represented in OPOSSUM and why it is split across two places instead of
one. For the settings themselves see [Pump scenarios](../reference/pump_scenarios.md); for the
steps to set one up, see [Model an amplifier](../howto%20guides/model_an_amplifier.md). Its passive
counterpart — energy a real medium takes *out* of the beam — is covered in
[Absorption](./absorption.md), and the two are designed to act together.

## There is no amplifier node

An amplifier is not a node type of its own. Amplification happens inside a component that encloses
a volume of material — currently only within a lens, a wedge or a cylindric lens — because that is where light spends a
path length inside a medium that can hand energy to it.

This follows from what an amplifier head physically is. The same slab of doped glass is a passive
window when it is not pumped and an amplifier when it is; nothing about the component changes
between the two. Giving amplification its own node type would mean the user has to decide which one
a component is *while drawing the model*, and would force a different model for every operating
condition of the same hardware.

## Hardware and operating point

The two halves of an amplifier are therefore kept apart:

**The hardware** — the geometry of the component and the material it is made of — belongs to the
node, is drawn on the canvas and travels with the model.

**The operating point** — how hard a component is driven during one particular analysis — belongs
to a *pump scenario*, which is a property of the document rather than of any node. A scenario is
named (e.g., "full power", "half power", "cold alignment") and states, for every amplifying component,
which gain model applies to it.

The gain is what an analysis is *run at*, not what a component *is*.

The practical consequence is that one model can be analyzed at several operating points without
being edited in between. An analyzer is given a list of scenarios and produces one report per
scenario, so the same chain at full and at half pump power is a single run and two reports that can
be compared directly. A component no scenario mentions behaves exactly like the passive optic it is,
which is why adding amplification to OPOSSUM changed no existing model's result.

## The "ideal" amplifier

The simplest gain model available today is a constant one: it multiplies the energy passing through
the medium by a fixed factor. That factor is independent of wavelength, independent of the path
length the light actually takes through the medium, and independent of how much energy has already
been extracted — the medium has no state that a pass could deplete.

This is deliberately a bookkeeping model, not a description of an amplifier. It is the right tool
for laying out a chain and seeing where the energy of a system ends up, and the wrong tool for
designing a single amplifier head. Saturation, in particular, is absent: an ideal amplifier will
happily turn a joule into a kilojoule if the factor says so.

## Every pass through the medium counts

The factor applies once per traversal of the medium, not once per analysis. Sending light through
the same head several times — a multipass amplifier, modeled with
[reference nodes](../reference/nodes/reference_node.md) — applies it once per pass.

The same is true in a [ghost focus analysis](../reference/analyzers.md). A stray reflection that
runs back through a pumped head picks up the gain on that pass just like the main beam. This matters
more than it may sound: a ghost is dangerous because of the fluence it carries onto a surface, and a
ghost that has crossed an amplifier more often than the main beam carries correspondingly more of
it. Counting those passes as unamplified would understate exactly the hazard the analysis exists to
find.

## The monochromatic small signal gain model

The constant gain model is deliberately unphysical — it is a bookkeeping tool. The monochromatic
small signal gain model is the first step towards a physical description of a laser amplifier.

**Inversion field.** A pumped medium has more atoms in the upper laser level than the lower one.
That population inversion is what a passing photon can stimulate: the local gain per unit path
length is g(r) = g₀ × β(r), where g₀ is the peak gain coefficient the model carries and
β(r) ∈ [0, 1] is the normalized pump shape — how strongly pumped that point is relative to the
peak. For a spatially non-uniform pump, OPOSSUM discretizes β onto a three-dimensional grid so
that two parallel rays at different radial positions pick up different gain.

**Two phases.** Before any ray is traced, OPOSSUM builds the inversion shape from the pump
source. During ray tracing, each ray accumulates a gain factor exp(∫ g₀ × β ds) along its actual
path through the medium — a voxel-exact integration. The path is the ray's real geometric path,
not an on-axis approximation, so off-axis and angled rays see the correct medium thickness
automatically.

**Pump profiles.** The pump source defines only the *shape* β — the peak gain coefficient g₀ is
stated once on the model. A constant pump is the shapeless case: β = 1 throughout the medium, no
grid needed, and the model integrates over the exact chord each ray travels. An analytic pump
composes a transversal profile (flat or super-Gaussian across the aperture) with a longitudinal
profile (flat or Lambert-Beer along the propagation axis, for end-pumped or side-pumped
geometries), resolved onto a grid stated on the pump itself.

**Monochromatic.** The gain coefficient g₀ is wavelength-independent — all wavelengths in a
polychromatic beam receive the same gain. A wavelength-dependent gain (gain bandwidth, lineshape)
is a separate model that builds on this one.

**Why energy analysis is excluded.** The gain a ray accumulates depends on the path it takes —
an off-axis ray through the edge of a Gaussian gain profile picks up less than an on-axis ray.
An energy flow analysis carries no spatial or geometric information about rays; it cannot determine
this, and silently averaging over it would give the wrong answer. Energy flow analysis therefore
refuses a model that includes a small signal gain component, rather than silently returning an
incorrect result.

**Still ideal.** This model is called *small signal* because it assumes the signal is too weak to
change the inversion. The gain field is built from the pump alone and is not modified as rays pass
through. That makes multi-pass counting exact — a ray that traverses the medium three times
accumulates three times the single-pass gain — but it overstates the gain for strong pulses, where
the first pass depletes the medium for subsequent ones.

## What comes next

The monochromatic small signal model has two idealisations: the signal is too weak to deplete the
medium, and the gain does not depend on wavelength. Later models lift one at a time.

Spectral gain first: a real gain medium amplifies some wavelengths more than others. 
This is manifested in the spectroscopic properties of a material. This behaviour can already be implemented for the small signal gain model

Saturation second: a strong pulse changes the inversion as it passes through. The first part of the
pulse sees more gain than the last, because the population inversion the front extracted is no
longer available to the tail. The Frantz–Nodvik model captures this depletion — the gain is then a
function of the fluence the pulse has already delivered, not just of the local inversion.

