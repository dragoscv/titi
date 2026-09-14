import SwiftUI
import TitiCore

struct HomeView: View {
    @Environment(AppModel.self) private var model
    @Environment(EngineHost.self) private var host
    let ns: Namespace.ID
    @State private var showCreate = false
    @State private var newName = ""
    @State private var invitePeer: Peer?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                HStack(spacing: 12) {
                    Avatar(name: model.settings.name, hue: model.settings.hue)
                    VStack(alignment: .leading) {
                        Text(model.settings.name).font(.headline)
                        Text(host.running ? "Radio on · \(host.peers.count) nearby" : "Searching…").font(.caption).foregroundStyle(Palette.muted)
                    }
                    Spacer()
                    Button { withAnimation(.titi) { model.route = .settings } } label: { Image(systemName: "gearshape").font(.title3) }.foregroundStyle(Palette.muted)
                }.padding(.top, 8)

                Text("Nearby").font(.subheadline.weight(.semibold)).foregroundStyle(Palette.muted).padding(.top, 16)
                Card {
                    if host.peers.isEmpty {
                        HStack(spacing: 14) { Radar(); Text("Looking for phones around you…").font(.subheadline).foregroundStyle(Palette.muted) }
                    } else {
                        ScrollView(.horizontal) {
                            HStack(spacing: 16) {
                                ForEach(host.peers.values.sorted { $0.name < $1.name }) { p in
                                    Button { invitePeer = p } label: {
                                        VStack(spacing: 6) {
                                            Avatar(name: p.name, hue: p.hue, size: 52)
                                            Text(p.name).font(.caption).lineLimit(1)
                                            Image(systemName: p.link.symbol).font(.caption2).foregroundStyle(p.inGroup ? Palette.teal : Palette.muted)
                                        }
                                    }.buttonStyle(.plain)
                                }
                            }
                        }.scrollIndicators(.hidden)
                    }
                }

                Text("Your groups").font(.subheadline.weight(.semibold)).foregroundStyle(Palette.muted).padding(.top, 8)
                if host.groups.isEmpty {
                    Card { VStack(spacing: 4) { Text("No groups yet").font(.headline); Text("Create one, or join with a code from a friend.").font(.subheadline).foregroundStyle(Palette.muted) }.frame(maxWidth: .infinity) }
                }
                ForEach(host.groups) { g in
                    Button { host.setActiveGroup(g.id); withAnimation(.titi) { model.route = .group(g.id) } } label: {
                        HStack(spacing: 14) {
                            let talking = g.floor == .busy || g.floor == .talking
                            RoundedRectangle(cornerRadius: 16, style: .continuous).fill(talking ? Palette.teal : Palette.amber.opacity(0.18)).frame(width: 48, height: 48)
                                .overlay(Text(String(g.name.prefix(1)).uppercased()).font(.title2.bold()).foregroundStyle(talking ? Palette.graphite : Palette.amber))
                                .matchedGeometryEffect(id: "icon-\(g.id)", in: ns)
                            VStack(alignment: .leading, spacing: 2) {
                                Text(g.name).font(.headline).matchedGeometryEffect(id: "title-\(g.id)", in: ns)
                                Text(talking && g.talkerName != nil ? "\(g.talkerName!) is talking" : "\(g.members.count) member\(g.members.count == 1 ? "" : "s")")
                                    .font(.caption).foregroundStyle(talking ? Palette.teal : Palette.muted)
                            }
                            Spacer()
                            if g.unread > 0 { Text("\(g.unread)").font(.caption2.bold()).padding(.horizontal, 7).padding(.vertical, 2).background(Palette.amber, in: Capsule()).foregroundStyle(Palette.graphite) }
                            if let l = g.link { Image(systemName: l.symbol).foregroundStyle(Palette.muted) }
                        }
                        .padding(18)
                        .background(g.id == host.activeGroup ? Palette.elevated : Palette.surface, in: RoundedRectangle(cornerRadius: 22, style: .continuous))
                    }.buttonStyle(.plain)
                }
            }
            .padding(.horizontal, 20).padding(.bottom, 120)
        }
        .safeAreaInset(edge: .bottom) {
            HStack(spacing: 12) {
                Button { showCreate = true } label: { Label("New group", systemImage: "plus") }.buttonStyle(PrimaryButton())
                Button { withAnimation(.titi) { model.route = .join } } label: { Label("Join", systemImage: "arrow.right.circle") }.buttonStyle(OutlineButton())
            }.padding(20).background(LinearGradient(colors: [.clear, Palette.graphite, Palette.graphite], startPoint: .top, endPoint: .bottom))
        }
        .sheet(isPresented: $showCreate) {
            VStack(alignment: .leading, spacing: 16) {
                Text("Name your group").font(.title2.bold())
                TextField("e.g. Cabana, Trail team, Ski lift", text: $newName).padding(14).background(Palette.graphite, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
                Button("Create") { host.createGroup(newName.trimmingCharacters(in: .whitespaces)); newName = ""; showCreate = false }
                    .buttonStyle(PrimaryButton()).disabled(newName.trimmingCharacters(in: .whitespaces).count < 2)
            }.padding(24).presentationDetents([.height(260)]).presentationBackground(Palette.surface)
        }
        .sheet(item: $invitePeer) { p in
            VStack(alignment: .leading, spacing: 12) {
                HStack(spacing: 12) { Avatar(name: p.name, hue: p.hue, size: 48); Text(p.name).font(.headline) }
                Text("Invite").font(.subheadline.weight(.semibold)).foregroundStyle(Palette.muted)
                if host.groups.isEmpty { Button("New group") { host.createGroup(p.name); invitePeer = nil }.buttonStyle(PrimaryButton()) }
                ForEach(host.groups) { g in Button(g.name) { host.invitePeer(g.id, p.node); invitePeer = nil }.buttonStyle(OutlineButton()) }
            }.padding(24).presentationDetents([.medium]).presentationBackground(Palette.surface)
        }
    }
}

struct Radar: View {
    @State private var angle = 0.0
    var body: some View {
        ZStack {
            ForEach(1...3, id: \.self) { k in Circle().stroke(Palette.outline, lineWidth: 1).frame(width: CGFloat(k) * 18, height: CGFloat(k) * 18) }
            Rectangle().fill(Palette.teal).frame(width: 2, height: 27).offset(y: -13.5).rotationEffect(.degrees(angle))
        }
        .frame(width: 56, height: 56)
        .onAppear { withAnimation(.linear(duration: 3.2).repeatForever(autoreverses: false)) { angle = 360 } }
    }
}

extension FfiLinkClass {
    var symbol: String {
        switch self {
        case .lan: "wifi"
        case .hotspot: "personalhotspot"
        case .wifiAware, .nearby: "dot.radiowaves.left.and.right"
        case .bleL2cap, .bleGatt, .btRfcomm: "bluetooth"
        case .internet, .webRtc: "cloud"
        }
    }
    var label: String {
        switch self {
        case .lan: "Wi‑Fi"; case .hotspot: "Hotspot"; case .wifiAware: "Wi‑Fi Aware"; case .nearby: "Nearby"
        case .bleL2cap, .bleGatt: "Bluetooth LE"; case .btRfcomm: "Bluetooth"; case .internet, .webRtc: "Internet"
        }
    }
}
