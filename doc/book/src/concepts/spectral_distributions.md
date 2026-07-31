# Spectral Distribution

In OPOSSUM, the spectral distribution defines the wavelength composition of an optical source. It describes which wavelength components are present in the source and how these components are relatively distributed.

While the position distribution defines where rays originate and the energy distribution defines the energy assigned to rays, the spectral distribution defines the wavelength components associated with the source.

The following spectral distributions are currently available:

- `LaserLines`
- `Gaussian`

## General Concepts

The spectral distribution describes how the source is distributed over wavelength. It generates wavelength components together with their corresponding relative contribution values.

The generated spectral distribution consists of wavelength-value pairs:

- The wavelength defines the spectral component.
- The value defines the relative contribution of that wavelength component within the spectral distribution.

The spectral distribution is defined independently from the position and energy distributions. The position distribution determines the spatial origin of rays, while the energy distribution determines the energy assigned to rays. The spectral distribution determines the wavelength composition of the source.

Different spectral distributions can therefore represent different types of optical sources, such as sources with discrete wavelength components or sources with continuous spectral profiles.

The spectral distribution describes the relative wavelength composition of the source and does not define the absolute energy of the source.

## Laser Lines Distribution

The `LaserLines` distribution represents sources containing discrete wavelength components.

Each laser line is defined by:

- wavelength
- relative contribution value

A single wavelength source can be represented by one laser line. For example:

- wavelength: `1.054 µm`
- relative contribution: `1.0`

represents a source containing one wavelength component with a relative contribution of 1.

Multiple laser lines can be defined to represent sources containing several discrete wavelengths. Each wavelength component can have its own relative contribution value, allowing different spectral components to be represented within the same source.

## Gaussian Distribution

The `Gaussian` distribution represents a continuous spectral profile based on a Gaussian function.

Instead of defining individual wavelength components, the Gaussian distribution generates multiple wavelength samples within a specified wavelength range. Each wavelength sample receives a relative contribution value calculated according to the Gaussian profile.

The Gaussian distribution is defined by the following parameters:

- `wvl_range` – defines the wavelength interval over which the spectral profile is generated.
- `num_points` – defines the number of discrete wavelength samples generated within the specified wavelength range.
- `μ` (`mu`) – defines the center wavelength of the Gaussian spectral profile.
- `FWHM` – defines the full width at half maximum of the spectral peak and controls the spectral width.
- `Power` – controls the shape of the Gaussian profile. A value of `1` produces a standard Gaussian distribution, while larger values produce super-Gaussian profiles.

The generated Gaussian spectral distribution is normalized so that the sum of all generated relative contribution values equals `1`. Therefore, the generated values describe the relative spectral contribution of each wavelength sample rather than an absolute energy value.

By changing the Gaussian parameters, different spectral profiles can be represented while maintaining the same wavelength-based representation.