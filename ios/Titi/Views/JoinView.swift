import AVFoundation
import SwiftUI
import TitiCore

struct JoinView: View {
    @Environment(AppModel.self) private var model
    @Environment(EngineHost.self) private var host
    @State private var code = ""
    @State private var searching = false
    @State private var scanning = false

    private var norm: String { code.lowercased().trimmingCharacters(in: .whitespaces).replacingOccurrences(of: " ", with: "-") }
    private var valid: Bool { !norm.isEmpty && parseInviteCode(text: norm) }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 4) {
                Button { withAnimation(.titi) { model.route = .home } } label: { Image(systemName: "chevron.left").font(.title3).padding(12) }.foregroundStyle(Palette.muted)
                Text("Join a group").font(.title3.weight(.semibold))
            }.padding(.horizontal, 8).padding(.top, 4)
            if scanning {
                QrScanner { text in
                    scanning = false
                    if text.hasPrefix("titi://") || text.contains("/j/") { host.joinByLink(text.hasPrefix("http") ? "titi://j/" + text.components(separatedBy: "/j/")[1] : text); searching = true } else { code = text }
                }
                Button("Cancel") { scanning = false }.buttonStyle(OutlineButton()).padding(20)
            } else {
                VStack(alignment: .leading, spacing: 16) {
                    Text("Ask the host for the three words and two digits on their screen. You must be near each other — same Wi‑Fi, hotspot, Bluetooth range — or both online.")
                        .foregroundStyle(Palette.muted)
                    TextField("word word word 00", text: $code).font(.system(size: 26, design: .monospaced)).textInputAutocapitalization(.never).autocorrectionDisabled()
                        .padding(16).background(Palette.surface, in: RoundedRectangle(cornerRadius: 18, style: .continuous)).overlay(RoundedRectangle(cornerRadius: 18, style: .continuous).stroke(Palette.outline))
                    HStack(spacing: 12) {
                        Button { scanning = true } label: { Label("Scan QR", systemImage: "qrcode.viewfinder") }.buttonStyle(OutlineButton())
                        Button("Join") { host.joinByCode(norm); searching = true }.buttonStyle(PrimaryButton()).disabled(!valid || searching)
                    }
                    if searching {
                        HStack(spacing: 12) { ProgressView().tint(Palette.amber); Text("Looking for the group…") }.font(.subheadline).padding(.top, 12)
                        if host.peers.isEmpty { Text("No phone reachable yet. Make sure Wi‑Fi / Bluetooth are on, or that the host is online.").font(.subheadline).foregroundStyle(Palette.muted) }
                    }
                }.padding(24)
            }
            Spacer()
        }
        .onChange(of: host.activeGroup) { _, a in if searching, let a { withAnimation(.titi) { model.route = .group(a) } } }
    }
}

/// AVFoundation QR scanner (VisionKit DataScanner is fine too; this one has no
/// device-capability restriction).
struct QrScanner: UIViewControllerRepresentable {
    let onResult: (String) -> Void
    func makeUIViewController(context: Context) -> Controller { let c = Controller(); c.onResult = onResult; return c }
    func updateUIViewController(_ c: Controller, context: Context) {}

    final class Controller: UIViewController, AVCaptureMetadataOutputObjectsDelegate {
        var onResult: ((String) -> Void)?
        private let session = AVCaptureSession()
        private var done = false
        override func viewDidLoad() {
            super.viewDidLoad()
            view.backgroundColor = .black
            guard let dev = AVCaptureDevice.default(for: .video), let input = try? AVCaptureDeviceInput(device: dev) else { return }
            session.addInput(input)
            let out = AVCaptureMetadataOutput(); session.addOutput(out)
            out.setMetadataObjectsDelegate(self, queue: .main); out.metadataObjectTypes = [.qr]
            let layer = AVCaptureVideoPreviewLayer(session: session); layer.videoGravity = .resizeAspectFill; layer.frame = view.bounds; view.layer.addSublayer(layer)
            DispatchQueue.global().async { self.session.startRunning() }
        }
        override func viewDidDisappear(_ a: Bool) { super.viewDidDisappear(a); session.stopRunning() }
        func metadataOutput(_ o: AVCaptureMetadataOutput, didOutput objs: [AVMetadataObject], from c: AVCaptureConnection) {
            guard !done, let s = (objs.first as? AVMetadataMachineReadableCodeObject)?.stringValue else { return }
            done = true; onResult?(s)
        }
    }
}
