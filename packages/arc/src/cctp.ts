/** Arc's CCTP V2 domain id. */
export const ARC_CCTP_DOMAIN = 26

/**
 * CCTP V2 domain ids → chain names. Circle assigns them and adds more over time.
 * Source: https://developers.circle.com/cctp/concepts/supported-chains-and-domains
 */
export const CCTP_DOMAINS = {
  0: 'Ethereum',
  1: 'Avalanche',
  2: 'OP Mainnet',
  3: 'Arbitrum',
  5: 'Solana',
  6: 'Base',
  7: 'Polygon PoS',
  8: 'Sui',
  9: 'Aptos',
  10: 'Unichain',
  11: 'Linea',
  12: 'Codex',
  13: 'Sonic',
  14: 'World Chain',
  15: 'Monad',
  16: 'Sei',
  17: 'BNB Smart Chain',
  18: 'XDC',
  19: 'HyperEVM',
  21: 'Ink',
  22: 'Plume',
  25: 'Starknet',
  26: 'Arc',
  27: 'Stellar',
  28: 'EDGE',
  29: 'Injective',
  30: 'Morph',
  31: 'Pharos',
  32: 'Cronos',
  33: 'Plasma',
  37: 'X Layer',
} as const

export type CctpDomain = keyof typeof CCTP_DOMAINS
