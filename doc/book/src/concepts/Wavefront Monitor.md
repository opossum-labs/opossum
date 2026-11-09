# Wavefront Monitor

The Wavefront Monitor measures the optical wavefront at the monitor position and displays the result as a color-coded map.
The displayed colors represent the spatial variation of the wavefront value across the monitor surface.

Wavefront analysis is an important tool for evaluating the quality of an optical system and identifying optical aberrations.

## Background

A wavefront is a surface of constant optical phase.

For a collimated beam, the rays are parallel and the wavefront is flat and perpendicular to the propagation direction.

A point source or diverging beam produces a spherical wavefront. Likewise, an ideal lens transforms a collimated beam into a spherical converging wavefront whose center is located at the focal point.

In an ideal optical system, the resulting wavefront follows the expected shape exactly. Real optical systems, however, introduce deviations due to manufacturing tolerances, alignment errors, and optical imperfections. These deviations from the ideal wavefront are known as wavefront aberrations.

The goal of optical design is to minimize these aberrations and obtain a wavefront as close as possible to the ideal case.

## Why is Wavefront Analysis Important?

Wavefront quality directly influences the performance of an optical system.

For an ideal optical system illuminated by a perfect plane wave:

* All rays converge to the same focal point.
* The focal spot size is minimized.
* Optical performance is optimal.

If the wavefront is distorted, the rays no longer converge perfectly, resulting in:

* Increased spot size
* Reduced image quality
* Lower optical performance

The Wavefront Monitor helps visualize and quantify these effects.

## Wavefront Tilt

A tilted wavefront does not necessarily indicate an aberrated beam.

For example, a perfectly planar wavefront can appear tilted if the beam propagation direction is not aligned with the normal direction of the monitor surface.
In this case, the monitor displays a linear gradient across the measurement area even though the wavefront itself remains flat. This effect is called wavefront tilt.

Since tilt can mask the aberrations of interest, the monitor provides a Compensate Tilt option. When enabled, the linear tilt component is removed from the measured wavefront, allowing the remaining wavefront aberrations to be evaluated more easily.

## Calculation

The Wavefront Monitor evaluates the optical path length of each ray and calculates the corresponding optical path difference (OPD).
OPD describes the difference in optical path length between a measured wavefront and a chosen reference wavefront.

The optical path length for a uniform medium is given by:

OPL = n · d

where:

* n is the refractive index of the medium
* d is the physical distance traveled by the ray

For example:

* Vacuum: n = 1
* Typical optical glass: n ≈ 1.5

Since rays travel through different media and may follow different paths, their optical path lengths are generally not identical.

The monitor calculates the optical path difference relative to a reference wavefront, which represents the expected ideal wavefront, and visualizes the deviation as a color-coded map.

## Interpretation of Results

For a perfect optical system, the measured wavefront follows the expected ideal shape.

Any deviation from the ideal wavefront indicates the presence of aberrations.

A commonly used metric is the Peak-to-Valley (PV) value, which is defined as the difference between the maximum and minimum wavefront values:

PV = Wmax - Wmin

The Peak-to-Valley value provides a measure of the overall wavefront deformation.

* Smaller PV values generally indicate a wavefront closer to the ideal reference shape.
* Larger PV values indicate stronger deviations from the reference wavefront and may lead to degraded optical performance.

## Measured Quantities

The Wavefront Monitor provides:

* Color-coded wavefront map
* Optical path difference distribution
* Wavefront error visualization
* Wavefront tilt compensation
* Peak-to-Valley evaluation

These results can be used to assess optical quality, identify aberrations, and optimize the performance of an optical system.
