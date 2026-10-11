import SwiftUI

/// A sheet over the window. One at a time: when one turns into another (the palette into the
/// snooze times), the first goes and the second comes.
enum Modal: Identifiable {
    case palette(PaletteScope)
    case label(Labeling)
    case snooze([String])
    /// iOS's settings; the Mac's take the window.
    case settings
    case gallery

    var id: String {
        switch self {
        case .palette(let scope): "palette/\(scope)"
        case .label(let labeling): "label/\(labeling.id)"
        case .snooze(let threads): "snooze/\(threads.joined(separator: ","))"
        case .settings: "settings"
        case .gallery: "gallery"
        }
    }
}

extension MailStore {
    var modal: Modal? {
        get {
            if let palette { return .palette(palette) }
            if let labeling { return .label(labeling) }
            if let snoozing { return .snooze(snoozing) }
            #if os(iOS)
            if settings != nil { return .settings }
            #endif
            return galleryOpen ? .gallery : nil
        }
        set {
            guard newValue == nil else { return }
            palette = nil
            labeling = nil
            snoozing = nil
            #if os(iOS)
            closeSettings()
            #endif
            galleryOpen = false
        }
    }
}

extension View {
    /// The sheets and the questions both apps show over everything.
    func mailSheets(_ store: MailStore) -> some View {
        modifier(MailSheets(store: store))
    }
}

private struct MailSheets: ViewModifier {
    @Bindable var store: MailStore

    func body(content: Content) -> some View {
        content
            .sheet(item: $store.modal) { modal in
                Group {
                    switch modal {
                    case .palette(let scope): CommandPalette(scope: scope)
                    case .label(let labeling): LabelPicker(labeling: labeling)
                    case .snooze(let threads): SnoozePicker(threads: threads)
                    case .settings:
                        #if os(iOS)
                        SettingsSheet()
                        #endif
                    case .gallery: GalleryView()
                    }
                }
                .environment(store)
            }
            .alert(
                store.confirmation?.title ?? "", isPresented: Binding(get: { store.confirmation != nil }, set: { if !$0 { store.confirmation = nil } }),
                presenting: store.confirmation
            ) { confirmation in
                Button(confirmation.action, role: .destructive, action: confirmation.perform)
                Button("Cancel", role: .cancel) {}
            } message: { confirmation in
                Text(confirmation.message)
            }
    }
}
