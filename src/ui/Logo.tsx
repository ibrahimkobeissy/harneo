// The Harneo mark: two rails and a bead on a blue tile.
export function Logo({ size = 24 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden="true">
      <rect width="24" height="24" rx="6" fill="var(--blue)" />
      <path
        d="M7 6v12M17 6v12"
        stroke="var(--on-blue)"
        strokeWidth="2.2"
        strokeLinecap="round"
        fill="none"
      />
      <path
        d="M7 12h4.2"
        stroke="var(--on-blue)"
        strokeWidth="2.2"
        strokeLinecap="round"
        fill="none"
      />
      <circle cx="14" cy="12" r="2.1" fill="var(--on-blue)" />
    </svg>
  );
}
