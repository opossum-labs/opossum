# Software architecture

This chapter discusses the overall software structure of the OPOSSUM system.

In a first version we want to concentrate on a framework providing the necessary entities (i.e. structs and traits) in order to model optical systems as [previously described](./optical_model.md). This system would simply require a `main` function calling the necessary structs. For better debugging purposes, we should already implement an export system to the `graphviz` package (dot-files) for visualization of the graph structures.

In a further step a commandline tool should be developed accepting a data file containing the model. This requires a proper serialization / deserialization system to be implemented. For this we would propose a very well-established standard crate `serde` which can then read and write data in various formats such as JSON or YAML.

For future extensions steps, the possibilities of modular design should be investigated in detail. This approach helps keeping the basic framework simple and might improve the integration of external code contributions. Hence, the possibilities of a plugin architecture should be considered.

A topmost level view could look like this:

![Toplevel architecture](./images/overall_architecture.svg)