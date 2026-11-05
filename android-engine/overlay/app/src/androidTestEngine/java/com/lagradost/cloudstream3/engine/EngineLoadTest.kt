package com.lagradost.cloudstream3.engine

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.lagradost.cloudstream3.CloudStreamApp
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.io.ByteArrayInputStream
import java.io.File

/** Runs in the installed APK on real ART; the acceptance script validates the written report. */
@RunWith(AndroidJUnit4::class)
class EngineLoadTest {
    private val context get() = InstrumentationRegistry.getInstrumentation().targetContext
    private val token = "a".repeat(64)

    private fun await(state: EngineState) {
        val end = System.currentTimeMillis() + 180_000
        while (state.status == EngineStatus.INITIALIZING) {
            assertTrue("Plugin load timed out", System.currentTimeMillis() < end)
            Thread.sleep(50)
        }
    }

    @Test
    fun allSevenOriginalPackagesLoadThroughTheOriginalLoader() {
        val state = EngineSession.start(context, token)
        await(state)
        val report = JSONObject(state.report.toString()).put("status", state.status.wire).put("provider_count", state.providers)
            .put("loaded_plugin_count", state.loadedPlugins).put("application_is_upstream", context.applicationContext is CloudStreamApp).put("registry_sha256", state.registrySha256)
        File(context.filesDir, "t02-load-report.json").writeText(report.toString())
        assertEquals(state.report.toString(), EngineStatus.READY, state.status)
    }

    @Test
    fun alteredPackageBytesFailTheSession() {
        val real = PluginAssets.bundled(context)
        val state = EngineSession.start(context, token, object : PluginAssets {
            override fun inventory() = real.inventory()
            override fun open(name: String) = ByteArrayInputStream(real.open(name).use { it.readBytes() } + byteArrayOf(0))
        })
        await(state)
        assertEquals(EngineStatus.FAILED, state.status)
        assertEquals(0, state.loadedPlugins)
    }
}
