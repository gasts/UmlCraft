# Changelog

What changed from release to release. The [README](README.md) describes what UmlCraft can do
today.

## Unreleased

- **New app icon**: an initial state with its transition into a state, as in a state machine
  diagram. `umlcraft.exe` now carries it in Explorer, the taskbar and its windows, and the
  macOS app shows it in the Dock and Finder.
- SVG and PNG images fill elements white, as the diagram shows them on screen, instead of
  Astah's yellow.
- Texts in SVG images keep even letter spacing in PDFs made with Word.
- Lines to a use case end at its ellipse, not at the rectangle around it.
- «include» and «extend» stand beside their line instead of on it, and can be dragged to
  another place on the diagram. A note anchored to the line keeps them on the other side.
- Use case names too wide for their ellipse wrap at spaces; the ellipse grows in height to
  hold them.
- The model tree and lists in the properties panel show names with line breaks on one line.
- «include» and «extend» from `.asta` files stay where Astah drew them, and names, roles and
  multiplicities of lines to use cases no longer shift on import.
- **System boundaries as in Astah**: a thin rectangle without fill, the name in regular type.
  Astah has no system boundary of its own; the rectangle with a text at its top that Astah
  users draw instead is imported as a boundary, and boundaries are exported that way.

## 0.6.0 – 2026-10-26

- **Linux packages:** besides the `.tar.gz`, each release has a `.deb` (Debian, Ubuntu), an
  `.rpm` (Fedora, openSUSE), an AppImage, a Flatpak and a Snap. They add UmlCraft with its icon
  to the application menu and open `.ucraft` files on double-click; the `.deb` and `.rpm`
  install the libraries UmlCraft needs. The Flatpak and the Snap cannot run the `.asta` export.
- **Use case diagrams redrawn**: bold names and 2 px ellipses; extension points under a line,
  left-aligned; use cases sized to their name and points; actors as a stick figure that keeps
  its size in larger boxes. The frame of a use case diagram reads `uc` and the diagram's name,
  and frames of use case diagrams in `.asta` files come along on import.
- **Notes on lines**: the note anchor connects a note to an association, include, extend or
  any other relationship, ending in the middle of its line. Imported from and exported to
  `.asta` files.
- Extension points can be added from a use case's context menu on the diagram.
- An extend's condition is no longer drawn at its line.
- Fixed in the `.asta` export: public attributes became private, and role names and
  multiplicities placed by hand went to the wrong end of an association whose line runs from
  its second end.

## 0.5.0 – 2026-10-25

- **Python import** (Tools › Python › Import Python files … / Import Python folder …): reads
  `.py` and `.pyi` files as modules in their packages: classes (`Protocol` as interfaces,
  `Enum` as enumerations), annotated attributes and those `__init__` assigns to `self`,
  methods with their decorators, docstrings and template parameters. Attributes without an
  annotation get no type; typed containers and `X | None` make associations. Names resolve
  through imports and what a package's `__init__.py` imports.
- **C++ import** (Tools › C++ › Import C++ files … / Import C++ folder …): reads declarations
  from headers and sources: namespaces, classes, structs, unions and enumerations with fields,
  member functions, constructors, operators, template parameters, base classes and `///`,
  `//!` and `/** */` documentation. Pure virtual functions are abstract, `const` ones queries.
  Members defined outside their class join it, typedefs and `using` aliases stand for their
  targets, and standard containers and smart pointers make associations. Of `#if`/`#ifdef` …
  `#else`, only the first branch is read; files that use macros in declarations are reported
  and left out.
- **C# import** (Tools › C# › Import C# files … / Import C# folder …): reads declarations from
  `.cs` sources into the open project as the Java import does: namespaces, classes, interfaces,
  structs, enumerations and records with fields, methods, constructors, operators, template
  parameters, base classes and interfaces, and `///` documentation. Auto-properties become
  attributes, other properties and indexers accessor operations (`get_Count`, `set_Count`);
  partial classes become one class. Library types become placeholders at their namespace path,
  fields typed by a model type become associations (`Order?` with `0..1`). Of `#if` … `#else`,
  only the first branch is read.
- **Java import**: importing again no longer depends on the order in which library types were
  first seen when two on-demand imports offer the same name.
- **Object diagrams**: objects `name : Class` on class diagrams with one slot per attribute of
  the class, inherited ones included, edited inline or in the properties panel
  (`name = value`); links between objects with roles, navigability and aggregation, optionally
  as an instance of an association. Imported from and exported to `.asta` files.
