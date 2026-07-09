# Source Port

![source port icon](../images/icons/node_source.svg)

The Source Port node replaces the previous Source node. Unlike the previous implementation, the Source Port node no longer contains source properties. Instead, all source related properties are defined and controlled through the associated Analyzer.

The Source Port node itself has no configurable properties. It serves as an interface between the optical system and the Analyzer, where the source definition and behavior are configured.

The following source parameters can be configured in the Analyzer:

* Ray type, which can be Collimated, Point, or Image.
* Ray tracing properties, including position, spectral, and energy distributions.
* The same source configuration options are available in the Ghost Focus Analyzer, providing a consistent workflow across analyzers.

If multiple Source Port nodes are used in the same optical setup, each Source Port can be assigned a unique name, allowing different Analyzers to reference different source definitions.

## Why use a Source Port?

The Source Port Node enables a more flexible workflow by separating the source definition from the optical model. Instead of storing source properties directly in the node, the source is configured through the associated Analyzer. This allows different analyzers to use the same optical configuration with different source settings.

For example, the same optical setup can be used with:

* A Ray Tracing Analyzer using one set of source parameters.
* An Energy Analyzer using a different source configuration.
* Multiple Source Port nodes to evaluate different source images or optical paths simultaneously.
* A Wavefront Analyzer using the same optical configuration without modifying the model.

Because the source definition is managed by the Analyzer, the optical system only needs to be created once. Different analyzers can apply their own source configurations without duplicating or rebuilding the setup. This makes the Source Port Node suitable for simulating the same optical system under different source conditions.
