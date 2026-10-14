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

`optic shape`
: Transversal extent of the grating. It decides which rays are diffracted: a ray outside of it is lost in every analysis (see [Optic shape](../nodes.md#optic-shape)). Defaults to a square of 25 mm × 25 mm. `Open` is refused.
