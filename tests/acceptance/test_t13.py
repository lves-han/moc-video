"""Independent controls for T13 evidence validation; these do not run a UI."""
import copy
import itertools
import unittest

import t13


def frame(palette="lime", lang="en", dark=False, reload=False):
    return dict(id="history-refresh-reset-and-preferences" if reload else "first-frame-" + palette,
                result="PASS", actual=dict(
                    first=dict(palette=palette, lang=lang, dark=dark, scheme="dark" if dark else "light", accent="#527a00"),
                    expected=dict(color_palette=palette, ui_locale=lang), emptyHistory=True, samePageReload=reload))


def main_report():
    rows = {name: dict(id=name, result="PASS", actual={}) for name in (
        "identity-first-frame", "return-and-language-filter", "palette-language-system-matrix",
        "minimum-window-and-dpi", "storage-failure-feedback", "playback-locale-palette-and-shortcuts",
        "history-resume-merge-delete-clear-focus", "error-and-poster-contrast", "console-and-overlay-health",
        "history-reload-precondition")}
    rows["palette-language-system-matrix"]["actual"] = [dict(
        palette=p, dark=d, lang=l, layout=dict(width=390, scrollWidth=390),
        **{kind: [dict(contrast=4.5)] for kind in (
            "contrast", "popupContrast", "catalogContrast", "playerContrast", "errorContrast")})
        for p, d, l in itertools.product(("lime", "ocean", "violet", "rose", "amber"), (False, True), ("en", "zh-CN"))]
    rows["minimum-window-and-dpi"]["actual"] = [dict(
        native=dict(zoom=z, width=960, height=640), layout=dict(width=960, scrollWidth=960)) for z in (1, 1.25, 1.5, 2)]
    rows["playback-locale-palette-and-shortcuts"]["actual"] = dict(videoWidth=1920, duration=30, position=6)
    rows["history-resume-merge-delete-clear-focus"]["actual"] = dict(
        resumedPosition=6, replayedPosition=0, mergedTitles=1, latestEpisode=3,
        cancelledDeletionPreservedData=True, removed=True, cleared=True)
    rows["console-and-overlay-health"]["actual"] = dict(logs=[])
    rows["history-reload-precondition"]["actual"] = dict(visibleTitlesBeforeReload=1)
    rows["history-refresh-reset-and-preferences"] = frame(reload=True)
    return dict(ok=True, cases=list(rows.values()))


def actual(report, identifier):
    return next(row["actual"] for row in report["cases"] if row["id"] == identifier)


