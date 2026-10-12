import Intents
import UserNotifications

/// New mail shown as a message from its sender: their avatar, with the app's icon in its corner.
public enum MessageNotification {
    /// The content, with its title as the sender's name and its body as the message, turned into
    /// a message from the sender. Without the entitlement it stays as it was.
    public static func fromSender(
        _ content: UNNotificationContent, email: String, initials: String, avatar: Data?
    ) async -> UNNotificationContent {
        let sender = INPerson(
            personHandle: INPersonHandle(value: email, type: .emailAddress),
            nameComponents: nil,
            displayName: content.title,
            image: SenderAvatar.image(initials: initials, email: email, avatar: avatar).map { INImage(imageData: $0) },
            contactIdentifier: nil,
            customIdentifier: email
        )
        let intent = INSendMessageIntent(
            recipients: nil, outgoingMessageType: .outgoingMessageText, content: content.body,
            speakableGroupName: nil, conversationIdentifier: content.threadIdentifier, serviceName: nil,
            sender: sender, attachments: nil
        )
        let interaction = INInteraction(intent: intent, response: nil)
        interaction.direction = .incoming
        try? await interaction.donate()
        return (try? content.updating(from: intent)) ?? content
    }
}
