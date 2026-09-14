import type { NextConfig } from "next";
import withSerwistInit from "@serwist/next";

const withSerwist = withSerwistInit({
  swSrc: "src/sw.ts",
  swDest: "public/sw.js",
  disable: process.env.NODE_ENV === "development",
});

const config: NextConfig = {
  reactStrictMode: true,
  reactCompiler: true,
  turbopack: {},
  headers: async () => [
    {
      source: "/(.*)",
      headers: [
        { key: "X-Content-Type-Options", value: "nosniff" },
        { key: "X-Frame-Options", value: "DENY" },
        { key: "Referrer-Policy", value: "strict-origin-when-cross-origin" },
        { key: "Permissions-Policy", value: "microphone=(self), camera=(self), geolocation=(self)" },
      ],
    },
    { source: "/.well-known/assetlinks.json", headers: [{ key: "Content-Type", value: "application/json" }] },
  ],
};

export default withSerwist(config);
