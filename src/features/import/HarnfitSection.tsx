import { useEffect, useState } from "react";
import { useLocation, useNavigate } from "react-router";
import type { Candidate } from "../../bindings/Candidate";
import { harnmapGet, profilesList } from "../../lib/ipc";
import { Button, harnfitUrl, PREVIEW_PREFIX } from "../../ui";
import styles from "./Import.module.css";

interface MappedProfile {
  id: bigint;
  name: string;
}

/**
 * Harnfit in the import review: pick a profile with a drawn map, then fit any candidate into
 * it. Nothing runs until the button is clicked. The fit is saved with the profile; adding the
 * candidate to the profile needs it imported first.
 */
export function HarnfitSection({
  chosen,
  token,
}: {
  chosen: Candidate[];
  /** The preview's cache token; without it the candidates can't be fitted. */
  token: string | null;
}) {
  const navigate = useNavigate();
  const here = useLocation();
  const [profiles, setProfiles] = useState<MappedProfile[] | null>(null);
  const [picked, setPicked] = useState("");

  useEffect(() => {
    let live = true;
    (async () => {
      const all = await profilesList();
      const mapped: MappedProfile[] = [];
      for (const p of Array.isArray(all) ? all : []) {
        const view = await harnmapGet(p.id).catch(() => null);
        if (view?.drawn) mapped.push({ id: p.id, name: p.name });
      }
      if (live) setProfiles(mapped);
    })().catch(() => live && setProfiles([]));
    return () => {
      live = false;
    };
  }, []);

  const profile =
    profiles?.find((p) => String(p.id) === picked) ?? profiles?.[0] ?? null;

  function fit(c: Candidate) {
    if (!profile || !token) return;
    navigate(harnfitUrl(profile.id, `${PREVIEW_PREFIX}${c.id}`), {
      state: {
        candidate: { type: "preview", token, candidateId: c.id },
        name: c.artifact.name,
        from: `${here.pathname}${here.search}`,
      },
    });
  }

  return (
    <>
      <h2 className={styles.h}>
        Harnfit <span>does it fit a profile’s map? Optional</span>
      </h2>
      {profiles === null ? (
        <p className={styles.muted}>Looking for profiles with a map…</p>
      ) : profiles.length === 0 ? (
        <p className={styles.muted}>
          No profile has a drawn map yet. Draw one in a profile’s Harnmap to use
          Harnfit.
        </p>
      ) : (
        <>
          <p className={styles.muted}>
            Harnfit runs only when you click it. Fits are saved with the
            profile.
          </p>
          <div className={styles.bulkTags}>
            <label className={styles.bulkLabel} htmlFor="harnfit-profile">
              Fit into
            </label>
            <select
              id="harnfit-profile"
              value={profile ? String(profile.id) : ""}
              onChange={(ev) => setPicked(ev.target.value)}
            >
              {profiles.map((p) => (
                <option key={String(p.id)} value={String(p.id)}>
                  {p.name}
                </option>
              ))}
            </select>
          </div>
          <div className={styles.group}>
            {chosen.map((c) => {
              const plugin = c.artifact.kind === "plugin";
              return (
                <div key={c.id} className={styles.row} role="group">
                  <span className={styles.artifactName} title={c.artifact.name}>
                    {c.artifact.name}
                  </span>
                  <Button
                    small
                    disabled={plugin || !token}
                    aria-label={`Harnfit ${c.artifact.name}`}
                    onClick={() => fit(c)}
                  >
                    Harnfit
                  </Button>
                  {plugin && (
                    <span className={styles.muted}>
                      Harnfit for plugins comes later
                    </span>
                  )}
                </div>
              );
            })}
          </div>
        </>
      )}
    </>
  );
}
