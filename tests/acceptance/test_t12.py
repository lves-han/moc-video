"""Independent negative controls for the real T12 native evidence oracle."""
import copy
import tempfile
from pathlib import Path
import unittest
import urllib.error
import urllib.request
import json
import queue
from types import SimpleNamespace
from unittest.mock import Mock, patch

import t12


def cycle():
    return dict(index=1, old_tracks_disabled=True, session_changed=True, new_frames=5, actual_gateway=True,
                errors=[], host=dict(live_sessions=1, requests=0, old_http_status=410),
                sample=dict(video_count=1, tracks=1, width=1920, height=1080, media_error=None,
                            audio_state="running", session_bound=True, audio_rms=.1, audio_peak_hz=880, pixel=[0, 0, 254, 255],
                            diagnostics=dict(hls_instances=0, listening=True, retiring=0)))


def stopped():
    return dict(source_removed=True, frames_after_stop=0, host=dict(live_sessions=0, requests=0, source_active=0),
                sample=dict(video_count=1, paused=True, ready_state=0, network_state=0, source_present=False,
                            tracks=0, audio_rms=0, diagnostics=dict(listening=False, hls_instances=0, retiring=0)))


def eos(paused=False):
    before = dict(epoch=7, session_id="current", phase="paused" if paused else "playing", paused=paused, ended=False)
    final = dict(epoch=7, session_id="current", phase="ended", paused=True, ended=True)
    return dict(ended=True, dom_events=1, before=before, final=final,
                events=[dict(trusted=True, state=final.copy())],
                stages=[dict(stage=stage) for stage in (["resume", "seek"] if paused else ["seek"])])


def image_expiry():
    host = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"
    reference = dict(source_id="metadata", source_revision="revision", data="show")
    calls = [dict(command="get_resource_status", request_id="bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
                  response_request_id="bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb", engine_epoch=host,
                  kind="image", target_id="poster", response_kind="image", response_target_id="poster",
                  state="expired", code="SESSION_EXPIRED", started_ms=2, finished_ms=3),
             dict(command="get_details", request_id="cccccccc-cccc-4ccc-8ccc-cccccccccccc",
                  response_request_id="cccccccc-cccc-4ccc-8ccc-cccccccccccc", engine_epoch=host,
                  ref=reference.copy(), response_ref=reference.copy(), code=None, started_ms=4, finished_ms=5)]
    return dict(expired_http_status=410, dto_refreshed=True, decoded=True, owning_ref=reference.copy(),
                refreshed_ref=reference.copy(), image_error="", resource_id="poster", refreshed_resource_id="new-poster", page_epoch=1, page_epoch_after=1,
                expected_host_epoch=host, host_epoch_after=host, calls=calls,
                stages=["before-eviction", "4096-lru-evicted", "expired-http-read", "host-dto-published",
                        "dto-recovery-finished", "refreshed-image-decoded"],
                events=[dict(type="error", trusted=True, original_dto=True, original_url=True, owning_url=True, resource_id="poster",
                             natural_width=0, page_epoch=1, at_ms=1),
                        dict(type="load", trusted=True, original_dto=False, owning_url=True, resource_id="new-poster", natural_width=1,
                             page_epoch=1, at_ms=6)],
                before=dict(command_counts=dict(get_resource_status=20, get_details=1)),
                after=dict(command_counts=dict(get_resource_status=21, get_details=2)))


