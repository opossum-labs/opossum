# Optic Analyzer Ideas

## Sequential analyzers

The analyzer is also responsible for deciding which nodes in a graph must be calculated at all. Sometimes the user is only interested in a part of the optical network and in this case, often not all nodes need to be calculated at all. Furthermore, modifications of the model often do not need a complete recalculation of the graph but use results from earlier simulation runs. Finally, the analyzer could also decide, which nodes do not directly depend on each other. In this case, nodes can be calculated in parallel thus saving time on multi-core CPU computers.

There might be different analyzers available such as:

- geometric analysis (Matrix optics)
- Gauss mode propagation (also matrix based, see LaserCalc)
- wavefront propagation (Fourier optics)
- simple calculation of energy / power
- ghost focus / back reflection analysis
- ...

## Sequential Analyzer



**Note**: Also in this case the model might contain inconsistent values such as a wavefront that does not fit the set of rays defined. [How do we deal with this?](https://git.gsi.de/phelix/rust/opossum/-/issues/9)

## Non-sequential Analyzer

A non-sequential analyzer does not make use of the edges for traversing the node network. This is the case for performing a free 3D raytracing analysis of an optical setup located in 3D space. Hence, for using this analyzer all nodes must have information of their location and orientation set by the respective attributes. Otherwise, these nodes are simply skipped in the simulation. One or more light source nodes are used to cast light rays into the scene. Detectors on the other hand can collect all incoming rays for further analysis. For ray casting, each node has to have a 3D mechanical representation and a (maybe default) surface definition.
