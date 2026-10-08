# Paraxial surface

![Paraxial icon](../images/icons/node_paraxial.svg)

A paraxial surface represents an ideal lens which is free of geometric or chromatic aberrations. During ray analysis, an incoming collimated beam is focussed down to a spot size of zero. During energy analysis, the light is not modified.

## Analysis

## Ports

`input_1`
: Input port.

`output_1`
: Light ouput. This port represents the light having passed this ideal lens.

## Properties

`focal length`
: The focal length of this ideal lens. A positive values denotes a focussing lens.

`clear aperture`
: Transversal extent of the ideal lens. Only rays within it are focussed. A ray outside of it follows the analyzer's missed surface strategy: with `Ignore` it passes unchanged, with `Stop` it is lost (see [Clear aperture and port apertures](../nodes.md#clear-aperture-and-port-apertures)). Defaults to a circle of 12.5 mm radius. As an idealization, the paraxial surface may also be unbounded (`Open`): it then focusses every ray, however far off the axis.
