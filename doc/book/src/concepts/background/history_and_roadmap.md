# Project history and initial roadmap

To achieve the project goals, the following activities were initially planned:

1. Survey of the optical aspects relevant to different research groups when designing laser systems. This was intended to provide the basis for the design of a data model structure and ensure that the relevant properties of optical systems could be represented.
2. Survey of existing optical simulation tools used by different institutes. Publicly available software packages were intended to be collected at a central location.
3. Analysis of existing tools and identification of possible interoperability approaches.
4. Development of a general data structure for modelling optical systems based on the results of the previous investigations.
5. Implementation of a framework based on this data structure.
6. Implementation of adapters for existing tools.
7. Development and implementation of simple modules covering common aspects of optical systems, such as geometric optics.
8. Development of a graphical user interface (GUI).

The main focus of the project was initially placed on points 4, 5, and 6, which represented the core development activities.

The implementation of the framework was planned as a step-by-step approach:

1. Implementation of basic data structures, such as `OpticScenery`, `OpticNode`, and related components.
2. Implementation of basic ideal optical nodes, including a source port, detector, propagation element, ideal beam splitter, and ideal filter.
3. Development of an initial simple analysis capability for energy transmission through a tree-like system. This was intended to validate the data structures and general design.