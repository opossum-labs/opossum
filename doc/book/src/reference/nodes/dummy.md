# Dummy

![dummy icon](../images/icons/node_unknown.svg)

## Analysis

The incoming light data is simply passed unmodified through the node. Given an `optic shape`, the dummy acts as an iris: a ray outside of it misses the dummy (see [Optic shape](../nodes.md#optic-shape)).

## Ports

`input_1`
: Input port.

`output_1`
: Light ouput. This port delivers a copy of the light data from the port `input_1`.

## Properties

`optic shape`
: The opening of the iris, unbounded (`Open`) by default, so that all light passes. Any shape with an edge can be set instead. A ray outside of it follows the analyzer's missed surface strategy: with `Ignore` it passes unchanged, with `Stop` it is lost (see [Optic shape](../nodes.md#optic-shape)).
