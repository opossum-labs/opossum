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

`clear aperture`
: Transversal extent of the mirror, measured parallel to the parent axis of the parabola, as catalogs state it for off-axis parabolas. It decides which rays are reflected: a ray outside of it is lost in every analysis (see [Clear aperture and port apertures](../nodes.md#clear-aperture-and-port-apertures)). Defaults to a circle of 12.5 mm radius; a shape without an edge (`Open`, Gaussian) is refused.
