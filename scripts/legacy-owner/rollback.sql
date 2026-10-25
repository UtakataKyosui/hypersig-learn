-- Returns the rows recorded for @USER@ to the legacy owner, and forgets them.
UPDATE medications SET owner='owner' WHERE owner='@USER@'
 AND id IN(SELECT row_id FROM legacy_owner_migration WHERE table_name='medications' AND new_owner='@USER@');
UPDATE push_subscriptions SET owner='owner' WHERE owner='@USER@'
 AND id IN(SELECT row_id FROM legacy_owner_migration WHERE table_name='push_subscriptions' AND new_owner='@USER@');
DELETE FROM legacy_owner_migration WHERE new_owner='@USER@';
