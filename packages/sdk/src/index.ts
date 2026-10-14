import type {
  AddressLabel,
  AddressProfile,
  ApiKeyCreated,
  ComplianceReport,
  Coverage,
  EntityProfile,
  EntitySummary,
  Flow,
  FlowPage,
  HealthStatus,
  NetworkInfo,
  Overview,
} from '@tideline/schema'

export type * from '@tideline/schema'

export interface TidelineClientOptions {
  baseUrl: string
  /** Data-license key. Without one the client is on the public tier. */
  apiKey?: string
  /** Override for tests or non-standard runtimes. */
  fetch?: typeof fetch
}

export interface FlowQuery {
  asset?: string
  kind?: string
  /** Entity slug on either side. */
  entity?: string
  /** Address on either side. */
  address?: string
  visibility?: 'public' | 'shielded' | 'disclosed'
  min_usd?: number
  since_hours?: number
  /** true: at least one side attributed. false: neither side attributed. */
  attributed?: boolean
  /** Pagination cursor: only flows with a lower id. */
  before?: number
  /** Live tailing: only flows with a higher id. */
  after?: number
  limit?: number
}

export interface AttributionInput {
  entity_slug: string
  /** Required with `category` when the entity does not exist yet. */
  entity_name?: string
  category?: string
  address: string
  label?: string
  confidence?: number
  /** Why the attribution holds. Must be a non-empty object. */
  evidence: Record<string, unknown>
  source?: string
}

export interface AttestationInput {
  entity_slug: string
  address: string
  /** EIP-191 signature by `address` over `attestationMessage(entity_slug, address)`. */
  signature: string
  label?: string
}

export interface DisclosureInput {
  flow_id: number
  /** Token units as a decimal string. */
  amount: string
  disclosed_by: string
  proof?: string
}

export class TidelineError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
    message: string,
  ) {
    super(message)
    this.name = 'TidelineError'
  }
}

/** The exact message an address owner signs to attest control for an entity. */
export function attestationMessage(entitySlug: string, address: string): string {
  return `Tideline attribution: address ${address.toLowerCase()} is controlled by ${entitySlug}.`
}

function queryString(params: object): string {
  const search = new URLSearchParams()
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== null && value !== '') search.set(key, String(value))
  }
  const out = search.toString()
  return out ? `?${out}` : ''
}

/** REST client for the Tideline flow-intelligence API. */
export class TidelineClient {
  private readonly baseUrl: string
  private readonly apiKey?: string
  private readonly fetchImpl: typeof fetch

  constructor(opts: TidelineClientOptions) {
    this.baseUrl = opts.baseUrl.replace(/\/$/, '')
    this.apiKey = opts.apiKey
    this.fetchImpl = opts.fetch ?? ((...args) => fetch(...args))
  }

  private headers(): Record<string, string> {
    return this.apiKey ? { authorization: `Bearer ${this.apiKey}` } : {}
  }

  private async request<T>(path: string, init?: RequestInit): Promise<T> {
    const res = await this.fetchImpl(`${this.baseUrl}${path}`, {
      ...init,
      headers: { ...this.headers(), ...init?.headers },
    })
    if (!res.ok) {
      const body = (await res.json().catch(() => null)) as {
        error?: string
        message?: string
      } | null
      throw new TidelineError(
        res.status,
        body?.error ?? 'http_error',
        body?.message ?? `request failed with status ${res.status}`,
      )
    }
    return (await res.json()) as T
  }

  private post<T>(path: string, body: unknown): Promise<T> {
    return this.request<T>(path, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(body),
    })
  }

  health(): Promise<HealthStatus> {
    return this.request('/health')
  }

  /** The Arc network the service indexes, with its chain id and explorer. */
  network(): Promise<NetworkInfo> {
    return this.request('/v1/network')
  }

  overview(windowHours = 24): Promise<Overview> {
    return this.request(`/v1/overview${queryString({ window_hours: windowHours })}`)
  }

  coverage(windowHours = 24): Promise<Coverage> {
    return this.request(`/v1/coverage${queryString({ window_hours: windowHours })}`)
  }

  flows(query: FlowQuery = {}): Promise<FlowPage> {
    return this.request(`/v1/flows${queryString(query)}`)
  }

  flow(id: number): Promise<Flow> {
    return this.request(`/v1/flows/${id}`)
  }

  entities(): Promise<EntitySummary[]> {
    return this.request('/v1/entities')
  }

  entity(slug: string, windowHours = 24): Promise<EntityProfile> {
    return this.request(
      `/v1/entities/${encodeURIComponent(slug)}${queryString({ window_hours: windowHours })}`,
    )
  }

  address(address: string): Promise<AddressProfile> {
    return this.request(`/v1/addresses/${encodeURIComponent(address)}`)
  }

  /** Every flow touching an entity in the window, with attribution evidence. Licensed. */
  complianceReport(entity: string, windowHours = 24): Promise<ComplianceReport> {
    return this.request(
      `/v1/compliance/report${queryString({ entity, window_hours: windowHours })}`,
    )
  }

  /** The same report as CSV. Licensed. */
  async complianceReportCsv(entity: string, windowHours = 24): Promise<string> {
    const path = `/v1/compliance/report${queryString({ entity, window_hours: windowHours, format: 'csv' })}`
    const res = await this.fetchImpl(`${this.baseUrl}${path}`, { headers: this.headers() })
    if (!res.ok) {
      throw new TidelineError(res.status, 'http_error', `report failed with status ${res.status}`)
    }
    return res.text()
  }

  /** Curated attribution. Admin. */
  createAttribution(input: AttributionInput): Promise<AddressLabel[]> {
    return this.post('/v1/attributions', input)
  }

  /** Signed self-attestation. Admin. */
  createAttestation(input: AttestationInput): Promise<AddressLabel[]> {
    return this.post('/v1/attestations', input)
  }

  /** Reveal the amount of a shielded flow. Admin. */
  createDisclosure(input: DisclosureInput): Promise<Flow> {
    return this.post('/v1/disclosures', input)
  }

  /** Mint a data-license key. Admin. */
  createKey(name: string, tier: 'licensed' | 'admin' = 'licensed'): Promise<ApiKeyCreated> {
    return this.post('/v1/keys', { name, tier })
  }
}
