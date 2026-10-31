package app.wonnet.theme

import android.graphics.Paint
import android.graphics.Typeface
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.unit.dp
import app.wonnet.App
import kotlin.math.cos
import kotlin.math.roundToInt
import kotlin.math.sin

/**
 * The icons the app draws, from Lucide's font (lucide-static 1.52.0), the Apple apps' file: each
 * is its character there. The filled star, which Lucide doesn't have, is drawn by hand.
 */
enum class Symbol(val character: String) {
    AlarmClock("\ue03a"), Archive("\ue041"), ArchiveRestore("\ue2cd"), ArrowLeft("\ue048"), ArrowUp("\ue04a"), AtSign("\ue04e"),
    Ban("\ue051"), Bell("\ue059"), BellOff("\ue05a"), CalendarClock("\ue304"), Check("\ue06c"), CheckCheck("\ue38e"),
    ChevronDown("\ue06d"), ChevronLeft("\ue06e"), ChevronRight("\ue06f"), ChevronUp("\ue070"), ChevronsDownUp("\ue228"),
    ChevronsUpDown("\ue211"), CircleAlert("\ue077"), CircleArrowDown("\ue078"), CircleCheck("\ue226"), Clock("\ue087"),
    Command("\ue09a"), Download("\ue0b2"), Ellipsis("\ue0b6"), File("\ue0c0"), FileArchive("\ue30d"), FileAudio("\ue31a"),
    FileImage("\ue31c"), FileSpreadsheet("\ue326"), FileText("\ue0cc"), FileVideo("\ue321"), FolderInput("\ue334"),
    Forward("\ue229"), Globe("\ue0e8"), Handshake("\ue5c0"), Image("\ue0f6"), Inbox("\ue0f7"), Keyboard("\ue284"),
    ListFilter("\ue460"), Loader("\ue109"), LogOut("\ue10e"), Mail("\ue10f"), MailMinus("\ue362"), MailOpen("\ue363"),
    MailX("\ue368"), Menu("\ue115"), Monitor("\ue11d"), Moon("\ue11e"), Paperclip("\ue12d"), Pencil("\ue1f9"), Plus("\ue13d"),
    Presentation("\ue4ae"), Printer("\ue141"), RefreshCw("\ue145"), Reply("\ue22a"), ReplyAll("\ue22b"), Search("\ue151"),
    Send("\ue152"), Server("\ue153"), Settings("\ue154"), Share("\ue155"), ShieldAlert("\ue1fe"), SquareCheck("\ue559"),
    SquarePen("\ue172"), Star("\ue176"),

    /** A star filled in, drawn by hand. */
    StarFilled(""),
    Sun("\ue178"), Tag("\ue17f"), Trash("\ue18e"), Undo("\ue2a1"), User("\ue19f"), WifiOff("\ue1af"), X("\ue1b2"), Zap("\ue1b4"),
    ;

    companion object {
        /** The icon a mailbox is drawn with, by the name the core gives it. */
        fun named(name: String): Symbol = when (name) {
            "inbox" -> Inbox
            "mail" -> Mail
            "star" -> Star
            "clock" -> Clock
            "send" -> Send
            "file" -> File
            "archive" -> Archive
            "shield-alert" -> ShieldAlert
            "trash" -> Trash
            else -> Tag
        }
    }
}

/** Lucide's font, read from the app's assets once. */
private val lucide: Typeface by lazy {
    runCatching { Typeface.createFromAsset(App.context.assets, "lucide.ttf") }.getOrDefault(Typeface.DEFAULT)
}

/** The side of a symbol `size` points wide on the Mac, as iOS enlarges it. */
fun symbolSide(size: Float) = (size * SCALE).roundToInt().dp

/** The symbol in one colour, `size` points wide on the Mac. */
@Composable
fun Icon(symbol: Symbol, size: Float, color: Color, modifier: Modifier = Modifier) {
    Canvas(modifier.size(symbolSide(size))) { drawSymbol(symbol, this.size.width, color) }
}

/** Draws the symbol in a square of `side` pixels at the top left. */
fun DrawScope.drawSymbol(symbol: Symbol, side: Float, color: Color, at: Offset = Offset.Zero) {
    if (symbol == Symbol.StarFilled) {
        drawPath(filledStar(side, at), color)
        return
    }
    val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        typeface = lucide
        textSize = side
        this.color = color.toArgb()
    }
    // Lucide's glyphs fill their square from the baseline up.
    drawContext.canvas.nativeCanvas.drawText(symbol.character, at.x, at.y + side, paint)
}

/** A five-pointed star in Lucide's square of 24, with the room it leaves around its icons. */
private fun filledStar(side: Float, at: Offset): Path {
    val path = Path()
    val centerX = at.x + side / 2
    val centerY = at.y + side / 2 + side * 0.02f
    val outer = side * 0.42f
    val inner = outer * 0.45f
    for (point in 0 until 10) {
        val radius = if (point % 2 == 0) outer else inner
        val angle = -Math.PI / 2 + point * Math.PI / 5
        val x = centerX + radius * cos(angle).toFloat()
        val y = centerY + radius * sin(angle).toFloat()
        if (point == 0) path.moveTo(x, y) else path.lineTo(x, y)
    }
    path.close()
    return path
}
