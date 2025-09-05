# Current Status
## Version 0.7 – Graphical User Interface

Version 0.7 introduces the first **graphical user interface (GUI)** for OPOSSUM and marks a major step towards accessibility and ease of use. While earlier versions required system descriptions through configuration files, users can now construct and explore optical setups in a fully interactive environment.  

The GUI enables the construction of optical systems by simply dragging and dropping optical nodes onto a canvas. Optical paths are created by connecting these nodes, while alignment and relative distances can be defined visually and intuitively. Each node provides a dedicated configuration menu that allows parameters to be adjusted interactively without the need for manual file editing.  

Analysis is equally straightforward: analyzer nodes can be placed directly into the optical setup, after which the corresponding diagnostics are automatically included in the simulation. Running the simulation then requires nothing more than pressing the **Run Simulation** button.  

To support these frontend advances, a new **RESTful Web API** provides the backbone for communication between GUI, server, and simulation core. It offers endpoints for scene creation, modification, and deletion, as well as for connecting and disconnecting nodes.  

On the core side, several new features and refinements have been introduced. Sources can now be defined as **point sources** and shifted off the optical axis. Visualization has been enhanced with adjustable ray transparency in the Ray Propagation Visualizer, improved analyzer handling in the GUI, and corrections for spectrometric analysis of single-wavelength bundles. Property definitions have been streamlined to reduce memory usage, and serialization of light data has been improved for clarity and robustness.  

Finally, version 0.7 establishes a **deployment strategy** with binary packages and installers, lowering the barrier to adoption. It also ships with **workshop examples** that demonstrate typical use cases and provide an accessible starting point for new users. Documentation is extended with this handbook, which accompanies the GUI and ensures that the new functionality is well supported.  

Overall, version 0.7 represents a milestone release: it makes OPOSSUM substantially more approachable, broadens its audience, and lays the groundwork for future extensions by uniting a modern graphical frontend with a robust and extensible simulation backend.  

