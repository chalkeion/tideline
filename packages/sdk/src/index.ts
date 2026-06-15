export interface TidelineClientOptions {
  baseUrl: string
}

export interface HealthResponse {
  ok: boolean
}

/**
 * Minimal REST/WS client skeleton for the Tideline API.
 * Week 0: only a `health()` probe is implemented.
 */
export class TidelineClient {
  private readonly baseUrl: string

  constructor(opts: TidelineClientOptions) {
    this.baseUrl = opts.baseUrl.replace(/\/$/, '')
  }

  async health(): Promise<HealthResponse> {
    const res = await fetch(`${this.baseUrl}/health`)
    return (await res.json()) as HealthResponse
  }
}
