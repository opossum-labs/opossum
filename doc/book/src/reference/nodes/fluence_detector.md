# Fluence detector

![fluence detector icon](../images/icons/node_fluence.svg)

## Analysis

As a detector node, incoming light data is passed unmodified through the node; only a port aperture can mask it. In a ray tracing or ghost focus analysis, the node records only the light within its `clear aperture`: light outside a set window passes on unrecorded, and the log and the report warn about it.

## Ports

`input_1`
: Input port.

`output_1`
: Light ouput. This port delivers a copy of the light data from the port `input_1`.

## Properties

`fluence estimator`
: The algorithm used to estimate the fluence. Currently there are the following options:

- Voronoi:
- KDE:
- Binning:
- HelperRays:

Further information can be found [here](../../concepts/fluence.md)

`clear aperture`
: The window the detector records within, unbounded (`Open`) by default. Any shape with an edge can be set instead, e.g. the size of a camera chip; `Open` restores the unbounded window. If the optical axis misses a set window during the alignment run, a warning says that the measurement may fail (see [Clear aperture and port apertures](../nodes.md#clear-aperture-and-port-apertures)).