# Amplification

Laser systems do not only transport light, they make more of it. This section explains how an
amplifying component is represented in OPOSSUM and why it is split across two places instead of
one. For the settings themselves see [Pump scenarios](../reference/pump_scenarios.md); for the
steps to set one up, see [Model an amplifier](../howto%20guides/model_an_amplifier.md).

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

## What comes next

Later gain models will read the state of the pumped medium rather than a fixed number. Once they do,
the path length through the material starts to matter, and after that the energy already extracted
does too — a strong pulse then depletes the medium for what follows it. How a component is *pumped*
becomes a setting of its own alongside the gain model at that point. The split described above is
what makes room for this: those models are further entries in the same place, not a different way of
building the model.
