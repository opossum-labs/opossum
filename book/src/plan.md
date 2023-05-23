# Project plan

For achieving the project goals, the following activities are planned:

1. Survey of the optical aspects the different research groups are interested in while designing laser systems. This would later be the base for the design of a data model structure and should ensure that most (if not all) of the system properties can be mapped.
2. Survey of existing tools used in the different institutes. The software packages might be collected at a central place (if publically available).
3. Analysis of the existing tools and the identification of interoperation possibilities
4. Development of a general data structure for modelling optical systems based on the result of the first point.
5. Implementation of a framework using this data structure
6. Implementation of adapters to the already existing tools
7. Direct development / implementation of (simple) modules fullfilling common aspects of optical systems (such as geometric optical)
8. Development of GUI

Up to now, the points 4, 5 & 6 will be the main activity within the project and will make up the majority of the work to be done.

The implementation of the framework can also be separated in a step-by-step approach:

1. Implementation of basic data structures (OpticScenery, OpticNode, etc..)
2. Implementation of very basic ideal nodes (Maybe only "source", "detector", "propagation", "ideal beam splitter", "ideal filter")
3. Above simple nodes would allow a first very simple analysis: energy transmission through a tree-like system. This would allow for first checks of the data structures and general design.