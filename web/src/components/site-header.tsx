import Link from "next/link";

import { WalletButton } from "@/components/wallet-button";

export function SiteHeader() {
  return (
    <header className="sticky top-0 z-20 border-b border-white/10 bg-slate-950/80 backdrop-blur">
      <div className="mx-auto flex w-full max-w-6xl items-center justify-between gap-4 px-4 py-3">
        <div className="flex items-center gap-6">
          <Link href="/" className="flex items-center gap-2">
            <span className="grid h-8 w-8 place-items-center rounded-lg bg-emerald-500/15 text-lg">
              🔐
            </span>
            <span className="text-sm font-semibold leading-tight text-slate-100">
              Inheritance
              <span className="block text-[11px] font-normal text-slate-400">
                Vault Console
              </span>
            </span>
          </Link>
          <nav className="hidden items-center gap-1 text-sm sm:flex">
            <Link
              href="/owner"
              className="rounded-lg px-3 py-1.5 text-slate-300 transition hover:bg-white/5 hover:text-white"
            >
              Owner
            </Link>
            <Link
              href="/trustee"
              className="rounded-lg px-3 py-1.5 text-slate-300 transition hover:bg-white/5 hover:text-white"
            >
              Trustee
            </Link>
          </nav>
        </div>
        <WalletButton />
      </div>
    </header>
  );
}
