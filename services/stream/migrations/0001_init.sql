-- Tideline core schema: entity attribution graph + enriched stablecoin flows.

CREATE TABLE entities (
    id          BIGSERIAL PRIMARY KEY,
    slug        TEXT NOT NULL UNIQUE,
    name        TEXT NOT NULL,
    -- issuer | protocol | asset_manager | bank | payments | market_infra | trading | infrastructure
    category    TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    website     TEXT,
    -- Why this entity is in the registry at all (provenance of the entity, not of any address).
    source      TEXT NOT NULL DEFAULT '',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- One row per (address, entity, method) attribution claim, each carrying its own evidence.
CREATE TABLE address_labels (
    id          BIGSERIAL PRIMARY KEY,
    address     TEXT NOT NULL,
    entity_id   BIGINT NOT NULL REFERENCES entities (id) ON DELETE CASCADE,
    label       TEXT NOT NULL DEFAULT '',
    -- protocol_registry | self_attested | curated | heuristic
    method      TEXT NOT NULL,
    confidence  REAL NOT NULL CHECK (confidence > 0 AND confidence <= 1),
    evidence    JSONB NOT NULL DEFAULT '{}'::jsonb,
    source      TEXT NOT NULL DEFAULT '',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (address, entity_id, method)
);
CREATE INDEX address_labels_address_idx ON address_labels (address);

-- Highest-confidence attribution per address. Flows join against this at read time, so a
-- new attribution re-labels all history without rewriting flow rows.
CREATE VIEW address_best_label AS
SELECT DISTINCT ON (l.address)
    l.address, l.entity_id, l.label, l.method, l.confidence, e.slug, e.name, e.category
FROM address_labels l
JOIN entities e ON e.id = l.entity_id
ORDER BY l.address, l.confidence DESC, l.id;

-- On-chain control relationships observed between addresses (clustering edges).
CREATE TABLE address_links (
    address_a   TEXT NOT NULL,
    address_b   TEXT NOT NULL,
    -- gateway_delegate: b signed a Gateway burn spending a's balance.
    link_type   TEXT NOT NULL,
    evidence_tx TEXT NOT NULL,
    seen_count  BIGINT NOT NULL DEFAULT 1,
    first_seen  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (address_a, address_b, link_type)
);
CREATE INDEX address_links_b_idx ON address_links (address_b);

CREATE TABLE flows (
    id            BIGSERIAL PRIMARY KEY,
    tx_hash       TEXT NOT NULL,
    log_index     INTEGER NOT NULL,
    block_number  BIGINT NOT NULL,
    block_time    TIMESTAMPTZ NOT NULL,
    ingested_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- True when the block was produced after this ingestor started (tip-following, not
    -- backfill). Only live flows count toward the settlement-to-attribution latency claim.
    live          BOOLEAN NOT NULL DEFAULT false,
    asset         TEXT NOT NULL,
    kind          TEXT NOT NULL,
    from_address  TEXT NOT NULL,
    to_address    TEXT NOT NULL,
    -- Raw token units. NULL when the amount is shielded (Arc confidential transfer).
    amount        NUMERIC(78, 0),
    usd_value     DOUBLE PRECISION,
    -- public | shielded | disclosed
    visibility    TEXT NOT NULL DEFAULT 'public',
    src_domain    INTEGER,
    dst_domain    INTEGER,
    details       JSONB NOT NULL DEFAULT '{}'::jsonb,
    UNIQUE (tx_hash, log_index)
);
CREATE INDEX flows_block_time_idx ON flows (block_time DESC);
CREATE INDEX flows_from_idx ON flows (from_address, block_time DESC);
CREATE INDEX flows_to_idx ON flows (to_address, block_time DESC);
CREATE INDEX flows_kind_idx ON flows (kind, block_time DESC);

-- Selective disclosures: an amount revealed for a shielded flow by a party holding a view key.
CREATE TABLE disclosures (
    id            BIGSERIAL PRIMARY KEY,
    flow_id       BIGINT NOT NULL REFERENCES flows (id) ON DELETE CASCADE,
    amount        NUMERIC(78, 0) NOT NULL,
    disclosed_by  TEXT NOT NULL,
    proof         TEXT NOT NULL DEFAULT '',
    -- operator_asserted until Arc publishes a verifiable view-key proof format.
    verification  TEXT NOT NULL DEFAULT 'operator_asserted',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (flow_id)
);

CREATE TABLE ingest_cursor (
    id          SMALLINT PRIMARY KEY CHECK (id = 1),
    next_block  BIGINT NOT NULL,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Data-license API keys. Only the keccak256 hash of a key is stored.
CREATE TABLE api_keys (
    id          BIGSERIAL PRIMARY KEY,
    name        TEXT NOT NULL,
    -- licensed | admin
    tier        TEXT NOT NULL,
    key_hash    TEXT NOT NULL UNIQUE,
    key_prefix  TEXT NOT NULL,
    revoked     BOOLEAN NOT NULL DEFAULT false,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Metered usage per key per day: the billing basis for the data license.
CREATE TABLE api_usage (
    key_id    BIGINT NOT NULL REFERENCES api_keys (id) ON DELETE CASCADE,
    day       DATE NOT NULL,
    requests  BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (key_id, day)
);
