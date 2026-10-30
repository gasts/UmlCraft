# Architecture

## Model and view

Like UML, `umlcraft-model` separates **elements** from **presentations**:

- An `Element` (class, association, lifeline, diagram …) has an owner and ordered children. The
  root is exactly one `ElementKind::Model`. `ElementKind::can_own` defines which kind may own which.
- References between elements (`ElementKind::references`) have a role (`RefRole`). **Strong**
  references, such as the ends of an association, delete the referencing element along with the
  target. **Weak** references, such as attribute types, are cleared; a type then becomes free text
  holding the old name.
- A type (`TypeRef`) is a classifier or template parameter, free text, or an expression built from
  other types (`TypeExpr`: base, template arguments, a prefix such as `? extends ` and a modifier
  such as `[]` or `*`). In `List<Foo>[]` both `List` and `Foo` stay references of their own;
  deleting one turns only that part into text. `member::parse_type` reads the written form and
  links what it finds, as seen from the member: template parameters of the member's owners
  first, then qualified names (`java.util.List`), then a classifier whose name is unique.
- Template parameters (`ElementKind::TemplateParameter`) belong to classes, interfaces, data types
  or operations and stand for a type (with bounds) or a value (with its type, C++'s `int N`),
  optionally with a default and variadic.
- Every diagram element has a `View` with presentations in z-order. A node has `bounds`; an edge
  connects two presentations of the same view. Free texts (`Shape::Text`) and free lines
  (`Shape::Line`, dashed and with arrowheads as chosen) show no element.

## Changes

The model changes only through `Model::apply(Op) -> inverse Op`. Every operation is validated
(ownership rules, reference targets, cycles in generalizations and in the ownership tree, attached
edges).

`Document::edit(label, |tx| …)` groups operations into one undo step. If the closure fails,
everything is rolled back. `Tx` offers the composite edits:

- `delete` deletes in cascade: subtree, strongly referencing elements, presentations and attached
  edges.
- `remove_presentation` removes something from the diagram only.
- `set_bounds`, `bring_to_front` etc. change presentations.

`Document::is_dirty` compares the topmost undo step with the save point.

## File format

`.ucraft` is pretty-printed JSON. Elements are sorted by ID (UUIDv7, i.e. by creation time), so new
elements end up at the end. Ownership is stored only through the ordered `children`. Default values
are omitted. `formatVersion` and the `MIGRATIONS` list in `umlcraft-format` handle future format
changes. On load, `Model::check` verifies all invariants. Version 2 added the geometry of
sequence diagrams; version 1 files load unchanged and their sequence diagrams are laid out once
when the app opens them (`sequence_edit::upgrade_document`, not an undoable edit; `load_versioned`
tells the app the file's version). Version 3 keeps the tagged values of languages under
UmlCraft's keys (`java.`, `csharp.`, `cpp.`); the migration renames Astah's `jude.` keys.

## User interface

`umlcraft-app` uses `gpui-kit` 0.8, which brings the matching GPUI release (`gpui-pre` 0.3.9) and
`gpui-component`:

- `Workspace` (a GPUI entity) holds the document, file path, open diagrams (tabs), selection and
  the active tool. All edits from the UI go through its methods. Each of them ends by fitting
  the classifier boxes sized to their content (`Style::auto_size`) whose texts the edit changed
  (`scene::fitted_box`); for that the workspace measures
  with the app's font system (`AppMeasure`, the advances summed like Java's). Resizing a box by
  hand ends its automatic size.
- `canvas.rs` paints the scene's primitives (`umlcraft_notation::draw`) and texts with
  `paint_quad`/`PathBuilder`/`shape_line` and handles pan and
  zoom itself, separately for each diagram. Drag operations (move, resize, rubber band, connect)
  are shown only as a `Preview` until the mouse is released and are then applied as one undo step.
  A right click selects what it hits and opens the context menu: for an association the
  navigability, aggregation and multiplicity per end, for edges the line style, ends back to
  the centers and labels back to their default places, then deleting.
- `properties.rs` commits text fields on Enter or when the field loses focus, always to the element
  that was shown when the field got focus.
- `main_view.rs` lays out the window and holds the file operations: the title bar (gpui-component's
  `TitleBar` with the File menu, save, undo and redo; the window draws its own title bar), the
  ribbon above three cards for model tree, canvas and properties, and a status
  line. The platform menu bar (`cx.set_menus`) stays for macOS.
- `ribbon.rs` describes the ribbon as data (tabs, groups, large and small buttons, each with an
  action) built on every render from a `ribbon::State` read once from the workspace, and draws
  it. The diagram tab holds the palette of the active diagram's kind and opens by itself when
  another diagram becomes active. Where the estimated width does not fit, buttons drop their
  labels; a double click on a tab folds the ribbon (`ribbonCollapsed` in the settings).
- `shell.rs` has the opaque backdrop the cards float on, the card style and icon buttons
  (`in_title_bar` keeps a press on a title bar button from moving the window on Windows). A
  system backdrop (Mica, macOS blur) was tried and dropped: with a non-opaque window GPUI
  renders text without subpixel antialiasing, and on Windows its antialiased edges come out
  too dark over the backdrop. `icons.rs` embeds
  `assets/icons/` (`AppAssets`, falling back to gpui-kit's Lucide icons) and maps tools, diagram
  kinds and model elements to their icons.
- `png.rs` rasterizes the SVG export (`umlcraft_notation::svg`) with `resvg` for PNG export, at
  twice the canvas size and with the system's fonts, on a background thread.
- `settings.rs` stores recent files, language, theme and the visibility column
  (`visibilityColumn`) in `UmlCraft/settings.json` in the platform's settings folder (`%APPDATA%` on Windows,
  `~/Library/Application Support` on macOS, `$XDG_CONFIG_HOME` or `~/.config` elsewhere). The column reaches layout and
  drawing through `TextMeasure::visibility_column` of the measures the canvas and the
  workspace use. `settings_dialog.rs` shows language, theme and the column in a dialog
  (`gpui-component`'s `Dialog`), which applies each change at once through `MainView`; for the
  column it sets the canvas's flag and gives the workspace a new measure.
- Texts come from `locales/{de,en}.yml` (rust-i18n). Without a setting, the system language is used.

## Notation

`umlcraft-notation` knows UML notation but no UI:

- `scene` turns a view into shapes in logical coordinates. Each node carries its text rows (`Row`)
  with position, font and the element the row edits. Painting, hit testing and inline editing share
  the same layout. Nodes grow with their content; text width is measured by a `TextMeasure` (the
  real font in the app, an estimate in tests). `scene` also places edge labels and provides
  resize handles, edge handles (waypoints), rubber-band selection and alignment. Containers (system
  boundaries, packages, states, components) graphically contain whatever lies fully inside them (`with_contents`);
  system boundaries are "hollow" and clickable only on their border and title.
- `route` runs edges in the line styles of `.asta` files (`Style::line_shape`): straight through the
  waypoints, at right angles with the waypoints as corners, or as one cubic Bézier curve whose
  control points are the first and the last waypoint. Right-angled lines are fitted when drawn,
  so their first and last segment stay straight while nodes move or grow. Without waypoints
  they get Astah's default corners: none when the ends are less than 2 apart across (the line
  then runs slightly slanted), else a step halfway between the nodes. The drawn corners are
  what gets dragged (a segment moves across, the end segments slide their anchor inside the
  node) and exported. Changing the style changes the route (`scene::restyle`):
  "line" drops the waypoints, right angles turn them into corners.
- `member` produces and parses the text form of attributes and operations and writes it back to
  the model; known classifiers become real type references.
- `object` derives an object's slots from its classifier: one per attribute, inherited ones
  included (the generals' first, depth first, then its own). Only slots with a
  value are stored, as `Slot` children of the object that refer to their attribute; deleting the
  attribute deletes them. So slots follow the class when attributes are added, renamed or
  removed. An object's box shows `name : Class` underlined, and below it
  always a slot compartment, 8 high when empty. Links are `Link` elements with two ends
  (instance, role, navigability, aggregation) and optionally the association they instantiate;
  they are drawn like associations, with plain role names.
- `clipboard` copies the selection including subtrees and the relationships among them, and pastes
  it as a duplicate (new IDs, free names) or as another presentation of the same elements.
- `palette` defines which tools a diagram type offers, which connections are allowed (e.g.
  realization only from class to interface) and where new elements land in the ownership tree.
- `sequence` draws sequence diagrams, `sequence_edit` edits them (see below).
- `draw` turns a scene's figures and end markers into a few primitives (polyline, polygon,
  ellipse, frame, curve) with color roles (`Paint`) and says where text baselines go. The canvas paints
  them with its theme colors (and a node's own fill color on a light background); `svg` writes
  them as the canvas shows them in the light theme: black lines, white where a node has no color
  of its own, a margin of 10 and 4/3 pixels per unit. The app's image export writes coordinates
  and sizes ten times larger (`render_scaled`, same image size): Word's PDF export puts the
  letters of SVG text on whole units, which at 12 units per letter height spaces them unevenly. So the figure geometry exists once,
  for both. Texts in nodes are centered by Java's whole-pixel width or the exact one, whichever
  is larger (`TextMeasure::astah_width`, with the tables for 12 and 14 px in `metrics`), so imported diagrams keep their text
  positions, and written at a fixed x. Message labels are centered by that width too; those
  moved by hand get the offset Astah's stored box implies on import.

## Sequence diagrams

Sequence diagrams store their whole geometry, as `.asta` files do, so that they import from and
export to `.asta` and back without change:

- The diagram frame ("sd" and the name in a tab) is a node presentation of the diagram itself.
- A lifeline is `Shape::Lifeline`: its head (a box, or with `Style::iconic` the icon of the
  represented classifier's robustness stereotype) and the length of its dashed line, which can
  end in a cross (`destroyed`).
- An activation bar is `Shape::Activation`: a lifeline presentation, a top and a height. Its x
  follows from the lifeline and from how many bars on it it lies within (10 further right each).
- A message is `Shape::Message`: the two lifeline presentations, the height where it leaves the
  sender and, for messages to self and messages that branch off another one, where it arrives.
  Found and lost messages keep the x of their open end (`gate_x`); on the frame's border it is a
  gate without a dot. The ends meet the bars at those heights.
- A combined fragment is `Shape::Fragment` with its bounds, the offsets of the operand
  separators and hand-placed guard offsets; an interaction use is a node presentation.
- Message numbers ("2.1") are not stored: a message sent from a bar is numbered below the call
  that started the bar; messages leaving the same point branch ("-A", "-B"). A reply's return
  value is shown after the call it answers. Both can be switched off per diagram
  (`DiagramProps::hide_message_numbers`, `hide_return_values`).

The interaction's order follows the drawing: `sequence_edit::tidy`, run after every edit of a
sequence diagram, grows bars to end 20 below what lies in them and lifelines to reach 30 below
what is on them, grows the frame around everything, and puts every message and fragment into the
operand of the innermost fragment drawn around it, ordered by height (`sync_order`). Edits follow
the rules measured through the `.asta` export API: a call gets a bar of 25 on the receiver (a create message none) and, where the
sender has none, a bar of 45 on the sender (60 for create, 30 for lost messages); a reply leaves
at the end of the bar it returns from; a message to self comes back 20 lower into a bar of its
own; a create message brings the new lifeline's head down to it; a destroy message ends the
lifeline in a cross below its bar. Dragging a message moves it with the bar it starts and
everything sent from that bar (`move_item`); a message's label can be dragged on its own
(`place_label`). Combined fragments grow to hold the bars and messages that start in them, and
bars that no message reaches any more are removed. The layout's `Slot`s carry the height the
user pointed at.

## State machines

A state owns its substates directly, or, as an orthogonal state, `Region` elements that own
them; internal transitions (`InternalTransition`, `trigger [guard] / effect`) are children of
the state and listed in it below entry, do and exit. Which region a substate is drawn in is up
to the presentation: a state's node keeps the layout of its regions (`Regions`: one below the
other or side by side, and the lines' distances from the top or left edge, as `.asta` files
keep them).
Lines it lacks divide the room after the last one evenly, and every region keeps at least 20.

`state_edit` keeps the model in line with the drawing: a vertex moved on the canvas
(`reparent_vertices`) or created there belongs to the innermost state drawn around its center,
or to the region of it that holds the center, else to the state machine; a state never goes
into itself or what it holds. Dividing a state the first time makes two regions and puts its
substates into the one they are drawn in; further regions halve the last one. Moving a line
between regions moves the substates on the other side into their new region; deleting a region
deletes what it holds and gives its room to the neighbor before it.

## .asta import

`umlcraft-asta` reads `.asta` files without any Java: `jser` is a generic reader for the Java
object serialization stream and turns it into a graph of objects addressed by class and field
name; `map` translates that graph into a new `Model`, element by element through `Model::apply`,
so every invariant is checked. What has no counterpart in UmlCraft is skipped and counted
(`Imported::skipped`). Template parameters, bound templates (as type expressions), array
modifiers, qualifiers and the tagged values of Astah's language profiles are imported too.
Objects bring their slot values (`UAttributeLinkImp`, whose value is an instance named by the
value text); links (`ULinkImp`) have no owner in Astah and go into the package of their first
object, their ends' navigability and aggregation come from the tagged values
`object_navigable_type` and `object_aggregation_kind`.
The format details are kept in local notes (`docs/asta-format.md`) that are not published.
Two examples help when exploring files: `asta-dump` (raw objects) and `asta-import` (the
resulting model).

## Source import

`umlcraft-reverse` reads declarations from source code into the open project, so that the
result matches what a Java import into an `.asta` project gives (see the comparison test below):
types, members, template parameters, supertypes and
documentation, but no method bodies and no diagrams. It reads Java, C#, C++ and Python: a
front-end per language lowers its syntax to a language-neutral IR (`ir`), and one
mapping for all languages puts the IR into the model.

- **Front-ends** (`lang::Frontend`) parse with tree-sitter. They resolve nothing; they lower
  language sugar instead (Java's implicit `public abstract` in interfaces, records; C#'s
  properties, which become attributes or accessor operations as .NET names them, `get_Count`).
  C++ adds members defined outside their class (`ir::Item::Definition`, `void A::f() {}`) and
  typedefs (`ir::Item::Alias`, `ir::Member::Alias`), which the mapping resolves. A Python file
  is a module: `parse` gets its module path (`source::python_module`, from the folders with an
  `__init__.py`), and what has no annotation gets the type `ir::TypeName::Unwritten`, which the
  mapping leaves without type and association. What they
  leave out (method bodies, local and anonymous classes, C# attributes, events, C++ macros,
  specializations, friends …) is counted (`lang::Skip`). A file with a syntax error is reported
  with line and column and left out; the others are imported. `source` collects files
  (recursively, sorted) and decodes them: a byte order mark, else UTF-8, else Windows-1252,
  which the report mentions. The C# and C++
  front-ends read only the first branch of `#if`/`#ifdef`/`#ifndef` … `#else`
  (`preprocessor`): the other branches are blanked out before parsing, keeping every byte's
  place, so conditionals that cut a declaration apart still parse. Macros are not expanded, so
  a C++ file that uses one in a declaration (`class API Name`, `int x GUARDED_BY(mu);`) fails
  to parse and is reported.
- **Rules** (`lang::Rules`) are what the mapping needs to know about a language: built-in
  types, implicit imports, value types, how to split a qualified name, and the standard
  library (`lang::Library`, read from a table by `library::TableLibrary`). For Java that is a
  table of the JDK's public types (`java/jdk.txt`, written by `tools/jdk-types/JdkTypes.java`)
  with their kind, type parameters and whether they are collections or maps; for C# a table
  kept by hand of the base class library's containers and value types (`csharp/bcl.txt`); for
  C++ one of the standard containers and smart pointers (`cpp/std.txt`, `unique_ptr` owns
  exactly one); for Python one of `builtins`, `typing` and `collections` (`python/stdlib.txt`).
  Where languages differ in how names resolve, a rule says so instead of the mapping asking
  which language it is: C# looks in the namespaces around a type's own
  (`enclosing_namespaces`), its base list does not say which entries are interfaces
  (`base_list_by_target`: a known target's kind decides, the front-end's guess by C#'s rules
  only gives unknown ones their kind), and
  `Order?` (C#) and `Order*` (C++) are optional (`optional_modifiers`); qualifiers such as
  C++'s `const ` that associations look through are `type_qualifiers`.
- **Resolution** (`resolve`) links names in Astah's order (template parameters, member types,
  imports, own package, on-demand imports, `java.lang`). Names that stand for nothing in the
  model become placeholder types: library types at their package path (`java › util › List`,
  `System › Collections › Generic › List`), qualified names under their path, unknown simple
  names in the root (Astah's default package). An alias (`using M = A.B.Map;`) is a single-type
  import by another name; a qualified name that starts with what a single import makes visible
  continues the import's path (`Gen.List` after `using Gen = System.Collections.Generic;`).
  With `enclosing_namespaces`, import paths are first relative to the namespaces around
  (C++'s `using namespace lib;` inside `acme`). On-demand imports find library types only in
  the library's turn, so whether a placeholder exists yet does not decide between two
  packages. With `Rules::reexports` (Python), a package re-exports what its own scope imports:
  `shop.Order` is `shop.orders.Order` after `from .orders import Order` in `shop/__init__.py`.
  An import of a module alone (`import numpy as np`) makes no placeholder. A typedef is
  replaced by its target where it is used (`Ptr*` is
  `shared_ptr<Order>*`); the model has no element for it.
- **Merging** (`merge`) finds packages and types by name or creates them. Members are matched
  with what the model has (attributes and literals by name, operations by name and parameter
  types, template parameters and parameters by position): matched ones are updated, new ones
  created, the rest deleted, and the members put in source order within literals, attributes
  and operations, which come in that order. A type whose kind changed is
  replaced by a new element that takes over its children, references and presentations
  (`merge::retype`). A name declared with two kinds in one import (`class Foo` in one file,
  `interface Foo` in another) keeps the kind of its first declaration. Generalizations and
  realizations follow the sources, so one drawn by hand between types of the sources is
  deleted; dependencies drawn by hand stay. A type declared in parts
  (C# `partial`) is one element with the members of all parts, documented by the first part
  that has documentation. A member defined outside its class joins the class's members: it fills
  in parameter names, documentation and a field's default the declaration leaves out, or adds
  an operation the class does not declare. Operations match by being a query too (C++'s
  `f() const`), so `f()` and `f() const` are two operations.
- **Associations** (`associations`): attributes typed by a type of the model become
  associations, collections and maps included (`*`, `{ordered}`, `{unique}`, a qualifier for
  the key, `0..1` for optionals). The import marks the associations it makes with the tagged
  value `umlcraft.imported`. On a new import an association with the same role is reused, and
  marked associations whose field is gone are removed; those drawn by hand stay.

Parsing needs only the files, so the app runs it in the background (`parse_sources`); `apply`
then runs on the UI thread inside one `Document::edit`, so one undo takes the import back. For
large imports `Tx::delete_many` deletes in one pass what `Tx::delete` would look for one by
one, and undo replays removals without looking through the model for references, which the
recorded operations rule out; 6300 files (225,000 elements) import in about ten seconds.
Qualified names and used single-type imports are resolved before any simple name, so the
result does not depend on the order of the files and importing again changes nothing.
`cargo run --release -p umlcraft-reverse --example source-import -- <java|csharp|cpp|python> <folder>`
imports a real project, checks the model, undoes, redoes and imports again, and reports any
difference; Newtonsoft.Json (951 C# files, 22,600 elements) imports in about 1.2 seconds,
leveldb (132 C++ files, 99 of them without macros in declarations, 1,357 elements) in 0.2
seconds, requests (37 Python files, 1,396 elements) in 40 milliseconds. The
tests import fixtures per rule (`crates/umlcraft-reverse/tests/fixtures/<language>`) and
compare the IR and the resulting model with snapshots (`expected.ir`, `expected.model`;
`UPDATE_SNAPSHOTS=1` rewrites them); `tests/common` holds what the languages' tests share.
`tests/astah_compare.rs` imports the Java fixtures and compares them with what Astah's own
import made of them (reference `.asta` files kept outside the repository). Where UmlCraft
deliberately differs, because Astah loses what the sources say (wildcards, varargs,
operation templates, more than one bound …), `tests/astah_deviations/<fixture>.txt` lists and
explains the differing lines; the test fails when they change.

## .asta export

Writing `.asta` ourselves would mean producing Astah's Java object graph, which is fragile and
changes between Astah versions. So `umlcraft-asta-export` has an installed Astah write the file
through its official API (`astah-api.jar`):

- `Astah::find` looks for an installation (`ASTAH_HOME`, else `astah-professional`, `astah-uml`
  or `astah-sysml` under "Program Files") with `astah-api.jar` and an edition jar. Without one,
  File › Export › Astah project … shows a dialog saying what is needed.
- `Export::new` serializes the model as `.ucraft` and adds a layout sidecar: node bounds and
  edges as drawn (nodes grow with their content). Sequence diagrams need none; the tool reads
  their geometry from the project. The app prepares this on the UI thread and runs the rest in
  the background.
- `Export::run` writes both files and the Java tool [`tools/asta-export/AstaExport.java`](../tools/asta-export/README.md)
  (embedded with `include_str!`) to a temporary folder, and runs the tool with Astah's bundled
  JRE in source-file mode, so no JDK and no build step are needed. The tool creates packages,
  classifiers, features and relationships with `BasicModelEditor` and `UseCaseModelEditor`, then
  the diagrams with the class, use case, sequence and activity diagram editors, all in one
  transaction. It
  reports what the API cannot express (`Report::dropped`) and single failures
  (`Report::problems`).

### How the model maps onto the API

- Astah has no free-text types. A type name that is neither a primitive (`int`, `boolean` …)
  nor a modeled classifier becomes a class in the attribute's package; the API does this on its
  own. Attributes and parameters without a type get `void`, because the API demands a type.
- Astah rejects empty and duplicate names among siblings; such elements get a numbered name and
  are reported as renamed. A dependency with the stereotype «use» becomes a usage.
- Node sizes are set explicitly with Astah's auto-resize turned off. Astah may still grow a node
  to fit it, e.g. a use case with extension points. Nodes without a fill color get Astah's usual
  `#FFFFCC` instead of the API's white. Notes are created per presentation, since Astah keeps
  comments as notes on diagrams; comments that are not on any diagram are dropped.
- Free texts and lines go through `createText` and `createLine` of the diagram's editor. Astah
  adds the line but reports an exception; the export then takes the line the diagram did not
  have before and sets `line.type` and `line.arrow.type` on it.
- Sequence diagrams: Astah puts lifeline heads at y = 30, so the whole drawing moves vertically
  by as much; x stays. Messages are created from top to bottom at their stored height, from the
  Astah bar made for the UmlCraft bar at that height (or the lifeline). Astah makes the bars
  itself by the same rules UmlCraft follows, and refuses to resize them (`setHeight`, reported as
  `barHeight`); a reply (`createReturnMessage`) goes to the end of the bar of the call it
  answers, where UmlCraft has it too. Lifeline lengths are set through `lifeline_length`; Astah
  refuses lengths shorter than what lies on a lifeline, interaction uses included. A reply's
  return value is stored on the call it answers, as Astah does. The round-trip test checks that
  heads and message heights come back unchanged.
- Activity diagrams: `createActivityDiagram` brings its own activity, so an activity exports its
  first diagram only. Astah creates activity nodes only on a diagram; nodes, flows and
  partitions not shown are dropped. Partitions are created left to right with
  `createPartition(superPartition, previous, …)`; Astah wraps top-level lanes in a "dimension"
  header of its own and lines them up below it, so the header is moved above UmlCraft's lanes
  before the lanes are placed. The API creates decision and merge nodes as one kind; the import
  tells them apart by their flows. Unnamed control nodes get Astah-like default names
  (`InitialNode0`), which the import drops again, together with Astah's own (`ForkNode1`) and the
  default dimension (`Dimension0`).
- Component and deployment diagrams: the API creates components, artifacts and nodes
  (`createComponent`, `createArtifact`, `createNode`, in packages only) but has no editors for
  these diagrams. They are exported as class diagrams, which show the same elements; instance
  specifications exist only there (`createInstanceSpecification`), always as objects, placed
  when created (they cannot be moved) and sized by Astah. UmlCraft's class diagrams accept
  components, nodes and instances, so such a file comes back intact apart from the diagram
  kind. A usage (`createUsage`) is an `IUsage`, not an `IDependency`.
- Object diagrams are class diagrams. Slot values are set with `getSlot(name).setValue`; links
  are created between two presentations (`createInstanceSpecificationLink`), and their ends are
  matched to UmlCraft's by instance. The API cannot set the association a link instantiates,
  which is reported as `linkAssociation`.
- State machine diagrams: `createStatemachineDiagram` brings its own state machine. States are
  created from the outside in, each composite state at its size before its substates go into it
  (`createState(name, parent, …)`). The API gives a transition leaving a state an event called
  "Trigger"; `setEvent(null)` removes it again. Astah keeps room for entry, do and exit, which
  makes a state at least 81 high, so states without them have that compartment hidden
  (`action_visibility`). Astah routes the first and last segment of a self-transition itself.
  `addRegion(state, horizontal)` divides a state once into two regions (calling it again does
  nothing), so further regions are reported as `region` and their contents go into the second.
  Astah picks a new substate's region by where it is created, so substates start inside their
  region and are moved to their place afterwards. Ending the transaction, Astah moves the line
  between the regions next to what the first one holds and shifts the second one's contents
  along; a second transaction puts the substates of orthogonal states back. Internal
  transitions are created with `addInternalTransition(trigger, guard, effect)`.
- Interaction uses are created in the order of the interaction (`createInteractionUse` with the
  covered lifelines, all of them when none are given) at their height and then sized; since
  Astah refers to sequence diagrams, the reference is set once every diagram exists. Found and
  lost messages use `createFoundMessage` and `createLostMessage` at the stored `gate_x`.
- Combined fragments are drawn where UmlCraft has them, outer ones before inner ones
  (`createCombinedFragment(name, operator, location, width, height)`); Astah takes the messages
  inside into the first operand. `addInteractionOperand` moves the messages of each further
  operand into it; Astah refuses that for empty operands and for operators with a single
  operand (reported as `emptyOperand`, `extraOperand`). The separators go to their places
  through `operand.N.length`, guards moved by hand through `guard.N.point.x/y`.
- Message display options: `parameter_visibility`, `parameter_type_visibility` and
  `return_value_visibility` (a call without arguments shows its operation's parameters,
  `Style::hide_parameter_types`, `Style::show_return_type`), and labels moved by hand through
  `name.point.x/y`. Astah moves a label by as much as `name.point` differs from its own value,
  so the layout sidecar says where Astah puts each such label by itself (`labelDefaults`: the
  last segment's middle minus half of Java's whole-pixel width, at most 200). The diagram's
  `message_index_visibility` and `message_return_value_initial_visibility` carry
  `DiagramProps::hide_message_numbers` and `hide_return_values`. The API makes bars white;
  they get their UmlCraft fill (`fill.color`, Astah's yellow by default).
- Not available in the API and reported as dropped: the query flag of operations, conditions and
  extension points of extends, return parameters, parameter multiplicity and default values.
- System boundaries, which Astah does not have, become what Astah users draw instead: a
  rectangle (`createRect`) and the name as a text (`createText`) centered at its top. The
  importer turns a rectangle on a use case diagram back into a boundary, named by the text
  that starts within 40 of its top edge. Boundaries no diagram shows are dropped.
- Line styles: `line.shape` is set before the points (Astah would otherwise drop or replace
  them); right-angled lines send their corners as drawn (the sidecar's `corners`).
- Edges: `setAllPoints` gets the source anchor, the waypoints and the target anchor as points;
  Astah derives its relative ends (`sourceEndX/Y`, `targetEndX/Y`) from the first and last
  point. Astah stores some links the other way round (a dependency starts at the supplier, a
  generalization at the general class) and rejects points in the wrong order; the current first
  point (`getAllPoints()[0]`) tells which node comes first, `getSource()` does not always.
  Labels placed by hand are set as properties with their position on the diagram (`name.point.x`,
  `end_a.name.point.x`, `end_b.multiplicity.point.y` …); the sidecar turns UmlCraft's offsets into
  those positions. `setProperty` silently ignores keys it does not know. Constraints of
  association ends become constraints on the end.

## Nassi-Shneiderman diagrams

`umlcraft-nsd` holds structograms in the `.nsd` format of
[Structorizer](https://structorizer.fisch.lu) 3.32-35. They are not part of the UML model:
a diagram is a file of its own. Structorizer is GPL-licensed; the format and the way
Structorizer writes it were worked out from its XML schema, its user guide and the files it
writes, and none of its code is used.

- **Model.** A `Diagram` has the attributes of `<root>` (header: version, parser keywords,
  author, dates, licence; the root's own text, comment, colour, `type` program/sub/includable,
  `style` nice/abbr) and a `Sequence` of `Element`s. An element has a kind (instruction, call,
  jump, alternative, case, for, while, repeat, forever, try, parallel), its attributes and its
  branches (`qTrue`/`qFalse`, one `qCase` per case …). Every node keeps its attributes as an
  ordered list exactly as read and offers typed access on top (`text`, `comment`, `color`,
  `disabled`, `branch_colors` …). Texts are lists of lines, stored as `"line1","line2"` with
  doubled quotes (`text`). Setting an attribute a node lacks puts it where Structorizer writes
  it (`ElementKind::order`, `ROOT_ORDER`).
- **Reading** (`read`) uses quick-xml. Attribute values are unescaped but not normalized: a
  tab Structorizer wrote stays a tab, where an XML parser (Structorizer's too) would read a
  space. Unknown attributes are kept; unknown elements, wrong branches and other encodings
  than UTF-8 are errors with line and column.
- **Writing** (`write`) is done by hand, because Structorizer escapes differently from XML
  libraries (`&#34;`, `&#60;`, `&#62;`, `&amp;`, `&#10;`): the declaration, a tab per level,
  elements without branches on one line, branch tags on lines of their own, LF line ends and
  none after `</root>`. Empty sequences carry `color="ffffff"`, as in Structorizer's files.
- **New diagrams and elements** (`create`) get the attributes Structorizer gives them: the
  header of a fresh installation's GUI (its keywords `"until "` and `"while "` end in a space),
  `branch_colors` with one slot per branch, a parallel element's branch count as its text,
  the attributes of counting, traversing and free-text for loops.
- **Editing.** `path` addresses elements (odd paths) and sequences (even paths; `[]` is the
  body), places between elements (`Slot`), runs of elements in one sequence (`Span`) and what
  is selected (`Selection`). `edit` holds the changes as functions on a `Diagram` that return
  the selection afterwards: insert, remove, move (never into itself), shift up or down, text,
  comment, colour, disabling, case and parallel branches, the root's signature and kind. Each
  keeps the shape Structorizer writes: empty sequences carry `color="ffffff"`, a case has a
  line per branch plus its selector, a parallel element's text is its branch count, an empty
  element text is written as `text=""`, and a for loop's attributes are derived from its text
  as Structorizer derives them when it loads a file (checked against a file it saved,
  `fixtures/structorizer/for-headers.nsd`). `history::Editor` makes each change one undo step
  over snapshots of the whole diagram (structograms are small) and tells whether the diagram
  differs from the file. `save` stamps a changed diagram with version, user and date as
  Structorizer does on saving; an unchanged one is written as read, byte for byte. `clip` puts
  elements on the clipboard as a whole `.nsd` file, the form in which Structorizer puts a
  diagram there; other text pastes as one instruction.
- **Text metrics** (`metrics`). Structorizer measures with Java's logical font Dialog 12,
  antialiased with integer metrics: a line is as wide as its characters' advances summed and
  rounded, the bold title's widths are cut off instead. The font behind Dialog differs by
  platform; `tools/nsd-metrics/DialogMetrics.java` writes a table per platform
  (`src/metrics/dialog12-<platform>.txt`). Linux (DejaVu Sans) and Windows (Arial, whole-pixel
  advances, title lines 16 px apart instead of 17) are measured; Linux uses its own, every
  other platform the Windows table, Structorizer's most common one. The include list's box
  above the title uses a smaller font whose widths are not tabled: its height and caption
  width were measured in Structorizer's images and stand in each table (`includes`).
- **Layout** (`layout`) works in whole pixels, with the rules read off Structorizer's images
  (GUI defaults, syntax highlighting off): `measure` gives each element its natural size from
  the inside out, `place` its box. A box is given by its lines; neighbours share theirs.
  Texts get a padding of 20 and 13 px per line, the frame 20 around the body and 17 per title
  line. An alternative's head is `13 n + 39` high and wide enough that every line of the
  condition, with its padding, fits between the diagonals at its baseline; spare width goes to
  the TRUE branch. Case columns are as wide as their contents or labels plus 10, spare width
  to the last; with more than 8 branches a branch of one simple element is turned by 90°
  where that is narrower. Loops have a bar of 19 and a head or footer; try elements rows of
  33 with bodies indented 20 (catch 40); parallel elements bands of 20. A sequence's last
  element takes spare height, a structured one hands it to its branches. Every element box
  has its path (element and branch indices); `Layout::hit` tells what a click selects and
  `Layout::slot_at` where a drop goes.
- Where the texts in triangular heads sit (an alternative's condition, a case's
  discriminator) follows no rule we found; a fit to Structorizer's positions puts them within
  a few pixels. Lines are exact.
- **Drawing** (`draw`) turns a layout into areas, 1 px lines and texts, fills first. Colours
  are roles (`Ink`) so that a painter can use its theme, except the elements' and branch heads'
  own colours. The hatching of disabled elements repeats Structorizer's 5×10 pixel tile from
  the image's origin. `svg::render` writes it as an SVG image as large as Structorizer's PNG
  export; the app rasterizes it with `png::from_svg` like UML diagrams.
- **In the app** (`umlcraft-app/src/nsd`), a structogram opens in a window of its own
  (`nsd::open`, `nsd::new`); the main window only gets the menu items, a switch by extension
  where files open (menu, command line, system, recent files) and two hooks (`nsd::connect`):
  saved structograms join the recent files, and quitting from a structogram window quits
  through the main window, which asks about the project and then about each structogram with
  unsaved changes (`nsd::confirm_all`). `NsdView` holds the `Editor` apart from the window, so
  that a tab can build on it later. It shows the SVG export rasterized with resvg, made again
  when the diagram, the zoom or the screen's scale changes, so the window holds exactly what
  an export does; GPUI cannot turn text, which turned case branches need. Selection and the
  place a drag would drop to are painted over the image and never reach an export. Text,
  comment and the root's signature are edited in a dialog (`nsd/dialog.rs`), as in
  Structorizer; a new element is only inserted when its dialog is confirmed. The diagram stays
  a white sheet in the dark theme; the area around it follows the theme. The window has
  toolbars instead of menus, because on macOS the menu bar belongs to the whole app.
- **From source code** (`umlcraft-nsd-code`; `java`, `csharp`, `python`): tree-sitter reads a
  `.java`, `.cs` or `.py` file (C# with only the first branch of each `#if`, as the class import
  reads it); `Source::methods` lists methods and constructors with a body (nested types
  included, not classes inside methods) with their signatures as written, in C# also finalizers,
  operators (`op_Addition`), the accessors of properties, indexers and events as .NET names them
  (`get_Count`, `set_Item`) and the top-level statements as `Program.Main`; in Python the
  functions of the module (without a class), methods without `self`/`cls` in the title, the
  docstring as the diagram's comment, and the code of the module as `__main__`;
  `Source::diagram` builds one with the elements of `umlcraft-nsd` (`edit::new_element`, so
  every rule of the editor holds). Each language reads its own nodes and hands the parts to a
  shared builder (`core`: comments, conditions and loops, the cases of a switch, the parts of a
  try), which a `Grammar` tells the node kinds that differ. Both languages give the same
  structogram for the same method (`tests/parity.rs`, differences on purpose in
  `tests/parity_deviations.txt`). Texts are written as structograms write them (`pseudo`): no
  types (the root is `name(a, b)`, a declaration without a value has no element), assignments as
  `x := value` (`total += a` as `total := total + a`, `i++` as `i := i + 1`), no `;`;
  expressions and conditions stay as in the code, conditions without their outer parentheses and
  ending in `?` (`a > 0?`, also a case's discriminator and a loop's condition); a returned value
  reads `gib x` (`return;` stays `return`). `f(x)` and `this.f(x)` are calls, other statements
  instructions (in Python only calls of functions the file defines, and `self.f(x)`, so that
  `print(x)` stays an instruction); an expression body (`=> a + b`) is a `return` or, where
  nothing is returned, a statement. A for loop that counts one variable by a constant step is a
  counting loop (`for i := 0 to n - 1`, `by 2`), `for (T x : c)` a traversal (`foreach x in c`),
  Python's `for i in range(a, b, s)` a counting loop with a constant step; Structorizer reads
  both with the attributes we write. Any other for loop becomes its initialization and a while
  loop with the update at the end of its body; without a condition, and `while (true)`, it is an
  endless loop. `switch` becomes a case element: labels sharing a body are joined, the default
  goes last (`%` without one), a case that runs on gets the following statements copied (Java
  only: C# forbids it, so a C# case never runs on), and a switch expression's values (C#'s arms
  too, `_` as the default, `throw` as a jump) are assigned or returned where the expression
  stood. Several catches become a case on the exception's type, C#'s `when` filters in its
  labels. Python's `elif` chains are nested alternatives, `match` a case element (`case _` the
  default, a `break` there leaves the loop around it), `with` assigns its contexts like C#'s
  `using`, and the `else` of a loop follows it, the `else` of a `try` ends the try block (both
  named in the report). Comments go to the element of the next statement, or of the one they end
  on. What has no element is kept as near as possible (`continue` and unreadable code as red,
  disabled instructions, a `synchronized` or `lock` body in place with the lock as a comment, a
  `using` resource assigned with `using` as a comment, `yield` and `goto` as written; labels and
  local functions left out) and named in the `Report`. `tests/snapshots.rs` holds every fixture
  method as a readable tree; `tests/structorizer_structure.rs` compares the nesting with
  Structorizer's own import of the same files (with `UMLCRAFT_NSD_REFS`), the deliberate
  differences listed in `tests/structorizer_deviations.txt`; the `nsd-import` example runs a
  whole source tree. Structorizer has no C# or Python import to compare with; besides the parity
  test, the example ran over Newtonsoft.Json (951 files, 8438 methods) and requests and httpx
  (97 files, 1745 functions), and Structorizer read the diagrams back as written (apart from its
  own keywords `input`/`output`, which it capitalizes). In the app, `nsd/source.rs` holds the
  menu command (the extension picks the language) and the dialog that picks methods; each opens
  unsaved, with the report in a bar at the bottom.

Every file Structorizer 3.32-35 wrote that we have reads and writes back byte for byte. The
tests check our own diagrams, one feature each, as Structorizer saved them
(`tests/fixtures/structorizer`), and a snapshot of a new diagram with every kind
(`tests/fixtures/new`, `UPDATE_SNAPSHOTS=1` rewrites it). With `UMLCRAFT_NSD_REFS` pointing at a
folder of further reference files kept outside the repository (Structorizer's samples and
Java imports, our measuring diagrams, a file saved by Structorizer's GUI on Windows, which
is byte for byte the Linux one; 258 files), `reference_files_round_trip` checks those too,
and `structorizer_drawing` draws all diagrams Structorizer can draw (389), rasterizes them with
resvg and compares their straight lines with Structorizer's PNG exports (`plain/…`, exported
with the GUI's settings and highlighting off); with the Windows table it does the same for the
exports made on Windows (`windows/…`, 187). Differences we understand are listed with their
reason in `tests/structorizer_deviations.txt`. Snapshots of the fixtures' layout and of one
SVG (`tests/drawing.rs`) guard the rest. `cargo run -p umlcraft-nsd --example nsd-svg --
FILE.nsd` draws a file.
