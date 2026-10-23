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

Every node with a shape of its own also has one property per surface, holding its coating and its laser induced
damage threshold, see [Surfaces](#surfaces).

## Ports

Ports are the connector points between nodes and can be connected by [edges](edges.md). Ports are strictly distinguished as "input_1" and "output_1" ports. Output ports can only be connected to input ports of another node and vice versa while it is forbidden to connect two input or two output ports. In addition, output ports do not need to be connected to other nodes. During analysis, any result will be simply discarded.

Ports have a specific name in order to distinguish them. For example, a beamsplitter cube might have one input port (e.g. named "input_1") and two output ports named "output_1" and "transmitted" for the two outgoing beams.

Nodes with output ports only form the optical sources while nodes containing (usually only one) input source will be called detectors. In most cases, a simulation of the model traverses the graph from all sources to a detector node using all possible paths (see [Analyzers](analyzers.md)).

A port is a connection point of the graph and nothing else: it carries no coating, no damage threshold and no
aperture. What happens to light at a component is a matter of its [surfaces](#surfaces) and its
[optic shape](#optic-shape). Each port sits on one surface of the node, which the node type decides: the input of a
lens on its front surface, its output on its rear surface, both ports of a mirror on its one surface.

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

1. Optic shape

   Each real-world optical component has a limited transversal size, which decides the light it can catch. Nodes state it
   in their `optic shape` property, see [Optic shape](#optic-shape).

1. 3D mechanical model

   For non-sequential analysis (e.g. ray tracing), a 3D geometric model is necessary. This could be provided as static 3D files (OBJ, STL, etc.) or programmatically derived. For example, the model of a spherical lens could be directly calculated.

1. Surface definitions

   Each surface of a component has a coating and a laser induced damage threshold, see [Surfaces](#surfaces). Further
   surface properties such as roughness are not modeled yet.

## Optic shape

Every component that light hits has a transversal size, its `optic shape`: where the material or the coated surface
actually is, the size a supplier quotes, with any bores through it. The optic shape decides which rays hit the
component. A ray that reaches the surface outside of it, or through one of its bores, misses the component. What
happens to that ray depends on the analysis:

- **Ray tracing** follows its `missed surface strategy` (see [Analyzers](analyzers.md)): with `Stop`, the default, the
  ray is lost; with `Ignore` it passes the component unchanged.
- **Ghost focus** analysis always lets it pass unchanged.
- **Energy** analysis traces no rays and ignores optic shapes.

A mirror, a parabolic mirror and a grating only reflect: a ray that misses them has no way on and is lost in every
analysis. A ray outside the optic shape of a beam splitter, or through one of its bores as on a scraper mirror,
follows the strategy above: with `Ignore` it passes on as transmitted light, with `Stop` it is lost.

**Edge and bores.** An optic shape is an edge with bores through it. The edge is a circle, a rectangle or a polygon,
centered on the component's axis and not turned: to shift or turn a component, align its node. A bore is a circle, a
rectangle or a polygon too, shifted off the axis and turned by its own offset and rotation; a ring, for instance, is a
circle with a centered bore. Every bore lies within the edge, and bores neither overlap nor touch each other or the
edge: OPOSSUM refuses a bore that does and says why. Overlapping bores and a notch cut into the edge cannot be stated.
In the node editor, choosing another kind of edge starts a fresh edge of that kind without bores.

The default depends on the kind of component:

| Component | Default optic shape |
|---|---|
| lens, wedge, mirror, parabolic mirror, beam splitter, ideal filter, paraxial surface | circle of 12.5 mm radius, the usual 1 inch mount |
| cylindric lens | rectangle of 20 mm across its curvature (x) and 30 mm along its cylinder axis (y) |
| grating | square of 25 mm × 25 mm |
| detectors | square window of 25 mm × 25 mm |
| dummy | unbounded (`Open`) |

The optic shape lies in the plane perpendicular to the component's axis and extends along that axis: for a rod cut at
Brewster's angle it is the cross section of the rod, as its manufacturer states it. An off-axis parabola measures it
parallel to its parent axis, as catalogs do. A
curved surface has to reach the edge of the optic shape, so its radius of curvature cannot be smaller than the
largest distance of that edge from the axis. The edge thickness of a lens or a wedge is not checked: where its
surfaces would cross, rays miss the rear surface.

**Detectors** let all light pass. Their optic shape is the window they record within, a square of 25 mm × 25 mm by
default. Light outside the window passes on unrecorded, and the log and the report warn about it. Set the window to
the size of the actual sensor, or to `Open` to record all light. The [paraxial surface](nodes/paraxial_surface.md) is
an idealization and may be unbounded as well. Every other component refuses `Open`.

**Iris.** A [dummy](nodes/dummy.md) passes light unchanged and is unbounded (`Open`) by default. Given an optic
shape, it acts as an iris: a ray outside of it misses the dummy and follows the rules above.

Only the optic shape limits light; ports and surfaces carry no aperture. Sources, ray propagation
visualizers and groups are idealized or abstract nodes and have no optic shape. To limit the beam of a source, place
a dummy behind it or shrink its ray distribution.

**Alignment.** The component alignment run (see [Geometry](../concepts/geometry.md)) respects optic shapes too: if
the optical axis misses the optic shape of a component, beside its edge or through one of its bores, the alignment
stops with an error naming that node. A bore meant to let the beam pass along the axis, as on a scraper mirror for
the main beam, is therefore not supported yet. At a detector a missed window only warns that the measurement may fail.

## Surfaces

Every face of a component is a surface of its own, with a `coating` and a laser induced damage threshold (`LIDT`). A
node keeps them as one property per face, named after the face:

| Node | Surface properties |
|---|---|
| lens, cylindric lens, wedge | `front surface`, `rear surface` |
| every other node except groups and references | `surface` |

The coating decides which part of the light a surface reflects and which part it transmits; the LIDT is the fluence
above which the ghost focus analysis reports the surface as at risk. By default a surface has an ideal antireflective
coating (`IdealAR`, no reflection) and an infinite LIDT, so it cannot be damaged. Further coatings are a constant
reflectivity (`ConstantR`), the Fresnel reflection of an uncoated surface (`Fresnel`), and a `Separator`, which
transmits a fixed ratio or a spectrum and reflects the rest, whatever the angle of incidence; on a mirror it acts, for
instance, as a dichroic coating. A thin mirror and a parabolic mirror start with a fully reflecting coating
(`ConstantR` of 100 %), a beam splitter with a `Separator` that transmits half of the light: the coating of its
surface is its split (see [Beam splitter](nodes/beam_splitter.md)).

A surface is one surface, however many ports sit on it. The input and the output of a mirror or a detector, and all
four ports of a beam splitter, share the one surface of the node: its coating, and the hits an analysis records on it.
The ghost focus report names a surface after its face, for instance `front surface`.

## Analysis interface

As discussed, the actual calculation is performed by the nodes. The presented framework will only make sure that all necessary input data will be provided. For this, each node has to implement an Analysis function with light data from the incoming edges as parameters. This function can now either directly perform a calculation or call specific external modules (such as C/C++ library code or a Python script).
