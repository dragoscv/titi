import SwiftUI
import TitiCore

/// Group chat (message model v2): quotes, reactions, edits with history,
/// delete-for-everyone tombstones, forwards, mentions, drafts and search.
struct ChatView: View {
    @Environment(EngineHost.self) private var host
    let gid: String

    @State private var text = ""
    @State private var replyTo: FfiChatItem?
    @State private var editing: FfiChatItem?
    @State private var stashedDraft = ""
    @State private var mentioned: [String: String] = [:] // node hex -> name as inserted
    @State private var mentionAll = false
    @State private var query = ""
    @State private var results: [FfiChatItem] = []
    @State private var reactTarget: FfiChatItem?
    @State private var historyTarget: FfiChatItem?
    @State private var forwardTarget: FfiChatItem?
    @State private var deleteTarget: FfiChatItem?
    @State private var scrollTarget: String?
    @State private var draftSave: Task<Void, Never>?

    init(gid: String) { self.gid = gid }

    private let quick = quickReactions()
    private var group: GroupState? { host.groups.first { $0.id == gid } }
    private var items: [FfiChatItem] { host.chats[gid] ?? [] }
    private var otherGroups: [GroupState] { host.groups.filter { $0.id != gid } }

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                if query.isEmpty { timeline } else { searchResults }
                composer
            }
            .background(Palette.surface)
            .navigationTitle(group?.name ?? "")
            .navigationBarTitleDisplayMode(.inline)
            .searchable(text: $query, prompt: Text("Search messages"))
        }
        .task {
            host.loadHistory(gid)
            text = host.draft(gid) ?? ""
        }
        .task(id: query) {
            let q = query.trimmingCharacters(in: .whitespaces)
            guard !q.isEmpty else { results = []; return }
            try? await Task.sleep(for: .milliseconds(250))
            guard !Task.isCancelled else { return }
            results = host.searchChat(gid, q)
        }
        .onChange(of: text) { _, new in
            guard editing == nil else { return }
            draftSave?.cancel()
            draftSave = Task {
                try? await Task.sleep(for: .milliseconds(600))
                guard !Task.isCancelled else { return }
                host.setDraft(gid, new)
            }
        }
        .onDisappear { draftSave?.cancel(); host.setDraft(gid, editing == nil ? text : stashedDraft) }
        .sheet(item: $reactTarget) { item in
            ReactSheet(quick: quick, mine: myReaction(item)) { e in toggle(item, e) }
        }
        .sheet(item: $historyTarget) { item in EditHistorySheet(revisions: host.editHistory(item.id)) }
        .sheet(item: $forwardTarget) { item in
            ForwardSheet(groups: otherGroups) { g in
                host.forward(from: gid, item.id, to: g.id)
                host.toast = String(localized: "Forwarded to \(g.name)")
            }
        }
        .confirmationDialog("Delete this message for everyone?", isPresented: Binding(get: { deleteTarget != nil }, set: { if !$0 { deleteTarget = nil } }),
                            titleVisibility: .visible, presenting: deleteTarget) { item in
            Button("Delete for everyone", role: .destructive) { host.deleteMessage(gid, item.id) }
        }
        .presentationDetents([.large]).presentationBackground(Palette.surface)
    }

    // MARK: timeline

    private var timeline: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 10) {
                    if items.isEmpty {
                        Text("No messages yet. Texts are stored and forwarded when a path exists.")
                            .font(.subheadline).foregroundStyle(Palette.muted).multilineTextAlignment(.center)
                            .frame(maxWidth: .infinity).padding(.top, 32)
                    }
                    ForEach(items) { item in
                        row(item).id(item.id)
                            .onAppear { if item.id == items.first?.id, host.chatHasMore[gid] == true { host.loadHistory(gid, older: true) } }
                    }
                }.padding(16)
            }
            .onChange(of: items.last?.id) { _, last in if let last { withAnimation { proxy.scrollTo(last, anchor: .bottom) } } }
            .onAppear {
                // a search hit sets the target while the timeline is unmounted
                guard let t = scrollTarget else { return }
                Task { @MainActor in proxy.scrollTo(t, anchor: .center); scrollTarget = nil }
            }
            .onChange(of: scrollTarget) { _, t in
                guard let t else { return }
                withAnimation { proxy.scrollTo(t, anchor: .center) }
                scrollTarget = nil
            }
        }
    }

    private var searchResults: some View {
        Group {
            if results.isEmpty {
                ContentUnavailableView.search(text: query)
            } else {
                List(results) { item in
                    Button {
                        let id = item.id
                        query = ""
                        scrollTarget = id
                    } label: {
                        VStack(alignment: .leading, spacing: 4) {
                            HStack {
                                Text(verbatim: senderName(item)).font(.caption.bold()).foregroundStyle(Palette.amber)
                                Spacer()
                                Text(verbatim: stamp(item.sentMs, dated: true)).font(.caption2.monospaced()).foregroundStyle(Palette.muted)
                            }
                            Text(verbatim: previewText(item)).font(.subheadline).foregroundStyle(Palette.ink).lineLimit(3)
                        }
                    }
                    .listRowBackground(Palette.elevated)
                }
                .scrollContentBackground(.hidden)
            }
        }
        .frame(maxHeight: .infinity)
    }

    @ViewBuilder private func row(_ item: FfiChatItem) -> some View {
        let mine = isMine(item)
        let gone = isGone(item)
        HStack(alignment: .bottom, spacing: 8) {
            if mine { Spacer(minLength: 40) } else { Avatar(name: senderName(item), hue: hue(item.from.hex), size: 28) }
            VStack(alignment: mine ? .trailing : .leading, spacing: 4) {
                bubble(item, mine: mine, gone: gone)
                    .contextMenu { if !gone { menu(item, mine: mine) } }
                if !item.reactions.isEmpty && !gone { reactions(item) }
            }
            if !mine { Spacer(minLength: 40) }
        }
    }

    private func bubble(_ item: FfiChatItem, mine: Bool, gone: Bool) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            if !mine { Text(verbatim: senderName(item)).font(.caption2).foregroundStyle(Palette.muted) }
            if !gone && item.forwarded > 0 {
                Group {
                    if item.forwarded >= 5 {
                        Label("Forwarded many times", systemImage: "arrowshape.turn.up.right.fill")
                    } else {
                        Label("Forwarded", systemImage: "arrowshape.turn.up.right")
                    }
                }
                .font(.caption2.italic()).foregroundStyle(Palette.muted)
            }
            if !gone, let r = item.reply { quote(r) }
            if gone {
                Label("This message was deleted", systemImage: "nosign").font(.body.italic()).foregroundStyle(Palette.muted)
            } else {
                content(item)
            }
            HStack(spacing: 6) {
                if item.edited > 0 && !gone {
                    Button { historyTarget = item } label: { Text("edited").font(.caption2.italic()) }
                        .buttonStyle(.plain).foregroundStyle(Palette.muted)
                        .accessibilityLabel(Text("Edited, show history"))
                }
                Text(verbatim: stamp(item.sentMs, dated: false) + (mine && !gone ? (item.acked > 0 ? " ✓✓" : " ✓") : ""))
                    .font(.caption2.monospaced()).foregroundStyle(Palette.muted)
            }
        }
        .padding(.horizontal, 14).padding(.vertical, 10)
        .background(mine ? Palette.amber.opacity(0.2) : Palette.elevated, in: RoundedRectangle(cornerRadius: 18, style: .continuous))
        .overlay {
            if item.mentionsMe && !gone { RoundedRectangle(cornerRadius: 18, style: .continuous).stroke(Palette.amber, lineWidth: 1.5) }
        }
        .frame(maxWidth: 280, alignment: mine ? .trailing : .leading)
        .accessibilityHint(item.mentionsMe && !gone ? Text("Mentions you") : Text(verbatim: ""))
    }

    @ViewBuilder private func content(_ item: FfiChatItem) -> some View {
        let name = senderName(item)
        switch item.body {
        case let .text(t)?: Text(styled(t, item)).font(.body).foregroundStyle(Palette.ink)
        case .location?: Text("\(name) shared a location").font(.body)
        case let .sos(_, _, _, cancelled)?:
            if cancelled { Text("\(name) cancelled the SOS").font(.body) } else { Text("SOS from \(name)").font(.body.bold()).foregroundStyle(Palette.emergency) }
        case .voiceNote?: Label("Voice note", systemImage: "waveform").font(.body)
        case nil: EmptyView()
        }
    }

    private func quote(_ r: FfiReply) -> some View {
        Button { scrollTarget = r.msgUuid.hex } label: {
            HStack(spacing: 8) {
                RoundedRectangle(cornerRadius: 1.5).fill(r.state == .present ? Palette.amber : Palette.outline).frame(width: 3)
                VStack(alignment: .leading, spacing: 2) {
                    Text(verbatim: r.sender.hex == host.nodeId ? String(localized: "You") : host.displayName(r.sender.hex))
                        .font(.caption.bold()).foregroundStyle(r.state == .present ? Palette.amber : Palette.muted)
                    Text(verbatim: quoteText(r)).font(.caption).italic(r.state != .present).foregroundStyle(Palette.muted).lineLimit(2)
                }
                Spacer(minLength: 0)
            }
            .padding(8)
            .background(Palette.graphite.opacity(0.5), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        }
        .buttonStyle(.plain)
        .disabled(r.state != .present)
        .accessibilityHint(Text("Shows the original message"))
    }

    private func reactions(_ item: FfiChatItem) -> some View {
        FlowLayout(spacing: 6) {
            ForEach(item.reactions, id: \.emoji) { r in
                Button { toggle(item, r.emoji) } label: {
                    Text(verbatim: "\(r.emoji) \(r.count)").font(.caption)
                        .padding(.horizontal, 8).padding(.vertical, 4)
                        .background(r.mine ? Palette.amber.opacity(0.25) : Palette.elevated, in: Capsule())
                        .overlay(Capsule().stroke(r.mine ? Palette.amber : Palette.outline))
                }
                .buttonStyle(.plain)
                .accessibilityLabel(r.mine ? Text("\(r.emoji) \(Int(r.count)), including you") : Text(verbatim: "\(r.emoji) \(r.count)"))
            }
        }
        .frame(maxWidth: 280, alignment: isMine(item) ? .trailing : .leading)
    }

    @ViewBuilder private func menu(_ item: FfiChatItem, mine: Bool) -> some View {
        Button { startReply(item) } label: { Label("Reply", systemImage: "arrowshape.turn.up.left") }
        Menu {
            ForEach(quick, id: \.self) { e in
                Button { toggle(item, e) } label: { Text(verbatim: e) }.accessibilityLabel(Text("React with \(e)"))
            }
            Button { reactTarget = item } label: { Label("Other emoji…", systemImage: "face.smiling") }
        } label: { Label("React", systemImage: "face.smiling") }
        if canEdit(item) { Button { startEdit(item) } label: { Label("Edit", systemImage: "pencil") } }
        if !otherGroups.isEmpty { Button { forwardTarget = item } label: { Label("Forward", systemImage: "arrowshape.turn.up.right") } }
        if case let .text(t)? = item.body { Button { UIPasteboard.general.string = t } label: { Label("Copy", systemImage: "doc.on.doc") } }
        if mine { Button(role: .destructive) { deleteTarget = item } label: { Label("Delete for everyone", systemImage: "trash") } }
    }

    // MARK: composer

    private var composer: some View {
        VStack(spacing: 8) {
            if let r = replyTo {
                chip(icon: "arrowshape.turn.up.left", title: Text("Replying to \(senderName(r))"), detail: previewText(r), cancelLabel: Text("Cancel reply")) { replyTo = nil }
            }
            if let e = editing {
                chip(icon: "pencil", title: Text("Editing message"), detail: previewText(e), cancelLabel: Text("Cancel editing")) { cancelEdit() }
            }
            if !suggestions.isEmpty {
                ScrollView(.horizontal) {
                    HStack(spacing: 8) {
                        ForEach(suggestions) { c in
                            Button { pick(c) } label: {
                                Group {
                                    if c.id == MentionCandidate.all { Text("@all · everyone") } else { Text(verbatim: "@" + c.name) }
                                }
                                .font(.subheadline).padding(.horizontal, 12).padding(.vertical, 6)
                                .background(Palette.elevated, in: Capsule()).overlay(Capsule().stroke(Palette.outline))
                            }
                            .buttonStyle(.plain)
                            .accessibilityLabel(c.id == MentionCandidate.all ? Text("Mention everyone") : Text("Mention \(c.name)"))
                        }
                    }
                }.scrollIndicators(.hidden)
            }
            HStack(spacing: 8) {
                TextField("Message", text: $text)
                    .padding(.horizontal, 16).padding(.vertical, 12)
                    .background(Palette.graphite, in: Capsule()).overlay(Capsule().stroke(Palette.outline))
                    .onSubmit(send)
                Button(action: send) { Image(systemName: editing == nil ? "paperplane.fill" : "checkmark").frame(width: 48, height: 48) }
                    .background(Palette.amber, in: RoundedRectangle(cornerRadius: 16, style: .continuous)).foregroundStyle(Palette.graphite)
                    .disabled(text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    .accessibilityLabel(editing == nil ? Text("Send") : Text("Save edit"))
            }
        }
        .padding(12)
    }

    private func chip(icon: String, title: Text, detail: String, cancelLabel: Text, onCancel: @escaping () -> Void) -> some View {
        HStack(spacing: 10) {
            Image(systemName: icon).foregroundStyle(Palette.amber)
            VStack(alignment: .leading, spacing: 2) {
                title.font(.caption.bold()).foregroundStyle(Palette.amber)
                Text(verbatim: detail).font(.caption).foregroundStyle(Palette.muted).lineLimit(1)
            }
            Spacer()
            Button(action: onCancel) { Image(systemName: "xmark.circle.fill").font(.title3) }
                .buttonStyle(.plain).foregroundStyle(Palette.muted).accessibilityLabel(cancelLabel)
        }
        .padding(10)
        .background(Palette.elevated, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
    }

    // MARK: mentions

    private var mentionQuery: String? {
        guard let last = text.split(separator: " ", omittingEmptySubsequences: false).last, last.hasPrefix("@") else { return nil }
        return String(last.dropFirst())
    }

    private var suggestions: [MentionCandidate] {
        guard let q = mentionQuery, let g = group else { return [] }
        let f = Self.fold(q)
        var out = g.members
            .filter { $0.node != host.nodeId && (f.isEmpty || Self.fold($0.name).hasPrefix(f)) }
            .map { MentionCandidate(id: $0.node, name: $0.name) }
        if g.isCreator && (f.isEmpty || "all".hasPrefix(f)) { out.insert(MentionCandidate(id: MentionCandidate.all, name: "all"), at: 0) }
        return Array(out.prefix(8))
    }

    private func pick(_ c: MentionCandidate) {
        var parts = text.split(separator: " ", omittingEmptySubsequences: false).map(String.init)
        if !parts.isEmpty { parts.removeLast() }
        parts.append("@" + c.name)
        text = parts.joined(separator: " ") + " "
        if c.id == MentionCandidate.all { mentionAll = true } else { mentioned[c.id] = c.name }
    }

    private static func fold(_ s: String) -> String { s.folding(options: [.caseInsensitive, .diacriticInsensitive], locale: nil) }

    // MARK: actions

    private func send() {
        let t = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !t.isEmpty else { return }
        if let e = editing {
            host.editMessage(gid, e.id, t)
            editing = nil
            text = stashedDraft
            return
        }
        let ids = mentioned.filter { t.contains("@" + $0.value) }.map(\.key)
        let all = mentionAll && (group?.isCreator ?? false) && t.contains("@all")
        host.sendTextV2(gid, t, replyTo: replyTo?.id, mentions: ids, mentionAll: all)
        draftSave?.cancel()
        text = ""; replyTo = nil; mentioned = [:]; mentionAll = false
        host.setDraft(gid, "")
    }

    private func startReply(_ item: FfiChatItem) { if editing != nil { cancelEdit() }; replyTo = item }

    private func startEdit(_ item: FfiChatItem) {
        guard case let .text(t)? = item.body else { return }
        if editing == nil { stashedDraft = text; draftSave?.cancel(); host.setDraft(gid, text) }
        replyTo = nil
        editing = item
        text = t
    }

    private func cancelEdit() { editing = nil; text = stashedDraft }

    /// Tapping your own reaction removes it; any other emoji replaces it.
    private func toggle(_ item: FfiChatItem, _ emoji: String) {
        host.react(gid, item.id, emoji.isEmpty || emoji == myReaction(item) ? "" : emoji)
    }

    // MARK: helpers

    private func myReaction(_ item: FfiChatItem) -> String? { item.reactions.first { $0.mine }?.emoji }
    private func isMine(_ item: FfiChatItem) -> Bool { item.from.hex == host.nodeId }
    private func isGone(_ item: FfiChatItem) -> Bool {
        if item.deleted { return true }
        if case .none = item.body { return true }
        return false
    }

    private func canEdit(_ item: FfiChatItem) -> Bool {
        guard isMine(item), !isGone(item), case .text? = item.body, item.edited < 10 else { return false }
        let now = UInt64(Date().timeIntervalSince1970 * 1000)
        return now < item.sentMs + editWindowMs()
    }

    private func senderName(_ item: FfiChatItem) -> String {
        isMine(item) ? String(localized: "You") : host.displayName(item.from.hex)
    }

    private func hue(_ node: String) -> Int {
        if let p = host.peers[node] { return p.hue }
        return group?.members.first(where: { $0.node == node })?.hue ?? 200
    }

    private func stamp(_ ms: UInt64, dated: Bool) -> String {
        Date(timeIntervalSince1970: Double(ms) / 1000).formatted(date: dated ? .abbreviated : .omitted, time: .shortened)
    }

    private func previewText(_ item: FfiChatItem) -> String {
        let name = senderName(item)
        switch item.body {
        case let .text(t)?: return t
        case .location?: return String(localized: "\(name) shared a location")
        case let .sos(_, _, _, cancelled)?: return cancelled ? String(localized: "\(name) cancelled the SOS") : String(localized: "SOS from \(name)")
        case .voiceNote?: return String(localized: "Voice note")
        case nil: return String(localized: "This message was deleted")
        }
    }

    private func quoteText(_ r: FfiReply) -> String {
        switch r.state {
        case .deleted: return String(localized: "Original message deleted")
        case .missing: return String(localized: "Message not on this device")
        case .present:
            if !r.preview.isEmpty { return r.preview }
            switch r.kind {
            case .text: return ""
            case .voice: return String(localized: "Voice note")
            case .location: return String(localized: "Location")
            case .sos: return String(localized: "SOS")
            }
        }
    }

    /// Bolds and tints every `@name` the message actually mentions.
    private func styled(_ t: String, _ item: FfiChatItem) -> AttributedString {
        var a = AttributedString(t)
        var tokens = item.mentions.map { "@" + host.displayName($0.hex) }
        if item.mentionAll { tokens.append("@all") }
        for tok in tokens where tok.count > 1 {
            var from = a.startIndex
            while from < a.endIndex, let r = a[from..<a.endIndex].range(of: tok) {
                a[r].swiftUI.foregroundColor = Palette.amber
                a[r].swiftUI.font = Font.body.bold()
                from = r.upperBound
            }
        }
        return a
    }
}

private struct MentionCandidate: Identifiable {
    static let all = "all"
    let id: String // member node hex, or `all`
    let name: String
}

// MARK: sheets

private struct ReactSheet: View {
    let quick: [String]
    let mine: String?
    let onPick: (String) -> Void
    @State private var custom = ""
    @Environment(\.dismiss) private var dismiss

    init(quick: [String], mine: String?, onPick: @escaping (String) -> Void) {
        self.quick = quick; self.mine = mine; self.onPick = onPick
    }

    var body: some View {
        VStack(spacing: 16) {
            Text("React").font(.headline).padding(.top, 8)
            HStack(spacing: 12) {
                ForEach(quick, id: \.self) { e in
                    Button { onPick(e); dismiss() } label: {
                        Text(verbatim: e).font(.largeTitle).padding(6)
                            .background(e == mine ? Palette.amber.opacity(0.25) : Color.clear, in: Circle())
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(Text("React with \(e)"))
                }
            }
            TextField("Any emoji", text: $custom)
                .multilineTextAlignment(.center).font(.title)
                .padding(.vertical, 10).background(Palette.graphite, in: Capsule()).overlay(Capsule().stroke(Palette.outline))
                .onSubmit(pickCustom)
                .onChange(of: custom) { _, _ in if custom.contains(where: \.isEmojiLike) { pickCustom() } }
            if mine != nil {
                Button("Remove my reaction", role: .destructive) { onPick(""); dismiss() }
            }
            Spacer(minLength: 0)
        }
        .padding(20)
        .presentationDetents([.height(300)]).presentationBackground(Palette.surface)
    }

    private func pickCustom() {
        guard let c = custom.first(where: \.isEmojiLike) else { return }
        onPick(String(c)); dismiss()
    }
}

private struct ForwardSheet: View {
    let groups: [GroupState]
    let onPick: (GroupState) -> Void
    @Environment(\.dismiss) private var dismiss

    init(groups: [GroupState], onPick: @escaping (GroupState) -> Void) { self.groups = groups; self.onPick = onPick }

    var body: some View {
        NavigationStack {
            List(groups) { g in
                Button { onPick(g); dismiss() } label: { Text(verbatim: g.name).foregroundStyle(Palette.ink) }
                    .listRowBackground(Palette.elevated)
            }
            .scrollContentBackground(.hidden)
            .navigationTitle(Text("Forward to"))
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } } }
        }
        .presentationDetents([.medium, .large]).presentationBackground(Palette.surface)
    }
}

