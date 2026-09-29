"use client";
import { installHost, Shell } from "@titi/app-ui";
import { WebHost } from "@titi/app-ui/web";

// Module scope: installed once per page load, before any screen reads `host`.
if (typeof window !== "undefined") installHost(new WebHost());

export function App({ joinLink }: { joinLink?: string }) {
  return <Shell joinLink={joinLink} />;
}
