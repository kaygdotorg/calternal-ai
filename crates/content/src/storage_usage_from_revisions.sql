SELECT
       COALESCE(SUM(CASE WHEN json_extract(k.definition,'$[1].format')='markdown'
                         THEN length(v.bytes) ELSE 0 END),0),
       COALESCE(SUM(CASE WHEN json_extract(k.definition,'$[1].format')='markdown'
                         THEN 0 ELSE length(v.bytes) END),0)
FROM items i
JOIN versions v ON v.item=i.id AND v.seq=i.seq
LEFT JOIN kinds k ON k.kind=i.kind
