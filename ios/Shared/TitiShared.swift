// Compiled into BOTH the Titi app and the TitiWidgets extension (project.yml sources).
// Keep this file free of TitiCore / app-only types: the extension links neither.
import ActivityKit
import AppIntents
import Foundation

/// The app installs these at launch. Intents below run in the APP process
/// (`openAppWhenRun` / LiveActivityIntent), so in the extension they stay nil and are never called.
@MainActor
enum IntentBridge {
    static var toggleTalk: (@MainActor () -> Void)?
    static var sosEnded: (@MainActor () -> Void)?
}

/// Action Button / Control Center / Shortcuts: first run opens the mic on the active group, the next run closes it.
struct TalkIntent: AppIntent {
    static let title: LocalizedStringResource = "Talk"
    static let description: IntentDescription? = IntentDescription("Starts or stops talking to your active group.")
    static let openAppWhenRun: Bool = true

    init() {}

    @MainActor
    func perform() async throws -> some IntentResult {
        IntentBridge.toggleTalk?()
        return .result()
    }
}

/// Live Activity shown on the Lock Screen and in the Dynamic Island after you send an SOS.
struct SosAttributes: ActivityAttributes {
    struct ContentState: Codable, Hashable {
        var startedAt: Date
        var status: String
    }

    var groupName: String
}

/// "Hide" button on the SOS Live Activity.
struct EndSosIntent: LiveActivityIntent {
    static let title: LocalizedStringResource = "End SOS"
    static let description: IntentDescription? = IntentDescription("Removes the SOS from your Lock Screen.")

    init() {}

    func perform() async throws -> some IntentResult {
        for activity in Activity<SosAttributes>.activities {
            await activity.end(nil, dismissalPolicy: .immediate)
        }
        await MainActor.run { IntentBridge.sosEnded?() }
        return .result()
    }
}

/// How long an SOS stays on the Lock Screen.
enum SosLive {
    static let duration: TimeInterval = 15 * 60
}
