# The core is called through JNI, which the shrinker can't see.
-keep class app.wonnet.core.Native { *; }
-keep, includedescriptorclasses class org.rustls.platformverifier.** { *; }
