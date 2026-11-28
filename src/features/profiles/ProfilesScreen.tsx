import { FileUp, Layers, Plus, RefreshCw, TriangleAlert } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Link, useNavigate } from "react-router";
import type { ProfileImportReport } from "../../bindings/ProfileImportReport";
import type { ProfileImportTicket } from "../../bindings/ProfileImportTicket";
import type { ProfileSummary } from "../../bindings/ProfileSummary";
import {
  profileExport,
  profileImportPreview,
  profilesList,
} from "../../lib/ipc";
import { Badge, Button, EmptyState, Kbd, Screen } from "../../ui";
import { labelOf, useTemplates } from "./categoryData";
import { CloneDialog } from "./CloneDialog";
import { Dialog } from "./Dialog";
import { dropDraft, NEW_PROFILE, newProfileDraft, type Draft } from "./drafts";
import { ExportDialog, type ExportChoice } from "./ExportDialog";
import { ImportDialog } from "./ImportDialog";
import { DeleteDialog, RenameDialog } from "./ProfileDialogs";
import { ProfileCard } from "./ProfileCard";
import styles from "./Profiles.module.css";

type Load =
  | { status: "loading" }
  | { status: "error"; message: string }
  | { status: "ready"; profiles: ProfileSummary[] };

const NOTICE_MS = 6000;

interface Notice {
  text: string;
  bad: boolean;
  /** Where an "Open Harnmap" link goes, for an import that brought a map. */
  harnmap?: bigint;
}

/** The sentence about the map an import brought, or nothing when it had none. */
function mapLine(report: ProfileImportReport): string {
  if (report.mapDraws === 0) return "";
  const draws = `${report.mapDraws} ${report.mapDraws === 1 ? "drawing" : "drawings"}`;
  const fits = `${report.fits} saved ${report.fits === 1 ? "fit" : "fits"}`;
  return ` Its Harnmap is ready, with ${draws} in history and ${fits}. No tokens spent.`;
}

/** `Light profile` becomes `light-profile.harneo-profile.json`. */
export function exportFileName(name: string): string {
  const kebab = name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return `${kebab || "profile"}.harneo-profile.json`;
}

