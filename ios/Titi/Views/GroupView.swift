import CoreImage.CIFilterBuiltins
import SwiftUI
import TitiCore

struct GroupView: View {
    @Environment(AppModel.self) private var model
    @Environment(EngineHost.self) private var host
    let id: String
    let ns: Namespace.ID
    @State private var showInvite = false
    @State private var showChat = false
    @State private var confirmLeave = false
    @State private var confirmSos = false

    private var g: GroupState? { host.groups.first { $0.id == id } }

    var body: some View {
        if let g {
            VStack(spacing: 0) {
                header(g)
                if g.suspended || g.handover != "stable" {
                    Text(g.suspended ? "Out of range — will resume" : g.handover == "switching" ? "Switching to \(g.link?.label ?? "")" : "Link weak — switching")
                        .font(.caption.weight(.medium)).padding(10).frame(maxWidth: .infinity)
                        .background(Palette.elevated, in: RoundedRectangle(cornerRadius: 12)).padding(.horizontal, 20)
                        .transition(.move(edge: .top).combined(with: .opacity))
                }
                ScrollView(.horizontal) {
                    HStack(spacing: 16) {
                        ForEach(g.members) { m in
                            VStack(spacing: 6) {
                                Avatar(name: m.name, hue: m.hue, size: 56, talking: g.talkerNode == m.node)
                                Text(m.node == host.nodeId ? "You" : m.name).font(.caption).lineLimit(1)
                                if let p = host.peers[m.node], m.node != host.nodeId { Text(p.hops > 1 ? "\(p.hops) hops" : p.link.label.lowercased()).font(.caption2.monospaced()).foregroundStyle(Palette.muted) }
                            }
                        }
                    }.padding(.horizontal, 20)
                }.scrollIndicators(.hidden).padding(.vertical, 16)
                Spacer()
                Text(statusText(g)).font(.headline).foregroundStyle(g.floor == .talking ? Palette.amber : g.floor == .busy ? Palette.teal : Palette.muted)
                    .contentTransition(.opacity).animation(.titi, value: g.floor)
                TalkButton(floor: g.floor, fullDuplex: g.fullDuplex, muted: host.muted, talkerName: g.talkerName, level: host.levelDbfs,
                           onDown: { host.pttDown() }, onUp: { host.pttUp() }, onToggleMute: { host.muted.toggle() })
                    .padding(.top, 20)
                HStack(spacing: 12) {
                    Picker("Mode", selection: Binding(get: { g.fullDuplex }, set: { host.setFullDuplex(g.id, $0) })) {
                        Text("Push to talk").tag(false); Text("Open mic").tag(true)
                    }.pickerStyle(.segmented)
                    Button { confirmSos = true } label: { Image(systemName: "sos").font(.headline).frame(width: 44, height: 44) }
                        .background(Palette.emergency.opacity(0.18), in: Circle()).foregroundStyle(Palette.emergency)
                }.padding(.horizontal, 24).padding(.top, 28).padding(.bottom, 24)
            }
            .sheet(isPresented: $showInvite) { InviteSheet(gid: g.id, gname: g.name) }
            .sheet(isPresented: $showChat) { ChatView(gid: g.id) }
            .confirmationDialog("Leave \(g.name)?", isPresented: $confirmLeave, titleVisibility: .visible) {
                Button("Leave group", role: .destructive) { host.leaveGroup(g.id); withAnimation(.titi) { model.route = .home } }
            }
            .confirmationDialog("Send an SOS to everyone in \(g.name) with your location?", isPresented: $confirmSos, titleVisibility: .visible) {
                Button("SOS", role: .destructive) { Location.current { lat, lon in host.sendSos(g.id, lat: lat, lon: lon) } }
            }
        } else {
            Color.clear.onAppear { withAnimation(.titi) { model.route = .home } }
        }
    }

