import ActivityKit
import Foundation

/// Lock Screen / Dynamic Island Live Activity after you send an SOS. The widget
/// extension that draws it targets iOS 18, so the activity is only requested there.
@MainActor
enum SosActivity {
    private static var endTask: Task<Void, Never>?

    static func start(group: String) {
        guard #available(iOS 18, *) else { return }
        guard ActivityAuthorizationInfo().areActivitiesEnabled else { return }
        let previous = Activity<SosAttributes>.activities
        let now = Date()
        let state = SosAttributes.ContentState(startedAt: now, status: String(localized: "Your group has your location"))
        let content = ActivityContent(state: state, staleDate: now.addingTimeInterval(SosLive.duration))
        do {
            _ = try Activity<SosAttributes>.request(attributes: SosAttributes(groupName: group), content: content, pushType: nil)
        } catch {
            print("SOS live activity failed: \(error)")
        }
        end(previous)
        endTask?.cancel()
        endTask = Task {
            try? await Task.sleep(for: .seconds(SosLive.duration))
            guard !Task.isCancelled else { return }
            endAll()
        }
    }

    /// The user hid it (Live Activity button) or wiped the app.
    static func endAll() {
        endTask?.cancel()
        endTask = nil
        end(Activity<SosAttributes>.activities)
    }

    /// Ends activities older than the SOS window (the app may have been suspended when the timer was due).
    static func endExpired() {
        let cutoff = Date().addingTimeInterval(-SosLive.duration)
        end(Activity<SosAttributes>.activities.filter { $0.content.state.startedAt < cutoff })
    }

    private static func end(_ list: [Activity<SosAttributes>]) {
        guard !list.isEmpty else { return }
        Task {
            for activity in list { await activity.end(nil, dismissalPolicy: .immediate) }
        }
    }
}
