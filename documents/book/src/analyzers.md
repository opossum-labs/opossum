# Analyzers

An analyzer is a module that "orchestrates" an optics simulation. An analyzer defines, how the model will be treated. There are basically two types of analyzers: Sequential and Non-Sequential. A sequential analyzer traverses the graph in a defined manner and triggers the corresponding calculation defined inside the nodes. In contrast, the non-sequential analyzer does not actually make use of the graph structure (i.e. the relations between the nodes) but only uses the nodes and their corresponding attributes such as 3D coordinates or mechanical model data.

**Note**: The use of different analyzers might lead to ["contradicting" models](https://git.gsi.de/phelix/rust/opossum/-/issues/9). For example, one can model a free-space propagation node between two other elements (e.g. two lenses). It can thus define a given geometric length as an attribute. On the other hand, the two lenses might have 3D coordinates with a different distance to each other than defined in the propagation node. In this case, the sequential analysis would simulate another model situation as the non-sequential analyzer...

## Sequential Analyzer

A sequential analyzer uses the node relations defined by the edges of the model graph. It would traverse the graph from one or more sources to one or more detector nodes. While traversing, it calls the respective analysis functions of each node. The input data is taken from previously calculated light information stored in the input edges of a node. The node's analysis result will be stored on the output edges.

The analyzer is also responsible for deciding which nodes in a graph must be calculated at all. Sometimes the user is only interested in a part of the optical network and in this case, often not all nodes need to be calculated at all. Furthermore, modifications of the model often do not need a complete recalculation of the graph but use results from earlier simulation runs. Finally, the analyzer could also decide, which nodes do not directly depend on each other. In this case, nodes can be calculated in parallel thus saving time on multi-core CPU computers.

There might be different analyzers available such as:

- geometric analysis (Matrix optics)
- Gauss mode propagation (also matrix based, see LaserCalc)
- wavefront propagation (Fourier optics)
- simple calculation of energy / power
- ghost focus / back reflection analysis
- ...

For each analysis, the corresponding node attributes must be set. For example, a light source node has to define a set of rays with a given position and angle if a geometric analysis is performed. For wavefront propagation, a complex intensity / wavefront matrix must be defined.

**Note**: Also in this case the model might contain inconsistent values such as a wavefront that does not fit the set of rays defined. [How do we deal with this?](https://git.gsi.de/phelix/rust/opossum/-/issues/9)

## Non-sequential Analyzer

A non-sequential analyzer does not make use of the edges for traversing the node network. This is the case for performing a free 3D raytracing analysis of an optical setup located in 3D space. Hence, for using this analyzer all nodes must have information of their location and orientation set by the respective attributes. Otherwise, these nodes are simply skipped in the simulation. One or more light source nodes are used to cast light rays into the scene. Detectors on the other hand can collect all incoming rays for further analysis. For ray casting, each node has to have a 3D mechanical representation and a (maybe default) surface definition.
