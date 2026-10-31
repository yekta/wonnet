package app.wonnet.screens

import android.annotation.SuppressLint
import android.content.Context
import android.os.Handler
import android.os.Looper
import android.view.ViewGroup
import android.webkit.JavascriptInterface
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import app.wonnet.core.MailStore
import app.wonnet.core.Platform
import app.wonnet.core.ShapeLine
import app.wonnet.theme.LocalDark
import app.wonnet.ui.BodyPlaceholder
import kotlin.math.abs

/** Measures the page, fits a page wider than the view to it, and says how tall it is. */
private const val MEASURE = """
(function () {
  function fit(body) {
    body.style.width = '';
    body.style.transform = '';
    var wide = document.documentElement.scrollWidth, shown = document.documentElement.clientWidth;
    if (wide <= shown + 1) { return; }
    body.style.width = wide + 'px';
    body.style.transformOrigin = '0 0';
    body.style.transform = 'scale(' + shown / wide + ')';
  }
  // Until its images are in, a page is shorter than it will be; a folded quote is not.
  function report(event) {
    var body = document.body;
    if (body) { fit(body); }
    var height = Math.ceil(body ? body.getBoundingClientRect().height : 0);
    var settled = document.readyState === 'complete' || (event && event.type === 'toggle');
    Wonnet.height(height, !!settled);
  }
  if (document.body) { new ResizeObserver(report).observe(document.body); }
  document.addEventListener('toggle', report, true);
  window.addEventListener('load', report);
  window.addEventListener('resize', report);
  report();
})();
"""

/**
 * A message body, as tall as its page. It opens at the height the store kept for it, when it was
 * last measured at this width, so the thread is laid out before the page is; else its placeholder
 * stands in, about as tall, until the page says how tall it is. It only grows until the page is
 * settled, and only a settled height is kept. Links open in the browser; the page never
 * navigates. `original` shows a designed mail as its sender made it.
 */
@Composable
fun MessageWebView(store: MailStore, message: String, html: String, shape: List<ShapeLine>, original: Boolean) {
    val dark = LocalDark.current
    BoxWithConstraints(Modifier.fillMaxWidth()) {
        val width = maxWidth.value.toDouble()
        var height by remember(message) { mutableStateOf(store.bodyHeight(message, width)) }
        val page = remember(html, dark) { shown(html, dark) }
        Box(Modifier.fillMaxWidth().let { modifier -> height?.let { modifier.height(it.dp) } ?: modifier }) {
            if (height == null) BodyPlaceholder(shape)
            AndroidView(
                factory = { context -> WebViewPool.take(context, page) },
                modifier = Modifier.fillMaxWidth().let { modifier -> height?.let { modifier.height(it.dp) } ?: modifier.height(1.dp) }.alpha(if (height == null) 0f else 1f),
                update = { view ->
                    view.onHeight = onHeight@{ measured, settled ->
                        if (settled) store.noteBodyHeight(message, width, measured)
                        if (!settled && measured <= (height ?: 0.0)) return@onHeight
                        if (height?.let { abs(it - measured) > 0.5 } != false) height = maxOf(measured, 20.0)
                    }
                    view.show(page, original)
                },
                onRelease = WebViewPool::give,
            )
        }
    }
}

/**
 * The page as this app shows it: in the app's scheme, not only the system's, and with its viewport
 * kept to the view's width, which WebView would otherwise widen to a wide page's.
 */
fun shown(html: String, dark: Boolean): String =
    html.replace("@media (prefers-color-scheme: dark)", if (dark) "@media all" else "@media not all")
        .replace("initial-scale=1", "initial-scale=1, minimum-scale=1, maximum-scale=1")

/**
 * The web views that draw message bodies, kept and reused: making one is the slow part. Pages
 * can be loaded ahead into spare ones, so they are parsed by the time their cards take them.
 */
@SuppressLint("StaticFieldLeak")
object WebViewPool {
    private const val MOST = 6
    private val spare = ArrayDeque<MessagePage>()

    /** Starts WebView before the first message shows, so the first card isn't the one that waits. */
    fun warm(context: Context) {
        if (spare.isEmpty()) spare.addLast(MessagePage(context.applicationContext))
    }

    /** Loads these pages into spare views ahead of their cards. */
    fun preload(context: Context, pages: List<String>) {
        for (page in pages.take(MOST)) {
            if (spare.any { it.loaded == page }) continue
            val view = spare.firstOrNull { it.loaded == null }?.also { spare.remove(it) } ?: MessagePage(context.applicationContext)
            view.show(page, false)
            spare.addLast(view)
        }
        while (spare.size > MOST) spare.removeFirst().destroy()
    }

    /** A spare view: the one already showing `page` when there is one, else any, else a new one. */
    fun take(context: Context, page: String): MessagePage {
        val view = spare.firstOrNull { it.loaded == page } ?: spare.lastOrNull()
        if (view == null) return MessagePage(context.applicationContext)
        spare.remove(view)
        return view
    }

    fun give(view: MessagePage) {
        view.onHeight = null
        if (spare.size >= MOST) return view.destroy()
        spare.addLast(view)
    }
}

/** A web view that shows one message's page, tells its height and opens its links elsewhere. */
@SuppressLint("SetJavaScriptEnabled", "ViewConstructor")
class MessagePage(context: Context) : WebView(context) {
    /** The page's height, and whether it is settled: its images in, or a quote folded. */
    var onHeight: ((Double, Boolean) -> Unit)? = null
    var loaded: String? = null
        private set
    private var original = false
    private val main = Handler(Looper.getMainLooper())

    init {
        layoutParams = ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT)
        setBackgroundColor(android.graphics.Color.TRANSPARENT)
        isVerticalScrollBarEnabled = false
        isHorizontalScrollBarEnabled = false
        overScrollMode = OVER_SCROLL_NEVER
        // Only the app's own measuring runs: the page's Content-Security-Policy blocks the mail's
        // scripts, and the script that fits and measures it is the app's, given by evaluateJavascript.
        settings.javaScriptEnabled = true
        settings.allowFileAccess = false
        settings.setSupportZoom(false)
        settings.builtInZoomControls = false
        // The page's viewport is the view's width, so a wider page is fitted to it rather than widening it.
        settings.useWideViewPort = true
        settings.loadWithOverviewMode = false
        addJavascriptInterface(Bridge(), "Wonnet")
        webViewClient = object : WebViewClient() {
            /** The page never navigates: a link the user taps opens in the browser, anything else is ignored. */
            override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                if (request.isForMainFrame && request.hasGesture()) Platform.open(context, request.url.toString())
                return true
            }

            override fun onPageFinished(view: WebView, url: String?) {
                evaluateJavascript(MEASURE, null)
                if (original) applyOriginal()
            }
        }
    }

    /** All the page can reach of the app: where it says how tall it is. */
    inner class Bridge {
        @JavascriptInterface
        fun height(value: Double, settled: Boolean) {
            main.post { onHeight?.invoke(value, settled) }
        }
    }

    fun show(html: String, original: Boolean) {
        val changed = this.original != original
        this.original = original
        if (loaded != html) {
            loaded = html
            loadDataWithBaseURL(null, html, "text/html", "utf-8", null)
            return
        }
        if (changed) applyOriginal()
    }

    /** The `dark` class the core put on the page's body taken off, and a paper card kept light. */
    private fun applyOriginal() {
        evaluateJavascript(
            "(() => { const body = document.body; if (!body) return; body.classList.toggle('dark', ${!original});" +
                " if (body.classList.contains('paper')) body.classList.toggle('light', $original); })();",
            null,
        )
    }
}
