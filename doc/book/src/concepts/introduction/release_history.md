
# Release History

## Overview Table

| Version | Date       | Key Highlights                                                                 |
|---------|------------|--------------------------------------------------------------------------------|
| 0.7.0   | 2025-12    | Graphical user interface, first version for wider audience                           |
| 0.6.0   | 2024-12-18 | Coatings support, gratings, ghost focus analysis, major refactoring, bug fixes |
| 0.5.0   | 2024-07-26 | Global coordinate system, 3D node alignment, SDF primitives, ambient medium    |
| 0.4.0   | 2024-04-04 | Real lens ray tracing, wavefront analysis, fluence detector, dispersion models |
| 0.3.0   | 2023       | Apertures, JSON/PDF report generation, basic ray tracing                       |
| 0.2.0   | 2023       | Maintenance: bug fixes, unit tests, documentation                              |
| 0.1.0   | 2023       | First technical preview: few optical nodes and a basic energy analyzer         |

---

## Narrative Summaries

### Version 0.7.0 (2025-12)

Version 0.7.0 is a major step toward wider accessibility, introducing a graphical user interface (GUI) that lets users build optical systems visually. Users can place and connect nodes on an interactive canvas, configure node properties via contextual panels, and get immediate visual feedback — simplifying setup, exploration, and iteration for both new and experienced users.

### Version 0.6.0 (2024-12-18)

Version 0.6.0 marked a significant step in OPOSSUM’s maturity. The introduction of coatings, assignable to optical surfaces, allowed for more realistic modeling of physical systems. A new grating node further broadened the range of supported optical components. Analysis capabilities were strengthened with ghost focus reports, now including detailed hit maps and ray-bounce tracking. Visualization was refined, with improvements to spot diagrams, fluence analysis, and ray tracing plots.  
This release also delivered a major refactor of the core codebase, introducing the new `OpticSurface` and `NodeGroup` abstractions, reorganized analyzers, and clearer separation of report structures. Together with performance optimizations for fluence estimation, extensive bug fixes, and new example setups, v0.6.0 represented a robust foundation for future extensions.

---

### Version 0.5.0 (2024-07-26)

The focus of v0.5.0 was on spatial organization and alignment. A global coordinate system was introduced, enabling consistent node positioning, alignment, and 3D placement of sources. Support for modeling the refractive index of ambient media between nodes further increased physical accuracy. Visualization and rendering were enriched with signed distance function (SDF) primitives for simple geometries.  
In addition, documentation was expanded with new examples, including tilted detectors and prism pairs, while numerous bug fixes addressed issues in ray plotting, node positioning, and data export. Internally, refactoring efforts simplified code structures and removed outdated dependencies.  

---

### Version 0.4.0 (2024-04-04)

Version 0.4.0 represented a milestone by adding the first support for ray tracing through real lenses and wavefront analysis. The release introduced a fluence detector node, energy-weighted spot diagrams, and new visualization options, including a ray propagation visualizer. Dispersion models for refractive indices were added, and lenses could now be modeled with both spherical and flat surfaces.  
Beyond these headline features, v0.4.0 contained extensive bug fixes and refinements, performance improvements, and large-scale refactoring of distribution strategies, sources, and properties. With enhanced documentation and an expanded test suite, it provided a solid basis for accurate and flexible optical simulations.

---

### Version 0.3.0 (2023)

With v0.3.0, OPOSSUM gained its first comprehensive simulation workflow. Apertures were introduced at input and output ports, supporting circular, rectangular, and Gaussian profiles, with the possibility of stacking apertures for complex shapes. Report generation was implemented, enabling both JSON and PDF analysis outputs. Basic ray tracing functionality was added with the implementation of paraxial surfaces (ideal lenses) and propagation nodes, laying the groundwork for realistic system modeling.  

---

### Version 0.2.0 (2023)

Version 0.2.0 was a maintenance release. It focused on improving stability, fixing bugs, extending unit test coverage, and enhancing documentation. While it did not introduce major new features, it consolidated the project’s foundations after the initial preview.  

---

### Version 0.1.0 (2023)

The very first technical preview of OPOSSUM introduced the core framework. At this early stage, only a small set of optical nodes and a basic energy analyzer were available, with limited functionality. Nevertheless, it demonstrated the potential of the project and established the initial architecture upon which later releases were built.  
