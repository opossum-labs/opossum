# Saving Options

Saving options in OPOSSUM define how files generated during the workflow are stored and managed. These settings determine where project files and analysis data are saved.

The selected saving configuration is stored and reused, allowing OPOSSUM to apply the same settings in future sessions without asking the user again.

## General Concepts

OPOSSUM uses a configuration file to store the selected saving options. The configuration contains the report directory, which defines the base location for storing project files and analysis data.

The general file repository can be deleted before starting an analysis. No leftover files are expected to remain after cleanup.

## Configuration and Report Directory

The configuration file is located under:

```text
C:\Users\<username>\AppData\Local\opossum labs\opossum\config


The report directory setting in the configuration defines the base directory where project files and analysis data are stored.

Each project file (.opm) is stored within this base directory. OPOSSUM creates a folder named:

opossum reports

inside the selected base directory. This folder contains the analysis data generated during the workflow.

Each analysis run is stored in a separate subfolder. For example, folders such as 1 and 2 can be created for different analysis runs, with each folder containing the corresponding analysis data.

If multiple analysis folders exist within the same base directory, the project name is used to identify the corresponding project. For an unsaved project, the default project name is:

default

If the project is saved with a specific name, the project file is stored using that name. For example, saving the project as test creates a project file named test.

The report directory should be selected carefully. Selecting a very broad directory, such as the complete C:\ drive, can result in files being  overwritten,or deleted in that location.