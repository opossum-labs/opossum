# Project Goals

Before defining the project goals, let us first look at the current situation while designing laser systems.

## Current Issues

As stated in the introduction, the design of complex high-energy / intensity laser systems requires a detailed simulation of optical (and of course mechanical) effects and aspects. Often, these aspects have to be simultaneous taken into account while optimizing a system design. Different aspects might even stand against each other, such that optimizing (e.g. maximizing/ minimizeing) one effect degrade the performance of other system parameters. So, a hollistic approach would be desireable.

In the past, many tools were developed partly adressing very particular optical effects at many institutes. Often theses tools are only used at the institutions which developed the software and even there only used by one or two people (e.g. in the frame of a master of PhD thesis). This of course sometimtes leads to the situation that different institutes repeat the work and "reinvent the wheel". Hence, a common set of tools accompanied by proper knowledge exchange would significantly reduce this inefficiency.

Besides the solutions for modelling particular aspects of optical systems, there are many more general-purpose tools on the market which are unfortunately commercial, closed-source solutions. Each software has its own underlying design strategy. Furthermore, many of these tools (e.g. ZEMAX, OSLO, etc.) are more designed for simulating more "traditional" optical systems such as camera objectives or illumination setups. In contrast, laser (chain) systems often demand for different features which are not always fully supported (or easy to model) by these software packages.

The usage of different tools during the design phase often requires to repeatedly model the optical system in the particular software and provide a bunch of input parameters. A common platform would allow for modelling the wanted system once and analyze it with the above mentioned tools and provide the input data in the particular format.

## Goals

Based on the current situation discussed above the project adresses the follonwing goals:

- Improve the knowledge exchange about already existing software to model particular aspects of optical systems
- Conception of a general system for describing optical systems. This concept would also be helpful as a base for open-data efforts serving as a metadata standard.
- Development of a software platform / framework with the goal to provide clear interfaces (e.g. API or file-based) for interoparation of the different tools.
- (Depending on time constraints), direct implementation of modules within the above framework instead of external interfaces (e.g. geometric optics / raytracing).
- (Depending on time constraints), development of intuitive GUI for easy modelling of optical systems.
