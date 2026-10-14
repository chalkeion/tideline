import type { Address } from 'viem'
import type { ArcNetwork } from './chains'

type ContractSet = Record<string, Record<string, Address>>

/** Ethereum-ecosystem contracts deployed at the same address on both networks. */
const common = {
  MULTICALL3: '0xcA11bde05977b3631167028862bE2a173976CA11',
  PERMIT2: '0x000000000022D473030F116dDEE9F6B43aC78BA3',
  CREATE2_FACTORY: '0x4e59b44847b379578588920cA78FbF26c0B4956C',
} as const satisfies Record<string, Address>

/**
 * Arc Mainnet contract addresses (chain id 5042).
 * Source: https://docs.arc.io/arc/references/contract-addresses. Verify on
 * https://explorer.arc.io before relying on-chain.
 */
export const mainnetAddresses = {
  stablecoins: {
    USDC: '0x3600000000000000000000000000000000000000',
    EURC: '0xbEf5f6d51CB62b58e6A8f77868681825C6fe21c1',
    USYC: '0x8a5D989Bbb96929F689B0200f435f53dA42bF490',
    USYC_ENTITLEMENTS: '0xb69ecb156Dc0028198028c501340d5367845ca72',
    USYC_TELLER: '0x51A8CE47dC08ba5CD19c7aa84EA6fD6664f60f9b',
  },
  cctp: {
    TOKEN_MESSENGER_V2: '0x28b5a0e9C621a5BadaA536219b3a228C8168cf5d',
    MESSAGE_TRANSMITTER_V2: '0x81D40F21F12A8F0E3252Bccb954D722d4c464B64',
    TOKEN_MINTER_V2: '0xfd78EE919681417d192449715b2594ab58f5D002',
    MESSAGE_V2: '0xec546b6B005471ECf012e5aF77FBeC07e0FD8f78',
    TOKEN_MESSENGER_WITH_FEES: '0x71f54F818671cD0D7ea140Da213e5C8b5C92a408',
    CROSS_CHAIN_TOKEN_SERVICE: '0x431871229103b780868f8C6BB820cd16ECf942BC',
  },
  gateway: {
    GATEWAY_WALLET: '0x77777777Dcc4d5A8B6E418Fd04D8997ef11000eE',
    GATEWAY_MINTER: '0x2222222d7164433c4C09B0b0D809a9b52C04C205',
  },
  payments: {
    FX_ESCROW: '0xe2E5F173576B513d994073CCbDaCBE027d43DFe6',
  },
  common,
} as const satisfies ContractSet

/**
 * Arc Testnet contract addresses (chain id 5042002).
 * Source: https://docs.arc.io/arc/references/contract-addresses. Verify on
 * https://explorer.testnet.arc.io before relying on-chain.
 */
export const testnetAddresses = {
  stablecoins: {
    USDC: '0x3600000000000000000000000000000000000000',
    EURC: '0x89B50855Aa3bE2F677cD6303Cec089B5F319D72a',
    USYC: '0xe9185F0c5F296Ed1797AaE4238D26CCaBEadb86C',
    USYC_ENTITLEMENTS: '0xcc205224862c7641930c87679e98999d23c26113',
    USYC_TELLER: '0x9fdF14c5B14173D74C08Af27AebFf39240dC105A',
  },
  cctp: {
    TOKEN_MESSENGER_V2: '0x8FE6B999Dc680CcFDD5Bf7EB0974218be2542DAA',
    MESSAGE_TRANSMITTER_V2: '0xE737e5cEBEEBa77EFE34D4aa090756590b1CE275',
    TOKEN_MINTER_V2: '0xb43db544E2c27092c107639Ad201b3dEfAbcF192',
    MESSAGE_V2: '0xbaC0179bB358A8936169a63408C8481D582390C4',
    TOKEN_MESSENGER_WITH_FEES: '0x8745D906D67C346E5eb1aEEED38Eb87F34DF0C0A',
    CROSS_CHAIN_TOKEN_SERVICE: '0x63753E722bd2C2A5DF6EE19C5106662208B81077',
  },
  gateway: {
    GATEWAY_WALLET: '0x0077777d7EBA4688BDeF3E311b846F25870A19B9',
    GATEWAY_MINTER: '0x0022222ABE238Cc2C7Bb1f21003F0a260052475B',
  },
  payments: {
    FX_ESCROW: '0xd68256f4D69C6BbEcB873D8588AE0Dc6B8E22E10',
  },
  common,
} as const satisfies ContractSet

export const addresses = {
  mainnet: mainnetAddresses,
  testnet: testnetAddresses,
} as const satisfies Record<ArcNetwork, ContractSet>

export type ArcAddresses = (typeof addresses)[ArcNetwork]
