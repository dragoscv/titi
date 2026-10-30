import SwiftUI

// Brand pack v1 (brand/, ADR-0007 Signal Amber): values come from the generated BrandTokens.swift.
enum Palette {
    static let graphite = BrandDark.bgCanvas
    static let surface = BrandDark.bgSurface
    static let elevated = BrandDark.bgElevated
    static let outline = BrandDark.borderDefault
    static let ink = BrandDark.fgDefault
    static let muted = BrandDark.fgMuted
    static let amber = BrandDark.accentSolid
    static let amberDeep = BrandDark.accentPressed
    static let teal = BrandDark.receiveSolid
    static let danger = BrandDark.dangerSolid
    static let emergency = BrandDark.emergencySolid
}

func hueColor(_ hue: Int) -> Color { Color(hue: Double(hue % 360) / 360, saturation: 0.72, brightness: 0.85) }

extension Animation {
    /// stiffness 380 / damping 30 ≈ response 0.32, damping fraction 0.78
    static let titi = Animation.spring(response: 0.32, dampingFraction: 0.78)
}

struct Avatar: View {
    let name: String
    let hue: Int
    var size: CGFloat = 40
    var talking = false
    @State private var pulse = false
    var body: some View {
        ZStack {
            if talking {
                Circle().stroke(Palette.teal, lineWidth: 2).frame(width: size + 8, height: size + 8)
                    .scaleEffect(pulse ? 1.18 : 1).opacity(pulse ? 0.4 : 0.9)
                    .animation(.easeInOut(duration: 0.9).repeatForever(), value: pulse)
                    .onAppear { pulse = true }
            }
            Circle().fill(hueColor(hue)).frame(width: size, height: size)
            Text(initials).font(.system(size: size * 0.38, weight: .bold)).foregroundStyle(Palette.graphite)
        }
        .accessibilityLabel("Avatar of \(name)")
    }
    private var initials: String {
        let parts = name.split(separator: " ").prefix(2).compactMap { $0.first.map(String.init) }
        return parts.isEmpty ? "?" : parts.joined().uppercased()
    }
}

struct Card<Content: View>: View {
    @ViewBuilder var content: Content
    var body: some View {
        content.padding(18).frame(maxWidth: .infinity, alignment: .leading)
            .background(Palette.surface, in: RoundedRectangle(cornerRadius: 22, style: .continuous))
    }
}

struct PrimaryButton: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.font(.headline).frame(maxWidth: .infinity).frame(height: 56)
            .background(Palette.amber, in: RoundedRectangle(cornerRadius: 18, style: .continuous))
            .foregroundStyle(Palette.graphite)
            .scaleEffect(configuration.isPressed ? 0.98 : 1)
            .animation(.titi, value: configuration.isPressed)
    }
}

struct OutlineButton: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.font(.headline).frame(maxWidth: .infinity).frame(height: 56)
            .overlay(RoundedRectangle(cornerRadius: 18, style: .continuous).stroke(Palette.outline))
            .foregroundStyle(Palette.ink)
            .scaleEffect(configuration.isPressed ? 0.98 : 1)
    }
}
