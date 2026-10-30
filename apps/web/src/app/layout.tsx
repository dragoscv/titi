import type { Metadata, Viewport } from "next";
import { Manrope, Geist_Mono } from "next/font/google";
import { SerwistProvider } from "@serwist/turbopack/react";
import { brand } from "@titi/app-ui/brand-tokens";
import "./globals.css";

const manrope = Manrope({ subsets: ["latin", "latin-ext"], variable: "--font-sans", display: "swap" });
const geistMono = Geist_Mono({ subsets: ["latin", "latin-ext"], variable: "--font-mono", display: "swap" });

export const metadata: Metadata = {
  title: "Titi — talk anywhere",
  description: "Offline-first walkie-talkie. Private radio over Wi-Fi, hotspot and Bluetooth; internet when it exists.",
  applicationName: "Titi",
  manifest: "/manifest.webmanifest",
  appleWebApp: { capable: true, statusBarStyle: "black-translucent", title: "Titi" },
  icons: {
    icon: [
      { url: "/favicon.ico", sizes: "32x32" },
      { url: process.env.VERCEL_ENV === "production" || !process.env.VERCEL_ENV ? "/icon.svg" : "/staging-icon.svg", type: "image/svg+xml" },
    ],
    apple: "/apple-icon.png",
  },
  openGraph: { type: "website", siteName: "Titi", images: [{ url: "/og.png", width: 1200, height: 630, alt: "Titi — talk anywhere, no signal needed" }] },
  twitter: { card: "summary_large_image", images: ["/og.png"] },
};

export const viewport: Viewport = {
  themeColor: brand.dark["bg.canvas"],
  width: "device-width",
  initialScale: 1,
  viewportFit: "cover",
  maximumScale: 1,
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className={`${manrope.variable} ${geistMono.variable}`}>
      <body className="min-h-dvh antialiased">
        <SerwistProvider swUrl="/serwist/sw.js" disable={process.env.NODE_ENV === "development"}>{children}</SerwistProvider>
      </body>
    </html>
  );
}
