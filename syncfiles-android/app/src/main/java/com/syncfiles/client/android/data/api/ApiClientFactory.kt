package com.syncfiles.client.android.data.api

import okhttp3.Interceptor
import okhttp3.OkHttpClient
import okhttp3.Response
import retrofit2.Retrofit
import retrofit2.converter.gson.GsonConverterFactory
import java.util.UUID
import java.util.concurrent.TimeUnit

class SessionInterceptor(private val sessionProvider: () -> String?) : Interceptor {
    override fun intercept(chain: Interceptor.Chain): Response {
        val builder = chain.request().newBuilder()
        sessionProvider()?.let { token ->
            builder.header("Authorization", "Bearer $token")
        }
        return chain.proceed(builder.build())
    }
}

object ApiClientFactory {

    fun create(baseUrl: String, sessionProvider: () -> String?): SyncFilesApi {
        val client = OkHttpClient.Builder()
            .addInterceptor(SessionInterceptor(sessionProvider))
            .connectTimeout(15, TimeUnit.SECONDS)
            .readTimeout(30, TimeUnit.SECONDS)
            .writeTimeout(30, TimeUnit.SECONDS)
            .build()

        val normalizedBase = if (baseUrl.endsWith("/")) baseUrl else "$baseUrl/"

        return Retrofit.Builder()
            .baseUrl(normalizedBase)
            .client(client)
            .addConverterFactory(GsonConverterFactory.create())
            .build()
            .create(SyncFilesApi::class.java)
    }

    fun newIdempotencyKey(): String = UUID.randomUUID().toString()

    fun newRequestId(): String = UUID.randomUUID().toString()
}
