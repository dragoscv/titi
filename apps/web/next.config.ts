import type { NextConfig } from "next";
import { withSerwist } from "@serwist/turbopack";

const config: NextConfig = {
  reactStrictMode: true,
  reactCompiler: true,
  transpilePackages: ["@titi/app-ui"],
  turbopack: {},
  // scripts/gates.ps1 type-checks first; Vercel (unset) still type-checks during its build
  typescript: { ignoreBuildErrors: process.env.TITI_TYPECHECKED === "1" },
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
