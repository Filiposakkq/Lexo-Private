-- The name of a computer (chosen by the user, optional) and the version of the application, for the lists of devices and for the admin.
ALTER TABLE activations ADD COLUMN name TEXT NOT NULL DEFAULT '';
ALTER TABLE activations ADD COLUMN app_version TEXT NOT NULL DEFAULT '';
