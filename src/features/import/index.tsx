import { ArrowLeft, CircleAlert, Download, Info } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { Link, useLocation, useNavigate } from "react-router";
import type { Audit } from "../../bindings/Audit";
import type { ImportReport } from "../../bindings/ImportReport";
import type { PreviewTicket } from "../../bindings/PreviewTicket";
import type { TagColor } from "../../bindings/TagColor";
import type { Source } from "../../bindings/Source";
import {
  homeClaudeDir,
  libraryImport,
  linkReject,
  libraryPreview,
  pickFolder,
  tagCreate,
  tagsList,
} from "../../lib/ipc";
import { resolveNames } from "../../lib/names";
import { Button, Screen, useProposals, WizardActions } from "../../ui";
import { ChooseStep, Scanning } from "./ChooseStep";
import { Done } from "./Done";
import {
  describeFailure,
  type ErrorField,
  type ImportFailure,
  nothingFound,
} from "./errors";
import styles from "./Import.module.css";
import { criticalIds, criticalRule } from "./trust";
import { ReviewStep } from "./ReviewStep";
import { type SourceForm, SourceStep } from "./SourceStep";
import { Stepper } from "./Stepper";
import { useDetection } from "./useDetection";
import { usePreviewConfirmation } from "./usePreviewConfirmation";

/** What the user sees for the chosen source, in messages and the scan title. */
function sourceName(form: SourceForm): string {
  if (form.choice === "claude") return "~/.claude";
  return (form.choice === "git" ? form.url : form.path).trim();
}

/** A key that changes whenever the source does, so a kept preview is reused only if unchanged. */
function sourceKey(form: SourceForm): string {
  return `${form.choice}|${sourceName(form)}`;
}

