"""Independent short/stalled/foreign/missing-resource counterexamples for T22."""
import copy
import math
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import t22


def process():
    owned = [dict(pid=i + 10, identity=f"owned-{i}", role=role, alive=True, rss_bytes=1024, cpu_seconds=i)
             for i, role in enumerate(("application", "renderer", "gpu", "network"))]
    support = [dict(pid=i + 20, identity=f"support-{i}", role=role, alive=True, rss_bytes=1024, cpu_seconds=i)
               for i, role in enumerate(("python_observer", "real_host", "real_host_child"))]
    return dict(owned_rss_bytes=4096, detail_rss_bytes=4096, processes=owned, support_processes=support)


def native():
    return dict(visible=True, focused=True, minimized=False, webkit_pids=dict(renderer=11, gpu=12, network=13))


def startups():
    rows, sessions = [], []
    for index in range(5):
        rows.append(dict(pid=100 + index, identity=f"launch-{index}", host_ready=True, nonblank=True,
            framework_overlay=False, errors=[], native=native(), ready_seconds=.8, health_seconds=1.2,
            owned_rss_bytes=4096, utc_started=f"2026-11-06T00:0{index}:00+00:00", utc_finished=f"2026-11-06T00:0{index}:01+00:00",
            responses=[dict(milliseconds=i / 10, valid=True, command="get_settings") for i in range(30)],
            process_sample=process(), gpu=dict(counters=[{"Device Utilization %": 1}])))
        sessions.append(dict(pid=100 + index, identity=f"launch-{index}", exit=0, lingering=[], forced_cleanup=[],
            wake_guard=dict(exit=0), peak_combined_rss_bytes=4096))
    return rows, sessions


def playback():
    rows = []
    for index in range(121):
        sample = dict(position=1 + index * 5, frames=1 + index * 150, width=1920, height=1080,
            paused=False, media_error=None, audio_state="running", audio_rms=.06, audio_peak_hz=440,
            session_bound=True, video_count=1)
        value = dict(sample=sample, session_id="current-session", actual_gateway=True, playback_rate=1,
            ui_position=sample["position"], visibility="visible", native=native(), errors=[], dropped_frames=0)
        rows.append(dict(host_monotonic=100 + index * 5, value=value, gpu=dict(counters=[{"Device Utilization %": 1}]), process=process()))
    return rows


def stream():
    vector = dict(seconds=180.01, bytes=450_000_000, target_bps=20_000_000, payload_checked=True,
        active_requests=0, live_sessions=0, ports_released=True,
        samples=[dict(seconds=i + .01, bytes=i * 2_500_000) for i in range(181)])
    samples = [dict(monotonic=i, rss_bytes=10 * 1024 ** 2, cpu_seconds=i / 10, identity="stream-process",
                    gpu=dict(counters=[{"Device Utilization %": 1}])) for i in range(180)]
    return vector, samples


