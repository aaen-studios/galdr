/**
 * Quality-tier mapping for hardware video encoders.
 *
 * Tiers indicate relative compression efficiency and ecosystem maturity.
 * Used only for sorting and plain-text labels — no colours or emoji.
 */

export interface EncoderTier {
  /** Canonical tier key. */
  tier: "best" | "good" | "fair" | "basic";
  /** Plain-text label. */
  label: string;
}

const TIER_MAP: Record<string, EncoderTier> = {
  nvidia: { tier: "best", label: "Best" },
  amd:    { tier: "good", label: "Good" },
  intel:  { tier: "fair", label: "Fair" },
  apple:  { tier: "fair", label: "Fair" },
  vaapi:  { tier: "basic", label: "Basic" },
};

const DEFAULT_TIER: EncoderTier = { tier: "fair", label: "Fair" };

/**
 * Returns the quality tier for a given vendor key.
 */
export function getEncoderTier(vendor: string): EncoderTier {
  return TIER_MAP[vendor] ?? DEFAULT_TIER;
}

/**
 * Quality tiers ordered by rank (highest first), useful for sorting.
 */
export const TIER_RANK: Record<string, number> = {
  best: 4,
  good: 3,
  fair: 2,
  basic: 1,
};
