# OpticScenery

The `OpticScenery`struct contains the entire model and possibly further metadata (e.g. such as a `description` field). It should also implement functions serialization / deserialization (which will be forwarded to the corresponding functions of the struct members) which allow for writing / reading an optical system to / from a storage medium.

In addition, it should implement the top-most function to export the graph to the graphwiz foramt for easy visualization. Last but not least it also conatins the top-level `analyze` function starting the acutal analysis process. The type of analysis has to be given here making use of the well-known strategy pattern.
