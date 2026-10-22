import styles from "./Announcer.module.css";

let region: HTMLElement | null = null;
let flip = false;

/**
 * Says `message` to assistive tech through one polite live region, created on first use and
 * kept in `document.body`. For changes that have no visible text of their own, such as
 * "Added #rust". The same message twice in a row is still announced.
 */
export function announce(message: string): void {
  if (!region || !region.isConnected) {
    region = document.createElement("div");
    region.className = styles.live ?? "";
    region.setAttribute("role", "status");
    region.setAttribute("aria-live", "polite");
    region.setAttribute("aria-atomic", "true");
    document.body.appendChild(region);
  }
  flip = !flip;
  region.textContent = flip ? message : message + String.fromCharCode(0x200b);
}
