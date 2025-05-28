# Analyzers

An analyzer is a module that "orchestrates" an optics simulation. An analyzer defines, how the model will be treated. There are basically two types of analyzers: Sequential and Non-Sequential. A sequential analyzer traverses the graph in a defined manner and triggers the corresponding calculation defined inside the nodes. In contrast, the non-sequential analyzer does not actually make use of the graph structure (i.e. the relations between the nodes) but only uses the nodes and their corresponding attributes such as 3D coordinates or mechanical model data.

**Note**: The use of different analyzers might lead to ["contradicting" models](https://git.gsi.de/phelix/rust/opossum/-/issues/9). For example, one can model a free-space propagation node between two other elements (e.g. two lenses). It can thus define a given geometric length as an attribute. On the other hand, the two lenses might have 3D coordinates with a different distance to each other than defined in the propagation node. In this case, the sequential analysis would simulate another model situation as the non-sequential analyzer...

## Sequential Analyzers

A sequential analyzer uses the node relations defined by the edges of the model graph. It would traverse the graph starting from one or more sources to one or more sink nodes. While traversing, it calls the respective analysis functions of each node. The input data is taken from previously calculated light information stored in the input edges of a node. The node's analysis result will be stored on the output edges.

The analyzer type also determines the type of data "flowing" between the nodes. While the for the energy analyzer the data mainly consists of a light spectrum, the ray tracing analyzer uses bundles of geometric rays. These rays itself contain things like position, direction, wavelength etc. Furthermore, each analyzer can have a specific set of configuration parameters that influence the respective analysis algorithm.

The following analyzers are implemented:

- Energy Analysis

    This is the most simple analyzer. It just calculates the energy spectrum while passing through the optical system. Filter
    nodes attenuate the spectrum while beam splitter nodes divide the energy spectrum in two arms according to the splitting 
    config. On the other hand, many other nodes such as lenses or gratings do not influence the data during the pass. Hence, 
    this analyzer gives you rather limited information about an optical system. The advantage on the other side is, that this 
    analyzer is really fast. The energy analzer has no further configuration parameters.
    
- Ray tracing Analysis

- Ghost focus Analysis

    The ghost focus analyzer can be seen as an extended ray tracing analyzer. In fact, the ghost focus analyzer with the `max bounces`
    parameter set to zero (see below) is the basic ray tracing analysis presented above.
