import SwiftUI

struct OnboardingView: View {
    @Environment(AppModel.self) private var model
    @State private var name = ""
    @State private var hue = 40
    private let hues = [40, 15, 350, 290, 220, 170, 120, 80]

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            RoundedRectangle(cornerRadius: 22, style: .continuous).fill(Palette.amber).frame(width: 72, height: 72)
                .overlay(Image(systemName: "mic.fill").font(.system(size: 34)).foregroundStyle(Palette.graphite))
                .padding(.top, 48)
            Text("Talk anywhere.\nNo signal needed.").font(.system(size: 36, weight: .heavy)).padding(.top, 28)
            Text("Titi turns nearby phones into a private radio over Wi‑Fi, hotspot and Bluetooth — and uses the internet when it exists.")
                .font(.body).foregroundStyle(Palette.muted).padding(.top, 12)
            Text("What should others call you?").font(.headline).padding(.top, 36)
            HStack(spacing: 14) {
                Avatar(name: name.isEmpty ? "?" : name, hue: hue, size: 52)
                TextField("Your name", text: $name).textFieldStyle(.plain).font(.title3).padding(14)
                    .background(Palette.surface, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).stroke(Palette.outline))
            }.padding(.top, 10)
            Text("Pick your colour").font(.headline).padding(.top, 24)
            HStack(spacing: 10) {
                ForEach(hues, id: \.self) { h in
                    Button { hue = h } label: {
                        Circle().fill(hueColor(h)).frame(width: 40, height: 40)
                            .overlay { if h == hue { Image(systemName: "checkmark").foregroundStyle(Palette.graphite).bold() } }
                            .overlay { if h == hue { Circle().stroke(Palette.ink, lineWidth: 3) } }
                    }
                }
            }.padding(.top, 10)
            Spacer()
            Button("Continue") {
                model.settings.name = name.trimmingCharacters(in: .whitespaces); model.settings.hue = hue; model.settings.onboarded = true
                model.save(); model.boot()
            }.buttonStyle(PrimaryButton()).disabled(name.trimmingCharacters(in: .whitespaces).count < 2)
        }
        .padding(24)
    }
}
