import CoreNFC
import Foundation

/// Writes a group invite (https universal link) to an NFC tag as one NDEF URI record.
/// Tapping the tag with any iPhone opens Titi through the applinks association.
final class NfcInviteWriter: NSObject, NFCNDEFReaderSessionDelegate, @unchecked Sendable {
    static var available: Bool { NFCNDEFReaderSession.readingAvailable }

    private var session: NFCNDEFReaderSession?
    private var message: NFCNDEFMessage?

    func write(_ url: URL) {
        guard Self.available, let payload = NFCNDEFPayload.wellKnownTypeURIPayload(url: url) else { return }
        message = NFCNDEFMessage(records: [payload])
        let s = NFCNDEFReaderSession(delegate: self, queue: nil, invalidateAfterFirstRead: false)
        s.alertMessage = String(localized: "Hold your iPhone near the NFC tag.")
        session = s
        s.begin()
    }

    // MARK: NFCNDEFReaderSessionDelegate
    func readerSession(_ session: NFCNDEFReaderSession, didInvalidateWithError error: Error) {
        self.session = nil
    }

    func readerSession(_ session: NFCNDEFReaderSession, didDetectNDEFs messages: [NFCNDEFMessage]) {}

    func readerSession(_ session: NFCNDEFReaderSession, didDetect tags: [NFCNDEFTag]) {
        guard let msg = message else { session.invalidate(); return }
        guard tags.count == 1, let tag = tags.first else {
            session.alertMessage = String(localized: "More than one tag found. Keep just one near your iPhone.")
            session.restartPolling()
            return
        }
        session.connect(to: tag) { error in
            if error != nil {
                session.invalidate(errorMessage: String(localized: "Could not connect to the tag. Try again."))
                return
            }
            tag.queryNDEFStatus { status, capacity, error in
                if error != nil {
                    session.invalidate(errorMessage: String(localized: "Could not read the tag. Try again."))
                    return
                }
                switch status {
                case .readWrite:
                    if capacity < msg.length {
                        session.invalidate(errorMessage: String(localized: "This tag is too small for the invite."))
                        return
                    }
                    tag.writeNDEF(msg) { error in
                        if error != nil {
                            session.invalidate(errorMessage: String(localized: "Writing failed. Try again."))
                        } else {
                            session.alertMessage = String(localized: "Done. Tap the tag to join the group.")
                            session.invalidate()
                        }
                    }
                case .readOnly:
                    session.invalidate(errorMessage: String(localized: "This tag is locked and cannot be written."))
                case .notSupported:
                    session.invalidate(errorMessage: String(localized: "This tag does not support NDEF."))
                @unknown default:
                    session.invalidate(errorMessage: String(localized: "This tag does not support NDEF."))
                }
            }
        }
    }
}
