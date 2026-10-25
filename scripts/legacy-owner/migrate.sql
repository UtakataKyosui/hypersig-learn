-- Hands the single-owner data (owner = 'owner') to the user @USER@. Nothing
-- changes unless that user exists and is active. Every moved row is recorded
-- in legacy_owner_migration, which `rollback.sql` reads.
INSERT OR IGNORE INTO legacy_owner_migration(table_name,row_id,new_owner,migrated_at)
 SELECT 'medications',id,'@USER@',strftime('%s','now')*1000 FROM medications
 WHERE owner='owner' AND EXISTS(SELECT 1 FROM users WHERE id='@USER@' AND status='active');
INSERT OR IGNORE INTO legacy_owner_migration(table_name,row_id,new_owner,migrated_at)
 SELECT 'push_subscriptions',id,'@USER@',strftime('%s','now')*1000 FROM push_subscriptions
 WHERE owner='owner' AND EXISTS(SELECT 1 FROM users WHERE id='@USER@' AND status='active');
UPDATE medications SET owner='@USER@' WHERE owner='owner'
 AND id IN(SELECT row_id FROM legacy_owner_migration WHERE table_name='medications' AND new_owner='@USER@');
UPDATE push_subscriptions SET owner='@USER@' WHERE owner='owner'
 AND id IN(SELECT row_id FROM legacy_owner_migration WHERE table_name='push_subscriptions' AND new_owner='@USER@');
