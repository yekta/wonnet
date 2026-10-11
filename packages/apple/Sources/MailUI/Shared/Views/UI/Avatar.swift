import SwiftUI

/// Someone's contact photo, or a circle with their initials in a colour picked from their address.
struct Avatar: View {
    let initials: String
    let email: String
    var size: CGFloat = 32

    private var tint: Color {
        let colors = Theme.accountColors
        let sum = email.unicodeScalars.reduce(0) { ($0 &* 31 &+ Int($1.value)) & 0xffff }
        return colors[sum % colors.count].color
    }

    var body: some View {
        if let photo = AddressBook.shared.photos[email.lowercased()] {
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

    private var initialsCircle: some View {
        Text(initials)
            .font(.ui(size * 0.4, .semibold))
            .foregroundStyle(.white)
            .frame(width: size * Platform.scale, height: size * Platform.scale)
            .background(Circle().fill(tint))
            .accessibilityHidden(true)
    }
}
