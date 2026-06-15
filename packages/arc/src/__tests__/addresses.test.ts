import { describe, expect, it } from 'vitest'
import { addresses } from '../addresses'

const HEX_ADDRESS = /^0x[0-9a-fA-F]{40}$/

describe('arc addresses', () => {
  const entries = Object.values(addresses).flatMap((group) =>
    Object.entries(group),
  )

  it('has at least one address', () => {
    expect(entries.length).toBeGreaterThan(0)
  })

  it.each(entries)('%s is a valid 42-char 0x hex address', (_name, addr) => {
    expect(addr).toMatch(HEX_ADDRESS)
    expect(addr).toHaveLength(42)
  })
})
