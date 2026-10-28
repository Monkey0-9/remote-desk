# Remote-Desk Android ProGuard / R8 Hardening Rules (OWASP MASVS-RESILIENCE)

# Preserve WebRTC JNI native bindings
-keep class org.webrtc.** { *; }
-dontwarn org.webrtc.**

# Preserve UniFFI Rust core bridge
-keep class uniffi.** { *; }
-keep class org.remotedesk.core.** { *; }

# Obfuscate internal business logic and gesture mapping
-repackageclasses 'org.remotedesk.internal'
-allowaccessmodification

# Security: Strip all logging statements in release builds to satisfy MASVS-STORAGE-2
-assumenosideeffects class android.util.Log {
    public static boolean isLoggable(java.lang.String, int);
    public static int v(...);
    public static int d(...);
    public static int i(...);
}

# Preserve Compose runtime
-keep class androidx.compose.** { *; }
