# Spectrometer

![Spectrometer icon](../images/icons/node_spectrometer.svg)

## Analysis

As a detector node, incoming light data is simply passed unmodified through the node. However, possible apodization due to input or output port apertures might occur.

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
