import CoreBluetooth
import Foundation
import TitiFrame

/// BLE L2CAP CoC transport (ADR-0002), interoperable with Android's
/// BleTransport: same service UUID, PSM characteristic, node id in the
/// scan response (iOS: service data), 8-byte node-id preamble from the
/// initiator, 2-byte length-prefixed frames. Peers keyed by node id (hex).
public final class BleTransport: NSObject, Transport, @unchecked Sendable {
    public let linkId: UInt32 = LinkIds.bleL2cap
    public let linkClass: FfiLinkClass = .bleL2cap
    public let mtu: UInt32? = 1000

    static let service = CBUUID(string: "74697469-0001-4000-8000-746974690001")
    static let chPsm = CBUUID(string: "74697469-0002-4000-8000-746974690001")
    static let chNode = CBUUID(string: "74697469-0003-4000-8000-746974690001")

    private let nodeId: () -> [UInt8]
    private weak var events: (any TransportEvents)?
    private let queue = DispatchQueue(label: "titi.ble")
    private var central: CBCentralManager!
    private var peripheral: CBPeripheralManager!
    private var psm: CBL2CAPPSM = 0
    private var peers: [String: Peer] = [:]            // node hex → peer
    private var connecting: [UUID: (CBPeripheral, String)] = [:]
    private var pendingGatt: [UUID: CBPeripheral] = [:]

    private final class Peer {
        let token: String
        let channel: CBL2CAPChannel
        var inBuf: [UInt8] = []
        init(token: String, channel: CBL2CAPChannel) { self.token = token; self.channel = channel }
    }

    public init(nodeId: @escaping () -> [UInt8]) {
        self.nodeId = nodeId
        super.init()
    }

    public func start(_ events: any TransportEvents) {
        self.events = events
        central = CBCentralManager(delegate: self, queue: queue, options: [CBCentralManagerOptionShowPowerAlertKey: false])
        peripheral = CBPeripheralManager(delegate: self, queue: queue)
    }

    public func stop() {
        central?.stopScan()
        peripheral?.stopAdvertising()
        if psm != 0 { peripheral?.unpublishL2CAPChannel(psm) }
        for p in peers.values { p.channel.inputStream.close(); p.channel.outputStream.close() }
        peers.removeAll()
        events?.linkDown(self)
    }

    public func send(peer: String?, bytes: [UInt8]) {
        queue.async { [self] in
            let targets = peer.map { peers[$0].map { [$0] } ?? [] } ?? Array(peers.values)
            var framed = [UInt8(bytes.count >> 8), UInt8(bytes.count & 0xFF)]
            framed.append(contentsOf: bytes)
            for p in targets { _ = framed.withUnsafeBufferPointer { p.channel.outputStream.write($0.baseAddress!, maxLength: framed.count) } }
        }
    }

    private func attach(token: String, channel: CBL2CAPChannel) {
        let p = Peer(token: token, channel: channel)
        channel.inputStream.delegate = self
        channel.inputStream.schedule(in: .main, forMode: .default)
        channel.inputStream.open()
        channel.outputStream.open()
        peers[token] = p
        events?.peerSeen(self, token: token)
    }

    private func drop(_ p: Peer) {
        p.channel.inputStream.close(); p.channel.outputStream.close()
        if peers.removeValue(forKey: p.token) != nil { events?.peerLost(self, token: p.token) }
    }

    private static func compare(_ a: [UInt8], _ b: [UInt8]) -> Int {
        for (x, y) in zip(a, b) where x != y { return Int(x) - Int(y) }
        return a.count - b.count
    }
}

// MARK: central (initiator when our node id is smaller)
extension BleTransport: CBCentralManagerDelegate, CBPeripheralDelegate {
    public func centralManagerDidUpdateState(_ c: CBCentralManager) {
        guard c.state == .poweredOn else { return }
        c.scanForPeripherals(withServices: [Self.service], options: [CBCentralManagerScanOptionAllowDuplicatesKey: false])
    }

    public func centralManager(_ c: CBCentralManager, didDiscover p: CBPeripheral, advertisementData: [String: Any], rssi: NSNumber) {
        guard let sd = advertisementData[CBAdvertisementDataServiceDataKey] as? [CBUUID: Data], let node = sd[Self.service], node.count == 8 else { return }
        let token = [UInt8](node).hex
        if peers[token] != nil || connecting.values.contains(where: { $0.1 == token }) { return }
        guard Self.compare(nodeId(), [UInt8](node)) < 0 else { return } // glare: smaller id initiates
        connecting[p.identifier] = (p, token)
        p.delegate = self
        c.connect(p)
    }

    public func centralManager(_ c: CBCentralManager, didConnect p: CBPeripheral) {
        p.discoverServices([Self.service])
    }

    public func centralManager(_ c: CBCentralManager, didFailToConnect p: CBPeripheral, error: Error?) {
        connecting.removeValue(forKey: p.identifier)
    }

