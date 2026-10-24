import { Highlight, type PrismTheme } from "prism-react-renderer";
import { useState, type ReactNode } from "react";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import styles from "./FileViewer.module.css";

/** Prism languages bundled with prism-react-renderer, by file extension. */
const languages: Record<string, string> = {
  js: "javascript",
  mjs: "javascript",
  cjs: "javascript",
  jsx: "jsx",
  ts: "typescript",
  tsx: "tsx",
  py: "python",
  json: "json",
  yaml: "yaml",
  yml: "yaml",
  css: "css",
  html: "markup",
  xml: "markup",
  svg: "markup",
  rs: "rust",
  go: "go",
  c: "c",
  h: "c",
  cpp: "cpp",
  sql: "sql",
  md: "markdown",
  markdown: "markdown",
};

function extensionOf(path: string): string {
  const name = path.split("/").pop() ?? path;
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
}

/** The Prism language for a file path; `text` (no highlighting) when unknown. */
export function languageOf(path: string): string {
  return languages[extensionOf(path)] ?? "text";
}

export function isMarkdown(path: string): boolean {
  const ext = extensionOf(path);
  return ext === "md" || ext === "markdown";
}

/** Token colours from the design tokens, so the viewer follows light and dark. */
const theme: PrismTheme = {
  plain: { color: "var(--t1)", backgroundColor: "transparent" },
  styles: [
    {
      types: ["comment", "prolog", "doctype", "cdata"],
      style: { color: "var(--t3)", fontStyle: "italic" },
    },
    { types: ["punctuation", "operator"], style: { color: "var(--t2)" } },
    {
      types: ["keyword", "tag", "selector", "atrule"],
      style: { color: "var(--blue)" },
    },
    {
      types: ["string", "char", "attr-value", "inserted"],
      style: { color: "var(--green)" },
    },
    {
      types: ["number", "boolean", "constant", "symbol", "deleted"],
      style: { color: "var(--orange)" },
    },
    {
      types: ["function", "class-name", "builtin", "attr-name"],
      style: { color: "var(--tint-violet)" },
    },
  ],
};

interface MdNode {
  type: string;
  value?: string;
  children?: MdNode[];
}

/** Turns raw HTML nodes into plain text, so it is shown as written and never rendered. */
function htmlAsText() {
  const walk = (node: MdNode) => {
    if (node.type === "html") node.type = "text";
    node.children?.forEach(walk);
  };
  return walk;
}

const markdownComponents = {
  // Remote images are never fetched: the alt text and the URL are shown instead.
  img: ({ src, alt }: { src?: string; alt?: string }) => (
    <span className={styles.img}>
      [image{alt ? `: ${alt}` : ""}
      {src ? ` (${src})` : ""}]
    </span>
  ),
  // Links are shown but not followed inside the app.
  a: ({ href, children }: { href?: string; children?: ReactNode }) => (
    <a
      href={href}
      title={href}
      rel="noreferrer"
      onClick={(event) => event.preventDefault()}
    >
      {children}
    </a>
  ),
};

function Code({ path, text }: { path: string; text: string }) {
  return (
    <Highlight
      theme={theme}
      code={text.replace(/\n$/, "")}
      language={languageOf(path)}
    >
      {({ tokens, getLineProps, getTokenProps }) => (
        <pre
          className={styles.pre}
          tabIndex={0}
          aria-label={`Contents of ${path}`}
        >
          {tokens.map((line, i) => (
            <div key={i} {...getLineProps({ line })} className={styles.ln}>
              <span className={styles.no} aria-hidden="true">
                {i + 1}
              </span>
              <span>
                {line.map((token, k) => (
                  <span key={k} {...getTokenProps({ token })} />
                ))}
                {"\n"}
              </span>
            </div>
          ))}
        </pre>
      )}
    </Highlight>
  );
}

/**
 * One file, read-only: Markdown rendered (with a Source toggle), other text highlighted with
 * line numbers. `text` is `null` for a file with no preview; the caller then explains why.
 */
export function FileViewer({
  path,
  text,
  empty,
}: {
  path: string;
  text: string | null;
  /** Shown instead of content when `text` is `null`. */
  empty?: ReactNode;
}) {
  const [source, setSource] = useState(false);
  if (text === null) return <div className={styles.empty}>{empty}</div>;
  const markdown = isMarkdown(path);
  return (
    <div className={styles.viewer}>
      {markdown && (
        <div className={styles.bar}>
          <button
            type="button"
            className={styles.toggle}
            aria-pressed={source}
            onClick={() => setSource((s) => !s)}
          >
            Source
          </button>
        </div>
      )}
      {markdown && !source ? (
        <div className={styles.md}>
          <Markdown
            remarkPlugins={[remarkGfm, htmlAsText]}
            components={markdownComponents}
          >
            {text}
          </Markdown>
        </div>
      ) : (
        <Code path={path} text={text} />
      )}
    </div>
  );
}
