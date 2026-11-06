"""Independent counterexamples for T21's request and process evidence gates."""
import copy
import hashlib
import unittest

from t21 import check_dash_samples, check_resources, check_trace


def media_trace():
    names = ["a.mp4", "b.mp4", "index.m3u8", "seg-0.ts", "seg-1.ts"]
    files = [dict(name=name, bytes=len(name), sha256=hashlib.sha256(name.encode()).hexdigest()) for name in names]
    trace = dict(inventory=copy.deepcopy(files), bindings=[], requests=[],
                 active_requests=0, upstream_active=0, local_credentials=False)
    for episode in range(52):
        name = "index.m3u8" if episode == 51 else "a.mp4" if episode % 2 == 0 else "b.mp4"
        owner = dict(context=f"context-{episode}", session_id=f"session-{episode}",
                     resolution_id=f"resolution-{episode}", stream_id=f"stream-{episode}", episode=episode, root=name)
        trace["bindings"].append(owner)
        for filename in [name, *(["seg-0.ts", "seg-1.ts"] if episode == 51 else [])]:
            item = next(item for item in files if item["name"] == filename)
            trace["requests"].append(dict(owner=copy.deepcopy(owner), name=filename,
                mode=0, status=200, bytes=item["bytes"], sha256=item["sha256"], range=None, local_credentials=False))
    return trace, dict(files=files)


def resources():
    processes = [dict(pid=index + 10, identity=100.0 + index, role=role,
                      alive=True, rss_bytes=1024) for index, role in enumerate(["application", "renderer", "gpu", "network"])]
    sample = dict(owned_rss_bytes=4096, detail_rss_bytes=4096, processes=processes,
        support_processes=[dict(pid=20 + index, identity=200.0 + index, role=role,
                                alive=True, rss_bytes=1024) for index, role in enumerate(["python_observer", "real_host"])])
    session = dict(exit=0, lingering=[], forced_cleanup=[], peak_combined_rss_bytes=8192,
        wake_guard=dict(exit=0), owned_processes=[dict(pid=row["pid"], identity=row["identity"]) for row in processes],
        webkit_pids=dict(renderer=11, gpu=12, network=13))
    return dict(native_sessions=[session], process_samples=[copy.deepcopy(sample) for _ in range(51)])


class SessionTraceTests(unittest.TestCase):
    def test_complete_shared_media_and_all_episode_requests_pass(self):
        trace, media = media_trace()
        result = check_trace(trace, media)
        self.assertEqual(result["episodes"], 52)
        self.assertEqual(result["files"], 5)

    def test_foreign_session_resolution_stream_context_and_episode_fail(self):
        for field in ("session_id", "resolution_id", "stream_id", "context", "episode"):
            with self.subTest(field=field):
                trace, media = media_trace()
                trace["requests"][0]["owner"][field] = "foreign"
                with self.assertRaises(ValueError):
                    check_trace(trace, media)

    def test_segment_cannot_belong_to_progressive_session(self):
        trace, media = media_trace()
        trace["requests"][-1]["owner"] = trace["bindings"][0]
        with self.assertRaisesRegex(ValueError, "another stream"):
            check_trace(trace, media)

    def test_missing_file_segment_episode_and_changed_media_fail(self):
        for change in ("segment", "episode", "hash", "body", "inventory", "duplicate"):
            with self.subTest(change=change):
                trace, media = media_trace()
                if change == "segment":
                    trace["requests"] = [row for row in trace["requests"] if row["name"] != "seg-1.ts"]
                elif change == "episode":
                    trace["requests"] = [row for row in trace["requests"] if row["owner"]["episode"] != 2]
                elif change == "inventory":
                    trace["inventory"].pop()
                elif change == "duplicate":
                    trace["bindings"].append(trace["bindings"][0])
                else:
                    trace["requests"][0]["sha256" if change == "hash" else "bytes"] = "wrong" if change == "hash" else 0
                with self.assertRaises(ValueError):
                    check_trace(trace, media)

    def test_local_credentials_or_live_requests_fail(self):
        for field, value in [("active_requests", 1), ("upstream_active", 1), ("local_credentials", True)]:
            trace, media = media_trace()
            trace[field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                check_trace(trace, media)
        trace, media = media_trace()
        trace["requests"][0]["local_credentials"] = True
        with self.assertRaises(ValueError):
            check_trace(trace, media)

    def test_fault_response_status_is_still_checked(self):
        trace, media = media_trace()
        trace["requests"].append(dict(trace["requests"][0], mode=1, status=401))
        check_trace(trace, media)
        trace["requests"][-1]["status"] = 200
        with self.assertRaises(ValueError):
            check_trace(trace, media)


class ResourceTests(unittest.TestCase):
    def test_complete_identity_rss_and_cleanup_pass(self):
        self.assertEqual(check_resources(resources())["samples"], 51)

    def test_missing_renderer_support_identity_rss_and_sample_fail(self):
        for change in ("renderer", "support", "identity", "rss", "total", "sample", "gpu", "role"):
            value = resources()
            if change in ("renderer", "support"):
                value["process_samples"][0]["processes" if change == "renderer" else "support_processes"] = []
            elif change == "identity":
                value["process_samples"][0]["processes"][0]["identity"] += 1
            elif change == "rss":
                value["process_samples"][0]["processes"][0]["rss_bytes"] = None
            elif change == "total":
                value["process_samples"][0]["detail_rss_bytes"] = 0
            elif change == "gpu":
                value["native_sessions"][0]["owned_processes"].pop(2)
            elif change == "role":
                value["native_sessions"][0]["webkit_pids"].pop("gpu")
            else:
                value["process_samples"].pop()
            with self.subTest(change=change), self.assertRaises(ValueError):
                check_resources(value)

    def test_budget_residue_forced_cleanup_and_missing_wake_exit_fail(self):
        for field, item in [("peak_combined_rss_bytes", 4096 * 1024 * 1024 + 1),
                            ("peak_combined_rss_bytes", 0), ("lingering", [10]),
                            ("forced_cleanup", [11]), ("exit", 1), ("wake_guard", dict(exit=1))]:
            value = resources()
            value["native_sessions"][0][field] = item
            with self.subTest(field=field), self.assertRaises(ValueError):
                check_resources(value)


class DashBaselineTests(unittest.TestCase):
    def test_synthetic_probe_is_separate_and_observed_dash_requires_full_gate(self):
        check_dash_samples([dict(kind="hls", observed_kind="hls")])
        for field in ("kind", "observed_kind"):
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "DASH baseline"):
                check_dash_samples([dict(kind="hls"), {field: "dash"}])
        with self.assertRaises(ValueError):
            check_dash_samples([])


if __name__ == "__main__":
    unittest.main()
