import type { ComponentProps } from "react";
import styles from "./Button.module.css";

export type ButtonVariant =
  "primary" | "secondary" | "ghost" | "danger" | "danger-solid";

interface ButtonProps extends ComponentProps<"button"> {
  variant?: ButtonVariant;
  small?: boolean;
}

export function Button({
  variant = "secondary",
  small = false,
  type = "button",
  className,
  ...rest
}: ButtonProps) {
  const classes = [
    styles.btn,
    styles[variant],
    small ? styles.sm : "",
    className,
  ]
    .filter(Boolean)
    .join(" ");
  return <button type={type} className={classes} {...rest} />;
}
