import type { Metadata } from "next";

import { Providers } from "@/components/providers";
import { SiteHeader } from "@/components/site-header";
import "./globals.css";

export const metadata: Metadata = {
  title: "Inheritance Vault Console",
  description:
    "Owner and trustee console for the FROST + VDF crypto-inheritance vault.",
};

export default function RootLayout({ children }: LayoutProps<"/">) {
  return (
    <html lang="en" className="h-full antialiased">
      <body className="flex min-h-full flex-col bg-slate-950 text-slate-100">
        <Providers>
          <SiteHeader />
          <main className="mx-auto w-full max-w-6xl flex-1 px-4 py-8">
            {children}
          </main>
          <footer className="border-t border-white/5 px-4 py-4 text-center text-xs text-slate-600">
            Local demo · FROST threshold signatures · VDF inactivity proofs
          </footer>
        </Providers>
      </body>
    </html>
  );
}
