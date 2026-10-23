import { ArrowLeft } from "lucide-react";
import { useEffect, useState } from "react";
import { Link, useParams } from "react-router";
import type { ProfileView } from "../../bindings/ProfileView";
import type { Resolved } from "../../bindings/Resolved";
import { profileGet, profileResolve } from "../../lib/ipc";
import { EmptyState, kinds, Screen } from "../../ui";
import styles from "./Profiles.module.css";

const back = (
  <Link to="/profiles" className={styles.top}>
    <ArrowLeft size={14} aria-hidden="true" />
    Profiles
  </Link>
);

/**
 * STUB: the profile builder (a later ticket) replaces this screen. It only shows a profile's
 * name, version and resolved artifacts so the list's links have somewhere to go.
 */
export function ProfileStub() {
  const { id } = useParams();
  if (id === "new") {
    return (
      <Screen title="New profile" crumb={back}>
        <p className={styles.stubNote}>
          The profile builder is not built yet. It will create the profile here.
        </p>
      </Screen>
    );
  }
  return <ExistingStub id={id ?? ""} />;
}

type State =
  | { status: "loading" }
  | { status: "error"; message: string }
  | { status: "ready"; view: ProfileView; resolved: Resolved };

function ExistingStub({ id }: { id: string }) {
  const [state, setState] = useState<State>({ status: "loading" });
  useEffect(() => {
    let stale = false;
    (async () => {
      const view = await profileGet(BigInt(id));
      const resolved = await profileResolve(view.spec);
      if (!stale) setState({ status: "ready", view, resolved });
    })().catch((err) => {
      if (!stale) setState({ status: "error", message: String(err) });
    });
    return () => {
      stale = true;
    };
  }, [id]);

  if (state.status !== "ready") {
    return (
      <Screen title="Profile" crumb={back}>
        {state.status === "error" ? (
          <EmptyState icon={ArrowLeft} title="Profile not found">
            {state.message}
          </EmptyState>
        ) : (
          <p className={styles.lead}>Loading…</p>
        )}
      </Screen>
    );
  }
  const { view, resolved } = state;
  return (
    <Screen
      title={view.name}
      crumb={back}
      badge={<span className={styles.ver}>v{view.version}</span>}
    >
      <p className={styles.stubNote}>
        The profile builder is not built yet; this is a read-only list.
      </p>
      <ul className={styles.files} aria-label="Artifacts">
        {resolved.artifacts.map((a) => (
          <li key={a.id}>
            {a.name}
            <span className={styles.kind}>{kinds[a.kind].label}</span>
          </li>
        ))}
      </ul>
    </Screen>
  );
}
