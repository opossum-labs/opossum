## Fluence 

In OPOSSUM, fluence describes the spatial distribution of optical energy deposited on a detector surface. It represents energy per unit area after the optical system has transformed the incoming beam.

Since ray tracing represents light using a finite number of rays, fluence must be reconstructed from discrete energy samples. Each ray carries a portion of energy and contributes to the detector at its intersection point. The reconstruction method defines how these discrete contributions are converted into a continuous fluence map.

This step is essential because quantities such as peak fluence, beam shape, and local intensity cannot be interpreted reliably from raw ray positions alone.

## Why Reconstruction is Needed

A straightforward approach is to divide the detector into a fixed grid and accumulate ray energy per cell. However, this introduces limitations. If the grid is too coarse, peak fluence is underestimated because energy is averaged over large areas. If the grid is too fine, the result becomes noisy unless a very large number of rays is used.

This creates a trade-off between spatial resolution and statistical stability. To address this, OPOSSUM provides multiple reconstruction methods that interpret the same ray data in different ways.


## Binning

The binning method divides the detector into a regular grid of square cells. Each ray contributes its energy to the cell where it lands, and fluence is computed as energy divided by cell area.

This method behaves like a pixelated representation of the detector. It is simple and computationally efficient, and it guarantees energy conservation within each cell.

However, the result strongly depends on grid resolution. A coarse grid smooths out details and can hide peak fluence regions, while a fine grid requires a high number of rays to avoid noise. As a result, binning is often used for fast but lower-resolution analysis.


## Voronoi

The Voronoi method divides the detector into regions based on proximity to rays. Each position on the detector belongs to the nearest ray, forming irregular cells that adapt to the spatial distribution of samples.

Each ray is treated as the center of a region that represents its local area of influence. The energy of the ray is assigned to this region.

This approach removes the dependence on a fixed grid and naturally adapts to variations in ray density. In dense regions, cells become small; in sparse regions, they become larger.

The result is more geometrically consistent than binning and better preserves local variations in the fluence distribution.


## KDE (Kernel Density Estimation)

The KDE method reconstructs fluence by treating each ray as a smooth energy distribution rather than a single point. Instead of assigning energy to a single location or cell, each ray spreads its energy over a surrounding area using a smooth kernel function.

The final fluence map is obtained by summing the contributions of all rays, resulting in a continuous and smooth intensity field.

The smoothing strength is controlled by the kernel width. If the width is too small, the result becomes noisy and resembles raw sampling. If it is too large, fine spatial details are lost and peak fluence can be reduced.

To balance this, the kernel width is typically chosen based on the density of samples using established statistical rules that adapt smoothing to the number and spread of rays.

This method provides the smoothest representation of fluence and is especially useful when a continuous interpretation of the beam profile is required.

## Peak Fluence and Interpretation

Peak fluence represents the maximum energy concentration on the detector surface and is one of the most important quantities in optical system analysis.

It is sensitive to both sampling density and reconstruction method. Insufficient sampling can underestimate peak values, while excessive smoothing can reduce peak intensity. Conversely, coarse binning can either exaggerate or suppress peaks depending on grid alignment and resolution.

Despite these differences, all reconstruction methods conserve total energy globally. The differences lie only in how the energy is distributed spatially.

For this reason, fluence reconstruction should always be interpreted together with ray density and system geometry.

## Key Takeaways

Fluence reconstruction converts discrete ray energy into a continuous spatial representation.

Binning uses fixed grid cells and is fast but resolution-dependent.

Voronoi uses adaptive geometric regions based on proximity to rays and better reflects local structure.

KDE produces a smooth continuous distribution by spreading each ray’s energy over space.

The choice of method affects how clearly peak fluence and spatial energy distribution are represented, especially in systems with focused beams or non-uniform illumination.