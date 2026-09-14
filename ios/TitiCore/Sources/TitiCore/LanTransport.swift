import Foundation
import Network
import TitiFrame

/// Same-LAN transport over Network.framework: UDP multicast group
/// 239.77.84.84:41414 (same as Android) for HELLO discovery, UDP unicast for
/// everything else. Token = "ip:port". Requires the multicast entitlement.
public final class LanTransport: Transport, @unchecked Sendable {
    public let linkId: UInt32
    public let linkClass: FfiLinkClass
    public let mtu: UInt32? = 1200

    private let queue = DispatchQueue(label: "titi.lan")
    private var group: NWConnectionGroup?
    private var listener: NWListener?
    private var unicast: [String: NWConnection] = [:]
    private var lastSeen: [String: Date] = [:]
    private weak var events: (any TransportEvents)?
    private var expiry: DispatchSourceTimer?
    private let port: NWEndpoint.Port = 41414
    private let mcast = NWEndpoint.hostPort(host: "239.77.84.84", port: 41414)

    public init(linkId: UInt32 = LinkIds.lan, linkClass: FfiLinkClass = .lan) {
        self.linkId = linkId
        self.linkClass = linkClass
    }

    public func start(_ events: any TransportEvents) {
        self.events = events
        let params = NWParameters.udp
        params.requiredInterfaceType = .wifi
        params.allowLocalEndpointReuse = true
        // multicast group for discovery + floods
        guard let desc = try? NWMulticastGroup(for: [mcast]) else { return }
        let g = NWConnectionGroup(with: desc, using: params)
        g.setReceiveHandler(maximumMessageSize: 2048, rejectOversizedMessages: true) { [weak self] msg, content, _ in
            guard let self, let content, let ep = msg.remoteEndpoint else { return }
            self.onDatagram(from: ep, bytes: [UInt8](content))
        }
        g.stateUpdateHandler = { [weak self] st in
            guard let self else { return }
            switch st {
            case .ready: self.events?.linkUp(self)
            case .failed, .cancelled: self.events?.linkDown(self)
            default: break
            }
        }
        g.start(queue: queue)
        group = g
        // unicast listener on the same port (peers send to ip:41414)
        if let l = try? NWListener(using: params, on: port) {
            l.newConnectionHandler = { [weak self] c in
                guard let self else { return }
                let tok = Self.token(c.endpoint)
                c.stateUpdateHandler = { [weak self, weak c] st in
                    if case .cancelled = st { self?.unicast.removeValue(forKey: tok) }
                    if case .failed = st { c?.cancel() }
                }
                self.unicast[tok] = c
                self.receiveLoop(c, token: tok)
                c.start(queue: self.queue)
            }
            l.start(queue: queue)
            listener = l
        }
        let t = DispatchSource.makeTimerSource(queue: queue)
        t.schedule(deadline: .now() + 5, repeating: 5)
        t.setEventHandler { [weak self] in self?.expire() }
        t.resume()
        expiry = t
    }

    public func stop() {
        expiry?.cancel(); expiry = nil
        group?.cancel(); group = nil
        listener?.cancel(); listener = nil
        unicast.values.forEach { $0.cancel() }
        unicast.removeAll()
        events?.linkDown(self)
    }

    public func send(peer: String?, bytes: [UInt8]) {
        let data = Data(bytes)
        queue.async { [self] in
            if let peer {
                let c = unicast[peer] ?? makeUnicast(peer)
                c.send(content: data, completion: .contentProcessed { _ in })
            } else {
                group?.send(content: data) { _ in }
            }
        }
    }

    private func makeUnicast(_ token: String) -> NWConnection {
        let parts = token.split(separator: ":", maxSplits: 1)
        let host = NWEndpoint.Host(String(parts[0]))
        let port = NWEndpoint.Port(rawValue: UInt16(parts.count > 1 ? parts[1] : "41414") ?? 41414) ?? self.port
        let c = NWConnection(host: host, port: port, using: .udp)
        c.stateUpdateHandler = { [weak self] st in if case .failed = st { self?.unicast.removeValue(forKey: token) } }
        unicast[token] = c
        receiveLoop(c, token: token)
        c.start(queue: queue)
        return c
    }

    private func receiveLoop(_ c: NWConnection, token: String) {
        c.receiveMessage { [weak self, weak c] content, _, _, err in
            guard let self, let c, err == nil else { return }
            if let content { self.onDatagram(from: c.endpoint, bytes: [UInt8](content)) }
            self.receiveLoop(c, token: token)
        }
    }

    private func onDatagram(from ep: NWEndpoint, bytes: [UInt8]) {
        let tok = Self.token(ep)
        let first = lastSeen.updateValue(Date(), forKey: tok) == nil
        if first { events?.peerSeen(self, token: tok) }
        events?.frame(self, token: tok, bytes: bytes)
    }

    private func expire() {
        let now = Date()
        for (tok, t) in lastSeen where now.timeIntervalSince(t) > 40 {
            lastSeen.removeValue(forKey: tok)
            unicast.removeValue(forKey: tok)?.cancel()
            events?.peerLost(self, token: tok)
        }
    }

    private static func token(_ ep: NWEndpoint) -> String {
        if case let .hostPort(host, port) = ep {
            var h = "\(host)"
            if let i = h.firstIndex(of: "%") { h = String(h[..<i]) } // strip interface scope
            return "\(h):\(port.rawValue)"
        }
        return "\(ep)"
    }
}
