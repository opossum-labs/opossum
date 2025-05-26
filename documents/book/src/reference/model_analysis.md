# Model analysis

Once a model is set up by defining the nodes, assigning specific node parameters, and connecting the nodes with edges, the model is ready to be analyzed / simulated. As already discussed in the introduction, the model could be analyzed in very different ways. One might simulate the system using geometric optics. A very simple analysis might only calculate the power / energy flow through the network. Another analysis would be a full-size Fourier optics propagation. In addition, a 3D raytracing procedure could give insight into illumination or straylight scenarios.

Besides this "direct" simulation of the model, it is possible to simply export the model into a given format suitable for external simulation software (e.g. ZEMAX or GLAD).

The underlying type of analysis fundamentally influences how to work with the model. For an energy flow or geometric analysis the system has to traverse in a particular way through the network in order to stepwise calculate the light field within the edges of the model. In contrast, for a 3D ray-trace analysis, the edges do not play a role at all but the nodes have to be placed at given 3D coordinates and light rays from one or more defined source nodes will be cast into the scene and collected by detector nodes.

For this flexibility, there are several *analyzers* provided. Note that these analyzers do not necessarily perform any calculations themselves directly but might only be responsible for calling the analysis functions of the nodes. In the following, we want to further discuss the analysis modes.
