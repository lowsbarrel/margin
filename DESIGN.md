# Margin design rules

How Margin looks, reads and behaves. Every visible change follows these rules;
review a change against the [checklist](#review-checklist) before calling it
done. Values live in one place, `margin-theme`; this document names them and
says when to use which. Architecture is in
[docs/architecture.md](docs/architecture.md).

## Principles

1. **The note is the product.** Chrome stays quiet, neutral and dense so the
   text carries the screen. Colour is for meaning (brand, state, syntax), never
   decoration.
2. **What you typed is what is on disk.** Rendering never changes the file. The
   editor reveals markup where the caret is and hides it elsewhere; it does not
   invent formatting.
3. **Keyboard first, mouse welcome.** Every command has a keyboard path
   (shortcut, Spotlight or menu). Pointer gestures are shortcuts to commands
   that exist elsewhere.
4. **Desktop, not web.** Persistent navigation, resizable regions, context
   menus, native window chrome and platform shortcuts (`⌘` on macOS, `Ctrl`
   elsewhere). No hover-only actions, no web-page layouts.
5. **State is visible.** Saved, syncing, locked, selected, focused, disabled
   and failed each have one consistent look.
6. **Tokens, not values.** No colour, radius, spacing, font size or duration is
   written outside `margin-theme`. A missing role is added there, with both
   themes, before it is used.
7. **Compose before inventing.** Use a GPUI Kit component when one exists.
   A new primitive needs behaviour no component has, not a different look.

## Colour

Colours are semantic roles, resolved per theme. The light and dark values are
the Tauri app's `tokens.css`, ported unchanged.

### Primitives

HSL ramps, 5 (lightest) to 95 (darkest): `neutral`, `orange` (brand), `green`,
`yellow`, `red`, `purple`, `blue`, `cyan`, plus black and white opacity steps.
Primitives are never used directly by views; only roles are.

### Roles

| Role                | Use for                                              | Light          | Dark            |
| ------------------- | ---------------------------------------------------- | -------------- | --------------- |
| `text.primary`      | Body text, titles, active labels                     | neutral-100    | neutral-0       |
| `text.secondary`    | Supporting text, inactive tabs, metadata             | neutral-60     | neutral-40      |
| `text.tertiary`     | Placeholders, panel titles, hints, hidden markup     | neutral-40     | neutral-60      |
| `text.disabled`     | Disabled controls                                    | neutral-30     | neutral-70      |
| `text.brand`        | Links, wiki links, the active accent                 | orange-70      | orange-50       |
| `text.positive` / `warning` / `negative` | Outcome text                    | green/yellow/red-70 | green-40/yellow-30/red-40 |
| `text.code`         | Inline code                                          | red-70         | orange-30       |
| `surface.0`         | Editor and main content                              | neutral-0      | neutral-100     |
| `surface.1`         | Sidebar, panels, status bar                          | 97.5% L        | 10.5% L         |
| `surface.2`         | Inputs, code blocks, table headers                   | 94.5% L        | 13.5% L         |
| `surface.3`         | Hover and pressed fills                              | 91% L          | 17% L           |
| `bg.brand` / `bg.brand.hover` | The one primary action, focus ring, caret accent | orange-70/60 | orange-50/40 |
| `bg.negative` / `bg.negative.hover` | Destructive confirmation buttons     | red-70/60      | red-40/30       |
| `bg.overlay`        | Scrim behind dialogs                                 | 8% L / 32%     | black / 56%     |
| `border.secondary`  | Control and card outlines                            | black 8%       | white 8%        |
| `border.divider`    | Region separators                                    | neutral-10     | neutral-80      |
| `border.strong`     | Emphasised outlines, drag targets                    | black 16%      | white 16%       |
| `brand.8` … `brand.32` | Selection (16% unfocused, 32% focused), drop zones, matches | brand at that alpha | — |
| `syntax.*`          | Code: keyword, string, number, function, comment, attr | see tokens   | see tokens      |
| `callout.*`         | The 13 callout kinds (note, abstract, info, todo, tip, success, question, warning, failure, danger, bug, example, quote) | see tokens | see tokens |
| `highlight.*`       | `==highlight==` and coloured marks: yellow, blue, green, purple, red, orange | at 38–45% | same |
| `canvas.grid` / `canvas.ink` | Canvas background grid and default ink       | see tokens     | see tokens      |

Rules:

- Brand orange appears once per decision area: the primary button, the focus
  ring, links. Two orange buttons side by side is a bug.
- Status uses text and icon first; a filled status background only for a
  destructive confirmation.
- Selection is a fill (`brand.16`/`brand.32`), never a coloured border or a
  leading bar.
- Images in the lightbox are the only content drawn on a fixed black backdrop.

## Typography

| Use                       | Font                  | Size / line height      | Weight    | Tracking |
| ------------------------- | --------------------- | ----------------------- | --------- | -------- |
| UI base (labels, menus)   | Inter                 | 14 / 20                 | 400       | −0.01em  |
| UI small (status, badges) | Inter                 | 12 / 16                 | 400–500   | −0.01em  |
| Panel title               | Inter, uppercase      | 12 / 16                 | 600       | +0.06em  |
| Dialog title              | Inter                 | 18 / 26                 | 600       | −0.02em  |
| Note body                 | Inter                 | 15 / 1.6                | 400       | −0.01em  |
| Note title (inline)       | Inter                 | 32 / 38                 | 600       | −0.02em  |
| Headings H1–H6            | Inter                 | 1.625, 1.35, 1.15, 1.05, 1, 1 em / 1.3 | 600 | −0.02em |
| Code, terminal            | JetBrains Mono        | 0.875 em of context; terminal 13 | 400 | 0       |

- Inter uses `dlig` and `ss07` on, `calt` off, as the Tauri app did.
- Weights are 400, 500, 600 only. Emphasis comes from weight or colour, not
  both, and never from uppercase plus colour plus weight together.
- The note column is 750 px wide, centred, with 40 px side padding; text never
  runs full window width.
- Fonts are bundled; the app never depends on installed fonts.

## Space, size and shape

- **Spacing scale** (px): 2, 4, 8, 12, 16, 24, 32. Use the GPUI rem helpers
  that map onto it (`gap_1`, `p_2`, …); no other values.
- **Radius scale** (px): 4, 8, 12, 16, 20, 24, full. Controls 8, popovers and
  cards 12, dialogs 16. A child's radius equals its parent's radius minus the
  inset between them.
- **Control heights**: small 28, medium 34 (default), large 40. Icon-only
  buttons are square at the same heights.
- **Icons**: Lucide only, 13 px in small controls, 16 px elsewhere; stroke
  colour follows the text role of the control.
- **Elevation**: `shadow.sm` for raised controls, `shadow.md` for popovers and
  menus, `shadow.lg` for dialogs and the Spotlight. Dark theme uses the
  stronger dark shadows from the token table.
- **Zoom** changes the base font size; everything sized in rem follows. Pixel
  values are allowed only for hairlines (1 px borders) and measured geometry.

## Layout

```
┌──────────┬─────────────────────────────────────────────┬──────────┐
│ Sidebar  │ Pane │ Pane …  (tab strip + breadcrumbs)     │ Backlinks│
│ file     │                                              │ or       │
│ tree     │         note column (750 px, centred)        │ history  │
│          ├──────────────────────────────────────────────┤          │
│          │ Terminal dock (tabs)                         │          │
├──────────┴──────────────────────────────────────────────┴──────────┤
│ Status bar: sync state · word count · view mode · panels           │
└────────────────────────────────────────────────────────────────────┘
```

- Sidebar 180–480 px (default 260), resizable, collapsible with `Mod+\`.
- Panes split horizontally; each has its own tab strip; dividers drag. Pinned
  tabs come first and show only their icon.
- Right panel 280–480 px; one of backlinks or history at a time.
- Terminal dock 120 px to 70 % of the window, toggled with ``Mod+` ``.
- Panel headers are 44 px tall: uppercase panel title left, icon actions right.
- The window remembers its size, panes, tabs, cursor positions, expanded
  folders, panel sizes and visibility per vault.

## Components

- **Buttons**: `primary` (the one commit action), `secondary` (default),
  `ghost` (toolbars, panel actions), `danger` (destructive confirmation only),
  `success` (rare: a completed state the user acts on). Labels are verbs.
  A button that opens a dialog ends with `…`.
- **Inputs** use `surface.2`, `border.secondary`, and the brand focus ring.
  Validation text sits under the field in `text.negative`.
- **Menus**: a `DropdownMenu` when more commands exist for a region, a
  `ContextMenu` for the object under the pointer. Same verb, icon and shortcut
  everywhere a command appears. Destructive items are last, after a separator.
- **Dialogs** for decisions and settings; **notifications** for outcomes that
  are not visible otherwise; **inline messages** for errors that belong to a
  field or panel. Never a dialog to report success.
- **Lists and trees** are virtualized, keyboard navigable, multi-selectable
  where the task needs it, and show selection as a fill.
- **Empty states** say what will appear and how to make it appear, in one line,
  with at most one action.
- **Loading**: skeletons for content with a known shape, a spinner in the
  control for an action in progress, nothing for work under 150 ms.

## Interaction

| State          | Treatment                                                       |
| -------------- | --------------------------------------------------------------- |
| Rest           | Clear affordance, no decoration                                 |
| Hover          | `surface.3` fill or `text.primary` on ghost controls; never the only cue |
| Pressed / open | Persistent pressed fill while its menu or popover is open       |
| Focus visible  | 2 px `bg.brand` ring outside the control                        |
| Selected       | `brand.16` fill (unfocused list) or `brand.32` (focused)        |
| Disabled       | `text.disabled`, no hover or press response                     |
| Loading        | Control keeps its size, shows a spinner, ignores repeated input |
| Error          | What happened and how to recover, next to the thing that failed |

- Focus never gets lost: closing an overlay returns focus to what opened it;
  closing a tab focuses its neighbour's editor.
- Escape closes the innermost overlay, then clears secondary cursors, then
  leaves find; it never closes a tab or discards input.
- Cursors: arrow for controls, pointing hand for links, I-beam for text,
  resize/grab only while that manipulation is possible.
- Drag and drop: files from the OS drop into the tree (import) or the editor
  (attachment); tree rows and tabs drag between folders and panes. A valid
  target shows a `brand.8` fill and `border.strong` outline; an invalid one
  shows nothing.
- Reversible destruction (delete to trash, close tab) happens immediately with
  an undo path (trash, `Mod+Shift+T`). Irreversible destruction (empty trash,
  delete forever, remove a vault) asks first in an alert dialog naming the
  object.

### Keyboard

`Mod` is `⌘` on macOS and `Ctrl` elsewhere. Every shortcut is listed next to
its command in menus and tooltips.

| Shortcut              | Action                                   |
| --------------------- | ---------------------------------------- |
| `Mod+P`, `Mod+K`      | Spotlight (`Mod+K` inserts a link inside the editor) |
| `Mod+Shift+F`         | Search the whole vault                   |
| `Mod+N`               | New note                                 |
| `Mod+\`               | Toggle the sidebar                       |
| ``Mod+` ``            | Toggle the terminal                      |
| `Mod+Shift+E`         | Live preview ↔ raw Markdown              |
| `Mod+W`, `Mod+Shift+T`| Close tab, reopen the last closed tab    |
| `Mod+F`, `Mod+H`      | Find, find and replace in the note       |
| `Mod+Shift+V`         | Paste as plain text                      |
| `F2`                  | Rename the selected file or folder       |
| `Mod+,`               | Settings                                 |

Editor keys follow Obsidian's defaults (Tab indents, `Mod+L` cycles a
checklist, `Mod+/` toggles a `%%` comment, `Mod+D` deletes the line, `Alt+click`
adds a cursor).

