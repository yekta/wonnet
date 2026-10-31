package app.wonnet.ui

import android.text.format.Formatter
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.MutableState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.wonnet.core.Address
import app.wonnet.core.ShapeLine
import app.wonnet.theme.Dims
import app.wonnet.theme.Icon
import app.wonnet.theme.Radius
import app.wonnet.theme.SCALE
import app.wonnet.theme.Space
import app.wonnet.theme.Symbol
import app.wonnet.theme.Text
import app.wonnet.theme.TextStyle
import app.wonnet.theme.Tokens

/** One thing a `Combobox` offers: what it is called, a word about it, and what its row starts with. */
data class Choice(
    val id: String,
    val title: String,
    val detail: String? = null,
    val symbol: Symbol? = null,
    val dot: Color? = null,
    val avatar: Address? = null,
    val keywords: String = "",
) {
    /** Whether every word typed is in its title, its detail or its keywords. */
    fun matches(query: String): Boolean {
        val haystack = listOf(title, detail ?: "", keywords).joinToString(" ")
        return query.split(Regex("\\s+")).filter { it.isNotEmpty() }.all { haystack.contains(it, ignoreCase = true) }
    }
}

/**
 * A choice from a list that can grow, found by typing a few letters of it: the address a message
 * goes from. Its face shows the choice in force, as words for a line of a message's head; it
 * opens a sheet of the `title` with a field that narrows the list.
 */
@Composable
fun Combobox(
    title: String,
    placeholder: String,
    choices: List<Choice>,
    selected: String?,
    empty: String = "Nothing by that name.",
    /** Whether it is open, for an owner that opens it with a key of its own. */
    opened: MutableState<Boolean> = remember { mutableStateOf(false) },
    pick: (Choice) -> Unit,
) {
    var open by opened
    val current = choices.firstOrNull { it.id == selected }
    Pressable({ open = true }, help = title) { hovered ->
        Row(
            Modifier
                .heightIn(min = Dims.chipHeight)
                .background(if (hovered || open) Tokens.backgroundAccent.color else Color.Transparent, RoundedCornerShape(Radius.sm))
                .padding(horizontal = Space.xxs),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(Space.s),
        ) {
            when {
                current == null -> Text(placeholder, TextStyle.Body, color = Tokens.mutedStrongerForeground.color, maxLines = 1)
                else -> {
                    current.dot?.let { Box(Modifier.size(8.dp).background(it, CircleShape)) }
                    Text(current.title, TextStyle.Body, Modifier.weight(1f, fill = false), maxLines = 1)
                    current.detail?.let { Text(it, TextStyle.Caption, Modifier.weight(1f, fill = false), maxLines = 1) }
                }
            }
            Icon(Symbol.ChevronDown, 12f, Tokens.mutedStrongerForeground.color)
        }
    }
    if (!open) return
    var query by remember { mutableStateOf("") }
    var highlighted by remember { mutableIntStateOf(0) }
    val found = if (query.isBlank()) choices else choices.filter { it.matches(query.trim()) }
    fun choose(index: Int) {
        val choice = found.getOrNull(index) ?: return
        pick(choice)
        open = false
    }
    Sheet(title, { open = false }) {
        SearchField(
            placeholder, query, { query = it; highlighted = 0 }, submit = { choose(highlighted) },
            move = { highlighted = (highlighted + it).coerceIn(0, (found.size - 1).coerceAtLeast(0)) }, escape = { open = false },
        )
        ChoiceList(found, highlighted, Modifier.weight(1f), empty, key = { it.id }) { index, choice ->
            ChoiceRow(
                choice.title, detail = choice.detail, symbol = choice.symbol, dot = choice.dot, avatar = choice.avatar,
                checked = choice.id == selected, highlighted = index == highlighted,
            ) { choose(index) }
        }
    }
}

/**
 * Where a message's page will be while it is on its way or being drawn: a bar for every line of
 * its text and a block for every image, so the thread is about as tall as it will be and barely
 * moves when the page comes. `shape` is the page's lines, as the core gives them.
 */
@Composable
fun BodyPlaceholder(shape: List<ShapeLine>, modifier: Modifier = Modifier) {
    val fill = Dims.cardAccent.color
    BoxWithConstraints(modifier.fillMaxWidth()) {
        val width = maxWidth.value
        val drawing = remember(shape, width) { placeholder(shape, width) }
        val radius = with(LocalDensity.current) { app.wonnet.theme.Radius.xs.toPx() }
        Canvas(Modifier.fillMaxWidth().height(drawing.second.dp)) {
            val scale = size.width / width.coerceAtLeast(1f)
            for (block in drawing.first) {
                drawRoundRect(
                    fill, Offset(block[0] * scale, block[1] * scale), Size(block[2] * scale, block[3] * scale), CornerRadius(radius, radius),
                )
            }
        }
    }
}

/** The page's line: its text is 15px on 1.55. */
private const val LINE_HEIGHT = 23.25f
private const val BAR_HEIGHT = 10f
/** An average character of the page's text, with what wrapping leaves at the ends of lines. */
private const val CHARACTER_WIDTH = 7.2f
/** Past this the placeholder is off the screen. */
private const val TALLEST = 1600f
/** How far full lines reach, as ragged as text. */
private val reach = listOf(1f, 0.94f, 0.97f, 0.9f, 0.98f)

