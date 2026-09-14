import SwiftUI
import TitiCore

struct RootView: View {
    @Environment(AppModel.self) private var model
    @Namespace private var ns

    var body: some View {
        ZStack {
            Palette.graphite.ignoresSafeArea()
            if !model.settings.onboarded {
                OnboardingView()
            } else if let host = model.host {
                content(host)
                    .environment(host)
                    .overlay(alignment: .top) { InviteBanner(host: host) }
                    .overlay(alignment: .bottom) { ToastView(host: host) }
            } else {
                ProgressView().task { model.boot() }
            }
        }
    }

    @ViewBuilder private func content(_ host: EngineHost) -> some View {
        switch model.route {
        case .home: HomeView(ns: ns).transition(.move(edge: .leading).combined(with: .opacity))
        case .group(let id): GroupView(id: id, ns: ns).transition(.move(edge: .trailing).combined(with: .opacity))
        case .join: JoinView().transition(.move(edge: .trailing).combined(with: .opacity))
        case .settings: SettingsView().transition(.move(edge: .trailing).combined(with: .opacity))
        }
    }
}

struct InviteBanner: View {
    @Bindable var host: EngineHost
    var body: some View {
        if let inv = host.invites.first {
            HStack(spacing: 12) {
                Avatar(name: inv.hostName, hue: Int(inv.host.prefix(2), radix: 16).map { $0 * 360 / 256 } ?? 40)
                VStack(alignment: .leading, spacing: 2) {
                    Text("\(inv.hostName) invites you").font(.subheadline.weight(.semibold))
                    Text("Join “\(inv.name)” · \(inv.members) members").font(.caption).foregroundStyle(Palette.muted)
                }
                Spacer()
                Button("Not now") { host.declineInvite(inv) }.font(.subheadline).foregroundStyle(Palette.muted)
                Button("Join") { host.acceptInvite(inv) }.buttonStyle(.borderedProminent).tint(Palette.amber).foregroundStyle(Palette.graphite)
            }
            .padding(14)
            .background(Palette.elevated, in: RoundedRectangle(cornerRadius: 22, style: .continuous))
            .shadow(radius: 20, y: 10)
            .padding(12)
            .transition(.move(edge: .top).combined(with: .opacity))
            .animation(.titi, value: host.invites.count)
        }
    }
}

struct ToastView: View {
    @Bindable var host: EngineHost
    var body: some View {
        if let t = host.toast {
            Text(t).font(.subheadline).padding(.horizontal, 16).padding(.vertical, 10)
                .background(Palette.elevated, in: Capsule()).padding(.bottom, 24)
                .transition(.move(edge: .bottom).combined(with: .opacity))
                .task { try? await Task.sleep(for: .seconds(3)); if host.toast == t { host.toast = nil } }
        }
    }
}
