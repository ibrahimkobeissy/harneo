import { Boxes, X } from "lucide-react";
import { useMemo, useState } from "react";
import type { ProposedLink } from "../bindings/ProposedLink";
import styles from "./Proposals.module.css";
import { Tag } from "./Tag";

/** One lead artifact and what Claude says it needs. */
export interface ProposalGroup {
  lead: string;
  links: ProposedLink[];
}

const pairKey = (l: ProposedLink) => `${l.from}\u0000${l.to}`;

/** Groups proposed links by their `from` side, in the order the leads first appear. */
export function groupByLead(links: readonly ProposedLink[]): ProposalGroup[] {
  const groups = new Map<string, ProposedLink[]>();
  for (const l of links) {
    const own = groups.get(l.from);
    if (own) own.push(l);
    else groups.set(l.from, [l]);
  }
  return [...groups].map(([lead, own]) => ({ lead, links: own }));
}

/** What the user did with a set of proposals: groups switched off, members dropped. */
export interface ProposalState {
  /** Groups that still have members. */
  groups: ProposalGroup[];
  isOn: (lead: string) => boolean;
  toggle: (lead: string) => void;
  drop: (link: ProposedLink) => void;
  /** Links of the groups that are on. */
  accepted: ProposedLink[];
  /** Links of the groups switched off (not the dropped members). */
  rejected: ProposedLink[];
}

/** Proposal choices for `links`; every group starts accepted. Remount when `links` changes. */
export function useProposals(links: readonly ProposedLink[]): ProposalState {
  const [off, setOff] = useState<ReadonlySet<string>>(new Set());
  const [dropped, setDropped] = useState<ReadonlySet<string>>(new Set());
  return useMemo(() => {
    const groups = groupByLead(links)
      .map((g) => ({
        lead: g.lead,
        links: g.links.filter((l) => !dropped.has(pairKey(l))),
      }))
      .filter((g) => g.links.length > 0);
    const isOn = (lead: string) => !off.has(lead);
    return {
      groups,
      isOn,
      toggle: (lead) =>
        setOff((prev) => {
          const next = new Set(prev);
          if (!next.delete(lead)) next.add(lead);
          return next;
        }),
      drop: (link) => setDropped((prev) => new Set(prev).add(pairKey(link))),
      accepted: groups.filter((g) => isOn(g.lead)).flatMap((g) => g.links),
      rejected: groups.filter((g) => !isOn(g.lead)).flatMap((g) => g.links),
    };
  }, [links, off, dropped]);
}

/**
 * Claude's proposed bundles: one group per lead with "+ N", the reason, a chip per member with
 * a remove button, and an accept switch per group. Shared by the import Review step and the
 * Find bundles dialog.
 */
export function Proposals({
  state,
  names,
  empty = "No bundles found: these artifacts don't depend on each other.",
}: {
  state: ProposalState;
  /** Display names by artifact id; an unknown id shows as itself. */
  names: ReadonlyMap<string, string>;
  empty?: string;
}) {
  const nameOf = (id: string) => names.get(id) ?? id;
  if (state.groups.length === 0) return <p className={styles.empty}>{empty}</p>;
  return (
    <>
      {state.groups.map((g) => {
        const on = state.isOn(g.lead);
        const lead = nameOf(g.lead);
        const reasons = [...new Set(g.links.map((l) => l.reason))].filter(
          (r) => r !== "",
        );
        return (
          <div key={g.lead} className={styles.group} data-on={on}>
            <header className={styles.head}>
              <Boxes size={15} aria-hidden="true" />
              <span className={styles.lead} title={lead}>
                {lead}
              </span>
              <span className={styles.n}>+ {g.links.length}</span>
              <span className={styles.grow} />
              <button
                type="button"
                role="switch"
                aria-checked={on}
                aria-label={`Accept bundle of ${lead}`}
                className={styles.switch}
                onClick={() => state.toggle(g.lead)}
              >
                <i />
              </button>
            </header>
            {reasons.length === 1 && (
              <div className={styles.row}>
                <span className={styles.why}>{reasons[0]}</span>
              </div>
            )}
            <div className={`${styles.row} ${styles.chips}`}>
              {g.links.map((l) => (
                <Tag key={l.to} tint="slate">
                  <span title={reasons.length > 1 ? l.reason : undefined}>
                    {nameOf(l.to)}
                  </span>
                  <button
                    type="button"
                    className={styles.chipX}
                    aria-label={`Remove ${nameOf(l.to)} from the bundle of ${lead}`}
                    onClick={() => state.drop(l)}
                  >
                    <X size={12} aria-hidden="true" />
                  </button>
                </Tag>
              ))}
            </div>
            {reasons.length > 1 && (
              <ul className={`${styles.row} ${styles.reasons}`}>
                {g.links.map((l) => (
                  <li key={l.to}>
                    <b>{nameOf(l.to)}</b>
                    {l.reason !== "" && ` — ${l.reason}`}
                  </li>
                ))}
              </ul>
            )}
          </div>
        );
      })}
    </>
  );
}
