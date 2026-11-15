const ZERO = "0x0000000000000000000000000000000000000000" as const;

function addressFromEnv(value: string | undefined): `0x${string}` {
  if (value && /^0x[0-9a-fA-F]{40}$/.test(value)) {
    return value as `0x${string}`;
  }
  return ZERO;
}

export const appConfig = {
  rpcUrl: process.env.NEXT_PUBLIC_RPC_URL ?? "http://127.0.0.1:8545",
  chainId: Number(process.env.NEXT_PUBLIC_CHAIN_ID ?? 31337),
  vault: addressFromEnv(process.env.NEXT_PUBLIC_VAULT_ADDRESS),
  vdf: addressFromEnv(process.env.NEXT_PUBLIC_VDF_ADDRESS),
  frost: addressFromEnv(process.env.NEXT_PUBLIC_FROST_ADDRESS),
  zero: ZERO,
};

export const isConfigured =
  appConfig.vault !== ZERO && appConfig.vdf !== ZERO && appConfig.frost !== ZERO;
