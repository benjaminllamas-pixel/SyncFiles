package com.syncfiles.client.android.data.util

import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * Funciones puras para sugerir, sanitizar y deduplicar nombres de archivo
 * de shares de texto (sin dependencias de Android — testables en JVM).
 */
object ShareNaming {

    private val URL_REGEX = Regex("https?://\\S+")
    private const val MAX_NAME_LENGTH = 100
    private const val MAX_DEDUPE_ATTEMPTS = 50

    /**
     * Sugiere un nombre de archivo a partir del contenido compartido:
     * - 1 URL → dominio sin `www.` (p.ej. `github.com`)
     * - >1 URL → `pestanas-yyyyMMdd`
     * - texto puro → primeras ~4 palabras en kebab-case
     * El sufijo `.txt` se agrega aparte con [ensureTxtExtension].
     */
    fun suggestFileName(content: String, now: Date = Date()): String {
        val urls = URL_REGEX.findAll(content).map { it.value.trim() }.toList()
        return when {
            urls.size == 1 -> {
                val host = try {
                    java.net.URI(urls[0]).host?.removePrefix("www.")
                } catch (_: Exception) {
                    null
                }
                if (host.isNullOrBlank()) fallbackName(content) else host
            }
            urls.size > 1 -> {
                val date = SimpleDateFormat("yyyyMMdd", Locale.US).format(now)
                "pestanas-$date"
            }
            else -> fallbackName(content)
        }
    }

    private fun fallbackName(content: String): String {
        val words = content.trim()
            .split(Regex("\\s+"))
            .filter { it.isNotBlank() }
            .take(4)
            .joinToString("-")
            .lowercase()
        if (words.isBlank()) return "compartido"
        return words
    }

    /**
     * Elimina caracteres prohibidos en nombres de archivo, colapsa espacios
     * y puntos, y recorta la longitud. Vacío tras sanitizar → `compartido`.
     */
    fun sanitizeFileName(raw: String): String {
        var name = raw.trim()
        name = name.replace(Regex("[/\\\\:*?\"<>|\\n\\r\\t]"), " ")
        name = name.replace(Regex("\\s+"), " ")
        name = name.replace(Regex("\\.{2,}"), ".")
        name = name.trim()
        if (name.length > MAX_NAME_LENGTH) name = name.take(MAX_NAME_LENGTH).trim()
        name = name.trimEnd('.', ' ')
        return name.ifBlank { "compartido" }
    }

    /**
     * Agrega `.txt` si el nombre no lo tiene ya (evita `nombre.txt.txt`).
     */
    fun ensureTxtExtension(name: String): String {
        return if (name.endsWith(".txt", ignoreCase = true)) name else "$name.txt"
    }

    /**
     * Si `sharedDir/<name>` ya existe, prueba `<base>-1.txt`, `-2.txt`, …
     * hasta [MAX_DEDUPE_ATTEMPTS]. Devuelve el primer nombre libre
     * (o el último intento si ninguno quedó libre).
     */
    fun dedupeFileName(sharedDir: File, name: String): String {
        if (!File(sharedDir, name).exists()) return name
        val dot = name.lastIndexOf('.')
        val base = if (dot > 0) name.substring(0, dot) else name
        val ext = if (dot > 0) name.substring(dot) else ""
        for (i in 1..MAX_DEDUPE_ATTEMPTS) {
            val candidate = "$base-$i$ext"
            if (!File(sharedDir, candidate).exists()) return candidate
        }
        return "$base-${MAX_DEDUPE_ATTEMPTS}$ext"
    }
}
