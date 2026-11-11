package com.lagradost.cloudstream3.engine

import fi.iki.elonen.NanoHTTPD
import org.json.JSONObject
import java.security.MessageDigest
import java.util.UUID

/** Loopback-only minimal bridge; later cards add the remaining bridge.openapi.json routes. */
class HealthServer(port: Int) : NanoHTTPD("127.0.0.1", port) {
    override fun serve(session: IHTTPSession): Response {
        val state = EngineSession.state ?: return error(Response.Status.UNAUTHORIZED, "UNAUTHORIZED", "No engine session", null)
        val expected = "Bearer ${state.token}".toByteArray()
        val supplied = (session.headers["authorization"] ?: "").toByteArray()
        if (!MessageDigest.isEqual(expected, supplied)) return error(Response.Status.UNAUTHORIZED, "UNAUTHORIZED", "Invalid bearer token", state.epoch)
        if (session.method != Method.GET || session.uri != "/v1/health") return error(Response.Status.NOT_FOUND, "BAD_REQUEST", "Unknown route", state.epoch)
        val request = session.headers["x-request-id"]
        val valid = request != null && runCatching { UUID.fromString(request).toString() == request.lowercase() }.getOrDefault(false)
        if (!valid) return error(Response.Status.BAD_REQUEST, "BAD_REQUEST", "X-Request-Id must be a UUID", state.epoch)
        val data = JSONObject().put("api_version", "1").put("runtime_version", ENGINE_VERSION).put("cloudstream_revision", UPSTREAM_COMMIT)
            .put("registry_sha256", state.registrySha256).put("status", state.status.wire).put("loaded_plugin_count", state.loadedPlugins)
            .put("provider_count", state.providers).put("display_mode", "headless")
        val body = JSONObject().put("request_id", request).put("engine_epoch", state.epoch).put("data", data)
        return newFixedLengthResponse(Response.Status.OK, "application/json", body.toString())
    }

    private fun error(status: Response.Status, code: String, message: String, epoch: String?): Response {
        val detail = JSONObject().put("code", code).put("message", message).put("retryable", false)
        val body = JSONObject().put("engine_epoch", epoch ?: JSONObject.NULL).put("error", detail)
        return newFixedLengthResponse(status, "application/json", body.toString())
    }

    companion object {
        const val ENGINE_VERSION = "t02-1"
        const val UPSTREAM_COMMIT = "a7ac454af8fbd64574c96847a936905855911720"
    }
}
