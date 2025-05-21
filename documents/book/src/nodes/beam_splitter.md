# Ideal beam splitter

![Beam splitter icon](../images/icons/node_beamsplitter.svg)

## Ports

### Inputs

`input_1`
: Input 1 of the beam splitter / combiner.

`input_2`
: Input 2 of the beam splitter / combiner.

### Outputs

`out1_trans1_refl2`
: Output 1. In terms of a beamsplitter cube, this corresponds to the transmitted light from `input_1` and the reflected light from `input_2`.

`out2_trans2_refl1`
: Output 2. In terms of a beamsplitter cube, this corresponds to the transmitted light from `input_2` and the reflected light from `input_1`.

## Properties

`splitter config`
: Splitter configuration. This parameter defines how incoming beams are split or merged respectively. Possible options are:

- Ratio: The incoming energies are split according to a fixed ratio between 0.0 and 1.0. Thereby a value of 0.0 completely filters the input of port `input_1` and fully transmits `input_2`. For 1.0 the situation is vice versa.
- Spectrum: The incoming light is split with respect to its wavelength. The provided spectrum defines a wavelegth dependent splitting ratio between 0.0 and 1.0.
