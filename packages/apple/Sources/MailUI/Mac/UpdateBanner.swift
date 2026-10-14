#if os(macOS)
import SwiftUI

/// A new version of the app, floating at the bottom of the window: the offer, the download as it
/// goes, and the restart that finishes it. Closed, it stays away until the update's state changes.
struct UpdateBanner: View {
    let updater: AppUpdater
    @State private var closed: AppUpdater.State?

    private var shown: Bool {
        updater.state != .idle && updater.state != closed
    }

    var body: some View {
        ZStack {
            if shown {
                banner
                    .padding(.bottom, Space.xl)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
            }
        }
        .animation(.smooth(duration: 0.3), value: shown)
        .onChange(of: updater.state) { closed = nil }
    }

    @ViewBuilder private var banner: some View {
        let close = { closed = updater.state }
        switch updater.state {
        case .idle:
            EmptyView()
        case .checking:
            Banner(symbol: .refreshCw, title: "Checking for updates", pending: true)
        case .upToDate:
            Banner(symbol: .circleCheck, tint: Tokens.success, title: "Wonnet is up to date", detail: "Wonnet \(updater.current) is the newest version.", close: close)
        case .available(let version):
            Banner(
                symbol: .circleArrowDown, tint: Tokens.success, title: "Wonnet \(version) is out", detail: "You have \(updater.current).",
                action: ActionButton(title: "Update", variant: .primary, size: .small) { updater.install() }, close: close
            )
        case .downloading(let version, let fraction):
            Banner(
                symbol: .circleArrowDown, tint: Tokens.success, title: "Downloading Wonnet \(version)",
                detail: "\(Int(fraction * 100))% done", progress: fraction
            )
        case .installing(let version):
            Banner(symbol: .circleArrowDown, title: "Installing Wonnet \(version)", pending: true)
        case .ready(let version):
            Banner(
                symbol: .circleCheck, tint: Tokens.success, title: "Wonnet \(version) is installed", detail: "Restart to start using it.",
                action: ActionButton(title: "Restart", variant: .primary, size: .small) { updater.relaunch() }, close: close
            )
        case .failed(let message):
            Banner(
                symbol: .circleAlert, tint: Tokens.destructive, title: "Update failed", detail: message,
                action: ActionButton(title: "Try again", size: .small) { updater.retry() }, close: close
            )
        }
    }
}
#endif