    private func header(_ g: GroupState) -> some View {
        HStack(spacing: 4) {
            Button { withAnimation(.titi) { model.route = .home } } label: { Image(systemName: "chevron.left").font(.title3).padding(12) }.foregroundStyle(Palette.muted)
            VStack(alignment: .leading, spacing: 2) {
                Text(g.name).font(.title3.weight(.semibold)).matchedGeometryEffect(id: "title-\(g.id)", in: ns)
                HStack(spacing: 8) {
                    Text("\(g.members.count) member\(g.members.count == 1 ? "" : "s")")
                    HStack(spacing: 5) {
                        if let l = g.link { Image(systemName: l.symbol); Text(l.label) } else { Text("Searching…") }
                    }.padding(.horizontal, 10).padding(.vertical, 4).overlay(Capsule().stroke(Palette.outline))
                }.font(.caption).foregroundStyle(Palette.muted)
            }
            Spacer()
            Button { showChat = true } label: { Image(systemName: "bubble.left").padding(12) }
                .overlay(alignment: .topTrailing) { if g.unread > 0 { Text("\(g.unread)").font(.caption2.bold()).padding(4).background(Palette.amber, in: Circle()).foregroundStyle(Palette.graphite) } }
            Button { showInvite = true } label: { Image(systemName: "person.badge.plus").padding(12) }
            Menu { Button("Leave group", role: .destructive) { confirmLeave = true } } label: { Image(systemName: "ellipsis").padding(12) }
        }.font(.title3).foregroundStyle(Palette.muted).padding(.horizontal, 8).padding(.top, 4)
    }

    private func statusText(_ g: GroupState) -> String {
        switch g.floor {
        case .talking: "You are talking"
        case .busy: "\(g.talkerName ?? "") is talking"
        case .queued: "Waiting for the channel…"
        case .pending: "…"
        case .idle: "Channel free"
        }
    }
}

struct InviteSheet: View {
    @Environment(EngineHost.self) private var host
    let gid: String
    let gname: String
    @State private var code: (String, Int)?
    private var link: String? { host.deepLink(gid) }

    var body: some View {
        VStack(spacing: 8) {
            Text("Invite to \(gname)").font(.title2.bold()).padding(.top, 8)
            let cands = host.peers.values.filter { !$0.inGroup }
            if !cands.isEmpty {
                Text("Tap a phone nearby").font(.subheadline).foregroundStyle(Palette.muted).padding(.top, 8)
                ScrollView(.horizontal) { HStack(spacing: 16) { ForEach(cands) { p in Button { host.invitePeer(gid, p.node) } label: { VStack { Avatar(name: p.name, hue: p.hue, size: 56); Text(p.name).font(.caption) } }.buttonStyle(.plain) } } }
            }
            Text("Say this code").font(.subheadline).foregroundStyle(Palette.muted).padding(.top, 12)
            Text(code?.0.replacingOccurrences(of: "-", with: " ") ?? "…").font(.system(size: 28, weight: .medium, design: .monospaced)).foregroundStyle(Palette.amber)
                .onTapGesture { if let c = code { UIPasteboard.general.string = c.0 } }
            Text("Changes in \((code?.1 ?? 0) / 60):\(String(format: "%02d", (code?.1 ?? 0) % 60))").font(.caption.monospaced()).foregroundStyle(Palette.muted)
            if let link, let img = qr(link.replacingOccurrences(of: "titi://j/", with: "https://titi.app/j/")) {
                Text("Or scan").font(.subheadline).foregroundStyle(Palette.muted).padding(.top, 12)
                Image(uiImage: img).interpolation(.none).resizable().frame(width: 200, height: 200).padding(8).background(.white, in: RoundedRectangle(cornerRadius: 16))
                ShareLink(item: URL(string: link.replacingOccurrences(of: "titi://j/", with: "https://titi.app/j/"))!) { Label("Share link", systemImage: "square.and.arrow.up") }.padding(.top, 12)
            }
            Spacer()
        }
        .padding(24).presentationDetents([.large]).presentationBackground(Palette.surface)
        .task { while !Task.isCancelled { code = host.currentCode(gid); try? await Task.sleep(for: .seconds(1)) } }
    }

    private func qr(_ s: String) -> UIImage? {
        let f = CIFilter.qrCodeGenerator(); f.message = Data(s.utf8); f.correctionLevel = "M"
        guard let out = f.outputImage else { return nil }
        return UIImage(ciImage: out.transformed(by: CGAffineTransform(scaleX: 8, y: 8)))
    }
}
