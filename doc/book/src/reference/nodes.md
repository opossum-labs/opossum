# Nodes

Nodes form the building blocks of the optical model and normally represent optical components. Nodes are configured through properties (see below) and can be connected by edges. Furthermore, nodes might contain an "inner state" (which cannot be modified directly). This is interesting (in the future) for simulating effects like depletion or simply accumulating energy on an energy meter during multiple passes etc... In addition, there are two special nodes

- [Group node](./nodes/node_group.md)

    A (sequential) group node contains a nested directed graph of other nodes or even further group nodes.
    This allows for struturing the model by modeling subsystems. Each group defines external ports by mapping
    input and / or output ports of internal nodes.

- [Reference node](./nodes/reference_node.md)

    A reference node refers to an already existing node in the model. Reference nodes are necessary in multi pass setups.
    Another use case is the modeling of linear resonators. A reference to a group must not be placed inside that same
    group or any group nested within it (see [Reference node](./nodes/reference_node.md)).

## Node Properties

All nodes con be configured through node properties. All nodes have a common set of properties as shown below. In addition, nodes can have additional properties specific to the respective node type. An example would be the center thickness of a lens. Properties common to all nodes are:

- `name`

    While not strictly necessary it is strongly recommended to assign a name to a node for easier identification.
    In principle, different nodes can have the same name but this might cause much confusion. Internally the model uses
    unique IDs for each node in order to distinguish them but these IDs are not available to the user.

- `inverted`

  Flag denoting the direction of the passing light. It might be necessary to propagate through a node in a reverse
  direction (e.g. for back reflection / ghost-focus analysis). Hence each node should have a "reverse" function. In the case 
  of a propagation node, this would be identical. For a basic node, it might change the sign of some properties such as the 
  radius of curvature. For group nodes, the underlying order of sub-nodes has to be reversed. The reference node only needs
  a qualifier to denote whether the propagation is reversed or not.

- `global position`

    Global position in 3D space.

- `local position`

    Local position / alignment relative to the `global position`.

- `damage threshold`

## Ports

Ports are the connector points between nodes and can be connected by [edges](edges.md). Ports are strictly distinguished as "input_1" and "output_1" ports. Output ports can only be connected to input ports of another node and vice versa while it is forbidden to connect two input or two output ports. In addition, output ports do not need to be connected to other nodes. During analysis, any result will be simply discarded.

Ports have a specific name in order to distinguish them. For example, a beamsplitter cube might have one input port (e.g. named "input_1") and two output ports named "output_1" and "transmitted" for the two outgoing beams.

Nodes with output ports only form the optical sources while nodes containing (usually only one) input source will be called detectors. In most cases, a simulation of the model traverses the graph from all sources to a detector node using all possible paths (see [Analyzers](analyzers.md)).

## Attributes

Each node can have a set of attributes that represent its specific properties. For example, a propagation node contains a propagation length and a material. An ideal lens contains the focal length as an attribute. Each attribute has a name and a strictly defined data type. Attributes may have default values or are completely optional.

In addition, nodes have a set of attributes that are common to all of them. However, all of these attributes are optional (e.g. can be "empty").

### Common attributes for all nodes

1. Component Database ID (optional)

   If set, this could be a reference to a (local) component database. It should be considered to also include the database information while exporting the model to a file. If the ID is not set, it would be a "manually configured" component.

1. Material

   Each optical element consists of a given material. These are mostly different glass materials but could also be metals (i.e. for mirrors) or other substances. Even for free-space propagation nodes a material must be given. This might often be "air" or "vacuum". Since materials have a plethora of attributes and will be used by different nodes within a model, the material will be a reference pointing to a materials database.

   **Note**: For interoperability, it might not be always a good idea to only have the material properties in a (local) database. If the model data is given to another user, this data might not be found in his (also local) database. Hence it should be possible to attach the actual material data to the model during export. The alternative would be to have a global database...

