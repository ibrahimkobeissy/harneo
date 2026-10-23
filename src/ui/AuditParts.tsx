import type { AuditSeverity } from "../bindings/AuditSeverity";
import { anyNote, ruleInfo, severityLabels } from "./audit";

/** Severity as a small badge: sentence case, the same colours on every screen. */
export function SeverityBadge({ severity }: { severity: AuditSeverity }) {
  return (
    <span className="audit-severity" data-sev={severity}>
      {severityLabels[severity]}
    </span>
  );
}

/** A finding's plain title with the rule id beside it in mono; the explanation is the tooltip. */
export function FindingTitle({ rule }: { rule: string }) {
  const { title, explanation } = ruleInfo(rule);
  return (
    <span
      className="audit-finding-title"
      title={explanation === "" ? undefined : explanation}
    >
      <b>{title}</b>
      {title !== rule && <code>{rule}</code>}
    </span>
  );
}

/** The short note that `*` means any, shown where reach lists appear. */
export function AnyNote({ className }: { className?: string }) {
  return <span className={className}>{anyNote}</span>;
}