def native_errors():
    host = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"
    outcomes, calls, events = [], [], []
    for index, mode in enumerate((1, 2, 3)):
        code = "MEDIA_UNSUPPORTED" if mode == 3 else "UPSTREAM_FAILED"
        session = f"session-{mode}"
        final = dict(epoch=mode, host_epoch=host, session_id=session, source_id="source", source_revision="revision",
                     episode_ref=dict(source_id="source", source_revision="revision", data="episode-50"),
                     session_path=f"/sessions/{session}/media", current_path=f"/sessions/{session}/media",
                     phase="failed", code=code, pending_requests=0, session_bound=True, assigned_bound=True, stream_kind="progressive",
                     action="" if mode == 3 else "retry",
                     media_error=dict(code=4, message="opaque browser message mentions 401 and 502"),
                     error=dict(details=dict(original_error=dict(details=dict(media_code=4)))))
        outcomes.append(dict(mode=mode, code=code, media_error=4, before=copy.deepcopy(final), opened=copy.deepcopy(final),
                             final=final, call_start=index, event_start=index * 2,
                             repeat=dict(stimulus="native video.load, same session URL after completed query",
                                         before=copy.deepcopy(final), after=copy.deepcopy(final),
                                         started_ms=4, finished_ms=6, event_start=index * 2 + 1,
                                         calls_before=index + 1, calls_after=index + 1, retry_button=mode != 3),
                             requests=[dict(mode=mode, target="/media/a.mp4?episode=50", status={1: 401, 2: 502, 3: 200}[mode],
                                            mime="video/mp4" if mode == 3 else "text/plain")]))
        request_id = f"bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbb{mode}"
        calls.append(dict(mode=mode, command="get_resource_status", request_id=request_id, kind="playback", target_id=session,
                          before=copy.deepcopy(final), started_ms=2, finished_ms=3,
                          response=dict(request_id=request_id, engine_epoch=host, data=dict(kind="playback", target_id=session,
                                        state="ready" if mode == 3 else "failed", error=None if mode == 3 else dict(code=code)))))
        events.append(dict(target="video", mode=mode, trusted=True, epoch=mode, session_id=session, at_ms=1, media_error=dict(code=4)))
        events.append(dict(target="video", mode=mode, trusted=True, epoch=mode, session_id=session, at_ms=5, media_error=dict(code=4)))
    outcomes.extend([dict(mode=4, call_start=3, event_start=6), dict(mode="unauthorized-query")])
    return dict(outcomes=outcomes, calls=calls, events=events, track_error="UPSTREAM_FAILED", reconnects=1)


