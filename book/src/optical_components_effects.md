# Physical Processes, Subsystems and Components

Modern laser systems are composed of a large number of optical subsystems and individual optical components. For a holistic description of such a system, all these optical components and the structures that are built from these must be modeled. Furthermore, a numerous amount of physical effects/processes must be included to create a realistic view of the full system.

In the following, a, probably not complete, list of relevant effects/processes, subsystems and components for high-intensity lasers will be compiled. 

## Physical Effects/Processes
### Gain
Obviously, gain modeling of high-intensity lasers is indispensable to design either single amplifier modules or to get useful output predictions of the whole laser facility. Furthermore, spatially, temporally and spectrally dependent gain will alter the dynamics of the beam propagation and is needed for a full description of the laser system. Therefore, several gain models should be on hand for this:
* Gain as simple multiplicator
* Full rate-equation modeling
* 3-level, 4-level, quasi-three level
* Analytic approximations
* Saturation effects: 
    * Frantz-Nodvik equation
    * Wavelength-dependent saturation: Homogeneous/Inhomogeneous broadening
    * Absorption
    * Spatial Hole Burning
* 3D, 2D, 1D descriptions
* Spatial/Temporal Mode support of the amplifier

Simultaneously, effects that may alter or limit the gain and output of an amplifier should be regarded, such as:
* Amplified Spontaneous Emission
* Parasitic Lasing
* Parametric Fluorescence

### Second-order nonlinear effects
Second-order nonlinear effects include all processes that result from the response of the second-order susceptibility $\chi^{(2)}$:
* Second-Harmonic Generation (SHG)
* Sum-Frequency Generation (SFG)
* Difference-Frequency Generation (DFG)
* Optical Rectification (OR)

Furthermore, processes that utilize these processes shall be part of the description, such as:
* Optical Parametric Amplification (OPA)
* Optical Parametric Chirped-Pulse Amplification (OPCPA)
* Optical Parametric Generation/Oscillation (OPG/OPO)
* Spontaneous Parametric Down Conversion (SPDC, Parametric Fluorescence)
* Cascaded Mixing effects
* THz Generation

These effects are typically modeled by solving the nonlinear propagation equation with $\chi^{(2)}$ acting as the source term. 

### Third-order nonlinear effects
Similar to the second-order effects, third-order effects result from the response of the medium, described by the third-order nonlinear susceptibility $\chi^{(3)}$. This nonlinear response is responsible for a variety of effects:
* Kerr effect
    * Self-Phase Modulation (SPM)
    * Cross-Phase Modulation (XPM)
    * Modulational Instability
    * Self-focusing (Whole beam and small scale)
    * Two-Photon Absorption
    * Self-Steepening
* Four-Wave Mixing
    * Third Harmonic Generation
    * Parametric Amplification in fibers
    * Cross-Polarized Wave Generation
    * Self-Diffraction
    * Optical Phase Conjugation

While the above-mentioned processes act quasi-instantaneous, delayed nonlinear responses of the medium are also relevant for the description of some processes. The most prominent effects here are:
* Stimulated Raman Scattering (SRS)
* Stimulated Brillouin Scattering (SBS)

### Other Effects
Aside from processes that are related to the nonlinear susceptibility of the medium, various other processes exist which may alter the electric field during propagation/amplification. These include:
* Thermal Effects such as thermal lenses, thermally induced birefringence
* Electro-Optic Effects: Pockels Effect, Photorefraction
* Acousto-Optic Effects: Deflection or frequency shifts in, e.g., acousto-optic modulators
* Gain/Absorption saturation

## Subsystems of modern laser facilities
### Amplifiers
The most relevant subsystems to create high intensities are the amplifiers of the laser. In modern laser systems, the amplification follows the structure of the "Master-Oscillator Power Amplifier" (MOPA). This means that the amplifiers are linked in succession and the output of a previous amplifier is used as input for the next one. This allows the amplifier chain to be conveniently described by means of nodes, whereby the individual amplifier nodes themselves can be, but do not necessarily have to be, described using a node system. 
Nowadays, there are a large number of amplifier types that are implemented in modern laser systems. In the following, several amplifier types and their relevant pumping mechanisms are listed:
* Fiber-based amplifiers
    * Continuous/pulsed laser diode pumping: Forward, backward, forward + backward
