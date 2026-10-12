#if os(macOS)
import AppKit
import SwiftUI

/// A key of a keybinding on a button that records another: clicked, it takes the next keys
/// pressed with their modifiers, or two in a row (G, then I). Esc or a click elsewhere stops it
/// without a change. Without `key` it is a plus, to add one.
struct KeyRecorder: View {
    /// How long it waits for a second key after one without ⌘, ⌃ or ⌥.
    private static let secondKey: Duration = .milliseconds(900)

    let key: String?
    var help = "Record another key"
    let record: (String) -> Void
    @State private var strokes: [String]?
    @State private var monitors: [Any] = []
    @State private var waiting: Task<Void, Never>?

    var body: some View {
        Button(action: start) {
            Face(key: key, strokes: strokes)
        }
        .buttonStyle(.press)
        .help(strokes == nil ? help : "Press the keys, or Esc to stop")
        .onDisappear(perform: stop)
    }

    private struct Face: View {
        let key: String?
        let strokes: [String]?
        @Environment(\.hovered) private var hovered

        var body: some View {
            Group {
                if let strokes {
                    Text(strokes.isEmpty ? "Press keys" : strokes.map(Keys.label).joined(separator: " "))
                        .textStyle(.label, color: Tokens.primary.color)
                } else if let key {
                    KeyCaps(caps: Keys.caps(key))
                } else {
                    Image(.plus, size: 13).foregroundStyle(hovered ? Tokens.foreground.color : Tokens.mutedForeground.color)
                }
            }
            .padding(.horizontal, Space.xs)
            .frame(minWidth: ControlSize.small.height, minHeight: ControlSize.small.height)
            .background(RoundedRectangle(cornerRadius: Radius.sm).fill(fill))
            .padding(.horizontal, Theme.buttonGap / 2)
            .contentShape(Rectangle())
        }

        private var fill: Color {
            if strokes != nil { return Tokens.backgroundAccentStronger.color }
            return hovered ? Tokens.backgroundAccent.color : .clear
        }
    }

    private func start() {
        guard strokes == nil else { return }
        strokes = []
        KeyCoordinator.recording = true
        let keys = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            let plain = event.modifierFlags.intersection([.command, .control, .option, .shift]).isEmpty
            if plain, event.keyCode == 53 {
                stop()
                return nil
            }
            guard let stroke = KeyNames.stroke(of: event) else { return nil }
            let pressed = (strokes ?? []) + [stroke]
            strokes = pressed
            waiting?.cancel()
            let held = ["mod+", "ctrl+", "alt+"].contains { stroke.contains($0) }
            guard pressed.count == 1, !held else {
                finish()
                return nil
            }
            waiting = Task {
                try? await Task.sleep(for: Self.secondKey)
                guard !Task.isCancelled else { return }
                finish()
            }
            return nil
        }
        let clicks = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown]) { event in
            stop()
            return event
        }
        monitors = [keys, clicks].compactMap { $0 }
    }

    private func finish() {
        let key = (strokes ?? []).joined(separator: " ")
        stop()
        guard !key.isEmpty else { return }
        record(key)
    }

    private func stop() {
        waiting?.cancel()
        waiting = nil
        monitors.forEach(NSEvent.removeMonitor)
        monitors = []
        strokes = nil
        KeyCoordinator.recording = false
    }
}
#endif
