CREATE TABLE resource_hooks(
    space TEXT PRIMARY KEY,
    package TEXT NOT NULL CHECK(length(package) BETWEEN 1 AND 128),
    checksum TEXT NOT NULL CHECK(length(checksum)=64),
    state_space TEXT
) STRICT, WITHOUT ROWID;

-- Keep only the most recent cumulative observation per generation. Replaying
-- a generation never requires every heartbeat; a monotonic cursor sees later
-- updates and terminal evidence.
CREATE TABLE resource_observations(
    cursor INTEGER PRIMARY KEY AUTOINCREMENT,
    machine TEXT NOT NULL,
    generation TEXT NOT NULL,
    scope_kind TEXT NOT NULL,
    scope_id TEXT NOT NULL,
    UNIQUE(machine,generation),
    FOREIGN KEY(machine,generation) REFERENCES guest_runtime(machine,generation) ON DELETE CASCADE
) STRICT;
CREATE INDEX resource_observations_scope_cursor ON resource_observations(scope_kind,scope_id,cursor);
CREATE TABLE resource_observation_floor(
    singleton INTEGER PRIMARY KEY CHECK(singleton=1),
    cursor INTEGER NOT NULL CHECK(cursor>=0)
) STRICT;
INSERT INTO resource_observation_floor VALUES(1,0);
INSERT INTO resource_observations(machine,generation,scope_kind,scope_id)
SELECT machine,generation,scope_kind,scope_id FROM guest_runtime ORDER BY received_ms,machine,generation;

CREATE TRIGGER resource_observation_new AFTER INSERT ON guest_runtime
BEGIN
    INSERT INTO resource_observations(machine,generation,scope_kind,scope_id)
    VALUES(NEW.machine,NEW.generation,NEW.scope_kind,NEW.scope_id);
END;
CREATE TRIGGER resource_observation_update AFTER UPDATE OF running_ns,state ON guest_runtime
WHEN NEW.running_ns<>OLD.running_ns OR NEW.state<>OLD.state
BEGIN
    DELETE FROM resource_observations WHERE machine=NEW.machine AND generation=NEW.generation;
    INSERT INTO resource_observations(machine,generation,scope_kind,scope_id)
    VALUES(NEW.machine,NEW.generation,NEW.scope_kind,NEW.scope_id);
END;
CREATE TRIGGER resource_observation_retire BEFORE DELETE ON guest_runtime
BEGIN
    UPDATE resource_observation_floor SET cursor=max(cursor,COALESCE(
        (SELECT cursor FROM resource_observations WHERE machine=OLD.machine AND generation=OLD.generation),0
    )) WHERE singleton=1;
END;
