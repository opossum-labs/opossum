# Fluence detector

![fluence detector icon](../images/icons/node_fluence.svg)

## Analysis

As a detector node, incoming light data is passed unmodified through the node. In a ray tracing or ghost focus analysis, the node records only the light within its `optic shape`: light outside its window passes on unrecorded, and the log and the report warn about it.

## Report

The report shows the fluence map, its peak fluence, its total energy and the estimator. For Voronoi and HelperRays it also tells `Energy in edge margins`: the share of the energy in the margins at the edges of the light, whose place is known only to half a ray tube. If the light folds over itself on the detector, as near a caustic, a warning in the log says that the fluence near the fold is unreliable. If the map cannot be computed, the report names the reason, for example rays with helper rays on a KDE or Binning detector.

## Ports

`input_1`
: Input port.

`output_1`
: Light ouput. This port delivers a copy of the light data from the port `input_1`.

## Properties

`fluence estimator`
: The algorithm used to estimate the fluence. Currently there are the following options:

- Voronoi: follows the ray tubes the source spans. Each corner spreads its energy over the area it stands for at the source, grown as much as its tubes grew on the way. Where light was cut away, as behind a bored mirror, the map stays dark.
- KDE: spreads the energy of each ray with a smooth kernel. Plain rays only.
- Binning: sums the energies of the rays in the cells of a grid. Plain rays only.
- HelperRays: follows the same ray tubes as Voronoi, with the fluence the helper rays of each ray measured at its corners.

Further information can be found [here](../../concepts/fluence.md)

`optic shape`
: The window the detector records within, a square of 25 mm × 25 mm by default. Set it to the size of the camera chip, or to `Open` to record all light. If the optical axis misses the window during the alignment run, a warning says that the measurement may fail (see [Optic shape](../nodes.md#optic-shape)).