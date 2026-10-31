import XCTest
import TitiCore

/// App-level checks that need the Rust core (run on the iOS Simulator in CI).
final class TitiTests: XCTestCase {
    func testInviteCodeParsing() {
        XCTAssertFalse(parseInviteCode(text: "not a code"))
        XCTAssertFalse(parseInviteCode(text: ""))
    }

    func testLinkUuidAcceptsBothLinkForms() throws {
        let uuid = Data((0..<16).map { UInt8($0) })
        let b64 = uuid.base64EncodedString().replacingOccurrences(of: "+", with: "-").replacingOccurrences(of: "/", with: "_").replacingOccurrences(of: "=", with: "")
        XCTAssertEqual(EngineHost.linkUuid("titi://j/\(b64)/k/1/p/s"), uuid)
        XCTAssertEqual(EngineHost.linkUuid("https://titi.dragoscatalin.ro/j/\(b64)/k/1/p/s"), uuid)
        XCTAssertNil(EngineHost.linkUuid("https://titi.dragoscatalin.ro/privacy"))
    }

    func testEngineBootsAndCreatesAGroup() throws {
        let e = try TitiEngine(seed: Data(), displayName: "ci", avatarHue: 40, rngSeed: 7)
        XCTAssertEqual(e.nodeId().count, 8)
        _ = try e.createGroup(name: "CI", nowMs: 1_700_000_000_000)
        XCTAssertEqual(e.groups().count, 1)
        let link = try XCTUnwrap(try e.deepLink(group: e.groups()[0].id, nowMs: 1_700_000_000_000, validMs: 600_000))
        XCTAssertNotNil(EngineHost.linkUuid(link))
    }
}
