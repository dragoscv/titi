import Foundation
import Testing
@testable import TitiFrame

// Vectors produced by `cargo run -p titi-core --bin gen-vectors --features vectors`
// (repo `testvectors/envelope.json`, copied into Tests/TitiFrameTests/vectors by CI).
struct EnvelopeVector: Decodable { let name: String; let input_hex: String; let expected_hex: String; let notes: String }

@Suite struct EnvelopeTests {
    @Test func roundTripBroadcast() {
        let h = EnvelopeHeader(type: .voiceFlood, ttl: 3, hopStart: 3, flags: [], msgId: 0xDEADBEEF, src: Array(repeating: 0xAB, count: 8), dst: nil)
        let b = h.encode(payload: [1, 2, 3])
        #expect(b.count == 19)
        let d = EnvelopeHeader.decode(b)
        #expect(d == h)
        #expect(Array(b[d!.headerLen...]) == [1, 2, 3])
    }

    @Test func roundTripUnicast() {
        let h = EnvelopeHeader(type: .handshake, ttl: 1, hopStart: 1, flags: [.signed, .unicast], msgId: 7, src: Array(0..<8), dst: Array(8..<16))
        let b = h.encode(payload: [])
        #expect(b.count == 24)
        #expect(b[3] & 1 == 1)
        #expect(EnvelopeHeader.decode(b) == h)
    }

    @Test func rejectsBadVersionAndShort() {
        #expect(EnvelopeHeader.decode([2, 0x11, 0, 0, 0, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8]) == nil)
        #expect(EnvelopeHeader.decode([1, 0x11, 0]) == nil)
    }

    @Test func matchesRustVectors() throws {
        guard let url = Bundle.module.url(forResource: "envelope", withExtension: "json", subdirectory: "vectors") else { return }
        let vs = try JSONDecoder().decode([EnvelopeVector].self, from: Data(contentsOf: url))
        for v in vs where v.name.hasPrefix("envelope_") {
            let expected = [UInt8](hex: v.expected_hex)!
            let h = try #require(EnvelopeHeader.decode(expected), "\(v.name)")
            #expect(h.msgId == 0xDEADBEEF, "\(v.name)")
            #expect(h.src == Array(repeating: 0x11, count: 8))
            switch v.name {
            case "envelope_broadcast":
                #expect(h.type == .hello && h.ttl == 7 && h.hopStart == 7 && h.flags == .signed && h.dst == nil)
                #expect(h.encode(payload: [UInt8](hex: v.input_hex)!) == expected)
            case "envelope_unicast":
                #expect(h.type == .control && h.dst == Array(repeating: 0x22, count: 8) && h.flags.contains(.unicast))
                #expect(h.encode(payload: [UInt8](hex: v.input_hex)!) == expected)
            case "envelope_relayed":
                #expect(h.ttl == 6 && h.flags.contains(.relayed))
            default: break
            }
        }
    }
}
