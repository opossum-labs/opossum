# Model analysis

Once a model is set up by defining the nodes, assigning specific node parameters, the model is ready to be analyzed / simulated.The model setup also includes creating connections between the input and output ports of the nodes and assigning the propagation distances between connected nodes, which define the optical path through the network. As already discussed in the introduction, the model could be analyzed in very different ways. One might simulate the system using geometric optics. A very simple analysis might only calculate the power / energy flow through the network.In addition, a 3D ray-tracing procedure could give insight into illumination or stray-light scenarios.

Model analysis is used to simulate the behavior of the optical system and obtain information about light propagation through the model. OPOSSUM provides different analyzers for different simulation goals. Depending on the selected analyzer, different aspects of the optical system can be analyzed and different types of optical information can be calculated.

The available analyzers and their respective analysis methods are described in the [Analyzers](../reference/Analyzers.md) section.
