import { useContext, type RefObject } from "react";
import { UNSAFE_DataRouterContext, useBlocker } from "react-router";
import { Dialog } from "./Dialog";

/**
 * Asks before the user leaves with unsaved changes. Blockers need a data router; under a plain
 * `MemoryRouter` (some tests) there is nothing to block and this renders nothing.
 */
export function LeaveGuard({
  when,
  bypass,
}: {
  when: boolean;
  bypass: RefObject<boolean>;
}) {
  const router = useContext(UNSAFE_DataRouterContext);
  if (!router) return null;
  return <Blocking when={when} bypass={bypass} />;
}

function Blocking({
  when,
  bypass,
}: {
  when: boolean;
  bypass: RefObject<boolean>;
}) {
  const blocker = useBlocker(
    ({ currentLocation, nextLocation }) =>
      when &&
      !bypass.current &&
      currentLocation.pathname !== nextLocation.pathname,
  );
  if (blocker.state !== "blocked") return null;
  return (
    <Dialog
      title="Leave without saving?"
      sub="Your changes stay as a draft in this profile until you save or discard them."
      submit="Leave"
      busy={false}
      onSubmit={() => blocker.proceed()}
      onClose={() => blocker.reset()}
    >
      <p>Stay to save them first.</p>
    </Dialog>
  );
}
