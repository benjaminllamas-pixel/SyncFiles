package com.syncfiles.client.android.data.local

import android.content.Context
import android.content.SharedPreferences

data class SyncStatus(
    val state: String,
    val message: String?,
    val updatedAt: Long
) {
    companion object {
        const val STATE_IDLE = "idle"
        const val STATE_IN_PROGRESS = "in_progress"
        const val STATE_UP_TO_DATE = "up_to_date"
        const val STATE_ERROR = "error"
        const val STATE_PAUSED = "paused"
    }
}

class SyncStatusStore(context: Context) {

    private val prefs: SharedPreferences =
        context.getSharedPreferences("syncfiles_status", Context.MODE_PRIVATE)

    fun get(): SyncStatus {
        return SyncStatus(
            state = prefs.getString(KEY_STATE, SyncStatus.STATE_IDLE) ?: SyncStatus.STATE_IDLE,
            message = prefs.getString(KEY_MESSAGE, null),
            updatedAt = prefs.getLong(KEY_UPDATED_AT, 0L)
        )
    }

    fun set(state: String, message: String? = null) {
        prefs.edit()
            .putString(KEY_STATE, state)
            .putString(KEY_MESSAGE, message)
            .putLong(KEY_UPDATED_AT, System.currentTimeMillis())
            .apply()
    }

    companion object {
        private const val KEY_STATE = "state"
        private const val KEY_MESSAGE = "message"
        private const val KEY_UPDATED_AT = "updated_at"
    }
}
