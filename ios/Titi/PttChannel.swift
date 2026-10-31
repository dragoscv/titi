import AVFoundation
import CoreLocation
import Foundation
import PushToTalk
import TitiCore

/// iOS PushToTalk framework integration: gives the system PTT pill, lets the
/// user transmit from the lock screen / with the screen off, and keeps the
/// audio session eligible in the background. The framework's own audio
/// session activation is honoured by AudioEngine (`.playAndRecord`).
final class PttChannel: NSObject, PTChannelManagerDelegate, PTChannelRestorationDelegate, @unchecked Sendable {
    private var manager: PTChannelManager?
    private let host: EngineHost
    private let channelUUID = UUID(uuidString: "74697469-7074-7400-0000-000000000001")!
    private var joinedName: String?
    /// Ephemeral APNs push-to-talk token of the joined channel (hex), registered with the relay.
    private(set) var pushToken: String?
    var onPushToken: ((String?) -> Void)?

    init(host: EngineHost) {
        self.host = host
        super.init()
        Task { @MainActor in
            manager = try? await PTChannelManager.channelManager(delegate: self, restorationDelegate: self)
            joinIfNeeded()
        }
    }

    /// The system PTT channel mirrors the active group: joined on the first group, renamed on a switch.
    @MainActor func joinIfNeeded() {
        guard let m = manager, let g = host.active else { return }
        let desc = PTChannelDescriptor(name: g.name, image: nil)
        if joinedName == nil {
            m.requestJoinChannel(channelUUID: channelUUID, descriptor: desc)
            joinedName = g.name
        } else if joinedName != g.name {
            Task { try? await m.setChannelDescriptor(desc, channelUUID: channelUUID) }
            joinedName = g.name
        }
    }

    // MARK: PTChannelManagerDelegate
    func channelManager(_ m: PTChannelManager, didJoinChannel u: UUID, reason: PTChannelJoinReason) {}
    func channelManager(_ m: PTChannelManager, didLeaveChannel u: UUID, reason: PTChannelLeaveReason) {
        Task { @MainActor in joinedName = nil; pushToken = nil; onPushToken?(nil) }
    }
    func channelManager(_ m: PTChannelManager, channelUUID: UUID, didBeginTransmittingFrom source: PTChannelTransmitRequestSource) { host.pttDown() }
    func channelManager(_ m: PTChannelManager, channelUUID: UUID, didEndTransmittingFrom source: PTChannelTransmitRequestSource) { host.pttUp() }
    func channelManager(_ m: PTChannelManager, receivedEphemeralPushToken token: Data) {
        let hex = token.map { String(format: "%02x", $0) }.joined()
        Task { @MainActor in pushToken = hex; onPushToken?(hex) }
    }
    func incomingPushResult(channelManager: PTChannelManager, channelUUID: UUID, pushPayload: [String: Any]) -> PTPushResult {
        .activeRemoteParticipant(PTParticipant(name: pushPayload["name"] as? String ?? "Titi", image: nil))
    }
    func channelManager(_ m: PTChannelManager, didActivate session: AVAudioSession) { try? host.audio.start() }
    func channelManager(_ m: PTChannelManager, didDeactivate session: AVAudioSession) {}

    // MARK: restoration
    func channelDescriptor(restoredChannelUUID: UUID) -> PTChannelDescriptor { PTChannelDescriptor(name: host.active?.name ?? "Titi", image: nil) }
}

/// One-shot position for SOS / location share (falls back to 0,0 after 6 s or on denial).
@MainActor
final class Location: NSObject, CLLocationManagerDelegate {
    private static let shared = Location()
    private let mgr = CLLocationManager()
    private var waiting: [(Double, Double) -> Void] = []

    override private init() { super.init(); mgr.delegate = self; mgr.desiredAccuracy = kCLLocationAccuracyNearestTenMeters }

    static func current(_ f: @escaping (Double, Double) -> Void) { shared.request(f) }

    private func request(_ f: @escaping (Double, Double) -> Void) {
        waiting.append(f)
        guard waiting.count == 1 else { return }
        if mgr.authorizationStatus == .notDetermined { mgr.requestWhenInUseAuthorization() } else { mgr.requestLocation() }
        Task { try? await Task.sleep(for: .seconds(6)); self.finish(nil) }
    }

    private func finish(_ l: CLLocation?) {
        let w = waiting; waiting = []
        let c = l?.coordinate
        w.forEach { $0(c?.latitude ?? 0, c?.longitude ?? 0) }
    }

    nonisolated func locationManagerDidChangeAuthorization(_ m: CLLocationManager) {
        Task { @MainActor in
            guard !waiting.isEmpty else { return }
            switch m.authorizationStatus {
            case .authorizedWhenInUse, .authorizedAlways: m.requestLocation()
            case .denied, .restricted: finish(nil)
            default: break
            }
        }
    }
    nonisolated func locationManager(_ m: CLLocationManager, didUpdateLocations ls: [CLLocation]) { let l = ls.last; Task { @MainActor in finish(l) } }
    nonisolated func locationManager(_ m: CLLocationManager, didFailWithError e: Error) { Task { @MainActor in finish(nil) } }
}
