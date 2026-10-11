import Contacts
import Foundation
import Observation

/// The device's contacts: their names go to the core, which shows them instead of the names mail
/// came with, and their photos to the avatars.
@Observable @MainActor
final class AddressBook {
    static let shared = AddressBook()

    /// Each address's contact photo.
    private(set) var photos: [String: PlatformImage] = [:]

    @ObservationIgnored private var bridge: CoreBridge?
    @ObservationIgnored private var observer: NSObjectProtocol?

    /// Contacts need the bundle's usage text; a bare `swift run` has none.
    private var available: Bool {
        Bundle.main.object(forInfoDictionaryKey: "NSContactsUsageDescription") != nil
    }

    /// Asks once, when there is mail to put names on, and reads the contacts again whenever they
    /// change.
    func start(_ bridge: CoreBridge) {
        guard available, self.bridge == nil else { return }
        self.bridge = bridge
        observer = NotificationCenter.default.addObserver(forName: .CNContactStoreDidChange, object: nil, queue: .main) { _ in
            Task { @MainActor in AddressBook.shared.load() }
        }
        guard CNContactStore.authorizationStatus(for: .contacts) == .notDetermined else {
            load()
            return
        }
        CNContactStore().requestAccess(for: .contacts) { _, _ in
            Task { @MainActor in AddressBook.shared.load() }
        }
    }

    /// Reads every contact away from the main thread. Without access, the names given before
    /// are forgotten.
    private func load() {
        guard let bridge else { return }
        Task.detached(priority: .utility) {
            let (people, photos) = Self.read()
            bridge.send("set_address_book", ["people": people])
            await MainActor.run {
                AddressBook.shared.photos = photos.compactMapValues(PlatformImage.init(data:))
            }
        }
    }

    nonisolated private static func read() -> ([[String: String]], [String: Data]) {
        guard allowed(CNContactStore.authorizationStatus(for: .contacts)) else { return ([], [:]) }
        let keys: [CNKeyDescriptor] = [
            CNContactFormatter.descriptorForRequiredKeys(for: .fullName),
            CNContactOrganizationNameKey as CNKeyDescriptor,
            CNContactEmailAddressesKey as CNKeyDescriptor,
            CNContactThumbnailImageDataKey as CNKeyDescriptor,
        ]
        var people: [[String: String]] = []
        var photos: [String: Data] = [:]
        try? CNContactStore().enumerateContacts(with: CNContactFetchRequest(keysToFetch: keys)) { contact, _ in
            let name = CNContactFormatter.string(from: contact, style: .fullName) ?? contact.organizationName
            for email in contact.emailAddresses {
                let address = (email.value as String).trimmingCharacters(in: .whitespaces).lowercased()
                guard !address.isEmpty else { continue }
                if !name.isEmpty { people.append(["name": name, "email": address]) }
                if let photo = contact.thumbnailImageData { photos[address] = photo }
            }
        }
        return (people, photos)
    }

    nonisolated private static func allowed(_ status: CNAuthorizationStatus) -> Bool {
        #if os(iOS)
        status == .authorized || status == .limited
        #else
        status == .authorized
        #endif
    }
}
