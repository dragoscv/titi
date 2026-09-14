import SwiftUI
import TitiCore

/// Morphing Talk button (ADR-0007): circle → squircle (armed) → wide bar
/// (transmitting) → mute pill (open mic). DragGesture(minimumDistance: 0)
/// so a thumb that drifts never releases the floor mid-sentence.
struct TalkButton: View {
    let floor: FloorState
    let fullDuplex: Bool
    let muted: Bool
    let talkerName: String?
    let level: Float
    let onDown: () -> Void
    let onUp: () -> Void
    let onToggleMute: () -> Void
    @State private var pressed = false

    private var transmitting: Bool { floor == .talking }
    private var busy: Bool { floor == .busy || floor == .queued }
    private var wide: Bool { transmitting || fullDuplex }
    private var armed: Bool { pressed && !wide }
    private var w: CGFloat { wide ? 320 : armed ? 196 : 176 }
    private var h: CGFloat { wide ? 96 : armed ? 196 : 176 }
    private var r: CGFloat { wide ? 48 : armed ? 56 : 88 }
    private var bg: Color { fullDuplex && muted ? Palette.elevated : fullDuplex ? Palette.teal : transmitting ? Palette.amber : busy ? Palette.elevated : armed ? Palette.amberDeep : Palette.amber }
    private var fg: Color { busy || (fullDuplex && muted) ? Palette.ink : Palette.graphite }
    private var lvl: CGFloat { CGFloat(min(1, max(0, (level + 50) / 50))) }

    var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius: r, style: .continuous).fill(bg)
                .shadow(color: Palette.amber.opacity(transmitting || armed ? 0.45 : 0.25), radius: 30, y: 16)
            if transmitting {
                GeometryReader { geo in
                    RoundedRectangle(cornerRadius: r, style: .continuous).fill(.white.opacity(0.22))
                        .frame(width: geo.size.width * (0.25 + 0.75 * lvl)).animation(.spring(response: 0.25, dampingFraction: 0.8), value: lvl)
                }.clipShape(RoundedRectangle(cornerRadius: r, style: .continuous))
            }
            if wide {
                HStack(spacing: 14) {
                    Image(systemName: fullDuplex && muted ? "mic.slash.fill" : transmitting ? "waveform" : "mic.fill").font(.system(size: 28))
                    VStack(alignment: .leading, spacing: 2) {
                        Text(fullDuplex ? (muted ? "Unmute" : "Open mic") : "You are talking").font(.system(size: 17, weight: .semibold))
                        Text(fullDuplex ? (muted ? "Open mic" : "Tap to mute") : "Release to finish").font(.caption).opacity(0.7)
                    }
                    Spacer()
                    if transmitting {
                        HStack(alignment: .bottom, spacing: 3) { ForEach(0..<6) { i in RoundedRectangle(cornerRadius: 1).frame(width: 4, height: CGFloat(10 + i * 4)).opacity(lvl >= CGFloat(i + 1) / 6 * 0.9 ? 1 : 0.25) } }
                    }
                }.padding(.horizontal, 24).transition(.opacity.combined(with: .scale(scale: 0.9)))
            } else {
                VStack(spacing: 6) {
                    Image(systemName: "mic.fill").font(.system(size: 44))
                    Text(busy ? (talkerName ?? "") : "Hold to talk").font(.subheadline.weight(.semibold)).opacity(0.85)
                }.transition(.opacity.combined(with: .scale(scale: 0.9)))
            }
        }
        .foregroundStyle(fg)
        .frame(width: w, height: h)
        .scaleEffect(pressed && !wide ? 0.96 : 1)
        .animation(.titi, value: w).animation(.titi, value: bg).animation(.titi, value: pressed)
        .gesture(fullDuplex ? nil : DragGesture(minimumDistance: 0)
            .onChanged { _ in if !pressed { pressed = true; UIImpactFeedbackGenerator(style: .medium).impactOccurred(); onDown() } }
            .onEnded { _ in pressed = false; onUp() })
        .onTapGesture { if fullDuplex { onToggleMute() } }
        .accessibilityLabel("Talk button. Press and hold to transmit.")
        .accessibilityAddTraits(.isButton)
    }
}
