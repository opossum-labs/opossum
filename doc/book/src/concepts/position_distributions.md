# Position Distribution Design

In OPOSSUM, position distributions define how rays are initially placed at the source before they enter an optical system. They describe the spatial structure of the beam at the start of the simulation, which is important because all subsequent ray tracing results depend on this initial ray pattern.

In other words, the position distribution determines where rays start in space, and this directly influences the appearance of the spot diagram in the ray tracing analysis report.

## General Concepts

Physically, a real light source emits energy continuously over an area. In ray-tracing simulations, a continuous optical beam is not modeled directly. Instead, it is approximated using a finite number of rays. The position distribution answers the question:

“How do we place discrete rays so they accurately represent a continuous optical beam?”

The choice of distribution affects how many rays are required to obtain stable results, how uniform the spot diagram appears, and whether clusters, gaps, or regular geometric patterns become visible in the sampled beam.

The spot diagram therefore provides a convenient way to compare the sampling behavior of different position distributions and to evaluate the effect of increasing the number of rays.

### spot Diagrams

The spot diagram in the ray tracing analysis report is the main tool to visualize the effect of a position distribution. Because every ray starts from a defined spatial pattern, the final spot diagram reflects:

1. How evenly the rays were originally spaced  
2. Whether rays appear in clusters or structured patterns  
3. Whether the sampling is random or quasi-uniform  

At low ray counts, the spot diagram may exhibit clusters, gaps, or visible sampling structures that do not accurately represent the continuous beam. As the number of rays increases, the diagram becomes smoother and more representative of the physical source, although differences between distributions remain visible.

## Position Distributions

All distributions in OPOSSUM share these properties:

1. Rays originate from a source centered at the origin  
2. The source can be rotated or translated using an isometry  
3. You specify the number of rays and the source area  
4. The average spacing between rays decreases as the number of rays increases (for a fixed source area), producing smoother spot diagrams  

### Random Distribution

The random distribution places rays without any spatial order. Each ray position is generated independently.

In the spot diagram, this appears as an irregular cloud of points. At low ray counts, strong clustering and empty regions are visible. These patterns are not physical effects of the optical system but purely a result of stochastic sampling.

As the number of rays increases, the overall coverage improves, but local clustering and density variations remain. Random distributions therefore typically require more rays than quasi-random distributions to achieve similarly smooth results.

### Fibonacci Distribution

The Fibonacci distribution places rays using a quasi-uniform sequence based on the golden ratio.

In the spot diagram, this produces a smooth and evenly filled pattern even at relatively low ray counts. There are no visible rows, rings, or clusters, and the points are distributed in a way that avoids repetition.

Compared to random sampling, Fibonacci distributions reduce clustering effects and produce a more stable representation of a continuous beam. The average distance between neighboring points remains nearly constant, resulting in highly uniform spatial coverage.

### Sobol Distribution

The Sobol distribution is a low-discrepancy quasi-random sequence designed for uniform spatial coverage.

In the spot diagram, it appears more evenly distributed than random sampling, even at low ray counts. Large empty regions and strong clustering are avoided.

As the number of rays increases, the source area is filled very evenly. Because clustering is minimized, Sobol distributions often achieve smooth and stable results using fewer rays than purely random sampling.

### Hexagonal Tiling

Hexagonal tiling arranges rays in a regular honeycomb structure.

In the spot diagram, this appears as a tightly packed pattern with uniform spacing. At low ray counts, the hexagonal geometry is clearly visible. As ray count increases, the pattern becomes denser and gradually appears more uniform.

This distribution provides very even spatial coverage and nearly constant spacing between neighboring rays, making it suitable for simulations where highly uniform sampling is required.

### Hexapolar Distribution

Hexapolar distribution arranges rays in a radial structure centered on the source origin.

In the spot diagram, this results in concentric rings combined with angular symmetry. At low ray counts, the radial structure is clearly visible as spoke-like patterns. As ray count increases, the rings fill in and form a circularly symmetric distribution.

This makes hexapolar particularly suitable for systems with circular apertures or strong rotational symmetry. The radial organization often aligns naturally with the geometry of rotationally symmetric optical systems.

### Grid Distribution

The grid distribution places rays on a regular rectangular lattice in the source plane. Rays are arranged in evenly spaced rows and columns, forming a structured Cartesian sampling pattern.

In the spot diagram, this appears as a clear grid-like pattern, especially at low ray counts. The structure remains visible even as the number of rays increases, although it becomes finer and denser.

Unlike quasi-random or random sampling methods, the grid distribution introduces a strong geometric structure into the sampling. This makes the spatial arrangement highly predictable, but it can also introduce artificial alignment effects if the optical system interacts with the grid symmetry. Because neighboring rays are equally spaced along the horizontal and vertical directions, the sampling pattern is highly predictable and reproducible.

## Key Takeaways

All position distributions represent the same physical concept: approximating a continuous optical source using a finite number of rays.

The primary difference between distributions is how rays are positioned within the source area. Random distributions may contain clusters and gaps, while Sobol and Fibonacci distributions provide more uniform coverage. Hexagonal and Grid distributions introduce regular geometric structures, whereas Hexapolar distributions preserve rotational symmetry.

These differences influence the appearance of the spot diagram and determine how many rays are required to obtain smooth and stable ray-tracing results.