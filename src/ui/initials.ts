/** The first letters of up to two words: "Review kit" gives "RK", "Rust" gives "R". */
export function initialsOf(name: string): string {
  const words = name.split(/[\s\-_]+/).filter((w) => w !== "");
  const letters = words.slice(0, 2).map((w) => Array.from(w)[0] ?? "");
  return letters.join("").toUpperCase() || "?";
}
