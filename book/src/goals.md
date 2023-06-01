# Project Goals

Before defining the project goals, let us first look at the current situation while designing laser systems.

## Current Issues

As stated in the introduction, the design of complex high-energy / intensity laser systems requires a detailed simulation of optical (and of course mechanical) effects and aspects. Often, these aspects have to be simultaneously taken into account while optimizing a system design. Different aspects might even stand against each other, such that optimizing (e.g. maximizing/ minimizing) one effect degrades the performance of other system parameters. So, a holistic approach would be desirable.

In the past, many tools were developed, often addressing very particular optical effects at several companies and research institutes. Often these tools are only used at the institutions which developed the software and even there only used by one or two people (e.g. in the frame of a master or PhD thesis). This of course sometimes leads to the situation that different institutes repeat the work and "reinvent the wheel". Hence, a common set of tools accompanied by proper knowledge exchange would significantly reduce this inefficiency.

Besides the solutions for modelling particular aspects of optical systems, there are many more general-purpose tools on the market which are unfortunately commercial, closed-source solutions. Each software has its own underlying design strategy. Furthermore, many of these tools (e.g. ZEMAX, OSLO, etc.) are designed for simulating more "traditional" optical systems such as camera objectives or illumination setups. In contrast, laser (chain) systems often demand different features which are not always fully supported (or easy to model) by these software packages.

The usage of different tools during the design phase often requires repeatedly modelling the optical system in the particular software and providing a bunch of input parameters. A common platform would allow for modelling the desired system once and analysing it with the above-mentioned tools and providing the input data in the particular format.

## Goals

Based on the current situation discussed above the project tries to address the following goals:

- Improve the knowledge exchange about already existing software to model particular aspects of optical systems.
- Conception of a general system for describing optical systems. This concept would also be helpful as a base for open-science / open-data efforts serving as a metadata standard.
- Development of a software platform / framework with the goal to provide clear interfaces (e.g. API or file-based) for the interoperation of the different tools.
- (Depending on time constraints), direct implementation of modules within the above framework instead of external interfaces (e.g. geometric optics / raytracing).
- (Depending on time constraints), development of intuitive GUI for easy modelling of optical systems.
