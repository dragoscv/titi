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

    init(host: EngineHost) {
        self.host = host
        super.init()
        Task { manager = try? await PTChannelManager.channelManager(delegate: self, restorationDelegate: self); joinIfNeeded() }
    }

    func joinIfNeeded() {
        guard let m = manager, let g = host.active else { return }
        let desc = PTChannelDescriptor(name: g.name, image: nil)
        m.requestJoinChannel(channelUUID: channelUUID, descriptor: desc)
    }

    // MARK: PTChannelManagerDelegate
    func channelManager(_ m: PTChannelManager, didJoinChannel u: UUID, reason: PTChannelJoinReason) {}
    func channelManager(_ m: PTChannelManager, didLeaveChannel u: UUID, reason: PTChannelLeaveReason) {}
    func channelManager(_ m: PTChannelManager, channelUUID: UUID, didBeginTransmittingFrom source: PTChannelTransmitRequestSource) { host.pttDown() }
    func channelManager(_ m: PTChannelManager, channelUUID: UUID, didEndTransmittingFrom source: PTChannelTransmitRequestSource) { host.pttUp() }
    func channelManager(_ m: PTChannelManager, receivedEphemeralPushToken token: Data) { /* relay push wake-up: V1.5 */ }
    func incomingPushResult(channelManager: PTChannelManager, channelUUID: UUID, pushPayload: [String: Any]) -> PTPushResult {
        .activeRemoteParticipant(PTParticipant(name: pushPayload["name"] as? String ?? "Titi", image: nil))
    }
    func channelManager(_ m: PTChannelManager, didActivate session: AVAudioSession) { try? host.audio.start() }
    func channelManager(_ m: PTChannelManager, didDeactivate session: AVAudioSession) {}

    // MARK: restoration
    func channelDescriptor(restoredChannelUUID: UUID) -> PTChannelDescriptor { PTChannelDescriptor(name: host.active?.name ?? "Titi", image: nil) }
}

enum Location {
    private static let mgr = CLLocationManager()
    static func current(_ f: @escaping (Double, Double) -> Void) {
        mgr.requestWhenInUseAuthorization()
        if let l = mgr.location { f(l.coordinate.latitude, l.coordinate.longitude) } else { f(0, 0) }
    }
}
