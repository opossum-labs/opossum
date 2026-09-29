# Ideal beam splitter

![Beam splitter icon](../images/icons/node_beamsplitter.svg)

## Analysis

For energy analysis, the spectrum arriving at each input is split according to the `splitter config`: the transmitted part leaves through the output of the same number (e.g. `input_1` → `out1_trans1_refl2`), the reflected part through the other output (e.g. `input_1` → `out2_trans2_refl1`).

For ray tracing, the beam splitter acts as a thin splitting surface lying in the plane of the node. The `splitter config` defines the transmission of this surface; everything that is not transmitted is reflected. The coatings of the ports are not used for the split. Transmitted rays keep their direction, while reflected rays are deflected according to the laws of reflection. The same holds for the optical axis, so nodes connected to the reflected output are placed along the *reflected* axis (see [Geometry](../../concepts/geometry.md)).

Like a mirror, a beam splitter with an alignment of 0° therefore reflects the light directly back towards the source. To deflect the reflected beam by 90°, rotate the beam splitter in the `Node Editor` panel under the `Alignment` section, e.g. by setting the **Roll Angle** to **45°**.

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

- Ratio: The incoming light is split according to a fixed (wavelength-independent) transmission between 0.0 and 1.0. A value of 1.0 fully transmits both inputs (`input_1` → `out1_trans1_refl2`, `input_2` → `out2_trans2_refl1`), while a value of 0.0 fully reflects them (`input_1` → `out2_trans2_refl1`, `input_2` → `out1_trans1_refl2`).
- Spectrum: The incoming light is split with respect to its wavelength. The provided spectrum defines a wavelength-dependent transmission between 0.0 and 1.0.
