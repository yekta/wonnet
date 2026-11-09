package app.wonnet.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import app.wonnet.theme.ControlSize
import app.wonnet.theme.Dims
import app.wonnet.theme.Icon
import app.wonnet.theme.Radius
import app.wonnet.theme.Space
import app.wonnet.theme.Symbol
import app.wonnet.theme.Text
import app.wonnet.theme.TextStyle
import app.wonnet.theme.Tokens

/** Where a search field sits: on a hairline over a sheet's list, over a popup's, or as a bar's capsule. */
enum class SearchStyle { Sheet, Popup, Bar }

/** ↑ and ↓ move through a list under a field (`-1`, `1`), Enter picks, Esc leaves it. */
fun Modifier.listKeys(move: (Int) -> Unit, escape: () -> Unit): Modifier = onPreviewKeyEvent { event ->
    if (event.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
    when (event.key) {
        Key.DirectionDown -> move(1)
        Key.DirectionUp -> move(-1)
        Key.Escape -> escape()
        else -> return@onPreviewKeyEvent false
    }
    true
}

/**
 * A field that narrows a list: the search icon, the words, and a way to clear them. It takes the
 * keys that move through the list (↑, ↓, Enter to pick, Esc to leave). `tag` is what the search
 * is narrowed to, a chip before the words; `clearTag` gives it an × and Backspace in the empty
 * field takes it away.
 */
@Composable
fun SearchField(
    placeholder: String,
    text: String,
    onText: (String) -> Unit,
    modifier: Modifier = Modifier,
    style: SearchStyle = SearchStyle.Sheet,
    symbol: Symbol = Symbol.Search,
    autofocus: Boolean = false,
    focusTrigger: Int = 0,
    tag: String? = null,
    clearTag: (() -> Unit)? = null,
    submit: () -> Unit = {},
    move: (Int) -> Unit = {},
    escape: () -> Unit = {},
    clear: () -> Unit = {},
) {
    val focus = remember { FocusRequester() }
    LaunchedEffect(autofocus, focusTrigger) { if (autofocus || focusTrigger > 0) runCatching { focus.requestFocus() } }
    val height = if (style == SearchStyle.Sheet) Dims.searchHeight else ControlSize.Regular.scaled
    val boxed = when (style) {
        SearchStyle.Bar -> Modifier.border(Dims.hairline, Tokens.borderInput.color, CircleShape)
        SearchStyle.Sheet -> Modifier.rule(Edge.Bottom, Tokens.borderCard)
        SearchStyle.Popup -> Modifier.rule(Edge.Bottom, Tokens.borderPopover)
    }
    Row(
        modifier.fillMaxWidth().height(height).then(boxed).padding(start = Space.m, end = if (text.isEmpty()) Space.m else Space.xxs),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(Space.s),
    ) {
        Icon(symbol, 14f, Tokens.mutedStrongerForeground.color)
        tag?.let { Chip(it, remove = clearTag) }
        val textStyle = (if (style == SearchStyle.Sheet) TextStyle.Body else TextStyle.Label).compose(Tokens.foreground.color)
        BasicTextField(
            text, onText,
            Modifier
                .weight(1f)
                .focusRequester(focus)
                .onPreviewKeyEvent { event ->
                    val untag = clearTag?.takeIf { text.isEmpty() && event.type == KeyEventType.KeyDown && event.key == Key.Backspace }
                    untag?.invoke()
                    untag != null
                }
                .listKeys(move) {
                    escape()
                    if (style == SearchStyle.Bar) onText("")
                },
            singleLine = true,
            textStyle = textStyle,
            cursorBrush = SolidColor(Tokens.foreground.color),
            keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.None, autoCorrectEnabled = false, imeAction = ImeAction.Search),
            keyboardActions = KeyboardActions(onSearch = { submit() }, onDone = { submit() }),
            decorationBox = { inner ->
                Box(contentAlignment = Alignment.CenterStart) {
                    if (text.isEmpty()) Text(placeholder, if (style == SearchStyle.Sheet) TextStyle.Body else TextStyle.Label, color = Tokens.mutedStrongerForeground.color, maxLines = 1)
                    inner()
                }
            },
        )
        if (text.isNotEmpty()) {
            IconButton(Symbol.X, "Clear", size = ControlSize.Small, circled = false, quiet = true) {
                onText("")
                clear()
            }
        }
    }
}

/**
 * A text field with its label above if it has one. With `lines` above one it is a text box that
 * many lines tall. `commit` runs on Return and when it loses the focus.
 */
