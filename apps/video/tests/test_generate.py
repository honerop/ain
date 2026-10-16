import copy
import json
import tempfile
from pathlib import Path
import unittest
from unittest.mock import patch

from apps.video.generate import embed_images, encode_video, validate


SAMPLE = {
    "width": 640,
    "height": 360,
    "fps": 24,
    "scenes": [{
        "frames": 3,
        "background": "#101825",
        "elements": [
            {"kind": "text", "x": 50, "y": 100, "value": "Test & demo", "font_size": 30},
            {"kind": "line", "x1": 0, "y1": 0, "x2": 100, "y2": 80,
             "stroke": "#abcdef", "from": {"x2": 20}, "ease": "smooth"},
        ]
    }],
}


class VideoValidationTests(unittest.TestCase):
    def test_valid_spec(self):
        self.assertEqual(validate(SAMPLE), (24, 3))

    def test_builtin_example(self):
        project = Path(__file__).resolve().parents[1] / "examples" / "explainer.json"
        self.assertEqual(validate(json.loads(project.read_text())), (24, 168))

    def test_rejects_odd_dimensions(self):
        spec = copy.deepcopy(SAMPLE)
        spec["width"] = 641
        with self.assertRaisesRegex(ValueError, "even"):
            validate(spec)

    def test_rejects_unbounded_duration(self):
        spec = copy.deepcopy(SAMPLE)
        spec["scenes"][0]["frames"] = 18001
        with self.assertRaises(ValueError):
            validate(spec)

    def test_rejects_injected_color(self):
        spec = copy.deepcopy(SAMPLE)
        spec["scenes"][0]["elements"][0]["fill"] = "red\" onload=\"evil"
        with self.assertRaisesRegex(ValueError, "color"):
            validate(spec)

    def test_rejects_bad_geometry(self):
        spec = copy.deepcopy(SAMPLE)
        spec["scenes"][0]["elements"][1]["to"] = {"x2": "100"}
        with self.assertRaisesRegex(ValueError, "integer"):
            validate(spec)

    def test_rejects_negative_radius(self):
        spec = copy.deepcopy(SAMPLE)
        spec["scenes"][0]["elements"].append(
            {"kind": "circle", "cx": 5, "cy": 5, "r": -4}
        )
        with self.assertRaises(ValueError):
            validate(spec)

    def test_noninteger_fps(self):
        spec = copy.deepcopy(SAMPLE)
        spec["fps"] = 29.97
        with self.assertRaises(ValueError):
            validate(spec)

    def test_embeds_local_png(self):
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            image = folder / "tiny.png"
            # A minimal header is sufficient to exercise the embedding path.
            image.write_bytes(bytes.fromhex("89504e470d0a1a0a") + b"test")
            spec = copy.deepcopy(SAMPLE)
            spec["scenes"][0]["elements"].append(
                {"kind": "image", "x": 0, "y": 0, "width": 25, "height": 25,
                 "path": "tiny.png"}
            )
            validate(spec)
            embedded = embed_images(spec, folder / "project.json")
            data = embedded["scenes"][0]["elements"][-1]
            self.assertTrue(data["data"].startswith("data:image/png;base64,"))
            self.assertNotIn("path", data)
            self.assertNotIn("data", spec["scenes"][0]["elements"][-1])

    def test_rejects_remote_image(self):
        spec = copy.deepcopy(SAMPLE)
        spec["scenes"][0]["elements"].append(
            {"kind": "image", "x": 0, "y": 0, "width": 25, "height": 25,
             "path": "https://example.com/image.png"}
        )
        with self.assertRaisesRegex(ValueError, "local"):
            validate(spec)

    @patch("apps.video.generate.subprocess.run")
    def test_ffmpeg_command_without_audio(self, run):
        encode_video(Path("/tmp/frames"), Path("/tmp/video.mp4"), 24, 48, None, None)
        argv = run.call_args.args[0]
        self.assertEqual(argv[0], "ffmpeg")
        self.assertIn("libx264", argv)
        self.assertIn("frame_%06d.png", " ".join(argv))
        self.assertNotIn("-filter_complex", argv)
        self.assertTrue(run.call_args.kwargs["check"])

    @patch("apps.video.generate.subprocess.run")
    def test_ffmpeg_audio_mix(self, run):
        encode_video(Path("/tmp/frames"), Path("/tmp/video.mp4"), 30, 90,
                     Path("/tmp/voice.wav"), Path("/tmp/music.mp3"))
        argv = run.call_args.args[0]
        filters = argv[argv.index("-filter_complex") + 1]
        self.assertIn("amix=inputs=2", filters)
        self.assertIn("atrim=duration=3.000000", filters)
        self.assertIn("-stream_loop", argv)


if __name__ == "__main__":
    unittest.main()
