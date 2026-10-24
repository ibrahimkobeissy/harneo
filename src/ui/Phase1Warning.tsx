import { ShieldQuestion } from "lucide-react";
import { Link } from "react-router";
import { Callout } from "./Callout";

/** The phase 1 only warning: shown whenever Claude is not confirming the audit's findings. */
export function Phase1Warning({ cliMissing }: { cliMissing: boolean }) {
  return (
    <Callout tone="warn" icon={ShieldQuestion}>
      <b>Phase 1 only.</b> These findings come from pattern matching and include
      false positives; Claude has not confirmed them.{" "}
      {cliMissing ? (
        <Link to="/settings#claude-settings">
          Set up the Claude CLI to confirm them
        </Link>
      ) : (
        <Link to="/settings#claude-settings">
          Turn on Confirm security findings with Claude
        </Link>
      )}
    </Callout>
  );
}
