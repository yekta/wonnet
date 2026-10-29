# The design system

What the Mac app and the iOS app are drawn with. Everything here lives in
`Sources/MailUI/Shared`: a view composes these and adds nothing of its own. What is missing is
added to the component, not to the view.

## Rules

- Colours come from `Tokens`, opacities from `Opacity`, corners from `Radius` and shadows from
  `ShadowSize` (all generated from `packages/theme/tokens.json`). Never a hex value, a system
  colour, a number for an opacity or a radius: `Tokens.primary.opacity(Opacity.lit)`,
  `.opacity(Opacity.disabled, when: !enabled)`, `.shadow(.md)`. What is missing is added to
  `tokens.json`.
- The page is `background`: the bars, the sidebar, the list. A form's card is `card` on it. The
  one card that fills the page, an open thread or a message being written, is `Theme.cardFill`,
  `cardLarger`, floating on the page (`.pageCard()`) outlined in `border`; on iOS a message
  being written is a sheet of `cardLarger`. Menus, popups and toasts are `popover`. A hairline
  on the page is `border`; inside a card it is `borderCard` (`Theme.cardRule` in the card that
  fills the page), around a popover `borderPopover`. A field is filled with `input` and outlined in `borderInput`.
- Text comes from the scale, `TextStyle`: `Text("…").textStyle(.caption)`. A view picks a
  style, never a size. A style carries its usual colour; `color:` overrides it.
- Distances come from `Space`: `xxs` 4, `xs` 6, `s` 8, `m` 12, `l` 16, `xl` 20, `xxl` 28.
  Heights of controls come from `ControlSize` and `Theme`.
- Nothing clickable is under 28pt tall. 36pt is the default on the Mac; iOS enlarges everything
  by `Platform.scale`. `ControlSize` is `.small` (28), `.regular` (36) or `.large` (44).
- Everything clickable has a hover state, shown at once: no animation on colours. A button's
  face reads `@Environment(\.hovered)`, set by `PressStyle`, which every button uses.
