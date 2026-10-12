import MailPush
import SwiftUI

/// Someone's contact photo, else their Gravatar or their company's logo, else a circle with their
/// initials in a colour picked from their address.
struct Avatar: View {
    let initials: String
    let email: String
    var size: CGFloat = 32

    private var tint: Color {
        let colors = Theme.accountColors
        return colors[SenderAvatar.colorIndex(email, count: colors.count)].color
    }

    private var photo: PlatformImage? {
        let key = email.lowercased()
        return AddressBook.shared.photos[key] ?? SenderPhotos.shared.photos[key]
    }

    var body: some View {
        Group {
            if let photo {
                Image(platformImage: photo)
                    .resizable()
                    .scaledToFill()
                    .frame(width: size * Platform.scale, height: size * Platform.scale)
                    .clipShape(Circle())
                    .accessibilityHidden(true)
            } else {
                initialsCircle
            }
        }
        .task(id: email) {
            guard AddressBook.shared.photos[email.lowercased()] == nil else { return }
            await SenderPhotos.shared.load(email)
        }
    }

    private var initialsCircle: some View {
        Text(initials)
            .font(.ui(size * 0.4, .semibold))
            .foregroundStyle(Tokens.primaryForeground.color)
            .frame(width: size * Platform.scale, height: size * Platform.scale)
            .background(Circle().fill(tint))
            .accessibilityHidden(true)
    }
}
