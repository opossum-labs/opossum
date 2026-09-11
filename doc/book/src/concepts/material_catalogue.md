# Material Catalogue

The Material Catalogue provides a library for managing materials. A material can be added to the library using the green **Add New Material** button on the right side of the Material Catalogue. The catalogue contains the material's name, manufacturer, description, refractive index (nd), latest version, and available actions. It also provides **Search Name / Manufacturer**, **Min nd**, and **Max nd** for finding and filtering materials.

## Adding materials to the catalogue

A new material can be added to the library by pressing the green **Add New Material** button on the right side of the Material Catalogue. When adding a material, a name and refractive index can be provided. If no name is given, the material is assigned the default name **New Material**.

Each material in the catalogue has the options to **Edit**, **Delete**, and **Publish New Version**. Editing allows the material properties to be changed, while deleting removes the material. **Publish New Version** is used when changes to the properties of a material are published as a new version.

## Searching and filtering materials

The Material Catalogue provides **Search Name / Manufacturer** for finding materials using their name or manufacturer. Materials can also be filtered using **Min nd** and **Max nd**, which specify the minimum and maximum refractive index values.

## Material selection for nodes

When a node requires a material, such as a lens, the **Lens Material** property is available in the node properties. The material can be defined locally or selected from the Material Catalogue.

The **Ad Hoc** option is used when the material is required locally and should not be stored in the library. An Ad Hoc material can be used in the GUI for a single use.

The properties of a local material can be edited using the **Material Editor**. After the local material properties have been changed, the changes can be saved.

Another option is to **replace the material with an existing catalogue material**. In this case, a material can be selected from the Material Catalogue library. When a particular material is selected from the catalogue, the version of the selected material is shown. A different material can also be selected from the catalogue.

A material selected from the catalogue can also be **detached from the library and converted into a local copy**.

## Material versions

Materials in the catalogue can have different versions. If changes are made to the properties of the first version of a material, the changes can be published as the second version.

Once the new version has been published, the older version cannot be used or accessed.

The version that is currently being used in the GUI does not change automatically when a new version is published. The version already being used in the GUI remains unchanged unless the user intentionally changes it to the newly published version.
