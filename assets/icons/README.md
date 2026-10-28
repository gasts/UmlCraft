# Toolbar icons

All icons are 24 × 24 SVGs in the style of Fluent UI System Icons (regular, 1.5 px lines). They
are meant for GPUI's `svg()` element, which uses only their shape and paints them in the current
text color, so light and dark themes and disabled buttons need no extra files.

## `fluent/`

Unchanged copies of [Fluent UI System Icons](https://github.com/microsoft/fluentui-system-icons)
by Microsoft, MIT license (`fluent/LICENSE`). The license text must ship with UmlCraft once the
icons are embedded.

| Use | File |
|---|---|
| Select tool | `ic_fluent_cursor_24_regular.svg` |
| Save | `ic_fluent_save_24_regular.svg` |
| Undo, redo | `ic_fluent_arrow_undo_24_regular.svg`, `ic_fluent_arrow_redo_24_regular.svg` |
| Zoom in, out, to fit | `ic_fluent_zoom_in_…`, `ic_fluent_zoom_out_…`, `ic_fluent_zoom_fit_…` |
| Align left, center, right | `ic_fluent_align_left_…`, `ic_fluent_align_center_horizontal_…`, `ic_fluent_align_right_…` |
| Align top, middle, bottom | `ic_fluent_align_top_…`, `ic_fluent_align_center_vertical_…`, `ic_fluent_align_bottom_…` |
| Distribute horizontally, vertically | `ic_fluent_align_space_evenly_horizontal_…`, `ic_fluent_align_space_evenly_vertical_…` |
| Theme | `ic_fluent_dark_theme_24_regular.svg` |
| Language | `ic_fluent_translate_24_regular.svg` |
| Text, rectangle, ellipse, line, freehand, image | `ic_fluent_text_t_…`, `ic_fluent_rectangle_landscape_…`, `ic_fluent_oval_…`, `ic_fluent_line_…`, `ic_fluent_pen_…`, `ic_fluent_image_…` |
| Pin (keep tool selected) | `ic_fluent_pin_24_regular.svg` |

## `uml/`

UmlCraft's own icons for the diagram tools, named after the palette's tool keys
(`Tool::key()`, e.g. `useCase.svg`, `asyncMessage.svg`). A few have no tool yet:
`navigableAssociation`, `aggregation`, `composition`, `associationClass`, `containment`,
`requiredInterface`, `subsystem` and `freeLine`.

`make_uml_icons.py` writes them; change the shapes there rather than in the SVGs. The shapes
are laid out on a 20 px grid and mapped onto Fluent's 24 px grid.
