import Foundation
import UserNotifications

/// Local notifications: time-sensitive SOS alerts from other members.
/// Permission is asked lazily (first active group or first SOS), never at launch.
@MainActor
enum Notifications {
    private static let delegate = NotificationDelegate()
    private static var asked = false

    static func install() { UNUserNotificationCenter.current().delegate = delegate }

    static func requestIfNeeded() {
        guard !asked else { return }
        asked = true
        Task {
            let center = UNUserNotificationCenter.current()
            let settings = await center.notificationSettings()
            guard settings.authorizationStatus == .notDetermined else { return }
            _ = try? await center.requestAuthorization(options: [.alert, .sound, .badge])
        }
    }

    static func postSos(groupId: String, groupName: String, from name: String) {
        let c = UNMutableNotificationContent()
        c.title = String(localized: "SOS from \(name)")
        c.body = groupName.isEmpty
            ? String(localized: "Open Titi to see where they are.")
            : String(localized: "In \(groupName) · open Titi to see where they are.")
        c.sound = .default
        c.interruptionLevel = .timeSensitive
        c.relevanceScore = 1
        c.threadIdentifier = "sos-\(groupId)"
        c.userInfo = ["group": groupId]
        let request = UNNotificationRequest(identifier: UUID().uuidString, content: c, trigger: nil)
        UNUserNotificationCenter.current().add(request) { error in
            if let error { print("SOS notification failed: \(error)") }
        }
    }
}

/// Shows SOS banners while Titi is open too, and opens the group when one is tapped.
final class NotificationDelegate: NSObject, UNUserNotificationCenterDelegate {
    func userNotificationCenter(_ center: UNUserNotificationCenter, willPresent notification: UNNotification,
                                withCompletionHandler completionHandler: @escaping @Sendable (UNNotificationPresentationOptions) -> Void) {
        completionHandler([.banner, .list, .sound])
    }

    func userNotificationCenter(_ center: UNUserNotificationCenter, didReceive response: UNNotificationResponse,
                                withCompletionHandler completionHandler: @escaping @Sendable () -> Void) {
        let group = response.notification.request.content.userInfo["group"] as? String
        Task { @MainActor in
            if let group { AppModel.shared?.openGroup(group) }
        }
        completionHandler()
    }
}
