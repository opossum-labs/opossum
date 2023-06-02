# Interfacing with external code

The presented project is set up as a *simulation platform* which means that it only provides the framework / infrastructure to perform complex simulations of optical systems. This can be done in different ways. Once an optical system is modeled as explained before it should basically contain all data necessary to run specific simulation codes. Besides that, the entire network can be exported to files compatible with other simulation systems. This is mostly of interest for closed-source (commercial) software packages such as ZEMAX or GLAD. Fortunately, these software packages have more or less human-readable project files which can be reverse-engineered with not too much effort.

For existing open-source software packages it is important to provide interfaces. So far, two codes could be used as proof-of-principle projects:

  - SHG software (GSI)

    This software simulates the behavior of non-linear crystals for second-harmonic generation. This package is written in Python and could be integrated into an "SHG node". It should be investigated, how the interfacing could be performed. Some preliminary work has been done in the current "play project" *opticplay*. The external code was called from Rust using the PyO3 library.

  - HASEonGPU (HZDR)

    This software package is written in C++ and works on graphic CPUs (using CUDA?). If this project provides an external library (DLL) it would be relatively easy to implement. This has to be investigated.