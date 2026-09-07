package com.syncfiles.client.android

import android.app.Application
import com.syncfiles.client.android.data.api.ApiClientFactory
import com.syncfiles.client.android.data.storage.SessionStore

class SyncFilesApplication : Application() {

    companion object {
        @Volatile
        private var instance: SyncFilesApplication? = null

        fun app(): SyncFilesApplication =
            checkNotNull(instance) { "SyncFilesApplication not initialized" }
    }

    val sessionStore: SessionStore by lazy { SessionStore(this) }

    override fun onCreate() {
        super.onCreate()
        instance = this
    }
}