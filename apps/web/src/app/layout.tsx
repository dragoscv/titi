import type { Metadata, Viewport } from "next";
import { Manrope, Geist_Mono } from "next/font/google";
import "./globals.css";

const manrope = Manrope({ subsets: ["latin", "latin-ext"], variable: "--font-sans", display: "swap" });
const geistMono = Geist_Mono({ subsets: ["latin"], variable: "--font-mono", display: "swap" });

export const metadata: Metadata = {
  title: "Titi — talk anywhere",
  description: "Offline-first walkie-talkie. Private radio over Wi-Fi, hotspot and Bluetooth; internet when it exists.",
  applicationName: "Titi",
  manifest: "/manifest.webmanifest",
  appleWebApp: { capable: true, statusBarStyle: "black-translucent", title: "Titi" },
  icons: { icon: "/icon.svg", apple: "/apple-icon.png" },
};

export const viewport: Viewport = {
  themeColor: "#0E1013",
  width: "device-width",
  initialScale: 1,
  viewportFit: "cover",
  maximumScale: 1,
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className={`${manrope.variable} ${geistMono.variable}`}>
      <body className="min-h-dvh antialiased">{children}</body>
    </html>
  );
}
