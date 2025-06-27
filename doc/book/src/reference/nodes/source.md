# Source

![Source node logo](../images/icons/node_source.svg)

## Analysis

## Ports

`input_1`
: Input port. **Note**: Don't us this port since it is not used during a normal calculation (input discard) but needed internally for a ghost focus calculation. This port might vanish in the future.

`output_1`
: Light ouput. This port delivers the light data as defined by the source's properties (see below).

## Properties

`light data`
: Definition of the light field of the source. See [here](../light_data.md) for more details.

`light data iso`
: Isometry of the light data field. By defining this isometry it is possible to move a point source with respect to the anchor point of the source.

`alignment wavelength`
: This property defines the wavelength of the single light ray which is used during alignment of the nodes.
