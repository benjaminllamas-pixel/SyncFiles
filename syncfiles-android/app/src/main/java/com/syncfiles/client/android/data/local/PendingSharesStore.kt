package com.syncfiles.client.android.data.local

import android.content.ContentValues
import android.content.Context
import android.database.sqlite.SQLiteDatabase

data class PendingShare(
    val id: Long,
    val content: String,
    val createdAt: Long
)

/**
 * Almacena shares de texto recibidos sin sesión/carpeta activa, para
 * procesarlos cuando el usuario complete el flujo. Cap FIFO de 20 entradas.
 */
class PendingSharesStore(context: Context) {

    private val helper = PendingSharesDbHelper(context)
    private val db: SQLiteDatabase
        get() = helper.writableDatabase

    companion object {
        private const val TABLE = "pending_shares"
        private const val MAX_ENTRIES = 20
    }

    /** Inserta y devuelve el id de la fila creada. */
    fun insert(content: String): Long {
        val now = System.currentTimeMillis()
        val values = ContentValues().apply {
            put("content", content)
            put("created_at", now)
        }
        val rowId = db.insert(TABLE, null, values)
        trimToCap()
        return rowId
    }

    fun oldest(): PendingShare? {
        return db.query(
            TABLE,
            null,
            null,
            null,
            null,
            null,
            "created_at ASC",
            "1"
        ).use { cursor ->
            if (!cursor.moveToFirst()) return null
            val idIndex = cursor.getColumnIndex("id")
            val contentIndex = cursor.getColumnIndex("content")
            val createdIndex = cursor.getColumnIndex("created_at")
            PendingShare(
                id = cursor.getLong(idIndex),
                content = cursor.getString(contentIndex),
                createdAt = cursor.getLong(createdIndex)
            )
        }
    }

    fun delete(id: Long) {
        db.compileStatement("DELETE FROM $TABLE WHERE id = ?").use { stmt ->
            stmt.bindLong(1, id)
            stmt.executeUpdateDelete()
        }
    }

    fun count(): Int {
        db.compileStatement("SELECT COUNT(*) FROM $TABLE").use { stmt ->
            return stmt.simpleQueryForLong().toInt()
        }
    }

    private fun trimToCap() {
        val total = count()
        if (total <= MAX_ENTRIES) return
        db.compileStatement(
            "DELETE FROM $TABLE WHERE id IN (SELECT id FROM $TABLE ORDER BY created_at ASC LIMIT ?)"
        ).use { stmt ->
            stmt.bindLong(1, (total - MAX_ENTRIES).toLong())
            stmt.execute()
        }
    }
}

class PendingSharesDbHelper(context: Context) :
    android.database.sqlite.SQLiteOpenHelper(context, "syncfiles_pending_shares.db", null, 1) {

    override fun onCreate(db: SQLiteDatabase) {
        db.execSQL(
            "CREATE TABLE IF NOT EXISTS pending_shares (" +
                "id INTEGER PRIMARY KEY AUTOINCREMENT, " +
                "content TEXT NOT NULL, " +
                "created_at INTEGER NOT NULL" +
                ")"
        )
    }

    override fun onUpgrade(db: SQLiteDatabase, oldVersion: Int, newVersion: Int) {}
}
