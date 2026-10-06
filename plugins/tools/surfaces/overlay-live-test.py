#!/usr/bin/env python3
"""Synthetic state only: no live host, compositor or screenshot processes."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from PIL import Image

from overlay_live import ProtocolEvidence, capture, crop_box, layer_rectangles


class OverlayPreviewSafety(unittest.TestCase):
    def setUp(self):
        self.monitors = [{"name": "eDP-2", "activeWorkspace": {"id": 10}, "specialWorkspace": {"id": 0},
            "x": 0, "y": 0, "width": 2560, "height": 1600, "scale": 5 / 3, "transform": 0}]
        self.workspaces = [{"id": 10, "windows": 0, "monitor": "eDP-2"}]
        self.rectangles = [{"output": "eDP-2", "rect": (90, 120, 288, 87), "namespace": "quadrille-reticle", "pid": 42}]

    def protocol(self):
        evidence = ProtocolEvidence()
        for line in [
            '[123][rs] -> zwlr_layer_shell_v1@8.get_layer_surface(zwlr_layer_surface_v1@17, wl_surface@15, wl_output@10, 3, Some("quadrille-reticle"))',
            '[123][rs] -> zwlr_layer_surface_v1@17.set_keyboard_interactivity(0)',
            '[123][rs] -> wl_compositor@3.create_region(wl_region@24)',
            '[123][rs] -> wl_surface@15.set_input_region(wl_region@24)',
            '[123][rs] -> wl_region@24.destroy()',
            '[123][rs] -> zwlr_layer_surface_v1@17.set_exclusive_zone(-1)',
        ]:
            evidence.feed(line)
        return evidence

    def test_passive_protocol_and_private_titles_are_filtered(self):
        evidence = self.protocol()
        evidence.feed('[123][rs] <- zwlr_foreign_toplevel_handle_v1@29.title, (Some("PRIVATE WINDOW TITLE"))')
        evidence.feed('a private log message')
        value = evidence.snapshot()
        self.assertEqual(len(value["layers"]), 1)
        self.assertTrue(value["layers"][0]["empty_input_region"])
        self.assertTrue(value["layers"][0]["overlay_layer"])
        self.assertEqual(value["layers"][0]["keyboard"], 0)
        self.assertEqual(value["layers"][0]["exclusive_zone"], -1)
        self.assertNotIn("PRIVATE", json.dumps(value))
        self.assertFalse(value["raw_protocol_retained"])

    def test_standard_libwayland_object_notation(self):
        evidence = ProtocolEvidence()
        for line in [
            ' -> zwlr_layer_shell_v1#8.get_layer_surface(new id zwlr_layer_surface_v1#17, wl_surface#15, wl_output#10, 3, "quadrille-reticle")',
            ' -> wl_compositor#3.create_region(new id wl_region#24)',
            ' -> wl_surface#15.set_input_region(wl_region#24)',
        ]:
            evidence.feed(line)
        self.assertTrue(evidence.snapshot()["layers"][0]["empty_input_region"])

    def test_nonempty_null_and_unknown_regions_are_refused(self):
        for region in ["wl_region@25", "nil", "wl_region@26"]:
            evidence = self.protocol()
            evidence.feed(' -> wl_compositor@3.create_region(wl_region@25)')
            evidence.feed(' -> wl_region@25.add(0, 0, 20, 20)')
            evidence.feed(f' -> wl_surface@15.set_input_region({region})')
            self.assertFalse(evidence.snapshot()["layers"][0]["empty_input_region"])
        evidence.feed(' -> zwlr_layer_surface_v1@17.set_keyboard_interactivity(1)')
        self.assertEqual(evidence.snapshot()["layers"][0]["keyboard"], 1)

    def test_layers_belong_only_to_the_owned_host(self):
        item = {"namespace": "quadrille-reticle", "pid": 42, "x": -40.2, "y": 23.5, "w": 288, "h": 87, "alpha": 1}
        value = {"eDP-2": {"levels": {"3": [item, dict(item, pid=100)]}}}
        self.assertEqual(len(layer_rectangles(value, 42)), 1)
        self.assertEqual(layer_rectangles(value, 42)[0]["rect"], (-40.2, 23.5, 288, 87))
        value["eDP-2"]["levels"] = {"2": [item]}
        with self.assertRaises(ValueError):
            layer_rectangles(value, 42)

    def test_global_geometry_and_bar_crop(self):
        monitor = dict(self.monitors[0], x=-952, y=-1440, scale=1)
        self.assertEqual(crop_box((-852, -1340, 320, 96), monitor, (3440, 1440)), (100, 100, 420, 196))
        self.assertEqual(crop_box((30, 5, 288, 87), self.monitors[0], (2560, 1600)), (50, 60, 530, 154))
        with self.assertRaises(ValueError):
            crop_box((30, 5, 288, 10), self.monitors[0], (2560, 1600))
        with self.assertRaises(ValueError):
            crop_box((30, 50, 288, 87), dict(self.monitors[0], transform=1), (1600, 2560))

    def test_occupied_output_is_never_grabbed(self):
        occupied = copy.deepcopy(self.workspaces)
        occupied[0]["windows"] = 1
        with tempfile.TemporaryDirectory() as directory:
            with patch("overlay_live.layers", return_value=self.rectangles), patch("overlay_live.live_state", return_value=(self.monitors, occupied)), patch("overlay_live.subprocess.run") as grab:
                with self.assertRaises(ValueError):
                    capture([{"name": "eDP-2"}], Path(directory), Path(directory), 42)
                grab.assert_not_called()

    def test_window_arrival_or_pointer_motion_discards_the_capture(self):
        occupied = copy.deepcopy(self.workspaces)
        occupied[0]["windows"] = 1
        moved = [dict(self.rectangles[0], rect=(91, 120, 288, 87))]
        for after, later_layers in [(occupied, self.rectangles), (self.workspaces, moved)]:
            with tempfile.TemporaryDirectory() as directory:
                def fake_grab(command, **kwargs):
                    Image.new("RGB", (2560, 1600)).save(command[-1])
                state = [(self.monitors, self.workspaces), (self.monitors, after)]
                with patch("overlay_live.layers", side_effect=[self.rectangles, later_layers]), patch("overlay_live.live_state", side_effect=state), patch("overlay_live.subprocess.run", side_effect=fake_grab):
                    with self.assertRaises(ValueError):
                        capture([{"name": "eDP-2"}], Path(directory), Path(directory), 42)
                self.assertFalse(list(Path(directory).glob("*.png")))

    def test_success_retains_only_the_layer_crop(self):
        with tempfile.TemporaryDirectory() as directory:
            def fake_grab(command, **kwargs):
                Image.new("RGB", (2560, 1600)).save(command[-1])
            with patch("overlay_live.layers", return_value=self.rectangles), patch("overlay_live.live_state", return_value=(self.monitors, self.workspaces)), patch("overlay_live.subprocess.run", side_effect=fake_grab):
                capture([{"name": "eDP-2"}], Path(directory), Path(directory), 42)
            files = list(Path(directory).glob("*.png"))
            self.assertEqual([p.name for p in files], ["live-overlay-eDP-2.png"])
            with Image.open(files[0]) as image:
                self.assertEqual(image.size, (480, 145))


if __name__ == "__main__":
    unittest.main()
