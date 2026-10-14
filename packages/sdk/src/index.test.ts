import { describe, expect, it, vi } from 'vitest'
import { attestationMessage, TidelineClient, TidelineError } from './index'

function mockFetch(status: number, body: unknown) {
  return vi.fn(async (_input: RequestInfo | URL, _init?: RequestInit) =>
    new Response(JSON.stringify(body), { status }),
  )
}

describe('TidelineClient', () => {
  it('builds flow queries, dropping unset filters and trailing slashes', async () => {
    const fetch = mockFetch(200, { flows: [], next_cursor: null })
    const client = new TidelineClient({ baseUrl: 'http://api.test/', fetch })
    await client.flows({ asset: 'USDC', min_usd: 1000, entity: undefined, attributed: false })
    expect(fetch.mock.calls[0]?.[0]).toBe(
      'http://api.test/v1/flows?asset=USDC&min_usd=1000&attributed=false',
    )
  })

  it('sends the license key as a bearer token only when configured', async () => {
    const fetch = mockFetch(200, [])
    await new TidelineClient({ baseUrl: 'http://api.test', apiKey: 'tdl_x', fetch }).entities()
    await new TidelineClient({ baseUrl: 'http://api.test', fetch }).entities()
    expect(fetch.mock.calls[0]?.[1]?.headers).toEqual({ authorization: 'Bearer tdl_x' })
    expect(fetch.mock.calls[1]?.[1]?.headers).toEqual({})
  })

  it('surfaces API errors with their status and code', async () => {
    const fetch = mockFetch(401, { error: 'license_required', message: 'needs a key' })
    const client = new TidelineClient({ baseUrl: 'http://api.test', fetch })
    const error = await client.complianceReport('circle').catch((e: unknown) => e)
    expect(error).toBeInstanceOf(TidelineError)
    expect(error).toMatchObject({ status: 401, code: 'license_required', message: 'needs a key' })
  })

  it('posts attributions as JSON', async () => {
    const fetch = mockFetch(200, [])
    const client = new TidelineClient({ baseUrl: 'http://api.test', apiKey: 'k', fetch })
    await client.createAttribution({
      entity_slug: 'visa',
      address: '0xabc',
      evidence: { note: 'x' },
    })
    const init = fetch.mock.calls[0]?.[1]
    expect(init?.method).toBe('POST')
    expect(JSON.parse(String(init?.body))).toMatchObject({ entity_slug: 'visa' })
  })

  it('addresses assets by contract', async () => {
    const fetch = mockFetch(200, { window_hours: 1, assets: [] })
    const client = new TidelineClient({ baseUrl: 'http://api.test', fetch })
    await client.assets(1)
    await client.asset('0xabc', 168)
    expect(fetch.mock.calls.map((c) => c[0])).toEqual([
      'http://api.test/v1/assets?window_hours=1',
      'http://api.test/v1/assets/0xabc?window_hours=168',
    ])
  })

  it('reads the indexed network', async () => {
    const network = { key: 'mainnet', name: 'Arc Mainnet', chain_id: 5042, explorer_url: 'https://explorer.arc.io' }
    const fetch = mockFetch(200, network)
    await expect(new TidelineClient({ baseUrl: 'http://api.test', fetch }).network()).resolves.toEqual(network)
    expect(fetch.mock.calls[0]?.[0]).toBe('http://api.test/v1/network')
  })

  it('lowercases the address in the attestation message', () => {
    expect(attestationMessage('visa', '0xABCD')).toBe(
      'Tideline attribution: address 0xabcd is controlled by visa.',
    )
  })
})
