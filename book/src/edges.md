# Edges

Edges connect the nodes in the model graph. Edges contain the "light data" which needs to be exchanged between the nodes if analyzed by a [sequential analyzer](./analyzers.md#sequential-analyzer). The content of an edge depends on the type of analysis performed. For geometric optics, it contains an array of ray vectors (position & angle) together with a wavelength and an intensity. For wavefront propagation,, it might contain a 2D complex array representing the nearfield distribution as well as the local phase.

Possible types of data:

- Energy / Power flow

    For simple calculation of light transmission through an optical network. This would also be a starting point for the software development. An extension could be an array of energies / powers depending on the wavelength (=spectrum). This could also propagate through the network and possibly be transformed using non-linear optics (frequency doubling)

- Geometric optics propagation

    In this case an array of 2D vectors representing the vertical distance from an optical axis and its angle to it. This vector might also be extended by an energy, or a wavelength. Later on, it must be extended by the horizontal information. Furthermore, polarization information could be added using Jones matrices.

- Wavefront optics propagation

    For this kind of simulation, a 2D complex matrix is necessary which simulates the nearfield intensity distribution and the local phase of the wavefront. In addition, would be an array of these matrices depending on the wavelength for simulating polychromatic light. Furthermore, similar to the case of geometric optics, a 2D array of local polarization information could be handled (e.g. simulation of local birefringence effects etc.).

- Time dependency

    It is still not clear how to [handle time-dependent phenomena](https://git.gsi.de/phelix/rust/opossum/-/issues/1)...

Of course,, the above information can be stored simultaneously in an edge and transported through the graph.
