import { Panel } from "../ui";
import { useAppInfo } from "./useAppInfo";

export function AboutPanel() {
  const state = useAppInfo();

  if (state.status === "loading") return <p role="status">Loading…</p>;
  if (state.status === "error") {
    return <p role="alert">Could not load app info: {state.message}</p>;
  }
  return (
    <Panel title="About Harneo" titleId="about-title">
      <p>Version {state.info.version}</p>
      <p>Data folder: {state.info.dataDir}</p>
    </Panel>
  );
}
