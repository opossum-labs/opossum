# Parabolic mirror

![Parabola icon](../images/icons/node_parabola.svg)

## Analysis

## Ports

`input_1`
: Input port.

`output_1`
: Light ouput. This port represents the light after the refelction on the mirro surface.

## Properties

`focal length`
: The focal length of the parabola. A positive value corresponds to a concave (=focussing) parabola.

`oa angle`
: Off-axis angle. This value determines the off-axis angle of the parabola. Internally the parabola surface is shifted accordingly by the respective amount with respect to the parabola's origin. The direction of the shift is defined by the property `oa direction`.

`oa direction`
: Orientation of the off-axis angle (see `oa angle`). This is a 2D vector denoting the direction of the off-axis shift with respect to the local coordinates.

`collimating`
: Boolean value determinig if the parabola is used to collimate a beam.
