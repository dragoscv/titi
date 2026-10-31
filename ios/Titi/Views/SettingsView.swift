import SwiftUI
import TitiCore

struct SettingsView: View {
    @Environment(AppModel.self) private var model
    @Environment(EngineHost.self) private var host
    private let hues = [40, 15, 350, 290, 220, 170, 120, 80]
    @State private var confirmWipe = false
    @AppStorage("word_filter") private var wordFilter = true

    var body: some View {
        @Bindable var m = model
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 4) {
                Button { withAnimation(.titi) { model.route = .home } } label: { Image(systemName: "chevron.left").font(.title3).padding(12) }.foregroundStyle(Palette.muted)
                Text("Settings").font(.title3.weight(.semibold))
            }.padding(.horizontal, 8).padding(.top, 4)
            ScrollView {
                VStack(spacing: 12) {
                    Card {
                        VStack(alignment: .leading, spacing: 10) {
                            Text("Profile").font(.subheadline.weight(.semibold)).foregroundStyle(Palette.amber)
                            TextField("Name", text: $m.settings.name).padding(12).background(Palette.graphite, in: RoundedRectangle(cornerRadius: 12)).onSubmit { model.save() }
                            HStack(spacing: 10) { ForEach(hues, id: \.self) { h in Button { model.settings.hue = h; model.save() } label: { Circle().fill(hueColor(h)).frame(width: 36, height: 36).overlay { if h == model.settings.hue { Circle().stroke(Palette.ink, lineWidth: 3) } } } } }
                        }
                    }
                    Card {
                        VStack(alignment: .leading, spacing: 10) {
                            Text("Radio").font(.subheadline.weight(.semibold)).foregroundStyle(Palette.amber)
                            Toggle("Volume buttons as PTT (with screen off via PushToTalk)", isOn: $m.settings.volumePtt).onChange(of: model.settings.volumePtt) { _, _ in model.save() }
                            Toggle("Hide offensive words in messages", isOn: $wordFilter)
                            Text("Relay URL").font(.caption).foregroundStyle(Palette.muted)
                            TextField("wss://", text: $m.settings.relayUrl).font(.caption.monospaced()).padding(12).background(Palette.graphite, in: RoundedRectangle(cornerRadius: 12)).onSubmit { model.save() }
                        }
                    }
                    Card {
                        VStack(alignment: .leading, spacing: 6) {
                            Text("About").font(.subheadline.weight(.semibold)).foregroundStyle(Palette.amber)
                            Text("Titi iOS 0.1.0").font(.subheadline)
                            Text("Device ID").font(.caption).foregroundStyle(Palette.muted)
                            Text(host.nodeId).font(.caption.monospaced())
                            Text("Voice and messages are end-to-end encrypted with the group key. Nothing leaves your phones unless you are online, and then the relay sees only encrypted frames.").font(.caption).foregroundStyle(Palette.muted).padding(.top, 8)
                            Link(destination: Safety.supportPage) { Label("Help & support", systemImage: "questionmark.circle") }.font(.subheadline).padding(.top, 8)
                            Link(destination: Safety.privacyPage) { Label("Privacy policy", systemImage: "hand.raised") }.font(.subheadline)
                            Link(destination: Safety.mail(subject: "Titi iOS")) { Label(Safety.supportEmail, systemImage: "envelope") }.font(.subheadline)
                        }
                    }
                    if !host.blocked.isEmpty {
                        Card {
                            VStack(alignment: .leading, spacing: 8) {
                                Text("Blocked").font(.subheadline.weight(.semibold)).foregroundStyle(Palette.amber)
                                ForEach(Array(host.blocked).sorted(), id: \.self) { n in
                                    HStack { Text(verbatim: host.displayName(n)); Spacer(); Button("Unblock") { host.setBlocked(n, false) } }.font(.subheadline)
                                }
                            }
                        }
                    }
                    Card {
                        VStack(alignment: .leading, spacing: 8) {
                            Button(role: .destructive) { confirmWipe = true } label: { Label("Delete identity & all data", systemImage: "trash") }
                            Text("Removes your device key, every group and every message from this phone. Other members keep their own copies.").font(.caption).foregroundStyle(Palette.muted)
                        }
                    }
                }.padding(20)
            }
        }
        .confirmationDialog("Delete your identity, groups and messages from this phone?", isPresented: $confirmWipe, titleVisibility: .visible) {
            Button("Delete everything", role: .destructive) { model.wipeAll() }
        }
    }
}
