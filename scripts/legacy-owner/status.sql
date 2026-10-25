SELECT 'medications' AS item, owner, count(*) AS rows FROM medications WHERE owner='owner' OR owner='@USER@' GROUP BY owner
UNION ALL
SELECT 'push_subscriptions', owner, count(*) FROM push_subscriptions WHERE owner='owner' OR owner='@USER@' GROUP BY owner
UNION ALL
SELECT 'recorded_' || table_name, new_owner, count(*) FROM legacy_owner_migration WHERE new_owner='@USER@' GROUP BY table_name;
