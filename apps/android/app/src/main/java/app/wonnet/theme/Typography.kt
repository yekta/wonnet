package app.wonnet.theme

import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.sp

/**
 * Every size of text the app draws, with the weight and the colour it usually has, as the Apple
 * apps' `TextStyle`. A view picks a style, never a size.
 */
enum class TextStyle(val size: Float, val weight: FontWeight) {
    /** The app's name on the first screen. */
    Display(28f, FontWeight.SemiBold),
    /** A thread's subject. */
    Title(24f, FontWeight.SemiBold),
    /** A sheet's or a pane's heading. */
    Heading(17f, FontWeight.SemiBold),
    /** A heading inside a page: a message's sender, a draft's name. */
    Subheading(14f, FontWeight.SemiBold),
    /** Mail, fields and rows. */
    Body(14f, FontWeight.Normal),
    /** Body text that stands out: an unread row, a selected one. */
    BodyStrong(14f, FontWeight.Medium),
    /** Controls, bars and the words beside them. */
    Label(13f, FontWeight.Normal),
    /** A control's words when they must stand out: a button. */
    LabelStrong(13f, FontWeight.Medium),
    /** A line under a title: a detail, a date, a hint. */
    Caption(12f, FontWeight.Normal),
    /** The smallest words: a count, a field's label. */
    Footnote(11f, FontWeight.Normal),
    /** The smallest words when they must stand out: the chosen tab. */
    FootnoteStrong(11f, FontWeight.Bold),
    /** Keys and codes. */
    Mono(12f, FontWeight.Normal),
    ;

    /** The colour the style has unless a view says otherwise. */
    val color: ThemeColor
        get() = when (this) {
            Caption, Footnote, FootnoteStrong -> Tokens.mutedStrongerForeground
            Mono -> Tokens.mutedForeground
            else -> Tokens.foreground
        }

    /** The style as Compose draws it, in `color`. */
    fun compose(color: Color): androidx.compose.ui.text.TextStyle = androidx.compose.ui.text.TextStyle(
        color = color,
        fontSize = (size * SCALE).sp,
        fontWeight = weight,
        fontFamily = if (this == Mono) FontFamily.Monospace else FontFamily.Default,
    )
}

/** Words in a style of the scale, in its colour or in `color`. */
@Composable
fun Text(
    text: String,
    style: TextStyle,
    modifier: Modifier = Modifier,
    color: Color? = null,
    maxLines: Int = Int.MAX_VALUE,
    overflow: TextOverflow = TextOverflow.Ellipsis,
    align: TextAlign? = null,
) {
    val drawn = style.compose(color ?: style.color.color).let { if (align != null) it.copy(textAlign = align) else it }
    BasicText(text, modifier, drawn, maxLines = maxLines, overflow = overflow)
}

/** Text with spans of its own colours, in a style of the scale. */
@Composable
fun Text(
    text: AnnotatedString,
    style: TextStyle,
    modifier: Modifier = Modifier,
    color: Color? = null,
    maxLines: Int = Int.MAX_VALUE,
) {
    BasicText(text, modifier, style.compose(color ?: style.color.color), maxLines = maxLines, overflow = TextOverflow.Ellipsis)
}