## The editor

- Live preview hides inline markup unless the caret touches the element, and
  block markup (`#`, `>`, fences, `$$`, callout headers) unless the caret is on
  that line or block. Hidden markup takes no space; revealing it never reflows
  surrounding lines except the line being edited.
- Raw mode shows every character with Markdown syntax colours from the
  `syntax.*` roles; nothing is hidden.
- Links and wiki links use `text.brand`, open on click release; a missing note
  link is `text.tertiary` and creates the note when opened.
- Callouts are tinted boxes in their `callout.*` colour at 8 % fill with a
  2 px left rule in full colour and the kind's icon; nested callouts inset.
- Code blocks use `surface.2`, 8 px radius, a language label and a copy button
  that appear in the header row; highlighting uses `syntax.*`.
- Math and diagrams render as images in the text colour of the theme and
  re-render when the theme changes; selecting them copies their source.
- Tables render as a grid with `border.divider` lines and a `surface.2` header;
  the cell being edited shows its source.
- Images keep their aspect ratio, never exceed the column, and honour
  `|width` or `|widthxheight` sizes.
- The caret is 2 px `text.primary`; secondary carets are the same colour at
  60 %.

## Motion

- Two durations: 120 ms (hover, press, small reveals) and 150 ms (popovers,
  panels, dialogs), both ease-out.