/** The `Source` to scan; `~` is expanded here since the core reads paths as given. */
async function toSource(form: SourceForm): Promise<Source> {
  if (form.choice === "git") {
    return { kind: "git", url: form.url.trim(), branch: null, subfolder: null };
  }
  const claude = await homeClaudeDir();
  if (form.choice === "claude") return { kind: "local", path: claude };
  const path = form.path.trim();
  const home = claude.replace(/[\\/]\.claude$/, "");
  const expanded =
    path === "~" || /^~[\\/]/.test(path) ? home + path.slice(1) : path;
  return { kind: "local", path: expanded };
}

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** Import: pick a source, choose what to bring in, review tags and secrets, then copy. */
export function ImportScreen() {
  const navigate = useNavigate();
  const location = useLocation();
  const firstRun =
    (location.state as { firstRun?: boolean } | null)?.firstRun === true;

  const [step, setStep] = useState(0);
  // `?path=` presets the folder source (the project page's "Import from this project").
  const presetPath = new URLSearchParams(location.search).get("path");
  // `?select=<candidate id>` (with `path`) imports just that artifact: the Drift tab's
  // "Import to library". Used once, by the first scan.
  const preselect = useRef(new URLSearchParams(location.search).get("select"));
  const [form, setForm] = useState<SourceForm>({
    choice: presetPath ? "folder" : "claude",
    url: "",
    path: presetPath ?? "",
  });
  const [failure, setFailure] = useState<ImportFailure | null>(null);
  const [scanning, setScanning] = useState(false);
  const [preview, setPreview] = useState<PreviewTicket | null>(null);
  const [scannedKey, setScannedKey] = useState("");
  const [excluded, setExcluded] = useState<ReadonlySet<string>>(new Set());
  const [refreshing, setRefreshing] = useState(false);
  // Claude's verdicts so far: each candidate's audit once phase 2 has judged it.
  const [verdicts, setVerdicts] = useState<Record<string, Audit>>({});
  // Candidates excluded by the automatic rule alone. A hand exclusion or restore leaves this
  // set, so a verdict never overrides the user.
  const autoExcluded = useRef(new Set<string>());
  const [tags, setTags] = useState<Record<string, string[]>>({});
  // Colors picked in Tag all for names new to the library, created before the import.
  const [newColors, setNewColors] = useState<Record<string, TagColor>>({});
  const [importing, setImporting] = useState(false);
  const [importError, setImportError] = useState<string | null>(null);
  const [report, setReport] = useState<ImportReport | null>(null);
  // A scan answers only if it is still the latest one (Back during a scan cancels the wait).
  const scanId = useRef(0);

  const field: ErrorField = form.choice === "git" ? "url" : "path";
  const shown = sourceName(form);
  const canContinue =
    form.choice === "claude" ||
    (form.choice === "git" ? form.url.trim() : form.path.trim()) !== "";

  async function scan() {
    const key = sourceKey(form);
    if (preview && key === scannedKey) {
      setStep(1);
      return;
    }
    const id = ++scanId.current;
    setFailure(null);
    setStep(1);
    setScanning(true);
    try {
      const found = await libraryPreview(await toSource(form));
      if (id !== scanId.current) return;
      if (found.candidates.length === 0) {
        setFailure({ field, message: nothingFound(shown), neutral: true });
        setStep(0);
        return;
      }
      setPreview(found);
      setScannedKey(key);
      const only = preselect.current;
      preselect.current = null;
      setVerdicts({});
      const picked = only && found.candidates.some((c) => c.id === only);
      const auto = picked ? [] : criticalIds(found.candidates);
      autoExcluded.current = new Set(auto);
      setExcluded(
        picked
          ? new Set(
              found.candidates.filter((c) => c.id !== only).map((c) => c.id),
            )
          : new Set(auto),
      );
      setTags(
        Object.fromEntries(
          found.candidates.map((c) => [c.id, c.suggestedTags]),
        ),
      );
    } catch (error) {
      if (id !== scanId.current) return;
      setFailure(describeFailure(messageOf(error), field, shown));
      setStep(0);
    } finally {
      if (id === scanId.current) setScanning(false);
    }
  }

  async function browse() {
    const chosen = await pickFolder().catch(() => null);
    if (chosen) {
      setForm((f) => ({ ...f, path: chosen }));
      setFailure(null);
    }
  }

  // The preview as it stands: phase 1 audits, with Claude's verdicts applied as they arrive.
  const view = useMemo(
    () =>
      preview
        ? {
            ...preview,
            candidates: preview.candidates.map((c) =>
              verdicts[c.id] ? { ...c, audit: verdicts[c.id] as Audit } : c,
            ),
          }
        : null,
    [preview, verdicts],
  );
  const phase2 = usePreviewConfirmation(preview?.token ?? null, onVerdict);

  function onVerdict(id: string, audit: Audit) {
    setVerdicts((prev) => ({ ...prev, [id]: audit }));
    const critical = audit.findings.some((f) => f.severity === "critical");
    if (!critical && autoExcluded.current.delete(id)) {
      setExcluded((prev) => {
        const next = new Set(prev);
        next.delete(id);
        return next;
      });
    }
  }

  const heldBack = (view?.candidates ?? []).filter(
    (c) =>
      !c.alreadyInLibrary && excluded.has(c.id) && criticalRule(c) !== null,
  ).length;
  const chosen = (view?.candidates ?? []).filter(
    (c) => !c.alreadyInLibrary && !excluded.has(c.id),
  );

  const chosenIds = useMemo(() => chosen.map((c) => c.id), [chosen]);
  const detection = useDetection(step === 2, preview, chosenIds, (found) => {
    // Claude's tags join the keyword suggestions; a tag already there (any case) is kept once.
    const mine = new Set(chosenIds);
    setTags((prev) => {
      const next = { ...prev };
      for (const s of found.tags) {
        if (!mine.has(s.artifact)) continue;
        const own = next[s.artifact] ?? [];
        const fresh = s.tags.filter(
          (t) => !own.some((o) => o.toLowerCase() === t.toLowerCase()),
        );
        if (fresh.length > 0) next[s.artifact] = [...own, ...fresh];
      }
      return next;
    });
  });
  const proposedLinks = useMemo(
    () =>
      detection.phase.kind === "done" ? detection.phase.detection.links : [],
    [detection.phase],
  );
  const proposals = useProposals(proposedLinks);
  const [names, setNames] = useState<ReadonlyMap<string, string>>(new Map());
  useEffect(() => {
    if (proposedLinks.length === 0) return;
    let live = true;
    const known = new Map(
      (preview?.candidates ?? []).map((c) => [c.id, c.artifact.name]),
    );
    void resolveNames(
      proposedLinks.flatMap((l) => [l.from, l.to]),
      known,
    ).then((found) => {
      if (live) setNames(found);
    });
    return () => {
      live = false;
    };
  }, [proposedLinks, preview]);

  async function includeManaged() {
    if (!preview) return;
    setRefreshing(true);
    try {
      const found = await libraryPreview(preview.source, true);
      // Only candidates new to this preview start excluded; the user's choices on the rest stand.
      const seen = new Set(preview.candidates.map((c) => c.id));
      const fresh = criticalIds(
        found.candidates.filter((c) => !seen.has(c.id)),
      );
      for (const id of fresh) autoExcluded.current.add(id);
      setExcluded((prev) => new Set([...prev, ...fresh]));
      setPreview(found);
      setTags((prev) => ({
        ...Object.fromEntries(
          found.candidates.map((c) => [c.id, c.suggestedTags]),
        ),
        ...prev,
      }));
    } catch (error) {
      setFailure(describeFailure(messageOf(error), field, shown));
      setStep(0);
    } finally {
      setRefreshing(false);
    }
  }

  /** `library_import` creates missing tags with the default color, so a color chosen in Tag
   * all is applied by creating the tag first. A name that already exists is left alone. */
  async function createColoredTags(used: string[]) {
    const wanted = new Map<string, string>();
    for (const name of used) {
      if (newColors[name.toLowerCase()]) wanted.set(name.toLowerCase(), name);
    }
    if (wanted.size === 0) return;
    const existing = new Set(
      (await tagsList()).map((t) => t.name.toLowerCase()),
    );
    for (const [lower, name] of wanted) {
      const color = newColors[lower];
      if (color && !existing.has(lower)) await tagCreate(name, color);
    }
  }

  async function runImport() {
    if (!preview) return;
    setImporting(true);
    setImportError(null);
    try {
      const picked: Record<string, string[]> = {};
      for (const c of chosen) picked[c.id] = tags[c.id] ?? [];
      await createColoredTags(Object.values(picked).flat());
      const accepted: [string, string, string][] = proposals.accepted.map(
        (l) => [l.from, l.to, l.reason],
      );
      setReport(
        await libraryImport(
          preview,
          chosen.map((c) => c.id),
          picked,
          accepted,
        ),
      );
      // A refused group is remembered only where both artifacts are already in the library.
      const inPreview = new Set((preview.candidates ?? []).map((c) => c.id));
      for (const l of proposals.rejected) {
        if (inPreview.has(l.from) || inPreview.has(l.to)) continue;
        await linkReject(l.from, l.to).catch(() => undefined);
      }
    } catch (error) {
      setImportError(messageOf(error));
    } finally {
      setImporting(false);
    }
  }

  function back() {
    scanId.current += 1;
    setScanning(false);
    setImportError(null);
    setStep((s) => s - 1);
  }

  function exclude(ids: string[]) {
    for (const id of ids) autoExcluded.current.delete(id);
    setExcluded((prev) => new Set([...prev, ...ids]));
  }

  function restore(ids: string[]) {
    for (const id of ids) autoExcluded.current.delete(id);
    setExcluded((prev) => {
      const next = new Set(prev);
      for (const id of ids) next.delete(id);
      return next;
    });
  }

  const linkCount =
    detection.phase.kind === "done" ? proposals.accepted.length : 0;
  const title = firstRun ? "Import your first artifacts" : "Import";
  const count = `${chosen.length} artifact${chosen.length === 1 ? "" : "s"}`;

  const crumb = firstRun ? null : (
    <div className={styles.crumb}>
      <Link to="/library">
        <ArrowLeft size={14} aria-hidden="true" />
        Library
      </Link>
    </div>
  );

  if (report) {
    const names = new Map(
      (preview?.candidates ?? []).map((c) => [c.id, c.artifact.name]),
    );
    return (
      <Screen title={title} narrow>
        <Done report={report} names={names} />
      </Screen>
    );
  }

  const foot = (
    <WizardActions
      back={
        step > 0 ? (
          <Button onClick={back} disabled={importing}>
            Back
          </Button>
        ) : (
          firstRun && (
            <Button variant="ghost" onClick={() => void navigate("/library")}>
              Skip for now
            </Button>
          )
        )
      }
    >
      {step === 0 && (
        <Button
          variant="primary"
          disabled={!canContinue}
          onClick={() => void scan()}
        >
          Continue
        </Button>
      )}
      {step === 1 && (
        <Button
          variant="primary"
          disabled={scanning || chosen.length === 0 || phase2.blocked}
          onClick={() => setStep(2)}
        >
          Continue
        </Button>
      )}
      {step === 2 && detection.phase.kind === "running" && (
        <Button variant="ghost" onClick={detection.skip}>
          Skip detection
        </Button>
      )}
      {step === 2 && (
        <Button
          variant="primary"
          disabled={
            importing ||
            chosen.length === 0 ||
            phase2.blocked ||
            detection.phase.kind === "running"
          }
          onClick={() => void runImport()}
        >
          <Download size={14} aria-hidden="true" />
          Import {count}
          {linkCount > 0 &&
            ` and ${linkCount} link${linkCount === 1 ? "" : "s"}`}
        </Button>
      )}
    </WizardActions>
  );

  return (
    <Screen
      title={title}
      narrow
      crumb={crumb}
      toolbar={<Stepper at={step} />}
      foot={foot}
    >
      {firstRun && step === 0 && (
        <div className={styles.callout} data-tone="info">
          <Info size={16} aria-hidden="true" />
          <p>
            Importing copies artifacts into Harneo's library. Nothing in your
            projects or <span className={styles.mono}>~/.claude</span> changes.
          </p>
        </div>
      )}
      {step === 0 && (
        <SourceStep
          form={form}
          failure={failure}
          onChange={(next) => {
            setForm((f) => ({ ...f, ...next }));
            setFailure(null);
          }}
          onBrowse={() => void browse()}
        />
      )}
      {step === 1 &&
        (scanning || !view ? (
          <Scanning source={shown} />
        ) : (
          <ChooseStep
            preview={view}
            progress={phase2.progress}
            phase1Only={phase2.phase1Only}
            phase2={phase2}
            onCancelImport={() => void navigate("/library")}
            sourceName={shown}
            excluded={excluded}
            onExclude={exclude}
            onRestore={restore}
            onIncludeManaged={() => void includeManaged()}
            refreshing={refreshing}
          />
        ))}
      {step === 2 && (
        <>
          {importError && (
            <div className={styles.callout} data-tone="bad" role="alert">
              <CircleAlert size={16} aria-hidden="true" />
              <p>
                <b>Nothing was imported.</b> {importError}
              </p>
            </div>
          )}
          <ReviewStep
            chosen={chosen}
            tags={tags}
            colors={newColors}
            heldBack={heldBack}
            phase1Only={phase2.phase1Only}
            confirming={phase2.progress}
            phase2={phase2}
            onCancelImport={() => void navigate("/library")}
            detection={detection}
            proposals={proposals}
            names={names}
            onReviewHeldBack={() => setStep(1)}
            onAddAll={(name, color) => {
              if (color) {
                setNewColors((prev) => ({
                  ...prev,
                  [name.toLowerCase()]: color,
                }));
              }
              setTags((prev) => {
                const next = { ...prev };
                for (const c of chosen) {
                  const own = prev[c.id] ?? [];
                  const dup = own.some(
                    (t) => t.toLowerCase() === name.toLowerCase(),
                  );
                  if (!dup) next[c.id] = [...own, name];
                }
                return next;
              });
            }}
            onRemoveAll={(name) =>
              setTags((prev) => {
                const next = { ...prev };
                for (const c of chosen) {
                  next[c.id] = (prev[c.id] ?? []).filter(
                    (t) => t.toLowerCase() !== name.toLowerCase(),
                  );
                }
                return next;
              })
            }
            onRemoveTag={(id, tag) =>
              setTags((prev) => ({
                ...prev,
                [id]: (prev[id] ?? []).filter((t) => t !== tag),
              }))
            }
            onAddTag={(id, tag) =>
              setTags((prev) => {
                const own = prev[id] ?? [];
                const dup = own.some(
                  (t) => t.toLowerCase() === tag.toLowerCase(),
                );
                return dup ? prev : { ...prev, [id]: [...own, tag] };
              })
            }
          />
        </>
      )}
    </Screen>
  );
}