@Composable
fun InputField(
    text: String,
    onText: (String) -> Unit,
    modifier: Modifier = Modifier,
    label: String? = null,
    placeholder: String = "",
    secure: Boolean = false,
    lines: Int = 1,
    autofocus: Boolean = false,
    keyboard: KeyboardType = KeyboardType.Text,
    capitalize: Boolean = false,
    commit: (() -> Unit)? = null,
) {
    val focus = remember { FocusRequester() }
    var focused by remember { mutableStateOf(false) }
    LaunchedEffect(autofocus) { if (autofocus) runCatching { focus.requestFocus() } }
    val shape = RoundedCornerShape(Radius.sm)
    Column(modifier, verticalArrangement = Arrangement.spacedBy(Space.xxs)) {
        label?.let { Text(it, TextStyle.Footnote) }
        BasicTextField(
            text, onText,
            Modifier
                .fillMaxWidth()
                .focusRequester(focus)
                .onFocusChanged {
                    if (focused && !it.isFocused) commit?.invoke()
                    focused = it.isFocused
                },
            singleLine = lines == 1,
            minLines = lines,
            textStyle = TextStyle.Body.compose(Tokens.foreground.color),
            cursorBrush = SolidColor(Tokens.foreground.color),
            visualTransformation = if (secure) PasswordVisualTransformation() else VisualTransformation.None,
            keyboardOptions = KeyboardOptions(
                capitalization = if (capitalize) KeyboardCapitalization.Sentences else KeyboardCapitalization.None,
                autoCorrectEnabled = lines > 1, keyboardType = if (secure) KeyboardType.Password else keyboard,
                imeAction = if (lines > 1) ImeAction.Default else ImeAction.Done,
            ),
            keyboardActions = KeyboardActions(onDone = { commit?.invoke() }),
            decorationBox = { inner ->
                Box(
                    Modifier
                        .fillMaxWidth()
                        .heightIn(min = Dims.fieldHeight)
                        .background(Tokens.input.color, shape)
                        .border(Dims.hairline, Tokens.borderInput.color, shape)
                        .padding(horizontal = Space.s + 2.dp, vertical = Space.s),
                    contentAlignment = if (lines > 1) Alignment.TopStart else Alignment.CenterStart,
                ) {
                    if (text.isEmpty()) Text(placeholder, TextStyle.Body, color = Tokens.mutedStrongerForeground.color)
                    inner()
                }
            },
        )
    }
}

/** The system's switch alone, in the primary colour when on. Its track is 32dp tall in a taller hit area. */
@Composable
fun Toggle(on: Boolean, onChange: (Boolean) -> Unit, enabled: Boolean = true) {
    Switch(
        on, onChange, Modifier.face(height = 32.dp), enabled = enabled,
        colors = SwitchDefaults.colors(
            checkedTrackColor = Tokens.primary.color, checkedThumbColor = Tokens.primaryForeground.color,
            uncheckedTrackColor = Tokens.backgroundAccentStronger.color, uncheckedThumbColor = Tokens.card.color,
            uncheckedBorderColor = Tokens.borderInput.color,
        ),
    )
}

/** A switch with its words. A tap anywhere on the row flips it; the row hovers as a large thing, its fill reaching `Space.s` past its words and its switch. */
@Composable
fun ToggleRow(title: String, on: Boolean, modifier: Modifier = Modifier, detail: String? = null, onChange: (Boolean) -> Unit) {
    Pressable({ onChange(!on) }, modifier.fillMaxWidth().pullOut(Space.s), role = Role.Switch) { hovered ->
        Row(
            Modifier
                .fillMaxWidth()
                .heightIn(min = ControlSize.Small.scaled)
                .background(if (hovered) Tokens.backgroundAccentLarger.color else Color.Transparent, RoundedCornerShape(Radius.md))
                .padding(horizontal = Space.s),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(Space.l),
        ) {
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(title, TextStyle.Body)
                detail?.let { Text(it, TextStyle.Caption) }
            }
            Toggle(on, onChange)
        }
    }
}

/**
 * Text to copy: in a field's box that can't be typed in, one line cut in the middle, selectable;
 * beside it the copy button, whose icon turns to a check once copied. `what` is what a screen reader says is copied. The button's face
 * reaches the end, as the field's the start.
 */
@Composable
fun CopyField(text: String, what: String, modifier: Modifier = Modifier) {
    val shape = RoundedCornerShape(Radius.sm)
    Row(modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(Space.s)) {
        Box(
            Modifier
                .weight(1f)
                .height(Dims.fieldHeight)
                .background(Tokens.input.color, shape)
                .border(Dims.hairline, Tokens.borderInput.color, shape)
                .padding(horizontal = Space.s + 2.dp),
            contentAlignment = Alignment.CenterStart,
        ) {
            SelectionContainer { Text(text, TextStyle.Body, maxLines = 1, overflow = TextOverflow.MiddleEllipsis) }
        }
        CopyButton(text, what, Modifier.pullOut(Dims.buttonGap / 2), size = ControlSize.Regular, iconOnly = true)
    }
}
