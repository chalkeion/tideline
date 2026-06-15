import { parseAbi } from 'viem'

/**
 * Minimal ERC-20 ABI covering just the `Transfer` event — sufficient for
 * monitoring USDC / EURC / USYC token movements on Arc.
 */
export const erc20TransferAbi = parseAbi([
  'event Transfer(address indexed from, address indexed to, uint256 value)',
])
