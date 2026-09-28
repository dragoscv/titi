// Titi radio envelope header (ADR-0003). Mirrors core/titi-core/src/frame.rs.
// Swift transports need only the header to route; payloads stay opaque.

import Foundation

public let protocolVersion: UInt8 = 1
public let envelopeMin = 16
public let envelopeUnicast = 24

public enum FrameType: UInt8, Sendable {
    case hello = 0x01, announce = 0x02, routeProbe = 0x03, handshake = 0x04, control = 0x05, groupControl = 0x06
    case voiceRouted = 0x10, voiceFlood = 0x11
    case message = 0x20, ack = 0x21, inventory = 0x22, fragment = 0x23
}

public struct Flags: OptionSet, Sendable {
    public let rawValue: UInt8
    public init(rawValue: UInt8) { self.rawValue = rawValue }
    public static let unicast = Flags(rawValue: 0b0000_0001)
    public static let signed = Flags(rawValue: 0b0000_0010)
    public static let relayed = Flags(rawValue: 0b0000_0100)
    public static let urgent = Flags(rawValue: 0b0000_1000)
}

public struct EnvelopeHeader: Sendable, Equatable {
    public var type: FrameType
    public var ttl: UInt8
    public var hopStart: UInt8
    public var flags: Flags
    public var msgId: UInt32
    public var src: [UInt8]      // 8
    public var dst: [UInt8]?     // 8 when unicast
    public var headerLen: Int { dst == nil ? envelopeMin : envelopeUnicast }

    public static func decode(_ b: [UInt8]) -> EnvelopeHeader? {
        guard b.count >= envelopeMin, b[0] == protocolVersion, let t = FrameType(rawValue: b[1]) else { return nil }
        let flags = Flags(rawValue: b[3])
        let unicast = flags.contains(.unicast)
        guard !unicast || b.count >= envelopeUnicast else { return nil }
        let msgId = UInt32(b[4]) << 24 | UInt32(b[5]) << 16 | UInt32(b[6]) << 8 | UInt32(b[7])
        return EnvelopeHeader(type: t, ttl: b[2] >> 4, hopStart: b[2] & 0x0F, flags: flags, msgId: msgId, src: Array(b[8..<16]), dst: unicast ? Array(b[16..<24]) : nil)
    }

    public func encode(payload: [UInt8]) -> [UInt8] {
        var out: [UInt8] = []
        out.reserveCapacity(headerLen + payload.count)
        out.append(protocolVersion)
        out.append(type.rawValue)
        out.append((ttl & 0x0F) << 4 | (hopStart & 0x0F))
        var f = flags.subtracting(.unicast)
        if dst != nil { f.insert(.unicast) }
        out.append(f.rawValue)
        out.append(contentsOf: [UInt8(msgId >> 24), UInt8((msgId >> 16) & 0xFF), UInt8((msgId >> 8) & 0xFF), UInt8(msgId & 0xFF)])
        out.append(contentsOf: src)
        if let d = dst { out.append(contentsOf: d) }
        out.append(contentsOf: payload)
        return out
    }
}

public extension Array where Element == UInt8 {
    var hex: String { map { String(format: "%02x", $0) }.joined() }
    init?(hex: String) {
        guard hex.count % 2 == 0 else { return nil }
        var out: [UInt8] = []
        var i = hex.startIndex
        while i < hex.endIndex {
            let j = hex.index(i, offsetBy: 2)
            guard let b = UInt8(hex[i..<j], radix: 16) else { return nil }
            out.append(b)
            i = j
        }
        self = out
    }
}

/// Relay WebSocket tags (signal.proto header comment).
public enum WsTag { public static let signal: UInt8 = 0x00; public static let envelope: UInt8 = 0x01 }
