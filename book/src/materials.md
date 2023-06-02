# Using optical materials

As discussed, almost all optical components (except ideal ones) consist of one or more materials. Often for simulations, several material parameters must be known. Since the same material data is shared between different nodes, an infrastructure for handling material parameters is needed. A simple approach could be a text file.

Preferable however is a database that would also allow for shared access between different users from a central server. A common materials database would thus reduce the effort of adding new materials. A draft of the database layout could be as follows:

![The database layout](./images/Materials%20database.png)

This structure allows for an arbitrary number of material parameters to be added with a strictly defined data format. Let's describe the structure in detail:

## Table: materials

This is the central table storing the materials with their name and a reference to a material type (see next section). The actual data table (materialdata) refers to this table for assigning a property(value) to a certain material.

## Table: materialtypes

This is a simple table storing different material types such as "glass", "metal", "gas", "crystal" etc. The purpose is simply to provide a filter to all materials while browsing a (possible) long list of materials. Each material refers to an entry of this table. This means the material belongs to a certain material type.

## Table: properties

This table defines the property names such as "refractive index" or "manufacturer" together with a description field. These properties are connected with one or more data types (see next section) which represent the property value type.

## Table: datatypes

The data type defines the way the value of a property is represented. This could be a value such as "numeric", "string", "2dData" etc. This information defines how the actual data fields (defined in materialdata) have to be interpreted or parsed.

## Table: datasources

This table stores information about where the data was taken from (website, info from the manufacturer, own measurement, etc...)

## Table: proptypes

This is the connecting table for relating a given property to a set of data types.

## Table: materialdata

This table stores the actual material properties.

# Materialdb software

As a first project, the materialdb software has been developed. This software consists of a [backend](https://git.gsi.de/phelix/rust/materialdb_backend) and a [frontend](https://git.gsi.de/phelix/rust/materialdb_frontend) part.

The backend is written in Rust using the `seaorm` package for database handling as well as the `rocket` web framework for the development of a web API. Furthermore, this crate also contains basic functions for accessing the database (read-only so far) from the node system to be developed. For viewing / editing the database a frontend package written in Angular was developed.