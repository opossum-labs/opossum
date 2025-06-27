# Mirror

![Mirror icon](../images/icons/node_mirror.svg)

This node represents a perfect mirror, which can be flat or spherically curved. Note, that it only represents the *mirror surface* thus being a thin mirror. Hence it only has one input and one output port and does not provide a "leakage" output. 

## Aanalysis

For ray tracing analysis, incoming rays are deflected according to the laws of reflection. For energy analysis, the incoming light is unmodified.

## Ports

`input_1`
: Input port.

`output_1`
: Light ouput. This port delivers reflected light data from the port `input_1`.

## Properties

`curvature`
: The radius of curvature of the mirror surface. A negative value corresponds to a concave (= focussing) mirror while a positive value corresponds to a convex (= defocussing) mirror. A value of `+infinity` or `-infinity` represents a flat mirror.
