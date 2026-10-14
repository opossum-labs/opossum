# Spectrometer

![Spectrometer icon](../images/icons/node_spectrometer.svg)

## Analysis

As a detector node, incoming light data is passed unmodified through the node. In a ray tracing or ghost focus analysis, the node records only the light within its `optic shape`: light outside its window passes on unrecorded, and the log and the report warn about it.

## Ports

`input_1`
: Input port.

`output_1`
: Light ouput. This port delivers a copy of the light data from the port `input_1`.

## Properties

`spectrometer type`
: The type of the spectrometer. Currently the following options are available.

- Ideal: Ideal spectrometer with constant (wavelength independent) sensitivity.
- H2000: Ocean Optics HR2000 spectrometer. Not really supported yet.

`optic shape`
: The window the spectrometer records within, a square of 25 mm × 25 mm by default. Set it to the size of the entrance, or to `Open` to record all light. If the optical axis misses the window during the alignment run, a warning says that the measurement may fail (see [Optic shape](../nodes.md#optic-shape)).
