# Cylindric lens

![Cylindric lens icon](../images/icons/node_cylindric_lens.svg)

This element represents a "real" lens with cylindrical front and back surfaces. Furthermore, the lens consists of an optical material with a given refractive index model. The `center thickness` denotes the distance of the front and back surfaces at symmetry axis of the lens.

By default, both cylinder axex are oriented along the y axis. This can be easily changed using the `alignment` property. 

Besides the usual aperture definitions of the `front` and `back` surfaces, a lens might limit the aperture additionally if the front and back surfaces intersect. E.g. this is always the case for a biconvex lens. In this case, rays outside this intersection circle are clipped.

## Analysis

- Energy Analysis

    During energy analysis, besides the common behaviour, this element does not alter the incoming the incoming light.
    The output is a copy of the input light. Above mentioned intrinsic apertures as well as explicit port apertures are ignored.

- Ray tracing Analysis

    During ray tracing analysis, incoming rays are refracted on the `front`surface according to Snellius' law of refraction.
    Inside the lens the ray propagate within the given medium. On the `rear`surface, the rays are again refracted.

- Ghost focus Analysis

    During this analysis, the lens behaves similar to the ray tracing analysis.


## Ports

`input_1`
: Input port. This represents the `front` surface of the lens.

`output_1`
: Light ouput. This represents the `rear` surface of the lens.

## Properties

`front curvature`
: Radius of curvature of the `front` surface.

`rear curvature`
: Radius if curvature of the `rear` surface.

`center thickness`
: Thickness of the lens center along the optical axis.

`refractive index`
: Refractive index of the (glass-) material. See [refractive index definition](../refractive_index.md).
