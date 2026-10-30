// Minimal i18n for the shared UI (no dependency). Only chat strings live here
// for now; every key must exist in both locales (test/i18n.test.ts).

const en = {
  "chat.title": "Chat",
  "chat.empty": "No messages yet. Texts are stored and forwarded when a path exists.",
  "chat.placeholder": "Message",
  "chat.send": "Send",
  "chat.save": "Save edit",
  "chat.loadOlder": "Load older messages",
  "chat.search": "Search messages",
  "chat.searchPlaceholder": "Search in this chat",
  "chat.searchClose": "Close search",
  "chat.searchNone": "No messages found",
  "chat.deleted": "This message was deleted",
  "chat.edited": "edited",
  "chat.editedTimes": "edited {n}×",
  "chat.editHistory": "Edit history",
  "chat.forwarded": "Forwarded",
  "chat.forwardedMany": "Forwarded many times",
  "chat.replyDeleted": "Original message deleted",
  "chat.replyMissing": "Message not on this device",
  "chat.kind.voice": "Voice note",
  "chat.kind.location": "Location",
  "chat.kind.sos": "SOS",
  "chat.location": "{name} shared a location",
  "chat.sos": "SOS from {name}",
  "chat.sosCancelled": "{name} cancelled the SOS",
  "chat.you": "You",
  "chat.actions": "Message actions",
  "chat.actionsFor": "Actions for message from {name}",
  "chat.reply": "Reply",
  "chat.react": "React",
  "chat.reactWith": "React with {emoji}",
  "chat.reactOther": "Any emoji",
  "chat.reactAdd": "Add reaction",
  "chat.reaction": "{emoji} {count}",
  "chat.reactionMine": "{emoji} {count}, including you",
  "chat.edit": "Edit",
  "chat.editing": "Editing message",
  "chat.delete": "Delete for everyone",
  "chat.deleteConfirm": "Delete this message for everyone?",
  "chat.deleteYes": "Delete",
  "chat.forward": "Forward",
  "chat.forwardTo": "Forward to…",
  "chat.forwardDone": "Forwarded to {name}",
  "chat.copy": "Copy",
  "chat.copied": "Copied",
  "chat.cancel": "Cancel",
  "chat.close": "Close",
  "chat.replyingTo": "Replying to {name}",
  "chat.cancelReply": "Cancel reply",
  "chat.cancelEdit": "Cancel edit",
  "chat.mentionAll": "Everyone in the group",
  "chat.mentions": "Mention someone",
  "chat.mentionedYou": "{name} mentioned you",
  "chat.sent": "Sent",
  "chat.delivered": "Delivered",
  "tv.react": "React",
  "tv.reactPick": "Choose a message, then press OK",
  "tv.reactNone": "No messages in this group yet",
} as const;

export type I18nKey = keyof typeof en;

const ro: Record<I18nKey, string> = {
  "chat.title": "Chat",
  "chat.empty": "Niciun mesaj încă. Textele sunt păstrate și trimise mai departe când există o cale.",
  "chat.placeholder": "Mesaj",
  "chat.send": "Trimite",
  "chat.save": "Salvează modificarea",
  "chat.loadOlder": "Încarcă mesajele mai vechi",
  "chat.search": "Caută mesaje",
  "chat.searchPlaceholder": "Caută în acest chat",
  "chat.searchClose": "Închide căutarea",
  "chat.searchNone": "Niciun mesaj găsit",
  "chat.deleted": "Acest mesaj a fost șters",
  "chat.edited": "editat",
  "chat.editedTimes": "editat de {n} ori",
  "chat.editHistory": "Istoricul modificărilor",
  "chat.forwarded": "Redirecționat",
  "chat.forwardedMany": "Redirecționat de multe ori",
  "chat.replyDeleted": "Mesajul original a fost șters",
  "chat.replyMissing": "Mesajul nu este pe acest dispozitiv",
  "chat.kind.voice": "Mesaj vocal",
  "chat.kind.location": "Locație",
  "chat.kind.sos": "SOS",
  "chat.location": "{name} a trimis o locație",
  "chat.sos": "SOS de la {name}",
  "chat.sosCancelled": "{name} a anulat SOS-ul",
  "chat.you": "Tu",
  "chat.actions": "Acțiuni mesaj",
  "chat.actionsFor": "Acțiuni pentru mesajul de la {name}",
  "chat.reply": "Răspunde",
  "chat.react": "Reacționează",
  "chat.reactWith": "Reacționează cu {emoji}",
  "chat.reactOther": "Orice emoji",
  "chat.reactAdd": "Adaugă reacția",
  "chat.reaction": "{emoji} {count}",
  "chat.reactionMine": "{emoji} {count}, inclusiv tu",
  "chat.edit": "Editează",
  "chat.editing": "Editezi mesajul",
  "chat.delete": "Șterge pentru toți",
  "chat.deleteConfirm": "Ștergi acest mesaj pentru toți?",
  "chat.deleteYes": "Șterge",
  "chat.forward": "Redirecționează",
  "chat.forwardTo": "Redirecționează către…",
  "chat.forwardDone": "Redirecționat către {name}",
  "chat.copy": "Copiază",
  "chat.copied": "Copiat",
  "chat.cancel": "Anulează",
  "chat.close": "Închide",
  "chat.replyingTo": "Răspunzi lui {name}",
  "chat.cancelReply": "Anulează răspunsul",
  "chat.cancelEdit": "Anulează editarea",
  "chat.mentionAll": "Toată lumea din grup",
  "chat.mentions": "Menționează pe cineva",
  "chat.mentionedYou": "{name} te-a menționat",
  "chat.sent": "Trimis",
  "chat.delivered": "Livrat",
  "tv.react": "Reacționează",
  "tv.reactPick": "Alege un mesaj, apoi apasă OK",
  "tv.reactNone": "Încă nu există mesaje în acest grup",
};

export type Locale = "en" | "ro";
export const dictionaries: Record<Locale, Record<I18nKey, string>> = { en, ro };

let override: Locale | null = null;

/** Force a locale (settings); `null` follows the system language again. */
export function setLocale(l: Locale | null) { override = l; }

export function detectLocale(lang: string | undefined): Locale {
  return lang?.toLowerCase().startsWith("ro") ? "ro" : "en";
}

export function locale(): Locale {
  return override ?? detectLocale(typeof navigator === "undefined" ? undefined : navigator.language);
}

/** Translate `key`, replacing `{var}` placeholders. */
export function t(key: I18nKey, vars?: Record<string, string | number>): string {
  const s = dictionaries[locale()][key];
  return vars ? s.replace(/\{(\w+)\}/g, (m, k: string) => (k in vars ? String(vars[k]) : m)) : s;
}
