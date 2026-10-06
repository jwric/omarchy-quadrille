#!/usr/bin/env python3
"""No live IPC: workspace ownership and capture guards use synthetic state."""
import copy
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from PIL import Image

from wallpaper_live import capture, require_empty, workspace_plan


class PreviewSafety(unittest.TestCase):
    def setUp(self):
        self.monitors = [
            {"name": "eDP-2", "activeWorkspace": {"id": 3}, "specialWorkspace": {"id": 0}},
            {"name": "HDMI-A-1", "activeWorkspace": {"id": 8}, "specialWorkspace": {"id": 0}},
        ]
        self.workspaces = [
            {"id": 3, "windows": 2, "monitor": "eDP-2"},
            {"id": 8, "windows": 0, "monitor": "HDMI-A-1"},
            {"id": 9, "windows": 0, "monitor": "HDMI-A-1"},
        ]
        self.names = [m["name"] for m in self.monitors]

    def test_existing_dell_workspace_never_targeted_from_laptop(self):
        planned = workspace_plan(self.monitors, self.workspaces)
        self.assertEqual(planned[0]["target"], 10)
        self.assertEqual(planned[1]["target"], 8)
        self.assertNotIn(planned[0]["target"], [w["id"] for w in self.workspaces])

    def test_guard_checks_displayed_workspaces_after_a_wrong_focus(self):
        with self.assertRaises(ValueError):
            require_empty(self.monitors, self.workspaces, self.names)
        monitors = copy.deepcopy(self.monitors)
        monitors[0]["activeWorkspace"]["id"] = 10
        workspaces = self.workspaces + [{"id": 10, "windows": 0, "monitor": "eDP-2"}]
        require_empty(monitors, workspaces, self.names)
        workspaces[-1]["windows"] = 1
        with self.assertRaises(ValueError):
            require_empty(monitors, workspaces, self.names)

    def test_special_workspaces_are_refused_by_plan_and_guard(self):
        self.monitors[0]["activeWorkspace"]["id"] = 10
        self.workspaces.append({"id": 10, "windows": 0, "monitor": "eDP-2"})
        self.monitors[1]["specialWorkspace"]["id"] = -99
        with self.assertRaises(ValueError):
            workspace_plan(self.monitors, self.workspaces)
        with self.assertRaises(ValueError):
            require_empty(self.monitors, self.workspaces, self.names)

    def test_new_monitor_is_refused_even_when_it_is_empty(self):
        self.monitors[0]["activeWorkspace"]["id"] = 10
        added = {"name": "DP-1", "activeWorkspace": {"id": 42}, "specialWorkspace": {"id": 0}}
        workspaces = self.workspaces + [{"id": 10, "windows": 0, "monitor": "eDP-2"},
            {"id": 42, "windows": 0, "monitor": "DP-1"}]
        with self.assertRaises(ValueError):
            require_empty(self.monitors + [added], workspaces, self.names)

    def test_capture_never_grabs_an_occupied_actual_workspace(self):
        with tempfile.TemporaryDirectory() as directory:
            with patch("wallpaper_live.live_state", return_value=(self.monitors, self.workspaces)), patch("wallpaper_live.subprocess.run") as grab:
                with self.assertRaises(ValueError):
                    capture([{"name": name} for name in self.names], Path(directory), Path(directory))
                grab.assert_not_called()

    def test_capture_discards_output_when_a_window_arrives_during_grab(self):
        before = copy.deepcopy(self.monitors)
        before[0]["activeWorkspace"]["id"] = 10
        empty = self.workspaces + [{"id": 10, "windows": 0, "monitor": "eDP-2"}]
        occupied = copy.deepcopy(empty)
        occupied[-1]["windows"] = 1
        with tempfile.TemporaryDirectory() as directory:
            def fake_grab(command, **kwargs):
                Image.new("RGB", (128, 128)).save(command[-1])
            with patch("wallpaper_live.live_state", side_effect=[(before, empty), (before, occupied)]), patch("wallpaper_live.subprocess.run", side_effect=fake_grab) as grab:
                with self.assertRaises(ValueError):
                    capture([{"name": name} for name in self.names], Path(directory), Path(directory))
                grab.assert_called_once()
                self.assertFalse(list(Path(directory).glob("live-*.png")))


if __name__ == "__main__":
    unittest.main()
