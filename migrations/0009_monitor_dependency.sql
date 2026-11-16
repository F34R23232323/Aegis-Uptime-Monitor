ALTER TABLE monitors ADD COLUMN depends_on uuid REFERENCES monitors(id) ON DELETE SET NULL;
CREATE INDEX monitors_dependency ON monitors(depends_on) WHERE depends_on IS NOT NULL;
