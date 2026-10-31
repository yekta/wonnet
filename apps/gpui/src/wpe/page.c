#include <wpe/webkit.h>
#include <wpe/wpe-platform.h>

typedef struct {
    void (*frame)(gpointer sink, const guint8 *pixels, guint stride, int width, int height,
                  const WPERectangle *damage, guint damaged);
    void (*said)(gpointer sink, const char *message);
    void (*open)(gpointer sink, const char *uri);
    void (*cursor)(gpointer sink, const char *name);
    void (*answer)(gpointer sink, guint64 question, const char *value);
    void (*failed)(gpointer sink);
} WonnetCallbacks;

typedef enum {
    WONNET_POINTER_MOVE,
    WONNET_POINTER_DOWN,
    WONNET_POINTER_UP,
    WONNET_POINTER_LEAVE,
} WonnetPointer;

typedef struct {
    WebKitWebView *web_view;
    gpointer sink;
} WonnetPage;

static const WonnetCallbacks *callbacks;
static WPEDisplay *display;
static WebKitWebContext *context;
static WebKitNetworkSession *session;
static GPtrArray *pages;

#define WONNET_TYPE_VIEW (wonnet_view_get_type())
G_DECLARE_FINAL_TYPE(WonnetView, wonnet_view, WONNET, VIEW, WPEView)

struct _WonnetView {
    WPEView parent;
    gpointer sink;
    WPEBuffer *pending;
    WPEBuffer *committed;
    GSource *tick;
    gint64 last_frame;
};

G_DEFINE_FINAL_TYPE(WonnetView, wonnet_view, WPE_TYPE_VIEW)

static gboolean wonnet_view_present(gpointer data)
{
    WonnetView *self = data;
    g_source_set_ready_time(self->tick, -1);
    if (!self->pending)
        return G_SOURCE_CONTINUE;
    if (self->committed) {
        wpe_view_buffer_released(WPE_VIEW(self), self->committed);
        g_object_unref(self->committed);
    }
    self->committed = g_steal_pointer(&self->pending);
    wpe_view_buffer_rendered(WPE_VIEW(self), self->committed);
    return G_SOURCE_CONTINUE;
}

static gboolean wonnet_view_tick_dispatch(GSource *source, GSourceFunc callback, gpointer data)
{
    return callback(data);
}

static GSourceFuncs tick_funcs = { .dispatch = wonnet_view_tick_dispatch };

static gboolean wonnet_view_render_buffer(WPEView *view, WPEBuffer *buffer, const WPERectangle *damage,
                                          guint damaged, GError **error)
{
    WonnetView *self = WONNET_VIEW(view);
    if (!WPE_IS_BUFFER_SHM(buffer)) {
        g_set_error_literal(error, WPE_VIEW_ERROR, WPE_VIEW_ERROR_RENDER_FAILED, "Only shared memory buffers are drawn");
        return FALSE;
    }
    if (self->sink) {
        WPEBufferSHM *shm = WPE_BUFFER_SHM(buffer);
        GBytes *bytes = wpe_buffer_shm_get_data(shm);
        callbacks->frame(self->sink, g_bytes_get_data(bytes, NULL), wpe_buffer_shm_get_stride(shm),
                         wpe_buffer_get_width(buffer), wpe_buffer_get_height(buffer), damage, damaged);
    }
    g_set_object(&self->pending, buffer);
    gint64 now = g_get_monotonic_time();
    gint64 next = self->last_frame + G_USEC_PER_SEC / 60;
    self->last_frame = MAX(now, next);
    g_source_set_ready_time(self->tick, next <= now ? 0 : next);
    return TRUE;
}

static void wonnet_view_set_cursor_from_name(WPEView *view, const char *name)
{
    WonnetView *self = WONNET_VIEW(view);
    if (self->sink)
        callbacks->cursor(self->sink, name);
}

static void wonnet_view_toplevel_changed(WPEView *view, GParamSpec *spec, gpointer data)
{
    WPEToplevel *toplevel = wpe_view_get_toplevel(view);
    if (!toplevel) {
        wpe_view_unmap(view);
        return;
    }
    int width, height;
    wpe_toplevel_get_size(toplevel, &width, &height);
    if (width && height)
        wpe_view_resized(view, width, height);
    wpe_view_map(view);
}

