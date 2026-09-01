# Absorption

Light does not cross a real medium unchanged: some of it is absorbed on the way through. This
section explains how OPOSSUM represents that loss and why it is bound to the material rather than to
the analysis. For the individual models and their parameters see
[Materials](../reference/materials.md); absorption is the passive counterpart of
[Amplification](./amplification.md), and the two are designed to work together.

## Absorption belongs to the material

Like amplification, absorption happens inside a component that encloses a volume of material —
currently a lens, a wedge or a cylindric lens — because that is where light spends a path length
inside a medium that can take energy from it.

Unlike amplification, absorption is *not* part of the operating point. How strongly a medium absorbs
is a fixed property of what it is made of: a slab of glass absorbs the same whether the system is run
at full or at half power. Absorption therefore lives on the component's **host material** and travels
with the model, exactly like the refractive index does. There is nothing to switch on in an analysis
and no scenario to name — a material simply carries an absorption model, or it does not.

The consequence is that absorption is **automatic and opt-in through the material**: whenever a
component's material defines an absorption model, rays crossing that component are attenuated
accordingly. A material with no absorption model is perfectly transparent, which is the default, so
adding absorption to OPOSSUM changed no existing model's result.

## Along the real path through the medium

Absorption is applied over the ray's actual geometric path length through the body, the same chord
the gain integration uses. An off-axis or angled ray travels a longer way through the medium and is
attenuated more, without any on-axis approximation. Most models follow the Beer–Lambert law
T(λ, L) = exp(−α(λ)·L), where α is the material's absorption coefficient at the ray's wavelength and
L is the chord through the medium. The transmittance depends on the ray's own wavelength, so a
polychromatic beam is attenuated line by line.

As with gain, the loss applies **once per traversal of the medium**, not once per analysis. A
multipass geometry, or a stray reflection running back through the same component in a
[ghost focus analysis](../reference/analyzers.md), is attenuated on every pass.

## Absorption and gain together

A doped gain medium is the case where both effects act at once, and they come from different places:
the **gain** comes from the dopant and is set by the [pump scenario](../reference/pump_scenarios.md),
while the **absorption** is a property of the surrounding **host material**. In a real amplifier head
the doped glass still absorbs at the signal wavelength; representing that faithfully means letting
the two coexist.

OPOSSUM applies them together on a single traversal of the medium. Both act multiplicatively on the
ray energy and commute, so the net factor over one pass is simply their product — the gain
exp(∫ g₀ × β ds) from the dopant times the transmittance T(λ, L) of the host material.

A component may carry either effect, both, or neither. With neither it is the passive optic it always
was; with only a host-material absorption it attenuates but does not amplify; with a gain model named
by the scenario *and* an absorbing host material it does both at once.

## Where absorption does not yet apply

Absorption is currently evaluated during **ray tracing**, where every ray carries a wavelength and a
path length. An [energy flow analysis](../reference/analyzers.md) carries neither, so — exactly like
the small signal gain model — a path-length-dependent absorption cannot be evaluated there yet. That
step is left for a later stage, once a nominal path length is defined for the energy analysis.