- Motion explains a change of place or state: a panel sliding open, a popover
  growing from its trigger. It never delays a result.
- Reduced motion (OS setting) turns every duration to 0.

## Writing

Copy lives in `messages/<locale>.json`. Write each language as a native
speaker would, not as a translation of the English shape.

- **Voice**: calm, direct, second person when addressing the user. No
  exclamation marks, no "successfully", no "please", no "Are you sure…".
- **Case**: sentence case for everything in English (`Sync now`, not `Sync
  Now`); Italian follows Italian rules. Uppercase only via the panel-title
  style, never typed into a message.
- **Punctuation**: no final period on labels, buttons, menu items, tabs,
  placeholders and short states; complete sentences end with one. Use `…`
  (one character) for commands that need more input and for ongoing states
  (`Syncing…`).
- **Buttons** say the result: `Delete`, `Restore`, `Discard changes`; `Cancel`
  always leaves without committing; `OK` only acknowledges information.
- **Confirmation dialogs**: the title is the decision (`Delete “Roadmap”
  forever?`), the body adds only consequence or recovery, the actions are
  `Cancel` and the result.
- **Errors** say what happened and what to do: `Couldn’t reach the bucket.
  Check the endpoint and try again.` Technical details go after, in
  `text.secondary`, only when they help.
