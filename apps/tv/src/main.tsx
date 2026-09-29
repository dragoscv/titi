import { createRoot } from "react-dom/client";
import { installHost } from "@titi/app-ui";
import { TvShell } from "@titi/app-ui/tv";
import { WebHost } from "@titi/app-ui/web";
import "./styles.css";

type Tizen = { tvinputdevice?: { registerKeyBatch(keys: string[]): void } };
try {
  // colour/media keys are not delivered unless registered (arrows/OK/Back are)
  (window as unknown as { tizen?: Tizen }).tizen?.tvinputdevice?.registerKeyBatch(["MediaPlayPause", "ColorF0Red", "ColorF1Green"]);
} catch { /* not on Tizen (browser dev) */ }

installHost(new WebHost({ kind: "tv", tenFoot: true }));
createRoot(document.getElementById("root")!).render(<TvShell />);
