import MailPush
import UserNotifications

/// Opens what the server pushed for new mail, sealed with this device's key, and shows it as a
/// message from its sender: their avatar with the app's icon in its corner, the subject and the
/// start of the message. Without the key the push reads "New mail".
final class NotificationService: UNNotificationServiceExtension {
    override func didReceive(_ request: UNNotificationRequest, withContentHandler contentHandler: @escaping (UNNotificationContent) -> Void) {
        guard let content = request.content.mutableCopy() as? UNMutableNotificationContent,
              let sealed = content.userInfo["sealed"] as? String,
              let mail = PushSeal.open(sealed),
              let thread = mail["thread"] as? String
        else {
            contentHandler(request.content)
            return
        }
        content.title = mail["from"] as? String ?? content.title
        content.subtitle = mail["subject"] as? String ?? ""
        content.body = mail["snippet"] as? String ?? ""
        content.threadIdentifier = thread
        content.userInfo = ["thread": thread]
        guard let email = mail["email"] as? String else {
            contentHandler(content)
            return
        }
        let initials = mail["initials"] as? String ?? ""
        let avatar = (mail["avatar"] as? String).flatMap(URL.init(string:))
        Task {
            let picture: Data? = if let avatar { await SenderAvatar.download(avatar) } else { nil }
            contentHandler(await MessageNotification.fromSender(content, email: email, initials: initials, avatar: picture))
        }
    }
}
