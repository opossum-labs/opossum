# Dummy

![dummy icon](../images/icons/node_unknown.svg)

## Analysis

The incoming light data is simply passed unmodified through the node. Given a `clear aperture`, the dummy acts as an iris: a ray outside of it misses the dummy (see [Clear aperture](../nodes.md#clear-aperture)).

## Ports

`input_1`
: Input port.

`output_1`
: Light ouput. This port delivers a copy of the light data from the port `input_1`.

## Properties

`clear aperture`
: The opening of the iris, unbounded (`Open`) by default, so that all light passes. Any shape with an edge can be set instead. A ray outside of it follows the analyzer's missed surface strategy: with `Ignore` it passes unchanged, with `Stop` it is lost (see [Clear aperture](../nodes.md#clear-aperture)).
