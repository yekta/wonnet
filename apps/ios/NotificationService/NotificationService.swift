import MailPush
import UserNotifications

/// Opens what the server pushed for new mail, sealed with this device's key, and shows it: the
/// sender, the subject and the start of the message. Without the key the push reads "New mail".
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
        contentHandler(content)
    }
}
