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
    var settings = Settings.load()
    private(set) var host: EngineHost?
    var route: Route = .home
    private var ptt: PttChannel?

    enum Route: Hashable { case home, group(String), join, settings }

    init() {
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
        } catch {
            print("engine boot failed: \(error)")
        }
    }

    /// Keeps the system PushToTalk channel on the active group.
    func activeGroupChanged() { ptt?.joinIfNeeded() }

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
