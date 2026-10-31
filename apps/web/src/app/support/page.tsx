import type { Metadata } from "next";
import Link from "next/link";

export const metadata: Metadata = {
  title: "Help & support — Titi",
  description: "How to use Titi, how to report abuse or block someone, and how to reach us.",
};

const EMAIL = "support@codai.ro";

export default function Support() {
  return (
    <main className="mx-auto max-w-2xl px-6 py-12 text-[15px] leading-relaxed">
      <Link href="/" className="text-sm text-amber">← Titi</Link>
      <h1 className="mt-4 text-3xl font-extrabold">Help &amp; support</h1>
      <p className="mt-1 text-sm text-muted">Ajutor și asistență</p>

      <Section title="Contact / Contact">
        <p>Write to <a className="text-amber underline underline-offset-2" href={`mailto:${EMAIL}`}>{EMAIL}</a>. We answer within 24 hours on working days; reports of abuse are handled first.</p>
        <p className="mt-2">Scrie-ne la <a className="text-amber underline underline-offset-2" href={`mailto:${EMAIL}`}>{EMAIL}</a>. Răspundem în 24 de ore în zilele lucrătoare; sesizările de abuz au prioritate.</p>
      </Section>

      <Section title="Get started / Primii pași">
        <ol className="list-decimal pl-5">
          <li>Open Titi, pick a name and create a group.</li>
          <li>Tap the invite button: say the three words and two digits, show the QR code, or share the link.</li>
          <li>Hold the Talk button to speak. Release to listen. Messages, replies and reactions live in the chat.</li>
        </ol>
        <p className="mt-2">Deschide Titi, alege un nume și creează un grup. Invită prietenii cu cele trei cuvinte și două cifre, cu codul QR sau cu linkul. Ține apăsat butonul Vorbește ca să vorbești.</p>
      </Section>

      <Section title="Without internet / Fără internet">
        <p>Phones talk directly over Bluetooth and, on Android, Wi‑Fi and hotspot. With internet, members far apart meet through our relay, which only forwards encrypted frames.</p>
        <p className="mt-2">Telefoanele vorbesc direct prin Bluetooth și, pe Android, prin Wi‑Fi și hotspot. Cu internet, membrii aflați departe se întâlnesc prin releul nostru, care transmite doar cadre criptate.</p>
      </Section>

      <Section title="Report or block someone / Raportează sau blochează">
        <p>Groups are invite-only. If someone bothers you: long-press their message or their avatar and choose <b>Block</b> — you stop seeing their messages and hearing their voice on this phone. Choose <b>Report</b> to send us the message and the sender’s device id; we act within 24 hours and help the group creator remove them. You can also leave the group at any time, and the creator can delete it for everyone.</p>
        <p className="mt-2">Grupurile sunt doar pe bază de invitație. Dacă cineva te deranjează, ține apăsat pe mesajul sau avatarul lui și alege <b>Blochează</b> sau <b>Raportează</b>. Poți părăsi grupul oricând, iar creatorul îl poate șterge pentru toți.</p>
      </Section>

      <Section title="Delete your data / Șterge-ți datele">
        <p>Settings → <b>Delete identity &amp; all data</b> removes your device key, groups and messages from the phone. We keep no account or copy on our servers. See the <Link className="text-amber underline underline-offset-2" href="/privacy">privacy policy</Link>.</p>
        <p className="mt-2">Setări → <b>Șterge identitatea și toate datele</b>. Nu păstrăm conturi sau copii pe servere.</p>
      </Section>
    </main>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="mt-8">
      <h2 className="mb-2 text-xl font-bold">{title}</h2>
      <div className="text-ink/90">{children}</div>
    </section>
  );
}
