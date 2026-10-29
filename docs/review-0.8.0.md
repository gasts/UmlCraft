# UmlCraft 0.8.0 code review (merged)

Merge of two independent full reviews of `main` at 6c397f2 (version 0.8.0), both from 2026-10-29:

- **F** — `docs/review-fable51.md` (Fable 5.1), IDs H1–H6, M1–M20.
- **O** — `docs/review-opus55.md` (Opus 5.5), IDs by area (A-, M-, N-, E-, R-, U-, X-, D-).

Every finding below names its sources, so details and exact reproduction steps can be looked up in
the originals. Where both reviews found the same defect, it appears once; where they rated it
differently, the higher severity is used and the difference noted. Line numbers refer to 6c397f2.
Since then only `umlcraft-app` changed (settings dialog, 0b69871; toolbar buttons removed,
83e73b8): line numbers in `main_view.rs`, `main.rs` and `settings.rs` may have moved, and findings
touching settings or the language switch are marked accordingly.

## Contents

1. [Summary](#summary)
2. [Quality gates](#quality-gates)
3. [Priorities](#priorities)
4. [Cross-cutting themes](#cross-cutting-themes)
5. [Findings by area](#findings-by-area)
   - [Astah import and export](#astah-import-and-export)
   - [Model and file format](#model-and-file-format)
   - [Notation: scene, drawing, SVG](#notation-scene-drawing-svg)
   - [Notation: editing](#notation-editing)
   - [Source import](#source-import)
   - [Application](#application)
   - [CI, release, packaging and repository](#ci-release-packaging-and-repository)
   - [Documentation](#documentation)
6. [Test coverage](#test-coverage)
7. [Structure and maintainability](#structure-and-maintainability)
8. [Strengths](#strengths)
9. [Suggested order of work](#suggested-order-of-work)
10. [Mapping of original IDs](#mapping-of-original-ids)

## Summary

Both reviews reach the same verdict: UmlCraft is a carefully built codebase. The architecture in
`docs/architecture.md` describes the code faithfully, every model change goes through
`Model::apply`/`Document::edit` with exact inverses, the notation crate has no UI dependency, one
geometry source feeds canvas, hit testing, inline editing and SVG, `unsafe` is forbidden, fmt and
clippy are clean on three platforms, both locales have the same 428 keys, and comments trace
constants to Astah measurements.

The problems cluster in five places:

1. **Hostile or odd `.asta` files.** The Java serialization reader trusts the declared ZIP size
   and the class-description chain; the mapper follows ownership and other chains without a cycle
   guard, on the UI thread. A file of a few dozen bytes aborts the process or hangs the UI.
2. **States the model accepts but cannot round-trip.** Non-finite coordinates and deeply nested
   types save fine and make the project unloadable; strong-reference and edge-attachment cycles
   make elements undeletable.
3. **Losing or corrupting work in the app.** An imported Astah project counts as saved and is
   discarded without a prompt; the properties panel can lose an edit depending on focus order,
   writes back fields nobody touched (turning type references into text), and pending text is
   not flushed before New/Open/Save/Close; dragging a container picks up nodes it passes over.
4. **Multi-element and sequence edits.** Paste fails at random (hash order of strong references)
   and produces wrong presentations on sequence diagrams; `tidy` rewrites the bundled sample on
   the first edit; a create message's bar is added and dropped in the same edit.
5. **Source re-import and scale.** One name declared with two kinds aborts the whole import;
   hand-drawn associations are deleted on re-import; linear lookups in loops make large flat
   namespaces and large diagrams quadratic or worse, and the canvas rebuilds and re-measures the
   whole scene every frame.

None of this needs an architectural change. The biggest gap in the safety net: about 21 tests
that exercise the `.asta` import and export need Astah or local reference files and silently pass
on CI.

Severity:

- **High:** crash, hang or silent data loss through ordinary use or by opening a file; or a core
  feature that fails for common input.
- **Medium:** wrong result, lost information or noticeable slowness in realistic use.
- **Low:** edge cases, polish, maintainability, docs.

Confidence: **confirmed** (reproduced with a scratch program or test by at least one reviewer),
**traced** (followed through the code), **plausible** (depends on behavior neither reviewer could
run, such as the GPU-backed UI).

## Quality gates

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `cargo test --workspace` | 330 passed, 1 ignored (`large_imports_are_fast`) on Windows (F); 305 passed without the app crate in a Linux container (O) |
| Tests needing Astah, `examples/` or local reference files | about 21, all report "ok" without checking anything on CI ([X-1](#x-1)) |
| `unsafe` | none (`unsafe_code = "forbid"`) |
| i18n | 428 keys in each locale, identical sets and placeholders, none missing or unused |
| `TODO`/`FIXME`/`#[allow]` | none worth mentioning |

`unwrap`/`expect` outside tests per crate (F): model 9, format 1, notation 14, asta 2,
asta-export 1, reverse 10, app 3. Only two are reachable from user data: `properties.rs:849`
([U-6](#u-6)) and the thread join in `umlcraft-asta/src/lib.rs:89` ([A-9](#a-9)).

## Priorities

| # | What | Findings |
| --- | --- | --- |
| 1 | Guard the `.asta` reader and mapper against cycles, deep nesting and huge declared sizes; import on a background thread | [A-1](#a-1)–[A-5](#a-5), [A-9](#a-9) |
| 2 | Stop losing work in the app: mark imports dirty, per-field edit target, commit only changed fields, flush before New/Open/Save/Close, fix the binding panic | [U-1](#u-1)–[U-6](#u-6) |
| 3 | Reject non-finite geometry and over-deep types in the model; add a golden `.ucraft` fixture | [M-1](#m-1), [E-6](#e-6) |
| 4 | Container drag computes its moved set once, from the unmoved scene | [U-7](#u-7) |
| 5 | Sequence diagrams: decide the fragment growth rule against the sample, create-message bars, paste that is deterministic and sequence-aware | [E-1](#e-1)–[E-4](#e-4) |
| 6 | Source import: one name with two kinds, mark import-made associations, alias cycles, qualifier-only imports, folder exclusions, iterative Java walk | [R-1](#r-1)–[R-7](#r-7) |
| 7 | Do not apply a background import to a document opened meanwhile | [U-8](#u-8) |
| 8 | Make Astah-dependent tests visibly skipped; synthetic `.asta` fixtures and byte-level jser tests; fuzz targets | [X-1](#x-1) |
| 9 | Indexes instead of linear lookups; scene and measure cache | [M-3](#m-3)–[M-5](#m-5), [N-1](#n-1), [N-2](#n-2), [R-9](#r-9) |
| 10 | Durable save (fsync, temp cleanup, retry on Windows sharing violations) | [M-6](#m-6) |

## Cross-cutting themes

### 1. The model accepts states it cannot round-trip or undo

| Gap | Consequence | Finding |
| --- | --- | --- |
| Non-finite `f32` in bounds, waypoints, font sizes | Saved as `null`, loading fails | [M-1](#m-1) |
| Unbounded `TypeRef` nesting | Saved fine, loading hits serde_json's recursion limit of 128 | [M-1](#m-1), [E-6](#e-6) |
| Edges attached to each other in a cycle | Node, element or diagram cannot be deleted | [M-2](#m-2) |
| Strong-reference cycles (`client = self`) | Element and owner cannot be deleted | [M-7](#m-7) |
| `binding` on non-class classifiers, message ends, presentation ids | Never validated or unlinked | [M-8](#m-8) |

Checking these in `apply` and `check` closes the gap for every source at once: canvas, `.asta`
import, source import and hand-edited files. A cheap extra net: let `umlcraft_format::save` parse
its own output before renaming it over the old file.

### 2. A crash or a frozen UI loses unsaved work

There is no autosave or recovery file. Abort paths found: stack overflow on `.asta` cycles
([A-1](#a-1), [A-2](#a-2)), on alias cycles ([R-4](#r-4)), on long Java expressions
([R-6](#r-6)) and on deeply nested type text ([E-6](#e-6)); aborting allocations from a declared
ZIP size ([A-3](#a-3)) or a self-referencing superclass ([A-2](#a-2)); `.asta` import and
`.ucraft` loading on the UI thread ([A-1](#a-1), [U-17](#u-17)); an export without timeout
([A-6](#a-6)). Besides the individual fixes, a periodic recovery file written while dirty would
bound the damage of whatever is not found yet.

### 3. Text edits are committed by focus events

The properties panel and the inline editor commit on Enter or blur, so the outcome depends on
GPUI's event order ([U-2](#u-2)), on whether a shortcut moves focus ([U-4](#u-4)) and on whether a
field was touched at all ([U-3](#u-3), [U-5](#u-5)). Together with member text that does not
round-trip for imported constructs ([E-7](#e-7), [E-8](#e-8)), simply clicking through an imported
model can change it. One rule fixes most of this: commit only when the text differs from what was
loaded, and keep the target element per field.

### 4. Linear lookups inside loops

Several paths search `IndexMap`s and `Vec`s linearly per item: `Scene::node`/`edge`, `View::get`,
`child_type`/`child_package` in the source import, the generalization parent map rebuilt per
insert, `unlink_all` per doomed element. Each is fine for a hand-drawn diagram and quadratic or
worse for an imported project. The documented "6,300 files in about ten seconds" holds for Java's
deep package trees, not for flat namespaces (C/C++ globals, generated SDKs).

### 5. Tests that pass without testing

The `.asta` layer (about 3,600 lines in `jser.rs` and `map.rs`) and the export are covered only by
tests that need Astah; on CI they return early and count as passed ([X-1](#x-1)). There is also no
golden file pinning the `.ucraft` format ([M-1](#m-1)).

## Findings by area

### Astah import and export

`umlcraft-asta`, `umlcraft-asta-export`, `tools/asta-export`. The primitives of the Java
serialization reader are careful (checked arithmetic in `take`, array preallocation capped at
65,536, range- and type-checked handles, `MAX_DEPTH` for values, a 512 MB stack thread,
`TC_RESET`, `TC_LONGSTRING`, proxies and old-protocol externalizables handled), and the writer
reproduces Astah files byte for byte. What is missing is protection at the structure level, in
`jser` and in the `map` walks. `.asta` files come from other people, so these are the most
important findings.

<a id="a-1"></a>
#### A-1 · High · Cycles in the Astah object graph crash or hang the import, on the UI thread

Sources: F M10 (Medium), O A-1 (High), O U-15. Confirmed.

`map.rs:73-82` (`Reader::text`), `:457-568` (`walk`), `:1176`/`:1229` (`vertices`),
`:1241`/`:1248` (`partition`), `:1342-1347` (fragment stack in `interaction`);
`umlcraft-app/src/main_view.rs:333`.

- **Defect:** the mapper records no visited objects, `map::import` does not run inside the
  big-stack `deep()` wrapper, and the app calls `umlcraft_asta::import` synchronously in the UI
  handler.
- **Scenarios:** a package whose `ownedElementOwnership` lists itself, or a partition whose
  `subGroup` contains itself → stack overflow, abort. A name object whose `body` is itself, or a
  combined fragment whose operand lists the fragment → still spinning after 5 s. `walk` also
  creates an element per pass.
- **Fix:** a `seen: HashSet<ObjId>` in `Importer`, checked before recursing; a visited set for the
  fragment stack; an iteration cap in `text()`; run `map::import` inside `deep()`; do the whole
  import on the background executor, as the source import does.

<a id="a-2"></a>
#### A-2 · High · The class-description chain is trusted

Sources: F H1, O A-2. Confirmed.

`jser.rs:757-769`, `:779`, `:792-824`; same loop in `jser/write.rs:155-158`, same recursion in
`Graph::copy_class` (`jser.rs:371`).

- **Self reference:** `class_desc` registers the handle before reading the superclass, so a
  `TC_REFERENCE` to the description itself is accepted. `instance()` walks `super_class` in a
  `while let` without a cycle check and grows `hierarchy` until the allocation fails and the
  process aborts. A crafted `EntityStore` of about 40 bytes is enough.
- **Unbounded nesting:** `class_desc` → `class_desc` recursion does not count toward `MAX_DEPTH`
  (the counter lives only in `value()`, so `TooDeep` never fires); 3 million nested descriptions
  in a 279 KB file overflow even the 512 MB stack.
- **Quadratic amplification:** every instance gets one `ClassData` per ancestor with a cloned
  name; a 542-byte file with a 3,000-long chain and 3,000 instances takes 920 MB and 3.8 s.
- **Fix:** reject cycles while reading `super_class` (visited set in all hierarchy walks); cap the
  chain length (for example 64); count description nesting in the depth; optionally a global
  budget for objects and field slots.

<a id="a-3"></a>
#### A-3 · High · The declared ZIP entry size is trusted

Sources: F H2, O A-3. Confirmed.

`lib.rs:46`

- **Defect:** `Vec::with_capacity(entry.size() as usize)` uses the size written in the file;
  nothing limits the decompressed size either, so a plain zip bomb works too.
- **Scenario:** `u64::MAX - 16` panics with "capacity overflow" on the UI thread; 1 TiB aborts
  with "memory allocation … failed" before a byte is inflated (allocation failure is not a
  catchable panic).
- **Fix:** reject entries above a sane limit (1–2 GiB), preallocate `min(size, cap)`, read
  through `.take(cap + 1)`.

<a id="a-4"></a>
#### A-4 · Medium · Exponential expansion of bound types

Source: O A-4. Confirmed.

`map.rs:755-779`

- The depth limit of 8 does not bound the work: k substitutions naming the same anonymous bound
  class cost k⁸. A 413-byte file takes 1.9 s at k = 5, 28 s at k = 7; k ≥ 10 effectively hangs.
- **Fix:** a visited set along the expansion path (a self reference becomes text) and a budget on
  the total number of nodes.

<a id="a-5"></a>
#### A-5 · Medium · NaN and infinity reach the model from `.asta` files

Source: O A-5. Confirmed. See [M-1](#m-1).

`map.rs:88-90` (`num`), `:103-105`, `:1523-1527`, `:2183`; font sizes in `fill()` via
`parse::<f32>()`, which accepts "inf".

- A `TextPresentation` with `x = NaN` imports as `Rect { x: NaN, … }`; the project saves as `null`
  and cannot be reopened; the same JSON goes to Java in the export.
- **Fix:** filter `is_finite()` in `Reader::num` and style parsing, plus the model check of
  [M-1](#m-1).

<a id="a-6"></a>
#### A-6 · Medium · The export's Java process has no timeout or cancel

Sources: F M12, O A-7. Traced.

`umlcraft-asta-export/src/lib.rs:148-150`

- `command.output()` blocks forever if Astah shows a modal, stalls in AWT or chokes on a huge
  model at `-Xmx1024m`. `busy` never clears, further exports and source imports are refused until
  restart, the temp folder is never cleaned, and license detection only works after exit.
- **Fix:** `spawn`, drain both pipes on threads, wait with a deadline and kill the child; show
  progress with a cancel button.

<a id="a-7"></a>
#### A-7 · Medium · Predictable, reusable export temp folder

Source: O A-6. Traced.

`umlcraft-asta-export/src/lib.rs:206-218`

- `temp_dir()/umlcraft-asta-export-{pid}-{n}` is created with `create_dir_all`, which succeeds if
  it (or a symlink to one) already exists. On a shared Linux `/tmp` another user can pre-create it
  and swap `AstaExport.java`, which Astah's Java then compiles and runs as the user. Per-user temp
  folders on Windows and macOS mitigate it, and the export is effectively Windows-only today.
- **Fix:** random name, `fs::create_dir` (fail if present), mode 0700; or the `tempfile` crate.

<a id="a-8"></a>
#### A-8 · Medium · Losses are miscounted or not counted at all

Source: F M11. Traced.

`map.rs:413-436`, `:1531-1535`, `:1869-1871`, `:2078`, `:2169`; `AstaExport.java:1462-1469`,
`:1500-1502`

- `Model::apply` errors are counted under the Astah class name, indistinguishable from
  "unsupported"; presentations whose insert fails or whose edge ends did not resolve are dropped
  with `.ok()?` and never counted. `hiddenDependency=true` is skipped on purpose but counted as
  unsupported (`map.rs:991-1012`).
- The Java tool has the same gap: `edge()` returns silently when a presentation is missing from
  `shown` or matches no branch, so `Report::is_complete` lies.
- **Fix:** separate counters for unsupported, failed and dropped presentations, and report them.

<a id="a-9"></a>
#### A-9 · Low · Reader details

Sources: F (crate section), O A-10.

- `join().expect("stream thread panicked")` (`lib.rs:89`) turns any parser panic into an abort;
  map `Err` to an `AstaError`. (F)
- `WrongHandle(0, …)` always reports handle 0 (`jser.rs:697`, `:789`, `:798`); the array element
  type taken from byte 1 of the name silently becomes `'L'` for a non-array name (`:803`,
  `write.rs:171`). (F, O)
- `jser.rs:769` clones the full `ClassDesc` for every class of every instance and `:779` copies
  each field name into every instance: tens of millions of `String`s on large projects. Borrow by
  index, store values by position. (O)
- `View::get` is linear inside `edge()`, `lane_of` and `boundary_text`: O(P²) per diagram. (O)

<a id="a-10"></a>
#### A-10 · Low · Steps outside `attempt` abort the whole Java export

Sources: O A-8, F (Java bullets). Traced.

`tools/asta-export/AstaExport.java:196-201`, `:249-256`, around `:886-913`

- The interaction-use loop (`setSequenceDiagram`) can fail the whole export. The second
  transaction (`setLocation` of substates) runs after the first committed; a failure saves
  nothing. `note()`, `style()`, `setLocation` and `resize` are not wrapped; one bad node drops the
  whole diagram, which stays half-built while the report counts it as dropped.
- `attempt(owner, placeLanes)` reports lane failures as "dropped activity" (`:993`);
  `shownComments.add` runs before `createNote` (`:1421`, `:1432`).
- **Fix:** wrap each item, as the tool's README says.

<a id="a-11"></a>
#### A-11 · Low · Export robustness details

Sources: O A-9, F (export bullets).

- Problem lines embed raw element names (`AstaExport.java:2210-2211`, `:2299`); a name with `\n`
  or `\t` splits the line, and `lib.rs:157` can then report a successful export as
  `ExportError::Astah`. Normalize whitespace as `describe()` does. (O)
- `dropped` lines overwrite rather than add (`lib.rs:158-161`); report parsing is inlined
  (`:152-165`) and untestable without Java. (F)
- Absolute temp paths are passed although the child runs in the temp folder; the Windows `java`
  launcher decodes arguments in the ANSI code page, so a profile path outside it is mangled
  (plausible). Pass bare file names. (O)
- `fs::copy(out.asta, target)` (`lib.rs:180`) overwrites the target in place; a failure midway
  truncates the user's existing file. Copy to a sibling temp file and rename. (O)
- The `Command::new("java")` fallback (`lib.rs:97`) is undocumented and its error does not say the
  bundled JRE was missing. `Astah::find` knows only Windows paths; elsewhere only `ASTAH_HOME`
  helps, and an invalid `ASTAH_HOME` does not fall back. (F, O)
- `Json.string()` has no bounds check after a backslash (`AstaExport.java:2458`, `:2466`). (F)

<a id="a-12"></a>
#### A-12 · Low · Small inconsistencies and duplication

Sources: F, O A-10.

- Comment says 40, constant `SELF_LOOP` is 49.9 (`map.rs:1759` vs `:40`). Font size 12.0 is a
  literal in `map.rs:1786` and `layout.rs:330` although `scene::FONT_SIZE` exists. The
  `is_default_node_name` doc sits on the wrong function (`:257-263`).
- The `field(...).and_then(as_bool) == Some(..)` flag read appears about ten times; the label
  position formula three times (`:1769-1791`, `:2216-2223`, `:2230-2237`); Java fill-colour
  normalisation three times (`:1333`, `:1391`, `:1483`); measured numbers `-2.45`, `25.09375`,
  `±120`, `y + 7` unnamed in the Java tool.
- `jser/write.rs`, `write_entity_store` and `write_asta` serve only the `asta-rewrite` example and
  `tests/rewrite.rs`; document them as tooling (or put them behind a feature). They are the
  natural basis for synthetic fixtures ([X-1](#x-1)).
- `examples/asta-dump.rs:68-76` computes `annotation` only to discard it.
- `map.rs` is 2,341 lines in one `impl`; see [Structure](#structure-and-maintainability).

### Model and file format

`umlcraft-model`, `umlcraft-format`. Eight primitive ops, every mutation through `Model::apply`
returning the inverse, `Model::check` as an independent load-time oracle. All inverses round-trip,
`is_dirty` is right across save/undo/redo/clear, and the `RefRole`/`visit_references`/`unlink`/
`for_each_reference_mut` quartet stays in sync. The presentation side is weaker.

<a id="m-1"></a>
#### M-1 · Medium · Saved projects that cannot be opened again

Sources: F M2, O M-1, O N-8. Confirmed.

`umlcraft-format/src/lib.rs:73`, `umlcraft-model/src/model.rs:356`, `:407`, `geometry.rs:5-43`

- **Defect:** nothing rejects non-finite floats or deeply nested `TypeRef`s. serde_json writes NaN
  and infinity as `null` and refuses `null` for `f32`; it also stops at 128 levels of nesting.
- **Scenarios:** `set_bounds` with NaN saves and fails to reopen; `"x": 1e39` loads as infinity
  and the next save is unloadable; an `.asta` with NaN coordinates ([A-5](#a-5)); an attribute
  typed `A<A<…>>` 45 levels deep ([E-6](#e-6)). Non-finite values also make items invisible and
  unclickable and write `viewBox="… inf"` and `NaN` attributes into SVG; `style.font_size` is not
  bounded either.
- **Fix:** reject non-finite geometry and font sizes in `apply` and `check` (or deserialize
  through a finite-f32 type); cap `TypeRef` depth (for example 32) in `check_references`; skip
  non-finite rects in `svg::bounds`; let `save` verify its output before the rename.
- **Test gap:** no golden file. `sample_roundtrips_byte_for_byte` checks only self-consistency, so
  renaming any serde field passes every test while breaking every user file. A committed v2
  `.ucraft` of the sample and a small v1 file with a sequence diagram are the most valuable tests
  to add. (F)

<a id="m-2"></a>
#### M-2 · Medium · Edge-attachment cycles block removal

Sources: F M1 (Medium), O M-7 (Low). Confirmed.

`model.rs:356-376`, `:407-416`; `document.rs:431-457`

- Via `Tx::update_presentation` or a loaded file, edge E1 ends on E2 and E2 on E1. Insert and
  `check` accept it; removing the node, deleting its element or the diagram then fails with
  "edges attached in a cycle".
- **Fix:** check the `endpoints()` chain transitively in `check_presentation` and `check`.

<a id="m-3"></a>
#### M-3 · Medium · Every generalization insert or update scans the whole model

Sources: O M-2, F M17. Confirmed.

`model.rs:438-466`

- `check_references` rebuilds `generalization_parents` over all elements per generalization op;
  redo and undo take the same path. 3,000 generalizations into a 200,000-element model take
  13.6 s; likely the dominant cost of large source imports.
- **Fix:** an incremental `specific → [generalization]` index in `Model`; at least iterate only
  generalizations.

<a id="m-4"></a>
#### M-4 · Medium · `delete_many` unlinking is quadratic

Sources: O M-3, F M17 and model lows. Confirmed.

`document.rs:235-244`, `:252`, `:306`, `:318-328`

- Per surviving referrer, `unlink_all` clones every doomed name and calls `unlink` once per doomed
  element, each scanning the model; `references()` allocates a `Vec` per element although
  `visit_references` exists. A package of 4,000 referenced classes takes 1.68 s to delete; one
  delete in a 200,000-element model 55 ms, and `merge.rs:443` calls `delete_many` once per type
  with stale supertypes.
- **Fix:** build the `id → name` map once; per survivor unlink only the targets it references.

<a id="m-5"></a>
#### M-5 · Medium · Removing many presentations is roughly cubic

Sources: O M-4, F (model section). Confirmed.

`document.rs:446-455`; `diagram.rs:477-483`

- Each removal rescans `gone` from the start, and each probe runs `edges_attached_to` over the
  whole view (`View` is a linear `Vec`, so building a diagram is O(P²) as well). Deleting a
  diagram with 1,000 nodes and 2,000 edges takes 2.75 s; select all → remove from diagram the same.
- **Fix:** count attachments per presentation once, remove in topological order with a queue;
  index `View` by id.

<a id="m-6"></a>
#### M-6 · Medium · `save` is atomic against a process crash only

Source: O M-5. Traced.

`umlcraft-format/src/lib.rs:129-138`

- `fs::write` then `fs::rename`, without `sync_all` on file or directory, no cleanup, no retry.
  Power loss shortly after the rename can leave an empty file (delayed allocation); on Windows a
  scanner, indexer or sync client holding the target makes the rename fail and leaves a stray
  `.ucraft.tmp`; replacing drops permissions and replaces a symlink by a plain file.
- **Fix:** `File::create` + `write_all` + `sync_all`, rename, fsync the directory on Unix (or
  `ReplaceFileW`/`MOVEFILE_WRITE_THROUGH`); retry briefly on sharing violations; remove the temp
  file on error.

<a id="m-7"></a>
#### M-7 · Low · Strong-reference cycles make elements undeletable

Source: O M-6. Confirmed.

`element.rs:977`, `document.rs:262-275`

- `Client`/`Supplier` accept any kind except Model and Diagram, so a dependency can name itself;
  deleting it or its package fails with "reference cycle".
- **Fix:** reject strong cycles in `check_references` and `check`, or restrict `Client`/`Supplier`
  to non-relationship kinds.

<a id="m-8"></a>
#### M-8 · Low · Gaps in `check` and `Tx`

Sources: F M20 (Medium), O M-8. Confirmed for the binding.

- `ClassifierProps::binding` is visited, remapped and unlinked only for `Class`
  (`element.rs:262`, `:1232`, `:1299`, `:1396`): a binding on an interface naming a missing id
  passes `apply` and `check`, is never unlinked and is missed by clipboard `remap`.
- `Shape::Message` ends are not required to be lifelines, unlike `Activation`
  (`model.rs:363-374`).
- `Tx::update_presentation` lets the closure change `presentation.id` and overwrite another
  presentation (`document.rs:355-369`).
- `Document::edit` leaves applied ops without undo entry if the closure panics
  (`document.rs:70-97`).

<a id="m-9"></a>
#### M-9 · Low · Versioning is bypassed

Sources: O M-9, F (format lows). Confirmed.

`umlcraft-format/src/lib.rs:26`, `:33-34`, `:47`, `:97`, `:110-127`

- `rename_language_tags` runs on every load outside `MIGRATIONS`, so a user's own tag
  `jude.java.mine` in a current file becomes `java.mine` and v2 files are not byte-stable. The
  v1 → v2 migration is a no-op; `from_str` drops the file's version, so the app runs
  `upgrade_document` on every open (`main_view.rs:322`).
- Nothing ties `MIGRATIONS.len()` to `FORMAT_VERSION`; `FormatError::Migration` is never
  constructed.
- **Fix:** make the rename a v3 migration, return the original version, and add
  `const _: () = assert!(MIGRATIONS.len() == …)`.

<a id="m-10"></a>
#### M-10 · Low · Robustness and small details

Sources: O M-10, F (model lows).

- A file starting with a UTF-8 BOM fails with "expected value at line 1 column 1"
  (`format/lib.rs:141`). Strip a leading `\u{feff}`. Confirmed. (O)
- `from_records` rejects a diagram whose `views` entry is missing instead of defaulting to empty,
  and the error names no id (`model.rs:516-524`, `:603-607`); a Git merge can produce this. (F)
- `remove_unreferenced` uses `swap_remove`, so the order of `elements()` changes after an undo or
  a failed edit; slot order of generals (`object.rs:20`) can change after an unrelated undo.
  Document the order as unspecified or use `shift_remove` (`model.rs:295`). (O)
- `Model::is_empty` always returns `false` (`model.rs:120-123`); `View::content_bounds` and
  `refers_strongly_to` are unused; `diagram.rs:29` mentions note anchors that do not exist;
  `op.rs:37` forgets activations; `document.rs:349, 361, 416, 432` return `NotADiagram` for an id
  that does not exist at all; `sample.rs:1-2` calls the sample the start document. (F, O)

### Notation: scene, drawing, SVG

Measured, careful work: nearly every constant traces to an Astah measurement, every division is
guarded (no reachable panic in 11,000 lines), route fitting keeps right angles, `fitted_box` is
idempotent.

<a id="n-1"></a>
#### N-1 · Medium · Scene rebuilt and every text re-measured on every frame

Sources: F M16, O U-7. Traced; cost on large imported diagrams not measured.

`scene.rs:657-676`, `:699-836`, `:1885-1912`; `umlcraft-app/src/canvas.rs:1580-1587`,
`workspace.rs:39-49`, `:176-199`; `main_view.rs:141-153`

- Every render runs the full `scene::build` with text shaping: every mouse move during a drag,
  every pan and zoom step, every caret blink. Two to three shaping calls per row; `wrapped`
  measures each growing candidate (O(words²)); `object::slots` scans all elements per object.
- Every edit, even a rename, runs `fitted_boxes` twice over all auto-sized boxes of all views and
  matches them with an O(n²) `before.iter().any` plus linear `View::get`. The model tree is
  rebuilt completely on every revision.
- **Fix:** a measure cache keyed by (text, size, bold) first; cache the scene by (revision,
  diagram, preview, locale, column setting) and keep it on pan and zoom; fit only presentations
  the transaction touched, with `before` indexed by id.

<a id="n-2"></a>
#### N-2 · Medium · `with_contents` is cubic and runs every frame of a drag

Source: O N-2. Confirmed.

`scene.rs:1728-1755`

- `scene.node(result[i])` is a linear lookup and `result.contains` a linear scan, in a loop over
  all nodes per container: 358 ms per call at 2,000 nodes with 200 packages, on every frame.
- **Fix:** a `HashSet` for `result` and an id → index map in `Scene` (`Scene::node`/`edge` are
  called in loops elsewhere too, for example `asta-export/src/layout.rs:48`). With [U-7](#u-7) it
  runs only once per drag.

<a id="n-3"></a>
#### N-3 · Medium · Lollipop and actor names are painted and editable but not hit-testable

Source: F M15. Traced.

`scene.rs:86-107`, `:1008-1018`, `:1036-1053`

- Their name rows lie outside `rect`; `hit` falls back to `rect.contains`, so a click selects
  nothing while a double click at the same spot starts editing. `extent()` ignores the rows too,
  so zoom-to-fit clips actor names.

<a id="n-4"></a>
#### N-4 · Medium · Rows and labels can contain newlines and control characters

Source: O N-3. Confirmed for the data, traced for the GPUI assert.

`scene.rs:611` (`Line::new`), `:1840-1868`

- Only notes, free texts and use case names are split into lines. The Java import stores raw
  initializer source (`reverse/src/java/mod.rs:358`), so `int[] XS = {\n 1,\n 2 }` reaches a row
  as is. GPUI's `shape_line` has a `debug_assert!` against `\n`: debug builds panic, release
  builds misdraw. Free texts keep the `\r` of CRLF text.
- **Fix:** normalize in `Line::new` and for edge labels (`\r\n`, `\n`, `\t` → space, drop other C0
  controls); collapse whitespace in imported default values.

<a id="n-5"></a>
#### N-5 · Medium · SVG export writes characters XML forbids

Sources: O N-4 (Medium), F (Low). Confirmed.

`svg.rs:367-379` (`escape`)

- Only `& < > "` are escaped; U+0000–U+0008, U+000B, U+000C, U+000E–U+001F, U+FFFE, U+FFFF are
  illegal in XML 1.0 even as references. A class named `"A\u{1}B"` or a note with a form feed
  (both possible from `.asta`) makes the file not well-formed. Markup injection is not possible.
- **Fix:** drop or replace with U+FFFD in `escape`.

<a id="n-6"></a>
#### N-6 · Medium · Align uses stored bounds, the user sees drawn bounds

Source: O N-5. Confirmed.

`umlcraft-app/src/workspace.rs:751-761` → `scene.rs:2596`

- A class stored 40 × 30 but drawn wider for its long name ends up with its drawn right edge at
  425.5 after Align Right to 200.
- **Fix:** align the drawn rects and apply the offset to the stored bounds.

<a id="n-7"></a>
#### N-7 · Low · Bounds and SVG clipping

Sources: F (notation lows), O N-9.

- `Scene::bounds` covers nodes only: a diagram of free lines has no bounds and zoom-to-fit does
  nothing; self-loops are clipped (`scene.rs:65-68`). `svg::bounds` does it right for these. (F)
- `svg::bounds` ignores association name arrows and end markers: the arrow tip can lie outside
  the image (`svg.rs:103-124`). Confirmed. (O)
- SVG writes only `rrggbb`; the canvas applies alpha (`canvas.rs:1441`). Add `fill-opacity`
  (`svg.rs:208`). (O)

<a id="n-8"></a>
#### N-8 · Low · Hit testing and containers

Sources: F (notation lows).

- `hit_test` always prefers edges over nodes regardless of z-order (`scene.rs:2440-2443`).
- Two definitions of "container": `with_contents` (Boundary, Package, Partition, State, Cube,
  Component) vs `palette::is_container` (no State, no Component) (`scene.rs:1728-1755`,
  `palette.rs:729-737`). A Component created after its artifacts covers them for clicks yet
  drags them.

<a id="n-9"></a>
#### N-9 · Low · Region lines drawn outside small states

Source: O N-6, F (state lows). Confirmed.

`scene.rs:1502-1507`, `:1535-1552`; `state_edit.rs:81`, `:123-130`, `:161-178`

- A state 40 high with 5 regions gets regions at y = −40 and −20; `add_region` never grows the
  state, so "every region keeps at least 20" does not hold. Fix: grow the state in `layout_state`
  to header plus `n × MIN_REGION`. (O)
- `move_region_line` stores the offset unclamped while drawing clamps it; `add_region` with the
  state on several diagrams lets the last diagram decide. (F)

<a id="n-10"></a>
#### N-10 · Low · Slightly slanted right-angled lines cannot be dragged

Source: O N-7. Confirmed.

`route.rs:196`

- Nodes side by side whose centers differ by 1.5 in y (common in imported files): the segment
  counts as vertical, the x ranges are disjoint, and dragging does nothing. Decide orientation
  with `|dx| >= |dy|`.

<a id="n-11"></a>
#### N-11 · Low · Smaller performance costs

Sources: F, O N-10.

- `keywords_away_from_notes` is O(E²) on every build (`scene.rs:869-908`): test `lines.contains`
  before `edges.iter().find`, or index edges. Scene build goes from 1.7 ms at 1,000 edges to
  5.6 ms at 2,000.
- `edge_at` clones the path per edge per hit test (`:2446-2452`); `unique_name` is O(k·n)
  (`palette.rs:773-778`).

<a id="n-12"></a>
#### N-12 · Low · Small inconsistencies

Sources: F, O N-11.

- The doc comment "States: the name, centered…" sits on `CUBE_DEPTH` (`scene.rs:1250-1253`).
- `Paint::Translucent` is documented as the boundary fill (`draw.rs:36`); boundaries use
  `Paint::None` (`:308`).
- "Astah width = max(exact, rounded)" is written out five times; the stereotype-name-block layout
  three times (`layout_box`, `layout_object`, `layout_classifier`); the `Fixed` test measure twice.
- `model.get(id).expect("caller checked")` appears seven times; passing `&Element` removes them.
- Unnamed numbers at `scene.rs:976, 1028, 1048, 1173, 1992, 2351, 2366-2370`, `sequence.rs:812`
  (duplicates `SELF_DROP`), `:639`, `:996`, `:1007`, `sequence_edit.rs:267-268, 284, 409, 411`,
  `draw.rs:637`.

### Notation: editing

Sequence, state, clipboard, member, object, palette. Every mutation goes through `Tx`; `tidy`
converges at a fixed point; `fit_bars` and `fit_fragments` are bounded.

<a id="e-1"></a>
#### E-1 · High · `tidy` rewrites the bundled sample and moves a message into a fragment

Source: F H5. Confirmed.

`sequence_edit.rs:635-681` (`fit_fragments`), `:746-862` (`sync_order`)

- `fit_fragments` grows a fragment to `bar.bottom + 10` for every bar whose top lies inside it,
  however far the bar runs out. In `sample::order_system` the Customer bar (150..265) starts
  inside the loop (120..190), so the first `tidy` grows the loop to 275 and `sync_order` moves
  `submit` (y 220) from the interaction into the loop operand. Any tidying edit triggers it,
  including `Workspace::delete_selection`, which runs `tidy_all` after deleting anything anywhere
  (`workspace.rs:797`).
- Related (O, structure): "inside a fragment" means the sender's center in `fit_fragments` but
  both ends in `sync_order`, so a message crossing a frame edge grows the fragment without being
  put into it.
- **Fix:** decide whether the sample's geometry or the growth rule is wrong; add a test "tidy is a
  no-op on the sample" either way.

<a id="e-2"></a>
#### E-2 · High · Paste fails at random

Source: O E-1. Confirmed.

`clipboard.rs:120-126`, `:196-215`, `:255-260`

- The copied roots come from a `HashSet`, so their order is random. `insert_tree` defers weak
  references to copies not yet inserted, but not strong ones, so `InsertElement` fails with
  `InvalidReference`. Select all, copy, paste on the sample sequence diagram fails 29 of 40 times;
  copying A and B where B's generalization is owned by B (as the source import stores it) fails
  18 of 40 times.
- **Fix:** keep the selection order; postpone subtree elements whose strong references point to
  copies not yet inserted and insert them in a second pass at their remembered owner and index.

<a id="e-3"></a>
#### E-3 · Medium · Paste and "show" make wrong presentations on sequence diagrams

Sources: F M7, O E-2, F M6 (paste without tidy). Confirmed.

`clipboard.rs:98-170`, `:286-301`; `palette.rs:741-757`; `umlcraft-app/src/workspace.rs:698-706`

- `copy` takes a lifeline's head as `bounds()` and `paste` creates `Shape::Node`, exactly what
  `sequence_edit::upgrade` converts at document open but never later. The pasted lifeline is drawn
  as an empty box, is no lane, and no message can connect ("messages connect lifelines").
  Fragments lose separators and operands; messages and bars have no `bounds()` and are not copied.
  `show_element` does the same for a `CombinedFragment`. Paste runs no `tidy`.
- A successful select-all paste also duplicates the `Diagram` element (its frame is selected) and
  a fragment holding a message that is never drawn. "Paste as same elements" of a class onto a
  sequence diagram, or a duplicate onto a state machine diagram, succeeds although `can_show`
  says no.
- On the next open, `upgrade_document` takes these nodes for version 1 content, lays out the
  diagram again and clears undo.
- **Fix:** route pastes through `can_show`, `add_lifeline` and `add_fragment`, then `tidy`; never
  copy the diagram frame or bars. Or refuse the paste on sequence diagrams until then.

<a id="e-4"></a>
#### E-4 · Medium · A create message's receiver bar is added and dropped in the same edit

Sources: F M8, O E-6. Confirmed.

`sequence_edit.rs:241`, `:357-370`, `:490`, `:568-584`

- `add_message` adds a bar at `line_top`, below the head; the create message ends at the head's
  center, 15 higher, so `drop_idle_bars` sees no message touching the bar and deletes it.
  `extent` and `move_item` never find a bar for create either; architecture.md's "25 on the
  receiver" is false for create.
- **Fix:** count the create message's end as touching a bar starting at `line_top`, anchor the bar
  at the message, or drop the code.

<a id="e-5"></a>
#### E-5 · Medium · Sequence edits without `tidy`, and `covered` without effect

Source: F M6. Traced.

`umlcraft-app/src/workspace.rs:957-961`; `properties.rs:415-428`, `:1752-1764`

- `update_element`, used for the message sort Select and the covered-lifelines checkboxes, runs
  no `tidy`, although a sort change affects bar fitting. (Paste: [E-3](#e-3).)
- `tidy`/`fit_fragments` never read `covered`, so the checkboxes have no visible effect.

<a id="e-6"></a>
#### E-6 · Medium · `parse_type` has no nesting limit

Source: O E-5. Confirmed.

`member.rs:478-527` (recursion at 510)

- 1,000 levels of `List<…>` overflow a 1 MB stack (Windows main thread, debug) and abort; parsing
  is quadratic (5,000 levels take 2 s); 45 levels are accepted and saved but cannot be loaded
  ([M-1](#m-1)).
- **Fix:** cap the depth (for example 32; the rest stays text).

<a id="e-7"></a>
#### E-7 · Medium · Member text does not round-trip

Sources: O E-3, F M9, F (member lows), O E-9. Confirmed.

`member.rs`

- **Variadic template parameters** (`:84-90`, `:555-557`): `+ f<Ts...>(…)` is applied back as the
  name `Ts...`; every edit adds three dots and breaks the link. (O)
- **Unnamed parameters and operator names** (`:275-281`, `:283-312`, `:392-404`), both produced by
  the C++ import: "no valid name", `operator<` gives "missing >", `operator()` "unexpected …".
  These operations cannot be edited inline at all. `parse_attribute` rejects `arr[]` without a
  type. (O, F)
- **Ambiguous types** (`:426-432`): with two classes `Item`, renaming an attribute turns
  `Element(Item)` into `Text("Item")`. (O; root of [U-3](#u-3) and [U-5](#u-5))
- **Anonymous bound classes** (`:14`): `type_label` uses `name()`, not `display_name`, so the text
  reads `+ pair : ` and applying it clears the type. (O)
- **String literals and comparison operators in defaults** (`:218-236`, `:256-261`,
  `:415-421`): `find_top` tracks `<>()[]` depth but not quotes, so
  `f(s : String = "a,b", n : int)` yields parameters `s`, `b"`, `n` (and `check_name` accepts
  `b"`); `f(a : bool = x < y, b : int)` parses as one parameter. (F, O)
- `apply_member_text` deletes `Return` parameters that `member_text` never shows
  (`:578-589`). (F)
- **Fix:** strip `...` and set `is_variadic`; accept empty parameter names and `operator…` names;
  keep an existing link when its name matches; use `display_name` in `type_label`/`type_text`;
  skip quoted strings and only count `<` as a bracket where a type can start.

<a id="e-8"></a>
#### E-8 · Medium · Editing an operation recreates all its parameters

Source: O E-4. Confirmed.

`member.rs:578-599`

- An imported parameter with multiplicity `*` and a stereotype loses both after a visibility
  change and gets a new id, which unlinks comments annotating it.
- **Fix:** update parameters by position; create or delete only the difference.

<a id="e-9"></a>
#### E-9 · Low · Batch moves can close ownership cycles

Source: O E-8. Confirmed.

`state_edit.rs:64`, `:71-74`; `sequence_edit.rs:556-564`, `:802`, `:851-860`

- Both check "not into itself or what it holds" before any move, then apply several moves. Two
  sibling states, each with its center inside the other, dragged together → `OwnershipCycle`. On
  an inconsistent sequence diagram `tidy` fails, and since `tidy_all` tidies every sequence
  diagram, every delete anywhere then fails.
- **Fix:** recheck `is_ancestor` before each move; let `tidy_all` skip diagrams it cannot tidy.

<a id="e-10"></a>
#### E-10 · Low · Destroy and operand edge cases

Source: O E-7. Confirmed.

- `sequence_edit.rs:374`: a destroy message above the target's head gives a negative lifeline
  length; `fit_lifelines` skips destroyed lifelines. Deleting the destroy message never resets
  `destroyed`.
- Deleting an operand leaves its separator (a stray line); deleting the last operand leaves a
  fragment without operands, and `sync_order` silently skips what is inside. A `delete_operand`
  edit should remove the matching separator and guard offset.

<a id="e-11"></a>
#### E-11 · Low · Smaller editing defects

Sources: O E-9, F (notation and object lows).

- `object.rs:67-71`: `set_slot` applies `split_once('=')` unconditionally, after the properties
  panel already split once (`properties.rs:756-759`): `name = "k=v"` stores `v"`. Strip the prefix
  only when it equals the attribute name. Confirmed. (F, O)
- `palette.rs:794-796`: new vertices take their owner from the top-left click point, moved ones
  from their center; pass `origin + size / 2`. Confirmed. (O)
- `move_item` (`sequence_edit.rs:465-542`): moves nested bars but not the messages that start
  them, so a callback loses its bar after the outer call moves (O); a message moved above its
  sender's bar starts from the bare lifeline, where `add_message` would create one (F).
- `palette.rs:598-604`: `can_show` checks only the kind, so lifelines and fragments of another
  interaction can be shown. Traced. (O)
- `palette.rs:924`: `create_fragment` treats an origin of exactly (0,0) as "no origin". (F)
- `sequence.rs:689-751`: `numbers` branching detection is asymmetric for self-messages at equal
  height, and `started` silently picks the last of two calls arriving at one bar top. (F)

### Source import

`umlcraft-reverse`. Four thin tree-sitter front-ends lower to one documented IR, then one
language-neutral resolve/merge/associations pipeline driven by `Rules`; no `if language ==`
branches; results are deterministic; re-import is idempotent per language; re-export and typedef
cycles terminate; symlinked folders are not followed.

<a id="r-1"></a>
#### R-1 · High · One name declared with two kinds aborts the whole import

Sources: F H3, O R-1. Confirmed.

`merge.rs:144-179`, `:246-253`

- `declare_type` retypes the existing element when a later declaration has another kind; the
  `Entry` pushed for the earlier declaration keeps the deleted id, and `properties()` calls
  `tx.update` on it → `NotFound`, the whole import rolls back with "no element or presentation
  <id>". Reproduced with `class Foo {}` and `interface Foo {}` in two Java files, C++
  `class Node` and `enum Node` in two headers, and Python modules of the same name in non-package
  folders. Common in multi-program C++ trees, script and test folders, multi-module projects.
- **Fix:** after a retype, rewrite `entries[*].id` and the `aliases` keys; better, settle each
  (owner, name) kind before creating, or keep conflicting kinds apart and report them.

<a id="r-2"></a>
#### R-2 · High · Re-import deletes associations drawn by hand between imported classes

Sources: F H4 (High, confirmed), O R-8 (Low, traced).

`associations.rs:120-158`; `merge.rs:438-443`

- The stale pass treats every association not produced by this import, whose navigable end has a
  role and whose opposite participant is a source type, as a field that is gone. A hand-drawn
  `A → B` with a role loses its navigability and is then deleted. architecture.md and `merge.rs:8`
  promise that hand-made relationships survive.
- Hand-drawn generalizations and realizations between imported classes are always deleted too;
  the docs should say so plainly.
- **Fix:** mark associations the import creates (tagged value) or remember created ids.

<a id="r-3"></a>
#### R-3 · Medium · Imports used only as qualifiers create bogus placeholder classes

Sources: F M13, O R-2. Confirmed.

`merge.rs:212-240`

- `import torch.nn as nn` with `nn.Module` gives `torch › nn` twice, as a class and a package;
  the same for `from django.db import models` with `models.Model`, C++
  `namespace fs = std::filesystem;` with `fs::path`, and C# `using Gen = System.Collections.Generic;`
  with `Gen.List<int>` (`path_lookup_package` consults only the model, not the library).
- **Fix:** pre-resolve a single import only when its visible name is used as a simple name (the
  qualified resolution already creates the right placeholders); consult `library.is_package`.

<a id="r-4"></a>
#### R-4 · Medium · Single-segment alias cycles overflow the stack; `retype` leaves stale supertypes

Source: F M14. Confirmed.

`resolve.rs:285-291`, `:378-394`; `merge.rs:396-407`

- `using A = B; using B = A;` aborts the process instead of reporting an error. Guard with the
  `reexporting`-style stack already used for re-exports.
- `retype` leaves `self.supertypes` pointing at the deleted id, so member lookup through a
  retyped supertype silently finds nothing for the rest of the import.

<a id="r-5"></a>
#### R-5 · Medium · Alias-typed parameters break re-import idempotence

Source: O R-3. Confirmed.

`merge.rs:626-634` vs `:823-839`

- The source signature uses the written name (`O`), the model signature the resolved name
  (`Order`). Python `from shop.orders import Order as O` or C# `using O = Shop.Order;` with a
  parameter of type `O`: every re-import deletes the operation and creates it with a new id;
  messages referring to it are unlinked.
- **Fix:** build the source signature from the resolved `TypeRef`, or map alias names to their
  target's last segment in `simple_text`.

<a id="r-6"></a>
#### R-6 · Medium · Recursion over Java method bodies can overflow the stack

Source: O R-4. Confirmed.

`java/mod.rs:431-447` (`count_nested_classes`)

- `return "s0" + "s1" + …` with about 10,000 terms overflows a 2 MB thread stack (GPUI's Linux
  workers); generated string tables and array initializers produce such expressions. tree-sitter
  is not the limit (C# parses 100,000 terms on 512 KB).
- **Fix:** walk with a `TreeCursor` or an explicit stack.

<a id="r-7"></a>
#### R-7 · Medium · Folder collection has no exclusions and fails on one unreadable folder

Sources: O R-5, F (reverse lows). Traced.

`source.rs:29-40`; `umlcraft-app/src/main_view.rs:428-441`

- Every subfolder is walked, including `.git`, `.venv`, `venv`, `site-packages`, `node_modules`,
  `bin`, `obj`, `build` and `_deps`: a venv project imports all of site-packages; a C# folder
  imports `obj/**/*.g.cs` with duplicated members from Debug and Release.
- `read_dir(dir)?` and `entry?` abort the whole collection; the app shows "Permission denied (os
  error 13)" without the path.
- `std::fs::read` has no size cap and `Parser::parse` no timeout.
- **Fix:** skip hidden folders and a list of build and vendor folders (or folders with
  `pyvenv.cfg`); report unreadable folders as file problems with their path and continue; cap file
  size.

<a id="r-8"></a>
#### R-8 · Medium · `.py` and `.pyi` of one module are merged as parts

Source: O R-6. Confirmed.

`lib.rs:70-85`, `source.rs:46`

- `shop/orders.py` and `shop/orders.pyi` both declaring `Order` give two attributes `x`, two
  `__init__`, two `total` (attrs ships such pairs). Where both exist, import only the `.pyi`.

<a id="r-9"></a>
#### R-9 · Medium · Name lookup is linear, so large flat namespaces are quadratic

Sources: O R-7, F M17. Confirmed.

`resolve.rs:58-70`, `:85-90`, `:385`, `:403-413`, `:527`; `merge.rs:204`, `:274`, `:487-503`,
`:586-597`

- `child_type`/`child_package` scan all children per name; `enclosing()` leaves out the root, so
  with `enclosing_namespaces` (C#, C++) global types are found only via `placeholder_under(root)`,
  which scans the root again. N global C++ structs with 11 references each: 0.36 s at 1,000,
  13.3 s at 8,000 (104,000 elements); about 61% in `nested_placeholder → child_type`.
- Same kind: `by_type.iter().find` per declaration (O(T²)); `by_type.iter().any` per definition;
  operation matching recomputes `signature_of_model` inside `position`; imports cloned and
  `on_demand` rebuilt per simple name; every `retype` scans the model about three times.
- **Fix:** a `HashMap<(owner, name), Id>` index for the import, updated on create and retype; a
  `HashMap` for `by_type`; model signatures once per class.

<a id="r-10"></a>
#### R-10 · Low · Smaller import defects

Sources: O R-8, F (reverse lows).

- `preprocessor.rs:197`: C's line continuation is applied to C# too, so `// C:\temp\` swallows
  the next `#endif` and everything after it is blanked silently. Make splicing C++-only.
  Confirmed. (O)
- `preprocessor.rs:16-18`: a line starting with `#if` inside a multi-line raw or verbatim string
  is blanked. (F)
- `cpp/mod.rs:835`: `operator==` and `operator ==` become two operations. Confirmed. (O)
- `cpp/mod.rs:343-367`: `template<typename>` yields a nameless `TemplateParameter`;
  `:296-303`: `typedef struct {…} Point, *PointPtr;` loses `PointPtr`. (F)
- `java/mod.rs:144-150`: `path()` splits raw text on `.`, so a comment inside a scoped name leaks.
  (F)
- `merge.rs:500`, `:612`: a definition's own `using`s are used only to find its owner, so
  `using namespace lib; void A::f(Str s) {}` links `Str` to a new placeholder `::Str`. Confirmed.
  (O)
- `merge.rs:170-176`: a new C# partial class in two files is reported as 1 created and 1 updated.
  (O)
- `lib.rs:47`: C++ extensions omit `.c`, `.inl`, `.ipp`, `.tpp`. (F)

### Application

`umlcraft-app`. The Workspace is a clean façade; no model change bypasses `Document::edit`/`Tx`;
every UI action is one labelled undo step; `validate()` keeps tabs and selection consistent after
undo; drags are previews that become one undo step; exports work on snapshots; a corrupt
`settings.json` falls back to defaults.

<a id="u-1"></a>
#### U-1 · High · An imported Astah project counts as saved; closing discards it silently

Sources: F H6 (High), O M-10 (Low). Traced.

`main_view.rs:223`, `:331-336`; `umlcraft-model/src/document.rs:33-41`

- `import` calls `replace_document(Document::new(imported.model), None)`, and `Document::new`
  counts as saved. `should_close`, `quit`, `new_project` and `open_*` gate on `is_dirty()`, and
  the status bar says "saved". After a long import, closing the window loses everything without a
  prompt. The sample project behaves the same.
- **Fix:** a "never saved" state in `Document`, or mark the document dirty after import.

<a id="u-2"></a>
#### U-2 · High · The properties panel loses an edit when focus moves to an earlier field

Source: O U-1. Traced through the app and GPUI sources; not reproduced in a running app.

`properties.rs:577-586`, `:595`

- On `Focus` the panel sets `editing = editing.or(shown)`, on `Blur` it commits and sets
  `editing = None`. GPUI calls focus listeners in registration order, and each `InputState`
  registers in its constructor, so when focus moves to a field created before the current one,
  `Focus(new)` arrives before `Blur(old)` and the new field ends up with `editing == None`.
- **Scenario:** type in Stereotypes, click into Name, type a new name, press Enter: `commit`
  returns early and the rename is lost without a message.
- **Fix:** keep the target per field (`HashMap<Field, Id>`, set on Focus, taken on Blur).

<a id="u-3"></a>
#### U-3 · Medium · Leaving an element writes back every field and re-parses types

Sources: F M3, O U-3. Traced; the parse behavior is confirmed.

`properties.rs:691-716`, `:782-796`, `:826-835`

- If any field had focus, a selection change commits all 25 text fields. Value type, bounds and
  defaults have no "unchanged" check and re-run `parse_type` on the unqualified names shown;
  constraints are re-split at commas, names trimmed; each change is its own silent undo step.
- **Scenario:** template parameter `T extends Node` with two classes `Node`; click into Name, then
  elsewhere: the bound becomes the free text "Node".
- **Fix:** commit only the field the user changed (track `InputEvent::Change`) and compare with
  the loaded text.

<a id="u-4"></a>
#### U-4 · Medium · Uncommitted text is neither saved nor asked about

Source: O U-4. Traced (close button plausible).

`main_view.rs:222-226`, `:651-707`, `:740-753`

- Fields commit only on Enter or blur; Ctrl+N, Ctrl+O, Ctrl+S and the window's close button do
  not move focus. Type a name without Enter, press Ctrl+N: the document is replaced without a
  prompt. Ctrl+O: the file dialog's blur commits after the dirty check, and the open discards it.
  Ctrl+S writes the file without the pending text.
- **Fix:** a `flush_edits()` (canvas `commit_edit` plus the focused properties field) at the start
  of `confirm_discard`, `should_close`, `save` and the exports.

<a id="u-5"></a>
#### U-5 · Medium · An inline edit is applied even when nothing changed

Source: O U-5. Confirmed.

`canvas.rs:1155-1167` → `workspace.rs:287-303`, `:600-636`

- Double-click a member row and click away without typing: an ambiguous type reference becomes
  text; an operation's parameters are recreated with new ids ([E-8](#e-8)), an undo step is added
  and the document becomes dirty; free texts imported from Astah or resized by hand are reset to
  the computed size.
- **Fix:** keep the original text in `InlineEdit` and skip unchanged commits, as the Declaration
  field already does (`properties.rs:603`).

<a id="u-6"></a>
#### U-6 · Medium · Reachable panic after clearing a binding

Source: F M4. Traced.

`properties.rs:823`, `:849`

- `selected` is checked at line 823, but the commits in between can run `set_binding(id, None)`,
  which deletes the «bind» dependency. If the new selection is that dependency,
  `expect("checked above")` panics. Re-fetch after the commits.

<a id="u-7"></a>
#### U-7 · High · Moving a container drags along nodes it passes over

Source: O U-2. Confirmed by simulating the canvas loop.

`canvas.rs:467-468` (preview), `:893-895` (commit), `:1580-1589`

- Preview and commit call `scene::with_contents(&self.scene, …)` on the previous frame's scene,
  in which the container is already moved. Dragging a package by 350 past class X makes X
  alternate between moved and unmoved every frame; on release, whether X moves depends on the
  parity of mouse-move frames. On state diagrams `reparent_vertices` then makes X a substate.
- **Fix:** compute the moved set once on mouse-down from the scene without preview, store it in
  `Drag::Move`.

<a id="u-8"></a>
#### U-8 · Medium · A background source import merges into whatever project is open when it ends

Sources: O U-6 (Medium), F (app lows). Traced.

`main_view.rs:258-311`, `:400-448`

- `busy` blocks only a second import or export; New, Open, Open Recent and Open Sample still run.
  Import a large Java folder, open project B: B receives the classes as an undo step.
- **Fix:** record a document generation at the start and discard (or ask) if it changed, or
  disable file operations while busy.

<a id="u-9"></a>
#### U-9 · Medium · Error texts and English strings outside the locale files

Sources: F M5 (Medium), O U-9 (Low), F (app lows).

- `ModelError`/`FormatError` `Display` texts with raw UUIDs ("no element or presentation 0192…")
  reach the status bar and dialogs, also in the German UI, whenever a connection is refused or a
  multiplicity is mistyped (`workspace.rs:208`, `properties.rs:776-779`, `main_view.rs:326, 348,
  437, 474, 601, 637, 703`); PNG export errors (`png.rs:20`, `:22`).
- Hard-coded dialog button `"OK"` (`main_view.rs:729`); English placeholders "entity, control",
  "filled, paid", "ordered, unique" (`properties.rs:312-336`).
- **Fix:** keys for the button and placeholders; a locale key with arguments per error variant.

<a id="u-10"></a>
#### U-10 · Low · Some edits skip the automatic refit

Sources: F, O U-8. Traced.

`workspace.rs:333-342`, `:346-362`, `:496-503`

- `connect_message`, `create_fragment` and `update_style` call `self.doc.edit` directly instead of
  `self.edit`, skipping the refit architecture.md promises. Ticking "Hide attributes" on an
  auto-sized class leaves an empty band that never refits.

<a id="u-11"></a>
#### U-11 · Low · Shortcuts and quitting per platform

Sources: F, O U-10.

`main.rs:123-155`

- Shortcuts use `ctrl-` instead of `secondary-`: on macOS they are Ctrl, not Cmd, and Ctrl+Z in a
  field undoes a model change.
- On Windows and Linux, Input binds only Ctrl+Z and Ctrl+Y, so Ctrl+Shift+Z in a field runs the
  model's Redo; `sync` then reloads every field and wipes the text being typed. A background
  import finishing while the user types does the same.
- No `on_app_quit` or reopen handler on macOS (plausible).
- `DIAGRAM_FONT = "Arial"` falls back silently on Linux without the font. (F)

<a id="u-12"></a>
#### U-12 · Low · Settings location and corrupt files

Sources: F, O U-12. `settings.rs` changed since 6c397f2 (settings dialog); the location code is
unchanged.

`settings.rs:1`, `:22-46`

- macOS uses `~/.config/UmlCraft`, not Application Support; Linux ignores `XDG_CONFIG_HOME`; the
  module doc, README and architecture.md mention only `%APPDATA%`.
- One wrongly typed field (`"dark": "yes"`) resets all settings including recent files; two
  instances overwrite each other's recent files; the write is a plain `fs::write`.
- **Fix:** per-field lenient reading, temp file and rename, platform config folder, docs.

<a id="u-13"></a>
#### U-13 · Low · Relative command-line paths and stale recent files

Sources: O U-13, F (app lows).

`main.rs:176-177`; `main_view.rs:304-311`, `:319-327`

- `umlcraft foo.ucraft` stores the relative path in recent files; opening it later from another
  working directory fails, and the file dialog's start folder becomes "". A missing recent file
  shows the error but stays in the list. Make paths absolute; drop entries that fail to open.

<a id="u-14"></a>
#### U-14 · Low · State that survives opening another document

Source: O U-11. Traced.

`main_view.rs:248-256`

- `replace_document` resets workspace and tree but not `notice` or `clipboard`. `notice` is keyed
  by revision, so an old "Imported …" message returns when the new document reaches the same
  revision; "Paste (same elements)" after opening another project fails with a raw UUID. Clear
  both.

<a id="u-15"></a>
#### U-15 · Low · Rename from the model tree can focus the wrong field

Source: O U-14. Plausible.

`main_view.rs:205-211`, `properties.rs:564-571`

- `focus_name` decides between Name and Declaration from the element shown before the action.
  Rename on an attribute while a class is shown focuses Name, which members do not render. Decide
  from `ws.selected_element`.

<a id="u-16"></a>
#### U-16 · Low · Changing the language leaves Select items in the old language

Source: F. Reported against `toggle_language` (`main_view.rs:817-826`); since 0b69871 the
language changes through `set_language`, which notifies the properties panel but does not call
`PropertiesPanel::sync`. Not re-checked.

- Select items in the properties panel keep the old language until the next workspace change.

<a id="u-17"></a>
#### U-17 · Low · Loading and `.asta` import block the UI

Source: O U-15. See [A-1](#a-1).

`main_view.rs:319`, `:333`

- Loading `.ucraft` files and importing `.asta` files run synchronously in the UI handler; a large
  file freezes the window, and any crash there takes the app down. Run both on the background
  executor.

<a id="u-18"></a>
#### U-18 · Low · Small details

Source: F.

- `Document::undo_label` exists but the Edit menu shows plain "Undo".
- `install_menus` builds the menu tree twice (`main_view.rs:1294`).
- `Transform::to_logical` duplicates `DiagramCanvas::to_logical`; `4.0 / zoom` appears six times
  next to the named `HANDLE_GRAB`.

### CI, release, packaging and repository

<a id="x-1"></a>
#### X-1 · Medium · About 21 tests pass on CI without testing anything

Sources: F M18, O X-1. Confirmed.

`umlcraft-asta/tests/import.rs:11-21`, `tests/rewrite.rs:7`;
`umlcraft-asta-export/tests/roundtrip.rs:611-623`; `umlcraft-reverse/tests/astah_compare.rs:283`

- The 11 import tests and the rewrite test look for Astah's samples under the fixed path
  `C:\Program Files\astah-professional` (ignoring `ASTAH_HOME`, which the export crate honours);
  the 8 round-trip tests need an installation; `astah_compare` needs local reference files. Without
  them they print "skipped" and return `Ok`. On all three CI platforms `map.rs` and `Export::run`
  are covered only by 5 + 3 unit tests.
- **Fix:**
  - Mark them `#[ignore = "needs Astah"]` and run them with `--ignored` where Astah exists; or an
    env var that turns a skip into a failure. Use `ASTAH_HOME` in the import tests.
  - Synthetic fixtures: `jser::Graph` plus `write_entity_store`/`write_asta` build `.asta` files in
    a test without Astah content in the repository. Byte-level jser tests for `TC_LONGSTRING`,
    `TC_RESET`, `TC_ENUM`, `TC_CLASS`, proxies, primitive arrays, externalizables and every error
    path (`Eof`, `BadHandle`, `WrongHandle`, `BadTypeCode`, `TooDeep`) are a few dozen bytes each;
    then one mapping test per element kind and the hostile cases of [A-1](#a-1)–[A-5](#a-5).
  - A fuzz target for `parse_entity_store` + `map::import` and for `umlcraft_format::from_str`.
  - Move the export's report parsing into a pure function and unit-test it.

<a id="x-2"></a>
#### X-2 · Low · Toolchain and dependency hygiene

Sources: F, O X-2.

- Toolchain is floating `stable`, edition 2024, no `rust-version`, no `rust-toolchain.toml`
  (`ci.yml:25`, `Cargo.toml:7`): a new clippy lint can turn CI red without a change; the code needs
  at least 1.88 (let chains). Declare `rust-version`; consider pinning the release toolchain.
- Third-party actions pinned to major tags, not SHAs, while `publish` has `contents: write`. Pin
  to SHAs with Dependabot updating them.
- No advisory or license check in CI (`cargo about` runs only on release); add `cargo deny`.
- The release workflow builds from the tag without running the tests; add a `needs:` on a test
  job or check the commit is on `main` with green CI.
- `resvg = "0.46"` in the workspace while gpui-component pulls 0.45.1: resvg, usvg, tiny-skia,
  png, roxmltree, kurbo and svgtypes are duplicated; `docs/icons.md` claims the same resvg as GPUI.
- No `[profile.release]` (`lto = "thin"`, `codegen-units = 1`, `strip = true`); optional.

<a id="x-3"></a>
#### X-3 · Low · Packaging

Sources: F, O X-3.

- `packaging/macos/Info.plist` declares no `CFBundleDocumentTypes` or UTI for `.ucraft`/`.asta`;
  the Windows release is a plain `.zip` without file association; on Linux the desktop and MIME
  files cover `.ucraft` only, not `.asta`.
- `metainfo.xml` `<releases>` holds only 0.8.0; 0.7.0 was replaced, not kept (`:35-39`).
- `assets/icons/fluent/LICENSE` is not in the release copy lists; not an obligation yet since no
  crate embeds the icons.

<a id="x-4"></a>
#### X-4 · Low · Repository files and platform tests

Sources: F, O X-4.

- No root `.gitattributes` (`* text=auto eol=lf`), `.editorconfig`, `deny.toml`, SECURITY.md,
  CONTRIBUTING.md or dependabot. The two nested `.gitattributes` for encoding fixtures are right.
- `main_view.rs`'s `recent_label` test runs only on Windows.
- `tools/asta_compare_all.py` and `svg_compare.py` hard-code Astah and Inkscape paths without
  override, documented only in TASKS.md. `JdkTypes.java:15-16` imports `IOException` twice.

### Documentation

<a id="d-1"></a>
#### D-1 · Medium · `docs/handoff-source-import.md` is tracked, public and stale

Sources: F M19 (Medium), O D-2 (Low).

- `.gitignore:27` keeps it public, while CLAUDE.md and TASKS.md say only `architecture.md` is. It
  is addressed to one person, calls C#, C++ and Python the goal (shipped in 0.5.0) and names
  `--example java-import` (lines 102, 238), now `source-import`.
- It is the only place in the repository where the project rules are written down; moving them to
  a `CONTRIBUTING.md` (commands, language of docs and comments, i18n rule, `Document::edit` rule,
  no Astah files) and dropping or ignoring the handoff keeps them visible.
- `.gitignore` excludes `docs/*` except two files, so new docs need `git add -f` or an entry.

<a id="d-2"></a>
#### D-2 · Low · Drift between docs and code

Sources: F, O D-1.

- architecture.md: slots are "own first, then the generals"; code, test and `object.rs:12-13` put
  the generals first. (F, O)
- architecture.md: a new vertex "belongs to the innermost state drawn around its center";
  creation uses the top-left ([E-11](#e-11)). (O)
- architecture.md: members "put in source order"; `merge.rs:522` groups literals, attributes,
  operations. The `source-import` example also takes `python`. (O)
- architecture.md names only system boundaries and packages as containers; `draw` also produces
  `Curve`; svg centres by `max(rounded, exact)`, not by rounded width; "25 on the receiver" is
  false for create ([E-4](#e-4)). (F)
- Settings are documented under `%APPDATA%` only ([U-12](#u-12)); "every region keeps at least 20"
  ([N-9](#n-9)). (O)
- `tools/asta-export/README.md:20` says the sidecar carries sequence message rows and frames;
  `layout.rs:263-265` and architecture.md say sequence diagrams need none. The README's command
  line lacks `-Xmx1024m`. (F, O)
- `map.rs:4-7` lists four diagram kinds; the module handles nine (state machines, components,
  deployment, instances, links, interaction uses missing). (F, O)
- README lists composite structure twice under "Not yet supported" (`:96`, `:99`). CHANGELOG 0.8.0
  repeats hidden slots, found-message signature and «create» without text. (F)
- TASKS.md's "Not committed yet" paragraph is stale. `assets/icons/README.md` says `subsystem` and
  `freeLine` have no tool; `subsystem` has one and the line tool is keyed `line`, so the
  `icons/uml/{Tool::key()}.svg` convention does not hold for line, text and select. (F)

## Test coverage

| Area | Tests | Well covered | Missing |
| --- | --- | --- | --- |
| model | 9 unit, 19 integration | cascade delete, weak unlinking, templates, generalization cycles, move/undo, z-order, dirty tracking | golden v1/v2 `.ucraft` files; every `skip_serializing_if` field of `Shape` and `Style`; negative paths of `check` and `apply` (`StillReferenced`, `StillShown`, `EdgesAttached`, `DuplicateId`, `ShapeChanged`, duplicate presentation id, second root); delete cascades for lifelines, slots, links and diagrams; a randomized apply/undo/redo property test (a throwaway one found no bug); overlapping `delete_many`; performance guards |
| format | 7 | byte-exact round trip, newer/foreign/orphaned files | non-finite floats, deep nesting, BOM, failing `save` |
| notation | 96 | Astah metrics, routing corners, label placement, hollow hit testing, `render_scaled`, main editing paths | `tidy` is a no-op on the sample; `fit_fragments` with a bar reaching out; create bars; nested fragments; operand and destroy deletion; branching numbers `-A/-B`; `hide_message_numbers`; paste with strong references and on sequence/state diagrams; member round trips of imported constructs and quoted defaults; nesting limit; `set_slot` with `=`; `restyle`; `drag_handle` through `EdgeShape`; `Preview::resized`/`line_end`; `message_label_at`; self-loops; hit precedence; lollipop/actor name hit; `with_contents` nesting and on a preview; align of grown nodes; many regions in a small state; `can_show`, `owner_for`, `wrap_in_fragment`, `move_to_slot`; SVG italic, underline, fill colour, curves, control characters and escaping |
| asta | 5 unit (+12 needing Astah) | jser primitives, byte-exact rewrite (with Astah) | everything in `map.rs` on CI, every jser type code and error path, hostile input |
| asta-export | 3 unit (+8 needing Astah) | layout sidecar | report parsing, temp-folder handling |
| reverse | 23 unit, 123 integration | IR and model snapshots per language and rule; undo/redo/second import for C#, C++, Python | the same loop for Java; same name with two kinds; hand-drawn association surviving re-import; alias cycles; alias-typed parameters; qualifier-only imports and namespace aliases to library namespaces; `.py` + `.pyi`; hidden/build/venv/unreadable folders; deep expressions; nested preprocessor fixtures; C# backslash comments; operator spelling; `Problem::Read`; `large_imports_are_fast` is ignored, asserts nothing and is too small for [R-9](#r-9) |
| app | 8–9 | workspace consistency, recent files | properties commit and sync (focus order, unchanged text); canvas math (`snap`, `Transform` round trip, `zoom_at` anchor invariance); preview vs commit; `split_types`, `associations_for`, `operations_for`, `kind_label`, `display_name`, `tree_name`, `owner_path`; `validate` after undo of a diagram creation; `close_diagram` next-tab choice; `delete_selection` cascade; file flows; settings load/save and corrupt settings (needs path parameters instead of env) |

All tests measure against `ApproxMeasure`; the real-font measurement is not covered.
`workspace.rs` has no GPUI dependency and could move into a crate whose tests run without GPUI's
system libraries; gpui-kit's test support should allow a headless focus test for the properties
panel ([U-2](#u-2)).

## Structure and maintainability

The crate boundaries are right; inside the crates a few modules have outgrown themselves:

| File | Lines | Suggested split |
| --- | --- | --- |
| `notation/src/scene.rs` | 3,857 (about 1,240 tests) | `scene/mod.rs` (types, `build`), `text.rs` (`TextMeasure`, `Line`, `wrap`, the one width rule), node layout per kind, `edges.rs` (`edge_for`, qualifiers, labels), `interact.rs` (hit testing, `nodes_in`, `with_contents`, handles, `align`, `restyle`), `geometry.rs` (`border_point`, `Outline`); `edge_for` (237 lines, 10–11 parameters), `layout_element` (187), `layout_classifier` (152) are the longest functions |
| `asta/src/map.rs` | 2,341 | structure, behaviors, diagrams, sequence diagrams, one shared visited guard ([A-1](#a-1)) |
| `app/src/properties.rs` | 1,907 | `render` 793 lines, `sync` 212, `commit` 204, `new` 255; the `Field` enum is kept in four parallel places (enum, `TEXT_FIELDS`, `watch` list, `commit`/`sync` matches), which is what makes [U-3](#u-3) possible. A table of `FieldSpec { field, state, label_key, read, write }` would collapse them and make read/write testable without GPUI. Fifteen near-identical `flag()` blocks (`:1227-1392`); Association vs Link end sync duplicated. `associations_for`, `operations_for`, `classifiers`, `split_types` belong in `umlcraft-notation` |
| `app/src/canvas.rs` | 1,838 | `paint.rs` (`Painter`, `Transform`, ~400 lines), `measure.rs`, context menu, inline editing, `Drag` state machine; `render` is 265 lines |
| `app/src/main_view.rs` | 1,535 | `files.rs` (prompt, open, import, save, export, recent files), `menus.rs`, `model_tree.rs` |
| `tools/asta-export/AstaExport.java` | 2,498 | acceptable for a single-file source launcher; keep every item inside `attempt` ([A-10](#a-10)) |

Other duplication:

- `sequence.rs` and `sequence_edit.rs` each have their own `Bar`/`Msg`/`Lane` with different
  fields (`sequence.rs:251-319`, `sequence_edit.rs:52-106`), and the two `bar_at` differ
  (innermost by level vs smallest height). "Bar touched at y within ±0.5" is written six times.
  `sequence::arrange` is 225 lines, `message_edge` 180.
- The four tree-sitter front-ends repeat `named_children`/`children`, `struct Reader { src,
  skipped }` with `text()`/`count()`, the `Frontend::parse` body, the preprocessor call and branch
  counting, `documentation()` (C# and C++ differ only by `//!`/`///<`), `squeeze`, `supertype()`
  and the per-declarator loop. A shared `lang.rs` would remove about 100 lines and give one place
  for iterative traversal ([R-6](#r-6)); `tag_prefix` belongs in `Rules`, `module_path` on
  `Frontend`.

## Strengths

- **One way to change the model.** `Model::apply` validates every primitive operation and returns
  its exact inverse; a randomized test of 39 seeds × 150 operations restored every state exactly,
  child order and z-order included. No path goes around `Document::edit`; undo is trivially right.
- **Load-time validation.** `Model::check` covers single root, ownership, double owners,
  reachability, reference roles and kinds, generalization cycles, views matching diagrams and
  presentation endpoints.
- **A deterministic, diff-friendly file format** with sorted ids, omitted defaults, a version and
  rejection of newer files.
- **Shared, measured geometry.** Canvas, hit testing, inline editing, SVG and PNG use
  `draw::Primitive`; Astah's text metrics come from measured tables; named constants
  (`RECEIVER_BAR`, `SENDER_BAR`, `SELF_DROP`, `MIN_REGION` …) match the docs.
- **A clean `.asta` reader and writer** without dependencies, byte-exact rewrite, and a mapping
  that covers templates, bindings, links, gates, fragments, label geometry and Java-pixel
  centring.
- **A language-neutral source import** with `Rules` as data, deterministic results, ids and
  presentations kept on re-import, and an Astah comparison harness with explained deviations.
- **Careful export through Astah's API:** no shell, paths as separate arguments, temp folder
  cleaned by RAII, license failure detected, every transaction ended, per-element failures
  reported.
- **Consistent hygiene:** `unsafe` forbidden; fmt, clippy and tests on three platforms under
  `--locked`; read-only CI permissions and concurrency cancel; complete bilingual locales; release
  checks of tag, crate version and metainfo; packaging tools verified by SHA-256; seven package
  formats with `cargo about` notices.

## Suggested order of work

1. **Work loss in the app** (a few lines each): [U-1](#u-1), [U-6](#u-6), [U-2](#u-2),
   [U-3](#u-3), [U-4](#u-4), [U-5](#u-5), [U-7](#u-7), [U-8](#u-8).
2. **Hostile `.asta` input:** [A-1](#a-1)–[A-5](#a-5), the thread join of [A-9](#a-9); import and
   loading off the UI thread ([U-17](#u-17)); byte-level jser tests and synthetic fixtures so CI
   covers them ([X-1](#x-1)).
3. **Unloadable projects:** [M-1](#m-1), [E-6](#e-6), a golden `.ucraft` fixture; durable save
   [M-6](#m-6).
4. **Sequence diagrams and paste:** [E-1](#e-1) (decide the growth rule, add the sample test),
   [E-4](#e-4), [E-2](#e-2), [E-3](#e-3), [E-5](#e-5).
5. **Source import:** [R-1](#r-1), [R-2](#r-2) (import marker), [R-4](#r-4), [R-3](#r-3),
   [R-5](#r-5)–[R-8](#r-8), each with a fixture; the Java re-import loop.
6. **Model invariants and everyday edit correctness:** [M-2](#m-2), [M-7](#m-7)–[M-9](#m-9),
   [E-7](#e-7), [E-8](#e-8), [N-3](#n-3)–[N-6](#n-6), [U-9](#u-9).
7. **Test infrastructure:** rest of [X-1](#x-1) (ignored instead of passing, fuzz targets), a
   property test for `apply`/undo, [D-1](#d-1).
8. **Performance**, when a large project feels slow: the measure and scene cache of [N-1](#n-1)
   first, then indexes for [M-3](#m-3)–[M-5](#m-5), [N-2](#n-2), [R-9](#r-9); timing guards at the
   sizes the docs advertise.
9. **Structure and the rest:** split `scene.rs`, `map.rs`, `properties.rs`, `canvas.rs`; shared
   tree-sitter helpers; [D-2](#d-2), [X-2](#x-2)–[X-4](#x-4); remaining Low findings.

## Mapping of original IDs

| Fable 5.1 | Here | | Opus 5.5 | Here | | Opus 5.5 | Here |
| --- | --- | --- | --- | --- | --- | --- | --- |
| H1 | A-2 | | A-1 | A-1 | | R-1 | R-1 |
| H2 | A-3 | | A-2 | A-2 | | R-2 | R-3 |
| H3 | R-1 | | A-3 | A-3 | | R-3 | R-5 |
| H4 | R-2 | | A-4 | A-4 | | R-4 | R-6 |
| H5 | E-1 | | A-5 | A-5 | | R-5 | R-7 |
| H6 | U-1 | | A-6 | A-7 | | R-6 | R-8 |
| M1 | M-2 | | A-7 | A-6 | | R-7 | R-9 |
| M2 | M-1 | | A-8 | A-10 | | R-8 | R-10, R-2 |
| M3 | U-3 | | A-9 | A-11 | | U-1 | U-2 |
| M4 | U-6 | | A-10 | A-9, A-12, D-2 | | U-2 | U-7 |
| M5 | U-9 | | M-1 | M-1 | | U-3 | U-3 |
| M6 | E-5, E-3 | | M-2 | M-3 | | U-4 | U-4 |
| M7 | E-3 | | M-3 | M-4 | | U-5 | U-5 |
| M8 | E-4 | | M-4 | M-5 | | U-6 | U-8 |
| M9 | E-7, E-11 | | M-5 | M-6 | | U-7 | N-1 |
| M10 | A-1 | | M-6 | M-7 | | U-8 | U-10 |
| M11 | A-8 | | M-7 | M-2 | | U-9 | U-9 |
| M12 | A-6 | | M-8 | M-8 | | U-10 | U-11 |
| M13 | R-3 | | M-9 | M-9 | | U-11 | U-14 |
| M14 | R-4 | | M-10 | M-10, U-1 | | U-12 | U-12 |
| M15 | N-3 | | N-2 | N-2 | | U-13 | U-13 |
| M16 | N-1 | | N-3 | N-4 | | U-14 | U-15 |
| M17 | M-3, M-4, R-9 | | N-4 | N-5 | | U-15 | U-17 |
| M18 | X-1 | | N-5 | N-6 | | X-1 | X-1 |
| M19 | D-1 | | N-6 | N-9 | | X-2 | X-2 |
| M20 | M-8 | | N-7 | N-10 | | X-3 | X-3 |
| | | | N-8 | M-1 | | X-4 | X-4 |
| | | | N-9 | N-7 | | D-1 | D-2 |
| | | | N-10 | N-11 | | D-2 | D-1 |
| | | | N-11 | N-12 | | | |
| | | | E-1 | E-2 | | | |
| | | | E-2 | E-3 | | | |
| | | | E-3 | E-7 | | | |
| | | | E-4 | E-8 | | | |
| | | | E-5 | E-6 | | | |
| | | | E-6 | E-4 | | | |
| | | | E-7 | E-10 | | | |
| | | | E-8 | E-9 | | | |
| | | | E-9 | E-11 | | | |
