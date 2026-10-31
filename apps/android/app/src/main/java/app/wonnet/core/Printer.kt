package app.wonnet.core

import android.annotation.SuppressLint
import android.print.PrintAttributes
import android.print.PrintManager
import android.webkit.WebView
import android.webkit.WebViewClient
import app.wonnet.App

/** Prints a page through the system's print dialog. */
@SuppressLint("StaticFieldLeak")
object Printer {
    /** The page being laid out for printing, kept until it is handed over, then let go. */
    private var printing: WebView? = null

    fun print(html: String) {
        val activity = App.activity ?: return
        val view = WebView(activity)
        view.settings.javaScriptEnabled = false
        view.webViewClient = object : WebViewClient() {
            override fun onPageFinished(page: WebView, url: String?) {
                val manager = activity.getSystemService(PrintManager::class.java) ?: return
                manager.print("Mail", page.createPrintDocumentAdapter("Mail"), PrintAttributes.Builder().build())
                printing = null
            }
        }
        printing = view
        view.loadDataWithBaseURL(null, html, "text/html", "utf-8", null)
    }
}
