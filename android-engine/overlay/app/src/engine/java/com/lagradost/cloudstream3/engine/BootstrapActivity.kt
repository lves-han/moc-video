package com.lagradost.cloudstream3.engine

import android.app.Activity
import android.content.Intent
import android.os.Bundle

/** Accepts only the session token and removes it from the Intent immediately (SPEC 5.4). */
class BootstrapActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        bootstrap(intent)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        bootstrap(intent)
    }

    private fun bootstrap(source: Intent) {
        val token = source.getStringExtra(EXTRA_TOKEN)
        source.replaceExtras(Bundle())
        setIntent(Intent(source))
        if (token != null && TOKEN.matches(token)) EngineSession.start(applicationContext, token)
    }

    companion object {
        const val EXTRA_TOKEN = "token"
        private val TOKEN = Regex("[0-9a-f]{64}")
    }
}