* Cavity-based amplifiers
    * Short-Pulse Oscillators (CW pump: e.g. frequency-doubled Nd:YAG)
    * Regenerative amplifiers (pulsed pumping: flash lamp, laser diode)
    * Optical parametric oscillators (CW pumping or two coupled resonators)
    * Thin-Disk amplifiers (same as regenerative amplifier but also CW laser-diode pumping for high repetition rate)
* Non-Cavity-based Multi-Pass Amplifiers
    * Relay-imaged multi-pass (pulsed/cw laser diodes)
    * Non-imaged multi-pass (pulsed/cw laser diodes)
    * Innoslab Amplifiers (pulsed/cw laser diodes)
* Rod Amplifiers (Complex Flash-Lamp Pump geometries)
* Slab Amplifiers (Complex Flash-Lamp Pump geometries)
* Coherent Beam Combination (no pump)
* OPA/uOPA/OPCPA (from cw to narrowband long pulse, broadband long pulse or short pulse pump)

### Beam Transport
As laser facilities consist of multiple, subsequent amplifier modules, beam transportation is necessary which is typically done via telescopes. Here, certain aspects are of relevance:
* Magnification
* Imaging of the beam: Relay-Imaging
* Aberrations
* Use for mode matching
* Use for diffractive Beam shaping
* Transmissive or reflective telescope
* Parabola usage

### Stretching and Compression
To reach highest intensities, the concept of chirped-pulse amplification is typically used which requires temporally stretching the pulses and compressing them after amplification. While the use of gratings for the main stretching and compression modules have prevailed, other systems are nonetheless found in laser systems. Accordingly, various setups should be regarded:
* Grating stretcher
    * Martinez setup (Unfolded with lenses)
    * Martinez setup (Unfolded with mirrors)
    * folded Martinez (Banks)
    * Öffner setup
* Grating Compressor
    * Four gratings
    * Two gratings with a double pass
    * Two gratings with single pass
* Prisms
* Grisms
* Chirped Fiber/Volume Bragg Gratings
* Dispersion-based Fiber Stretcher (long fiber)
* Chirped mirror setups
* Gires-Tournois Interferometer setups

Effects concerning stretching and compression process which should be included are:
* Calculation of the phase
* Misalignment sensitivity
* Contrast issues due to Spatiospectral coupling

### Measurement devices
Every laser system relies on constantly doing measurements to observe the quality of the provided pulses/beams. Consequently, holistic simulation software should provide the outcome of the simulation either in a direct, "ideal" way or modulated as measured with a certain device. For example, a measured signal of a photodiode is a convolution of the real input signal with the response function of the diode. Such a response of a device may be specifically calculated for each type of device or even more specific for an exact rebuild of a given device. Below, a list of potential parameters of interest and a list of measurement devices is given.
* Parameters
    * Energy
    * Power
    * Fluence
    * Intensity
    * Spectrum
    * Spectral Phase
    * Pulse duration
    * Electric Field
    * Beam size
    * Wave front
    * Temporal Contrast
    * Spatial Contrast
* Measurement Devices
    * Energy Meter
    * Photo Diode
    * Camera
    * Shack-Hartmann Sensor
    * Spectrometer
    * FROG, SPIDER, Wizzler
    * Autocorrelator, Cross-Correlator
    * Shear Plate
    * Interferometer setups


## Optical Components
Every subsystem is composed of at least one or multiple optical components, which are the basis for the modeling of the whole laser system. Each component itself may be modeled separately and effective values may be used in its simulation node. Alternatively, the whole structure of the component may be modeled to get the most realistic output. For example, a more idealistic lens may contain information about its focal length only and a more thorough lens may use information such as the radius of curvature of all surfaces, its aperture and refractive index profile. In optical systems, a broad variety of different single optical components are used, which however may be classified into several groups:
* Lenses
    * Spherical, Aspherical, Achromatic, Cylindrical, 
    * Fresnel, Axicon, GRIN, Powel Lens
    * Microlens array
* Mirrors
    * Flat, Curved
    * Dielectric, Metalic
* Beamsplitter
    * Polarizing, Spectral, Pellicle, Polka Dot
* Filter
    * Spectral, Polarizing, Neutral density
* Dispersive Elements
    * Prisms, gratings, Grisms, fiber gratings
* Nonlinear Crystals
    * Uniaxial, Biaxial
* Laser Medium
* Fiber
* Phase plates
* Polarization Manipulation
    * Pockels Cell, Faraday Rotator, Wave plates
* Apertures
    * Pinholes, Serrated

Whether a fixed classification in group nodes is useful still has to be determined.
