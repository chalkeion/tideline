import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'
import { addresses, type ArcAddresses } from '../addresses'
import { arcChains, type ArcNetwork } from '../chains'

const HEX_ADDRESS = /^0x[0-9a-fA-F]{40}$/

interface Seed {
  entities: { slug: string; addresses: { network: string; address: string }[] }[]
}

interface TokenSeed {
  tokens: { network: string; address: string; symbol: string }[]
}

/** The stream service's seeds, which must label and list the same contracts. */
function streamSeed<T>(file: string): T {
  return JSON.parse(
    readFileSync(new URL(`../../../../services/stream/data/${file}`, import.meta.url), 'utf8'),
  ) as T
}
const seed = streamSeed<Seed>('entities.seed.json')
const tokenSeed = streamSeed<TokenSeed>('tokens.seed.json')

/** Every contract Circle operates, except MessageV2: a library that never holds funds. */
function circleContracts(set: ArcAddresses): [string, string][] {
  const { stablecoins, cctp, gateway, payments, wrapped } = set
  return Object.entries({
    ...stablecoins,
    ...cctp,
    ...gateway,
    ...payments,
    CIRBTC: wrapped.CIRBTC,
  }).filter(([name]) => name !== 'MESSAGE_V2')
}

describe.each(Object.keys(arcChains) as ArcNetwork[])('%s addresses', (network) => {
  const entries = Object.values(addresses[network]).flatMap((group) => Object.entries(group))

  it('has at least one address', () => {
    expect(entries.length).toBeGreaterThan(0)
  })

  it.each(entries)('%s is a valid 42-char 0x hex address', (_name, addr) => {
    expect(addr).toMatch(HEX_ADDRESS)
    expect(addr).toHaveLength(42)
  })

  const seeded = new Set(
    seed.entities
      .find((e) => e.slug === 'circle')!
      .addresses.filter((a) => a.network === network)
      .map((a) => a.address.toLowerCase()),
  )

  it.each(circleContracts(addresses[network]))('%s is labelled in the registry seed', (_, address) => {
    expect(seeded.has(address.toLowerCase())).toBe(true)
  })

  it('lists exactly the stablecoins and wrapped assets as priced tokens', () => {
    const { stablecoins, wrapped } = addresses[network]
    const tokens = [stablecoins.USDC, stablecoins.EURC, stablecoins.USYC, wrapped.CIRBTC, wrapped.WETH]
    const listed = tokenSeed.tokens.filter((t) => t.network === network).map((t) => t.address.toLowerCase())
    expect(listed.sort()).toEqual(tokens.map((a) => a.toLowerCase()).sort())
  })
})
