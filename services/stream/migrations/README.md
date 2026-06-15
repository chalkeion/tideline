# Migrations

SQL migrations for `arc-stablecoin-stream`, applied with the [`sqlx`](https://github.com/launchbadge/sqlx) CLI.

## Usage

```bash
# install once
cargo install sqlx-cli --no-default-features --features rustls,postgres

# create a new migration
sqlx migrate add <name>

# apply pending migrations (uses DATABASE_URL)
sqlx migrate run

# revert the last migration
sqlx migrate revert
```

`DATABASE_URL` is read from the environment (see root `.env.example`). No migrations
exist yet at Week 0.