/** The bars and blocks (x, y, width, height) at this width, and how tall they stand: at least a line. */
fun placeholder(shape: List<ShapeLine>, width: Float): Pair<List<FloatArray>, Float> {
    val blocks = mutableListOf<FloatArray>()
    var height = 0f
    if (width <= 0) return blocks to LINE_HEIGHT
    val perLine = maxOf(1, (width / CHARACTER_WIDTH).toInt())
    fun bar(fraction: Float) {
        blocks += floatArrayOf(0f, height + (LINE_HEIGHT - BAR_HEIGHT) / 2, width * fraction, BAR_HEIGHT)
        height += LINE_HEIGHT
    }
    for (line in shape) {
        if (height >= TALLEST) break
        when (line) {
            is ShapeLine.Text -> {
                if (line.length <= 0) {
                    height += LINE_HEIGHT
                    continue
                }
                val full = (line.length - 1) / perLine
                repeat(full) { bar(reach[blocks.size % reach.size]) }
                bar(maxOf((line.length - full * perLine).toFloat() / perLine, 0.06f))
            }
            is ShapeLine.Image -> {
                val shown = minOf(maxOf(line.width, 1).toFloat(), width)
                val tall = line.height * shown / maxOf(line.width, 1)
                blocks += floatArrayOf(0f, height, shown, tall)
                height += tall
            }
        }
    }
    return blocks to maxOf(height, LINE_HEIGHT)
}

/**
 * A message's file as a card, as Gmail shows them under a message: a picture of it over its name,
 * its kind and its size. Until the picture is drawn, or for a file that has none, the icon of its kind.
 */
@Composable
fun AttachmentCard(
    name: String,
    mime: String,
    size: Long,
    preview: ImageBitmap?,
    pending: Boolean,
    modifier: Modifier = Modifier,
    onLongClick: (() -> Unit)? = null,
    onClick: () -> Unit,
) {
    val context = LocalContext.current
    val onCard = LocalOnCard.current
    val kind = attachmentKind(mime)
    val type = name.substringAfterLast('.', "").uppercase()
    val sized = Formatter.formatShortFileSize(context, size)
    val detail = if (type.isEmpty() || type.length > 5) sized else "$type · $sized"
    Pressable(onClick, modifier, help = "Open $name", onLongClick = onLongClick) { hovered ->
        val shape = RoundedCornerShape(Radius.md)
        val border = if (onCard) Dims.cardRule else Tokens.border
        val previewFill = if (onCard) (if (hovered) Dims.cardAccentStronger else Dims.cardAccent) else (if (hovered) Tokens.backgroundSecondaryAccent else Tokens.backgroundSecondary)
        val footerFill = if (onCard) (if (hovered) Dims.cardAccent else Dims.cardFill) else (if (hovered) Tokens.backgroundAccentLarger else Tokens.card)
        Column(Modifier.fillMaxWidth().clip(shape).border(Dims.hairline, border.color, shape)) {
            Box(Modifier.fillMaxWidth().height(Dims.attachmentPreviewHeight).background(previewFill.color), contentAlignment = Alignment.Center) {
                if (preview != null) {
                    Image(preview, null, Modifier.fillMaxWidth().height(Dims.attachmentPreviewHeight), contentScale = ContentScale.Crop, alignment = Alignment.TopCenter)
                } else {
                    Icon(kind, 28f, Tokens.mutedStrongerForeground.color)
                }
            }
            Rule(color = border)
            Row(
                Modifier.fillMaxWidth().background(footerFill.color).padding(horizontal = Space.s, vertical = Space.xs),
                horizontalArrangement = Arrangement.spacedBy(Space.xs),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                ButtonIcon(kind, pending, 14f, Tokens.mutedForeground.color)
                Column(verticalArrangement = Arrangement.spacedBy(1.dp)) {
                    Text(name, TextStyle.Caption, color = Tokens.foreground.color, maxLines = 1)
                    Text(detail, TextStyle.Footnote, maxLines = 1)
                }
            }
        }
    }
}

/** The icon of a file's kind. */
fun attachmentKind(mime: String): Symbol {
    val type = mime.lowercase()
    fun has(vararg words: String) = words.any { type.contains(it) }
    return when {
        type.startsWith("image/") -> Symbol.FileImage
        type.startsWith("video/") -> Symbol.FileVideo
        type.startsWith("audio/") -> Symbol.FileAudio
        has("spreadsheet", "excel", "csv", "numbers") -> Symbol.FileSpreadsheet
        has("presentation", "powerpoint", "keynote") -> Symbol.Presentation
        has("zip", "compressed", "tar", "rar", "7z", "gzip") -> Symbol.FileArchive
        has("pdf", "text", "word", "document", "rtf", "pages") -> Symbol.FileText
        else -> Symbol.File
    }
}

/** A long thread's run of folded messages tucked into one line, their count in a circle where the avatars stand. It shows them. */
@Composable
fun FoldedCount(count: Int, onClick: () -> Unit) {
    val onCard = LocalOnCard.current
    Pressable(onClick, Modifier.fillMaxWidth(), help = "Show $count earlier messages") { hovered ->
        Row(Modifier.fillMaxWidth().padding(vertical = Space.m), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(Space.m)) {
            val side: Dp = (Dims.messageAvatar * SCALE).dp
            Box(
                Modifier
                    .size(side)
                    .background(if (hovered) (if (onCard) Dims.cardAccent else Tokens.backgroundAccent).color else Color.Transparent, CircleShape)
                    .border(Dims.hairline, (if (onCard) Dims.cardRule else Tokens.border).color, CircleShape),
                contentAlignment = Alignment.Center,
            ) {
                Text("$count", TextStyle.LabelStrong, color = Tokens.mutedForeground.color)
            }
            Text("$count earlier messages", TextStyle.Body, color = if (hovered) Tokens.foreground.color else Tokens.mutedStrongerForeground.color)
            Spacer(Modifier.weight(1f))
        }
    }
}