- **Orthogonal states**: a state can be divided into regions, one below the other or side by
  side (properties panel); the lines between them can be dragged. **Internal transitions**
  (`trigger [guard] / effect`) are listed in the state below entry, do and exit and edited
  inline or in the properties panel. Both are imported from and exported to `.asta` files; the
  export keeps two regions per state, as the API allows no more.
- States dragged into another state or one of its regions become part of it, and dragged out
  of it, part of the state machine again. New states land in the region they are placed in.
- **Object nodes in a state**: `order : Order [filled]` shows the state in a line of its own
  below the name; type and state are set in the properties panel. Imported from and exported
  to `.asta` files.
- Transition labels, flow guards and `<<include>>`/`<<extend>>` are placed anew: names centered
  below the middle of the line, `<<include>>` and `<<extend>>` right above it.

Projects with slots, links, regions or internal transitions cannot be opened by UmlCraft 0.4.0
and older. Older versions open other projects of this version, but drop the state of object
nodes when they save.

## 0.4.0 – 2026-10-24

- **Java import** (Tools › Java › Import Java files … / Import Java folder …): reads declarations
  from `.java` sources into the open project: packages, classes, interfaces, enumerations,
  records and annotation types with attributes, operations, template parameters, supertypes and
  Javadoc. Library types become placeholders at their package path (`java › util › List`),
  fields typed by a model type become associations. Method bodies are skipped, files with
  syntax errors are reported, and importing again updates the model instead of duplicating it.
  The whole import is one undo step. New crate `umlcraft-reverse`, built to take C# and C++
  later.
- **Class diagrams**: template parameters (type and value parameters with bounds and defaults)
  in a dashed box, bound templates such as `Box<Holder, 20>`, qualifiers in a box at the
  association end, static ends underlined, leaf classes and operations, visibility symbols in a
  column of their own, four line styles for edges (straight or curved, each also right-angled).
  All of it imported from and exported to `.asta` files.
- Deleting and undoing large edits is much faster (undoing an import of 225,000 elements takes a
  moment instead of minutes).
- The toolbar and shortcuts such as Ctrl+Z work right after start and after a menu or dialog
  closed, without clicking into the diagram first.

Projects that contain template parameters or bound types such as `List<Order>` (the Java import
creates them) cannot be opened by UmlCraft 0.3.0 and older. Older versions open other projects
of this version, but drop qualifiers and the new details of association ends when they save.

## 0.3.0 – 2026-10-24

- **State machine diagrams**: states with entry, do and exit, composite states with substates,
  final states, initial, choice, junction, fork, join and history pseudostates, transitions
  labeled `trigger [guard] / effect`. Imported from and exported to `.asta` files.
- **Component and deployment diagrams**: components, artifacts and nodes with their icons,
  objects, component and node instances (`name : Type`), dependencies, realizations and
  associations; nodes take what is deployed on them along. Imported from `.asta` files;
  exported to them as class diagrams, because the API has no editors for them.
- **Sequence diagrams redrawn**: stored and drawn with the geometry of `.asta` files (the "sd"
  frame, lifeline heads and lengths, activation bars, message heights and numbers, fragment
  frames), so imported diagrams keep their layout. Interaction uses ("ref" frames) and found
  and lost messages. Files from earlier versions are laid out once when opened.
- **PNG export** of the open diagram, at twice the canvas size.

## 0.2.0 – 2026-10-23

- **Activity diagrams**: actions, initial, activity final and flow final nodes, decision, merge,
  fork and join, object nodes, flows with guards, partitions (also nested). Imported from and
  exported to `.asta` files, whose UML 1.4 activity graphs are translated.
- **SVG export** of the open diagram.
- The `.asta` export is always offered and explains what is missing when no Astah is installed.

## 0.1.0 – 2026-10-23

The first release: the editor with class, use case and sequence diagrams, and `.asta` import
and export.

- Model core with validated, invertible operations and transactions (undo/redo); JSON project
  format `.ucraft`.
- Editor: menu bar, diagram tabs, tool palette per diagram type with connection rules,
  rubber-band and multiple selection, resizing, alignment, zoom to fit, properties panel, model
  tree with context menu and drag and drop, copy/cut/paste, waypoints, light and dark theme,
  German and English.
- Class diagrams with inline editing (`+ name : Type [0..*] = init`, `+ op(p : T) : R`),
  association ends, self-associations, hidden compartments and interfaces as circles.
- Use case diagrams with system boundary, extension points, include and extend.
- Sequence diagrams with synchronous, asynchronous, reply, create, destroy and self messages,
  activation bars and combined fragments.
- Import of Astah `.asta` files and export to them through an installed Astah.
