-- The Arc network a database holds. One database indexes exactly one network: mainnet and
-- testnet share the USDC address, and their block numbers would collide in the cursor.
-- `db::bind_network` fills this on first start and refuses any other network afterwards.
CREATE TABLE network (
    id          SMALLINT PRIMARY KEY CHECK (id = 1),
    chain_id    BIGINT NOT NULL,
    bound_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Every start seeds the registry, so a database that has entities ran before the network
-- switch, when Tideline indexed Arc Testnet only.
INSERT INTO network (id, chain_id)
SELECT 1, 5042002
WHERE EXISTS (SELECT 1 FROM entities);
