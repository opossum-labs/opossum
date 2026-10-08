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
: The radius of curvature of the mirror surface. A negative value corresponds to a concave (= focussing) mirror while a positive value corresponds to a convex (= defocussing) mirror. A value of `+infinity` or `-infinity` represents a flat mirror. The curved surface must reach the edge of the `clear aperture`: for a radius below 12.5 mm, reduce the clear aperture first.

`clear aperture`
: Transversal extent of the mirror. It decides which rays are reflected: a ray outside of it, or through a hole of a stacked shape, is lost in every analysis, since the mirror has no output for it (see [Clear aperture and port apertures](../nodes.md#clear-aperture-and-port-apertures)). Defaults to a circle of 12.5 mm radius; a shape without an edge (`Open`, Gaussian) is refused.
