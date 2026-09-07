package com.syncfiles.client.android.data.local

import android.content.Context
import android.database.sqlite.SQLiteDatabase
import android.util.Log

data class LocalFile(
    val fileId: String,
    val relativePath: String,
    val pathHash: String,
    val checksum: String,
    val sizeBytes: Long,
    val modifiedAt: Long,
    val status: String,
    val lastSyncVersion: Long
)

class LocalFileSyncStore(context: Context) {

    private val helper = DbHelper(context)
    private val db: SQLiteDatabase
        get() = helper.writableDatabase

    companion object {
        private const val TAG = "LocalFileSyncStore"
    }

    fun upsert(file: LocalFile) {
        val sql = "INSERT OR REPLACE INTO files (file_id, relative_path, path_hash, checksum, size_bytes, modified_at, status, last_sync_version) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
        db.compileStatement(sql).use { stmt ->
            stmt.bindString(1, file.fileId)
            stmt.bindString(2, file.relativePath)
            stmt.bindString(3, file.pathHash)
            stmt.bindString(4, file.checksum)
            stmt.bindLong(5, file.sizeBytes)
            stmt.bindLong(6, file.modifiedAt)
            stmt.bindString(7, file.status)
            stmt.bindLong(8, file.lastSyncVersion)
            stmt.execute()
        }
    }

    fun findByPathHash(pathHash: String): LocalFile? {
        val sql = "SELECT file_id, relative_path, path_hash, checksum, size_bytes, modified_at, status, last_sync_version FROM files WHERE path_hash = ?"
        return db.query("files", null, "path_hash = ?", arrayOf(pathHash), null, null, null).use { cursor ->
            if (!cursor.moveToNext()) return null
            LocalFile(
                fileId = cursor.getString(0),
                relativePath = cursor.getString(1),
                pathHash = cursor.getString(2),
                checksum = cursor.getString(3),
                sizeBytes = cursor.getLong(4),
                modifiedAt = cursor.getLong(5),
                status = cursor.getString(6),
                lastSyncVersion = cursor.getLong(7)
            )
        }
    }

    fun listAll(): List<LocalFile> {
        return db.query("files", null, null, null, null, null, "relative_path ASC").use { cursor ->
            buildList {
                while (cursor.moveToNext()) {
                    add(
                        LocalFile(
                            fileId = cursor.getString(0),
                            relativePath = cursor.getString(1),
                            pathHash = cursor.getString(2),
                            checksum = cursor.getString(3),
                            sizeBytes = cursor.getLong(4),
                            modifiedAt = cursor.getLong(5),
                            status = cursor.getString(6),
                            lastSyncVersion = cursor.getLong(7)
                        )
                    )
                }
            }
        }
    }

    class DbHelper(context: Context) :
        android.database.sqlite.SQLiteOpenHelper(context, "syncfiles_files.db", null, 1) {

        override fun onCreate(db: SQLiteDatabase) {
            db.execSQL("CREATE TABLE IF NOT EXISTS files (" +
                "file_id TEXT PRIMARY KEY, " +
                "relative_path TEXT NOT NULL, " +
                "path_hash TEXT NOT NULL, " +
                "checksum TEXT NOT NULL, " +
                "size_bytes INTEGER NOT NULL, " +
                "modified_at INTEGER NOT NULL, " +
                "status TEXT NOT NULL DEFAULT 'synced', " +
                "last_sync_version INTEGER NOT NULL DEFAULT 0" +
                ")")
            db.execSQL("CREATE INDEX IF NOT EXISTS idx_files_path ON files(path_hash)")
        }

        override fun onUpgrade(db: SQLiteDatabase, oldVersion: Int, newVersion: Int) {}
    }
}