static void wonnet_view_constructed(GObject *object)
{
    G_OBJECT_CLASS(wonnet_view_parent_class)->constructed(object);
    WonnetView *self = WONNET_VIEW(object);
    g_signal_connect(object, "notify::toplevel", G_CALLBACK(wonnet_view_toplevel_changed), NULL);
    self->tick = g_source_new(&tick_funcs, sizeof(GSource));
    g_source_set_callback(self->tick, wonnet_view_present, self, NULL);
    g_source_set_ready_time(self->tick, -1);
    g_source_attach(self->tick, NULL);
}

static void wonnet_view_dispose(GObject *object)
{
    WonnetView *self = WONNET_VIEW(object);
    if (self->tick) {
        g_source_destroy(self->tick);
        g_clear_pointer(&self->tick, g_source_unref);
    }
    g_clear_object(&self->pending);
    g_clear_object(&self->committed);
    G_OBJECT_CLASS(wonnet_view_parent_class)->dispose(object);
}

static void wonnet_view_class_init(WonnetViewClass *klass)
{
    GObjectClass *object_class = G_OBJECT_CLASS(klass);
    object_class->constructed = wonnet_view_constructed;
    object_class->dispose = wonnet_view_dispose;
    WPEViewClass *view_class = WPE_VIEW_CLASS(klass);
    view_class->render_buffer = wonnet_view_render_buffer;
    view_class->set_cursor_from_name = wonnet_view_set_cursor_from_name;
}

static void wonnet_view_init(WonnetView *self)
{
}

#define WONNET_TYPE_TOPLEVEL (wonnet_toplevel_get_type())
G_DECLARE_FINAL_TYPE(WonnetToplevel, wonnet_toplevel, WONNET, TOPLEVEL, WPEToplevel)

struct _WonnetToplevel {
    WPEToplevel parent;
};

G_DEFINE_FINAL_TYPE(WonnetToplevel, wonnet_toplevel, WPE_TYPE_TOPLEVEL)

static gboolean wonnet_toplevel_resize_view(WPEToplevel *toplevel, WPEView *view, gpointer data)
{
    int width, height;
    wpe_toplevel_get_size(toplevel, &width, &height);
    wpe_view_resized(view, width, height);
    return FALSE;
}

static gboolean wonnet_toplevel_resize(WPEToplevel *toplevel, int width, int height)
{
    wpe_toplevel_resized(toplevel, width, height);
    wpe_toplevel_foreach_view(toplevel, wonnet_toplevel_resize_view, NULL);
    return TRUE;
}

static void wonnet_toplevel_constructed(GObject *object)
{
    G_OBJECT_CLASS(wonnet_toplevel_parent_class)->constructed(object);
    wpe_toplevel_state_changed(WPE_TOPLEVEL(object), WPE_TOPLEVEL_STATE_ACTIVE);
}

static void wonnet_toplevel_class_init(WonnetToplevelClass *klass)
{
    G_OBJECT_CLASS(klass)->constructed = wonnet_toplevel_constructed;
    WPE_TOPLEVEL_CLASS(klass)->resize = wonnet_toplevel_resize;
}

static void wonnet_toplevel_init(WonnetToplevel *self)
{
}

#define WONNET_TYPE_DISPLAY (wonnet_display_get_type())
G_DECLARE_FINAL_TYPE(WonnetDisplay, wonnet_display, WONNET, DISPLAY, WPEDisplay)

struct _WonnetDisplay {
    WPEDisplay parent;
};

G_DEFINE_FINAL_TYPE(WonnetDisplay, wonnet_display, WPE_TYPE_DISPLAY)

static gboolean wonnet_display_connect(WPEDisplay *display, GError **error)
{
    return TRUE;
}

static WPEView *wonnet_display_create_view(WPEDisplay *display)
{
    return g_object_new(WONNET_TYPE_VIEW, "display", display, NULL);
}

static WPEToplevel *wonnet_display_create_toplevel(WPEDisplay *display, guint max_views)
{
    return g_object_new(WONNET_TYPE_TOPLEVEL, "display", display, NULL);
}

static void wonnet_display_class_init(WonnetDisplayClass *klass)
{
    WPEDisplayClass *display_class = WPE_DISPLAY_CLASS(klass);
    display_class->connect = wonnet_display_connect;
    display_class->create_view = wonnet_display_create_view;
    display_class->create_toplevel = wonnet_display_create_toplevel;
}

static void wonnet_display_init(WonnetDisplay *self)
{
}

