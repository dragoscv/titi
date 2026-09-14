import SwiftUI

// ADR-0007 Signal Amber
enum Palette {
    static let graphite = Color(red: 0x0E / 255, green: 0x10 / 255, blue: 0x13 / 255)
    static let surface = Color(red: 0x16 / 255, green: 0x19 / 255, blue: 0x1E / 255)
    static let elevated = Color(red: 0x1E / 255, green: 0x22 / 255, blue: 0x29 / 255)
    static let outline = Color(red: 0x2A / 255, green: 0x2F / 255, blue: 0x38 / 255)
    static let ink = Color(red: 0xF2 / 255, green: 0xF4 / 255, blue: 0xF7 / 255)
    static let muted = Color(red: 0x9A / 255, green: 0xA3 / 255, blue: 0xB2 / 255)
    static let amber = Color(red: 1.0, green: 0xB0 / 255, blue: 0x20 / 255)
    static let amberDeep = Color(red: 0xD9 / 255, green: 0x8E / 255, blue: 0)
    static let teal = Color(red: 0x2D / 255, green: 0xD4 / 255, blue: 0xBF / 255)
    static let danger = Color(red: 1.0, green: 0x5A / 255, blue: 0x5F / 255)
    static let emergency = Color(red: 1.0, green: 0x2D / 255, blue: 0x55 / 255)
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
