import {
  ArrowRight,
  Download,
  FolderGit2,
  Layers,
  LayoutGrid,
} from "lucide-react";
import { Fragment } from "react";
import { useNavigate } from "react-router";
import { Button, Logo } from "../../ui";
import styles from "./Welcome.module.css";

const steps = [
  {
    icon: LayoutGrid,
    tint: "blue",
    title: "Library",
    text: "Everything you import, with its source and tags. Kept outside your projects.",
  },
  {
    icon: Layers,
    tint: "violet",
    title: "Profile",
    text: "A named selection, like “Light” or “Heavy”. Picking an artifact brings its whole bundle: a skill or command plus the subagents and scripts it needs.",
  },
  {
    icon: FolderGit2,
    tint: "slate",
    title: "Project",
    text: "Scaffold a new one or refit an existing one. You see every file first; replaced files are backed up.",
  },
];

/** The first-run welcome, shown while the library holds nothing. */
export function Welcome() {
  const navigate = useNavigate();
  return (
    <section className={styles.welcome} aria-labelledby="welcome-title">
      <div className={styles.card}>
        <div className={styles.brand}>
          <Logo size={32} />
          <b>Harneo</b>
        </div>
        <h1 id="welcome-title" className={styles.title}>
          Your Claude Code harness, fitted to every project.
        </h1>
        <p className={styles.lead}>
          Harneo keeps the skills, subagents, commands, settings and MCP servers
          you trust in one library, and installs the right set into any project,
          with a preview and a backup.
        </p>
        <div className={styles.flow}>
          {steps.map(({ icon: Icon, tint, title, text }, i) => (
            <Fragment key={title}>
              {i > 0 && (
                <ArrowRight
                  className={styles.arrow}
                  size={16}
                  aria-hidden="true"
                />
              )}
              <div className={styles.node}>
                <b>
                  <span className={styles.ic} data-tint={tint}>
                    <Icon size={13} aria-hidden="true" />
                  </span>
                  {title}
                </b>
                <p>{text}</p>
              </div>
            </Fragment>
          ))}
        </div>
        <div className={styles.actions}>
          <Button
            variant="primary"
            onClick={() =>
              void navigate("/import", { state: { firstRun: true } })
            }
          >
            <Download size={14} aria-hidden="true" />
            Import your first artifacts
          </Button>
          <span className={styles.hint}>
            Start from <code>~/.claude</code>, a git URL, or a folder.
          </span>
        </div>
      </div>
    </section>
  );
}
