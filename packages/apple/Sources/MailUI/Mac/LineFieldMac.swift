#if os(macOS)
import AppKit
import SwiftUI

/// The Mac's one line to type in. SwiftUI's plain field is a borderless `NSTextField`, whose
/// editor reaches past the field's sides, so the field cuts off what is drawn there: the caret
/// after the last letter of a line that scrolls. This one's cell keeps the text `inset` in from
/// its sides, as a bordered field's does, and the field stands out past its frame by as much, so
/// its text lies where a SwiftUI field's would. Return runs `onReturn`; Esc goes on to what lies
/// behind it.
struct LineField: View {
    var placeholder = ""
    @Binding var text: String
    let font: NSFont
    var color = Tokens.foreground
    var secure = false
    var onReturn: (() -> Void)?

    var body: some View {
        LineFieldRepresentable(field: self)
            .id(secure)
            .padding(.horizontal, -LineFieldCell.inset)
    }
}

final class LineFieldCell: NSTextFieldCell {
    static let inset: CGFloat = 3

    override func drawingRect(forBounds rect: NSRect) -> NSRect {
        super.drawingRect(forBounds: rect).insetBy(dx: Self.inset, dy: 0)
    }

    override func edit(withFrame rect: NSRect, in controlView: NSView, editor: NSText, delegate: Any?, event: NSEvent?) {
        super.edit(withFrame: rect.insetBy(dx: Self.inset, dy: 0), in: controlView, editor: editor, delegate: delegate, event: event)
    }

    override func select(withFrame rect: NSRect, in controlView: NSView, editor: NSText, delegate: Any?, start: Int, length: Int) {
        super.select(withFrame: rect.insetBy(dx: Self.inset, dy: 0), in: controlView, editor: editor, delegate: delegate, start: start, length: length)
    }
}

/// `LineFieldCell` for a secure field, whose editor shows dots.
final class LineSecureFieldCell: NSSecureTextFieldCell {
    override func drawingRect(forBounds rect: NSRect) -> NSRect {
        super.drawingRect(forBounds: rect).insetBy(dx: LineFieldCell.inset, dy: 0)
    }

    override func edit(withFrame rect: NSRect, in controlView: NSView, editor: NSText, delegate: Any?, event: NSEvent?) {
        super.edit(withFrame: rect.insetBy(dx: LineFieldCell.inset, dy: 0), in: controlView, editor: editor, delegate: delegate, event: event)
    }

    override func select(withFrame rect: NSRect, in controlView: NSView, editor: NSText, delegate: Any?, start: Int, length: Int) {
        super.select(
            withFrame: rect.insetBy(dx: LineFieldCell.inset, dy: 0), in: controlView, editor: editor, delegate: delegate, start: start,
            length: length
        )
    }
}

private struct LineFieldRepresentable: NSViewRepresentable {
    let field: LineField

    func makeNSView(context: Context) -> NSTextField {
        let view = NSTextField()
        let cell = field.secure ? LineSecureFieldCell(textCell: "") : LineFieldCell(textCell: "")
        cell.isEditable = true
        cell.isSelectable = true
        cell.isScrollable = true
        cell.usesSingleLineMode = true
        cell.lineBreakMode = .byClipping
        view.cell = cell
        view.isBordered = false
        view.isBezeled = false
        view.drawsBackground = false
        view.focusRingType = .none
        view.delegate = context.coordinator
        view.setContentHuggingPriority(.defaultLow, for: .horizontal)
        view.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        return view
    }

    func updateNSView(_ view: NSTextField, context: Context) {
        context.coordinator.field = field
        if view.font != field.font { view.font = field.font }
        view.textColor = field.color.platform
        if view.placeholderString != field.placeholder { view.placeholderString = field.placeholder }
        if view.stringValue != field.text {
            // While it is typed in, its editor holds the text.
            if let editor = view.currentEditor() {
                editor.string = field.text
            } else {
                view.stringValue = field.text
            }
        }
        view.isEnabled = context.environment.isEnabled
    }

    func sizeThatFits(_ proposal: ProposedViewSize, nsView view: NSTextField, context: Context) -> CGSize? {
        CGSize(width: proposal.width ?? view.intrinsicContentSize.width, height: view.intrinsicContentSize.height)
    }

    func makeCoordinator() -> Coordinator { Coordinator(field: field) }

    final class Coordinator: NSObject, NSTextFieldDelegate {
        var field: LineField

        init(field: LineField) {
            self.field = field
        }

        func controlTextDidChange(_ notification: Notification) {
            guard let view = notification.object as? NSTextField else { return }
            field.text = view.stringValue
        }

        func control(_ control: NSControl, textView: NSTextView, doCommandBy selector: Selector) -> Bool {
            switch selector {
            case #selector(NSResponder.insertNewline(_:)):
                field.onReturn?()
                return true
            case #selector(NSResponder.cancelOperation(_:)):
                // Esc goes on to what lies behind the field, never to the editor, which would
                // offer to complete the word.
                _ = control.nextResponder?.tryToPerform(#selector(NSResponder.cancelOperation(_:)), with: control)
                return true
            default:
                return false
            }
        }
    }
}
#endif
