-- Every token contract seen on the network. Symbols are free text on chain (anyone can
-- deploy a token called USDC), so a token is identified by its address. Only a listed token,
-- one whose address Arc or its issuer publishes (data/tokens.seed.json), owns its symbol and
-- carries a price; everything else is discovered from its own logs and left unpriced.
CREATE TABLE tokens (
    address           TEXT PRIMARY KEY,
    -- erc20 | erc721
    standard          TEXT NOT NULL,
    -- As the contract reports them. NULL when the call reverts or returns garbage.
    symbol            TEXT,
    name              TEXT,
    -- NULL when unknown: amounts are then shown in raw units. 0 for NFTs.
    decimals          INTEGER,
    listed            BOOLEAN NOT NULL DEFAULT false,
    -- Entity slug of the issuer, for listed tokens.
    issuer_slug       TEXT,
    -- usd | eur | usyc | btc | eth. NULL: unpriced.
    price_source      TEXT,
    first_seen_block  BIGINT,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX tokens_symbol_idx ON tokens (upper(symbol));

ALTER TABLE flows ADD COLUMN asset_address TEXT;
-- ERC-721 token id. NULL for fungible tokens.
ALTER TABLE flows ADD COLUMN token_id NUMERIC(78, 0);

-- Flows stored before this migration are USDC, EURC or USYC on the database's network.
UPDATE flows f SET asset_address = t.address
FROM network n, (VALUES
    (5042,    'USDC', '0x3600000000000000000000000000000000000000'),
    (5042,    'EURC', '0xbef5f6d51cb62b58e6a8f77868681825c6fe21c1'),
    (5042,    'USYC', '0x8a5d989bbb96929f689b0200f435f53da42bf490'),
    (5042002, 'USDC', '0x3600000000000000000000000000000000000000'),
    (5042002, 'EURC', '0x89b50855aa3be2f677cd6303cec089b5f319d72a'),
    (5042002, 'USYC', '0xe9185f0c5f296ed1797aae4238d26ccabeadb86c')
) AS t (chain_id, symbol, address)
WHERE n.chain_id = t.chain_id AND f.asset = t.symbol;

ALTER TABLE flows ALTER COLUMN asset_address SET NOT NULL;
CREATE INDEX flows_asset_idx ON flows (asset_address, block_time DESC);
