# Energy meter

![Energy meter icon](../images/icons/node_energymeter.svg)

## Analysis

As a detector node, incoming light data is simply passed unmodified through the node. However, possible apodization due to input or output port apertures might occur.

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