1. 3D location

   Each node can have information about its location in 3D space determined as XYZ coordinates with respect to a given global origin. The anchor position of the node depends on the specific node type. This attribute together with the orientation information (see next point) can be used for doing a 3D ray-trace analysis or simply for visualization.

1. 3D orientation

   Besides the location (see previous point) an optical component has a given orientation in 3D space. This orientation is defined by angles around the axes of the global coordinate system.

1. Clear aperture

   Each real-world optical component has a limited transversal size, which decides the light it can catch. Nodes state it
   in their `clear aperture` property, see [Clear aperture and port apertures](#clear-aperture-and-port-apertures).

1. 3D mechanical model

   For non-sequential analysis (e.g. ray tracing), a 3D geometric model is necessary. This could be provided as static 3D files (OBJ, STL, etc.) or programmatically derived. For example, the model of a spherical lens could be directly calculated.

1. Surface definitions

   For both, sequential and non-sequential analysis, surface properties such as coating or roughness which define the way light will be reflected or propagated through should be defined. *This could also be modeled using basic nodes*

## Clear aperture and port apertures

Every component that light hits has a transversal size, its `clear aperture`: the size the material or the coated
surface is actually available in, the figure a supplier quotes. The clear aperture decides which rays hit the component.
A ray that reaches the surface outside of it misses the component. What happens to that ray depends on the analysis:

- **Ray tracing** follows its `missed surface strategy` (see [Analyzers](analyzers.md)): with `Stop`, the default, the
  ray is lost; with `Ignore` it passes the component unchanged.
- **Ghost focus** analysis always lets it pass unchanged.
- **Energy** analysis traces no rays and ignores all apertures.

A mirror, a parabolic mirror and a grating only reflect: a ray that misses them has no way on and is lost in every
analysis. A ray outside the clear aperture of a beam splitter, or through a hole in it as on a scraper mirror, follows
the strategy above: with `Ignore` it passes on as transmitted light, with `Stop` it is lost.

**Shapes.** A component needs a shape with an edge: a circle, a rectangle, a polygon, or a stack of such shapes holding
at least one hole, e.g. a ring or a mirror with a central bore. The default is a circle of 12.5 mm radius, the usual
1 inch mount. The clear aperture lies in the plane perpendicular to the component's axis and extends along that axis:
for a rod cut at Brewster's angle it is the cross section of the rod, as its manufacturer states it. An off-axis
parabola measures it parallel to its parent axis, as catalogs do. A curved surface has to reach the edge of the clear
aperture, so its radius of curvature cannot be smaller than the largest distance of that edge from the axis. The edge
thickness of a lens or a wedge is not checked: where its surfaces would cross, rays miss the rear surface.

**Detectors** let all light pass. Their clear aperture is the window they record within, unbounded (`Open`) by
default. Light outside a set window passes on unrecorded, and the log and the report warn about it. The
[paraxial surface](nodes/paraxial_surface.md) is an idealization and may be unbounded as well.

**Port apertures** are an additional mask on the light that passed a surface, e.g. a soft Gaussian apodization. They
never decide whether a ray hits a component: they act only on rays that did. In the `Node Editor` they are found under
each port as `Aperture (additional mask)`.

**Alignment.** The component alignment run (see [Geometry](../concepts/geometry.md)) respects clear apertures too: if
the optical axis misses the clear aperture of a component, the alignment stops with an error naming that node. At a
detector it only warns that the measurement may fail. Port apertures do not act during the alignment run.

**Older model files** are upgraded when they are read. A centered hole at the input port that gave a component its
size becomes its clear aperture, and the port apertures that carried that hole are opened. A clear aperture that a curvature cannot
reach is reduced to fit. Components that had no such hole get the default of 12.5 mm. The log lists what was changed;
saving the model writes the upgraded version.

## Analysis interface

As discussed, the actual calculation is performed by the nodes. The presented framework will only make sure that all necessary input data will be provided. For this, each node has to implement an Analysis function with light data from the incoming edges as parameters. This function can now either directly perform a calculation or call specific external modules (such as C/C++ library code or a Python script).
