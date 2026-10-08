# Energy meter

![Energy meter icon](../images/icons/node_energymeter.svg)

## Analysis

As a detector node, incoming light data is passed unmodified through the node; only a port aperture can mask it. In a ray tracing or ghost focus analysis, the node measures only the light within its `clear aperture`: light outside a set window passes on unmeasured, and the log and the report warn about it.

## Ports

`input_1`
: Input port.

`output_1`
: Light ouput. This port delivers a copy of the light data from the port `input_1`.

## Properties

`meter type`
: The type of the energy meter. This property will be used in the future to model different energy / power meters with its characteristic properties (e.g. wavelength depenedent sensitivity, etc.). Current options are:

- IdealEnergyMeter
- IdealPowerMeter (currently not used)

`clear aperture`
: The window the meter measures within, unbounded (`Open`) by default. Any shape with an edge can be set instead, e.g. the size of the sensor; `Open` restores the unbounded window. If the optical axis misses a set window during the alignment run, a warning says that the measurement may fail (see [Clear aperture and port apertures](../nodes.md#clear-aperture-and-port-apertures)).
