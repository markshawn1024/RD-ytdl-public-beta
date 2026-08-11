import type { ButtonHTMLAttributes, PropsWithChildren } from "react";

type Variant = "primary" | "ghost" | "danger";

interface Props extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
}

const styles: Record<Variant, string> = {
  primary:
    "bg-[var(--accent)] text-white hover:brightness-110 disabled:opacity-50",
  ghost:
    "bg-transparent text-[var(--ink)] border border-[var(--line)] hover:bg-white/60 disabled:opacity-50",
  danger:
    "bg-[var(--danger)] text-white hover:brightness-110 disabled:opacity-50",
};

export function Button({
  variant = "primary",
  className = "",
  children,
  ...rest
}: PropsWithChildren<Props>) {
  return (
    <button
      className={`inline-flex items-center justify-center gap-2 rounded-md px-4 py-2 text-sm tracking-wide transition ${styles[variant]} ${className}`}
      {...rest}
    >
      {children}
    </button>
  );
}
