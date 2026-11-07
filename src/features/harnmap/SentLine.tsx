import styles from "./Harnmap.module.css";

/** The what-is-sent line next to every Draw: pressing the button is the go-ahead. */
export function SentLine({ count }: { count: number }) {
  return (
    <p className={styles.sent}>
      <b>What is sent:</b> each artifact’s name, type, description and files (
      {count} in all), the stages, your description. <b>Never:</b> project
      files, secrets, other profiles. Pressing Draw is your go-ahead.
    </p>
  );
}
