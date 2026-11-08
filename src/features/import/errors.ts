/** Which input an error belongs under on the Source step. */
/** `source` is a stored source that could not be replayed: shown above the choices. */
export type ErrorField = "url" | "path" | "source";

export interface ImportFailure {
  field: ErrorField;
  message: string;
  /** Not a failure (nothing found): shown as a neutral note, not an error. */
  neutral?: boolean;
}

/** First capture group of `pattern` in `raw`, or `fallback`. */
function after(raw: string, pattern: RegExp, fallback: string): string {
  return pattern.exec(raw)?.[1]?.trim() || fallback;
}

/** The "nothing found" message, also used when a local scan comes back empty. */
/** `git@host:owner/repo.git` for an HTTPS repo URL (a `/tree/<branch>/…` tail dropped), or null. */
export function sshForm(url: string): string | null {
  const m =
    /^https?:\/\/([^/@\s]+)\/([^/\s]+)\/([^/\s]+?)(?:\.git)?(?:\/.*)?$/.exec(
      url.trim(),
    );
  return m ? `git@${m[1]}:${m[2]}/${m[3]}.git` : null;
}

/** The host of an SSH URL (`git@host:…` or `ssh://user@host/…`), or null. */
function sshHost(url: string): string | null {
  const m =
    /^ssh:\/\/(?:[^@/]+@)?([^/:]+)/.exec(url.trim()) ??
    /^[^@\s/]+@([^:/\s]+):/.exec(url.trim());
  return m?.[1] ?? null;
}

/**
 * What to do when a repo needs a login (or doesn't exist): the advice depends on the URL, since
 * an SSH agent is only used for SSH URLs and Harneo never prompts for a password (#264).
 */
export function authAdvice(shown: string): string {
  const host = sshHost(shown);
  if (host) {
    return `SSH could not log in to ${host} for ${shown}, or the repository does not exist. Check that your key is loaded (ssh-add -l), that “ssh -T git@${host}” works, and that Harneo was started from a session that sees your SSH agent.`;
  }
  const ssh = sshForm(shown);
  const use = ssh
    ? `its SSH form, ${ssh},`
    : "its SSH form (git@host:owner/repo.git)";
  return `Authentication required for ${shown}, or the repository does not exist. This is an HTTPS URL and Harneo never asks for a password. For a private repo, use ${use} with a key loaded in your SSH agent, or set up a git credential helper (for example “gh auth setup-git”).`;
}

export function nothingFound(shown: string): string {
  return `No artifacts found in ${shown}. Harneo looks for .claude/, CLAUDE.md, .mcp.json and loose SKILL.md files.`;
}

/**
 * Turns a core error message into the prototype's wording, under the field the user filled.
 * Core errors cross IPC as plain strings, so they are recognised by their stable text.
 */
export function describeFailure(
  raw: string,
  field: ErrorField,
  shown: string,
): ImportFailure {
  const say = (message: string): ImportFailure => ({ field, message });
  if (raw.includes("git is not installed")) {
    return say(
      "Git is not installed, or is not on your PATH. Install git to import from a URL.",
    );
  }
  if (raw.includes("needs credentials or does not exist")) {
    return say(authAdvice(shown));
  }
  if (raw.includes("cannot reach the git repository")) {
    return say(
      `Repository not found or unreachable: ${shown}. Check the URL and your connection.`,
    );
  }
  if (raw.includes("has no branch or tag named")) {
    const branch = after(raw, /named (.+)$/, "that name");
    return say(`Unknown branch or tag “${branch}” in ${shown}.`);
  }
  if (raw.includes("the repository has no folder")) {
    const folder = after(raw, /no folder (.+)$/, "that folder");
    return say(`The repository has no folder “${folder}” at that branch.`);
  }
  if (raw.includes("git did not finish within")) {
    return say(
      "Git took too long and was stopped. Check your connection and try again.",
    );
  }
  if (raw.includes("no artifacts found")) {
    return { ...say(nothingFound(shown)), neutral: true };
  }
  if (raw.includes("not a folder")) {
    return say(`Folder not found: ${shown}. Check the path.`);
  }
  if (raw.includes("not a git URL")) {
    return say(
      "That doesn't look like a repository URL. Use github.com/owner/repo.",
    );
  }
  return say(raw);
}
