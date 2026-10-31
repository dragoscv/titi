import SwiftUI
import TitiCore

@main
struct TitiApp: App {
    @State private var model = AppModel()

    var body: some Scene {
        WindowGroup {
            RootView()
                .environment(model)
                .preferredColorScheme(.dark)
                .tint(Palette.amber)
                .onOpenURL { url in model.handle(url) }
        }
    }
}

/// App-level model: settings + engine host + transports. iOS keeps the radio
/// alive via `UIBackgroundModes audio` (AVAudioSession active) rather than a
/// foreground service; PushToTalk framework is wired in PttChannel.
@Observable
@MainActor
final class AppModel {
    /// The running model, for App Intents (they run in-process with `openAppWhenRun`).
    static weak var shared: AppModel?

    var settings = Settings.load()
    private(set) var host: EngineHost?
    var route: Route = .home
    private var ptt: PttChannel?

    enum Route: Hashable { case home, group(String), join, settings }

    init() {
        Self.shared = self
        Notifications.install()
        IntentBridge.toggleTalk = { AppModel.shared?.toggleTalk() }
        IntentBridge.sosEnded = { SosActivity.endAll() }
        SosActivity.endExpired()
        if settings.onboarded { boot() }
    }

    func boot() {
        guard host == nil else { return }
        do {
            let h = try EngineHost(displayName: settings.name, hue: settings.hue)
            host = h
            var ts: [any Transport] = [LanTransport()]
            ts.append(BleTransport(nodeId: { [UInt8](h.engine.nodeId()) }))
            let relayUrl = URL(string: settings.relayUrl) ?? URL(string: Settings().relayUrl)!
            let name = settings.name, hue = settings.hue
            ts.append(RelayTransport(url: { relayUrl }, nodeId: { [UInt8](h.engine.nodeId()) }, displayName: { name }, hue: { hue }))
            h.start(ts)
            ptt = PttChannel(host: h)
            ptt?.onPushToken = { [weak h] tok in h?.setPushToken(tok, sandbox: AppModel.isSandboxBuild) }
            h.onIncomingSos = { gid, gname, from in
                Notifications.postSos(groupId: gid, groupName: gname, from: from)
            }
        } catch {
            print("engine boot failed: \(error)")
        }
    }

    /// Keeps the system PushToTalk channel on the active group.
    func activeGroupChanged() {
        ptt?.joinIfNeeded()
        if host?.activeGroup != nil { Notifications.requestIfNeeded() }
        TitiShortcuts.updateAppShortcutParameters()
    }

    // MARK: App Intents
    /// Talk intent (Action Button, Control Center, Siri): open the mic, or close it when you are already on air / waiting.
    func toggleTalk() {
        boot()
        guard let host, let g = host.active else { route = .home; return }
        if g.floor == .talking || g.floor == .pending || g.floor == .queued {
            host.pttUp()
        } else {
            host.pttDown()
        }
        route = .group(g.id)
    }

    func openGroup(_ id: String) {
        boot()
        host?.setActiveGroup(id)
        route = .group(id)
    }

    /// Active group once the engine restored its groups after a cold launch (≤ ~1 s).
    func waitForActiveGroup() async -> GroupState? {
        for _ in 0..<10 {
            if let g = host?.active { return g }
            try? await Task.sleep(for: .milliseconds(100))
        }
        return host?.active
    }

    /// SOS with the current position, plus the Lock Screen Live Activity. Used by GroupView and SendSOSIntent.
    func sendSos(_ gid: String) {
        guard let host else { return }
        Notifications.requestIfNeeded()
        let name = host.groups.first { $0.id == gid }?.name ?? ""
        Location.current { lat, lon in host.sendSos(gid, lat: lat, lon: lon) }
        SosActivity.start(group: name)
    }

    /// Development-signed builds get APNs sandbox tokens; TestFlight/App Store get production ones.
    static let isSandboxBuild: Bool = {
        guard let url = Bundle.main.url(forResource: "embedded", withExtension: "mobileprovision"),
              let s = try? String(contentsOf: url, encoding: .isoLatin1) else { return false }
        return s.range(of: "<key>aps-environment</key>\\s*<string>development</string>", options: .regularExpression) != nil
    }()

    func save() { settings.save(); host?.engine.setDisplayName(name: settings.name, hue: UInt16(settings.hue)) }

    func handle(_ url: URL) {
        // the core parses titi://j/… and https://titi.dragoscatalin.ro/j/… itself
        guard url.absoluteString.contains("/j/") || url.scheme == "titi" else { return }
        boot()
        host?.joinByLink(url.absoluteString)
        route = .join
    }

    /// Settings › Delete identity & all data: back to a fresh install.
    func wipeAll() {
        SosActivity.endAll()
        host?.wipeAll()
        host = nil
        ptt = nil
        UserDefaults.standard.removeObject(forKey: "settings")
        settings = Settings()
        route = .home
    }
}

struct Settings: Codable {
    var name = ""
    var hue = 40
    var onboarded = false
    var relayUrl = "wss://titi-relay-x3clqgvrdq-ew.a.run.app/v1/ws"
    var volumePtt = true

    static func load() -> Settings {
        guard let d = UserDefaults.standard.data(forKey: "settings"), let s = try? JSONDecoder().decode(Settings.self, from: d) else { return Settings() }
        return s
    }
    func save() { UserDefaults.standard.set(try? JSONEncoder().encode(self), forKey: "settings") }
}
