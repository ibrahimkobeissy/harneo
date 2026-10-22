/** Which input an error belongs under on the Source step. */
export type ErrorField = "url" | "path";

export interface ImportFailure {
  field: ErrorField;
  message: string;
}

/** First capture group of `pattern` in `raw`, or `fallback`. */
function after(raw: string, pattern: RegExp, fallback: string): string {
  return pattern.exec(raw)?.[1]?.trim() || fallback;
}

/** The "nothing found" message, also used when a local scan comes back empty. */
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
    return say(
      `Authentication required for ${shown}. Harneo never asks for a password: use a public URL, or load an SSH key in your agent.`,
    );
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
    return say(nothingFound(shown));
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
