import { toEventSelector } from 'viem'
import { describe, expect, it } from 'vitest'
import { CCTP_V2_ABI, GATEWAY_ABI } from '../abis'

/** topic0 values observed on Arc Testnet and Mainnet; the Rust decoder matches on the same constants. */
const ON_CHAIN_TOPICS: Record<string, string> = {
  DepositForBurn: '0x0c8c1cbdc5190613ebd485511d4e2812cfa45eecb79d845893331fedad5130a5',
  MessageSent: '0x8c5261668696ce22758910d05bab8f186d6eb247ceac2af2e82c7dc17669b036',
  Deposited: '0x4174a9435a04d04d274c76779cad136a41fde6937c56241c09ab9d3c7064a1a9',
  GatewayBurned: '0x12ee2719e7e2dec9f2a0041286b66669153dff0d36719f692b8bbaa4dfe0aa87',
  AttestationUsed: '0xbb312ce0cc311b2cb0746e09ccd2f91fdb9e2ac755d2f11c65300eb0d0fffd63',
}

describe('protocol ABIs', () => {
  const events = [...CCTP_V2_ABI, ...GATEWAY_ABI]

  it.each(Object.entries(ON_CHAIN_TOPICS))('%s hashes to its on-chain topic0', (name, topic) => {
    const event = events.find((e) => e.name === name)
    expect(event).toBeDefined()
    expect(toEventSelector(event!)).toBe(topic)
  })
})
