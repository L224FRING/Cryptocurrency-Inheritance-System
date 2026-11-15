export type FrostStateResponse = {
  configured: boolean;
  trustees?: number;
  threshold?: number;
  groupKey?: `0x${string}`;
  session?: `0x${string}`;
  createdAt?: string;
  shares: number[];
  error?: string;
};

export type DkgRequest = {
  trustees: number;
  threshold: number;
};

export type DkgResponse = {
  status: "ok";
  trustees: number;
  threshold: number;
  groupKey: `0x${string}`;
  session: `0x${string}`;
  shares: number[];
};

export type SignRequest = {
  releaseHash: `0x${string}`;
  participants?: number[];
};

export type SignResponse = {
  status: "ok";
  signature: `0x${string}`;
  groupKey: `0x${string}`;
  participants: number[];
  session: `0x${string}`;
};

export type VdfProveRequest = {
  /** Decimal string of `vdf.currentChallenge`, or a `0x` hex word. */
  challenge: string;
  /** On-chain `T`. */
  t: number;
  /** Decimal string of on-chain `N`. */
  n: string;
};

export type VdfProveResponse = {
  status: "ok";
  y: `0x${string}`;
  proofPoints: string[];
  x: `0x${string}`;
  t: number;
  n: `0x${string}`;
};

export type HealthResponse = {
  ok: boolean;
  chainId: number;
  blockNumber?: number;
  rpcUrl: string;
  contracts: {
    vault: `0x${string}`;
    vdf: `0x${string}`;
    frost: `0x${string}`;
  };
  frost: {
    serviceBin: string;
    relayBin: string;
    serviceBinExists: boolean;
    relayBinExists: boolean;
    stateDir: string;
    hasShares: boolean;
  };
  error?: string;
};

export type MineResponse = {
  status: "ok";
  mined: number;
  blockNumber?: number;
};
