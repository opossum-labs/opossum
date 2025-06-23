# Modelling optical systems

In this chapter we want to develop a general model for describing optical systems. This taks represents a major part of the project. A careful planning of this model is a crucial point, since wrong decisions in the (data) model could lead to severe shortcomings in later real-world problems to be solved. Hence, this also directly influences the acceptance of this work in the community. A model not being used is of course of very little use...

In general, optical systems consist of light sources which provide a more or less complex light field (time invariant or time dependent) and optical components, which modify this light field.  Furthermore, there are light sinks such as simple beam dumps, targets or detectors. These are the elements which produce a "result" (e.g. measurable signal) and thus make a system "productive". The components - light sources (such as a laser) or optical elements (e.g. Faraday isolators) - might itself consist of sub components. In principle, these components again might consist of sub components with an unlimited nesting level.

Of course for a full system description, it would be sufficient to simply place the mechanical model of the optical components in a 3D space along with their particular orientation. For certain tasks, such as illumination or straylight analysis this would be an appropriate approach (and thus will be supported by our model). However, typical systems mostly cast optical rays or light fields in a directed way from one component to the next one. Optical systems can thus be rather decribed in network- or most often in tree-like structures.

## Directed graphs as primary model structure

For the above mentioned networks of optical components, well-established structures could be used which already exist for a long time: [directed graphs](https://en.wikipedia.org/wiki/Directed_graph). A directed graph consists of so-called *[nodes](nodes.md)* and *[edges](edges.md)*. For our purposes nodes respresent the optical components, while edges represent the information about the light (energy, wavelength, wavefront, nearfield distribution, etc.) to be handed from one node to the next one. Note, that in this picture, a free space propagation is also represented by a node.

A node has one or more *ports* where edges can be connected to. We thereby strictly distinguish between incoming and outgoing ports. A node with no input ports represents a light source. Nodes with no output ports are detectors. A simple (ideal) propagation node would have one input and one output port. Furthermpre, an ideal beam splitter has one input port and two or more output ports. More realistic components such as a real lens could also have more than one input and output ports e.g for simulating ghost reflections from lens surfaces.

There are different node types representing various optical components (ideal / real lenses, beam splitters, waveplates, etc.). Each node has, depending on its node type, various attributes, which describe component parameters such as length (e.g. for propagation nodes), focal length (ideal lenses), radii of curvature (real lenses) etc. In addition, there are *group nodes* which represent a set of other nodes. These nodes are also arranged in a directed graph. In this case non-conected ports form the "externally visible" ports of the group. Of course, the nodes of such a group itself can be group nodes thus allowing for setting up hierarchic, nested structures.

## Loops for modelling resonators

A directed graph can also model optical resonators by forming loops. This works well for ring resonators but might lead to problems for linear resonators. Let us assume we have the most simple linear cavity consisting of a mirror node, a propagation node, and a second mirror node. Forming a loop here might introduce some ambiguities. While the intented loop would consist of all nodes, a simple "reverse" edge from the second mirror back to the propagation node and then further to the first mirror would actually form three loops: The intended large loop from mirror to mirror ans two smaller loops directly between each mirror and the propagation node (which does not make sense in a real world setup). This becomes even worse for more complex resonators containing additional components (lenses, amplifier rods, etc.).

One way out might be the introduction of *reference nodes*. A referenece node, as the name says, only contains a reference to another node. So this node exactly behaves like the node it references. This way, a linear resonator could be translated into a corresponding ring resonator.

**Note**: Strictly speaking, light in a ring resonator can propagate in both directions (if not suppressed by optical components). Since we have a directed graph, only one direction can be modelled so far. Solutions need to be further [investigated](https://git.gsi.de/phelix/rust/opossum/-/issues/2).

## Intermediate data format

While being not yet clear at this stage how to fully describe an optical system using the described graph system we would propose to use simple text files for storing optical models. For this the YAML format seems to be appropriate since it is more or less human readable and allows for comments (in contrast to the JSON format). Furthermore, the standard rust serialization library `serde` already supports this format. As the software progresses, new features will be added or changed. For this, a version system should be considered stright from the beginning.

A graphical representation could be the usage of the [graphviz software](https://graphviz.org/) package. While the proposed rust graph library `petgraph` already provides some basic export to the graphviz [.dot files](https://graphviz.org/doc/info/lang.html) this needs to be extended.

On the long run a graphical (drag & drop) editor would of course the favorable option.
