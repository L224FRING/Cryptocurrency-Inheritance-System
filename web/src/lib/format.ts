import { formatUnits } from "viem";

export function shorten(
  value: string | null | undefined,
  head = 6,
  tail = 4,
): string {
  if (!value) return "—";
  if (value.length <= head + tail + 2) return value;
  return `${value.slice(0, head)}…${value.slice(-tail)}`;
}

export function formatAmount(
  value: bigint | null | undefined,
  decimals = 18,
  digits = 4,
): string {
  if (value === null || value === undefined) return "—";
  const raw = formatUnits(value, decimals);
  const [whole, fraction = ""] = raw.split(".");
  const trimmed = fraction.slice(0, digits).replace(/0+$/, "");
  return trimmed ? `${whole}.${trimmed}` : whole;
}

export function formatDate(seconds: bigint | null | undefined): string {
  if (seconds === null || seconds === undefined) return "—";
  return new Date(Number(seconds) * 1000).toLocaleString();
}

export function formatBlock(block: bigint | null | undefined): string {
  return block === null || block === undefined ? "—" : block.toString();
}

/** Convert a decimal or `0x` value into a 32-byte hex quantity. */
export function toWord(value: string | bigint): `0x${string}` {
  const big = typeof value === "bigint" ? value : BigInt(value);
  return `0x${big.toString(16).padStart(64, "0")}`;
}

/** Convert a decimal or `0x` value into an even-length hex quantity (no padding). */
export function toQuantity(value: string | bigint): `0x${string}` {
  const big = typeof value === "bigint" ? value : BigInt(value);
  let hex = big.toString(16);
  if (hex.length % 2 !== 0) hex = `0${hex}`;
  return `0x${hex}`;
}

/**
 * On-chain compressed key `prefix || x` from a verifier's stored coordinates.
 * Returns `undefined` when the verifier has no key.
 */
export function compressedGroupKey(
  hasKey: boolean | undefined,
  x: bigint | undefined,
  prefix: number | undefined,
): `0x${string}` | undefined {
  if (!hasKey || x === undefined || prefix === undefined) return undefined;
  return `0x${prefix.toString(16).padStart(2, "0")}${x
    .toString(16)
    .padStart(64, "0")}`;
}
