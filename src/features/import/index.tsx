import { ArrowLeft, CircleAlert, Download, Info } from "lucide-react";
import { useRef, useState } from "react";
import { Link, useLocation, useNavigate } from "react-router";
import type { ImportReport } from "../../bindings/ImportReport";
import type { Preview } from "../../bindings/Preview";
import type { Source } from "../../bindings/Source";
import {
  homeClaudeDir,
  libraryImport,
  libraryPreview,
  pickFolder,
} from "../../lib/ipc";
import { Button, Screen } from "../../ui";
import { ChooseStep, Scanning } from "./ChooseStep";
import { Done } from "./Done";
import {
  describeFailure,
  type ErrorField,
  type ImportFailure,
  nothingFound,
} from "./errors";
import styles from "./Import.module.css";
import { ReviewStep } from "./ReviewStep";
import { type SourceForm, SourceStep } from "./SourceStep";
import { Stepper } from "./Stepper";

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
  const [form, setForm] = useState<SourceForm>({
    choice: "claude",
    url: "",
    path: "",
  });
  const [failure, setFailure] = useState<ImportFailure | null>(null);
  const [scanning, setScanning] = useState(false);
  const [preview, setPreview] = useState<Preview | null>(null);
  const [scannedKey, setScannedKey] = useState("");
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const [tags, setTags] = useState<Record<string, string[]>>({});
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
      setSelected(
        new Set(
          found.candidates.filter((c) => !c.alreadyInLibrary).map((c) => c.id),
        ),
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

  const chosen = (preview?.candidates ?? []).filter((c) => selected.has(c.id));

  async function runImport() {
    if (!preview) return;
    setImporting(true);
    setImportError(null);
    try {
      const picked: Record<string, string[]> = {};
      for (const c of chosen) picked[c.id] = tags[c.id] ?? [];
      setReport(
        await libraryImport(
          preview,
          chosen.map((c) => c.id),
          picked,
        ),
      );
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

  function toggle(id: string) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (!next.delete(id)) next.add(id);
      return next;
    });
  }

  function toggleGroup(ids: string[], on: boolean) {
    setSelected((prev) => {
      const next = new Set(prev);
      for (const id of ids) {
        if (on) next.add(id);
        else next.delete(id);
      }
      return next;
    });
  }

  const title = firstRun ? "Import your first artifacts" : "Import";
  const count = `${selected.size} artifact${selected.size === 1 ? "" : "s"}`;

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

  return (
    <Screen title={title} narrow crumb={crumb}>
      {firstRun && step === 0 && (
        <div className={styles.callout} data-tone="info">
          <Info size={16} aria-hidden="true" />
          <p>
            Importing copies artifacts into Harneo's library. Nothing in your
            projects or <span className={styles.mono}>~/.claude</span> changes.
          </p>
        </div>
      )}
      <Stepper at={step} />

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
        (scanning || !preview ? (
          <Scanning source={shown} />
        ) : (
          <ChooseStep
            preview={preview}
            sourceName={shown}
            selected={selected}
            onToggle={toggle}
            onToggleGroup={toggleGroup}
            onSelect={(ids) => setSelected(new Set(ids))}
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

      <div className={styles.foot}>
        {step > 0 ? (
          <Button onClick={back} disabled={importing}>
            Back
          </Button>
        ) : (
          firstRun && (
            <Button variant="ghost" onClick={() => void navigate("/library")}>
              Skip for now
            </Button>
          )
        )}
        <span className={styles.grow} />
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
            disabled={scanning || selected.size === 0}
            onClick={() => setStep(2)}
          >
            Continue
          </Button>
        )}
        {step === 2 && (
          <Button
            variant="primary"
            disabled={importing || chosen.length === 0}
            onClick={() => void runImport()}
          >
            <Download size={14} aria-hidden="true" />
            Import {count}
          </Button>
        )}
      </div>
    </Screen>
  );
}
