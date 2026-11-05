package com.lagradost.cloudstream3.engine

import android.content.Context
import com.lagradost.cloudstream3.APIHolder
import com.lagradost.cloudstream3.plugins.PluginData
import com.lagradost.cloudstream3.plugins.PluginManager
import dalvik.system.DexFile
import dalvik.system.PathClassLoader
import kotlinx.coroutines.runBlocking
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.security.MessageDigest
import java.util.UUID

enum class EngineStatus(val wire: String) { INITIALIZING("initializing"), READY("ready"), FAILED("failed") }

/** One guest session: its epoch and token live only in memory and end with the next bootstrap. */
class EngineState(val token: String, val epoch: String = UUID.randomUUID().toString()) {
    @Volatile var status = EngineStatus.INITIALIZING
    @Volatile var loadedPlugins = 0
    @Volatile var providers = 0
    @Volatile var registrySha256 = ""
    @Volatile var report = JSONObject()
}

object EngineSession {
    const val PORT = 18790
    @Volatile var state: EngineState? = null
        private set
    private var server: HealthServer? = null

    @Synchronized
    fun start(context: Context, token: String, plugins: PluginAssets = PluginAssets.bundled(context), hold: () -> Unit = {}): EngineState {
        val session = EngineState(token)
        state = session
        if (server == null) server = HealthServer(PORT).also { it.start(5000, false) }
        Thread({
            hold()
            load(context.applicationContext, session, plugins)
        }, "engine-plugin-load").start()
        return session
    }

    private fun load(context: Context, session: EngineState, plugins: PluginAssets) {
        val rows = JSONArray()
        val failed = runCatching {
            val inventory = plugins.inventory()
            session.registrySha256 = inventory.getString("registry_sha256")
            val expected = inventory.getJSONArray("plugins")
            val directory = File(context.filesDir, "engine-plugins").apply { mkdirs() }
            var ok = 0
            for (index in 0 until expected.length()) {
                val row = loadOne(context, directory, expected.getJSONObject(index), plugins)
                rows.put(row)
                if (row.getBoolean("loaded") && row.getJSONArray("linkage_errors").length() == 0) ok++
            }
            session.loadedPlugins = ok
            ok != expected.length()
        }.getOrElse { cause ->
            rows.put(JSONObject().put("error", cause.toString()))
            true
        }
        session.providers = synchronized(APIHolder.allProviders) { APIHolder.allProviders.size }
        session.report = JSONObject().put("application", context.javaClass.name).put("plugins", rows)
        session.status = if (failed) EngineStatus.FAILED else EngineStatus.READY
    }

    /** The original loader runs on the original bytes; any mismatch or linkage error stays a failure. */
    private fun loadOne(context: Context, directory: File, expected: JSONObject, plugins: PluginAssets): JSONObject {
        val name = expected.getString("internalName")
        val file = File(directory, "$name.cs3")
        if (file.exists()) { file.setWritable(true); file.delete() }
        plugins.open(name).use { input -> file.outputStream().use { input.copyTo(it) } }
        val digest = sha256(file)
        val row = JSONObject().put("internal_name", name).put("sha256", digest).put("expected_sha256", expected.getString("sha256"))
            .put("loaded", false).put("linkage_errors", JSONArray()).put("providers", JSONArray())
        if (digest != expected.getString("sha256")) return row.put("error", "Package bytes differ from the locked inventory")
        val before = providerSet()
        val loaded = runBlocking { PluginManager.loadPlugin(context, file, PluginData(name, null, false, file.absolutePath, PluginManager.PLUGIN_VERSION_NOT_SET)) }
        val providers = JSONArray()
        (providerSet() - before).forEach { api -> providers.put(JSONObject().put("name", api.name).put("class", api.javaClass.name).put("main_url", api.mainUrl)) }
        row.put("loaded", loaded).put("providers", providers).put("read_only", !file.canWrite())
        row.put("plugin_class", PluginManager.plugins[file.absolutePath]?.javaClass?.name ?: JSONObject.NULL)
        if (loaded) linkAll(context, file, row)
        return row
    }

    /** Forces every class in the package to resolve its members against this runtime. */
    private fun linkAll(context: Context, file: File, row: JSONObject) {
        val loader = PathClassLoader(file.absolutePath, context.classLoader)
        val errors = row.getJSONArray("linkage_errors")
        var classes = 0
        var methods = 0
        @Suppress("DEPRECATION")
        DexFile(file).let { dex ->
            try {
                for (name in dex.entries()) {
                    try {
                        val type = Class.forName(name, false, loader)
                        classes++
                        methods += type.declaredMethods.size + type.declaredConstructors.size
                        type.declaredFields
                    } catch (cause: Throwable) {
                        errors.put(JSONObject().put("class", name).put("error", cause.toString()))
                    }
                }
            } finally { dex.close() }
        }
        row.put("classes", classes).put("methods", methods)
    }

    private fun providerSet() = synchronized(APIHolder.allProviders) { APIHolder.allProviders.toSet() }
    private fun sha256(file: File) = MessageDigest.getInstance("SHA-256").digest(file.readBytes()).joinToString("") { "%02x".format(it) }
}

/** Where a session reads its inventory and package bytes; tests substitute corrupted ones. */
interface PluginAssets {
    fun inventory(): JSONObject
    fun open(name: String): java.io.InputStream

    companion object {
        fun bundled(context: Context) = object : PluginAssets {
            override fun inventory() = JSONObject(context.assets.open("engine/inventory.json").bufferedReader().use { it.readText() })
            override fun open(name: String) = context.assets.open("engine/$name.cs3")
        }
    }
}