void wonnet_wpe_run(const WonnetCallbacks *given)
{
    callbacks = given;
    GError *error = NULL;
    display = g_object_new(WONNET_TYPE_DISPLAY, NULL);
    if (!wpe_display_connect(display, &error)) {
        g_warning("The page display didn't start: %s", error->message);
        g_error_free(error);
    }
    wpe_display_set_primary(display);
    context = webkit_web_context_new();
    webkit_web_context_set_cache_model(context, WEBKIT_CACHE_MODEL_DOCUMENT_BROWSER);
    session = webkit_network_session_new_ephemeral();
    pages = g_ptr_array_new();
    g_main_loop_run(g_main_loop_new(NULL, FALSE));
}

static gboolean wonnet_page_decide(WebKitWebView *web_view, WebKitPolicyDecision *decision,
                                   WebKitPolicyDecisionType type, WonnetPage *page)
{
    if (type == WEBKIT_POLICY_DECISION_TYPE_RESPONSE)
        return FALSE;
    WebKitNavigationAction *action =
        webkit_navigation_policy_decision_get_navigation_action(WEBKIT_NAVIGATION_POLICY_DECISION(decision));
    const char *uri = webkit_uri_request_get_uri(webkit_navigation_action_get_request(action));
    gboolean inside = g_str_has_prefix(uri, "about:") || g_str_has_prefix(uri, "data:");
    if (type == WEBKIT_POLICY_DECISION_TYPE_NAVIGATION_ACTION && inside)
        return FALSE;
    if (webkit_navigation_action_get_navigation_type(action) == WEBKIT_NAVIGATION_TYPE_LINK_CLICKED && !inside)
        callbacks->open(page->sink, uri);
    webkit_policy_decision_ignore(decision);
    return TRUE;
}

static gboolean wonnet_page_context_menu(WebKitWebView *web_view, WebKitContextMenu *menu,
                                         WebKitHitTestResult *hit, WonnetPage *page)
{
    return TRUE;
}

static void wonnet_page_terminated(WebKitWebView *web_view, WebKitWebProcessTerminationReason reason, WonnetPage *page)
{
    callbacks->failed(page->sink);
}

static void wonnet_page_message(WebKitUserContentManager *manager, JSCValue *value, WonnetPage *page)
{
    char *message = jsc_value_to_string(value);
    callbacks->said(page->sink, message);
    g_free(message);
}

WonnetPage *wonnet_page_new(gpointer sink, const char *script, int width, int height, double scale)
{
    WonnetPage *page = g_new0(WonnetPage, 1);
    page->sink = sink;

    WebKitUserContentManager *content = webkit_user_content_manager_new();
    WebKitUserScript *user_script = webkit_user_script_new(script, WEBKIT_USER_CONTENT_INJECT_TOP_FRAME,
                                                           WEBKIT_USER_SCRIPT_INJECT_AT_DOCUMENT_START, NULL, NULL);
    webkit_user_content_manager_add_script(content, user_script);
    webkit_user_script_unref(user_script);
    webkit_user_content_manager_register_script_message_handler(content, "ipc", NULL);
    g_signal_connect(content, "script-message-received::ipc", G_CALLBACK(wonnet_page_message), page);

    WebKitSettings *settings = webkit_settings_new_with_settings(
        "enable-javascript-markup", FALSE,
        "enable-webgl", FALSE,
        "enable-media", FALSE,
        NULL);
    WebKitFeatureList *features = webkit_settings_get_all_features();
    for (gsize index = 0; index < webkit_feature_list_get_length(features); index++) {
        WebKitFeature *feature = webkit_feature_list_get(features, index);
        if (g_str_equal(webkit_feature_get_identifier(feature), "HardwareAccelerationEnabled"))
            webkit_settings_set_feature_enabled(settings, feature, FALSE);
    }
    webkit_feature_list_unref(features);

    WebKitWebView *related = pages->len ? g_ptr_array_index(pages, 0) : NULL;
    page->web_view = g_object_new(WEBKIT_TYPE_WEB_VIEW,
        "display", display,
        "web-context", context,
        "network-session", session,
        "settings", settings,
        "user-content-manager", content,
        related ? "related-view" : NULL, related,
        NULL);
    g_object_unref(settings);
    g_object_unref(content);
    g_ptr_array_add(pages, page->web_view);
    g_object_set_data(G_OBJECT(page->web_view), "wonnet-page", page);

    WPEView *view = webkit_web_view_get_wpe_view(page->web_view);
    WONNET_VIEW(view)->sink = sink;
    WPEToplevel *toplevel = wpe_view_get_toplevel(view);
    wpe_toplevel_scale_changed(toplevel, scale);
    wpe_toplevel_resize(toplevel, width, height);
    wpe_view_focus_in(view);

    g_signal_connect(page->web_view, "decide-policy", G_CALLBACK(wonnet_page_decide), page);
    g_signal_connect(page->web_view, "context-menu", G_CALLBACK(wonnet_page_context_menu), page);
    g_signal_connect(page->web_view, "web-process-terminated", G_CALLBACK(wonnet_page_terminated), page);
    return page;
}

