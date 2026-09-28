# Material Catalogue

The Material Catalogue provides a library for managing materials. A material can be added to the library using the green **Add New Material** button on the right side of the Material Catalogue. The catalogue contains the material's name, manufacturer, description, refractive index (nd), latest version, and available actions. It also provides **Search Name / Manufacturer**, **Min nd**, and **Max nd** for finding and filtering materials.

n_d refers to the refractive index of an optical material measured at the yellow sodium D-line wavelength of approximately 587.56 nm.

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

The version information was introduced to allow optical models to refer to a specific (older) version of a material definition. This way, (community) updates to the material catalogue do not break older models. In the future there will be a mechanism to optionally update material references in optical nodes to the most up to date catalogue version.

The version that is currently being used in the GUI does not change automatically when a new version is published. The version already being used in the GUI remains unchanged unless the user intentionally changes it to the newly published version.

## Asset Catalog Synchronization

The **Asset Catalog Synchronization** feature allows the material versions used in an Opossum model to be synchronized with the corresponding versions available in the Material Catalogue.

When a new material or a new version of an existing material is added to the catalogue, the model can be synchronized with the updated catalogue information. The synchronization functionality is available near **Add New Material**.

Synchronization can be performed in both directions:

- **Catalogue → Model:** The material used in the model can be updated to a corresponding version available in the catalogue.
- **Model → Catalogue:** The catalogue can be synchronized with the material version defined in the model.

This allows users to explicitly manage differences between the material version referenced by a model and the version available in the catalogue, rather than changing the material reference automatically when a new catalogue version is published.

Opossum also provides a shared opossum Catalog, which serves as the standard asset catalogue for Opossum.

The catalogue can be updated from the shared repository using **Pull Updates from Git** on the main Opossum page. This retrieves the latest catalogue updates and makes newly added materials and material versions available for synchronization with models.