class EvidenceTests(unittest.TestCase):
    def test_literal_relative_report_arguments_are_root_relative_for_save(self):
        report = Path("evidence/acceptance/T22-startup-ui.json")
        junit = Path("evidence/acceptance/T22-startup-ui.xml")
        run = t22.Acceptance("startup-ui", report, junit)
        self.addCleanup(run.folder.rmdir)
        self.assertEqual(run.report_path.relative_to(t22.ROOT), report)
        self.assertEqual(run.junit_path.relative_to(t22.ROOT), junit)

    def test_empty_suite_cannot_pass_via_source_stability_only(self):
        with tempfile.TemporaryDirectory(prefix="t22-oracle-", dir=t22.ROOT / "evidence") as temporary:
            run = t22.Acceptance.__new__(t22.Acceptance)
            run.suite, run.source, run.started = "startup-ui", "source", "2026-11-06T00:00:00+00:00"
            run.folder = Path(temporary)
            run.report_path, run.junit_path = run.folder / "empty.json", run.folder / "empty.xml"
            run.cases, run.commands, run.observed = [], [], {}
            with mock.patch.object(t22.workflow, "snapshot", return_value="source"), mock.patch.object(t22.workflow, "protected_hash", return_value="protected"):
                self.assertEqual(run.save(), 1)
            self.assertTrue(all(row["result"] == "FAIL" for row in t22.read_json(run.report_path)["summaries"]))

    def test_percentile_uses_raw_nonempty_finite_values(self):
        self.assertEqual(t22.p95(list(range(1, 101))), 95)
        for values in ([], [math.nan], [math.inf], [True], [-1]):
            with self.subTest(values=values), self.assertRaises(ValueError):
                t22.p95(values)

    def test_five_actual_cold_launches_and_clean_process_receipts_pass(self):
        self.assertEqual(t22.check_startups(*startups())["launches"], 5)

    def test_missing_launch_responses_foreign_identity_or_cleanup_fail(self):
        for kind in ("launch", "response", "identity", "repeat", "forced", "error", "rss", "cpu", "rust-child"):
            rows, sessions = startups()
            if kind == "launch": rows.pop()
            elif kind == "response": rows[0]["responses"].pop()
            elif kind == "identity": sessions[0]["identity"] = "another"
            elif kind == "repeat":
                rows[1]["pid"], rows[1]["identity"] = rows[0]["pid"], rows[0]["identity"]
                sessions[1]["pid"], sessions[1]["identity"] = rows[0]["pid"], rows[0]["identity"]
            elif kind == "forced": sessions[0]["forced_cleanup"] = [100]
            elif kind == "error": rows[0]["errors"] = ["native failure"]
            elif kind == "rss": rows[0]["owned_rss_bytes"] = 0
            elif kind == "cpu": rows[0]["process_sample"]["processes"][0]["cpu_seconds"] = None
            else: rows[0]["process_sample"]["support_processes"].pop()
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                t22.check_startups(rows, sessions)

    def test_response_targets_are_observations_with_reliability_kept(self):
        rows, sessions = startups()
        for row in rows:
            row["ready_seconds"] = 2
            row["health_seconds"] = 65
            row["responses"][0]["milliseconds"] = 200
        self.assertEqual(t22.check_startups(rows, sessions)["bridge_samples"], 150)

    def test_minimum_window_scales_ui_and_metadata_inventory_required(self):
        value = dict(responses=[dict(milliseconds=i) for i in range(10)],
            metadata_responses=[dict(milliseconds=i, command="get_details", valid=True) for i in range(30)],
            layouts=[dict(zoom=zoom, controls=5, official_controls=4, play_pause=True, width=960 / zoom, height=620 / zoom, no_overflow=True,
                native=dict(physical_width=1920, physical_height=1280, scale_factor=2, top_content_inset=20)) for zoom in (1, 1.25, 1.5, 2)],
            fullscreen=True, restored=True)
        t22.check_ui(value)
        for field in ("responses", "metadata_responses", "layouts"):
            changed = copy.deepcopy(value); changed[field].pop()
            with self.subTest(field=field), self.assertRaises(ValueError):
                t22.check_ui(changed)

    def test_full_ten_minute_current_gateway_pcm_and_frames_pass(self):
        self.assertEqual(t22.check_endurance(playback())["seconds"], 600)

    def test_short_missing_stalled_old_frame_audio_and_session_fail(self):
        for kind in ("short", "gap", "frame", "clock", "session", "audio", "gateway", "cpu", "gpu", "ui"):
            rows = playback()
            if kind == "short": rows.pop()
            elif kind == "gap": rows[10]["host_monotonic"] += 20
            elif kind == "frame": rows[10]["value"]["sample"]["frames"] = 0
            elif kind == "clock": rows[10]["value"]["sample"]["position"] = 0
            elif kind == "session": rows[10]["value"]["session_id"] = "old-session"
            elif kind == "audio": rows[10]["value"]["sample"]["audio_peak_hz"] = 880
            elif kind == "gateway": rows[10]["value"]["actual_gateway"] = False
            elif kind == "cpu": rows[10]["process"]["processes"][0]["cpu_seconds"] = None
            elif kind == "gpu": rows[10]["gpu"]["counters"] = []
            else: rows[10]["value"]["ui_position"] = 0
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                t22.check_endurance(rows)

    def test_full_stream_samples_bytes_and_cleanup_pass(self):
        self.assertGreater(t22.check_stream(*stream())["mbps"], 19.99)

    def test_short_stream_wrong_load_omitted_resources_and_leak_fail(self):
        for kind in ("seconds", "bytes", "target_bps", "active_requests", "live_sessions", "ports_released", "cpu", "sample", "identity", "leak", "gap"):
            vector, samples = stream()
            if kind in ("seconds", "bytes", "target_bps"): vector[kind] = 1
            elif kind in ("active_requests", "live_sessions"): vector[kind] = 1
            elif kind == "ports_released": vector[kind] = False
            elif kind == "cpu": samples[0]["cpu_seconds"] = None
            elif kind == "sample": samples = []
            elif kind == "identity": samples[10]["identity"] = "another"
            elif kind == "leak":
                for row in samples[10:]: row["rss_bytes"] = 100 * 1024 ** 2
            else: vector["samples"][10]["seconds"] += 10
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                t22.check_stream(vector, samples)


if __name__ == "__main__":
    unittest.main()
