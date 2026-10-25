-- Reverts 0002_auth.sql. Not applied by `wrangler d1 migrations apply`; run it
-- with `wrangler d1 execute` only after removing the deployed Worker version
-- that reads these tables. Medication and subscription data is unaffected.
DROP TABLE IF EXISTS oauth_flows;
DROP TABLE IF EXISTS sessions;
DROP TABLE IF EXISTS external_identities;
DROP TABLE IF EXISTS users;
DELETE FROM d1_migrations WHERE name='0002_auth.sql';
