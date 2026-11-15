import Link from "next/link";

import { HealthPanel } from "@/components/health-panel";
import { Badge, Card } from "@/components/ui";

const steps = [
  {
    actor: "Owner",
    title: "Set up the vault",
    body: "Connect MetaMask, choose a beneficiary, fund the vault and run a FROST DKG ceremony to mint the trustee committee key.",
    href: "/owner",
    tone: "success" as const,
  },
  {
    actor: "Owner",
    title: "Stay alive",
    body: "Periodic check-ins reset the vault challenge and the VDF inactivity clock. Missing them starts the inheritance timer.",
    href: "/owner",
    tone: "info" as const,
  },
  {
    actor: "Trustee",
    title: "Prove inactivity",
    body: "Once the required delay has passed, any account can generate a VDF proof and submit it to confirm owner inactivity.",
    href: "/trustee",
    tone: "warning" as const,
  },
  {
    actor: "Trustee",
    title: "Release the inheritance",
    body: "A t-of-n FROST committee signs the release hash; the vault verifies the signature on-chain and pays the beneficiary.",
    href: "/trustee",
    tone: "danger" as const,
  },
];

export default function Home() {
  return (
    <div className="space-y-8">
      <section className="grid gap-6 lg:grid-cols-[1.4fr_1fr] lg:items-center">
        <div>
          <Badge tone="success">FROST · VDF · MetaMask</Badge>
          <h1 className="mt-4 text-4xl font-bold leading-tight tracking-tight text-white sm:text-5xl">
            A crypto-inheritance vault that cannot be rushed.
          </h1>
          <p className="mt-4 max-w-2xl text-base leading-relaxed text-slate-400">
            The owner protects assets behind a periodic proof-of-life. If they
            stop checking in, a verifiable delay function confirms the
            inactivity, and a threshold of trustees sign the release. No single
            trustee, and no early heir, can move funds alone.
          </p>
          <div className="mt-6 flex flex-wrap gap-3">
            <Link
              href="/owner"
              className="inline-flex items-center gap-2 rounded-lg bg-emerald-500 px-5 py-2.5 text-sm font-semibold text-slate-950 transition hover:bg-emerald-400"
            >
              Open owner console →
            </Link>
            <Link
              href="/trustee"
              className="inline-flex items-center gap-2 rounded-lg border border-white/15 bg-white/5 px-5 py-2.5 text-sm font-semibold text-slate-100 transition hover:bg-white/10"
            >
              Open trustee console
            </Link>
          </div>
          <p className="mt-4 text-xs text-slate-500">
            Runs against local Anvil. Import the Anvil deployer account into
            MetaMask to act as the owner.
          </p>
        </div>

        <Card
          title="How the double gate works"
          description="Both conditions must hold before the vault pays out."
        >
          <ol className="space-y-3">
            <li className="rounded-xl border border-white/5 bg-slate-950/40 p-3">
              <div className="flex items-center gap-2">
                <span className="grid h-6 w-6 shrink-0 place-items-center rounded-full bg-emerald-500/15 text-xs font-bold text-emerald-300">
                  1
                </span>
                <span className="text-sm font-semibold text-slate-100">
                  VDF inactivity proof
                </span>
              </div>
              <p className="mt-1.5 text-xs leading-relaxed text-slate-400">
                A sequential computation over the last check-in challenge. It
                cannot be shortcut, so trustees cannot fake the passage of time.
              </p>
            </li>
            <li className="rounded-xl border border-white/5 bg-slate-950/40 p-3">
              <div className="flex items-center gap-2">
                <span className="grid h-6 w-6 shrink-0 place-items-center rounded-full bg-sky-500/15 text-xs font-bold text-sky-300">
                  2
                </span>
                <span className="text-sm font-semibold text-slate-100">
                  FROST threshold signature
                </span>
              </div>
              <p className="mt-1.5 text-xs leading-relaxed text-slate-400">
                At least <span className="font-mono">t</span> of{" "}
                <span className="font-mono">n</span> trustees must sign the
                vault, beneficiary and current challenge. The key never exists
                in one place.
              </p>
            </li>
          </ol>
        </Card>
      </section>

      <section className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        {steps.map((step) => (
          <Link key={step.title} href={step.href} className="group">
            <Card
              className="h-full transition group-hover:border-emerald-500/30 group-hover:bg-slate-900/80"
              title={step.title}
              actions={<Badge tone={step.tone}>{step.actor}</Badge>}
            >
              <p className="text-sm leading-relaxed text-slate-400">
                {step.body}
              </p>
            </Card>
          </Link>
        ))}
      </section>

      <HealthPanel />
    </div>
  );
}
