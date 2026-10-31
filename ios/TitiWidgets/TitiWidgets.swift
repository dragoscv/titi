import ActivityKit
import AppIntents
import SwiftUI
import WidgetKit

// Extension deployment target is iOS 18.0 (project.yml), so ControlWidget is always available.

@main
struct TitiWidgetsBundle: WidgetBundle {
    var body: some Widget {
        TalkControl()
        SosLiveActivity()
    }
}

/// Control Center / Lock Screen / Action Button control: one tap = start talking, next tap = stop.
struct TalkControl: ControlWidget {
    static let kind = "ro.titi.app.widgets.talk"

    var body: some ControlWidgetConfiguration {
        StaticControlConfiguration(kind: Self.kind) {
            ControlWidgetButton(action: TalkIntent()) {
                Label("Talk", systemImage: "mic.fill")
            }
        }
        .displayName("Talk")
        .description("Starts or stops talking to your active group.")
    }
}

struct SosLiveActivity: Widget {
    var body: some WidgetConfiguration {
        ActivityConfiguration(for: SosAttributes.self) { context in
            SosLockScreenView(context: context)
                .activityBackgroundTint(BrandDark.bgSurface)
                .activitySystemActionForegroundColor(BrandDark.fgDefault)
        } dynamicIsland: { context in
            DynamicIsland {
                DynamicIslandExpandedRegion(.leading) {
                    SosBadge(size: 40)
                }
                DynamicIslandExpandedRegion(.trailing) {
                    Text(timerInterval: context.state.startedAt...Date.distantFuture, countsDown: false)
                        .font(.headline.monospacedDigit())
                        .foregroundStyle(BrandDark.fgDefault)
                        .multilineTextAlignment(.trailing)
                        .frame(maxWidth: 72)
                }
                DynamicIslandExpandedRegion(.center) {
                    VStack(spacing: 2) {
                        Text("SOS sent").font(.headline).foregroundStyle(BrandDark.emergencySolid)
                        Text(context.attributes.groupName).font(.subheadline).foregroundStyle(BrandDark.fgDefault).lineLimit(1)
                    }
                }
                DynamicIslandExpandedRegion(.bottom) {
                    HStack {
                        Text(context.state.status).font(.caption).foregroundStyle(BrandDark.fgMuted).lineLimit(1)
                        Spacer()
                        Button(intent: EndSosIntent()) { Text("Hide") }
                            .font(.caption.weight(.semibold))
                            .tint(BrandDark.accentSolid)
                    }
                }
            } compactLeading: {
                Image(systemName: "sos").foregroundStyle(BrandDark.emergencySolid)
            } compactTrailing: {
                Text(timerInterval: context.state.startedAt...Date.distantFuture, countsDown: false)
                    .monospacedDigit()
                    .foregroundStyle(BrandDark.fgDefault)
                    .frame(maxWidth: 48)
            } minimal: {
                Image(systemName: "sos").foregroundStyle(BrandDark.emergencySolid)
            }
            .keylineTint(BrandDark.emergencySolid)
        }
    }
}

private struct SosBadge: View {
    let size: CGFloat
    var body: some View {
        Image(systemName: "sos")
            .font(.system(size: size * 0.42, weight: .bold))
            .foregroundStyle(BrandDark.fgOnEmergency)
            .frame(width: size, height: size)
            .background(BrandDark.emergencySolid, in: Circle())
            .accessibilityHidden(true)
    }
}

private struct SosLockScreenView: View {
    let context: ActivityViewContext<SosAttributes>

    var body: some View {
        HStack(spacing: 14) {
            SosBadge(size: 48)
            VStack(alignment: .leading, spacing: 3) {
                Text("SOS sent to \(context.attributes.groupName)")
                    .font(.headline).foregroundStyle(BrandDark.fgDefault).lineLimit(1)
                Text(context.state.status)
                    .font(.subheadline).foregroundStyle(BrandDark.fgMuted).lineLimit(1)
                Text(timerInterval: context.state.startedAt...Date.distantFuture, countsDown: false)
                    .font(.caption.monospacedDigit()).foregroundStyle(BrandDark.accentFg)
            }
            Spacer(minLength: 8)
            Button(intent: EndSosIntent()) { Text("Hide") }
                .font(.subheadline.weight(.semibold))
                .tint(BrandDark.accentSolid)
        }
        .padding(16)
        .accessibilityElement(children: .combine)
    }
}
