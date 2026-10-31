package app.wonnet.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.wonnet.core.Address
import app.wonnet.core.ShapeLine
import app.wonnet.theme.ControlSize
import app.wonnet.theme.Dims
import app.wonnet.theme.Space
import app.wonnet.theme.Symbol
import app.wonnet.theme.Tokens
import app.wonnet.ui.ActionButton
import app.wonnet.ui.AppMark
import app.wonnet.ui.AttachmentCard
import app.wonnet.ui.Avatar
import app.wonnet.ui.Banner
import app.wonnet.ui.BodyPlaceholder
import app.wonnet.ui.ButtonVariant
import app.wonnet.ui.Card
import app.wonnet.ui.Chip
import app.wonnet.ui.ChoiceMenu
import app.wonnet.ui.ChoiceRow
import app.wonnet.ui.EmptyState
import app.wonnet.ui.FloatingButton
import app.wonnet.ui.FoldedCount
import app.wonnet.ui.FormSection
import app.wonnet.ui.GlassBar
import app.wonnet.ui.GlassCircle
import app.wonnet.ui.GoogleMark
import app.wonnet.ui.IconButton
import app.wonnet.ui.InputField
import app.wonnet.ui.ItemRow
import app.wonnet.ui.NavRow
import app.wonnet.ui.Notice
import app.wonnet.ui.OptionButton
import app.wonnet.ui.PageCard
import app.wonnet.ui.SearchField
import app.wonnet.ui.SearchStyle
import app.wonnet.ui.SelectCircle
import app.wonnet.ui.Sheet
import app.wonnet.ui.Spinner
import app.wonnet.ui.StarButton
import app.wonnet.ui.SwatchRow
import app.wonnet.ui.TabStrip
import app.wonnet.ui.ToggleRow

/** Every component in its states, for working on the design: the palette's "Component gallery". */
@Composable
fun Gallery(close: () -> Unit) {
    var text by remember { mutableStateOf("") }
    var on by remember { mutableStateOf(true) }
    var starred by remember { mutableStateOf(true) }
    var checked by remember { mutableStateOf(true) }
    var choice by remember { mutableStateOf("System") }
    var swatch by remember { mutableStateOf("account-7") }
    var tab by remember { mutableStateOf("Important") }
    Sheet("Component gallery", close) {
        Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(Space.l), verticalArrangement = Arrangement.spacedBy(Space.xxl)) {
            FormSection("Buttons") {
                FlowRow(verticalArrangement = Arrangement.spacedBy(Space.s)) {
                    for (variant in ButtonVariant.entries) ActionButton(variant.name, variant = variant) {}
                    ActionButton("Pending", symbol = Symbol.Send, variant = ButtonVariant.Primary, pending = true) {}
                    ActionButton("Disabled", enabled = false) {}
                    ActionButton("Icon", symbol = Symbol.Archive, iconOnly = true) {}
                    for (size in ControlSize.entries) ActionButton(size.name, symbol = Symbol.Reply, size = size) {}
                }
                Row {
                    IconButton(Symbol.Archive, "Circled") {}
                    IconButton(Symbol.Trash, "Plain", circled = false) {}
                    IconButton(Symbol.AlarmClock, "Active", active = true) {}
                    IconButton(Symbol.Sun, "Quiet", quiet = true) {}
                    IconButton(Symbol.Reply, "Pending", pending = true) {}
                    StarButton(starred) { starred = !starred }
                    SelectCircle(checked, "Select") { checked = !checked }
                }
                Row(horizontalArrangement = Arrangement.spacedBy(Space.s)) {
                    FloatingButton(Symbol.SquarePen, "Compose") {}
                    FloatingButton(Symbol.ArrowUp, "Back to Top", small = true) {}
                }
                Row(horizontalArrangement = Arrangement.spacedBy(Space.s)) {
                    GlassBar(Modifier.weight(1f)) {
                        IconButton(Symbol.Archive, "Archive", circled = false, strong = true) {}
                        IconButton(Symbol.Trash, "Trash", circled = false, strong = true) {}
                        IconButton(Symbol.Clock, "Snooze", circled = false, strong = true) {}
                    }
                    GlassCircle(Symbol.Reply, "Reply") {}
                }
                ChoiceMenu(listOf("System", "Light", "Dark"), choice, { it }) { choice = it }
            }
            FormSection("Fields") {
                InputField(text, { text = it }, label = "Label", placeholder = "Placeholder")
                SearchField("Search", text, { text = it })
                SearchField("Search", text, { text = it }, style = SearchStyle.Bar)
                ToggleRow("A switch", on, detail = "With a line about it") { on = it }
            }
            FormSection("Chips") {
                FlowRow(horizontalArrangement = Arrangement.spacedBy(Space.xs), verticalArrangement = Arrangement.spacedBy(Space.xs)) {
                    Chip("ann@example.com", remove = {})
                    Chip("not an address", invalid = true, remove = {})
                    Chip("Travel", symbol = Symbol.Tag)
                    Chip("report.pdf", symbol = Symbol.Paperclip, detail = "1.2 MB", pending = true)
                    Chip("•••", onClick = {})
                }
            }
            FormSection("Rows") {
                Card {
                    NavRow("Inbox", symbol = Symbol.Inbox, count = 4, selected = true)
                    NavRow("Work", dot = Dims.accountColor("account-5").color, onClick = {})
                    ItemRow("An item", detail = "With a detail", symbol = Symbol.Tag)
                    ChoiceRow("Tomorrow", detail = "Tue 8:00 AM", highlighted = true) {}
                    ChoiceRow("Ann Lee", detail = "ann@example.com", avatar = Address("Ann Lee", "ann@example.com"), checked = true) {}
                }
                TabStrip(listOf("Important", "Other"), { it }, tab, { it }, { if (it == "Other") 3 else 0 }) { tab = it }
                OptionButton("Continue with Google", "Gmail and Google Workspace", {}) { GoogleMark() }
                SwatchRow(swatch) { swatch = it }
            }
            FormSection("Faces") {
                Row(horizontalArrangement = Arrangement.spacedBy(Space.m)) {
                    AppMark(40f)
                    Avatar("AL", "ann@example.com", 34f)
                    Spinner(16f)
                }
                Notice("A muted line about something.")
                Notice("Something went wrong.", error = true, symbol = Symbol.CircleAlert, action = "Retry")
                Banner(Symbol.CircleArrowDown, "v1.2.0 is out", "You have v1.1.0.", tint = Tokens.primary, action = { ActionButton("Update", variant = ButtonVariant.Primary, size = ControlSize.Small) {} }, close = {})
                Banner(Symbol.CircleArrowDown, "Downloading v1.2.0", "40%", tint = Tokens.process, progress = 0.4)
            }
            FormSection("On the card") {
                PageCard {
                    Column(Modifier.padding(Space.l), verticalArrangement = Arrangement.spacedBy(Space.m)) {
                        FoldedCount(5) {}
                        BodyPlaceholder(listOf(ShapeLine.Text(120), ShapeLine.Text(0), ShapeLine.Image(300, 80), ShapeLine.Text(60)))
                        Row(horizontalArrangement = Arrangement.spacedBy(Space.s)) {
                            AttachmentCard("report.pdf", "application/pdf", 1_200_000, null, pending = false, modifier = Modifier.width(168.dp)) {}
                            AttachmentCard("photo.jpg", "image/jpeg", 2_400_000, null, pending = true, modifier = Modifier.width(168.dp)) {}
                        }
                    }
                }
                EmptyState(Symbol.Inbox, "All done. Enjoy the quiet.", Modifier.padding(vertical = Space.l))
            }
        }
    }
}