void wonnet_page_free(WonnetPage *page)
{
    WebKitUserContentManager *content = webkit_web_view_get_user_content_manager(page->web_view);
    g_signal_handlers_disconnect_by_data(content, page);
    g_signal_handlers_disconnect_by_data(page->web_view, page);
    WONNET_VIEW(webkit_web_view_get_wpe_view(page->web_view))->sink = NULL;
    g_object_set_data(G_OBJECT(page->web_view), "wonnet-page", NULL);
    g_ptr_array_remove(pages, page->web_view);
    g_object_unref(page->web_view);
    g_free(page);
}

void wonnet_page_load(WonnetPage *page, const char *html, double red, double green, double blue)
{
    WebKitColor background = { red, green, blue, 1 };
    webkit_web_view_set_background_color(page->web_view, &background);
    webkit_web_view_load_html(page->web_view, html, NULL);
}

void wonnet_page_resize(WonnetPage *page, int width, int height, double scale)
{
    WPEToplevel *toplevel = wpe_view_get_toplevel(webkit_web_view_get_wpe_view(page->web_view));
    if (wpe_toplevel_get_scale(toplevel) != scale)
        wpe_toplevel_scale_changed(toplevel, scale);
    wpe_toplevel_resize(toplevel, width, height);
}

typedef struct {
    WebKitWebView *web_view;
    guint64 question;
} WonnetQuestion;

static void wonnet_page_answered(GObject *object, GAsyncResult *result, gpointer data)
{
    WonnetQuestion *question = data;
    JSCValue *value = webkit_web_view_evaluate_javascript_finish(question->web_view, result, NULL);
    WonnetPage *page = g_object_get_data(G_OBJECT(question->web_view), "wonnet-page");
    if (page && value) {
        char *text = jsc_value_to_string(value);
        callbacks->answer(page->sink, question->question, text);
        g_free(text);
    }
    g_clear_object(&value);
    g_object_unref(question->web_view);
    g_free(question);
}

void wonnet_page_run(WonnetPage *page, const char *script, guint64 question)
{
    if (!question) {
        webkit_web_view_evaluate_javascript(page->web_view, script, -1, NULL, NULL, NULL, NULL, NULL);
        return;
    }
    WonnetQuestion *asked = g_new0(WonnetQuestion, 1);
    asked->web_view = g_object_ref(page->web_view);
    asked->question = question;
    webkit_web_view_evaluate_javascript(page->web_view, script, -1, NULL, NULL, NULL, wonnet_page_answered, asked);
}

void wonnet_page_pointer(WonnetPage *page, WonnetPointer kind, guint button, double x, double y,
                         WPEModifiers modifiers, guint count)
{
    WPEView *view = webkit_web_view_get_wpe_view(page->web_view);
    guint32 time = g_get_monotonic_time() / 1000;
    WPEEvent *event = NULL;
    switch (kind) {
    case WONNET_POINTER_MOVE:
        event = wpe_event_pointer_move_new(WPE_EVENT_POINTER_MOVE, view, WPE_INPUT_SOURCE_MOUSE, time, modifiers, x, y, 0, 0);
        break;
    case WONNET_POINTER_DOWN:
        event = wpe_event_pointer_button_new(WPE_EVENT_POINTER_DOWN, view, WPE_INPUT_SOURCE_MOUSE, time, modifiers, button, x, y, count);
        break;
    case WONNET_POINTER_UP:
        event = wpe_event_pointer_button_new(WPE_EVENT_POINTER_UP, view, WPE_INPUT_SOURCE_MOUSE, time, modifiers, button, x, y, 0);
        break;
    case WONNET_POINTER_LEAVE:
        event = wpe_event_pointer_move_new(WPE_EVENT_POINTER_LEAVE, view, WPE_INPUT_SOURCE_MOUSE, time, modifiers, x, y, 0, 0);
        break;
    }
    wpe_view_event(view, event);
    wpe_event_unref(event);
}
