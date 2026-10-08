# Reflective grating

![Grating icon](../images/icons/node_grating.svg)

## Analysis

## Ports

`input_1`
: Input port.

`output_1`
: Light ouput. This port represents the light which has being reflected on the grating.

## Properties

`line density`
: The density (lines per length) of the regular structure on the grating.

`diffraction order`
: The diffraction order delivered a output port `output_1`.

`clear aperture`
: Transversal extent of the grating. It decides which rays are diffracted: a ray outside of it is lost in every analysis (see [Clear aperture and port apertures](../nodes.md#clear-aperture-and-port-apertures)). Defaults to a circle of 12.5 mm radius; gratings are often rectangles. A shape without an edge (`Open`, Gaussian) is refused.
