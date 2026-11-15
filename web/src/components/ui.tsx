"use client";

import {
  useState,
  type ButtonHTMLAttributes,
  type InputHTMLAttributes,
  type ReactNode,
} from "react";

type Tone = "neutral" | "success" | "warning" | "danger" | "info";

const badgeTones: Record<Tone, string> = {
  neutral: "border-slate-600/60 bg-slate-800/60 text-slate-300",
  success: "border-emerald-500/40 bg-emerald-500/10 text-emerald-300",
  warning: "border-amber-500/40 bg-amber-500/10 text-amber-300",
  danger: "border-rose-500/40 bg-rose-500/10 text-rose-300",
  info: "border-sky-500/40 bg-sky-500/10 text-sky-300",
};

export function Badge({
  tone = "neutral",
  children,
}: {
  tone?: Tone;
  children: ReactNode;
}) {
  return (
    <span
      className={`inline-flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-xs font-medium ${badgeTones[tone]}`}
    >
      {children}
    </span>
  );
}

export function Card({
  title,
  description,
  actions,
  children,
  className = "",
}: {
  title?: ReactNode;
  description?: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section
      className={`rounded-2xl border border-white/10 bg-slate-900/60 p-5 shadow-xl shadow-black/20 backdrop-blur ${className}`}
    >
      {(title || actions) && (
        <header className="mb-4 flex flex-wrap items-start justify-between gap-3">
          <div>
            {title && (
              <h2 className="text-base font-semibold text-slate-100">{title}</h2>
            )}
            {description && (
              <p className="mt-1 text-sm text-slate-400">{description}</p>
            )}
          </div>
          {actions && <div className="flex flex-wrap gap-2">{actions}</div>}
        </header>
      )}
      {children}
    </section>
  );
}

const buttonVariants = {
  primary:
    "bg-emerald-500 text-slate-950 hover:bg-emerald-400 disabled:bg-emerald-500/30 disabled:text-slate-500",
  secondary:
    "border border-white/15 bg-white/5 text-slate-100 hover:bg-white/10 disabled:text-slate-500",
  ghost: "text-slate-300 hover:bg-white/5 disabled:text-slate-600",
  danger:
    "bg-rose-500 text-white hover:bg-rose-400 disabled:bg-rose-500/30 disabled:text-slate-500",
} as const;

export function Button({
  variant = "primary",
  size = "md",
  busy = false,
  children,
  className = "",
  disabled,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: keyof typeof buttonVariants;
  size?: "sm" | "md";
  busy?: boolean;
}) {
  const sizes = size === "sm" ? "px-3 py-1.5 text-xs" : "px-4 py-2 text-sm";
  return (
    <button
      {...props}
      disabled={disabled || busy}
      className={`inline-flex items-center justify-center gap-2 rounded-lg font-semibold transition disabled:cursor-not-allowed ${buttonVariants[variant]} ${sizes} ${className}`}
    >
      {busy && <Spinner />}
      {children}
    </button>
  );
}

export function Spinner({ className = "" }: { className?: string }) {
  return (
    <span
      aria-hidden
      className={`inline-block h-3.5 w-3.5 animate-spin rounded-full border-2 border-current border-t-transparent ${className}`}
    />
  );
}

export function Field({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: ReactNode;
  children: ReactNode;
}) {
  return (
    <label className="block">
      <span className="mb-1.5 block text-xs font-semibold uppercase tracking-wide text-slate-400">
        {label}
      </span>
      {children}
      {hint && <span className="mt-1 block text-xs text-slate-500">{hint}</span>}
    </label>
  );
}

export function TextInput({
  className = "",
  ...props
}: InputHTMLAttributes<HTMLInputElement>) {
  return (
    <input
      {...props}
      className={`w-full rounded-lg border border-white/10 bg-slate-950/60 px-3 py-2 font-mono text-sm text-slate-100 outline-none transition placeholder:text-slate-600 focus:border-emerald-500/60 focus:ring-2 focus:ring-emerald-500/20 ${className}`}
    />
  );
}

export function Stat({
  label,
  value,
  hint,
  mono = false,
}: {
  label: string;
  value: ReactNode;
  hint?: ReactNode;
  mono?: boolean;
}) {
  return (
    <div className="rounded-xl border border-white/5 bg-slate-950/40 px-3 py-2.5">
      <div className="text-xs font-medium uppercase tracking-wide text-slate-500">
        {label}
      </div>
      <div
        className={`mt-1 text-sm text-slate-100 ${mono ? "break-all font-mono" : "font-semibold"}`}
      >
        {value}
      </div>
      {hint && <div className="mt-0.5 text-xs text-slate-500">{hint}</div>}
    </div>
  );
}

export function Alert({
  tone = "info",
  title,
  children,
}: {
  tone?: Tone;
  title?: string;
  children: ReactNode;
}) {
  const tones: Record<Tone, string> = {
    neutral: "border-slate-600/40 bg-slate-800/40 text-slate-300",
    success: "border-emerald-500/30 bg-emerald-500/10 text-emerald-200",
    warning: "border-amber-500/30 bg-amber-500/10 text-amber-200",
    danger: "border-rose-500/30 bg-rose-500/10 text-rose-200",
    info: "border-sky-500/30 bg-sky-500/10 text-sky-200",
  };
  return (
    <div className={`rounded-xl border px-3 py-2.5 text-sm ${tones[tone]}`}>
      {title && <div className="mb-0.5 font-semibold">{title}</div>}
      <div className="text-[13px] leading-relaxed opacity-90">{children}</div>
    </div>
  );
}

export function CopyValue({
  value,
  label,
}: {
  value: string;
  label?: string;
}) {
  const [copied, setCopied] = useState(false);
  return (
    <button
      type="button"
      title={`Copy ${label ?? "value"}`}
      onClick={async () => {
        try {
          await navigator.clipboard.writeText(value);
          setCopied(true);
          setTimeout(() => setCopied(false), 1200);
        } catch {
          setCopied(false);
        }
      }}
      className="group inline-flex max-w-full items-center gap-1.5 rounded-md px-1 py-0.5 text-left font-mono text-xs text-slate-300 hover:bg-white/5 hover:text-slate-100"
    >
      <span className="break-all">{value}</span>
      <span className="shrink-0 text-[10px] uppercase text-slate-500 group-hover:text-slate-300">
        {copied ? "copied" : "copy"}
      </span>
    </button>
  );
}

export function KeyValue({
  label,
  children,
  mono = true,
}: {
  label: string;
  children: ReactNode;
  mono?: boolean;
}) {
  return (
    <div className="flex flex-col gap-0.5 border-b border-white/5 py-2 last:border-0 sm:flex-row sm:items-start sm:justify-between sm:gap-6">
      <dt className="text-xs font-medium uppercase tracking-wide text-slate-500">
        {label}
      </dt>
      <dd
        className={`text-sm text-slate-200 sm:text-right ${mono ? "font-mono break-all" : ""}`}
      >
        {children}
      </dd>
    </div>
  );
}
