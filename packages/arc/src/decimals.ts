export const ISSUER_DECIMALS = {
  USDC: 6,
  EURC: 6,
  USYC: 6,
  /**
   * Gotcha: USDC is a 6-decimal ERC-20 token, but Arc uses USDC as the native
   * gas currency with 18 decimals (like ETH on Ethereum). Always use 18 when
   * formatting native balances / gas, and 6 when formatting ERC-20 transfers.
   */
  USDC_NATIVE_GAS: 18,
} as const

export type IssuerDecimals = typeof ISSUER_DECIMALS
