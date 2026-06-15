import { describe, expect, it } from 'vitest'
import { TidelineClient } from './index'

describe('TidelineClient', () => {
  it('constructs and exposes health()', () => {
    const client = new TidelineClient({ baseUrl: 'http://localhost:8080' })
    expect(typeof client.health).toBe('function')
  })
})