class UiOracleTests(unittest.TestCase):
    def test_complete_main_report_with_history_reload_is_accepted(self):
        # Regression: history reset used to send every main-suite row through
        # the standalone frame branch, raising KeyError on the first row.
        t13.validate_ui(main_report())

    def test_startup_frames_are_accepted_for_each_palette_language_and_mode(self):
        for palette, lang, dark in itertools.product(
                ("lime", "ocean", "violet", "rose", "amber"), ("en", "zh-CN"), (False, True)):
            with self.subTest(palette=palette, lang=lang, dark=dark):
                t13.validate_ui(dict(ok=True, cases=[frame(palette, lang, dark)]))

    def test_failed_duplicate_and_missing_main_cases_are_rejected(self):
        for index in range(len(main_report()["cases"])):
            for mode in ("missing", "failed", "duplicate"):
                report = main_report()
                if mode == "missing":
                    report["cases"].pop(index)
                elif mode == "failed":
                    report["cases"][index]["result"] = "FAIL"
                else:
                    report["cases"].append(copy.deepcopy(report["cases"][index]))
                with self.subTest(index=index, mode=mode), self.assertRaises(ValueError):
                    t13.validate_ui(report)
        for report in (dict(ok=False, cases=[frame()]), dict(ok=True, cases=[])):
            with self.assertRaises(ValueError):
                t13.validate_ui(report)

    def test_incomplete_duplicate_or_wrong_matrix_is_rejected(self):
        for mode in ("missing", "duplicate", "palette", "dark", "lang"):
            report = main_report()
            matrix = actual(report, "palette-language-system-matrix")
            if mode == "missing":
                matrix.pop()
            elif mode == "duplicate":
                matrix[-1] = copy.deepcopy(matrix[0])
            else:
                matrix[0][mode] = {"palette": "unknown", "dark": "false", "lang": "fr"}[mode]
            with self.subTest(mode=mode), self.assertRaises(ValueError):
                t13.validate_ui(report)

    def test_clipped_matrix_and_missing_or_failed_contrast_are_rejected(self):
        report = main_report()
        actual(report, "palette-language-system-matrix")[0]["layout"]["scrollWidth"] = 392
        with self.assertRaises(ValueError):
            t13.validate_ui(report)
        report = main_report()
        actual(report, "palette-language-system-matrix")[0]["layout"]["width"] = 960
        with self.assertRaises(ValueError):
            t13.validate_ui(report)
        for kind in ("contrast", "popupContrast", "catalogContrast", "playerContrast", "errorContrast"):
            for samples in ([], [dict(contrast=4.49)], [dict(contrast=float("nan"))], [dict(contrast=float("inf"))]):
                report = main_report()
                actual(report, "palette-language-system-matrix")[0][kind] = samples
                with self.subTest(kind=kind, samples=samples), self.assertRaises(ValueError):
                    t13.validate_ui(report)

    def test_missing_dpi_wrong_window_and_clipped_dpi_are_rejected(self):
        for mode in ("missing", "zoom", "width", "height", "clipped"):
            report = main_report()
            rows = actual(report, "minimum-window-and-dpi")
            if mode == "missing":
                rows.pop()
            elif mode == "clipped":
                rows[0]["layout"]["scrollWidth"] = 962
            else:
                rows[0]["native"][mode] = 3
            with self.subTest(mode=mode), self.assertRaises(ValueError):
                t13.validate_ui(report)

    def test_missing_decoded_media_or_progress_is_rejected(self):
        for key, value in (("videoWidth", 0), ("duration", 0), ("duration", float("nan")), ("position", 0)):
            report = main_report()
            actual(report, "playback-locale-palette-and-shortcuts")[key] = value
            with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                t13.validate_ui(report)

    def test_wrong_history_resume_merge_or_confirmation_is_rejected(self):
        for key, value in (("resumedPosition", 0), ("replayedPosition", 8), ("mergedTitles", 2), ("latestEpisode", 2),
                           ("cancelledDeletionPreservedData", False), ("removed", False), ("cleared", False)):
            report = main_report()
            actual(report, "history-resume-merge-delete-clear-focus")[key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                t13.validate_ui(report)
        report = main_report()
        actual(report, "history-reload-precondition")["visibleTitlesBeforeReload"] = 0
        with self.assertRaises(ValueError):
            t13.validate_ui(report)

    def test_wrong_or_missing_first_frame_is_rejected_in_both_report_shapes(self):
        for full in (False, True):
            for key, value in (("palette", "rose"), ("lang", "zh-CN"), ("scheme", "dark"), ("dark", "false"), ("accent", "")):
                report = main_report() if full else dict(ok=True, cases=[frame()])
                row = report["cases"][-1]
                row["actual"]["first"][key] = value
                with self.subTest(full=full, key=key), self.assertRaises(ValueError):
                    t13.validate_ui(report)
            for key in ("first", "expected"):
                report = main_report() if full else dict(ok=True, cases=[frame()])
                del report["cases"][-1]["actual"][key]
                with self.subTest(full=full, missing=key), self.assertRaises(ValueError):
                    t13.validate_ui(report)

    def test_history_retained_across_reload_or_wrong_reload_is_rejected(self):
        for full in (False, True):
            for key in ("emptyHistory", "samePageReload"):
                report = main_report() if full else dict(ok=True, cases=[frame(reload=True)])
                report["cases"][-1]["actual"][key] = False
                with self.subTest(full=full, key=key), self.assertRaises(ValueError):
                    t13.validate_ui(report)

    def test_startup_case_identity_or_mixed_incomplete_inventory_is_rejected(self):
        report = dict(ok=True, cases=[frame()])
        report["cases"][0]["id"] = "first-frame-rose"
        with self.assertRaises(ValueError):
            t13.validate_ui(report)
        for field, value in (("color_palette", "unknown"), ("ui_locale", "fr")):
            report = dict(ok=True, cases=[frame()])
            report["cases"][0]["actual"]["expected"][field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                t13.validate_ui(report)
        for cases in ([frame(), frame("rose")], [frame(reload=True)]):
            with self.subTest(cases=cases), self.assertRaises(ValueError):
                t13.validate_ui(dict(ok=True, cases=cases))

    def test_console_errors_are_rejected(self):
        report = main_report()
        actual(report, "console-and-overlay-health")["logs"] = ["uncaught error"]
        with self.assertRaises(ValueError):
            t13.validate_ui(report)


if __name__ == "__main__":
    unittest.main()
