import Foundation
import MailPush
import os
import UserNotifications

#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// What a notification's button asks for.
enum NotificationAnswer {
    case archive
    case read
    case reply(String)
}

/// New mail as notifications while the app is in the background, a click on one opening its
/// thread and its buttons archiving, marking read or replying; and the unread count of the
/// inboxes on the app's icon. On the Mac the app tells of mail as it syncs. On iOS the server
/// pushes it, sealed with this device's key, which the notification extension opens. A thread's
/// notification is taken down once it is read or leaves the inbox, here or on another device.
@MainActor
final class Notifier: NSObject, UNUserNotificationCenterDelegate {
    static let shared = Notifier()

    /// Opens the thread of a clicked notification. One clicked before it is set waits for it.
    var open: ((String) -> Void)? {
        didSet {
            guard let open, let pending else { return }
            self.pending = nil
            open(pending)
        }
    }

    /// Does what a notification's button asked for, and returns once the server has it.
    var answer: ((NotificationAnswer, String) async -> Void)?

    /// The core, to give it the push token and to ask which threads are still unread.
    var bridge: CoreBridge? {
        didSet { registerDevice() }
    }

    private var pending: String?
    private var asked = false
    private var badge = 0
    /// This device's push token, once APNs gave it: the server tells of new mail from then on.
    private var device: [String: Any]?

    /// Notifications need a bundle; a bare `swift run` has none.
    private var center: UNUserNotificationCenter? {
        Bundle.main.bundleIdentifier == nil ? nil : UNUserNotificationCenter.current()
    }

    /// Becomes the notifications' delegate, as the app starts, to hear of a click that launched it.
    func install() {
        guard let center else { return }
        center.delegate = self
        let archive = UNNotificationAction(identifier: "archive", title: "Archive", options: [.authenticationRequired])
        let read = UNNotificationAction(identifier: "read", title: "Mark as Read", options: [])
        let reply = UNTextInputNotificationAction(
            identifier: "reply", title: "Reply", options: [.authenticationRequired], textInputButtonTitle: "Send", textInputPlaceholder: ""
        )
        center.setNotificationCategories([UNNotificationCategory(identifier: "message", actions: [archive, read, reply], intentIdentifiers: [])])
    }

    /// Asks once, when there is mail to tell of: after an account is added, not at first launch.
    func askPermission() {
        guard !asked, let center else { return }
        asked = true
        center.requestAuthorization(options: [.alert, .sound, .badge]) { _, _ in }
    }

    func notify(_ messages: [NewMailItem]) {
        // The server's pushes tell of it.
        guard device == nil else { return }
        guard let center, !Self.frontmost else { return }
        for message in messages.suffix(5) {
            let content = UNMutableNotificationContent()
            content.title = message.from
            content.subtitle = message.subject
            content.body = message.snippet
            content.sound = .default
            content.threadIdentifier = message.thread
            content.categoryIdentifier = "message"
            content.userInfo = ["thread": message.thread]
            center.add(UNNotificationRequest(identifier: UUID().uuidString, content: content, trigger: nil))
        }
    }

    func setBadge(_ count: Int) {
        guard count != badge else { return }
        badge = count
        #if os(macOS)
        NSApp.dockTile.badgeLabel = count > 0 ? "\(count)" : nil
        #else
        center?.setBadgeCount(count)
        #endif
    }

    /// Takes down every notification and the badge: they were of mail no longer on this device.
    func signedOut() {
        setBadge(0)
        center?.removeAllDeliveredNotifications()
    }

    /// APNs gave this device its token.
    func registered(token: Data) {
        let hex = token.map { String(format: "%02x", $0) }.joined()
        guard let key = PushSeal.key() else {
            Self.log.error("No push key in the keychain, so device \(hex.prefix(8), privacy: .public) isn't registered")
            return
        }
        guard let topic = Bundle.main.bundleIdentifier else { return }
        let sandbox = Bundle.main.object(forInfoDictionaryKey: "PushEnvironment") as? String != "production"
        Self.log.info("APNs gave device \(hex.prefix(8), privacy: .public) (sandbox: \(sandbox, privacy: .public))")
        device = ["token": hex, "topic": topic, "sandbox": sandbox, "key": key]
        registerDevice()
    }

    /// APNs gave no token.
    func failedToRegister(_ error: Error) {
        Self.log.error("APNs gave no token: \(error.localizedDescription, privacy: .public)")
    }

    private static let log = Logger(subsystem: Bundle.main.bundleIdentifier ?? "Wonnet", category: "push")

    private func registerDevice() {
        guard let bridge, let device else { return }
        Task {
            do {
                _ = try await bridge.call("set_push_device", device, as: Empty.self)
                Self.log.info("The server has this device")
            } catch {
                Self.log.error("The server didn't take this device: \(error.localizedDescription, privacy: .public)")
            }
        }
    }

    /// A background push: the badge, and the threads whose notifications to take down.
    func received(_ push: [AnyHashable: Any]) async {
        if let badge = (push["aps"] as? [String: Any])?["badge"] as? Int { setBadge(badge) }
        guard let sealed = push["sealed"] as? String, let settled = PushSeal.open(sealed)?["settled"] as? [String] else { return }
        await takeDown(Set(settled))
    }

    /// Takes down the notifications of these threads (all shown, when nil) that are no longer
    /// unread in an inbox.
    func settle(_ threads: [String]?) async {
        guard let center, let bridge else { return }
        let shown = Set(await center.deliveredNotifications().compactMap { $0.request.content.userInfo["thread"] as? String })
        let changed = threads.map { shown.intersection($0) } ?? shown
        guard !changed.isEmpty else { return }
        guard let unread = try? await bridge.call("unread_in_inbox", ["threads": Array(changed)], as: ThreadList.self) else { return }
        await takeDown(changed.subtracting(unread.threads))
    }

    private func takeDown(_ threads: Set<String>) async {
        guard let center, !threads.isEmpty else { return }
        let delivered = await center.deliveredNotifications()
        let gone = delivered.filter { threads.contains($0.request.content.userInfo["thread"] as? String ?? "") }
        center.removeDeliveredNotifications(withIdentifiers: gone.map(\.request.identifier))
    }

    private static var frontmost: Bool {
        #if os(macOS)
        NSApp.isActive
        #else
        UIApplication.shared.applicationState == .active
        #endif
    }

    nonisolated func userNotificationCenter(_ center: UNUserNotificationCenter, didReceive response: UNNotificationResponse) async {
        guard let thread = response.notification.request.content.userInfo["thread"] as? String else { return }
        let text = (response as? UNTextInputNotificationResponse)?.userText
        await respond(to: response.actionIdentifier, thread: thread, text: text)
    }

    private func respond(to action: String, thread: String, text: String?) async {
        switch action {
        case UNNotificationDefaultActionIdentifier:
            guard let open else {
                pending = thread
                return
            }
            open(thread)
        case "archive": await answer?(.archive, thread)
        case "read": await answer?(.read, thread)
        case "reply":
            guard let text, !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
            await answer?(.reply(text), thread)
        default: break
        }
    }

    nonisolated func userNotificationCenter(_ center: UNUserNotificationCenter, willPresent notification: UNNotification) async -> UNNotificationPresentationOptions {
        []
    }
}

private struct ThreadList: Decodable {
    let threads: [String]
}
