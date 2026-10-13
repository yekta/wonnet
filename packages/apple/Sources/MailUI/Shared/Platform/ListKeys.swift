import SwiftUI

#if os(macOS)
import AppKit
#endif

/// The keys a field reads before the system does: ↑ and ↓ through a list under it, Tab to pick
/// from it, Esc to leave it, and Backspace with nothing typed, to take a chip back.
enum FieldKey {
    case up, down, tab, escape, delete
}

extension View {
    /// `handle` reads the keys; `.ignored` leaves one to the field. The Mac's text fields keep
    /// these keys to themselves, so there they are read before the field gets them.
    func onFieldKeys(_ handle: @escaping (FieldKey) -> KeyPress.Result) -> some View {
        #if os(macOS)
        background(FieldKeyMonitor(handle: handle))
        #else
        onKeyPress(.upArrow) { handle(.up) }
            .onKeyPress(.downArrow) { handle(.down) }
            .onKeyPress(.tab) { handle(.tab) }
            .onKeyPress(.escape) { handle(.escape) }
            .onKeyPress(.delete) { handle(.delete) }
        #endif
    }

    /// ↑ and ↓ move through a list under a field (`-1`, `1`), Esc leaves it.
    func onListKeys(move: @escaping (Int) -> Void, escape: @escaping () -> Void) -> some View {
        onFieldKeys { key in
            switch key {
            case .up: move(-1)
            case .down: move(1)
            case .escape: escape()
            case .tab, .delete: return .ignored
            }
            return .handled
        }
    }
}

#if os(macOS)
private struct FieldKeyMonitor: NSViewRepresentable {
    let handle: (FieldKey) -> KeyPress.Result

    final class Coordinator {
        var monitor: Any?
        var handle: (FieldKey) -> KeyPress.Result = { _ in .ignored }
    }

    func makeCoordinator() -> Coordinator { Coordinator() }

    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        let coordinator = context.coordinator
        coordinator.monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak view] event in
            guard let window = view?.window, event.window === window else { return event }
            let flags = event.modifierFlags.intersection([.command, .control, .option, .shift])
            guard flags.isEmpty, let key = FieldKey(keyCode: event.keyCode) else { return event }
            return MainActor.assumeIsolated { coordinator.handle(key) } == .handled ? nil : event
        }
        return view
    }

    func updateNSView(_ view: NSView, context: Context) {
        context.coordinator.handle = handle
    }

    static func dismantleNSView(_ view: NSView, coordinator: Coordinator) {
        if let monitor = coordinator.monitor { NSEvent.removeMonitor(monitor) }
    }
}

private extension FieldKey {
    init?(keyCode: UInt16) {
        switch keyCode {
        case 126: self = .up
        case 125: self = .down
        case 48: self = .tab
        case 53: self = .escape
        case 51: self = .delete
        default: return nil
        }
    }
}
#endif