- **Placeholders** are examples or hints, never the only label.
- **No concatenation**: a sentence is one message with `{placeholders}`, so
  every language can reorder it.
- **Plurals** use separate keys ending in `_one` and `_other`
  (`sync_conflicts_one`, `sync_conflicts_other`); never `conflict(s)`.
- **Lexicon** (use these words, everywhere):

| English     | Italian                  | Meaning                                         |
| ----------- | ------------------------ | ----------------------------------------------- |
| vault       | vault                    | A folder of notes Margin manages                |
| note        | nota                     | A Markdown file                                 |
| attachment  | allegato                 | An image or file embedded in notes              |
| canvas      | canvas                   | A drawing file                                  |
| passphrase  | passphrase               | The 12 words that derive the vault key          |
| sync        | sincronizzazione / sincronizza | Encrypted mirroring to the bucket         |
| bucket      | bucket                   | The S3-compatible storage                       |
| trash       | cestino                  | Deleted files kept for 30 days                  |
| history     | cronologia               | Earlier versions of a note                      |
| version     | versione                 | One entry in the history                        |
| backlinks   | collegamenti in entrata  | Notes that link to this one                     |
| Spotlight   | Spotlight                | The palette                                     |
| tag         | tag                      | A `#tag`                                        |

## Accessibility

- Text contrast at least 4.5:1 (3:1 for 18 px and up and for icons); both
  themes are checked when a role changes.
- Every control has an accessible name (AccessKit); icon-only buttons have a
  tooltip with the same name and the shortcut.
- Everything works with the keyboard alone, in a logical focus order, with a
  visible focus ring.
- Respect the OS settings for reduced motion, high contrast and text scaling.
- Never convey state by colour alone: pair it with an icon or text.

## Platform fit

- macOS: native menu bar with the standard App/Edit/View/Window menus, traffic
  lights in the title bar, `⌘` shortcuts.
- Windows and Linux: integrated title bar with the platform's window controls,
  `Ctrl` shortcuts, no menu bar (commands live in Spotlight and context menus).
- Follow the OS light/dark appearance until the user picks a theme.
- File dialogs, open-in-default-app and reveal-in-file-manager are the native
  ones.

## Review checklist

- [ ] Uses roles from `margin-theme` only; checked in light and dark.
- [ ] Every interactive state from the table is designed and reachable.
- [ ] Works with the keyboard alone; focus returns where it came from.
- [ ] Copy is in `messages/en.json` and `messages/it.json`, follows Writing,
      and fits in Italian (longer) without clipping.
- [ ] At most one primary action per decision area.
- [ ] Destructive actions are either undoable or confirmed by name.
- [ ] Spacing, radius and size come from the scales; no stray pixel values.
- [ ] Checked at 100 % and 150 % zoom, and on Windows, macOS and Linux.