export function ProfilesScreen() {
  const navigate = useNavigate();
  const templates = useTemplates();
  const [load, setLoad] = useState<Load>({ status: "loading" });
  const [notice, setNotice] = useState<Notice | null>(null);
  const [cloning, setCloning] = useState<ProfileSummary | null>(null);
  const [renaming, setRenaming] = useState<ProfileSummary | null>(null);
  const [deleting, setDeleting] = useState<ProfileSummary | null>(null);
  const [exporting, setExporting] = useState<ProfileSummary | null>(null);
  const [importing, setImporting] = useState<ProfileImportTicket | null>(null);

  const reload = useCallback(() => {
    profilesList()
      .then((profiles) => setLoad({ status: "ready", profiles }))
      .catch((err) => setLoad({ status: "error", message: String(err) }));
  }, []);
  useEffect(reload, [reload]);

  // A new profile the user left with edits: shown as a card, and New profile asks about it.
  const [draft, setDraft] = useState<Draft | undefined>(newProfileDraft);
  const [askDraft, setAskDraft] = useState(false);
  const openNew = useCallback(() => void navigate("/profiles/new"), [navigate]);
  const discardDraft = useCallback(() => {
    dropDraft(NEW_PROFILE);
    setDraft(undefined);
  }, []);
  const newProfile = useCallback(() => {
    if (newProfileDraft()) setAskDraft(true);
    else openNew();
  }, [openNew]);

  // N starts a new profile, unless the user is typing or a dialog is open.
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key.toLowerCase() !== "n") return;
      if (event.ctrlKey || event.metaKey || event.altKey) return;
      const target = event.target;
      if (
        target instanceof HTMLElement &&
        (target.closest("input, textarea, select, [contenteditable]") ||
          target.closest('[role="dialog"]'))
      ) {
        return;
      }
      if (cloning || importing || renaming || deleting || exporting) return;
      event.preventDefault();
      newProfile();
    }
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [cloning, importing, renaming, deleting, exporting, newProfile]);

  // A good result is a toast: it goes by itself. A failure stays until dismissed.
  useEffect(() => {
    if (!notice || notice.bad) return;
    const timer = setTimeout(() => setNotice(null), NOTICE_MS);
    return () => clearTimeout(timer);
  }, [notice]);

  function say(text: string, bad = false, harnmap?: bigint) {
    setNotice({ text, bad, harnmap });
  }

  async function exportProfile(profile: ProfileSummary, choice: ExportChoice) {
    setExporting(null);
    try {
      // The save dialog is the backend's: it picks the path and writes the file.
      const path = await profileExport(
        profile.id,
        exportFileName(profile.name),
        undefined,
        choice,
      );
      if (path === null) return;
      say(`Exported ${profile.name} v${profile.version} to ${path}.`);
    } catch (err) {
      say(`Could not export ${profile.name}: ${String(err)}`, true);
    }
  }

  async function importFile() {
    try {
      // The file dialog is the backend's; the file stays there under a token.
      const ticket = await profileImportPreview();
      if (ticket === null) return;
      setNotice(null);
      setImporting(ticket);
    } catch (err) {
      say(`Could not read the profile file: ${String(err)}`, true);
    }
  }

  const profiles = load.status === "ready" ? load.profiles : [];
  const actions = (
    <>
      <span style={{ flex: 1 }} />
      <Button onClick={() => void importFile()}>
        <FileUp size={14} aria-hidden="true" />
        Import profile file
      </Button>
      <Button variant="primary" onClick={newProfile}>
        <Plus size={14} aria-hidden="true" />
        New profile <Kbd>N</Kbd>
      </Button>
    </>
  );

  return (
    <Screen title="Profiles" actions={actions}>
      <p className={styles.lead}>
        A profile is a named selection of library artifacts that gets installed
        into projects.
      </p>
      {notice && (
        <div
          role="status"
          className={`${styles.notice} ${notice.bad ? styles.bad : ""}`}
        >
          {notice.bad && <TriangleAlert size={14} aria-hidden="true" />}
          <span className={styles.noticeText}>{notice.text}</span>
          {notice.harnmap !== undefined && (
            <Link to={`/profiles/${notice.harnmap}/harnmap`}>Open Harnmap</Link>
          )}
          <Button variant="ghost" small onClick={() => setNotice(null)}>
            Dismiss
          </Button>
        </div>
      )}
      {load.status === "loading" && (
        <ul
          className={styles.grid}
          aria-busy="true"
          aria-label="Loading profiles"
        >
          {[0, 1, 2].map((n) => (
            <li key={n} className={styles.skel} />
          ))}
        </ul>
      )}
      {load.status === "error" && (
        <EmptyState
          icon={TriangleAlert}
          title="Profiles did not load"
          action={
            <Button
              onClick={() => {
                setLoad({ status: "loading" });
                reload();
              }}
            >
              <RefreshCw size={14} aria-hidden="true" />
              Retry
            </Button>
          }
        >
          {load.message}
        </EmptyState>
      )}
      {draft && (
        <ul className={styles.grid} aria-label="Unsaved draft">
          <DraftCard
            draft={draft}
            onContinue={openNew}
            onDiscard={discardDraft}
          />
        </ul>
      )}
      {load.status === "ready" && profiles.length === 0 && (
        <EmptyState
          icon={Layers}
          title="No profiles yet"
          action={
            <Button variant="primary" onClick={newProfile}>
              <Plus size={14} aria-hidden="true" />
              New profile
            </Button>
          }
        >
          Pick artifacts from your library into a profile, then scaffold or
          refit projects with it.
        </EmptyState>
      )}
      {profiles.length > 0 && (
        <ul className={styles.grid} aria-label="Profiles">
          {profiles.map((p) => (
            <ProfileCard
              key={String(p.id)}
              profile={p}
              categoryLabel={labelOf(templates, p.category)}
              onClone={setCloning}
              onExport={setExporting}
              onRename={setRenaming}
              onDelete={setDeleting}
            />
          ))}
        </ul>
      )}
      {cloning && (
        <CloneDialog profile={cloning} onClose={() => setCloning(null)} />
      )}
      {exporting && (
        <ExportDialog
          profile={exporting}
          fileName={exportFileName(exporting.name)}
          onClose={() => setExporting(null)}
          onExport={(choice) => exportProfile(exporting, choice)}
        />
      )}
      {renaming && (
        <RenameDialog
          profile={renaming}
          onClose={() => setRenaming(null)}
          onDone={(name) => {
            setRenaming(null);
            say(`Renamed ${renaming.name} to ${name}.`);
            reload();
          }}
        />
      )}
      {deleting && (
        <DeleteDialog
          profile={deleting}
          onClose={() => setDeleting(null)}
          onDone={() => {
            setDeleting(null);
            say(`Deleted ${deleting.name}.`);
            reload();
          }}
        />
      )}
      {importing && (
        <ImportDialog
          ticket={importing}
          onClose={() => setImporting(null)}
          onDone={(report) => {
            setImporting(null);
            say(
              `Imported ${report.name}. ${report.imported.length} new ${report.imported.length === 1 ? "artifact" : "artifacts"} added to your library.${mapLine(report)}`,
              false,
              report.mapDraws > 0 ? report.profileId : undefined,
            );
            reload();
          }}
        />
      )}
      {askDraft && draft && (
        <Dialog
          title="You have an unsaved new profile"
          sub={`${draftLabel(draft)}. Continue it, or discard it and start a new one.`}
          submit="Continue draft"
          busy={false}
          onSubmit={() => {
            setAskDraft(false);
            openNew();
          }}
          onClose={() => setAskDraft(false)}
        >
          <Button
            variant="ghost"
            onClick={() => {
              discardDraft();
              setAskDraft(false);
              openNew();
            }}
          >
            Discard it and start a new one
          </Button>
        </Dialog>
      )}
    </Screen>
  );
}

/** "Software Factory · 60 artifacts", or "Untitled" for a draft not named yet. */
function draftLabel(draft: Draft): string {
  const n = draft.spec.items.length;
  const name = draft.name.trim() || "Untitled";
  return `${name} · ${n} ${n === 1 ? "artifact" : "artifacts"}`;
}

/** The new profile the user left with edits, until they continue or discard it. */
function DraftCard({
  draft,
  onContinue,
  onDiscard,
}: {
  draft: Draft;
  onContinue: () => void;
  onDiscard: () => void;
}) {
  return (
    <li className={styles.card}>
      <div className={styles.topText}>
        <h2 className={styles.title}>
          <span>{draft.name.trim() || "Untitled"}</span>{" "}
          <Badge tone="warn">Unsaved draft</Badge>
        </h2>
        <p className={styles.category}>{draftLabel(draft).split(" · ")[1]}</p>
      </div>
      <div className={styles.foot}>
        <Button small variant="primary" onClick={onContinue}>
          Continue
        </Button>
        <Button small variant="ghost" onClick={onDiscard}>
          Discard
        </Button>
      </div>
    </li>
  );
}
