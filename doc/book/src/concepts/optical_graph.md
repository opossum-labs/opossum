# Modelling Optical Systems

In general, optical systems consist of **light sources**, which provide a more or less complex light field (time-invariant or time-dependent), and **optical components**, which modify this light field.

A typical system includes the following elements:

* **Light Sources:** Elements that generate light (e.g., lasers).
* **Optical Components:** Elements that modify the light field (e.g., lenses, mirrors, Faraday isolators).
* **Light Sinks:** Elements that produce a "result" or measurable signal (e.g., beam dumps, targets, or detectors).

Components may consist of sub-components with an unlimited nesting level.

## The "Design Idea" vs. Physical Layout

For a full physical description, it would be sufficient to place the mechanical model of the optical components—along with their optical properties—into a 3D space with specific orientations. This approach is appropriate for tasks such as straylight analysis and is supported by our model.

However, a purely physical layout often misses the **design idea**.

For example, if one places a light source and two lenses in a setup, a physical model simply sees three objects. The *design idea*, however, specifies the intent: the two lenses form a Kepler or Galilei telescope to image an object or act as a beam expander. The light is *intended* to hit the first lens and then the second lens.

Therefore, optical systems are best described as networks or tree-like structures where optical rays or light fields are cast in a specific direction from one component to the next.

## Directed Graphs as Primary Model Structure

To model these networks of optical components, OPOSSUM uses [directed graphs](https://en.wikipedia.org/wiki/Directed_graph). A directed graph consists of **[nodes](nodes.md)** and **[edges](edges.md)**:

* **Nodes:** Represent the optical components.
* **Edges:** Represent the information about the light (energy, wavelength, wavefront, nearfield distribution, etc.) being handed from one node to the next.

### Ports and Connections

A node has one or more **ports** where edges can be connected. We strictly distinguish between incoming and outgoing ports:

* **Light Source:** A node with no input ports.
* **Detector:** A node with no output ports.
* **Ideal Lens:** Typically has one input and one output port.
* **Beam Splitter:** Typically has one input port and two or more output ports.

*Note: Realistic components may have additional ports, for example, to simulate ghost reflections from lens surfaces.*

### Node Types and Groups

There are different node types representing various optical components (ideal/real lenses, beam splitters, waveplates, etc.). Each node has attributes describing its parameters, such as center thickness (for glass plates), focal length (ideal lenses), or radii of curvature (real lenses).

Additionally, **Group Nodes** represent a set of other nodes arranged in a subgraph. Non-connected ports within the group form the "externally visible" ports of the group node. This allows for the creation of hierarchical, nested structures.

## Loops for Modelling Resonators

Directed graphs can model optical resonators by forming loops. While this works well for ring resonators, it can create ambiguities for linear resonators.

Consider a simple linear cavity consisting of: `Mirror 1 -> Propagation -> Mirror 2`.

Creating a simple "reverse" edge from *Mirror 2* back to *Mirror 1* would technically form three loops:

1. The intended large loop (Mirror 1 to Mirror 2 and back).
2. A small loop between Mirror 1 and Propagation.
3. A small loop between Mirror 2 and Propagation.

This structure is physically nonsensical and becomes unmanageable with complex resonators containing additional components (lenses, amplifier rods, etc.).

### Reference Nodes

To solve this, OPOSSUM uses **Reference Nodes**. A reference node contains only a reference to another existing node and behaves exactly like the node it references. This allows a linear resonator to be "unrolled" in the graph, effectively translating it into a ring resonator structure without topological ambiguity.

> **Note:** Strictly speaking, light in a ring resonator can propagate in both directions (unless suppressed by optical components). Since we use a directed graph, only one direction can be modelled at a time. Solutions for bidirectional propagation are currently being [investigated](https://git.gsi.de/phelix/rust/opossum/-/issues/2).

> **Note:** A reference node that refers to a group must not be placed inside that same group, or inside any group nested within it. This would make the group depend on its own reference, which OPOSSUM rejects when creating, moving, cutting, or pasting the reference node.
