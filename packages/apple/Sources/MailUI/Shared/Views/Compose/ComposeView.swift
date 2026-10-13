import SwiftUI
import UniformTypeIdentifiers

/// Writing a message: who it goes to and from, its subject, its text with the signature and the
/// quote under it, and its files. It is kept as a draft as it is written. The bar on top has
/// Send alone; everything else about the message is in the toolbar under it. Sending waits for
/// the undo delay, or for a time picked to send it at.
struct ComposeView: View {
    @Environment(MailStore.self) private var store
    @State private var compose: Compose
    /// Typed into the address fields and not yet made a chip.
    @State private var to = ""
    @State private var cc = ""
    @State private var bcc = ""
    /// The draft as it was last kept, to know when there is something new to keep.
    @State private var kept: Draft
    @State private var edited = false
    @State private var closing = false
    @State private var showQuote = false
    @State private var importing = false
    @State private var timing: Timing?
    @State private var pickingSnippet = false
    @State private var pickingSender = false
    /// The address line a shortcut opened, to put the cursor in it.
    @State private var revealed: String?
    /// The word after a `;` at the caret, while there is one.
    @State private var trigger: String?
    @State private var triggerIndex = 0
    @State private var warning: Warning?
    @State private var discarding = false
    @State private var dropping = false
    /// A line about something that went wrong, under the title.
    @State private var notice: String?
    @State private var bodyHeight: CGFloat = 80
    @State private var editing = BodyEditing()

    fileprivate enum Timing: String, Identifiable {
        case sendLater, remind
        var id: String { rawValue }
    }

    /// Something to confirm before sending.
    fileprivate struct Warning: Identifiable {
        let id = UUID()
        let message: String
        let at: Date?
        let archive: Bool
    }

    init(compose: Compose) {
        _compose = State(initialValue: compose)
        _kept = State(initialValue: compose.draft)
        _to = State(initialValue: compose.typing?.to ?? "")
        _cc = State(initialValue: compose.typing?.cc ?? "")
        _bcc = State(initialValue: compose.typing?.bcc ?? "")
    }

    var body: some View {
        #if os(macOS)
        VStack(spacing: 0) {
            HeaderBar(title: title) {
                EmptyView()
            } trailing: {
                ActionButton(title: "Send", symbol: .send, variant: .primary) { send() }
                    .keyboardShortcut(.return, modifiers: .command)
                    .disabled(cannotSend)
            }
            .toolbar {
                if store.settings == nil {
                    ToolbarItem(placement: .navigation) {
                        IconButton(symbol: .arrowLeft, help: "Back (Esc)", action: close)
                            .keyboardShortcut(.cancelAction)
                    }
                    .withoutSystemGlass()
                }
            }
            Rule()
            GeometryReader { window in
                ScrollView {
                    card
                        .frame(maxWidth: Theme.cardWidth, minHeight: window.size.height - 2 * Space.l)
                        .background(RoundedRectangle(cornerRadius: Theme.cardRadius).fill(Tokens.card.color).shadow(.lg))
                        .padding(.vertical, Space.l)
                        .padding(.horizontal, Space.xl + 4)
                        .frame(maxWidth: .infinity)
                }
            }
            Rule()
            toolbar
        }
        .background(Tokens.background.color)
        .modifier(behaviour)
        #else
        Sheet(title: title, size: .large, close: "Cancel", background: Tokens.card) {
            VStack(spacing: 0) {
                ScrollView {
                    card
                }
                .scrollDismissesKeyboard(.interactively)
                Rule(color: Tokens.borderCard)
                toolbar
            }
            .modifier(behaviour)
        } trailing: {
            Button("Send") { send() }
                .buttonStyle(.borderedProminent)
                .keyboardShortcut(.return, modifiers: .command)
                .disabled(cannotSend)
        }
        #endif
    }

    /// The message: its head, then its text, with the whole of the card's rest for writing.
    private var card: some View {
        VStack(spacing: 0) {
            if let notice {
                Notice(text: notice, tone: .error, symbol: .circleAlert)
                    .padding(.horizontal, Theme.cardInset)
                    .padding(.vertical, Space.m)
                Rule(color: Tokens.borderCard).padding(.horizontal, Theme.cardInset)
            }
            fields
            message
        }
    }

