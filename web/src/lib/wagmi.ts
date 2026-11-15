import { createConfig, http } from "wagmi";
import { injected } from "wagmi/connectors";
import { defineChain } from "viem";

import { appConfig } from "@/lib/config";

/**
 * The local Anvil chain, or whatever `NEXT_PUBLIC_CHAIN_ID` points at.
 * Defining it from env means a testnet deployment only needs a different
 * `.env.local`; no code changes.
 */
export const targetChain = defineChain({
  id: appConfig.chainId,
  name: appConfig.chainId === 31337 ? "Anvil Local" : `Chain ${appConfig.chainId}`,
  nativeCurrency: { name: "Ether", symbol: "ETH", decimals: 18 },
  rpcUrls: {
    default: { http: [appConfig.rpcUrl] },
  },
  testnet: true,
});

export const wagmiConfig = createConfig({
  chains: [targetChain],
  connectors: [injected({ shimDisconnect: true })],
  transports: {
    [targetChain.id]: http(appConfig.rpcUrl),
  },
  ssr: true,
});
