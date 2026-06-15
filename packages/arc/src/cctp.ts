/** Arc's CCTP V2 domain id. */
export const ARC_CCTP_DOMAIN = 26

/**
 * Known CCTP domain ids → chain names.
 *
 * NOTE: verify against Circle docs before relying on for cross-chain attribution.
 * Domain ids are assigned by Circle and may change / expand over time.
 */
export const CCTP_DOMAINS = {
  0: 'Ethereum',
  1: 'Avalanche',
  2: 'Optimism',
  3: 'Arbitrum',
  5: 'Solana',
  6: 'Base',
  7: 'Polygon',
  26: 'Arc',
} as const

export type CctpDomain = keyof typeof CCTP_DOMAINS
