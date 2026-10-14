import { parseAbi } from 'viem'

/**
 * Gateway Wallet and Gateway Minter events Tideline decodes.
 * Signatures verified against live Arc Testnet logs (topic0 match).
 */
export const GATEWAY_ABI = parseAbi([
  'event Deposited(address indexed token, address indexed depositor, address indexed sender, uint256 value)',
  'event GatewayBurned(address indexed token, address indexed depositor, bytes32 indexed transferSpecHash, uint32 destinationDomain, bytes32 destinationRecipient, address signer, uint256 value, uint256 fee, uint256 fromAvailable, uint256 fromWithdrawing)',
  'event AttestationUsed(address indexed token, address indexed recipient, bytes32 indexed transferSpecHash, uint32 sourceDomain, bytes32 sourceDepositor, bytes32 sourceSigner, uint256 value)',
])
