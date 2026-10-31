package app.wonnet.theme

import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.dropShadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.shadow.Shadow
import androidx.compose.ui.unit.DpOffset
import androidx.compose.ui.unit.dp

/** Whether the app is drawn in the dark scheme. */
val LocalDark = staticCompositionLocalOf { false }

/** A number of `packages/theme/tokens.json`, light and dark. */
data class ThemeNumber(val light: Float, val dark: Float) {
    fun value(dark: Boolean): Float = if (dark) this.dark else light

    val current: Float
        @Composable @ReadOnlyComposable get() = value(LocalDark.current)
}

/** A colour of `packages/theme/tokens.json`, light and dark, as `0xRRGGBB`. */
data class ThemeColor(val light: Int, val dark: Int, val alpha: ThemeNumber = ThemeNumber(1f, 1f)) {
    /** The colour at an opacity of the theme, such as `Tokens.shadow.opacity(Opacity.shadow)`. */
    fun opacity(alpha: ThemeNumber) = copy(alpha = alpha)

    fun color(dark: Boolean): Color {
        val hex = if (dark) this.dark else light
        return Color(0xff000000.toInt() or hex).copy(alpha = alpha.value(dark))
    }

    /** The colour in the scheme the app is drawn in. */
    val color: Color
        @Composable @ReadOnlyComposable get() = color(LocalDark.current)
}

/** Thins the whole view to one of the opacities, while `on`. */
@Composable
fun Modifier.opacity(opacity: ThemeNumber, on: Boolean = true): Modifier = if (on) alpha(opacity.current) else this

/** A shadow of the size, in the colour shadow at one of its opacities, under `shape`. */
@Composable
fun Modifier.shadow(size: ShadowSize, shape: Shape = CircleShape, strength: ThemeNumber = Opacity.shadow): Modifier =
    dropShadow(shape, Shadow(radius = size.blur.dp, color = Tokens.shadow.opacity(strength).color, offset = DpOffset(0.dp, size.down.dp)))

/** The distances between things. Views space with these, never a number of their own. */
object Space {
    /** A label and its field, a control's edge and its contents. */
    val xxs = 4.dp
    /** Inside a control: an icon and its words. */
    val xs = 6.dp
    /** Between controls on a line. */
    val s = 8.dp
    /** Between the lines of a block. */
    val m = 12.dp
    /** Between blocks, and a page's inset on a phone. */
    val l = 16.dp
    /** A page's inset on a tablet. */
    val xl = 20.dp
    /** Between the sections of a page. */
    val xxl = 28.dp
}

/** How much larger than on the Mac text and the controls around it are, as on iOS. */
const val SCALE = 1.15f

/** How tall a control is. Nothing clickable is under `Small`; `Regular` is the usual. */
enum class ControlSize(val height: Float, val icon: Float, val text: TextStyle) {
    Small(28f, 13f, TextStyle.LabelStrong),
    Regular(36f, 15f, TextStyle.LabelStrong),
    Large(44f, 17f, TextStyle.BodyStrong),
    ;

    val scaled get() = (height * SCALE).dp
}

/** The sizes the app is drawn with, measured from Newton Mail, as the Apple apps have them. */
object Dims {
    /** A hairline: a rule, a border. */
    val hairline = 1.dp
    /** Between the faces of buttons side by side, padded half inside each button's hit area. */
    val buttonGap = 6.dp
    /** Between the faces of rows one above the other, kept like `buttonGap`. */
    val rowGap = 2.dp
    val fieldHeight = ControlSize.Regular.scaled
    val rowHeight = ControlSize.Regular.scaled
    val optionHeight = (56 * SCALE).dp
    val formWidth = 360.dp
    val chipHeight = ControlSize.Small.scaled
    /** A message's sender in a thread. */
    val messageAvatar = 34f
    val attachmentPreviewHeight = (96 * SCALE).dp
    /** A bar across the screen: the top bar, a sheet's header, a toolbar. */
    val barHeight = 52.dp
    val searchHeight = ControlSize.Large.scaled
    val popupWidth = 380.dp
    /** Room after the sidebar's last row, above its footer. */
    val sidebarEnd = 48.dp
    /** A thread row: sender, subject, snippet. */
    val rowHeightThread = 88.dp
    /** How far a pull from the top of the list goes to refresh. */
    val pullHeight = 80.dp
    val accountBarWidth = 3.dp
    val selectCircle = 18f
    val selectCircleLine = 1.5.dp
    /** The widest the thread's card and a sheet's content grow on a large screen. */
    val cardWidth = 1000.dp
    val cardInset = Space.l
    val cardRadius = Radius.xl
    val sidebarWidth = 300.dp
    val floatingButton = 56.dp
    val floatingButtonSmall = 40.dp
    /** A screen at least this wide is laid out as a tablet's: the settings beside their sections. */
    val wide = 700.dp

    /**
     * The card that fills the page: the thread's messages, a message being written. It floats on
     * the page, `cardLarger` outlined in `border`, with `borderCard` hairlines inside; what rests
     * on it (a chip) fills with `cardLargerAccent` and hovers in `cardLargerAccentStronger`.
     */
    val cardFill = Tokens.cardLarger
    val cardRule = Tokens.borderCard
    val cardAccent = Tokens.cardLargerAccent
    val cardAccentStronger = Tokens.cardLargerAccentStronger

    val accountColors = listOf(
        Tokens.account1, Tokens.account2, Tokens.account3, Tokens.account4, Tokens.account5,
        Tokens.account6, Tokens.account7, Tokens.account8, Tokens.account9, Tokens.account10,
    )

    /** The names the core knows the colours by, `account-1` to `account-10`. */
    val accountColorNames = accountColors.indices.map { "account-${it + 1}" }

    /** An account's colour by the token name the core gives it. */
    fun accountColor(name: String): ThemeColor {
        val number = name.removePrefix("account-").toIntOrNull() ?: return accountColors[0]
        return accountColors.getOrElse(number - 1) { accountColors[0] }
    }

    /** Which of the account colours an address gets, as the Apple apps' `SenderAvatar` picks it. */
    fun colorIndex(email: String, count: Int): Int {
        var sum = 0
        for (point in email.codePoints().toArray()) sum = (sum * 31 + point) and 0xffff
        return sum % count
    }
}
