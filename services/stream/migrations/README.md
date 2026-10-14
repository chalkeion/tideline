# Migrations

SQL migrations for `arc-stablecoin-stream`. They are embedded in the binary with
`sqlx::migrate!` and applied automatically on startup, so there is no separate step.

Add a migration by creating the next numbered file here (`0002_<name>.sql`). Never edit a
migration that has been applied anywhere: sqlx checksums them.
