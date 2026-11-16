-- Array memberships retain explicit ordering. Deletion must not turn a scoped
-- notification route into the empty-array "all monitors" default.
CREATE FUNCTION aegis_monitor_deleted() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 UPDATE notification_channels SET
   monitor_ids=array_remove(monitor_ids,OLD.id),
   enabled=enabled AND cardinality(array_remove(monitor_ids,OLD.id))>0,
   updated_at=now()
 WHERE OLD.id=ANY(monitor_ids);
 UPDATE status_resources SET monitor_ids=array_remove(monitor_ids,OLD.id),updated_at=now() WHERE OLD.id=ANY(monitor_ids);
 UPDATE status_pages SET monitor_ids=array_remove(monitor_ids,OLD.id),updated_at=now() WHERE OLD.id=ANY(monitor_ids);
 UPDATE maintenance_windows SET monitor_ids=array_remove(monitor_ids,OLD.id) WHERE OLD.id=ANY(monitor_ids);
 UPDATE notification_deliveries SET state='failed',last_error='Monitor was deleted; delivery cancelled'
 WHERE state='pending' AND payload->>'monitor_id'=OLD.id::text;
 RETURN OLD;
END;
$$;
CREATE TRIGGER monitors_cleanup AFTER DELETE ON monitors FOR EACH ROW EXECUTE FUNCTION aegis_monitor_deleted();

CREATE FUNCTION aegis_resource_deleted() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 UPDATE status_pages SET resource_ids=array_remove(resource_ids,OLD.id),updated_at=now() WHERE OLD.id=ANY(resource_ids);
 UPDATE incidents SET resource_ids=array_remove(resource_ids,OLD.id),
   published=published AND cardinality(array_remove(resource_ids,OLD.id))>0,updated_at=now()
 WHERE OLD.id=ANY(resource_ids);
 UPDATE maintenance_windows SET resource_ids=array_remove(resource_ids,OLD.id) WHERE OLD.id=ANY(resource_ids);
 RETURN OLD;
END;
$$;
CREATE TRIGGER resources_cleanup AFTER DELETE ON status_resources FOR EACH ROW EXECUTE FUNCTION aegis_resource_deleted();

CREATE FUNCTION aegis_page_deleted() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 UPDATE incidents SET status_page_ids=array_remove(status_page_ids,OLD.id),
   published=published AND cardinality(array_remove(status_page_ids,OLD.id))>0,updated_at=now()
 WHERE OLD.id=ANY(status_page_ids);
 UPDATE maintenance_windows SET status_page_ids=array_remove(status_page_ids,OLD.id) WHERE OLD.id=ANY(status_page_ids);
 RETURN OLD;
END;
$$;
CREATE TRIGGER pages_cleanup AFTER DELETE ON status_pages FOR EACH ROW EXECUTE FUNCTION aegis_page_deleted();

-- Repair stale monitor IDs left by earlier versions without widening routes.
UPDATE notification_channels c SET
 enabled=c.enabled AND EXISTS(SELECT 1 FROM monitors m WHERE m.id=ANY(c.monitor_ids)),
 monitor_ids=ARRAY(SELECT u.id FROM unnest(c.monitor_ids) WITH ORDINALITY u(id,ord) JOIN monitors m ON m.id=u.id ORDER BY u.ord)
WHERE cardinality(c.monitor_ids)>0;
UPDATE status_resources r SET monitor_ids=ARRAY(SELECT u.id FROM unnest(r.monitor_ids) WITH ORDINALITY u(id,ord) JOIN monitors m ON m.id=u.id ORDER BY u.ord);
UPDATE status_pages p SET monitor_ids=ARRAY(SELECT u.id FROM unnest(p.monitor_ids) WITH ORDINALITY u(id,ord) JOIN monitors m ON m.id=u.id ORDER BY u.ord);
UPDATE maintenance_windows w SET monitor_ids=ARRAY(SELECT u.id FROM unnest(w.monitor_ids) WITH ORDINALITY u(id,ord) JOIN monitors m ON m.id=u.id ORDER BY u.ord);
