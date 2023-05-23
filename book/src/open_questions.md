# Open questions & Ideas

Here is a list of questions and ideas to be discussed.

- Project name

  We need a nice name and logo for the project :-) Suggestions:
    - LibreOptics: possible but boring?
    - BJOERN: what does it mean? :-)
    - LOST: Laser optics simulation tool
    - OPOSSUM: OPen-source Optics Simulation System and Unified Modeller
    - OCTOPUS - Open-source Computing Toolkit for Optics and Laser Systems
    - KOALA - Killer Optical Analysis for Laser Applications
    - Stingray: Simulation Tool for interactive non-linear and geometric rays (a laser system from Coherent with this name already exists...)
   
- Simulation of time-dependency

- Simulation of resonators

  This might be problem with directed graphs since light in a (ring) resonator can propagate in both directions.

- Ghost focus analysis

  It is not clear, how many internal reflections from an optical component can propagate through the system on the same path...and do we have to take interference into account...

- Component database

  If we use a database of components. We should still save the components in the data file in order to be portable. However, we would need to have some sort synchronization procedure. This would be similar to AutoDesk Inventor. In addition, nodes would need an (optional) field for a database id (or maybe better a hash value) in order to mark this component to be part of a database.

- Coating database

  How far should we go here? Just a "magic layer" with given properties or a real coating simulation which can become extremely complex. In principle a coating would also be a special kind of OpticNode. So we could start with simple "ideal" coatings. How should we treat uncoated surfaces? This could be modelled as an "uncoated" coating node (which would be e.g. a Fresnel model) or should this be part of a general surface node? This would depend the refractive indeces of neighboring materials...

- Type of data flowing through the model

  Should we use a "one size fits all" data type to model the light flowing through the nodes of a scenery? For many cases this might be way to heavy. I.e. if you simply want to simulate the light intensity through a set of filters it is not necessary to store the wavefront or spectrum.

- Simulation of tilted surfaces in sequential mode

  Several simulation systems (such as ZEMAX) use so-called "coordinate breaks" in order to simulate systems with non-linear geometry. Using these coordinate breaks is always a nightmare...While component tilts can be directly stored in the node properties a real axis break can so far not be modelled. Maybe we have to introduce a similar mechanism...