# Gesamtkonzept: Material- und Komponentensystem für OPOSSUM

## 1\. Einleitung und Zielsetzung

Dieses Dokument beschreibt die Architektur für ein neues Material- und Komponentensystem innerhalb der Optiksimulationssoftware OPOSSUM (opossum\_core, opossum\_cli, opossum\_backend, opossum\_gui).  
Ziele des Systems sind die Modellierung von physikalischen Eigenschaften (Brechungsindex, Absorption, Doppelbrechung etc.) sowie die Integration vorkonfigurierter Katalogkomponenten (z. B. Herstellerlinsen). Dabei stehen die strikte Reproduzierbarkeit wissenschaftlicher Daten, Erweiterbarkeit, Portabilität von Designdateien und Nutzerfreundlichkeit im Fokus.

## 2\. Datenhaltung und Registry-Architektur

Anstelle einer starren relationalen Datenbank wird ein **Registry-System nach dem Vorbild von Cargo (crates.io)** verwendet.

* **Dateiformat:** Materialien und Komponenten werden als Einzeldateien im **RON-Format** (Rust Object Notation) gespeichert. Dies unterstützt optikspezifische Werte wie NaN oder Infinity nativ und lässt sich optimal in das Rust-Ökosystem integrieren.  
* **Versionskontrolle & Verteilung:** Die globale Registry wird über ein öffentliches Git-Repository gepflegt. Endanwender synchronisieren dieses über in OPOSSUM integrierte Git-Routinen (z. B. via git2 Crate), ohne Git selbst bedienen zu müssen.

## 3\. Identifikation und strikte Versionierung (Append-Only)

Um die wissenschaftliche Reproduzierbarkeit von Simulationen zu garantieren, dürfen sich einmal referenzierte Materialdaten niemals nachträglich ändern. Ein Verweis auf einen globalen Git-Commit-Hash ist hierfür unzureichend, da dieser alle Materialien in der Zeit einfriert.

* **Globale Identifikation (UUID):** Jedes Material und jede Komponente wird über eine eindeutige, unveränderliche **UUID** identifiziert. Der Name (z. B. "N-BK7") ist ein veränderliches Metadatum.  
* **Append-Only Prinzip:** Die Datenbank folgt einer "Nur-Hinzufügen"-Strategie. Bei Modifikationen an physikalischen Eigenschaften wird die alte Datei nicht überschrieben. Stattdessen wird eine neue Version angelegt.  
* **Verzeichnisstruktur:** Jede UUID erhält einen eigenen Ordner, der die verschiedenen Dateiversionen enthält (z. B. materials/550e.../v1.ron und materials/550e.../v2.ron).  
* **Referenzierung:** Ein Design verweist stets auf eine Kombination aus UUID und exakter Version.

## 4\. Validierung und Community-Beiträge (Single Point of Truth)

Nutzer sollen neue Materialien oder Fehlerkorrekturen komfortabel über die GUI einreichen können. Um die Integrität der Datenbank zu schützen, findet die Validierung zentral statt.

* **Backend als Gatekeeper:** Wenn ein Nutzer ein Material einreicht, sendet die GUI die Daten an das opossum\_backend.  
* **Lokale Validierung:** Das Backend nutzt exakt dieselben Rust-Strukturen (structs) wie opossum\_core, um das eingereichte RON-Format zu parsen und auf logische Fehler (z. B. Brechungsindex \< 1.0) zu prüfen.  
* **Automatischer Pull Request:** Nur bei erfolgreicher Validierung nutzt das Backend die GitHub-API (z. B. via octocrab), um im Namen des Nutzers (mit dessen Token) einen Pull Request im offiziellen Katalog-Repository zu erstellen. Fehlerhafte Daten erreichen das Repository somit gar nicht erst.

## 5\. Suchfunktion und Performance (In-Memory Index)

Um bei Tausenden von Einzeldateien eine performante Suche in der GUI zu gewährleisten, wird ein Suchindex verwendet.

* **Index-Aufbau:** Beim Start von OPOSSUM (oder nach einem Registry-Update) iteriert das Backend einmalig über alle .ron-Dateien.  
* **Trennung von Metadaten und Payload:** Es wird ein leichtgewichtiger Index im Arbeitsspeicher erstellt, der nur die für die Suche relevanten Felder enthält (UUID, höchste Version, Name, Hersteller, Kategorie).  
* **Suche:** Nutzeranfragen (z. B. "Alle Gläser von Schott") filtern blitzschnell diesen RAM-Vektor. Die vollständigen, speicherintensiven physikalischen Daten werden erst beim tatsächlichen Einbau in das Design (Lazy Loading) geladen.

## 6\. Portabilität von OPM-Dateien (Offline-Fähigkeit)

Eine .opm-Datei muss deterministisch und portabel sein. Das Design darf nicht fehlschlagen, wenn der Empfänger der Datei eine andere lokale Datenbank besitzt.

* **Das Container-Prinzip (Vendoring):** Das OPM-File fungiert als autarkes Archiv.  
* **Eingebettete Abhängigkeiten:** Neben dem Design-Graphen enthält das OPM-File die Bereiche embedded\_materials und embedded\_components. Hierin werden exakte, lokale Kopien der verwendeten Materialien (in der spezifischen Version) und Komponenten zur Zeit der Speicherung hinterlegt.  
* **Update-Mechanismus:** Beim Laden eines Designs vergleicht die Software die Versionen der eingebetteten Materialien mit dem Index der globalen Registry. Liegt im UUID-Ordner der Registry eine neuere Version (z. B. v3.ron statt der eingebetteten v2.ron) vor, kann die GUI ein optionales Update anbieten.

## 7\. Komponentenarchitektur im Design-Graphen

Katalogkomponenten müssen im Design als solche erkennbar bleiben, um Updates oder spezifische GUI-Darstellungen zu ermöglichen.

* **CatalogNode:** Es wird ein neuer Node-Typ eingeführt, der die Katalogkomponente als logische Einheit kapselt. Er referenziert die Komponenten-UUID (inklusive Version) und speichert nutzerspezifische Einstellungen (z. B. Translation, Rotation).  
* **Proprietäre Materialien:** Benötigt eine Katalogkomponente ein spezielles Material, das nicht in der Standard-Registry auftauchen soll, wird dieses regulär über das UUID/Version-System referenziert, in der Registry aber mit einem Flag (z. B. is\_vendor\_specific) versehen, sodass es in der allgemeinen Suche ausgeblendet wird.
* **Möglichkeit zr Konversion:** Es wird eine Möglichkeit angeboten, eine CatalogNode in ein oder mehrere Standard Nodes zu konvertieren. Diese können dann weiter bearbeitet werden, sind aber nicht mehr mit der Datenbank verbunden.
