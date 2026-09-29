import type { Metadata } from "next";
import Link from "next/link";

export const metadata: Metadata = { title: "Privacy policy — Titi", description: "What Titi collects (almost nothing) and how voice and messages are protected." };

const UPDATED = "14 September 2026";

export default function Privacy() {
  return (
    <main className="mx-auto max-w-2xl px-6 py-12 text-[15px] leading-relaxed">
      <Link href="/" className="text-sm text-amber">← Titi</Link>
      <h1 className="mt-4 text-3xl font-extrabold">Privacy policy</h1>
      <p className="mt-1 text-sm text-muted">Politica de confidențialitate · Last updated {UPDATED}</p>

      <Section title="Summary / Pe scurt">
        <p>Titi is an offline-first walkie-talkie. Your voice and messages travel directly between phones over Wi‑Fi, hotspot or Bluetooth and are <b>end-to-end encrypted</b> with a key that only your group holds. We do not have accounts, we do not collect analytics, and we do not sell or share data.</p>
        <p className="mt-2">Titi este un walkie-talkie care funcționează fără internet. Vocea și mesajele circulă direct între telefoane și sunt <b>criptate end-to-end</b> cu o cheie pe care o are doar grupul tău. Nu există conturi, nu colectăm analytics și nu vindem sau partajăm date.</p>
      </Section>

      <Section title="Data stored on your device">
        <ul className="list-disc pl-5">
          <li>Your display name and colour (chosen by you).</li>
          <li>A random device identity key pair, generated on first launch and never sent to us.</li>
          <li>Your groups (name, members’ display names and public keys, the group key), encrypted at rest by the operating system.</li>
          <li>Recent text messages of your groups (kept locally; you can leave a group to delete them).</li>
        </ul>
        <p className="mt-2">Uninstalling the app deletes all of the above.</p>
      </Section>

      <Section title="Data that leaves your device">
        <p><b>Nearby mode (no internet):</b> nothing leaves the phones in range. Discovery broadcasts contain your display name, colour and a 4-byte hash of each group, so nearby phones can show you in “Nearby”.</p>
        <p className="mt-2"><b>Internet mode (optional):</b> when you are online, the app connects to our relay server (Google Cloud Run, region europe-west1) so members of the same group who are far away can still talk. The relay sees only: your display name and colour, a 4-byte group hash, your IP address (as any server does), and <b>encrypted</b> voice/message frames it cannot read. It stores nothing after you disconnect (a resume token lives at most 5 minutes in memory). No logs of who talked to whom are retained.</p>
        <p className="mt-2"><b>Location:</b> only if you tap SOS or “share location”, your coordinates are sent — encrypted — to your group. We never receive them.</p>
      </Section>

      <Section title="Permissions">
        <ul className="list-disc pl-5">
          <li><b>Microphone</b> — to transmit your voice while you hold the Talk button or have Open mic on.</li>
          <li><b>Nearby devices / Bluetooth / Wi‑Fi</b> — to find and connect to other phones running Titi without internet.</li>
          <li><b>Location (while in use)</b> — required by Android for Wi‑Fi Direct / Wi‑Fi Aware scanning, and for the optional SOS. Titi does not track your position.</li>
          <li><b>Camera</b> — only to scan a group QR code.</li>
          <li><b>Notifications / foreground service</b> — to keep the radio on with the screen off.</li>
        </ul>
      </Section>

      <Section title="Children">
        <p>Titi is not directed at children under 13 and collects no personal data from anyone.</p>
      </Section>

      <Section title="Your rights (GDPR)">
        <p>Because we hold no personal data on our servers, there is nothing for us to export or erase; all data is on your device and under your control. For any question contact <a className="text-amber underline underline-offset-2" href="mailto:privacy@titi.dragoscatalin.ro">privacy@titi.dragoscatalin.ro</a>.</p>
      </Section>

      <Section title="Changes">
        <p>We will post any change here and update the date above.</p>
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
