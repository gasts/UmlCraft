# UmlCraft code review

Full review of the repository at version 0.8.0 (commit `6c397f2`, 2026-10-28): all seven crates,
the Java export tool, CI, release workflow, packaging and docs.

## Contents

1. [Summary](#summary)
2. [How the review was done](#how-the-review-was-done)
3. [Quality gates](#quality-gates)
4. [Priorities](#priorities)
5. [Cross-cutting themes](#cross-cutting-themes)
6. [Findings by area](#findings-by-area)
   - [Model and file format](#model-and-file-format-umlcraft-model-umlcraft-format)
   - [Notation: scene, routing, drawing, SVG](#notation-scene-routing-drawing-svg)
   - [Notation: editing](#notation-editing-sequence-state-clipboard-member-object-palette)
   - [Astah import and export](#astah-import-and-export-umlcraft-asta-umlcraft-asta-export-toolsasta-export)
   - [Source import](#source-import-umlcraft-reverse)
   - [Application](#application-umlcraft-app)
   - [CI, release and packaging](#ci-release-and-packaging)
   - [Documentation](#documentation)
7. [Test coverage](#test-coverage)
8. [Structure and maintainability](#structure-and-maintainability)
9. [Strengths](#strengths)
10. [Suggested order of work](#suggested-order-of-work)

## Summary

UmlCraft is a carefully built codebase. The architecture in `docs/architecture.md` is real, not
aspirational: every model change goes through `Model::apply` and `Document::edit`, undo is built
on exact inverse operations, the notation crate has no UI dependency, and one geometry source
feeds the canvas, hit testing, inline editing and the SVG export. Clippy, rustfmt and all 305
tests pass; the i18n key sets are complete and identical in both languages. Comments name the
Astah measurements behind the constants, and the per-language source import shares one mapping
through `Rules` instead of language switches.

The problems are concentrated in four places:

1. **Hostile or unusual input.** A small crafted `.asta` file can crash or hang the app (cycles
   in the object graph, a self-referencing superclass, a declared ZIP size of a terabyte), and the
   import runs on the UI thread. Non-finite coordinates and deeply nested types pass every check,
   so UmlCraft can save projects it cannot open again.
2. **Committing text from the UI.** The properties panel can lose a typed edit when focus moves
   between fields, writes back fields the user never changed (turning type references into
   free text), and uncommitted text is not flushed before New, Open, Save or closing the window.
3. **Multi-element edits.** Paste fails at random because copies with strong references are
   inserted in hash order; moving a container drags along nodes it merely passes over; batch
   reparenting can close ownership cycles.
4. **Scale.** Several hot paths use linear lookups inside loops (name lookup in the source
   import, generalization checks, `delete_many`, removing presentations, `with_contents`), and
   the canvas rebuilds the whole scene on every frame.

None of these needs an architectural change; almost all fixes are local. The biggest gap in the
safety net is that the 21 tests that exercise the `.asta` import and export against Astah's own
files pass without checking anything on CI, because they need an Astah installation (or local
reference files) that CI runners do not have.

Severity used below:

- **High:** crash, hang or silent data loss through ordinary use or by opening a file; or a core
  feature that fails for common input.
- **Medium:** wrong result, lost information or noticeable slowness in realistic use.
- **Low:** edge cases, polish, maintainability, docs.

Confidence: **confirmed** (reproduced with a test or a scratch program), **traced** (followed
through the code, callers included), **plausible** (depends on behavior that could not be run
here, such as the GPU-backed UI).

## How the review was done

- Read all crates, the Java tools, workflows, packaging files and docs against
  `docs/architecture.md` and the project rules in `docs/handoff-source-import.md`.
- Ran `cargo fmt --all --check`, `cargo clippy --workspace --all-targets` and
  `cargo test --workspace` (the app crate's tests compile but do not link in a container without
  `libxkbcommon`; `cargo check` and `cargo clippy` of the app pass).
- Confirmed findings with temporary tests and scratch programs that fed crafted input to the
  public APIs (crafted `.asta` files built with `jser::Graph` and `write_asta`, randomized
  edit/undo/redo sequences, timed large models). All temporary files were removed; the
  repository contains only this document as a change.
- Could not run the GPUI app itself (no GPU or display). UI findings are traced through the app
  code and the sources of `gpui-pre` 0.3.9, `gpui-base` and `gpui-component` 0.8.

## Quality gates

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `cargo test --workspace` (without the app) | 305 passed, 1 ignored (`large_imports_are_fast`) |
| App crate | `cargo check` and `clippy` clean; 8 tests, not runnable here (link needs `libxkbcommon`) |
| Tests that need Astah | 21 report "ok" without checking anything when Astah is missing (see [X-1](#x-1)) |
| `unsafe` | none (`unsafe_code = "forbid"` in the workspace) |
| i18n | 428 keys in each locale, identical sets, none missing, none unused |

## Priorities

The ten things to do first, by risk and by how cheap the fix is:

| # | What | Findings |
| --- | --- | --- |
| 1 | Guard the `.asta` reader and mapper against cycles, deep nesting and huge declared sizes, and import on a background thread | [A-1](#a-1), [A-2](#a-2), [A-3](#a-3), [A-4](#a-4) |
| 2 | Reject non-finite geometry and over-deep types in the model, so no saved project is unreadable | [M-1](#m-1) |
| 3 | Track the edited element per properties field, commit only changed fields, flush pending edits before New/Open/Save/Close | [U-1](#u-1), [U-3](#u-3), [U-4](#u-4), [U-5](#u-5) |
| 4 | Compute what a container drag moves once, from the unmoved scene | [U-2](#u-2) |
| 5 | Make paste deterministic and aware of sequence diagrams | [E-1](#e-1), [E-2](#e-2) |
| 6 | Source import: survive one name declared with two kinds, skip build/venv folders, walk Java bodies iteratively | [R-1](#r-1), [R-4](#r-4), [R-5](#r-5) |
| 7 | Do not apply a background import to a document opened meanwhile | [U-6](#u-6) |
| 8 | Make the Astah-dependent tests visibly skipped and add synthetic `.asta` fixtures and a fuzz target | [X-1](#x-1) |
| 9 | Replace linear lookups in the hot paths with indexes | [M-2](#m-2), [M-3](#m-3), [M-4](#m-4), [N-2](#n-2), [R-7](#r-7), [U-7](#u-7) |
| 10 | Make saving durable (fsync, temp-file cleanup, retry on Windows sharing violations) | [M-5](#m-5) |

## Cross-cutting themes

### 1. The model accepts states it cannot round-trip or undo

`Model::check` and `Model::apply` are thorough about ownership and references but leave out a few
properties that other code relies on:

| Gap | Consequence | Finding |
| --- | --- | --- |
| Non-finite `f32` in bounds, waypoints, font sizes | Saved as `null`, loading fails: the project cannot be reopened | [M-1](#m-1) |
| Unbounded `TypeRef` nesting | Saved fine, loading hits serde_json's recursion limit of 128 | [M-1](#m-1), [E-5](#e-5) |
| Cycles of strong references (`client = self`) | Element and its owner cannot be deleted | [M-6](#m-6) |
| Edges attached to each other in a cycle | Node, element or diagram cannot be deleted | [M-7](#m-7) |
| `binding` on non-class classifiers | Never validated, never unlinked | [M-8](#m-8) |

Checking these in `apply` and `check` closes the gap for every source at once: the canvas,
`.asta` import, source import and hand-edited `.ucraft` files. A cheap extra safety net is to have
`umlcraft_format::save` parse its own output before renaming it over the old file.

### 2. A crash or a frozen UI loses unsaved work

There is no autosave or recovery file, so every abort loses everything since the last save. The
paths found:

- Stack overflow on `.asta` cycles ([A-1](#a-1), [A-2](#a-2)), on long Java expressions
  ([R-4](#r-4)) and on deeply nested type text ([E-5](#e-5)).
- Aborting allocation from a declared ZIP size ([A-3](#a-3)) or a long superclass chain
  ([A-2](#a-2)).
- `.asta` import and `.ucraft` loading on the UI thread ([A-1](#a-1), [U-15](#u-15)); the
  `.asta` export has no timeout ([A-7](#a-7)).

Besides the individual fixes, a periodic recovery file (for example the `.ucraft` JSON written to
the settings folder every few minutes while dirty) would bound the damage of whatever is not
found yet.

### 3. Text edits are committed by focus events

The properties panel and the inline editor commit on Enter or blur. This makes the outcome depend
on GPUI's event order ([U-1](#u-1)), on whether a shortcut moves focus ([U-4](#u-4)), and on
whether a field was touched at all ([U-3](#u-3), [U-5](#u-5)). Combined with the member text
form not round-tripping some imported constructs ([E-3](#e-3), [E-4](#e-4)), simply clicking
through an imported model can change it. One rule fixes most of this: commit only when the text
differs from what was loaded, and keep the target element per field.

### 4. Linear lookups inside loops

The model and scene are `IndexMap`s and `Vec`s with by-id access, but several paths search them
linearly per item: `Scene::node`/`Scene::edge`, `View::get`, `child_type`/`child_package` in the
source import, the generalization parent map rebuilt per insert, `unlink_all` per doomed element.
Each is fine for a hand-drawn diagram and quadratic or worse for an imported project
([M-2](#m-2) to [M-4](#m-4), [N-2](#n-2), [N-10](#n-10), [R-7](#r-7), [U-7](#u-7)). The
documented "6,300 files in about ten seconds" is real for Java's deep package trees but does not
hold for flat namespaces (C/C++ global types, generated SDKs).

### 5. Tests that pass without testing

The `.asta` import (about 3,600 lines in `jser.rs` and `map.rs`) and the export are covered only
by tests that need Astah's own files or installation; on CI they return early and count as
passed. See [X-1](#x-1).

## Findings by area

### Model and file format (`umlcraft-model`, `umlcraft-format`)

<a id="m-1"></a>
#### M-1 · Medium · Saved projects that cannot be opened again

`crates/umlcraft-format/src/lib.rs:73`, `crates/umlcraft-model/src/model.rs:407`,
`crates/umlcraft-model/src/geometry.rs:5-43`

- **Defect:** nothing rejects non-finite floats or deeply nested `TypeRef`s. serde_json writes
  NaN and infinity as `null` and refuses `null` for `f32` on reading; it also stops at 128 levels
  of nesting.
- **Scenario (confirmed):**
  - `set_bounds(.., Rect::new(NAN, ..))` saves; reopening fails with "invalid type: null,
    expected f32".
  - A file with `"x": 1e39` loads as infinity (f64 → f32 with `as`); the next save makes it
    unloadable.
  - An `.asta` `TextPresentation` with `x = NaN` imports, the project saves, and cannot be
    reopened ([A-5](#a-5)).
  - An attribute typed `A<A<…>>` 45 levels deep (accepted by `member::parse_type`) saves and fails
    to reload with "recursion limit exceeded".
- **Fix:** reject non-finite geometry and font sizes in `apply` and `check` (or deserialize
  through a finite-f32 type); cap `TypeRef` depth (for example 32) in `check_references`; let
  `save` verify its output before the rename.

<a id="m-2"></a>
#### M-2 · Medium · Every generalization insert or update scans the whole model

`crates/umlcraft-model/src/model.rs:438-466`

- **Defect:** `check_references` rebuilds `generalization_parents` over all elements for each
  generalization insert or update; redo takes the same path.
- **Scenario (confirmed, release build):** 3,000 generalizations into a 200,000-element model take
  13.6 s; 2,000 into 40,000 take 0.96 s, redo 0.82 s. This is likely the dominant cost of the
  large source imports the docs advertise.
- **Fix:** keep an incremental `specific → [generalization]` index in `Model`, maintained by
  insert, remove and update; at least iterate only generalizations.

<a id="m-3"></a>
#### M-3 · Medium · `delete_many` unlinking is quadratic

`crates/umlcraft-model/src/document.rs:242-244`, `318-327`

- **Defect:** for every surviving referrer, `unlink_all` clones the name of every doomed element
  and calls `unlink` once per doomed element. Every call also scans the whole model at least once.
- **Scenario (confirmed):** deleting a package of N classes, each referenced once from outside,
  takes 0.10 / 0.36 / 1.68 s for N = 1,000 / 2,000 / 4,000. Deleting one element in a 200,000
  element model takes 55 ms, and `umlcraft-reverse/src/merge.rs:443` calls `delete_many` once per
  type with stale supertypes. Deleting the `java` placeholder package after an import is the
  realistic case.
- **Fix:** build the `id → name` map once; per survivor, unlink only the targets it references
  (from `visit_references`).

<a id="m-4"></a>
#### M-4 · Medium · Removing many presentations is roughly cubic

`crates/umlcraft-model/src/document.rs:446-455`

- **Defect:** each removal rescans `gone` from the start, and each probe runs
  `edges_attached_to`, which scans the whole view.
- **Scenario (confirmed):** deleting a diagram with 250 / 500 / 1,000 nodes and twice as many
  edges takes 0.07 / 0.42 / 2.75 s. Select all → remove from diagram takes the same path.
- **Fix:** count attachments per presentation once, then remove in topological order with a
  queue.

<a id="m-5"></a>
#### M-5 · Medium · `save` is atomic against a process crash only

`crates/umlcraft-format/src/lib.rs:129-138`

- **Defect:** `fs::write` then `fs::rename`, without `sync_all` on the file or the directory, no
  cleanup and no retry. The doc comment promises "a crash never leaves a half-written project".
- **Scenarios:**
  - Power loss or OS crash shortly after the rename can leave an empty or partial file on NTFS,
    APFS or ext4 (delayed allocation).
  - On Windows, a virus scanner, the search indexer or a sync client holding the target without
    `FILE_SHARE_DELETE` makes the rename fail; the user sees an error and a stray `.ucraft.tmp`.
  - Replacing the file drops its permissions and replaces a symlink by a plain file.
- **Fix:** `File::create` + `write_all` + `sync_all`, rename, fsync the directory on Unix (or
  `ReplaceFileW`/`MOVEFILE_WRITE_THROUGH` on Windows); retry briefly on sharing violations; remove
  the temp file on error. Confidence: traced.

<a id="m-6"></a>
#### M-6 · Low · Strong-reference cycles make elements undeletable

`crates/umlcraft-model/src/element.rs:977`, `crates/umlcraft-model/src/document.rs:262-275`

- **Defect:** `Client`/`Supplier` accept any kind except Model and Diagram, so a dependency can
  name itself or another relationship; neither `apply` nor `check` rejects strong cycles.
- **Scenario (confirmed):** set a dependency's client to itself; deleting it or its package fails
  with "cannot delete …: reference cycle".
- **Fix:** reject strong-reference cycles in `check_references` and `check`, or restrict
  `Client`/`Supplier` to non-relationship kinds.

<a id="m-7"></a>
#### M-7 · Low · Edge-attachment cycles block removal

`crates/umlcraft-model/src/model.rs:356-376`, `407-416`

- **Scenario (confirmed):** `UpdatePresentation` or a loaded file makes edge E1 end on E2 and E2
  on E1. Removing a node, deleting its element or deleting the diagram then fails with "edges
  attached in a cycle".
- **Fix:** check attachment cycles in `check_presentation` and `check`.

<a id="m-8"></a>
#### M-8 · Low · `binding` is validated only on classes

`crates/umlcraft-model/src/element.rs:262`, `1232`

- **Scenario (confirmed):** an interface whose `binding` names a missing id passes `apply` and
  `check`, and is never unlinked when its target is deleted.
- **Fix:** visit and unlink `binding` for every classifier kind, or reject it on non-class kinds.

<a id="m-9"></a>
#### M-9 · Low · Versioning is bypassed

`crates/umlcraft-format/src/lib.rs:33`, `97`, `110-127`

- **Defect:** `rename_language_tags` runs on every load outside `MIGRATIONS`; the v1 → v2
  migration is a no-op; `from_str` drops the file's version, so the app runs
  `sequence_edit::upgrade_document` on every open (`main_view.rs:322`).
- **Scenario (confirmed):** in a current file, a user's own tagged value `jude.java.mine` silently
  becomes `java.mine`.
- **Fix:** make the rename a v3 migration and return the original version, so only v1 files are
  upgraded.

<a id="m-10"></a>
#### M-10 · Low · Robustness and dirty-tracking details

- `crates/umlcraft-format/src/lib.rs:141`: a file starting with a UTF-8 BOM (some Windows editors
  add one) fails with "expected value at line 1 column 1". Strip a leading `\u{feff}`.
  Confirmed.
- `crates/umlcraft-model/src/document.rs:33-41`: `Document::new` counts as saved, so an unsaved
  `.asta` import or the sample project never triggers the unsaved-changes prompt
  (`main_view.rs:223`, `331`). Add a "never saved" state. Traced.
- `crates/umlcraft-model/src/model.rs:295`: `remove_unreferenced` uses `IndexMap::swap_remove`, so
  the order of `elements()` changes after an undo or a failed edit. Callers that depend on it
  (for example the order of generals for slot order in `notation/src/object.rs:20`) can change
  after an unrelated undo. Document the order as unspecified or use `shift_remove`.
- `crates/umlcraft-model/src/sample.rs:1-2` says the sample is the start document; the app starts
  with `blank_document()` and offers the sample through "Open sample" only.

Error texts of `ModelError` and `FormatError` reaching the UI are covered under [U-9](#u-9).

### Notation: scene, routing, drawing, SVG

<a id="n-2"></a>
#### N-2 · Medium · `with_contents` is cubic and runs every frame of a drag

`crates/umlcraft-notation/src/scene.rs:1728-1755`

- **Defect:** `scene.node(result[i])` is a linear lookup and `result.contains` a linear scan,
  inside a loop over all nodes per container.
- **Scenario (confirmed, release):** with 2,000 nodes of which 200 are packages, select all and
  drag: 358 ms per call (44 ms at 1,000 nodes), on every frame; the whole scene build takes
  1.5 ms.
- **Fix:** a `HashSet` for `result`, an id → index map in `Scene` (`Scene::node`/`edge` are called
  in loops elsewhere too, for example `umlcraft-asta-export/src/layout.rs:48`). With the fix for
  [U-2](#u-2) it also runs only once per drag.

The bug in how the canvas calls `with_contents` is [U-2](#u-2).

<a id="n-3"></a>
#### N-3 · Medium · Rows and labels can contain newlines and control characters

`crates/umlcraft-notation/src/scene.rs:611` (`Line::new`), `1840-1868`

- **Defect:** only notes, free texts and use case names are split into lines. Class names,
  members, default values, guards and edge labels go through as they are.
- **Scenario:** the Java import stores raw initializer source (`reverse/src/java/mod.rs:358`):
  `int[] XS = {\n 1,\n 2 }` gives the row `"+ XS = {\n    1,\n    2 }"` (confirmed). GPUI's
  `shape_line` has `debug_assert!` against `\n` (`gpui-pre` `text_system.rs:645`), so debug
  builds panic and release builds misdraw (traced). Free texts split on `\n` but keep the `\r` of
  CRLF text.
- **Fix:** normalize in `Line::new` and for edge labels: `\r\n`, `\n`, `\t` to a space, drop other
  C0 control characters. Also collapse whitespace in imported default values.

<a id="n-4"></a>
#### N-4 · Medium · SVG export writes characters XML forbids

`crates/umlcraft-notation/src/svg.rs:367-379` (`escape`)

- **Defect:** only `& < > "` are escaped; XML 1.0 forbids U+0000–U+0008, U+000B, U+000C,
  U+000E–U+001F, U+FFFE and U+FFFF, even as character references.
- **Scenario (confirmed):** a class named `"A\u{1}B"` or a note with a form feed (both possible
  from `.asta`) makes the file not well-formed; browsers and Word reject the whole image.
- **Fix:** drop or replace these characters with U+FFFD in `escape`. Injection of markup is not
  possible: user text never goes into attributes, colors come from `u8`.

<a id="n-5"></a>
#### N-5 · Medium · Align uses stored bounds, the user sees drawn bounds

`crates/umlcraft-app/src/workspace.rs:751-761` → `crates/umlcraft-notation/src/scene.rs:2596`

- **Scenario (confirmed):** reference box (0, 0, 200, 50); class B stored 40 × 30 but drawn wider
  for its long name. After Align Right the drawn right edges are 200 and 425.5.
- **Fix:** align the drawn rects (`scene.node(id).rect`) and apply the offset to the stored
  bounds, keeping their size.

<a id="n-6"></a>
#### N-6 · Low · Region lines drawn outside small states

`crates/umlcraft-notation/src/scene.rs:1502-1507`, `1535-1552`; `state_edit.rs:81`

- **Scenario (confirmed for `region_shapes`):** a state 40 high with 5 regions gets regions at
  y = −40 and −20, drawn above the state; with 3 regions in 50, the first line crosses the name.
  `add_region` never grows the state. The documented rule "every region keeps at least 20" does
  not hold.
- **Fix:** in `layout_state`, grow the state to the header plus `n × MIN_REGION` (width for side
  by side regions).

<a id="n-7"></a>
#### N-7 · Low · Slightly slanted right-angled lines cannot be dragged

`crates/umlcraft-notation/src/route.rs:196`

- **Scenario (confirmed):** two nodes side by side whose centers differ by 1.5 in y (common in
  imported files, which `default_corners` deliberately leaves slanted). The segment counts as
  vertical (`same(a.y, b.y)` within 0.5 only), the x ranges are disjoint, and dragging it does
  nothing.
- **Fix:** decide orientation with `|dx| >= |dy|`.

<a id="n-8"></a>
#### N-8 · Low · Non-finite geometry makes items invisible and the SVG invalid

- **Scenario (confirmed for the SVG):** NaN or infinite coordinates (see [M-1](#m-1),
  [A-5](#a-5)) give `viewBox="-10 -10 520 inf"` and `NaN` attributes; the nodes and edges are
  invisible and cannot be clicked. `style.font_size` is not bounded either.
- **Fix:** with [M-1](#m-1); additionally skip non-finite rects in `svg::bounds`.

<a id="n-9"></a>
#### N-9 · Low · SVG export cuts off name arrows and end markers, drops fill alpha

`crates/umlcraft-notation/src/svg.rs:103-124`, `208`

- `bounds` ignores association name arrows (14 past the label) and end markers: with two narrow
  boxes and a long association name, the arrow tip at x = 179.6 lies outside the image ending at
  175.6 (confirmed).
- Only `rrggbb` is written; the canvas applies alpha (`canvas.rs:1441`). Add `fill-opacity` when
  alpha is not 255 (traced).

<a id="n-10"></a>
#### N-10 · Low · `keywords_away_from_notes` is quadratic in edges on every build

`crates/umlcraft-notation/src/scene.rs:881-893`

- `edges.iter().find(|e| e.id == p.id)` runs for every edge presentation before the cheap
  `lines.contains` test. Scene build goes from 1.7 ms at 1,000 edges to 5.6 ms at 2,000 (release).
  Test `lines.contains` first or index edges by id.

<a id="n-11"></a>
#### N-11 · Low · Small inconsistencies

- The doc comment "States: the name, centered…" sits on `CUBE_DEPTH` (`scene.rs:1250-1253`).
- `Paint::Translucent` is documented as the fill of system boundaries (`draw.rs:36`); boundaries
  use `Paint::None` (`draw.rs:308`), only partitions use it.
- "Astah width = max(exact, rounded)" is written out five times (`Line::astah_width_of`,
  `label_width`, the use case `width` closure, the object `astah` closure, `needs` in the
  classifier layout).
- `model.get(id).expect("caller checked")` appears seven times; passing `&Element` removes them.
- `edge_for` (236 lines, 11 parameters packed into tuples to pass `too_many_arguments`) and
  `layout_element` (186 lines) are the longest functions in the crate.

### Notation: editing (sequence, state, clipboard, member, object, palette)

<a id="e-1"></a>
#### E-1 · High · Paste fails at random

`crates/umlcraft-notation/src/clipboard.rs:120-126`, `196-215`, `255-260`

- **Defect:** the copied roots come from a `HashSet`, so their order is random. `insert_tree`
  defers weak references to copies not yet inserted, but not strong ones, so `InsertElement`
  fails with `InvalidReference` when a strong reference points to a later copy.
- **Scenarios (confirmed):**
  - Select all, copy, paste on the sample sequence diagram fails 29 times out of 40: messages in
    a copied fragment refer to lifeline copies not inserted yet.
  - Class B extends A with the generalization owned by B (the source import stores it so,
    `umlcraft-reverse/src/merge.rs:457`); copying A and B and pasting fails 18 times out of 40.
- **Fix:** keep the selection order; postpone any subtree element whose strong references point to
  copies not yet inserted and insert it in a second pass at its remembered owner and index.

<a id="e-2"></a>
#### E-2 · Medium · Paste and "show" make the wrong presentations on sequence diagrams

`crates/umlcraft-notation/src/clipboard.rs:100-111`, `286-301`; `palette.rs:741-757`

- **Defect:** paste recreates every node as `Shape::Node`; `show_element` does the same for a
  `CombinedFragment`; neither checks `can_show`.
- **Scenarios (confirmed):**
  - Pasted lifelines become empty boxes; drawing a message to one fails with "messages connect
    lifelines".
  - A successful select-all paste also duplicates the `Diagram` element (its frame is selected)
    and a fragment holding a message that is never drawn.
  - "Paste as same elements" of a class onto a sequence diagram succeeds although `can_show` says
    no; so does a duplicate paste of a class onto a state machine diagram.
  - On the next open, `upgrade_document` takes these nodes for version 1 content, lays out the
    diagram again and clears undo.
- **Fix:** route pastes through `can_show`, `add_lifeline` and `add_fragment`, then `tidy`; never
  copy the diagram frame or activation bars.

<a id="e-3"></a>
#### E-3 · Medium · Member text does not round-trip for imported constructs

`crates/umlcraft-notation/src/member.rs`

Confirmed with tests:

- **Variadic template parameters** (lines 84-90, 555-557): `+ f<Ts...>(…)` is applied back as the
  name `Ts...`; every edit adds three more dots and breaks the `Ts` link.
- **Unnamed parameters and operator names** (lines 275-281, 392-393), both produced by the C++
  import (`operator==( : const Shape&)`): "no valid name", `operator<` gives "missing >",
  `operator()` "unexpected …". These operations cannot be edited inline at all, not even their
  visibility.
- **Ambiguous types** (lines 426-432): with two classes named `Item`, renaming an attribute turns
  `Element(Item)` into `Text("Item")`, since the shown bare name does not resolve uniquely.
- **Anonymous bound classes** (line 14): `type_label` uses `name()`, not `display_name`, so the
  text reads `+ pair : ` and applying it clears the type.

Fix: strip `...` and set `is_variadic`; accept empty parameter names and `operator…` names; keep
an existing link when its name matches the typed text; use `display_name` in
`type_label`/`type_text`.

<a id="e-4"></a>
#### E-4 · Medium · Editing an operation recreates all its parameters

`crates/umlcraft-notation/src/member.rs:578-599`

- **Scenario (confirmed):** an imported parameter with multiplicity `*` and a stereotype loses both
  after a mere visibility change, and gets a new id, which unlinks comments annotating it.
- **Fix:** update parameters by position; create or delete only the difference.

<a id="e-5"></a>
#### E-5 · Medium · `parse_type` has no nesting limit

`crates/umlcraft-notation/src/member.rs:478-527` (recursion at 510)

- **Scenario (confirmed):** 1,000 levels of `List<…>` overflow a 1 MB stack (Windows main thread,
  debug build) and abort the app; parsing is quadratic (`split_top` rescans each level, 5,000
  levels take 2 s); 45 levels are accepted and saved but cannot be loaded ([M-1](#m-1)).
- **Fix:** cap the depth (for example 32 levels; the rest stays text).

<a id="e-6"></a>
#### E-6 · Medium · A create message's receiver bar is removed in the same `tidy`

`crates/umlcraft-notation/src/sequence_edit.rs:357-370` vs `568-584`

- **Defect:** the bar starts at `line_top` (below the head) but the message ends at the head's
  center, 15 higher; `drop_idle_bars` sees no message touching the bar and deletes it.
- **Scenario (confirmed):** after a create message, the created lifeline has no bar.
- **Fix:** count the create message's end as touching a bar starting at `line_top`, or anchor the
  bar at the message.

<a id="e-7"></a>
#### E-7 · Low · Destroy and operand edge cases

- `sequence_edit.rs:374`: a destroy message above the target's head gives a negative lifeline
  length (`y + 25 - line_top`); `fit_lifelines` (line 685) skips destroyed lifelines, so nothing
  repairs it (confirmed: create at 300, destroy at 150 → length −140). Deleting the destroy
  message never resets `destroyed`, so the cross stays (traced).
- Deleting an operand leaves its separator (`separators: [90.09]` stays and draws a stray line);
  deleting the last operand leaves a fragment without operands, and `sync_order` silently skips
  what is inside (confirmed). A `delete_operand` edit should remove the matching separator and
  guard offset.

<a id="e-8"></a>
#### E-8 · Low · Batch moves can close ownership cycles

`crates/umlcraft-notation/src/state_edit.rs:64`, `71-74`; `sequence_edit.rs:802`, `851-860`

- **Defect:** both check "not into itself or what it holds" against the model before any move,
  then apply several moves in a row.
- **Scenarios (confirmed):** two sibling states, each with its center inside the other, dragged
  together → `OwnershipCycle`, the move is rejected. On an inconsistent sequence diagram (C in A's
  operand, drawn A ⊂ B ⊂ C) `tidy` fails, and since `tidy_all` (lines 556-564) tidies every
  sequence diagram, every delete anywhere in the model then fails.
- **Fix:** recheck `is_ancestor` before each move and skip moves that would close a cycle; let
  `tidy_all` skip diagrams it cannot tidy instead of failing the edit.

<a id="e-9"></a>
#### E-9 · Low · Smaller editing defects

- `palette.rs:794-796`: new vertices take their owner from the click point (top-left), moved ones
  from their center, so a state created near the bottom-right inside "Big" belongs to Big and
  moves to the machine on the next zero-distance move (confirmed). Pass `origin + size / 2`.
- `object.rs:68-71`: a slot value is split at its first `=`, after the properties panel already
  split once (`properties.rs:756-759`); typing `name = "k=v"` stores `v"` (confirmed).
- `sequence_edit.rs:497-501`: `move_item` moves nested bars but not the messages that start them;
  a callback loses its bar after the outer call is moved (confirmed).
- `member.rs:218-236`: `<`, `>` and `->` in a parameter default change the bracket depth;
  `f(a : bool = x < y, b : int)` parses as one parameter (confirmed).
- `palette.rs:598-604`: `can_show` checks only the kind, so lifelines and fragments of another
  interaction can be shown, and messages between them reference another interaction's lifelines
  (traced).

### Astah import and export (`umlcraft-asta`, `umlcraft-asta-export`, `tools/asta-export`)

The primitives of the Java serialization reader are careful (checked arithmetic in `take`, capped
array preallocation, range- and type-checked handles, `MAX_DEPTH` for values, a 512 MB stack
thread). What is missing is protection at the structure level, both in `jser` and in the graph
`map` walks. `.asta` files come from other people, so these are the most important findings of
the review.

<a id="a-1"></a>
#### A-1 · High · Cycles in the Astah graph crash or hang the import, on the UI thread

`crates/umlcraft-asta/src/map.rs:481` (`walk`), `1176`/`1229` (`vertices`), `1241`/`1248`
(`partition`), `1342-1347` (fragment stack in `interaction`), `75-79` (`Reader::text`);
`crates/umlcraft-app/src/main_view.rs:333`

- **Defect:** the mapper records no visited objects, and `map::import` does not run inside the
  big-stack `deep()` wrapper; the app calls `umlcraft_asta::import` synchronously in the UI
  handler.
- **Scenarios (confirmed with crafted files):**
  - A package whose `ownedElementOwnership` lists itself → "has overflowed its stack", abort.
  - A partition whose `subGroup` contains itself → stack overflow, abort.
  - A name object whose `body` is itself → `import` still spinning after 5 s.
  - A combined fragment whose operand lists the fragment → still spinning after 5 s.
- **Fix:** a `seen: HashSet<ObjId>` in `Importer`, checked before recursing; a visited set for
  the fragment stack; an iteration cap in `text()`; run `map::import` inside `deep()`; do the
  whole import on the background executor, as the source import does.

<a id="a-2"></a>
#### A-2 · High · The class-description chain is trusted

`crates/umlcraft-asta/src/jser.rs:763-766`, `769`, `779`, `817`

- **Self reference (confirmed):** a class description whose superclass is a back-reference to
  itself is accepted; any object of that class loops in `instance()`, growing `hierarchy` until
  the allocation of 4 GiB fails and the process aborts.
- **Unbounded nesting (confirmed):** `class_desc` → `class_desc` recursion does not count toward
  `MAX_DEPTH`; 3 million nested descriptions in a 279 KB `.asta` overflow even the 512 MB stack
  (release); 300,000 in 28 KB suffice in debug.
- **Quadratic amplification (confirmed):** every instance gets one `ClassData` per ancestor, with
  a cloned name. A 3,000-long chain plus 3,000 instances is a 542-byte `.asta` that takes about
  920 MB and 3.8 s to parse.
- **Fix:** reject cycles while reading `super_class`; cap the chain length (for example 64);
  count class-description nesting in the depth; optionally a global budget for objects and field
  slots.

<a id="a-3"></a>
#### A-3 · High · The declared ZIP entry size is trusted

`crates/umlcraft-asta/src/lib.rs:46`

- **Defect:** `Vec::with_capacity(entry.size() as usize)` uses the size written in the file.
- **Scenario (confirmed):** a declared size of `u64::MAX - 16` panics with "capacity overflow" on
  the UI thread; 1 TiB aborts with "memory allocation of 1099511627776 bytes failed". The zip
  crate limits output to the declared size, but nothing limits the declared size, so a plain zip
  bomb works too.
- **Fix:** reject entries above a sane limit (1–2 GiB), preallocate `min(size, cap)` and read
  through `.take(cap + 1)`.

<a id="a-4"></a>
#### A-4 · Medium · Exponential expansion of bound types

`crates/umlcraft-asta/src/map.rs:755-779`

- **Scenario (confirmed):** the depth limit of 8 does not bound the work: k substitutions naming
  the same anonymous bound class cost k⁸. A 413-byte file takes 1.9 s at k = 5, 8.2 s at 6, 28 s
  at 7; k ≥ 10 effectively hangs while building a huge `TypeExpr`.
- **Fix:** a visited set along the expansion path (a self reference becomes text) and a budget on
  the total number of nodes.

<a id="a-5"></a>
#### A-5 · Medium · NaN and infinity reach the model from `.asta` files

`crates/umlcraft-asta/src/map.rs:88-90` (`num`), `103-105`, `1523-1527`, `2183`; font sizes in
`fill()` via `parse::<f32>()` (accepts "inf")

- **Scenario (confirmed):** a `TextPresentation` with `x = NaN` imports as `Rect { x: NaN, … }`;
  the project then saves as `null` and cannot be reopened ([M-1](#m-1)). The same JSON goes to
  Java in the export.
- **Fix:** filter `is_finite()` in `Reader::num` and in style parsing, plus the model-level check
  of [M-1](#m-1).

<a id="a-6"></a>
#### A-6 · Medium · Predictable, reusable export temp folder

`crates/umlcraft-asta-export/src/lib.rs:206-218`

- **Defect:** the folder is `temp_dir()/umlcraft-asta-export-{pid}-{n}` and created with
  `create_dir_all`, which succeeds if it (or a symlink to one) already exists; default
  permissions.
- **Scenario (traced):** on a Linux machine with a shared `/tmp`, another user pre-creates the
  folder and swaps `AstaExport.java`, which Astah's Java then compiles and runs as the user; they
  can also read the project. Per-user temp folders on Windows and macOS mitigate this, and the
  export is effectively Windows-only today ([A-10](#a-10)).
- **Fix:** random name, `fs::create_dir` (fail if present), mode 0700; or the `tempfile` crate.

<a id="a-7"></a>
#### A-7 · Medium · The export's Java process has no timeout or cancel

`crates/umlcraft-asta-export/src/lib.rs:148-150`

- **Scenario (traced):** if the JVM hangs (license dialog, AWT stall, a huge model at
  `-Xmx1024m`), `Command::output()` never returns, `busy` never clears, and further exports and
  source imports are refused until the app restarts.
- **Fix:** `spawn`, drain both pipes on threads, wait with a deadline and kill the child; show
  progress with a cancel button.

<a id="a-8"></a>
#### A-8 · Low · Steps outside `attempt` abort the whole export

`tools/asta-export/AstaExport.java:196-201`, `249-256`, around `886-913`

- The interaction-use loop (`setSequenceDiagram`) can fail the whole export.
- The second transaction (`setLocation` of substates) runs after the first one committed; a
  failure there saves nothing.
- `note()`, `style()`, `setLocation` and `resize` in diagrams are not wrapped; one bad node drops
  the whole diagram, which stays half-built in the project while the report counts it as dropped.
- Fix: wrap each item, as the README's rule says. Confidence: traced (these API methods declare
  `InvalidEditingException`).

<a id="a-9"></a>
#### A-9 · Low · Export robustness details

- `AstaExport.java:2210-2211`, `2299`: problem lines embed raw element names; a name containing
  `\n` or `\t` splits the line, and `lib.rs:157` can then report a successful export as
  `ExportError::Astah`. Normalize whitespace as `describe()` already does.
- `lib.rs:144-146`: absolute temp paths are passed although the child runs in the temp folder;
  the Windows `java` launcher decodes arguments in the ANSI code page, so a profile path outside
  it is mangled (plausible). Pass bare file names.
- `lib.rs:180`: `fs::copy(out.asta, target)` overwrites the target in place; a failure midway
  (disk full) leaves the user's existing file truncated. Copy to a sibling temp file and rename.

<a id="a-10"></a>
#### A-10 · Low · Performance, docs and structure

- `jser.rs:769` clones the full `ClassDesc` (fields and annotation) for every class of every
  instance, and `779` copies each field name into every instance: tens of millions of `String`
  allocations on large projects. Borrow by index and store values by position.
- `View::get` is linear inside `edge()`, `lane_of` and `boundary_text` (`map.rs`): O(P²) per
  diagram, minor at today's sizes.
- `tools/asta-export/README.md:20` says `layout.json` carries "message rows and fragment frames
  of sequence diagrams"; `layout.rs` and the architecture doc say sequence diagrams need none.
  The README's command line lacks `-Xmx1024m`.
- `map.rs:4-7` lists only part of what is imported (state machines, component and deployment
  diagrams, instances, links and interaction uses are missing).
- `Astah::find` only knows Windows install paths; Linux and macOS installs are found only through
  `ASTAH_HOME`, and an invalid `ASTAH_HOME` does not fall back. The docs could say so.
- `jser.rs:697`, `798`: `WrongHandle(0, …)` always reports handle 0.
- `jser/write.rs`, `write_entity_store` and `write_asta` serve only the `asta-rewrite` example and
  `tests/rewrite.rs`. Document them as tooling (or put them behind a feature); they are also the
  natural basis for synthetic test fixtures ([X-1](#x-1)).
- `map.rs` is 2,341 lines in one `impl`; modules for structure, behaviors, diagrams and sequence
  diagrams, sharing one visited guard, would help.

### Source import (`umlcraft-reverse`)

The front-ends are defensive (no `unwrap` on tree nodes, syntax errors reported with line and
column), results are deterministic, and preprocessor blanking keeps byte positions and valid
UTF-8. The weak spots are in the mapping, on real-world source trees.

<a id="r-1"></a>
#### R-1 · High · One name declared with two kinds aborts the whole import

`crates/umlcraft-reverse/src/merge.rs:153-163`, `179`, `246-247`

- **Defect:** `declare_type` retypes the existing element when a later declaration has another
  kind; earlier `Entry`s keep the deleted id, and `properties()` then calls `tx.update` on it.
- **Scenario (confirmed):** `a/a.h: class Node { int x; };` and `b/b.h: enum Node { A, B };`; or
  in Python `x/util.py: class Mode:` and `y/util.py: class Mode(Enum):` (non-package folders share
  the module `util`). `import_into` returns "no element or presentation <id>" and the whole import
  rolls back with a message the user cannot act on. Common in multi-program C++ trees, Python
  script and test folders, and multi-module Java projects.
- **Fix:** after a retype, rewrite `entries[*].id` and the `aliases` keys to the new id; better,
  settle each (owner, name) kind before creating, or keep conflicting kinds apart and report them.

<a id="r-2"></a>
#### R-2 · Medium · Imports used only as qualifiers create bogus placeholder classes

`crates/umlcraft-reverse/src/merge.rs:212-240`

- **Scenario (confirmed):** `import torch.nn as nn` with `class Net(nn.Module)` gives `torch › nn`
  twice, as a class `nn` and a package `nn › Module`. The same for `from django.db import models`
  with `models.Model`, and C++ `namespace fs = std::filesystem;` with `fs::path`.
- **Fix:** pre-resolve a single import only when its visible name is used as a simple name; the
  qualified resolution already creates the right placeholders.

<a id="r-3"></a>
#### R-3 · Medium · Alias-typed parameters break re-import idempotence

`crates/umlcraft-reverse/src/merge.rs:626-634` vs `823-839`

- **Defect:** the source signature uses the written name (`O`), the model signature the resolved
  element's name (`Order`); they never match.
- **Scenario (confirmed):** Python `from shop.orders import Order as O` with
  `def add(self, o: O)`, or C# `using O = Shop.Order;` with `void Add(O o)`: every re-import
  deletes `add` and creates it with a new id; messages in sequence diagrams that referred to it
  are unlinked.
- **Fix:** build the source signature from the resolved `TypeRef` via `type_text`, or map alias
  names to their target's last segment in `simple_text`.

<a id="r-4"></a>
#### R-4 · Medium · Recursion over Java method bodies can overflow the stack

`crates/umlcraft-reverse/src/java/mod.rs:431-447` (`count_nested_classes`)

- **Scenario (confirmed, release):** `return "s0" + "s1" + …` with about 10,000 terms overflows a
  2 MB thread stack (the std default of GPUI's Linux workers), about 2,500 terms a 512 KB one; the
  process aborts. Generated code (string tables, big array initializers) produces such
  expressions. The C# front-end parses 100,000 terms on 512 KB, so tree-sitter is not the limit.
- **Fix:** walk with a `TreeCursor` or an explicit stack.

<a id="r-5"></a>
#### R-5 · Medium · Folder collection has no exclusions and fails on one unreadable folder

`crates/umlcraft-reverse/src/source.rs:29-40`

- **Defect:** every subfolder is walked, including `.git`, `.venv`, `venv`, `site-packages`,
  `node_modules`, `bin`, `obj`, `build` and `_deps`; `read_dir(dir)?` and `entry?` abort the whole
  collection, and the app shows "Permission denied (os error 13)" without the path
  (`main_view.rs:428-441`). Symlinked folders are not followed (`file_type()` does not follow
  links), so there are no symlink loops.
- **Scenarios (traced):** a uv or venv project imports all of site-packages from `.venv`; a C#
  folder imports `obj/**/*.g.cs`, and partial WPF classes get duplicated `_contentLoaded` and
  `InitializeComponent` from the Debug and Release copies; one root-owned folder in the tree
  aborts the import.
- **Fix:** skip hidden folders and a list of build and vendor folders (or folders containing
  `pyvenv.cfg`); report unreadable folders with their path as file problems and continue.

<a id="r-6"></a>
#### R-6 · Medium · `.py` and `.pyi` of one module are merged as parts

`crates/umlcraft-reverse/src/lib.rs:70-85`, `source.rs:46`

- **Scenario (confirmed):** `shop/orders.py` and `shop/orders.pyi` both declare `Order`; the class
  gets two attributes `x`, two `__init__` and two `total`. attrs, for example, ships `.pyi` files
  next to its `.py` files.
- **Fix:** where both exist, import only the `.pyi`, as type checkers do.

<a id="r-7"></a>
#### R-7 · Medium · Name lookup is linear, so large flat namespaces are quadratic

`crates/umlcraft-reverse/src/resolve.rs:58-70`, `85-90`

- **Defect:** `child_type`/`child_package` scan all children with a hash lookup per child.
  `enclosing()` leaves out the root, so with `enclosing_namespaces` (C#, C++) a global type is
  found only through `placeholder_under(root)`, which scans the root again.
- **Measured (C++ headers, N global structs with 11 references each):**

  | N | `apply` | elements |
  | --- | --- | --- |
  | 1,000 | 0.36 s | 13,000 |
  | 2,000 | 0.71 s | 26,000 |
  | 4,000 | 2.6–3.2 s | 52,000 |
  | 8,000 | 13.3 s | 104,000 |

  About 61% of the time is `nested_placeholder → child_type` and `Model::get` hashing. Plain C
  headers and generated SDK packages hit this.
- **Smaller costs of the same kind:** `by_type` grouping is O(T²) (`merge.rs:204`); `definitions`
  calls `by_type.iter().any` per definition (`merge.rs:274`); operation matching recomputes
  `signature_of_model` inside `position` (`merge.rs:586-597`); every `retype` scans the model
  about three times.
- **Fix:** a `HashMap<(owner, name), Id>` index of types and packages for the import, updated on
  create and retype; a `HashMap` for `by_type`; model signatures once per class.

<a id="r-8"></a>
#### R-8 · Low · Smaller import defects

- `preprocessor.rs:197`: C's line continuation is applied to C# too, so a comment ending in `\`
  (`// C:\temp\`) swallows the next `#endif` and everything after it is blanked without an error
  (confirmed). Make line splicing a C++-only rule.
- `cpp/mod.rs:835`: `operator==` and `operator ==` become two operations when the declaration and
  the definition spell them differently (confirmed). Remove whitespace inside operator names.
- `merge.rs:500`, `612`: a definition's own `using`s are used only to find its owner, so
  `using namespace lib; void A::f(Str s) {}` links `Str` to a new placeholder `::Str`, not
  `lib::Str` (confirmed).
- `associations.rs:121-158`: any unused association with a named, navigable end between imported
  classes counts as a former field association, so an association drawn by hand is removed on
  re-import (traced). Mark associations the import creates.
- `merge.rs:170-176`: a new C# partial class in two files is reported as 1 created and 1 updated.
- Each front-end repeats `named_children`, `children`, `text`, `count`, `squeeze` and the parser
  setup; the C# and C++ `documentation()` are nearly identical. A shared tree-sitter helper module
  would remove about 100 lines and give one place for iterative traversal ([R-4](#r-4)).

### Application (`umlcraft-app`)

No model change bypasses `Document::edit`/`Tx`, every UI action is one labelled undo step,
`Workspace::validate` keeps tabs and selection consistent after undo and delete, and exports work
on snapshots taken before the file dialog.

<a id="u-1"></a>
#### U-1 · High · The properties panel loses an edit when focus moves to an earlier field

`crates/umlcraft-app/src/properties.rs:577-586`, `595`

- **Defect:** on `Focus` the panel sets `editing = editing.or(shown)`, on `Blur` it commits and
  sets `editing = None`. GPUI calls window focus listeners in registration order (a
  `SubscriberSet` keyed by subscription id, `gpui-pre` `window.rs` around line 3547); each
  `InputState` registers its `on_focus`/`on_blur` in its constructor (`gpui-base`
  `input/base/state.rs:852-853`) and emits `InputEvent::Focus`/`Blur`, which are delivered in
  order. When focus moves to a field created before the current one, `Focus(new)` arrives before
  `Blur(old)`, and the new field ends up with `editing == None`.
- **Scenario:** select a class, type in Stereotypes (or Documentation), click into Name, type a
  new name, press Enter: `commit` returns at `let Some(id) = self.editing else { return }`, the
  next blur or selection change does the same, and the field is reset. The rename is lost without
  a message.
- **Fix:** keep the target per field (`HashMap<Field, Id>`, set on Focus from `shown_id()`, taken
  on Blur).
- **Confidence:** traced through the app and the GPUI sources; not reproduced in a running app.

<a id="u-2"></a>
#### U-2 · High · Moving a container drags along nodes it passes over

`crates/umlcraft-app/src/canvas.rs:467-468` (preview), `893-895` (commit), `1580-1589`

- **Defect:** the preview and the commit both call `scene::with_contents(&self.scene, …)`, and
  `self.scene` is the previous frame's scene, in which the container is already moved. Containment
  is therefore tested at the dragged position, against nodes that are drawn unmoved.
- **Scenario (confirmed by simulating the canvas loop):** package at (0, 0, 300, 300), class X at
  (400, 50); dragging the package by 350 makes X alternate between moved and unmoved on every
  frame. On release, whether X moves depends on whether the number of mouse-move frames was odd
  or even. On state diagrams `reparent_vertices` then also makes X a substate. Undo restores it.
- **Fix:** compute the moved set once on mouse-down, from the scene without preview, store it in
  `Drag::Move`, and use it for the preview and the commit.

<a id="u-3"></a>
#### U-3 · Medium · Leaving an element writes back every field and re-parses types

`crates/umlcraft-app/src/properties.rs:826-835` (with `691-716`, `782-796`)

- **Defect:** if any field of an element had focus, a selection change commits all text fields.
  Value type, bounds and defaults have no "unchanged" check and re-run `parse_type` on the
  unqualified names shown; constraints are re-split at commas, names trimmed; every field that
  changes becomes its own undo step.
- **Scenario (traced; the parse behavior is confirmed in [U-5](#u-5)):** a template parameter's
  bound is `Node`, and there are two classes `Node` (common after an import). Click into the
  parameter's name, then elsewhere: the bound silently becomes the free text "Node".
- **Fix:** commit only fields the user changed (track `InputEvent::Change`) and compare with the
  text that was loaded.

<a id="u-4"></a>
#### U-4 · Medium · Uncommitted text is neither saved nor asked about

`crates/umlcraft-app/src/main_view.rs:222-226`, `651-707`, `740-753`

- **Defect:** fields commit only on Enter or blur; Ctrl+N, Ctrl+O and Ctrl+S are window-wide
  bindings that do not move focus, and neither does the window's close button.
- **Scenarios (traced):**
  - Type a name without Enter, press Ctrl+N: `is_dirty()` is false, the document is replaced
    without a prompt, the edit is gone.
  - Ctrl+O: the native file dialog deactivates the window, blur commits the edit after the dirty
    check, and the open discards it without a prompt.
  - Close button: the window closes without a prompt (plausible).
  - Ctrl+S: the file is written without the pending text.
- **Fix:** a `flush_edits()` (canvas `commit_edit` plus a commit of the focused properties field)
  at the start of `confirm_discard`, `should_close`, `save` and the exports.

<a id="u-5"></a>
#### U-5 · Medium · An inline edit is applied even when nothing changed

`crates/umlcraft-app/src/canvas.rs:1155-1167` → `workspace.rs:287-303`, `600-636`

- **Scenario (confirmed):** double-click a member row and click away without typing:
  - `+ root : Node` turns its type reference into `Text("Node")` when the name is ambiguous;
  - on an operation all parameters are deleted and recreated with new ids ([E-4](#e-4)), an undo
    step is added and the document becomes dirty;
  - free texts imported from Astah or resized by hand are reset to the computed size.
- **Fix:** keep the original text in `InlineEdit` and skip unchanged commits, as the Declaration
  field of the properties panel already does (`properties.rs:603`).

<a id="u-6"></a>
#### U-6 · Medium · A background source import merges into whatever project is open when it ends

`crates/umlcraft-app/src/main_view.rs:258-311`, `432-436`

- **Defect:** `busy` blocks only a second import or export. New, Open, Open Recent and Open Sample
  still run; `replace_document` swaps the document inside the same `Workspace`, and `apply_source`
  then applies the parsed sources to it.
- **Scenario (traced):** import a large Java folder, then open project B: B receives the imported
  classes as an undo step and becomes dirty.
- **Fix:** record a document generation when the import starts and discard (or ask) if it
  changed, or disable file operations while busy.

<a id="u-7"></a>
#### U-7 · Medium · Per-frame and per-edit work scales with the whole model

`crates/umlcraft-app/src/canvas.rs:1580-1587`, `workspace.rs:39-48`, `176-199`,
`main_view.rs:141-153`

- Every render runs the full `scene::build` with text shaping: on every mouse move during a drag,
  every pan and zoom step, every caret blink of views that are not cached. `object::slots` scans
  all model elements per object inside it.
- Every edit, even a rename, runs `fitted_boxes` twice over all auto-sized boxes of all views of
  the model (new classifiers are auto-sized by default, `palette.rs:749`) and matches the results
  with an O(n²) `before.iter().any` plus linear `View::get`.
- The model tree is rebuilt completely on every revision.
- **Fix:** cache the scene by (revision, diagram, preview, locale, column setting) and keep it on
  pan and zoom; fit only presentations the transaction touched and index `before` by id.
- **Confidence:** traced; the actual cost on large imported diagrams could not be measured here.

<a id="u-8"></a>
#### U-8 · Low · Some edits skip the automatic refit

`crates/umlcraft-app/src/workspace.rs:333-342`, `346-362`, `496-503`

- `connect_message`, `create_fragment` and `update_style` call `self.doc.edit` directly instead of
  `self.edit`. Ticking "Hide attributes" on an auto-sized class leaves an empty band that never
  refits, since the content comparison is per edit (traced). Route them through `self.edit`.

<a id="u-9"></a>
#### U-9 · Low · User-visible English texts outside the locale files

Against the project rule that UI texts go only through `locales/`:

- `main_view.rs:729`: the dialog button `&["OK"]`.
- `properties.rs:312-336`: field placeholders such as "filled, paid", "ordered, unique".
- `workspace.rs:208`, `main_view.rs:326`, `703`: `ModelError`/`FormatError` `Display` texts in the
  status bar and dialogs, with raw UUIDs ("invalid model: no valid name in …", "… refers to
  0199…, which does not exist"); also in the German UI.
- `properties.rs:777`: the multiplicity parse error; `png.rs:20`, `22`: PNG export errors.
- Fix: keys for the button and placeholders; a locale key plus arguments per error variant,
  translated in the app.

<a id="u-10"></a>
#### U-10 · Low · Shortcuts and quitting per platform

`crates/umlcraft-app/src/main.rs:127-155`

- Shortcuts use `ctrl-` instead of GPUI's `secondary-`, so on macOS they are Ctrl, not Cmd, and
  Ctrl+Z typed in a field undoes a model change (Input binds Cmd+Z there).
- On Windows and Linux, Input binds only Ctrl+Z and Ctrl+Y, so Ctrl+Shift+Z in a field runs the
  model's Redo; `sync` (`properties.rs:843-1031`) then reloads every field and wipes the text
  being typed. A background import finishing while the user types does the same.
- No `on_app_quit` or reopen handler: on macOS the app keeps running after its window closes
  (plausible).
- Fix: `secondary-` shortcuts; quit and reopen handling on macOS.

<a id="u-11"></a>
#### U-11 · Low · State that survives opening another document

`crates/umlcraft-app/src/main_view.rs:248-256`

- `replace_document` resets workspace and tree but not `notice` or `clipboard`. `notice` is keyed
  by revision number, so an old "Imported …" message comes back when the new document reaches
  the same revision; "Paste (same elements)" after opening another project fails with an English
  error containing a raw UUID (traced). Clear both.

<a id="u-12"></a>
#### U-12 · Low · Settings location and corrupt files

`crates/umlcraft-app/src/settings.rs:22-46`

- macOS uses `~/.config/UmlCraft`, not `~/Library/Application Support/UmlCraft`; Linux ignores
  `XDG_CONFIG_HOME`; the module doc and `docs/architecture.md` mention only `%APPDATA%`.
- One wrongly typed field (`"dark": "yes"`) resets all settings including recent files, and the
  next save overwrites the file; two running instances overwrite each other's recent files.
- The write is a plain `fs::write`, not temp file and rename.
- Fix: read fields leniently (per-field defaults), write via temp file and rename, use the
  platform's config folder, update the docs.

<a id="u-13"></a>
#### U-13 · Low · Relative command-line paths and stale recent files

`crates/umlcraft-app/src/main.rs:176-177`

- `umlcraft foo.ucraft` from a terminal stores the relative path in recent files; opening it
  later from another working directory fails, and the file dialog's start folder becomes "".
  Recent entries whose file is gone stay in the list after the error. Make paths absolute; drop
  entries that fail to open.

<a id="u-14"></a>
#### U-14 · Low · Rename from the model tree can focus the wrong field

`crates/umlcraft-app/src/main_view.rs:205-211`, `properties.rs:564-571`

- `focus_name` chooses between Name and Declaration from the element the panel showed before the
  action (observers run after the handler; a right-click in the tree does not select). Rename on
  an attribute while a class is shown focuses Name, which members do not render (plausible).
  Decide from `ws.selected_element`.

<a id="u-15"></a>
#### U-15 · Low · Loading and `.asta` import block the UI

`crates/umlcraft-app/src/main_view.rs:319`, `333`

- Loading `.ucraft` files and importing `.asta` files run synchronously in the UI handler; a large
  file freezes the window without progress, and any crash there ([A-1](#a-1) to [A-3](#a-3))
  takes the app down. Run both on the background executor, as the source import does.

### CI, release and packaging

<a id="x-1"></a>
#### X-1 · Medium · 21 tests pass on CI without testing anything

`crates/umlcraft-asta/tests/import.rs:11-21`, `tests/rewrite.rs:7`,
`crates/umlcraft-asta-export/tests/roundtrip.rs:611-621`,
`crates/umlcraft-reverse/tests/astah_compare.rs:283`

- **Defect:** the 11 import tests and the rewrite test look for Astah's samples under the fixed
  path `C:\Program Files\astah-professional` (ignoring `ASTAH_HOME`), the 8 round-trip tests need
  an Astah installation, `astah_compare` needs local reference files. Without them they print
  "skipped" to stderr and return, which counts as passed. CI runners have no Astah, so on all
  three platforms the `.asta` mapping (`map.rs`) and `Export::run` are tested only by 5 + 3 unit
  tests.
- **Confirmed:** `cargo test -- --nocapture` in this container shows every one of them skipping
  in 0.00 s.
- **Fix:**
  - Mark them `#[ignore = "needs Astah"]` and run them with `--ignored` where Astah exists, so the
    test summary shows them as ignored instead of passed.
  - Add synthetic fixtures: `jser::Graph` plus `write_entity_store`/`write_asta` can build `.asta`
    files in a test, without any Astah content in the repository. Start with one mapping test per
    element kind and the hostile cases of [A-1](#a-1) to [A-5](#a-5).
  - Add a fuzz target (`cargo fuzz`) for `parse_entity_store` + `map::import` and for
    `umlcraft_format::from_str`.
  - Move the export's report parsing out of `Export::run` into a pure function and unit-test it
    (dropped, problem, error, license failure, missing report, a name with a newline).

<a id="x-2"></a>
#### X-2 · Low · Toolchain and dependency hygiene

- No `rust-version` in `Cargo.toml` and no `rust-toolchain.toml`; CI and releases build with
  whatever `stable` is that day. Edition 2024 needs at least 1.85, and the code uses let chains
  (1.88). Declare `rust-version` so contributors get a clear error, and consider pinning the
  release toolchain for reproducible builds.
- No advisory or license check in CI. `cargo about` runs only in the release workflow, so a
  dependency with a license outside `about.toml`'s list is found only when tagging. `cargo deny
  check` (advisories, licenses, bans, sources) in CI would catch both early.
- Third-party actions are pinned to major tags (`dtolnay/rust-toolchain@stable`,
  `Swatinem/rust-cache@v2`, `taiki-e/install-action@v2`, `snapcore/action-build@v1`). The
  release workflow's `publish` job has `contents: write`; pinning actions to commit SHAs (with
  Dependabot updating them) closes the supply-chain path. The downloaded nfpm, appimagetool and
  AppImage runtime are already checked by SHA-256, which is good.
- The release workflow builds from the tag without running the tests; a tag on a commit that never
  went through CI produces packages. A `needs:` on a test job (or a check that the tagged commit
  is on `main` with green CI) avoids that.
- No `[profile.release]` settings. `lto = "thin"`, `codegen-units = 1` and `strip = true` would
  shrink the GPUI binary noticeably; optional.

<a id="x-3"></a>
#### X-3 · Low · File associations are missing outside Linux

- `packaging/macos/Info.plist` declares no `CFBundleDocumentTypes` or exported UTI for `.ucraft`
  (or `.asta`), so Finder does not offer UmlCraft for project files.
- The Windows release is a plain `.zip` without file association.
- On Linux, `com.umlcraft.app.desktop` and the MIME file cover `.ucraft` only; `.asta` files,
  which the app opens directly, are not associated.

<a id="x-4"></a>
#### X-4 · Low · Platform-specific test

- `main_view.rs`'s `recent_label` test runs only on Windows, so Linux and macOS CI never exercise
  the label logic for their own path forms.

### Documentation

<a id="d-1"></a>
#### D-1 · Low · Drift between docs and code

- `docs/architecture.md:98`: slots are "own first, then the generals"; code and test put the
  generals first.
- `docs/architecture.md:167`: a vertex "created there belongs to the innermost state drawn around
  its center"; creation uses the top-left corner ([E-9](#e-9)).
- `docs/architecture.md:251`: members are "put in source order"; `merge.rs:522` groups literals,
  then attributes, then operations.
- `docs/architecture.md:272`: the `source-import` example takes `<java|csharp|cpp>`; it also takes
  `python`.
- `docs/architecture.md` ("User interface") and `settings.rs:1`: settings live in
  `%APPDATA%\UmlCraft\settings.json`; on Linux and macOS they are in `~/.config/UmlCraft`
  ([U-12](#u-12)).
- "Every region keeps at least 20" does not hold for small states ([N-6](#n-6)).
- `tools/asta-export/README.md:20` and `map.rs:4-7`, see [A-10](#a-10).

<a id="d-2"></a>
#### D-2 · Low · The handoff document is outdated

- `docs/handoff-source-import.md` describes adding C#, C++ and Python as work to do; all three
  shipped (README, CHANGELOG). It still names `--example java-import` (lines 102, 238), which is
  now `source-import`.
- It is also the only place in the repository where the project rules from the local
  `CLAUDE.md` are written down. Moving them to a `CONTRIBUTING.md` (commands, language of docs and
  comments, i18n rule, `Document::edit` rule, no Astah files) and archiving the handoff would keep
  the rules visible for contributors.
- `.gitignore` excludes `docs/*` except two files, so new docs (this review included) must be
  added with `git add -f` or listed there.

## Test coverage

| Area | Tests | Well covered | Missing |
| --- | --- | --- | --- |
| model | 9 unit, 19 integration | cascade delete, weak unlinking, templates, generalization cycles, move/undo, z-order, dirty tracking | randomized apply/undo/redo/rollback property test (a throwaway version found no bug and would be cheap to keep), overlapping `delete_many`, a delete failing midway, hostile `from_records` cases, performance guards |
| format | 7 | byte-exact round trip, newer/foreign/orphaned files | non-finite floats, deep nesting, BOM, failing `save` |
| notation | 96 | Astah metrics, routing corners, label placement, hollow hit testing, `render_scaled`, main editing paths | `restyle`, `message_label_at`, self-loops per line style, degenerate geometry, control characters in SVG, many regions in a small state, `with_contents` on a preview, align of grown nodes, paste with strong references (repeated or fixed order), paste into sequence and other diagram kinds, member-text round trips of imported constructs, nesting limit, `tidy` idempotence, operand and destroy deletion, create-message bars |
| asta | 5 unit (+12 that need Astah) | `jser` primitives, byte-exact rewrite (with Astah) | everything in `map.rs` on CI, hostile input |
| asta-export | 3 unit (+8 that need Astah) | layout sidecar | report parsing, temp-folder handling |
| reverse | 23 unit, 123 integration | IR and model snapshots per language and rule; undo/redo/second import for C#, C++ and Python | the same loop for Java; duplicate declarations with different kinds; `.py` + `.pyi`; alias-typed parameters; qualifier-only imports; hidden/build/venv/unreadable folders; deep expressions; C# backslash comments; operator spelling; `large_imports_are_fast` is ignored, asserts no limit and is too small to reach [R-7](#r-7) |
| app | 8 | workspace consistency, recent files | properties commit and sync (focus order, unchanged text), canvas math (`to_logical`, `zoom_at`, `snap`), preview vs commit, file flows, corrupt settings |

All tests measure against `ApproxMeasure`; the real-font measurement in the app is not covered.
`workspace.rs` has no GPUI dependency and could move into a crate of its own whose tests run
without GPUI's system libraries; gpui-kit's test support should allow a headless focus test for
the properties panel.

## Structure and maintainability

The crate boundaries are right: model, format, notation, importers and exporters have no UI
dependency, and the app is thin over them. Inside the crates, a few modules have outgrown
themselves:

| File | Lines | Suggested split |
| --- | --- | --- |
| `notation/src/scene.rs` | 3,857 (about 1,240 of them tests) | `scene/mod.rs` (types, `build`), `text.rs` (`TextMeasure`, `Line`, `wrap`, the one width rule), `nodes/{classifier,use_case,state,activity,instance}.rs`, `edges.rs` (`edge_for`, qualifiers, labels), `interact.rs` (hit testing, `nodes_in`, `with_contents`, handles, `align`, `restyle`); frames shared with sequence diagrams in their own module |
| `asta/src/map.rs` | 2,341 | structure, behaviors, diagrams, sequence diagrams; one shared visited guard |
| `app/src/properties.rs` | 1,907 | one table per field (applies to which kinds, load, store) instead of parallel `match`es in `sync`, `commit` and `render`; this duplication is what makes [U-3](#u-3) possible. `associations_for`, `operations_for`, `classifiers` and `split_types` belong in `umlcraft-notation` |
| `app/src/canvas.rs` | 1,838 | `paint.rs` (transform, painter), `measure.rs` (`FontMeasure`, `AppMeasure`), context menu, inline editing, drag state machine |
| `app/src/main_view.rs` | 1,535 | `files.rs` (prompt, open, import, save, export, recent files), `menus.rs`, `model_tree.rs` |
| `tools/asta-export/AstaExport.java` | 2,498 | acceptable for a single-file source launcher; keep every item inside `attempt` |

Other duplication: `sequence.rs` and `sequence_edit.rs` each have their own bar model with
different rules (`bar_at` picks the deepest level in one, the shortest bar in the other), and
"inside a fragment" means the sender's center in `fit_fragments` but both ends in `sync_order`,
so a message crossing a frame edge grows the fragment without being put into it. The tree-sitter
helpers are repeated in every front-end ([R-8](#r-8)).

## Strengths

- **One way to change the model.** `Model::apply` validates every primitive operation and returns
  its exact inverse; a randomized test of 39 seeds × 150 operations (creates, moves, deletes,
  `delete_many`, presentation operations and edits failing partway) restored every state exactly,
  child order and z-order included. No code path in the app, the importers or the editors goes
  around `Document::edit`.
- **Load-time validation.** `Model::check` covers single root, ownership rules, double owners,
  reachability, reference roles and kinds, generalization cycles (iteratively), views matching
  diagrams and presentation endpoints.
- **A deterministic, diff-friendly file format** with sorted ids, omitted defaults, a version and
  rejection of newer files.
- **Shared geometry.** Canvas, hit testing, inline editing, SVG and PNG all use `draw::Primitive`;
  Astah's text metrics are reproduced from measured tables, so imported diagrams keep their text
  positions.
- **A language-neutral source import.** Four front-ends lower to one IR; language differences
  are `Rules` data, not branches; results do not depend on file order; re-import keeps ids,
  presentations and hand-drawn dependencies.
- **Careful export through Astah's API**, with what the API cannot express reported to the user
  rather than silently lost, and no shell involved in starting Java.
- **Consistent hygiene:** `unsafe` forbidden, clippy and rustfmt clean on three platforms, complete
  bilingual locale files, release packages checked against the workspace version and the
  AppStream metadata, downloaded packaging tools verified by SHA-256.

## Suggested order of work

1. **Crash and data-loss fixes** (small, local): [A-1](#a-1)–[A-5](#a-5), [M-1](#m-1),
   [E-5](#e-5), [R-4](#r-4), [U-1](#u-1), [U-2](#u-2), [U-4](#u-4), [U-6](#u-6), [M-5](#m-5);
   move `.asta` import and project loading off the UI thread ([U-15](#u-15)). Each comes with the
   regression test described in its finding.
2. **Test infrastructure:** [X-1](#x-1) (ignored instead of passing, synthetic `.asta` fixtures,
   fuzz targets), a property test for `apply`/undo, the Java re-import loop.
3. **Correctness of everyday edits:** [E-1](#e-1)–[E-4](#e-4), [E-6](#e-6), [U-3](#u-3),
   [U-5](#u-5), [N-5](#n-5), [R-1](#r-1)–[R-3](#r-3), [R-5](#r-5), [R-6](#r-6).
4. **Model invariants:** [M-6](#m-6)–[M-9](#m-9).
5. **Performance:** indexes for [M-2](#m-2)–[M-4](#m-4), [N-2](#n-2), [R-7](#r-7); scene caching
   and targeted refitting ([U-7](#u-7)); add timing guards with the sizes the docs advertise.
6. **Structure and docs:** split `scene.rs`, `map.rs`, `properties.rs`; table-driven properties
   panel; shared tree-sitter helpers; [D-1](#d-1), [D-2](#d-2); [X-2](#x-2)–[X-4](#x-4); the
   remaining Low findings.
