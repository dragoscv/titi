import Foundation
import TitiFrame

/// WebSocket link to the Cloud Run relay (ADR-0005). Rooms are 4-byte group
/// hashes (+ rendezvous rooms for join-by-code). Wire: tag 0x00 Signal
/// protobuf / 0x01 raw envelope. Protobuf is hand-encoded for the 4 messages
/// used (same as Android MiniProto); swift-protobuf generation via `buf` is
/// wired in proto/buf.gen.yaml for the rest of the app.
public final class RelayTransport: NSObject, Transport, URLSessionWebSocketDelegate, @unchecked Sendable {
    public let linkId: UInt32 = LinkIds.internet
    public let linkClass: FfiLinkClass = .internet
    public let mtu: UInt32? = nil

    private let url: () -> URL
    private let nodeId: () -> [UInt8]
    private let displayName: () -> String
    private let hue: () -> Int
    private weak var events: (any TransportEvents)?
    private var task: URLSessionWebSocketTask?
    private lazy var session = URLSession(configuration: .default, delegate: self, delegateQueue: nil)
    private var rooms: [String: Bool] = [:]   // hex(hash) → rendezvous
    private var joined: Set<String> = []
    private var resumeToken: [UInt8] = []
    private var peers: Set<String> = []
    private var attempt = 0
    private var enabled = false
    private var linkIsUp = false

    public init(url: @escaping () -> URL, nodeId: @escaping () -> [UInt8], displayName: @escaping () -> String, hue: @escaping () -> Int) {
        self.url = url; self.nodeId = nodeId; self.displayName = displayName; self.hue = hue
        super.init()
    }

    public func start(_ events: any TransportEvents) { self.events = events; enabled = true; connect() }
    public func stop() {
        enabled = false
        task?.cancel(with: .normalClosure, reason: nil); task = nil
        if linkIsUp { linkIsUp = false; events?.linkDown(self) }
    }

    public func setRooms(_ want: [(hash: [UInt8], rendezvous: Bool)]) {
        let m = Dictionary(uniqueKeysWithValues: want.map { ($0.hash.hex, $0.rendezvous) })
        guard m != rooms else { return }
        for k in rooms.keys where m[k] == nil { sendLeave(k) }
        rooms = m
        sendJoins()
    }

    public func send(peer: String?, bytes: [UInt8]) {
        guard linkIsUp, let t = task else { return }
        t.send(.data(Data([WsTag.envelope] + bytes))) { _ in }
    }

    // MARK: connection
    private func connect() {
        guard enabled else { return }
        let t = session.webSocketTask(with: url())
        task = t
        t.resume()
        receive()
    }

    public func urlSession(_ s: URLSession, webSocketTask: URLSessionWebSocketTask, didOpenWithProtocol p: String?) {
        attempt = 0; joined.removeAll(); sendJoins()
    }
    public func urlSession(_ s: URLSession, webSocketTask: URLSessionWebSocketTask, didCloseWith c: URLSessionWebSocketTask.CloseCode, reason: Data?) { onClosed() }
    public func urlSession(_ s: URLSession, task: URLSessionTask, didCompleteWithError e: Error?) { if e != nil { onClosed() } }

    private func onClosed() {
        task = nil
        if linkIsUp { linkIsUp = false; peers.forEach { events?.peerLost(self, token: $0) }; peers.removeAll(); events?.linkDown(self) }
        guard enabled else { return }
        attempt += 1
        let backoff = min(30.0, 0.5 * pow(2.0, Double(min(attempt, 6))))
        DispatchQueue.global().asyncAfter(deadline: .now() + backoff) { [weak self] in self?.connect() }
    }

    private func receive() {
        task?.receive { [weak self] r in
            guard let self else { return }
            if case let .success(msg) = r {
                if case let .data(d) = msg { self.onMessage([UInt8](d)) }
                self.receive()
            }
        }
    }

    // MARK: signals (hand-rolled protobuf, see signal.proto)
    private func sendSignal(_ body: [UInt8]) { task?.send(.data(Data([WsTag.signal] + body))) { _ in } }

