import AppIntents
import Foundation
import TitiCore

// App-only intents (they need AppModel / EngineHost). `TalkIntent`, `EndSosIntent` and
// `SosAttributes` live in ios/Shared because the widget extension uses them too.

/// A Titi group as Shortcuts / Siri / Spotlight see it.
struct GroupEntity: AppEntity {
    static var typeDisplayRepresentation: TypeDisplayRepresentation { TypeDisplayRepresentation(name: "Group") }
    static let defaultQuery = GroupQuery()

    let id: String
    let name: String

    var displayRepresentation: DisplayRepresentation { DisplayRepresentation(title: "\(name)") }
}

struct GroupQuery: EntityQuery {
    init() {}

    @MainActor
    func entities(for identifiers: [GroupEntity.ID]) async throws -> [GroupEntity] {
        await Self.all().filter { identifiers.contains($0.id) }
    }

    @MainActor
    func suggestedEntities() async throws -> [GroupEntity] { await Self.all() }

    /// Groups of the running host; waits briefly after a cold launch while the engine restores them.
    @MainActor
    private static func all() async -> [GroupEntity] {
        guard let model = AppModel.shared else { return [] }
        model.boot()
        guard let host = model.host else { return [] }
        var tries = 0
        while host.groups.isEmpty && tries < 10 {
            try? await Task.sleep(for: .milliseconds(100))
            tries += 1
        }
        return host.groups.map { GroupEntity(id: $0.id, name: $0.name) }
    }
}

/// Makes a group the active one (the one Talk, the Action Button and Control Center use).
struct SwitchGroupIntent: AppIntent {
    static let title: LocalizedStringResource = "Switch group"
    static let description: IntentDescription? = IntentDescription("Makes another group the one you talk to.")
    static let openAppWhenRun: Bool = true

    @Parameter(title: "Group")
    var group: GroupEntity

    init() {}

    static var parameterSummary: some ParameterSummary { Summary("Switch to \(\.$group)") }

    @MainActor
    func perform() async throws -> some IntentResult {
        AppModel.shared?.openGroup(group.id)
        return .result()
    }
}

/// Sends an SOS with your location to the active group, after you confirm.
struct SendSOSIntent: AppIntent {
    static let title: LocalizedStringResource = "Send SOS"
    static let description: IntentDescription? = IntentDescription("Sends an SOS with your location to your active group.")
    static let openAppWhenRun: Bool = true

    init() {}

    @MainActor
    func perform() async throws -> some IntentResult & ProvidesDialog {
        guard let model = AppModel.shared else { return .result(dialog: "Open Titi and pick a group first.") }
        model.boot()
        guard let group = await model.waitForActiveGroup() else { return .result(dialog: "Open Titi and pick a group first.") }
        let name = group.name
        if #available(iOS 18, *) {
            try await requestConfirmation(actionName: .send, dialog: "Send an SOS to everyone in \(name) with your location?")
        } else {
            let ask = IntentResultContainer<Never, Never, Never, IntentDialog>.result(dialog: "Send an SOS to everyone in \(name) with your location?")
            try await requestConfirmation(result: ask, confirmationActionName: .send)
        }
        model.sendSos(group.id)
        return .result(dialog: "SOS sent to \(name).")
    }
}

/// Siri phrases, the Shortcuts app gallery and Spotlight actions. Every phrase names the app.
struct TitiShortcuts: AppShortcutsProvider {
    static var appShortcuts: [AppShortcut] {
        AppShortcut(
            intent: TalkIntent(),
            phrases: [
                "Talk on \(.applicationName)",
                "Start talking on \(.applicationName)",
                "Stop talking on \(.applicationName)",
            ],
            shortTitle: "Talk",
            systemImageName: "mic.fill"
        )
        AppShortcut(
            intent: SwitchGroupIntent(),
            phrases: [
                "Switch \(.applicationName) to \(\.$group)",
                "Switch group in \(.applicationName)",
            ],
            shortTitle: "Switch group",
            systemImageName: "person.3.fill"
        )
        AppShortcut(
            intent: SendSOSIntent(),
            phrases: [
                "Send an SOS with \(.applicationName)",
                "\(.applicationName) SOS",
            ],
            shortTitle: "Send SOS",
            systemImageName: "sos"
        )
    }

    static var shortcutTileColor: ShortcutTileColor { .orange }
}
