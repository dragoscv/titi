import type { MetadataRoute } from "next";
import { brand } from "@titi/app-ui/brand-tokens";

export default function manifest(): MetadataRoute.Manifest {
  return {
    name: "Titi — talk anywhere",
    short_name: "Titi",
    description: "Offline-first walkie-talkie for groups. Private radio over Wi-Fi, hotspot and Bluetooth.",
    start_url: "/",
    scope: "/",
    display: "standalone",
    orientation: "portrait",
    background_color: brand.dark["bg.canvas"],
    theme_color: brand.dark["bg.canvas"],
    lang: "en",
    categories: ["communication", "utilities"],
    icons: [
      { src: "/icon-192.png", sizes: "192x192", type: "image/png" },
      { src: "/icon-512.png", sizes: "512x512", type: "image/png" },
      { src: "/icon-512-maskable.png", sizes: "512x512", type: "image/png", purpose: "maskable" },
    ],
    shortcuts: [{ name: "Join a group", url: "/join", description: "Enter a code" }],
  };
}