- Hovered and selected things are filled from two pairs of tokens. Small things (buttons, tabs,
  the sidebar's rows) hover in `backgroundAccent` and are selected in
  `backgroundAccentStronger`. Large things (thread rows, choice rows, a banner) hover in
  `backgroundAccentLarger` and are pressed or highlighted in `backgroundAccentLargerStronger`:
  the same steps dimmer, because a fill across a whole row reads stronger than one behind a
  button. The thread row that is open stays in `backgroundAccentLarger`, the hover's fill, not
  the stronger one. From dimmest to strongest they go `backgroundAccentLarger`,
  `backgroundAccentLargerStronger`, `backgroundAccent`, `backgroundAccentStronger`, so a small
  thing's hover shows on a selected row. The actions on a hovered thread row hover in
  `backgroundAccentLargerStrongest`, so they show on the row's fill. A chip rests in
  `backgroundSecondary` and hovers in `backgroundSecondaryAccent`; on the card that fills the
  page (`.pageCard()`, which sets `onCard`) it rests in `Theme.cardAccent` and hovers in
  `Theme.cardAccentStronger`, since that card is as dark as `backgroundSecondary`. An accent is never thinned with an opacity: if a step is
  missing, add a token for it. A control with a colour of its own (the star) hovers in a wash
  of it: the colour at `Opacity.colorTint`, lighter in the dark scheme. Pressed is hovered: on
  iOS a press shows the hover's fill. A disabled button shows at `Opacity.disabled`, a filled
  one lights at `Opacity.lit`.
- Things side by side have no gap between them: an `HStack(spacing: 0)` of buttons, rows with
  no spacing. The space is inside each control, so hover areas and hit areas touch. Buttons
  with a face (a circle, a pill) show `Theme.buttonGap` (6pt) between the faces, rows one
  above the other `Theme.rowGap` (2pt); each pads half of it inside its hit area, and its hit
  shape is the whole rectangle. The gap is only drawn, never dead: a view never adds spacing
  between buttons. Buttons at the edge of content are pulled out by that half, so the face
  lines up with the content and the hit area reaches past it.
- No words end in "…": not a button, a menu item, a status or a placeholder. "Remove Account",
  "Syncing", never "Remove Account…".
- Every button can carry an icon and has a `pending` state; while pending, the spinner stands
  where the icon goes and the button waits.
- Everything is shown as soon as it is known; what is being fetched again is shown quietly
  (a chip's spinner, a mini spinner in a row), never by replacing what is there.
- Errors are always shown, with what went wrong: a `Notice` in error tone where the user is
  looking, or a toast with the message.
- The platform's controls are used when they fit, not by default: the switch (`ToggleRow`),
  alerts, menus, iOS's navigation bar for sheets' titles and buttons, Quick Look, the file
  pickers. Choice menus, dropdowns, fields and buttons are ours.
- A menu or a dropdown holds a few choices, as many as fit a glance. A list that can grow is
  never a menu to scroll: a choice of one thing (the address to send as) is a `Combobox`, a
  popup under its face on the Mac and a sheet on iOS; what is done to a thread (labels, times)
  is a `Sheet` with a `SearchField` that narrows a `ChoiceList`.
- The primary colour is for the one thing a page is for: Send over a message, the switch
  that is on, a selected row's check. Nothing else is blue: a sheet's Cancel, an icon in a
  bar and a link are in the foreground's colour.
- iOS's settings are the system's own: a page sheet of grouped `List`s, on `background`
  with rows on `card`, with its `Toggle`s, menu `Picker`s, `TextField`s, pages pushed for
  what is written, swipes to remove and a hold to reorder, each group a `SettingsListSection`
  given its name and the line under it as words, never a `Section` styled by hand. The Mac's
  are ours: the sections beside the page, and `SettingsGroup`s of `SettingsRow`s.
- A list whose rows the user puts in order is a `MovableList`. It picks a row up itself when it
  is pressed and pulled (held, on iOS): a picture of it shrinks a little on its light and
  follows the pointer, the rows it passes slide out of its way, and let go it slides into its
  place. Its rows are faces, not buttons: the list takes the clicks and tells each row when it
  is hovered. Nothing else drags: no handle, no edit mode.
- Sheets go through `Sheet`, on `background`: on iOS it gets the system's title and its X, or
  its Cancel, at the start of the navigation bar, in the foreground's colour; on the Mac our
  `HeaderBar` and Esc. What trails in the bar is ours on the Mac and the system's plain
  `Button`s on iOS, the main one `.borderedProminent`: the system draws the bar's buttons, and
  ours inside it would be a pill in a pill. The bar holds the one thing the sheet is for; what
  else can be done is in a toolbar under its content, as the message being written has it.
  `SheetSize` picks the window on the Mac; on iOS every sheet comes up the whole screen, in
  one move, so a list in it is never half shown and then pushed up by the keyboard.
- What floats over the mail on iOS is on the system's glass, as Motile has it: a `GlassBar` or
  a `GlassCircle` with its icons `strong`, in the foreground's colour; the sheet's X is ours, on
  the system's round glass button; the system's search box (`searchField`, in
  `iOS/GlassIOS.swift`) sits in the bottom bar with its magnifier in the foreground's colour
  and its placeholder in `mutedStrongerForeground`. Nothing on glass is in the primary colour.

## Components

| Component | What it is |
| --- | --- |
| `ActionButton`, `ActionMenu` | Words, an optional icon, five variants (`primary`, `success` for what finishes a thing done, `outline`, `ghost`, `destructive`), three sizes, `pending`, `wide`, `iconOnly`; the menu opens one |
| `IconButton`, `IconMenu`, `IconCircle` | An icon in a thin circle; `active`, `quiet`, `strong` (the foreground's colour, on a bar), `circled`, `pending` |
| `FloatingButton` | An icon on a bordered circle of the page's colour, with a little shadow, over a page: Compose over the list on iOS; `small`, the way back to the top of a list scrolled past its first page |
| `GlassBar`, `GlassCircle` | A bar floating over a page on the system's glass (the page's colour on a bordered capsule before iOS 26): icon buttons spread evenly across a capsule, and one action on a circle beside it; the commands under an open thread and a selection on iOS |
| `OptionButton` | A wide button on a card, for one of a few ways to do something: a mark, what it is, a line about it and a chevron; the providers an account comes from |
| `AppMark`, `GoogleMark` | The app's icon drawn small, and Google's G in its colours |
| `PlainButton` | Only its content, still with hover, press and disabled states, and the minimum height; `tint` colours its hover |
| `StarButton` | A thread's star, in the star colour when starred or hovered |
| `SelectCircle` | The circle that selects threads, filled in the primary colour with a check once selected; the Mac's rows draw it in the gutter before them |
| `InputField` | A text field with its label above, or a text box of `lines` |
| `FieldArea` | Behind a field's box, makes all of it the field: a click anywhere on it puts the caret in the field, and the pointer is the I-beam over it. Every field has one |
| `SearchField` | The search icon, the words, Clear; `.sheet` on a hairline with list keys, in a `SearchedList`, over the list on the Mac: on iOS `SearchedList` gives the list the system's search box along the bottom instead, unfocused until tapped; `.popup` the same, smaller, at the top of a popup; `.bar` as the top bar's capsule |
| `Dropdown` | A choice from a short list, with a label |
| `Combobox` | A choice from a list that can grow, found by typing: its face shows the one in force, a `.field` with a label or `.inline` words; opens a popup with a `SearchField` over a `ChoiceList` on the Mac, drawn by the window's or the sheet's `comboboxHost` so nothing is over it, a sheet on iOS |
| `ChoiceMenu` | One of a few choices, as a button with a chevron that opens them |
| `ToggleRow` | A switch with its words |
| `Disclosure` | A line that opens more |
| `Chip` | A small pill: an address, a file, a label; `remove`, `action`, `pending`, `invalid` |
| `AttachmentCard` | A message's file under it, as Gmail shows them: its picture (or its kind's icon) over its name, kind and size; `pending` |
| `FoldedCount` | A long thread's run of folded messages tucked into one line, their count in a circle where the avatars stand; it shows them |
| `Spinner` | Lucide's loader turning, in the muted text colour or a `tint`, for what is still coming: a mailbox syncing after its title, the next page of a list, a pending button, an update. Never the system's `ProgressView`, which the Mac won't colour |
| `ItemRow` | A thing in a list with its controls at the end |
| `NavRow` | A row that goes somewhere, with a count and a selected state; without an action, its face for a `MovableList` |
| `ChoiceRow`, `ChoiceList` | One choice under a field, starting with an icon, an account's dot or a person's `Avatar`, with a check when it is the one in force; and the list that scrolls to the highlighted one |
| `PopupCard` | A card over the page for suggestions; `inset` is the space to its edges |
| `TabStrip` | Tabs on a line with counts |
| `PillTabs` | Tabs on a line with an icon and a count, the chosen one filled in a rounded rectangle; dragged to reorder, with a menu on each |
| `BottomTabs` | Tabs across the bottom of the screen, full width, each an icon over its name and count; held and dragged to reorder (iOS) |
| `MovableList` | Rows along an axis that the list picks up and moves itself, in groups, with the rows under one going along; the sidebar and the tab strips are made of it |
| `SectionHeading`, `FormSection` | A section's name over a muted line about it, with its controls |
| `Card`, `FormButtons` | A rounded card, and Save and Cancel under a form |
| `Notice` | A line about something, muted or in error tone, with an optional action |
| `EmptyState` | An icon and a calm line for an empty list |
| `BodyPlaceholder` | A bar for every line of a message's text, as long as the line, where its page will be until the page is drawn |
| `Banner`, `ToastView` | A card floating at the bottom that stays, with its icon, a line (or a progress bar beside it), an action and a close (the Mac's update), always the same height; its tint is its state: `process` under way, `primary` waiting on the user, `success` done (its button too: Restart is `success`), `destructive` failed. And the note at the bottom that goes |
| `Avatar` | Initials in a circle of the address's colour |
| `SwatchRow` | The account colours on a line, to pick one |
| `SettingsGroup`, `SettingsCard`, `SettingsRow`, `SettingsLabel`, `SettingsDivider`, `SettingsNote` | The Mac's settings: a group's name over a card of rows, each what it is about on the left and its controls on the right; a control at either end of a row is as far from the card's side as from the row's top and bottom |
| `Switch` | The system's switch alone, for a row that says what it does |
| `KeyCaps`, `KeyRecorder` | Keys on caps, and the Mac's button that records new ones for a keybinding |
| `Rule`, `.rule(edge)` | A hairline, in `border` or in `borderCard` |
| `Sheet`, `HeaderBar` | Every sheet's shell, and the bar across the top of a page: the Mac's, and iOS's over the list; with a `center`, the sides split what it leaves; `inset` is the space to its edges |
| `FlowLayout` | Wrapping layout for chips |

The gallery shows all of them in every state: open the palette (⌘K) and pick "Component
gallery".
