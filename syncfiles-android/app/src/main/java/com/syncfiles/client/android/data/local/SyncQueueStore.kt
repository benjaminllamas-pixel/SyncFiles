package com.syncfiles.client.android.data.local

import android.content.Context
import android.database.sqlite.SQLiteDatabase
import android.database.sqlite.SQLiteStatement
import java.util.UUID

data class QueueEntry(
    val queueId: String,
    val fileId: String,
    val relativePath: String,
    val operation: String,
    val status: String,
    val attempts: Int,
    val idempotencyKey: String,
    val payloadJson: String?,
    val lastError: String?,
    val createdAt: Long,
    val updatedAt: Long
)

class SyncQueueStore(context: Context) {

    private val helper = SyncQueueDbHelper(context)
    private val db: SQLiteDatabase
        get() = helper.writableDatabase

    companion object {
        private const val TABLE = "sync_queue"
    }

    fun enqueue(
        fileId: String,
        relativePath: String,
        operation: String,
        idempotencyKey: String = UUID.randomUUID().toString(),
        payloadJson: String? = null
    ) {
        val now = System.currentTimeMillis()
        val sql = "INSERT OR IGNORE INTO $TABLE (queue_id, file_id, relative_path, operation, status, attempts, idempotency_key, payload_json, created_at, updated_at) VALUES (?, ?, ?, ?, 'queued', 0, ?, ?, ?, ?)"
        db.compileStatement(sql).also { stmt ->
            stmt.bindString(1, UUID.randomUUID().toString())
            stmt.bindString(2, fileId)
            stmt.bindString(3, relativePath)
            stmt.bindString(4, operation)
            stmt.bindString(5, idempotencyKey)
            if (payloadJson == null) stmt.bindNull(6) else stmt.bindString(6, payloadJson)
            stmt.bindLong(7, now)
            stmt.bindLong(8, now)
            stmt.execute()
            stmt.close()
        }
    }

    fun listByStatus(status: String): List<QueueEntry> {
        val sql = "SELECT queue_id, file_id, relative_path, operation, status, attempts, idempotency_key, payload_json, last_error, created_at, updated_at FROM $TABLE WHERE status = ?"
        return db.query(
            TABLE,
            null,
            "status = ?",
            arrayOf(status),
            null,
            null,
            "created_at ASC"
        ).use { cursor ->
            buildList {
                while (cursor.moveToNext()) {
                    add(
                        QueueEntry(
                            queueId = cursor.getString(0),
                            fileId = cursor.getString(1),
                            relativePath = cursor.getString(2),
                            operation = cursor.getString(3),
                            status = cursor.getString(4),
                            attempts = cursor.getInt(5),
                            idempotencyKey = cursor.getString(6),
                            payloadJson = cursor.getString(7),
                            lastError = cursor.getString(8),
                            createdAt = cursor.getLong(9),
                            updatedAt = cursor.getLong(10)
                        )
                    )
                }
            }
        }
    }

    fun markDone(queueId: String) {
        updateStatus(queueId, "done", null)
    }

    fun markRetry(queueId: String, error: String) {
        updateStatus(queueId, "retry", error)
    }

    private fun updateStatus(queueId: String, status: String, error: String?) {
        val sql = "UPDATE $TABLE SET status = ?, attempts = attempts + 1, last_error = ?, updated_at = ? WHERE queue_id = ?"
        db.compileStatement(sql).also { stmt ->
            stmt.bindString(1, status)
            if (error == null) stmt.bindNull(2) else stmt.bindString(2, error)
            stmt.bindLong(3, System.currentTimeMillis())
            stmt.bindString(4, queueId)
            stmt.execute()
            stmt.close()
        }
    }

    fun countByStatus(status: String): Int {
        val sql = "SELECT COUNT(*) FROM $TABLE WHERE status = ?"
        return db.compileStatement(sql).use { stmt ->
            stmt.bindString(1, status)
            stmt.simpleQueryForLong().toInt()
        }
    }
}

class SyncQueueDbHelper(context: Context) :
    android.database.sqlite.SQLiteOpenHelper(context, "syncfiles_queue.db", null, 1) {

    override fun onCreate(db: SQLiteDatabase) {
        db.execSQL(
            "CREATE TABLE IF NOT EXISTS sync_queue (" +
                "queue_id TEXT PRIMARY KEY, " +
                "file_id TEXT NOT NULL, " +
                "relative_path TEXT NOT NULL, " +
                "operation TEXT NOT NULL, " +
                "status TEXT NOT NULL DEFAULT 'queued', " +
                "attempts INTEGER NOT NULL DEFAULT 0, " +
                "idempotency_key TEXT NOT NULL, " +
                "payload_json TEXT, " +
                "last_error TEXT, " +
                "created_at INTEGER NOT NULL, " +
                "updated_at INTEGER NOT NULL" +
                ")"
        )
        db.execSQL("CREATE INDEX IF NOT EXISTS idx_queue_status ON sync_queue(status, created_at)")
    }

    override fun onUpgrade(db: SQLiteDatabase, oldVersion: Int, newVersion: Int) {}
}