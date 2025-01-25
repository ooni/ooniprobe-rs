import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

sealed class ClientOption {
    data class BaseUrl(val url: String) : ClientOption()
    data class Timeout(val seconds: Long) : ClientOption()
    data class UserAgent(val agent: String) : ClientOption()

    internal fun toJson(): String = when (this) {
        is BaseUrl -> """{"BaseUrl":"$url"}"""
        is Timeout -> """{"Timeout":$seconds}"""
        is UserAgent -> """{"UserAgent":"$agent"}"""
    }
}

class OoniProbeClient private constructor(private val clientPtr: Long) {
    companion object {
        init {
            System.loadLibrary("ooniprobe_services")
        }
    }

    class Builder {
        private var builderPtr: Long = createBuilder()

        init {
            if (builderPtr == 0L) {
                throw IllegalStateException("Failed to create builder")
            }
        }

        fun configure(block: Builder.() -> Unit): Builder = apply(block)

        fun baseUrl(url: String) = apply {
            setOption(ClientOption.BaseUrl(url))
        }

        fun timeout(seconds: Long) = apply {
            setOption(ClientOption.Timeout(seconds))
        }

        fun userAgent(agent: String) = apply {
            setOption(ClientOption.UserAgent(agent))
        }

        private fun setOption(option: ClientOption) {
            builderPtr = setOption(builderPtr, option.toJson())
            if (builderPtr == 0L) {
                throw IllegalStateException("Failed to set option: ${option::class.simpleName}")
            }
        }

        fun build(): OoniClient {
            val clientPtr = build(builderPtr)
            if (clientPtr == 0L) {
                throw IllegalStateException("Failed to build client")
            }
            destroyBuilder(builderPtr)
            return OoniClient(clientPtr)
        }

        private external fun createBuilder(): Long
        private external fun setOption(builderPtr: Long, optionJson: String): Long
        private external fun build(builderPtr: Long): Long
        private external fun destroyBuilder(builderPtr: Long)

        protected fun finalize() {
            if (builderPtr != 0L) {
                destroyBuilder(builderPtr)
            }
        }
    }

    /**
     * Performs a GET request to the specified URL.
     *
     * @param url The URL to send the request to
     * @return ByteArray containing the response body
     * @throws OoniException if the request fails
     */
    suspend fun get(url: String): ByteArray = withContext(Dispatchers.IO) {
        try {
            get(clientPtr, url) ?: throw OoniException("Request failed")
        } catch (e: Exception) {
            throw OoniException("Request failed", e)
        }
    }

    private external fun get(clientPtr: Long, url: String): ByteArray?
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
