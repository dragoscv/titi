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
            ts.append(BleTransport(nodeId: { h.engine.nodeId() }))
            ts.append(RelayTransport(url: { URL(string: self.settings.relayUrl)! }, nodeId: { h.engine.nodeId() }, displayName: { self.settings.name }, hue: { self.settings.hue }))
            h.start(ts)
            ptt = PttChannel(host: h)
        } catch {
            print("engine boot failed: \(error)")
        }
    }

    func save() { settings.save(); host?.engine.setDisplayName(name: settings.name, hue: UInt16(settings.hue)) }

    func handle(_ url: URL) {
        let s = url.absoluteString
        let link = url.scheme == "titi" ? s : (s.contains("/j/") ? "titi://j/" + s.components(separatedBy: "/j/")[1] : nil)
        guard let link else { return }
        boot()
        host?.joinByLink(link)
        route = .join
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
