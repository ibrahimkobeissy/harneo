import { useEffect, useRef, type ReactNode } from "react";

interface ModalProps {
  label: string;
  children: ReactNode;
}

// Accessible modal: labelled, focus moves in on open and returns on close.
// Closing is the owner's job (Esc is handled by the Layout key handler).
export function Modal({ label, children }: ModalProps) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const previous = document.activeElement;
    const node = ref.current;
    if (node && !node.contains(document.activeElement)) node.focus();
    return () => {
      if (previous instanceof HTMLElement) previous.focus();
    };
  }, []);

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={label}
      tabIndex={-1}
      ref={ref}
    >
      {children}
    </div>
  );
}
