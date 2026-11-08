import { ChevronDown, Info, Layers, ScanSearch } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useLocation, useNavigate } from "react-router";
import type { ArtifactFit } from "../../bindings/ArtifactFit";
import type { ArtifactKind } from "../../bindings/ArtifactKind";
import type { ProfileSummary } from "../../bindings/ProfileSummary";
import type { Template } from "../../bindings/Template";
import { harnfitForArtifact, profilesList, templatesList } from "../../lib/ipc";
import { Badge, Button, harnfitUrl, verdictLabel, verdictTone } from "../../ui";
import { isPlugin } from "../../ui/plugin";
import styles from "./Collections.module.css";
import { formatDate } from "./ItemMeta";

/**
 * The detail panel's "Harnfit in…" menu: pick the profile to fit this artifact into. Each row
 * shows the profile's category and its saved verdict, or "not fitted yet". A plugin taken whole
 * can't be fitted yet: the menu says so and its rows show as disabled. Picking opens the Harnfit screen (the click is the go-ahead); a saved fit
 * opens there at once, without a Claude call.
 */
export function HarnfitMenu({
  artifactId,
  artifactName,
  artifactKind,
}: {
  artifactId: string;
  artifactName: string;
  artifactKind: ArtifactKind;
}) {
  const navigate = useNavigate();
  const here = useLocation();
  const [open, setOpen] = useState(false);
  const [profiles, setProfiles] = useState<ProfileSummary[]>([]);
  const [fits, setFits] = useState<ArtifactFit[]>([]);
  const [templates, setTemplates] = useState<Template[]>([]);
  const wrap = useRef<HTMLDivElement>(null);
  const plugin = isPlugin(artifactKind);

  useEffect(() => {
    if (!open) return;
    let live = true;
    profilesList().then(
      (list) => live && setProfiles(Array.isArray(list) ? list : []),
      () => undefined,
    );
    harnfitForArtifact(artifactId).then(
      (list) => live && setFits(Array.isArray(list) ? list : []),
      () => undefined,
    );
    templatesList().then(
      (list) => live && setTemplates(Array.isArray(list) ? list : []),
      () => undefined,
    );
    function onDown(event: MouseEvent) {
      if (wrap.current && !wrap.current.contains(event.target as Node))
        setOpen(false);
    }
    function onKeyDown(event: KeyboardEvent) {
      if (event.key !== "Escape") return;
      event.preventDefault();
      event.stopPropagation();
      setOpen(false);
      wrap.current?.querySelector<HTMLElement>("button")?.focus();
    }
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKeyDown, true);
    return () => {
      live = false;
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKeyDown, true);
    };
  }, [open, artifactId]);

  function pick(profile: ProfileSummary) {
    setOpen(false);
    navigate(harnfitUrl(profile.id, `lib:${artifactId}`), {
      state: {
        from: `${here.pathname}${here.search}`,
        name: artifactName,
      },
    });
  }

  return (
    <div className={styles.anchor} ref={wrap}>
      <Button
        small
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
      >
        <ScanSearch size={14} aria-hidden="true" />
        Harnfit in…
        <ChevronDown size={14} aria-hidden="true" />
      </Button>
      {open && (
        <div className={styles.menu} role="menu" aria-label="Harnfit in">
          {plugin ? (
            <div className={styles.menuNote} role="note">
              <Info size={14} aria-hidden="true" />
              <p>
                <strong>A plugin kept whole can’t be fitted yet.</strong> Once a
                plugin is unpacked, each of its parts can be fitted on its own.
              </p>
            </div>
          ) : (
            <p className={styles.menuHead}>Fit it into a profile’s map</p>
          )}
          <div className={styles.menuScroll}>
            {profiles.map((p) => {
              const fit = fits.find((f) => f.profileId === p.id);
              const category =
                templates.find((t) => t.id === p.category)?.label ?? p.category;
              return (
                <button
                  key={String(p.id)}
                  type="button"
                  role="menuitem"
                  className={styles.menuItem}
                  disabled={plugin}
                  onClick={() => pick(p)}
                >
                  <Layers size={14} aria-hidden="true" />
                  <span className={styles.menuName}>
                    {p.name}
                    <span className={styles.menuTag}> · {category}</span>
                  </span>
                  {plugin ? (
                    <span className={styles.menuTag}>not available</span>
                  ) : fit ? (
                    <>
                      <span className={styles.menuTag}>
                        fit saved {formatDate(fit.createdAt)}
                        {fit.stale && " · out of date"}
                      </span>
                      <Badge tone={verdictTone(fit.verdict)}>
                        {verdictLabel(fit.verdict)}
                      </Badge>
                    </>
                  ) : (
                    <span className={styles.menuTag}>not fitted yet</span>
                  )}
                </button>
              );
            })}
          </div>
          {profiles.length === 0 && (
            <p className={styles.menuEmpty}>No profiles yet.</p>
          )}
        </div>
      )}
    </div>
  );
}
