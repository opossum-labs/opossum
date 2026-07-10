# Source_port

![Source_port logo](../images/icons/node_source.svg)

## Analysis

The Source Port is a node in the optical system that does not define any properties itself. Source-related properties are defined and configured through the associated Analyzer rather than the Source Port itself.

## Ports

`input_1`
: Input port. This port is not used by the Source Port.

`output_1`
: Light output. This port delivers the light data defined by the associated Analyzer.


## Properties

## Properties

The Source Port itself has no properties.
The following source parameters are configured in the associated Analyzer:

`ray type`
: Ray type, which can be Collimated, Point, or Image.

`ray tracing`
: Ray tracing properties, including position, spectral, and energy distributions.

`ghost focus analyzer`
: A similar set of source configuration options are available in the Ghost Focus Analyzer, providing a consistent workflow across analyzers.


If multiple Source Ports are used in the same optical setup, each Source Port can be assigned a unique name, allowing different Analyzers to reference different source definitions.

The Source Port separates the source definition from the optical model by storing all source configuration in the associated Analyzer. This allows the same optical system to be evaluated with different source configurations without modifying the optical model.

For example, the same optical setup can be used with:

* A Ray Tracing Analyzer using one set of source parameters.
* An Energy Analyzer using a different source configuration.
* Multiple Source Ports to evaluate different source images or optical paths simultaneously.
* A Wavefront Analyzer using the same optical configuration without modifying the model.

Because the source definition is managed by the Analyzer, the optical system only needs to be created once. Different Analyzers can apply their own source configurations to the same optical setup without duplicating or rebuilding the optical model.