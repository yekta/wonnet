-- Who carries a device's pushes: `apns`, or `fcm` for an Android device, whose `token` is its FCM
-- registration token and `topic` the app's package name.
ALTER TABLE devices ADD COLUMN platform TEXT NOT NULL DEFAULT 'apns';
