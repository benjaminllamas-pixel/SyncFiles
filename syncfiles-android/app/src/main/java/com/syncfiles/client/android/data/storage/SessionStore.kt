package com.syncfiles.client.android.data.storage

import android.content.Context
import android.content.SharedPreferences
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey
import java.util.UUID

data class StoredSession(
    val sessionId: String,
    val userId: String,
    val deviceId: String,
    val expiresAt: Long
)

data class ServerConfig(
    val baseUrl: String
)

class SessionStore(context: Context) {

    private val prefs: SharedPreferences by lazy {
        val masterKey = MasterKey.Builder(context)
            .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
            .build()

        EncryptedSharedPreferences.create(
            context,
            "syncfiles_secure_prefs",
            masterKey,
            EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
            EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM
        )
    }

    private fun getOrCreateDeviceId(): String {
        val existing = prefs.getString(KEY_DEVICE_ID, null)
        if (existing != null) return existing

        val deviceId = "android-" + UUID.randomUUID().toString()
        prefs.edit().putString(KEY_DEVICE_ID, deviceId).apply()
        return deviceId
    }

    val deviceId: String
        get() = getOrCreateDeviceId()

    fun saveServerConfig(config: ServerConfig) {
        prefs.edit().putString(KEY_SERVER_URL, config.baseUrl).apply()
    }

    fun getServerConfig(): ServerConfig? {
        val url = prefs.getString(KEY_SERVER_URL, null) ?: return null
        return ServerConfig(url)
    }

    fun saveSession(session: StoredSession) {
        prefs.edit()
            .putString(KEY_SESSION_ID, session.sessionId)
            .putString(KEY_USER_ID, session.userId)
            .putString(KEY_DEVICE_ID, session.deviceId)
            .putLong(KEY_EXPIRES_AT, session.expiresAt)
            .apply()
    }

    fun getActiveSession(): StoredSession? {
        val sessionId = prefs.getString(KEY_SESSION_ID, null) ?: return null
        val userId = prefs.getString(KEY_USER_ID, null) ?: return null
        val deviceId = prefs.getString(KEY_DEVICE_ID, null) ?: return null
        val expiresAt = prefs.getLong(KEY_EXPIRES_AT, 0L)

        if (expiresAt > 0 && System.currentTimeMillis() >= expiresAt) {
            clearSession()
            return null
        }

        return StoredSession(sessionId, userId, deviceId, expiresAt)
    }

    fun clearSession() {
        prefs.edit()
            .remove(KEY_SESSION_ID)
            .remove(KEY_USER_ID)
            .remove(KEY_EXPIRES_AT)
            .apply()
    }

    fun saveSyncRoot(uriString: String, displayName: String?) {
        prefs.edit()
            .putString(KEY_SYNC_ROOT_URI, uriString)
            .putString(KEY_SYNC_ROOT_NAME, displayName)
            .apply()
    }

    fun getSyncRootUri(): String? = prefs.getString(KEY_SYNC_ROOT_URI, null)

    fun getSyncRootName(): String? = prefs.getString(KEY_SYNC_ROOT_NAME, null)

    fun clearSyncRoot() {
        prefs.edit()
            .remove(KEY_SYNC_ROOT_URI)
            .remove(KEY_SYNC_ROOT_NAME)
            .apply()
    }

    var lastServerSeq: Long
        get() = prefs.getLong(KEY_LAST_SERVER_SEQ, 0L)
        set(value) { prefs.edit().putLong(KEY_LAST_SERVER_SEQ, value).apply() }

    companion object {
        private const val KEY_DEVICE_ID = "device_id"
        private const val KEY_SESSION_ID = "session_id"
        private const val KEY_USER_ID = "user_id"
        private const val KEY_EXPIRES_AT = "expires_at"
        private const val KEY_SERVER_URL = "server_url"
        private const val KEY_LAST_SERVER_SEQ = "last_server_seq"
        private const val KEY_SYNC_ROOT_URI = "sync_root_uri"
        private const val KEY_SYNC_ROOT_NAME = "sync_root_name"
    }
}
