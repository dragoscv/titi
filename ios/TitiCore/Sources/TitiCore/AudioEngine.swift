import AVFoundation
import Foundation

/// 48 kHz mono Int16 in/out in 20 ms ticks, mirroring Android AudioEngine.
/// `.playAndRecord` + `.voiceChat` gives hardware AEC/NS and BT routing;
/// `UIBackgroundModes audio` keeps the session alive with the screen off.
public final class AudioEngine: @unchecked Sendable {
    public var onPcm: (([Int16], UInt64) -> Void)?
    public var muted = false

    private let engine = AVAudioEngine()
    private let player = AVAudioPlayerNode()
    private let fmt = AVAudioFormat(commonFormat: .pcmFormatInt16, sampleRate: 48_000, channels: 1, interleaved: true)!
    private let floatFmt = AVAudioFormat(standardFormatWithSampleRate: 48_000, channels: 1)!
    private var capturing = false
    private var running = false
    private var accum: [Int16] = []
    private var ring: [Int16] = []
    private let ringLock = NSLock()
    private var converter: AVAudioConverter?

    public init() {}

    public func start() throws {
        guard !running else { return }
        let s = AVAudioSession.sharedInstance()
        try s.setCategory(.playAndRecord, mode: .voiceChat, options: [.allowBluetooth, .allowBluetoothA2DP, .defaultToSpeaker])
        try s.setPreferredSampleRate(48_000)
        try s.setPreferredIOBufferDuration(0.02)
        try s.setActive(true)
        engine.attach(player)
        engine.connect(player, to: engine.mainMixerNode, format: floatFmt)
        // pull-based playout: feed 20 ms buffers from the ring on a timer
        try engine.start()
        player.play()
        running = true
        scheduleNext()
    }

    public func stop() {
        stopCapture()
        player.stop(); engine.stop()
        try? AVAudioSession.sharedInstance().setActive(false)
        running = false
    }

    public func startCapture() {
        guard running, !capturing else { return }
        let input = engine.inputNode
        let inFmt = input.outputFormat(forBus: 0)
        converter = AVAudioConverter(from: inFmt, to: fmt)
        input.installTap(onBus: 0, bufferSize: 960, format: inFmt) { [weak self] buf, _ in
            guard let self, let conv = self.converter else { return }
            let cap = AVAudioFrameCount(Double(buf.frameLength) * 48_000 / inFmt.sampleRate) + 16
            guard let out = AVAudioPCMBuffer(pcmFormat: self.fmt, frameCapacity: cap) else { return }
            var err: NSError?
            var consumed = false
            conv.convert(to: out, error: &err) { _, st in if consumed { st.pointee = .noDataNow; return nil }; consumed = true; st.pointee = .haveData; return buf }
            guard err == nil, let p = out.int16ChannelData else { return }
            var pcm = Array(UnsafeBufferPointer(start: p[0], count: Int(out.frameLength)))
            if self.muted { pcm = [Int16](repeating: 0, count: pcm.count) }
            self.accum.append(contentsOf: pcm)
            while self.accum.count >= 960 {
                let frame = Array(self.accum[0..<960]); self.accum.removeFirst(960)
                self.onPcm?(frame, UInt64(Date().timeIntervalSince1970 * 1000))
            }
        }
        capturing = true
    }

    public func stopCapture() {
        guard capturing else { return }
        engine.inputNode.removeTap(onBus: 0)
        capturing = false
        accum.removeAll()
    }

    /// Called from the engine thread with 20 ms of mixed PCM.
    public func play(_ pcm: [Int16]) {
        ringLock.lock(); ring.append(contentsOf: pcm); if ring.count > 48_000 * 2 { ring.removeFirst(ring.count - 48_000 * 2) }; ringLock.unlock()
    }

    private func scheduleNext() {
        guard running else { return }
        ringLock.lock()
        let n = min(960, ring.count)
        let chunk = Array(ring.prefix(n)); ring.removeFirst(n)
        ringLock.unlock()
        if let buf = AVAudioPCMBuffer(pcmFormat: floatFmt, frameCapacity: 960) {
            buf.frameLength = 960
            let f = buf.floatChannelData![0]
            for i in 0..<960 { f[i] = i < n ? Float(chunk[i]) / 32768 : 0 }
            player.scheduleBuffer(buf) { [weak self] in self?.scheduleNext() }
        }
    }
}
