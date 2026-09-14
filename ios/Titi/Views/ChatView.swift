import SwiftUI
import TitiCore

struct ChatView: View {
    @Environment(EngineHost.self) private var host
    let gid: String
    @State private var text = ""

    var body: some View {
        VStack(spacing: 0) {
            let msgs = host.messages.filter { $0.group == gid }
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 8) {
                        if msgs.isEmpty { Text("No messages yet. Texts are stored and forwarded when a path exists.").font(.subheadline).foregroundStyle(Palette.muted).frame(maxWidth: .infinity).padding(.top, 32) }
                        ForEach(msgs) { m in
                            HStack(alignment: .bottom, spacing: 8) {
                                if m.mine { Spacer() } else { Avatar(name: m.fromName, hue: host.peers[m.from]?.hue ?? 200, size: 28) }
                                VStack(alignment: .leading, spacing: 2) {
                                    if !m.mine { Text(m.fromName).font(.caption2).foregroundStyle(Palette.muted) }
                                    Text(bodyText(m)).font(.body)
                                    Text(Date(timeIntervalSince1970: Double(m.sentMs) / 1000).formatted(date: .omitted, time: .shortened) + (m.mine ? (m.acked ? " ✓✓" : " ✓") : "")).font(.caption2.monospaced()).foregroundStyle(Palette.muted)
                                }
                                .padding(.horizontal, 14).padding(.vertical, 10)
                                .background(m.mine ? Palette.amber.opacity(0.2) : Palette.elevated, in: RoundedRectangle(cornerRadius: 18, style: .continuous))
                                .frame(maxWidth: 280, alignment: .leading)
                                if !m.mine { Spacer() }
                            }.id(m.id)
                        }
                    }.padding(16)
                }
                .onChange(of: msgs.count) { _, _ in if let l = msgs.last { withAnimation { proxy.scrollTo(l.id, anchor: .bottom) } } }
            }
            HStack(spacing: 8) {
                TextField("Message", text: $text).padding(.horizontal, 16).padding(.vertical, 12).background(Palette.graphite, in: Capsule()).overlay(Capsule().stroke(Palette.outline)).onSubmit(send)
                Button(action: send) { Image(systemName: "paperplane.fill").frame(width: 48, height: 48) }
                    .background(Palette.amber, in: RoundedRectangle(cornerRadius: 16, style: .continuous)).foregroundStyle(Palette.graphite).disabled(text.trimmingCharacters(in: .whitespaces).isEmpty)
            }.padding(12)
        }
        .presentationDetents([.large]).presentationBackground(Palette.surface)
    }

    private func send() { let t = text.trimmingCharacters(in: .whitespaces); guard !t.isEmpty else { return }; host.sendText(gid, t); text = "" }

    private func bodyText(_ m: ChatMessage) -> String {
        switch m.body {
        case let .text(t): t
        case .location: "\(m.fromName) shared a location"
        case let .sos(_, _, _, cancelled): cancelled ? "\(m.fromName) cancelled the SOS" : "SOS from \(m.fromName)"
        case .voiceNote: "Voice note"
        }
    }
}
