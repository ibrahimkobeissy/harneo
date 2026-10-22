import { ChevronLeft, ChevronRight } from "lucide-react";
import { Button } from "../../ui";
import styles from "./Library.module.css";

/** Page numbers to show: first, last and the neighbours of the current page, gaps as null. */
export function pageNumbers(current: number, pages: number): (number | null)[] {
  const out: (number | null)[] = [];
  for (let i = 1; i <= pages; i++) {
    if (i === 1 || i === pages || Math.abs(i - current) <= 1) out.push(i);
    else if (out[out.length - 1] !== null) out.push(null);
  }
  return out;
}

export function Pager({
  page,
  perPage,
  total,
  onPage,
}: {
  page: number;
  perPage: number;
  total: number;
  onPage: (page: number) => void;
}) {
  const pages = Math.ceil(total / perPage);
  if (pages < 2) return null;
  const start = (page - 1) * perPage + 1;
  const end = Math.min(page * perPage, total);
  return (
    <nav className={styles.pager} aria-label="Pages">
      <span className={styles.sec}>
        {start}–{end} of {total}
      </span>
      <div className={styles.pages}>
        <Button
          small
          disabled={page === 1}
          aria-label="Previous page"
          onClick={() => onPage(page - 1)}
        >
          <ChevronLeft size={14} aria-hidden="true" />
        </Button>
        {pageNumbers(page, pages).map((n, i) =>
          n === null ? (
            <span key={`gap${i}`} className={styles.muted}>
              …
            </span>
          ) : (
            <button
              key={n}
              type="button"
              className={n === page ? `${styles.pg} ${styles.pgOn}` : styles.pg}
              aria-current={n === page ? "page" : undefined}
              onClick={() => onPage(n)}
            >
              {n}
            </button>
          ),
        )}
        <Button
          small
          disabled={page === pages}
          aria-label="Next page"
          onClick={() => onPage(page + 1)}
        >
          <ChevronRight size={14} aria-hidden="true" />
        </Button>
      </div>
    </nav>
  );
}