private struct EditHistorySheet: View {
    let revisions: [FfiEditRevision]
    @Environment(\.dismiss) private var dismiss

    init(revisions: [FfiEditRevision]) { self.revisions = revisions }

    var body: some View {
        NavigationStack {
            List {
                ForEach(Array(revisions.enumerated()), id: \.offset) { i, r in
                    VStack(alignment: .leading, spacing: 4) {
                        HStack {
                            if i == 0 { Text("Original") } else { Text("Edit \(i)") }
                            Spacer()
                            Text(verbatim: Date(timeIntervalSince1970: Double(r.sentMs) / 1000).formatted(date: .abbreviated, time: .shortened))
                                .font(.caption2.monospaced())
                        }
                        .font(.caption.bold()).foregroundStyle(Palette.muted)
                        Text(verbatim: r.text).foregroundStyle(Palette.ink)
                    }
                    .listRowBackground(Palette.elevated)
                }
            }
            .scrollContentBackground(.hidden)
            .navigationTitle(Text("Edit history"))
            .navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
        }
        .presentationDetents([.medium, .large]).presentationBackground(Palette.surface)
    }
}

/// Wraps reaction chips onto as many rows as needed.
private struct FlowLayout: Layout {
    var spacing: CGFloat = 6

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let maxW = proposal.width ?? .infinity
        var x: CGFloat = 0, y: CGFloat = 0, rowH: CGFloat = 0, width: CGFloat = 0
        for s in subviews {
            let sz = s.sizeThatFits(.unspecified)
            if x > 0 && x + sz.width > maxW { x = 0; y += rowH + spacing; rowH = 0 }
            x += sz.width + spacing
            rowH = max(rowH, sz.height)
            width = max(width, x - spacing)
        }
        return CGSize(width: width, height: y + rowH)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var x = bounds.minX, y = bounds.minY, rowH: CGFloat = 0
        for s in subviews {
            let sz = s.sizeThatFits(.unspecified)
            if x > bounds.minX && x + sz.width > bounds.maxX { x = bounds.minX; y += rowH + spacing; rowH = 0 }
            s.place(at: CGPoint(x: x, y: y), proposal: ProposedViewSize(sz))
            x += sz.width + spacing
            rowH = max(rowH, sz.height)
        }
    }
}

private extension Character {
    /// True for characters that render as emoji (excludes plain digits, `#`, `*`).
    var isEmojiLike: Bool {
        guard let s = unicodeScalars.first else { return false }
        return s.properties.isEmojiPresentation || (s.properties.isEmoji && unicodeScalars.count > 1)
    }
}
