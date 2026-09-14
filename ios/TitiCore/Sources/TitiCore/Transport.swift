import Foundation
import TitiFrame

/// One engine link. Dumb pipe: bytes in/out + peer presence. All protocol
/// logic lives in the Rust core (see TitiEngine).
public protocol Transport: AnyObject, Sendable {
    var linkId: UInt32 { get }
    var linkClass: FfiLinkClass { get }
    var mtu: UInt32? { get }
    func start(_ events: any TransportEvents)
    func stop()
    /// peer == nil → broadcast to every connected peer on this link.
    func send(peer: String?, bytes: [UInt8])
}

public protocol TransportEvents: AnyObject, Sendable {
    func linkUp(_ t: any Transport)
    func linkDown(_ t: any Transport)
    func peerSeen(_ t: any Transport, token: String)
    func peerLost(_ t: any Transport, token: String)
    func frame(_ t: any Transport, token: String, bytes: [UInt8])
}

/// Fixed link ids (same numbering as Android `LinkIds`).
public enum LinkIds {
    public static let lan: UInt32 = 1
    public static let hotspot: UInt32 = 2
    public static let wifiAware: UInt32 = 3
    public static let bleL2cap: UInt32 = 5
    public static let internet: UInt32 = 8
}