class NativeOracleTests(unittest.TestCase):
    def test_native_error_matrix_accepts_http_diagnosis_and_real_progressive_codec(self):
        t12.validate_errors(native_errors())
        # A failed source selection may clear currentSrc; the assigned URL and
        # actual upstream request still identify the current loading attempt.
        row = native_errors(); row["outcomes"][2]["final"]["current_path"] = None
        t12.validate_errors(row)

    def test_native_error_matrix_rejects_hls_path_wrong_status_mime_and_codec(self):
        edits = [("stream_kind", "hls"), ("code", "UPSTREAM_FAILED"), ("assigned_bound", False),
                 ("current_path", "/sessions/old/media"), ("pending_requests", 1)]
        for field, value in edits:
            row = native_errors(); row["outcomes"][2]["final"][field] = value
            with self.subTest(final=field), self.assertRaises(ValueError):
                t12.validate_errors(row)
        for field, value in [("target", "/media/index.m3u8"), ("status", 502), ("mime", "application/vnd.apple.mpegurl")]:
            row = native_errors(); row["outcomes"][2]["requests"][0][field] = value
            with self.subTest(request=field), self.assertRaises(ValueError):
                t12.validate_errors(row)
        for code in (1, 2):
            row = native_errors(); row["outcomes"][2]["final"]["media_error"]["code"] = code
            row["outcomes"][2]["media_error"] = code
            with self.subTest(codec=code), self.assertRaises(ValueError):
                t12.validate_errors(row)

    def test_native_error_matrix_rejects_old_query_identity_and_gateway_classification(self):
        changes = [("request_id", "bad"), ("engine_epoch", "old-host")]
        for field, value in changes:
            row = native_errors(); row["calls"][0]["response"][field] = value
            with self.subTest(response=field), self.assertRaises(ValueError):
                t12.validate_errors(row)
        for field, value in [("target_id", "old"), ("kind", "image"), ("state", "ready"), ("error", dict(code="UNAUTHORIZED"))]:
            row = native_errors(); row["calls"][0]["response"]["data"][field] = value
            with self.subTest(data=field), self.assertRaises(ValueError):
                t12.validate_errors(row)
        for field, value in [("epoch", 0), ("source_revision", "old"), ("session_id", "old"), ("host_epoch", "old")]:
            row = native_errors(); row["calls"][0]["before"][field] = value
            with self.subTest(before=field), self.assertRaises(ValueError):
                t12.validate_errors(row)
        row = native_errors(); row["calls"][0]["finished_ms"] = 3003
        with self.assertRaises(ValueError):
            t12.validate_errors(row)

    def test_native_repeat_requires_completed_query_current_identity_and_preserved_error_action(self):
        row = native_errors(); del row["outcomes"][0]["repeat"]
        with self.assertRaises(ValueError):
            t12.validate_errors(row)
        for field, value in [("code", "MEDIA_UNSUPPORTED"), ("action", ""), ("error", {}),
                             ("epoch", 0), ("session_id", "old"), ("host_epoch", "old"),
                             ("source_revision", "old"), ("assigned_bound", False), ("pending_requests", 1)]:
            for phase in ("before", "after"):
                row = native_errors(); row["outcomes"][0]["repeat"][phase][field] = value
                with self.subTest(phase=phase, field=field), self.assertRaises(ValueError):
                    t12.validate_errors(row)
        for field, value in [("started_ms", 2), ("calls_after", 2), ("calls_before", 0),
                             ("event_start", 0), ("retry_button", False), ("stimulus", "synthetic Event")]:
            row = native_errors(); row["outcomes"][0]["repeat"][field] = value
            with self.subTest(repeat=field), self.assertRaises(ValueError):
                t12.validate_errors(row)
        for field, value in [("trusted", False), ("at_ms", 2), ("session_id", "old"), ("epoch", 0)]:
            row = native_errors(); row["events"][1][field] = value
            with self.subTest(event=field), self.assertRaises(ValueError):
                t12.validate_errors(row)

    def test_native_error_matrix_rejects_missing_duplicate_queries_and_untrusted_old_events(self):
        for count in (0, 2):
            row = native_errors(); row["calls"][:1] = [copy.deepcopy(row["calls"][0]) for _ in range(count)]
            for item in row["outcomes"][1:4]:
                item["call_start"] += count - 1
            with self.subTest(queries=count), self.assertRaises(ValueError):
                t12.validate_errors(row)
        for field, value in [("trusted", False), ("epoch", 0), ("session_id", "old"), ("at_ms", 4)]:
            row = native_errors(); row["events"][0][field] = value
            with self.subTest(event=field), self.assertRaises(ValueError):
                t12.validate_errors(row)
        row = native_errors(); row["events"] = []
        with self.assertRaises(ValueError):
            t12.validate_errors(row)

    def test_eos_accepts_native_current_epoch_for_playing_and_paused_actions(self):
        for paused in (False, True):
            with self.subTest(paused=paused):
                t12.validate_eos(eos(paused))

    def test_eos_rejects_synthetic_missing_duplicate_or_unmapped_events(self):
        for change in (dict(ended=False), dict(dom_events=0), dict(dom_events=2), dict(events=[])):
            row = eos(); row.update(change)
            with self.subTest(change=change), self.assertRaises(ValueError):
                t12.validate_eos(row)
        row = eos(); row["events"].append(copy.deepcopy(row["events"][0]))
        with self.assertRaises(ValueError):
            t12.validate_eos(row)
        for target, field, value in [("event", "trusted", False), ("event-state", "ended", False),
                                      ("final", "ended", False), ("final", "phase", "playing")]:
            row = eos()
            state = row["events"][0] if target == "event" else row["events"][0]["state"] if target == "event-state" else row["final"]
            state[field] = value
            with self.subTest(target=target, field=field), self.assertRaises(ValueError):
                t12.validate_eos(row)

    def test_eos_rejects_old_epoch_or_session_and_wrong_toggle_order(self):
        for target in ("event", "final"):
            for field, value in [("epoch", 6), ("session_id", "old")]:
                row = eos()
                (row["events"][0]["state"] if target == "event" else row["final"])[field] = value
                with self.subTest(target=target, field=field), self.assertRaises(ValueError):
                    t12.validate_eos(row)
        for paused, stages in [(False, ["resume", "seek"]), (True, ["seek", "resume"]), (True, ["seek"])]:
            row = eos(paused); row["stages"] = [dict(stage=stage) for stage in stages]
            with self.subTest(paused=paused, stages=stages), self.assertRaises(ValueError):
                t12.validate_eos(row)

    def test_rss_windows_keep_all_fifty_switches_and_reject_only_leak_sized_growth(self):
        mib = 1024 * 1024
        samples = [dict(owned_rss_bytes=n) for n in range(51)]
        self.assertEqual(t12.rss_windows(samples), list(range(3, 50, 5)))
        # The real failed run: bounded cache warmup near 300MB is acceptable.
        t12.validate_rss_windows([297041920, 300122112, 302415872, 305856512, 309592064,
                                  314064896, 319832064, 323928064, 327532544, 331481088])
        t12.validate_rss_windows([300 * mib + 64 * mib * i // 9 for i in range(10)])
        leak = t12.rss_windows([dict(owned_rss_bytes=300 * mib + 2 * mib * n) for n in range(51)])
        with self.assertRaisesRegex(ValueError, "leak"):
            t12.validate_rss_windows(leak)
        with self.assertRaisesRegex(ValueError, "exceeds 512MiB"):
            t12.validate_rss_windows([513 * mib] * 10)

    def test_source_rematch_requires_visible_confirmation_and_resumed_playback(self):
        good = dict(stages=["playing", "confirmation-visible", "cancelled", "playing-again"], recovered=True, host=dict(live_sessions=0),
                    during_confirmation=dict(session=None, videos=0, route="details"))
        t12.validate_source_rematch(good)
        for bad in (dict(good, stages=["playing"]), dict(good, recovered=False), dict(good, host=dict(live_sessions=1)),
                    dict(good, during_confirmation=dict(session=None, videos=1, route="details"))):
            with self.assertRaises(ValueError):
                t12.validate_source_rematch(bad)

    def test_rss_windows_reject_missing_samples_windows_and_invalid_bytes(self):
        for count in (0, 1, 50, 52):
            with self.subTest(count=count), self.assertRaisesRegex(ValueError, "51 raw"):
                t12.rss_windows([dict(owned_rss_bytes=100)] * count)
        for invalid in (None, "100", True, -1, float("nan"), float("inf")):
            samples = [dict(owned_rss_bytes=100) for _ in range(51)]
            samples[25]["owned_rss_bytes"] = invalid
            with self.subTest(invalid=invalid), self.assertRaisesRegex(ValueError, "51 raw"):
                t12.rss_windows(samples)
        with self.assertRaisesRegex(ValueError, "ten five-switch"):
            t12.validate_rss_windows([100] * 9)

    def test_failed_rss_diagnostics_separate_media_observer_and_view_without_replacing_samples(self):
        samples = [dict(owned_rss_bytes=300 * 1024 * 1024 + 2 * 1024 * 1024 * n) for n in range(51)]
        medians = t12.rss_windows(samples)
        run = SimpleNamespace(observed=dict(process_samples=samples, rss_windows=medians), commands=[])
        actions = []
        session = SimpleNamespace(command=lambda op, **args: actions.append((op, args)) or stopped())
        processes = [dict(pid=20, identity="renderer-created", role="renderer", alive=True),
                     dict(pid=30, identity="gpu-created", role="gpu", alive=True)]
        def command(argv, timeout):
            self.assertEqual(argv[:2], ["/usr/bin/vmmap", "-summary"])
            self.assertEqual(timeout, 10)
            run.commands.append(dict(log=f"map-{len(run.commands)}.log"))
        run.command = command
        with patch.object(t12, "process_sample", return_value=dict(processes=processes)) as sample, \
                patch.object(t12.platform, "system", return_value="Darwin"), \
                patch.object(t12.Path, "is_file", return_value=True), \
                patch.object(t12.workflow.runtime, "alive", return_value=True):
            t12.diagnose_rss_failure(run, session, None)
        self.assertEqual(sample.call_count, 4)
        self.assertEqual([args["action"] for op, args in actions], ["stop", "release-observer", "dispose"])
        self.assertTrue(all(op == "t12" for op, args in actions))
        stages = run.observed["rss_failure_diagnostics"]
        self.assertEqual([row["stage"] for row in stages], ["live", "media-stopped", "observer-released", "view-disposed"])
        self.assertEqual(len(run.commands), 8)
        self.assertTrue(all(record["status"] == "CAPTURED" for row in stages for record in row["memory_maps"]))
        self.assertIs(run.observed["process_samples"], samples)
        self.assertIs(run.observed["rss_windows"], medians)
        # The same original samples must still fail even if post-stop RSS falls.
        with self.assertRaisesRegex(ValueError, "leak"):
            t12.validate_rss_windows(medians)

    def test_failed_memory_probe_is_recorded_once_without_retry_or_skipping_owned_cleanup(self):
        run = SimpleNamespace(observed={}, commands=[], command=Mock(side_effect=ValueError("task_for_pid denied")))
        actions = []
        session = SimpleNamespace(command=lambda op, **args: actions.append(args["action"]) or stopped())
        process = dict(pid=20, identity="created", role="renderer", alive=True)
        with patch.object(t12, "process_sample", return_value=dict(processes=[process])), \
                patch.object(t12.platform, "system", return_value="Darwin"), \
                patch.object(t12.Path, "is_file", return_value=True), \
                patch.object(t12.workflow.runtime, "alive", return_value=True):
            t12.diagnose_rss_failure(run, session, None)
        self.assertEqual(run.command.call_count, 1)
        self.assertEqual(actions, ["stop", "release-observer", "dispose"])
        records = [row["memory_maps"][0] for row in run.observed["rss_failure_diagnostics"]]
        self.assertEqual([row["status"] for row in records], ["FAILED", "NOT RUN", "NOT RUN", "NOT RUN"])
        self.assertIn("task_for_pid denied", records[0]["error"])
        run.command.reset_mock(); actions.clear()
        with patch.object(t12, "process_sample", return_value=dict(processes=[process])), \
                patch.object(t12.platform, "system", return_value="Darwin"), \
                patch.object(t12.Path, "is_file", return_value=True), \
                patch.object(t12.workflow.runtime, "alive", return_value=False):
            t12.diagnose_rss_failure(run, session, None)
        run.command.assert_not_called()
        self.assertEqual(actions, ["stop", "release-observer", "dispose"])
        self.assertIn("identity changed", run.observed["rss_failure_diagnostics"][0]["memory_maps"][0]["error"])

    def test_failed_diagnostic_stage_retains_error_and_leaves_cleanup_to_original_finally(self):
        run = SimpleNamespace(observed={}, commands=[])
        session = SimpleNamespace(command=Mock(side_effect=ValueError("stop failed")))
        with patch.object(t12, "process_sample", return_value=dict(processes=[])), \
                patch.object(t12.platform, "system", return_value="other"):
            t12.diagnose_rss_failure(run, session, None)
        stages = run.observed["rss_failure_diagnostics"]
        self.assertEqual([row["stage"] for row in stages], ["live", "media-stopped"])
        self.assertEqual(stages[1]["error"], "stop failed")
        session.command.assert_called_once_with("t12", action="stop")

    def test_process_detail_keeps_original_aggregate_and_pid_creation_identities(self):
        with tempfile.TemporaryDirectory() as folder:
            observations = []
            session = SimpleNamespace(observe=lambda: observations.append(True) or 900, process=SimpleNamespace(pid=10),
                owned={10: "created:1", 20: "created:2", 30: "created:3", 40: "created:4"},
                record=dict(index=1, webkit_pids=dict(renderer=20), wake_guard=dict(pid=40), commands=[{}]),
                run=SimpleNamespace(folder=Path(folder)), samples=[{}],
                fixture=SimpleNamespace(commands=queue.Queue(), results=queue.Queue(), samples=queue.Queue()))
            calls = []
            def process_for(pid, token):
                calls.append((pid, token))
                if pid == 30:
                    return None  # Reused/exited identity must contribute no detail RSS/CPU.
                return SimpleNamespace(cpu_times=lambda: SimpleNamespace(user=1, system=.5),
                    name=lambda: "fixture-process", memory_info=lambda: SimpleNamespace(rss=pid * 10))
            with patch.object(t12.workflow.runtime, "process_for", side_effect=process_for), \
                    patch.object(t12.workflow.runtime, "identity", return_value="created:observer"), \
                    patch.object(t12.workflow.runtime, "family", return_value=[
                        dict(pid=60, identity="created:host"), dict(pid=70, identity="created:host-child")]) as family, \
                    patch.object(t12.os, "getpid", return_value=50):
                result = t12.process_sample(session, SimpleNamespace(lines=["one", "two"], sequence=5,
                    process=SimpleNamespace(pid=60), token="created:host"))
            self.assertEqual(observations, [True])
            family.assert_called_once_with(60, "created:host")
            self.assertEqual(calls, list(session.owned.items()) + [(50, "created:observer"), (60, "created:host"), (70, "created:host-child")])
            self.assertEqual(result["owned_rss_bytes"], 900)
            self.assertEqual(result["detail_rss_bytes"], 700)
            self.assertEqual(result["owned_cpu_seconds"], 4.5)
            self.assertEqual([row["role"] for row in result["processes"]], ["application", "renderer", "owned_child", "wake_guard"])
            self.assertEqual(result["processes"][2], dict(pid=30, identity="created:3", role="owned_child", alive=False, rss_bytes=None, cpu_seconds=None))
            self.assertEqual([row["role"] for row in result["support_processes"]], ["python_observer", "real_host", "real_host_child"])
            self.assertEqual([row["rss_bytes"] for row in result["support_processes"]], [500, 600, 700])
            self.assertEqual(result["observer"], dict(command_results=1, queued_commands=0, queued_results=0, queued_samples=0,
                retained_samples=1, result_log_bytes=0, host_log_lines=2, host_requests=5))

    def test_image_expiry_accepts_native_error_once_query_owning_dto_refresh_and_decode(self):
        t12.validate_image_expiry(image_expiry())

    def test_image_expiry_rejects_cached_success_missing_native_events_and_wrong_dto(self):
        edits = [("expired_http_status", 200), ("dto_refreshed", False), ("decoded", False),
                 ("refreshed_ref", dict(source_id="other", source_revision="revision", data="show")),
                 ("image_error", "still failed"), ("page_epoch_after", 2), ("host_epoch_after", "other"),
                 ("events", []), ("stages", [])]
        for key, value in edits:
            row = image_expiry(); row[key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                t12.validate_image_expiry(row)
        for index, changes in [(0, dict(trusted=False)), (0, dict(original_dto=False)), (0, dict(original_url=False)),
                               (0, dict(resource_id="wrong")), (0, dict(owning_url=False)), (0, dict(natural_width=1)), (0, dict(page_epoch=2)),
                               (0, dict(at_ms=7)), (1, dict(trusted=False)), (1, dict(original_dto=True)),
                               (1, dict(natural_width=0)), (1, dict(owning_url=False)), (1, dict(resource_id="wrong")),
                               (1, dict(page_epoch=2)), (1, dict(at_ms=30000))]:
            row = image_expiry(); row["events"][index].update(changes)
            with self.subTest(index=index, changes=changes), self.assertRaises(ValueError):
                t12.validate_image_expiry(row)

    def test_image_expiry_rejects_duplicate_query_refresh_or_native_events(self):
        for field in ("calls", "events"):
            for index in (0, 1):
                row = image_expiry(); row[field].append(copy.deepcopy(row[field][index]))
                with self.subTest(field=field, index=index), self.assertRaises(ValueError):
                    t12.validate_image_expiry(row)
        for command in ("get_resource_status", "get_details"):
            for count in (0, 2):
                row = image_expiry()
                row["after"]["command_counts"][command] = row["before"]["command_counts"][command] + count
                with self.subTest(command=command, count=count), self.assertRaises(ValueError):
                    t12.validate_image_expiry(row)

    def test_image_expiry_rejects_query_identity_target_state_budget_and_refresh_revision(self):
        changes = [dict(request_id="bad"), dict(response_request_id="other"), dict(engine_epoch="other"),
                   dict(kind="playback"), dict(target_id="wrong"), dict(response_kind="playback"),
                   dict(response_target_id="wrong"), dict(state="ready"), dict(code=None),
                   dict(started_ms=0), dict(finished_ms=3004)]
        for change in changes:
            row = image_expiry(); row["calls"][0].update(change)
            with self.subTest(query=change), self.assertRaises(ValueError):
                t12.validate_image_expiry(row)
        changes = [dict(request_id="bad"), dict(response_request_id="other"), dict(engine_epoch="other"),
                   dict(ref=dict(source_id="other")), dict(response_ref=dict(source_revision="old")),
                   dict(code="UPSTREAM_FAILED"), dict(started_ms=2), dict(finished_ms=7)]
        for change in changes:
            row = image_expiry(); row["calls"][1].update(change)
            with self.subTest(refresh=change), self.assertRaises(ValueError):
                t12.validate_image_expiry(row)

    def test_image_query_rejects_over_3s_with_otherwise_ordered_native_events(self):
        row = image_expiry()
        row["calls"][0]["finished_ms"] = 3003
        row["calls"][1].update(started_ms=3004, finished_ms=3005)
        row["events"][1]["at_ms"] = 3006
        with self.assertRaisesRegex(ValueError, "3s budget"):
            t12.validate_image_expiry(row)

    def test_frontend_pins_tap_and_keeps_nonzero_inventory(self):
        from types import SimpleNamespace
        calls = []
        run = SimpleNamespace(observed={}, command=lambda argv: calls.append(argv) or "TAP version 13\n# tests 43\n# fail 0\n")
        self.assertEqual(t12.frontend(run), dict(tests=43, failed=0))
        self.assertIn("--test-reporter=tap", calls[0])
        self.assertIn("tests/t12-webview.test.ts", calls[0])
        self.assertEqual(calls[1], ["npm", "run", "check"])

    def test_frontend_rejects_failures_missing_summary_and_insufficient_inventory(self):
        from types import SimpleNamespace
        for output in ("", "ℹ tests 43\nℹ fail 0\n", "# tests 0\n# fail 0\n", "# tests 39\n# fail 0\n",
                       "# tests 43\n# fail 1\n", "# tests 43\n", "# fail 0\n"):
            calls = []
            run = SimpleNamespace(observed={}, command=lambda argv: calls.append(argv) or output)
            with self.subTest(output=output), self.assertRaises(ValueError):
                t12.frontend(run)
            self.assertEqual(len(calls), 1)
            self.assertNotIn("frontend_tests", run.observed)

    def test_cycle_accepts_current_audio_color_and_released_old_handles(self):
        t12.validate_cycle(cycle(), 1)
        value = cycle()
        value["index"] = 2
        value["sample"].update(pixel=[254, 0, 0, 255], audio_peak_hz=440)
        t12.validate_cycle(value, 2)

    def test_cycle_blue_green_calibration_preserves_red_and_hls_boundaries(self):
        for index, frequency, accepted, limit in [(1, 880, [54, 63], 64), (2, 440, [49], 50), (51, 440, [49], 50)]:
            value = cycle()
            value["index"] = index
            value["sample"]["audio_peak_hz"] = frequency
            if index == 51:
                value["native"] = dict(audio_playing=True)
            for green in accepted:
                value["sample"]["pixel"] = [4, green, 255, 255] if index == 1 else [254, green, 0, 255]
                t12.validate_cycle(value, index)
            value["sample"]["pixel"][1] = limit
            with self.subTest(index=index), self.assertRaises(ValueError):
                t12.validate_cycle(value, index)

    def test_cycle_rejects_every_resource_audio_frame_and_identity_corruption(self):
        edits = [("index", 2), ("old_tracks_disabled", False), ("session_changed", False),
                 ("new_frames", 0), ("actual_gateway", False), ("errors", ["runtime failed"])]
        for key, value in edits:
            with self.subTest(key=key), self.assertRaises(ValueError):
                t12.validate_cycle(dict(cycle(), **{key: value}), 1)
        edits = [("video_count", 2), ("tracks", 2), ("width", 0), ("height", 0), ("media_error", 3),
                 ("audio_state", "suspended"), ("session_bound", False), ("audio_rms", 0), ("audio_peak_hz", 440),
                 ("pixel", [254, 0, 0, 255]), ("pixel", [0, 0, 0, 255])]
        for key, value in edits:
            row = cycle(); row["sample"][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                t12.validate_cycle(row, 1)
        for key, value in [("hls_instances", 2), ("listening", False), ("retiring", 1)]:
            row = cycle(); row["sample"]["diagnostics"][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                t12.validate_cycle(row, 1)
        for key, value in [("live_sessions", 2), ("requests", 1), ("old_http_status", 200)]:
            row = cycle(); row["host"][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                t12.validate_cycle(row, 1)

    def test_stop_rejects_old_audio_frame_and_every_retained_native_resource(self):
        t12.validate_stop(stopped())
        for key, value in [("source_removed", False), ("frames_after_stop", 2)]:
            with self.subTest(key=key), self.assertRaises(ValueError):
                t12.validate_stop(dict(stopped(), **{key: value}))
        for key, value in [("video_count", 2), ("paused", False), ("ready_state", 1), ("network_state", 2),
                           ("source_present", True), ("tracks", 1), ("audio_rms", .1)]:
            row = stopped(); row["sample"][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                t12.validate_stop(row)
        for key, value in [("listening", True), ("hls_instances", 1), ("retiring", 1)]:
            row = stopped(); row["sample"]["diagnostics"][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                t12.validate_stop(row)
        for key in ("live_sessions", "requests", "source_active"):
            row = stopped(); row["host"][key] = 1
            with self.subTest(key=key), self.assertRaises(ValueError):
                t12.validate_stop(row)

    def test_requirement_summary_requires_real_children_and_no_skipped_required_behavior(self):
        self.assertTrue(all(r["result"] == "FAIL" for r in t12.summaries([])))
        cases = [dict(id="all", requirements=t12.IDS, result="PASS")]
        self.assertTrue(all(r["result"] == "PASS" for r in t12.summaries(cases)))
        for result in ("FAIL", "SKIP", "NOT RUN"):
            self.assertTrue(all(r["result"] == "FAIL" for r in t12.summaries([dict(cases[0], result=result)])))

    def test_locked_native_desktop_stops_before_build_or_launch(self):
        from types import SimpleNamespace
        from unittest.mock import patch
        run = SimpleNamespace(observed={})
        with patch.object(t12.t01, "native_gui_session", return_value=dict(screen_locked=True)), \
             patch.object(t12.t01, "build") as build:
            with self.assertRaisesRegex(ValueError, "desktop locked"):
                t12.webview(run)
            build.assert_not_called()


class TransportTests(unittest.TestCase):
    def test_real_host_transport_exists_only_when_explicitly_supplied_and_authenticated(self):
        calls = []
        with tempfile.TemporaryDirectory() as temporary:
            for enabled in (False, True):
                with t12.t01.Fixture(Path(temporary), invoke=(lambda value: calls.append(value) or dict(ok=True)) if enabled else None) as fixture:
                    url = fixture.base + "control/invoke?cap=" + fixture.cap
                    request = urllib.request.Request(url, data=json.dumps(dict(command="runtime_status")).encode(), headers={"Content-Type": "application/json", "Origin": "tauri://localhost"})
                    if enabled:
                        with urllib.request.urlopen(request) as response:
                            self.assertEqual(json.load(response), dict(ok=True))
                    else:
                        with self.assertRaises(urllib.error.HTTPError) as error:
                            urllib.request.urlopen(request)
                        self.assertEqual(error.exception.code, 405)
                    denied = urllib.request.Request(url, data=b'{}', headers={"Origin": "https://untrusted.invalid"})
                    with self.assertRaises(urllib.error.HTTPError) as error:
                        urllib.request.urlopen(denied)
                    self.assertEqual(error.exception.code, 403)
            self.assertEqual(calls, [dict(command="runtime_status")])


if __name__ == "__main__":
    unittest.main()