    public func peripheral(_ p: CBPeripheral, didDiscoverServices error: Error?) {
        guard let s = p.services?.first(where: { $0.uuid == Self.service }) else { return }
        p.discoverCharacteristics([Self.chPsm], for: s)
    }

    public func peripheral(_ p: CBPeripheral, didDiscoverCharacteristicsFor s: CBService, error: Error?) {
        guard let ch = s.characteristics?.first(where: { $0.uuid == Self.chPsm }) else { return }
        p.readValue(for: ch)
    }

    public func peripheral(_ p: CBPeripheral, didUpdateValueFor ch: CBCharacteristic, error: Error?) {
        guard ch.uuid == Self.chPsm, let v = ch.value, v.count == 2 else { return }
        let psm = CBL2CAPPSM(UInt16(v[0]) << 8 | UInt16(v[1]))
        p.openL2CAPChannel(psm)
    }

    public func peripheral(_ p: CBPeripheral, didOpen channel: CBL2CAPChannel?, error: Error?) {
        guard let channel, let (_, token) = connecting.removeValue(forKey: p.identifier) else { return }
        // initiator preamble: our node id
        let id = nodeId()
        channel.outputStream.open()
        _ = id.withUnsafeBufferPointer { channel.outputStream.write($0.baseAddress!, maxLength: 8) }
        attach(token: token, channel: channel)
        pendingGatt[p.identifier] = p // keep the CBPeripheral alive while the channel lives
    }
}

// MARK: peripheral (advertise + accept)
extension BleTransport: CBPeripheralManagerDelegate {
    public func peripheralManagerDidUpdateState(_ pm: CBPeripheralManager) {
        guard pm.state == .poweredOn else { return }
        let svc = CBMutableService(type: Self.service, primary: true)
        let psmCh = CBMutableCharacteristic(type: Self.chPsm, properties: .read, value: nil, permissions: .readable)
        let nodeCh = CBMutableCharacteristic(type: Self.chNode, properties: .read, value: Data(nodeId()), permissions: .readable)
        svc.characteristics = [psmCh, nodeCh]
        pm.add(svc)
        pm.publishL2CAPChannel(withEncryption: false)
    }

    public func peripheralManager(_ pm: CBPeripheralManager, didPublishL2CAPChannel psm: CBL2CAPPSM, error: Error?) {
        self.psm = psm
        // iOS puts service data in the scan response automatically when the primary payload is full
        pm.startAdvertising([CBAdvertisementDataServiceUUIDsKey: [Self.service], CBAdvertisementDataServiceDataKey: [Self.service: Data(nodeId())]])
        events?.linkUp(self)
    }

    public func peripheralManager(_ pm: CBPeripheralManager, didReceiveRead req: CBATTRequest) {
        if req.characteristic.uuid == Self.chPsm { req.value = Data([UInt8(psm >> 8), UInt8(psm & 0xFF)]) }
        else if req.characteristic.uuid == Self.chNode { req.value = Data(nodeId()) }
        pm.respond(to: req, withResult: .success)
    }

    public func peripheralManager(_ pm: CBPeripheralManager, didOpen channel: CBL2CAPChannel?, error: Error?) {
        guard let channel else { return }
        // acceptor: read the 8-byte node-id preamble first, then attach
        channel.inputStream.open(); channel.outputStream.open()
        var id = [UInt8](repeating: 0, count: 8)
        var got = 0
        let deadline = Date().addingTimeInterval(3)
        while got < 8, Date() < deadline {
            if channel.inputStream.hasBytesAvailable {
                let n = id.withUnsafeMutableBufferPointer { channel.inputStream.read($0.baseAddress! + got, maxLength: 8 - got) }
                if n <= 0 { break }
                got += n
            } else { Thread.sleep(forTimeInterval: 0.01) }
        }
        guard got == 8 else { channel.inputStream.close(); channel.outputStream.close(); return }
        attach(token: id.hex, channel: channel)
    }
}

// MARK: stream input (length-prefixed frames)
extension BleTransport: StreamDelegate {
    public func stream(_ s: Stream, handle e: Stream.Event) {
        guard let p = peers.values.first(where: { $0.channel.inputStream === s }) else { return }
        switch e {
        case .hasBytesAvailable:
            var buf = [UInt8](repeating: 0, count: 4096)
            let n = buf.withUnsafeMutableBufferPointer { p.channel.inputStream.read($0.baseAddress!, maxLength: 4096) }
            guard n > 0 else { return }
            p.inBuf.append(contentsOf: buf[0..<n])
            while p.inBuf.count >= 2 {
                let len = Int(p.inBuf[0]) << 8 | Int(p.inBuf[1])
                guard len > 0, len <= 4096 else { drop(p); return }
                guard p.inBuf.count >= 2 + len else { break }
                let frame = Array(p.inBuf[2..<(2 + len)])
                p.inBuf.removeFirst(2 + len)
                events?.frame(self, token: p.token, bytes: frame)
            }
        case .endEncountered, .errorOccurred:
            drop(p)
        default: break
        }
    }
}
