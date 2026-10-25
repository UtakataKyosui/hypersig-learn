-- Reverts 0004_flow_limit.sql. Not applied by `wrangler d1 migrations apply`; run it
-- with `wrangler d1 execute` only after removing the deployed Worker version
-- that writes `client`.
DROP INDEX IF EXISTS flows_client;
ALTER TABLE oauth_flows DROP COLUMN client;
DELETE FROM d1_migrations WHERE name='0004_flow_limit.sql';