    /// What happens around the page on both platforms: drops, the file picker, the sheets, the
    /// questions, the autosave and the keeping of the draft when the view goes.
    private var behaviour: some ViewModifier {
        Behaviour(
            dropping: $dropping, importing: $importing, timing: $timing, pickingSnippet: $pickingSnippet, pickingSender: $pickingSender,
            warning: $warning, discarding: $discarding, view: self
        )
    }

    private struct Behaviour: ViewModifier {
        @Binding var dropping: Bool
        @Binding var importing: Bool
        @Binding var timing: Timing?
        @Binding var pickingSnippet: Bool
        @Binding var pickingSender: Bool
        @Binding var warning: Warning?
        @Binding var discarding: Bool
        let view: ComposeView

        func body(content: Content) -> some View {
            content
                .overlay(alignment: .bottomLeading) { view.snippetMenu }
                .overlay {
                    if dropping {
                        RoundedRectangle(cornerRadius: Radius.sm)
                            .strokeBorder(Tokens.primary.color, style: StrokeStyle(lineWidth: 2, dash: [6, 4]))
                            .padding(Space.xs + 2)
                            .allowsHitTesting(false)
                    }
                }
                .background { view.shortcuts }
                .onDrop(of: [.item], isTargeted: $dropping) { providers in
                    Task { view.add(await Outgoing.stage(providers)) }
                    return true
                }
                .fileImporter(isPresented: $importing, allowedContentTypes: [.item], allowsMultipleSelection: true) { result in
                    guard case .success(let urls) = result else { return }
                    Task { view.add(await Outgoing.stage(urls.map(Incoming.file))) }
                }
                .sheet(item: $timing) { timing in
                    TimePicker(title: timing == .sendLater ? "Send later" : "Remind me if nobody replies") { choice in view.picked(choice, for: timing) }
                        .environment(view.store)
                }
                .sheet(isPresented: $pickingSnippet) {
                    SnippetPicker(snippets: Snippet.all(in: view.store.preferences)) { snippet in
                        pickingSnippet = false
                        view.put(snippet, replacingTrigger: false)
                    }
                }
                .sheet(isPresented: $pickingSender) {
                    SenderPicker(senders: view.senders, current: view.sender, pick: view.choose)
                }
                .alert("Send anyway?", isPresented: Binding(get: { warning != nil }, set: { if !$0 { warning = nil } }), presenting: warning) { warning in
                    Button("Send") { view.finish(at: warning.at, archive: warning.archive) }
                    Button("Keep writing", role: .cancel) {}
                } message: { warning in
                    Text(warning.message)
                }
                .alert("Discard this draft?", isPresented: $discarding) {
                    Button("Discard", role: .destructive, action: view.discard)
                    Button("Keep it", role: .cancel) {}
                }
                .task(id: view.live) {
                    view.store.noteCompose(view.live)
                    await view.autosave()
                }
                .task {
                    if !view.store.preferencesLoaded { await view.store.loadPreferences() }
                }
                .onAppear {
                    guard !view.compose.draft.to.isEmpty else { return }
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) { view.editing.focus() }
                }
                .onDisappear(perform: view.left)
        }
    }

    // MARK: Parts

    private var cannotSend: Bool {
        assembled.draft.to.isEmpty && assembled.draft.cc.isEmpty && assembled.draft.bcc.isEmpty
    }

    @ViewBuilder private var fields: some View {
        AddressField(label: "To", addresses: $compose.draft.to, text: $to, autofocus: compose.draft.to.isEmpty) {
            if !compose.showCc {
                ActionButton(title: "Cc Bcc", variant: .ghost, size: .small) { reveal("Cc") }
                    .padding(.trailing, -Theme.buttonGap / 2)
            }
        }
        if compose.showCc {
            AddressField(label: "Cc", addresses: $compose.draft.cc, text: $cc, autofocus: revealed == "Cc")
            AddressField(label: "Bcc", addresses: $compose.draft.bcc, text: $bcc, autofocus: revealed == "Bcc")
        }
        if senders.count > 1 {
            FieldRow(label: "From") {
                from
                Spacer(minLength: 0)
            }
        }
        FieldRow(label: "Subject") {
            TextField("", text: $compose.draft.subject).textFieldStyle(.plain)
                .onSubmit { editing.focus() }
        }
    }

    /// The address the message goes from, opening the choice of another. Pulled out by the
    /// button's own padding, so its words line up with the other fields'.
    private var from: some View {
        PlainButton(help: "Send as another address (⌘⇧F)") { pickingSender = true } label: {
            HStack(spacing: Space.s) {
                if let sender {
                    Circle().fill(sender.color).frame(width: 8, height: 8)
                    Text(sender.name).textStyle(.body).lineLimit(1)
                    if sender.identity.name != nil {
                        Text(sender.identity.email).textStyle(.caption).lineLimit(1).truncationMode(.middle)
                    }
                }
                Image(.chevronDown, size: 12).foregroundStyle(Tokens.mutedStrongerForeground.color)
            }
        }
        .padding(.leading, -Space.xs)
    }

    private var message: some View {
        VStack(alignment: .leading, spacing: Space.l) {
            BodyEditor(
                text: $compose.draft.text, height: $bodyHeight, editing: editing, menuOpen: !triggerMatches.isEmpty,
                onTrigger: { word in
                    trigger = word
                    triggerIndex = 0
                },
                onKey: triggerKey,
                onPaste: { incoming in Task { add(await Outgoing.stage(incoming)) } }
            )
            .frame(height: max(bodyHeight, 80))
            if let signature {
                Text(signature).textStyle(.body, color: Tokens.mutedStrongerForeground.color).textSelection(.enabled)
            }
            if let quote = compose.draft.quote, !quote.isEmpty {
                Chip(title: "•••", help: showQuote ? "Hide the quoted text" : "Show the quoted text", action: { showQuote.toggle() })
                if showQuote {
                    Text(quote)
                        .textStyle(.label, color: Tokens.mutedStrongerForeground.color)
                        .textSelection(.enabled)
                        .padding(.leading, Space.m)
                        .overlay(alignment: .leading) { Rectangle().fill(Tokens.borderCard.color).frame(width: 2) }
                }
            }
            if !compose.draft.attachments.isEmpty || compose.remindAt != nil {
                FlowLayout {
                    ForEach(Array(compose.draft.attachments.enumerated()), id: \.offset) { index, file in
                        Chip(
                            title: file.name, symbol: .paperclip, detail: ByteCountFormatter.string(fromByteCount: file.size, countStyle: .file),
                            remove: { compose.draft.attachments.remove(at: index) }
                        )
                    }
                    if let remind = compose.remindAt {
                        Chip(title: "Remind me \(remind.label) if nobody replies", symbol: .alarmClock, remove: { compose.remindAt = nil })
                    }
                }
            }
        }
        .padding(.horizontal, Theme.cardInset)
        .padding(.vertical, Space.l)
        // The rest of the card is for writing too: a click anywhere in it puts the caret there.
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .contentShape(Rectangle())
        .onTapGesture { editing.focus() }
    }

    /// Everything about the message but sending it, under the card: its files, a snippet, when
    /// to send it, a reminder, and the way to throw it away at the far end.
    private var toolbar: some View {
        HStack(spacing: 0) {
            IconButton(symbol: .paperclip, help: "Attach files (⌘⇧U)") { importing = true }
                .keyboardShortcut("u", modifiers: [.command, .shift])
            IconButton(symbol: .zap, help: "Snippets (⌘;), or type ; and a name") { pickingSnippet = true }
                .keyboardShortcut(";", modifiers: .command)
            IconButton(symbol: .calendarClock, help: "Send later (⌘⇧L)") { timing = .sendLater }
                .keyboardShortcut("l", modifiers: [.command, .shift])
            IconButton(symbol: .alarmClock, help: "Remind me if nobody replies (⌘⇧H)", active: compose.remindAt != nil) { timing = .remind }
                .keyboardShortcut("h", modifiers: [.command, .shift])
            if compose.thread != nil {
                IconButton(symbol: .archive, help: "Send and archive (⌘⇧↩)") { send(archive: true) }
                    .keyboardShortcut(.return, modifiers: [.command, .shift])
                    .disabled(cannotSend)
            }
            if compose.draft.inReplyTo != nil {
                IconButton(symbol: .handshake, help: "Instant Intro: move who introduced you to Bcc (⌘⇧I)", action: instantIntro)
                    .keyboardShortcut("i", modifiers: [.command, .shift])
            }
            Spacer()
            IconButton(symbol: .trash, help: "Discard the draft (⌘⇧,)") { discarding = true }
                .keyboardShortcut(",", modifiers: [.command, .shift])
        }
        .padding(.horizontal, Space.l)
        .frame(height: Theme.barHeight)
    }

    /// The shortcuts that have no button of their own: the ones that open a line of the head.
    fileprivate var shortcuts: some View {
        Group {
            Button("Close") { close() }.keyboardShortcut("w")
            Button("Cc") { reveal("Cc") }.keyboardShortcut("c", modifiers: [.command, .shift])
            Button("Bcc") { reveal("Bcc") }.keyboardShortcut("b", modifiers: [.command, .shift])
            if senders.count > 1 {
                Button("Send as") { pickingSender = true }.keyboardShortcut("f", modifiers: [.command, .shift])
            }
        }
        .opacity(0)
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }

    /// The snippets matching what follows a `;`, over the toolbar.
    @ViewBuilder fileprivate var snippetMenu: some View {
        let matches = triggerMatches
        if !matches.isEmpty {
            PopupCard {
                ForEach(Array(matches.enumerated()), id: \.element.id) { index, snippet in
                    ChoiceRow(title: snippet.name, detail: snippet.text.replacingOccurrences(of: "\n", with: " "), symbol: .zap, highlighted: index == triggerIndex) {
                        put(snippet, replacingTrigger: true)
                    }
                }
            }
            .frame(maxWidth: 380)
            .padding(.leading, Space.l)
            .padding(.bottom, Theme.barHeight + Space.xs)
        }
    }

    // MARK: State

    private var title: String {
        guard compose.draft.inReplyTo == nil else { return "Reply" }
        return compose.draft.forwardAttachmentsOf != nil || compose.draft.subject.hasPrefix("Fwd:") ? "Forward" : "New message"
    }

    /// The compose as it is, with what is typed in the address fields, for the store to keep.
    fileprivate var live: Compose {
        var current = compose
        current.typing = Compose.Typing(to: to, cc: cc, bcc: bcc)
        return current
    }

    /// The compose with what is typed in the address fields made addresses.
    fileprivate var assembled: Compose {
        var outgoing = compose
        outgoing.draft.to += Address.typed(to)
        outgoing.draft.cc += Address.typed(cc)
        outgoing.draft.bcc += Address.typed(bcc)
        return outgoing
    }

    fileprivate var senders: [Sender] { Sender.all(in: store.accounts) }

    fileprivate var sender: Sender? {
        senders.first { option in
            guard option.account.id == compose.draft.accountId else { return false }
            guard let from = compose.draft.from else { return option.first }
            return option.identity.email.caseInsensitiveCompare(from.email) == .orderedSame
        }
    }

    private var signature: String? {
        store.signature(account: compose.draft.accountId, email: sender?.identity.email)
    }

    private var triggerMatches: [Snippet] {
        guard let trigger else { return [] }
        let all = Snippet.all(in: store.preferences)
        guard !trigger.isEmpty else { return all }
        return all.filter { $0.name.localizedCaseInsensitiveContains(trigger) }
    }

    // MARK: Doing

    /// Shows the Cc and Bcc lines, with the cursor in the one asked for.
    private func reveal(_ line: String) {
        revealed = line
        compose.showCc = true
    }

    fileprivate func choose(_ sender: Sender) {
        compose.draft.accountId = sender.account.id
        compose.draft.from = sender.first ? nil : Address(name: sender.identity.name, email: sender.identity.email)
        compose.from = sender.identity.email
    }

    private func triggerKey(_ key: EditorKey) -> Bool {
        let matches = triggerMatches
        guard !matches.isEmpty else { return false }
        switch key {
        case .up: triggerIndex = max(triggerIndex - 1, 0)
        case .down: triggerIndex = min(triggerIndex + 1, matches.count - 1)
        case .accept: put(matches[min(triggerIndex, matches.count - 1)], replacingTrigger: true)
        case .cancel: trigger = nil
        }
        return true
    }

    /// Puts a snippet in at the caret, with the first recipient's first name.
    fileprivate func put(_ snippet: Snippet, replacingTrigger: Bool) {
        let text = snippet.filled(firstName: assembled.draft.to.first?.firstName)
        trigger = nil
        guard replacingTrigger else {
            DispatchQueue.main.async {
                editing.focus()
                editing.insert(text)
            }
            return
        }
        editing.replaceTrigger(with: text)
    }

    /// Moves whoever made the introduction to Bcc and thanks them first thing.
    private func instantIntro() {
        var draft = assembled.draft
        guard !draft.to.isEmpty else { return }
        let introducer = draft.to.removeFirst()
        draft.to += draft.cc
        draft.cc = []
        draft.bcc.append(introducer)
        let thanks = introducer.firstName.map { "Thanks for the intro, \($0)! Moving you to Bcc." } ?? "Thanks for the intro! Moving you to Bcc."
        draft.text = thanks + "\n\n" + draft.text
        compose.draft = draft
        compose.showCc = true
        (to, cc, bcc) = ("", "", "")
    }

    fileprivate func add(_ files: [DraftAttachment]) {
        var total = compose.draft.attachments.reduce(Int64(0)) { $0 + $1.size }
        for file in files {
            guard total + file.size <= Outgoing.limit else {
                notice = "\(file.name) wasn't added: attachments can be 25 MB together at most."
                continue
            }
            total += file.size
            compose.draft.attachments.append(file)
        }
    }

    fileprivate func picked(_ choice: TimeChoice, for timing: Timing) {
        self.timing = nil
        guard timing == .sendLater else {
            compose.remindAt = choice
            return
        }
        // After the picker's sheet has gone, so a question before sending can be shown.
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.35) { send(at: choice.date) }
    }

    fileprivate func autosave() async {
        let snapshot = assembled
        guard snapshot.draft != kept else { return }
        store.noteCompose(snapshot)
        edited = true
        try? await Task.sleep(for: .seconds(2))
        guard !Task.isCancelled, !closing else { return }
        kept = snapshot.draft
        compose.draftId = await store.saveDraft(snapshot)
    }

    private func close() {
        closing = true
        store.closeCompose(assembled, keep: edited || compose.keep || compose.draftId != nil)
    }

    fileprivate func discard() {
        closing = true
        store.discardDraft(compose)
    }

    /// Gone without Send or Back: swiped away on iOS. Kept as when closed.
    fileprivate func left() {
        guard !closing, timing == nil, !pickingSnippet, !pickingSender, !importing else { return }
        store.closeCompose(assembled, keep: edited || compose.keep || compose.draftId != nil)
    }

    /// Checks the message, asks about what looks forgotten, then sends.
    private func send(at date: Date? = nil, archive: Bool = false) {
        let draft = assembled.draft
        let everyone = draft.to + draft.cc + draft.bcc
        guard !everyone.isEmpty else {
            notice = "Add who it goes to."
            return
        }
        if let wrong = everyone.first(where: { !$0.isValid }) {
            notice = "“\(wrong.email)” isn't an address."
            return
        }
        notice = nil
        var worries: [String] = []
        let placeholders = Outgoing.placeholders(in: draft.text)
        if !placeholders.isEmpty {
            worries.append("\(placeholders.joined(separator: ", ")) still \(placeholders.count == 1 ? "needs" : "need") filling in.")
        }
        if draft.attachments.isEmpty, draft.forwardAttachmentsOf == nil, Outgoing.mentionsAttachment(draft.text) {
            worries.append("It mentions an attachment, but nothing is attached.")
        }
        guard worries.isEmpty else {
            warning = Warning(message: worries.joined(separator: " "), at: date, archive: archive)
            return
        }
        finish(at: date, archive: archive)
    }

    fileprivate func finish(at date: Date?, archive: Bool) {
        closing = true
        store.send(assembled, at: date, archive: archive)
    }
}
