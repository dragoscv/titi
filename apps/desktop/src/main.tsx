import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { installHost, Shell } from "@titi/app-ui";
import { DesktopHost } from "./host";
import { Overlay } from "./Overlay";
import "./styles.css";

const isOverlay = location.hash === "#overlay";
document.documentElement.dataset.platform = "desktop";
if (isOverlay) document.documentElement.dataset.overlay = "1";
installHost(new DesktopHost());

createRoot(document.getElementById("root")!).render(<StrictMode>{isOverlay ? <Overlay /> : <Shell />}</StrictMode>);
