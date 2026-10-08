# Wedge

![Wedge icon](../images/icons/node_wedge.svg)

This element represents a plate of optical material whose front and back surfaces are flat but not
parallel. A wedge angle of zero turns it into a plane-parallel plate.

Since this element encloses a volume of material, it can be operated as an amplifier: a
[pump scenario](../pump_scenarios.md) assigns it a gain model, and light travelling through the
medium is amplified accordingly. Without such an assignment the wedge is the passive component
described here. With flat surfaces and a suitable clear aperture it is the closest match to a slab
or disk amplifier head.

## Ports

`input_1`
: Input port.

`output_1`
: Light ouput. This port represents the light having passed the `back` surface of the wedge.

## Properties

`center thickness`
: Thickness of the wedge in its center along the (local) optical axis.

`material`
: The (glass-) material the wedge is made of. It carries the refractive index model — a constant
  value or a dispersion formula such as Sellmeier or Schott — which is what the refraction at both
  surfaces is calculated from.

`wedge`
: Wedge angle. Angle between the front and back surface. An angle of zero corresponds to parallel surfaces.

`clear aperture`
: Transversal extent of the wedge: the size the material is actually available in. It decides which
  rays hit the wedge, at both of its faces: a ray that would enter or leave through the side misses it
  and follows the analyzer's missed surface strategy (see
  [Clear aperture and port apertures](../nodes.md#clear-aperture-and-port-apertures)). Defaults to a
  circle of 12.5 mm radius, i.e. the usual 1 inch mount. Choose it so that the wedge keeps a positive
  thickness: beyond its thin edge rays miss the rear surface. A shape without an edge (`Open`,
  Gaussian) is refused. Not to be confused with the aperture of a port: a port aperture only masks the
  light that passed a surface, while the clear aperture states where the material ends.