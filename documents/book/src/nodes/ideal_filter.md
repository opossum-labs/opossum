# Ideal filter

![ideal filter icon](../images/icons/node_filter.svg)

## Ports

`input_1`
: Input port.

`output_1`
: Light ouput. This port delivers a copy of the light data from the port `input_1`.

## Properties

`filter type`
: The filter definition. Currently there are these options:

- Constant: The incoming energy is attenuated by a constant (wavelength independent) factor. A value of 0.0 corresponds to a total absorption while 1.0 denotes a fully transparent filter.
- Spectrum: The incoming energy is attenuated according to the given filter spectrum. The filter spectrum is an array of filter values (between 0.0 and 1.0) with respect to a wavelength bin.
