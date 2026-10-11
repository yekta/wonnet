import CryptoKit
import Foundation
import Security

/// This device's push key and what pushes carry sealed with it. The server seals a new message's
/// sender, subject and snippet with the key (AES-256-GCM), so Apple carries only ciphertext. The
/// key stays in the keychain of the app group, which the app and its notification extension share.
public enum PushSeal {
    /// The device's key as base64, made the first time.
    public static func key() -> String? {
        guard let key = stored() ?? made() else { return nil }
        return key.withUnsafeBytes { Data($0) }.base64EncodedString()
    }

    /// What a push's `sealed` says, or nil when it isn't sealed with this device's key.
    public static func open(_ sealed: String) -> [String: Any]? {
        guard let key = stored(),
              let data = Data(base64Encoded: sealed),
              let box = try? AES.GCM.SealedBox(combined: data),
              let plain = try? AES.GCM.open(box, using: key)
        else { return nil }
        return try? JSONSerialization.jsonObject(with: plain) as? [String: Any]
    }

    /// The keychain item. Readable after the first unlock, when pushes come to a locked phone.
    private static var query: [String: Any] {
        var query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: "push-key",
            kSecUseDataProtectionKeychain as String: true,
        ]
        if let group = Bundle.main.object(forInfoDictionaryKey: "AppGroup") as? String, !group.isEmpty {
            query[kSecAttrAccessGroup as String] = group
        }
        return query
    }

    private static func stored() -> SymmetricKey? {
        var lookup = query
        lookup[kSecReturnData as String] = true
        var found: AnyObject?
        guard SecItemCopyMatching(lookup as CFDictionary, &found) == errSecSuccess, let data = found as? Data else { return nil }
        return SymmetricKey(data: data)
    }

    private static func made() -> SymmetricKey? {
        let key = SymmetricKey(size: .bits256)
        var item = query
        item[kSecValueData as String] = key.withUnsafeBytes { Data($0) }
        item[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlock
        guard SecItemAdd(item as CFDictionary, nil) == errSecSuccess else { return nil }
        return key
    }
}
