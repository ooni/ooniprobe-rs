package main

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.*
import kotlinx.serialization.json.*

@Serializable
data class ClientOptions(val base_url: String?, val timeout: Long?, val user_agent: String?)

@Serializable
data class Response(
        val status_code: Long,
        val version: String,
        // We place inside of text the headers which we can parse to a string and in
        // bytes those which cannot be parsed as string as a base64 encoding of
        // them.
        val headers_list_text: List<List<String>>?,
        val headers_list_b64_bytes: List<List<String>>?,
        val body_text: String?,
        val body_b64_bytes: String?
)

class OoniProbeClient private constructor(private val clientPtr: Long) {
    companion object {
        private var isLibraryLoaded = false

        private fun loadLibrary() {
            if (!isLibraryLoaded) {
                val libPath = "${System.getProperty("user.dir")}/build/libs/libooniprobe.dylib"
                println("loading library from $libPath")
                System.load(libPath)
                isLibraryLoaded = true
            }
        }
    }

    class Builder {
        private var builderPtr: Long

        init {
            OoniProbeClient.loadLibrary()
            builderPtr = createBuilder()
            if (builderPtr == 0L) {
                throw IllegalStateException("Failed to create builder")
            }
        }

        fun setOptions(options: ClientOptions) = apply {
            builderPtr = setOptions(builderPtr, Json.encodeToString(options))
            if (builderPtr == 0L) {
                throw IllegalStateException("Failed to set options")
            }
        }

        fun build(): OoniProbeClient {
            val clientPtr = buildClient(builderPtr)
            // once the client is built the builder is implicitly destroyed
            builderPtr = 0
            if (clientPtr == 0L) {
                throw IllegalStateException("Failed to build client")
            }
            return OoniProbeClient(clientPtr)
        }

        private external fun createBuilder(): Long
        private external fun setOptions(builderPtr: Long, optionsJson: String): Long
        private external fun buildClient(builderPtr: Long): Long
    }

    class Request
    constructor(
            private val clientPtr: Long,
            private var requestBuilderPtr: Long,
    ) {
        suspend fun send(): Response {
            val response = withContext(Dispatchers.IO) { execute(clientPtr, requestBuilderPtr) }
            return Json.decodeFromString<Response>(response)
        }

        fun addHeader(name: String, value: String) = apply {
            requestBuilderPtr = addHeader(requestBuilderPtr, name, value)
        }

        private external fun addHeader(requestBuilderPtr: Long, name: String, value: String): Long
        private external fun execute(clientPtr: Long, requestBuilderPtr: Long): String
    }

    fun request(method: String, url: String): Request {
        val requestBuilderPtr = request(clientPtr, method, url)
        return Request(clientPtr, requestBuilderPtr)
    }

    suspend fun get(url: String): Response {
        val request = request("GET", url)
        return request.send()
    }

    private external fun request(clientPtr: Long, method: String, url: String): Long
    private external fun destroyRequestBuilder(requestBuilderPtr: Long)
    private external fun destroyClient(clientPtr: Long)

    protected fun finalize() {
        if (clientPtr != 0L) {
            destroyClient(clientPtr)
        }
    }
}

class OoniException : Exception {
    constructor(message: String) : super(message)
    constructor(message: String, cause: Throwable) : super(message, cause)
}
