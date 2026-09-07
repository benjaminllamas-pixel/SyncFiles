package com.syncfiles.client.android.data.local

import android.content.Context
import android.net.Uri
import android.os.FileObserver
import android.provider.OpenableColumns
import androidx.documentfile.provider.DocumentFile
import com.syncfiles.client.android.data.util.Hashing
import java.io.File
import java.util.concurrent.atomic.AtomicBoolean

class FileWatcher(
    private val rootDir: File,
    private val onEvent: (String) -> Unit
) : FileObserver(rootDir.absolutePath, MASK) {

    companion object {
        private const val MASK =
            MOVED_TO or MOVED_FROM or CREATE or DELETE or MODIFY or CLOSE_WRITE or CLOSE_NOWRITE
    }

    private val running = AtomicBoolean(false)

    override fun startWatching() {
        if (running.compareAndSet(false, true)) super.startWatching()
    }

    override fun stopWatching() {
        if (running.compareAndSet(true, false)) super.stopWatching()
    }

    override fun onEvent(event: Int, path: String?) {
        if (path == null) return
        val relative = path.trimStart('/')
        onEvent(relative)
    }

    fun computeHash(file: File): String? =
        if (file.exists() && file.isFile) Hashing.sha256Hex(file.readBytes()) else null

    fun computePathHash(relativePath: String): String = Hashing.sha256Hex(relativePath)

    fun queryDisplayName(context: Context, uri: Uri): String? {
        val cursor = context.contentResolver.query(uri, null, null, null, null) ?: return null
        return cursor.use {
            val idx = it.columnNames.indexOf(OpenableColumns.DISPLAY_NAME)
            if (it.moveToFirst() && idx >= 0) it.getString(idx) else null
        }
    }
}