    private func sendJoins() {
        for (k, rdv) in rooms where !joined.contains(k) {
            guard let gh = [UInt8](hex: k) else { continue }
            var node = PB(); node.bytes(1, nodeId()); node.string(3, displayName()); node.varint(4, UInt64(hue()))
            var join = PB(); join.bytes(1, gh); join.message(2, node); join.bytes(3, resumeToken); join.varint(4, rdv ? 1 : 0)
            var sig = PB(); sig.message(1, join)
            sendSignal(sig.out); joined.insert(k)
        }
    }
    private func sendLeave(_ k: String) {
        joined.remove(k)
        guard let gh = [UInt8](hex: k) else { return }
        var leave = PB(); leave.bytes(1, gh)
        var sig = PB(); sig.message(3, leave)
        sendSignal(sig.out)
    }

    private func onMessage(_ b: [UInt8]) {
        guard let tag = b.first else { return }
        let body = Array(b.dropFirst())
        if tag == WsTag.envelope {
            guard body.count >= 16 else { return }
            let src = Array(body[8..<16]).hex
            if peers.insert(src).inserted { events?.peerSeen(self, token: src) }
            events?.frame(self, token: src, bytes: body)
            return
        }
        guard tag == WsTag.signal else { return }
        var r = PBReader(body)
        while let f = r.next() {
            switch f {
            case 2: // roomJoined
                var m = r.message()
                while let g = m.next() {
                    switch g {
                    case 1: resumeToken = m.bytes()
                    case 2: var p = m.message(); while let h = p.next() { if h == 1 { let id = p.bytes().hex; if peers.insert(id).inserted { events?.peerSeen(self, token: id) } } else { p.skip() } }
                    default: m.skip()
                    }
                }
                if !linkIsUp { linkIsUp = true; events?.linkUp(self) }
            case 4: // peerEvent
                var m = r.message(); var id: String?; var j = false
                while let g = m.next() {
                    switch g {
                    case 1: var p = m.message(); while let h = p.next() { if h == 1 { id = p.bytes().hex } else { p.skip() } }
                    case 2: j = m.varint() != 0
                    default: m.skip()
                    }
                }
                if let id { if j { if peers.insert(id).inserted { events?.peerSeen(self, token: id) } } else if peers.remove(id) != nil { events?.peerLost(self, token: id) } }
            case 9: // error
                var m = r.message(); var code: UInt64 = 0
                while let g = m.next() { if g == 1 { code = m.varint() } else { m.skip() } }
                if code == 5 { resumeToken = []; joined.removeAll(); sendJoins() }
            default: r.skip()
            }
        }
    }
}

// Minimal protobuf writer/reader.
struct PB {
    var out: [UInt8] = []
    mutating func varint(_ f: Int, _ v: UInt64) { tag(f, 0); vint(v) }
    mutating func bytes(_ f: Int, _ b: [UInt8]) { tag(f, 2); vint(UInt64(b.count)); out += b }
    mutating func string(_ f: Int, _ s: String) { bytes(f, Array(s.utf8)) }
    mutating func message(_ f: Int, _ m: PB) { bytes(f, m.out) }
    private mutating func tag(_ f: Int, _ wt: Int) { vint(UInt64(f << 3 | wt)) }
    private mutating func vint(_ v0: UInt64) { var v = v0; while v >= 0x80 { out.append(UInt8(v & 0x7F) | 0x80); v >>= 7 }; out.append(UInt8(v)) }
}

struct PBReader {
    private let b: [UInt8]; private var pos: Int; private let end: Int; private var wt = 0
    init(_ b: [UInt8], _ pos: Int = 0, _ end: Int? = nil) { self.b = b; self.pos = pos; self.end = end ?? b.count }
    mutating func next() -> Int? { guard pos < end else { return nil }; let t = vint(); wt = Int(t & 7); return Int(t >> 3) }
    mutating func varint() -> UInt64 { vint() }
    mutating func bytes() -> [UInt8] { let n = Int(vint()); let r = Array(b[pos..<min(pos + n, end)]); pos += n; return r }
    mutating func message() -> PBReader { let n = Int(vint()); let r = PBReader(b, pos, min(pos + n, end)); pos += n; return r }
    mutating func skip() { switch wt { case 0: _ = vint(); case 1: pos += 8; case 2: pos += Int(vint()); case 5: pos += 4; default: pos = end } }
    private mutating func vint() -> UInt64 { var s: UInt64 = 0; var r: UInt64 = 0; while pos < end { let x = UInt64(b[pos]); pos += 1; r |= (x & 0x7F) << s; if x & 0x80 == 0 { break }; s += 7 }; return r }
}
