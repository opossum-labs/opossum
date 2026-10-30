# Optic Analyzer Ideas

## Sequential analyzers

The analyzer is also responsible for deciding which nodes in a graph must be calculated at all. Sometimes the user is only interested in a part of the optical network and in this case, often not all nodes need to be calculated at all. Furthermore, modifications of the model often do not need a complete recalculation of the graph but use results from earlier simulation runs. Finally, the analyzer could also decide, which nodes do not directly depend on each other. In this case, nodes can be calculated in parallel thus saving time on multi-core CPU computers.

**Note**:Even in this case, the model might contain inconsistent values, for example, when the defined wavefront is not consistent with the defined rays.

Different analyzers are available in Opossum, such as:

- Ray tracing analyzer
- Energy analyzer
- Ghost focus / back reflection analyzer

### Ray Tracing

The Ray Tracing Analyzer provides configuration options for controlling ray propagation through the optical system. These include the maximum number of refractions, the maximum number of bounces and the minimum ray energy. Additionally, a mixed surface strategy and an ambient material can be configured.

### Energy Analysis

The Energy Analyzer provides configuration options for defining the energy source port configuration, the energy type and the wavelength used for the analysis.

### Ghost Focus Analysis

The Ghost Focus Analyzer provides configuration options for the maximum number of bounces, the fluence estimator and the ambient materials used for the analysis.

## Non-sequential Analyzer

A non-sequential analyzer does not make use of the edges for traversing the node network. This is the case for performing a free 3D raytracing analysis of an optical setup located in 3D space. Hence, for using this analyzer all nodes must have information of their location and orientation set by the respective attributes. Otherwise, these nodes are simply skipped in the simulation. One or more light source nodes are used to cast light rays into the scene. Detectors on the other hand can collect all incoming rays for further analysis. For ray casting, each node has to have a 3D mechanical representation and a (maybe default) surface definition.
