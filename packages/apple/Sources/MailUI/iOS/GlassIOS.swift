#if os(iOS)
import SwiftUI

extension View {
    /// The system's search field, in the bottom bar where the system has it there, with its
    /// symbol in the foreground and its placeholder in the colour every field's has. The field
    /// ignores both colours of SwiftUI, so they are set on the field once it is in the window.
    @ViewBuilder func searchField(text: Binding<String>, prompt: String) -> some View {
        if #available(iOS 26, *) {
            searchable(text: text, prompt: prompt)
                .toolbar { DefaultToolbarItem(kind: .search, placement: .bottomBar) }
                .background(SearchFieldColors(prompt: prompt))
        } else {
            searchable(text: text, placement: .navigationBarDrawer(displayMode: .always), prompt: prompt)
                .background(SearchFieldColors(prompt: prompt))
        }
    }
}

private struct SearchFieldColors: UIViewRepresentable {
    let prompt: String

    func makeUIView(context: Context) -> Finder { Finder() }

    func updateUIView(_ view: Finder, context: Context) {
        view.prompt = prompt
    }

    final class Finder: UIView {
        var prompt = "" {
            didSet { color() }
        }

        override func didMoveToWindow() {
            super.didMoveToWindow()
            color()
        }

        private func color(attempt: Int = 0) {
            guard attempt < 10 else { return }
            guard let field = UIView.first(UISearchTextField.self, in: stack ?? window) else {
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { [weak self] in self?.color(attempt: attempt + 1) }
                return
            }
            field.attributedPlaceholder = NSAttributedString(string: prompt, attributes: [.foregroundColor: Tokens.mutedStrongerForeground.platform])
            field.leftView?.tintColor = Tokens.foreground.platform
        }

        /// The navigation stack the field belongs to, where another sheet's field can't be found.
        private var stack: UIView? {
            sequence(first: self as UIResponder, next: \.next).lazy.compactMap { $0 as? UINavigationController }.first?.view
        }
    }
}

extension UIView {
    static func first<Found: UIView>(_ kind: Found.Type, in view: UIView?) -> Found? {
        guard let view else { return nil }
        if let found = view as? Found { return found }
        return view.subviews.lazy.compactMap { first(kind, in: $0) }.first
    }
}
#endif
