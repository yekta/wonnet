import SwiftUI

/// Someone the thread is with: who they are, a way to write to them, and the latest threads with
/// them. Beside the thread on a wide Mac window; a sheet on iOS. `openThread` makes the threads
/// open on a click. In a sheet, `closeSheet` closes it before compose opens.
struct PersonView: View {
    @Environment(MailStore.self) private var store
    let email: String
    var closeSheet: (() -> Void)?
    var openThread: ((String) -> Void)?
    @State private var person: Person?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                VStack(alignment: .leading, spacing: Space.l) {
                    Avatar(initials: person?.initials ?? String(email.prefix(1)).uppercased(), email: email, size: 52)
                    VStack(alignment: .leading, spacing: Space.xxs) {
                        Text(name).textStyle(.heading)
                        Text(email).textStyle(.label, color: Tokens.mutedStrongerForeground.color).textSelection(.enabled)
                    }
                    ActionButton(title: "New message", symbol: .squarePen, variant: .outline, action: write)
                }
                .padding(Space.xl)
                if let threads = person?.threads, !threads.isEmpty {
                    SectionHeading(title: "Recent")
                        .padding(.horizontal, Space.xl)
                        .padding(.top, Space.xxs)
                        .padding(.bottom, Space.m - Theme.rowGap / 2)
                    VStack(spacing: 0) {
                        ForEach(threads) { row in
                            NavRow(title: row.subject.isEmpty ? "(no subject)" : row.subject, inset: Space.xl) { openThread?(row.id) } trailing: {
                                Text(row.date).textStyle(.caption)
                            }
                            .disabled(openThread == nil)
                        }
                    }
                    .padding(.bottom, Space.xl)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(Tokens.card.color)
        .environment(\.onCard, true)
        .task(id: email) { person = await store.person(email) }
    }

    private func write() {
        let address = Address(name: person?.name.isEmpty == false ? person?.name : nil, email: email)
        guard let closeSheet else {
            store.write(to: address)
            return
        }
        closeSheet()
        // A sheet can't show while another is still going away.
        Task {
            try? await Task.sleep(for: .milliseconds(350))
            store.write(to: address)
        }
    }

    private var name: String {
        guard let person, !person.name.isEmpty else { return email }
        return person.name
    }
}
