import { TriangleAlert } from "lucide-react";
import { Button, EmptyState } from "../ui";

/** Shown instead of the app when a screen throws: calm, with a way to start over. */
export function ErrorFallback() {
  return (
    <EmptyState
      icon={TriangleAlert}
      title="Something went wrong"
      action={
        <Button variant="primary" onClick={() => window.location.reload()}>
          Reload Harneo
        </Button>
      }
    >
      Your library and projects are untouched. Reloading usually fixes it.
    </EmptyState>
  );
}
