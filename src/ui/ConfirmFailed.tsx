import { ShieldQuestion } from "lucide-react";
import type { Confirmation } from "../bindings/Confirmation";
import { Button } from "./Button";
import { Callout } from "./Callout";
import styles from "./ConfirmFailed.module.css";
import { confirmationFailed, confirmationReason } from "./confirmFailure";

/**
 * Phase 2 could not confirm an artifact's findings: why, and a button to ask Claude again.
 * Renders nothing for any other state.
 */
export function ConfirmFailed({
  confirmation,
  onRetry,
  busy,
  error,
}: {
  confirmation: Confirmation | null | undefined;
  onRetry: () => void;
  busy: boolean;
  /** Why the retry itself could not start. */
  error?: string | null;
}) {
  if (!confirmation || !confirmationFailed(confirmation)) return null;
  return (
    <Callout tone="warn" icon={ShieldQuestion}>
      <p className={styles.text}>
        {`Claude could not confirm the findings: ${confirmationReason(confirmation)}. Findings are shown as the rules found them.`}
      </p>
      {error && (
        <p role="alert" className={styles.text}>
          {`Could not ask Claude: ${error}`}
        </p>
      )}
      <Button small disabled={busy} onClick={onRetry}>
        {busy ? "Confirming…" : "Confirm with Claude"}
      </Button>
    </Callout>
  );
}
