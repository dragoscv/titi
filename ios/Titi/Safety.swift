import Foundation
import UIKit

/// App Review 1.2 (user-generated content): report by e-mail with context, published contact,
/// and an on-device word filter. Groups are invite-only; nothing is moderated server-side because
/// the relay only ever sees ciphertext.
enum Safety {
    static let supportEmail = "support@codai.ro"
    static let supportPage = URL(string: "https://titi.dragoscatalin.ro/support")!
    static let privacyPage = URL(string: "https://titi.dragoscatalin.ro/privacy")!

    static func mail(subject: String, body: String = "") -> URL {
        var c = URLComponents()
        c.scheme = "mailto"
        c.path = supportEmail
        c.queryItems = [URLQueryItem(name: "subject", value: subject), URLQueryItem(name: "body", value: body)]
        return c.url ?? URL(string: "mailto:\(supportEmail)")!
    }

    /// Opens a pre-filled report mail. The reported text is included only because the reporter can read it.
    @MainActor static func report(group: String, gid: String, node: String, name: String, msg: String?, text: String?) {
        var lines = [
            "Report from Titi iOS",
            "Group: \(group) (\(gid.prefix(8)))",
            "Member: \(name) — device \(node)",
        ]
        if let msg { lines.append("Message: \(msg)") }
        if let text { lines.append("Text: \(text)") }
        lines.append("")
        lines.append("What happened:")
        UIApplication.shared.open(mail(subject: "Titi report: \(name)", body: lines.joined(separator: "\n")))
    }

    /// Masks common slurs and profanity (EN + RO) when the filter is on (Settings, default on).
    static func filtered(_ t: String) -> String {
        guard UserDefaults.standard.object(forKey: "word_filter") as? Bool ?? true else { return t }
        var out = t
        for w in words {
            out = out.replacingOccurrences(of: "\\b\(w)\\w*", with: String(repeating: "•", count: 4), options: [.regularExpression, .caseInsensitive, .diacriticInsensitive])
        }
        return out
    }

    private static let words = [
        "fuck", "shit", "cunt", "bitch", "asshole", "nigg", "fagg", "retard", "whore", "slut",
        "pula", "pizd", "muie", "futu", "cacat", "curv", "bulangi", "poponar", "tigan",
    ]
}
