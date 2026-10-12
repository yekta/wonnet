import Foundation
import Observation

/// Senders' pictures from the server, their Gravatar or their company's logo, for the people the
/// contacts have no photo of.
@Observable @MainActor
final class SenderPhotos {
    static let shared = SenderPhotos()

    private(set) var photos: [String: PlatformImage] = [:]

    @ObservationIgnored var bridge: CoreBridge?
    @ObservationIgnored private var asked: Set<String> = []

    /// Asks the core once a launch; the avatar shows the picture when it comes.
    func load(_ email: String) async {
        let email = email.lowercased()
        guard let bridge, !asked.contains(email) else { return }
        asked.insert(email)
        guard let data = await Self.read(email, bridge), let image = PlatformImage(data: data) else { return }
        photos[email] = image
    }

    /// The picture as image data, for a notification.
    func data(_ email: String) async -> Data? {
        guard let bridge else { return nil }
        return await Self.read(email.lowercased(), bridge)
    }

    nonisolated private static func read(_ email: String, _ bridge: CoreBridge) async -> Data? {
        guard let reply = try? await bridge.call("avatar", ["email": email], as: AvatarReply.self),
              let path = reply.path
        else { return nil }
        return try? Data(contentsOf: URL(fileURLWithPath: path))
    }
}
