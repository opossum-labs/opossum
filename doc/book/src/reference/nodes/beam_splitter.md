# Ideal beam splitter

![Beam splitter icon](../images/icons/node_beamsplitter.svg)

## Analysis

For energy analysis, the spectrum arriving at each input is split according to the `splitter config`: the transmitted part leaves through the output of the same number (e.g. `input_1` → `out1_trans1_refl2`), the reflected part through the other output (e.g. `input_1` → `out2_trans2_refl1`).

For ray tracing, the beam splitter acts as a thin splitting surface lying in the plane of the node. The `splitter config` defines the transmission of this surface; everything that is not transmitted is reflected. The coatings of the ports are not used for the split. Transmitted rays keep their direction, while reflected rays are deflected according to the laws of reflection. The same holds for the optical axis, so nodes connected to the reflected output are placed along the *reflected* axis (see [Geometry](../../concepts/geometry.md)).

Like a mirror, a beam splitter with an alignment of 0° therefore reflects the light directly back towards the source. To deflect the reflected beam by 90°, rotate the beam splitter in the `Node Editor` panel under the `Alignment` section, e.g. by setting the **Roll Angle** to **45°**.

## Positioning

During the alignment run (which precedes ray tracing and ghost-focus analysis), the beam splitter is placed from the optical axis of one of its inputs, and that axis is then split onto the two outputs so the following nodes can be placed in turn.

**Which input places the splitter.** If `input_1` carries the optical axis it is used; otherwise `input_2` is used. A beam splitter can therefore be positioned from either input on its own — connecting only `input_2` works just as well as connecting only `input_1`.

**Where the second input's beam comes from.** Both inputs share the same splitting surface. Because a beam entering `input_2` passes straight through to `out2_trans2_refl1`, its optical axis must arrive travelling in the direction that leaves `out2` — the mirror image, on the splitting surface, of the `input_1` axis. For an untilted beam splitter (0°) this is the direction straight back towards a source on the opposite side; for one rolled to fold the beam by 90°, it is the folded (sideways) direction. The up direction is mirrored to match, so both inputs share a consistent frame.

**Both inputs connected.** When both inputs carry an axis, the splitter is placed from `input_1` — deterministically, regardless of the order in which the two arms happen to be traced — and the axis reaching `input_2` is then checked against that placement. If the two do not agree (different direction, wrong distance along the axis, or a sideways miss), OPOSSUM keeps the placement from `input_1` and logs a warning describing the deviation and the exact position and direction the `input_2` beam would need to be consistent. See [Geometry → Beam Combiners and Parallel Paths](../../concepts/geometry.md#beam-combiners-and-parallel-paths) for the details and for the case of two independent sources.

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
