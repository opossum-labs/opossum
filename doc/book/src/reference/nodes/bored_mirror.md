# Bored mirror

![Bored mirror icon](../images/icons/node_bored_mirror.svg)

This node represents a mirror with a bore through it, such as a scraper mirror: a glass plate, 6 mm thick by default, whose
front face is mirrored, with one bore through the plate. The mirror reflects the light on its front face, while the light
in the bore passes the plate untouched. It therefore splits a beam in two, as a [beam splitter](beam_splitter.md) does,
and has two inputs and two outputs: `mirror` and `through_bore` for the light arriving at the front and from behind,
`in_front` and `behind` for the light leaving in front of and behind the plate. It is the only component that takes a
bore (see [Optic shape](../nodes.md#optic-shape)).

## Analysis

For ray tracing, a ray arriving at `mirror` meets the mirrored front face:

- **On the mirror** it is reflected according to the coating of the `front surface`, fully by default, and leaves through
  `in_front`. The part the coating lets through enters the glass. Light inside the glass is not traced yet: it is lost,
  and the log warns about it with its energy.
- **In the bore** it passes the plate untouched, whatever the analyzer's missed surface strategy, and leaves through
  `behind`. It has to leave the bore at the rear face as well. A ray that runs into the wall of the bore on its way,
  because the bore is tilted against it or because the beam converges, is lost, and the log warns about it with its
  energy. No point on the wall is calculated: the ray ends where it was before it reached the mirror.
- **Beside the edge** it follows the analyzer's missed surface strategy: with `Ignore` it passes on as transmitted light,
  with `Stop` it is lost.

A ray arriving at `through_bore` meets the rear face from behind. In the bore it passes the same way and leaves through
`in_front`, together with what the mirror reflects of `mirror`. On the rear face it enters the glass and is lost with a
warning, except for the part the coating of the `rear surface` reflects, which leaves through `behind`. The mirror never
reflects it.

A ghost focus analysis treats the light the same way, except that a ray beside the edge always passes on.

An energy analysis knows no geometry, so it cannot tell the light in the bore from the light on the mirror. As soon as
light reaches a bored mirror it stops with an error naming the node. Use a ray tracing analysis instead.

## Positioning

The bored mirror is placed from the optical axis of `mirror` if it carries one, otherwise from `through_bore`. The axis is
split onto both outputs, so that the following nodes can be placed along both branches. Like a beam splitter, a bored
mirror with an alignment of 0° reflects the light straight back. To deflect the reflected beam by 90°, rotate it by 45°
in the `Alignment` section of the `Node Editor`.

**Placed through the bore, the axis comes from behind.** Light at `through_bore` only passes the bore, so a mirror
placed through it gets the optical axis from behind along the axis of the bore, through the center of the bore on the
rear face. The distance of the connection counts to that point. With an alignment of 0° the light thus meets the
uncoated rear face first, with the mirrored face turned away from it: that is where light from behind arrives. Unlike
the second input of a beam splitter, `through_bore` is not placed as if the mirror reflected its light. A source set to
`automatic` in front of `through_bore` is placed behind the bore, aimed along it.

**The axis sees only the edge.** The axis is reflected at the front face as if there were no bore, even where it runs
through the bore. A bore on the axis, as on a scraper mirror that lets the center of the beam pass, is placed without
an error.

**Behind the plate the axis runs through the bore.** The transmitted axis does not continue from the point where the
arriving axis meets the mirror. It starts anew at the center of the bore on the face it enters through, which is where
the axis of the bore meets that face. From there it runs parallel to the arriving axis through the plate, and the nodes
connected to `behind` are placed from where it leaves the rear face. A bore beside the axis therefore shifts the
transmitted branch sideways onto the light the bore lets pass. The way to the mirror belongs to the reflected branch; the
3D view draws no line from the point the axis meets the mirror to the bore.

**A tilted mirror wants a tilted bore.** An untilted bore runs along the axis of the mirror. On a mirror rotated by 45°,
light along the arriving beam would partly run into its wall. To let the beam pass, tilt the bore along it: on a mirror
turned by a `Roll` of r, set `Tilt direction` to 90° and `Tilt angle` to r; on one turned by a `Pitch` of p, set
`Tilt direction` to 180° and `Tilt angle` to p (see [Optic shape](../nodes.md#optic-shape)). For light from behind
through `through_bore` to join the reflected beam, tilt the bore along the reflected beam instead.

## Ports

### Inputs

`mirror`
: At the mirrored front face.

`through_bore`
: At the rear face, for light from behind that passes the bore.

### Outputs

`behind`
: Behind the plate: the light `mirror` sends through the bore and the light the rear face reflects of `through_bore`.

`in_front`
: In front of the plate: the light the mirror reflects of `mirror` and the light `through_bore` sends through the bore.

## Properties

`curvature`
: The radius of curvature of the mirrored front face. A negative value corresponds to a concave (= focusing) mirror, a
  positive value to a convex (= defocusing) one. A value of `+infinity` or `-infinity`, the default, represents a flat
  mirror. The curved face must reach the edge of the `optic shape`.

`center thickness`
: Thickness of the plate on its axis, 6 mm by default.

`material`
: The glass of the plate, by default a draft material "mirror substrate" with a constant refractive index of 1.5. Light
  inside the glass is not traced yet (see [Analysis](#analysis)).

`optic shape`
: The edge of the mirror and the bore through it. The default is a circle of 12.5 mm radius with a centered circular
  bore of 2.5 mm radius. The bore can be shifted, turned and tilted, but not removed, and `Open` is refused. The bore has
  to lie within the edge on the front face and on the rear face (see [Optic shape](../nodes.md#optic-shape)).

`front surface`
: Coating and laser induced damage threshold of the mirror, a fully reflecting coating (`ConstantR` of 100 %) by default
  (see [Surfaces](../nodes.md#surfaces)). It takes any coating but a `Separator`.

`rear surface`
: Coating and laser induced damage threshold of the rear face, an ideal antireflective coating (`IdealAR`) by default.
