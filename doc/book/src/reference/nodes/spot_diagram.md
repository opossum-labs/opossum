# Spot diagram

![Spot diagram icon](../images/icons/node_spotdiagram.svg)

## Analysis

As a detector node, incoming light data is simply passed unmodified through the node. However, possible apodization due to input or output port apertures might occur.

## Ports

`input_1`
: Input port.

`output_1`
: Light ouput. This port delivers a copy of the light data from the port `input_1`.

## Properties

`plot aperture`
: Boolean value. Show the aperture of the `input_1` port in the plot. Defaults to `false`.
