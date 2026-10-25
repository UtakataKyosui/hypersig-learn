-- Which client started each OAuth flow, so a client's unconsumed flows can be
-- counted. `client` is a hash of the connecting address, never the address.
ALTER TABLE oauth_flows ADD COLUMN client TEXT NOT NULL DEFAULT '';
CREATE INDEX flows_client ON oauth_flows(client, expires_at);
