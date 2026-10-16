# Ain Video: JSON animation to MP4

A small, usable video pipeline built **with Ain**. It is aimed at mathematical
explainers, animated technical diagrams and scripted faceless videos.

The timeline engine, scene selection, easing and keyframe interpolation are
written in [render.ain](render.ain). Ain's native `svgwrite` exporter creates
one vector frame at a time. A narrow Python standard-library CLI validates the
JSON, invokes Ain, rasterizes SVG with `rsvg-convert`, and muxes MP4/audio with
FFmpeg. There is no dependency on MoviePy, Manim, a browser or Python graphics
packages.

It is intentionally an early subset of those tools, not a replacement for them.

## Use

From the **repository root** on NixOS:

```sh
nix develop
# Nix shell must include ffmpeg and librsvg as well as rustc/cargo.
python3 apps/video/generate.py apps/video/examples/explainer.json explainer.mp4
```

Or on other Linux systems, install `cargo`, `rustc`, `ffmpeg` (with
`libx264`) and `rsvg-convert` from librsvg, then use the same command.

With voiceover and music:

```sh
python3 apps/video/generate.py movie.json movie.mp4 \
  --voiceover narration.wav --music music.mp3
```

The JSON may also contain `"voiceover": "narration.wav"` and
`"music": "music.mp3"`. Those files are resolved relative to the JSON.
Music is looped to video length and mixed at 18% amplitude below narration.
The same low gain is applied to music-only projects. Audio is trimmed/padded
to the exact video duration.

Use `--keep-frames` to copy all SVG and PNG frames next to the MP4 for
debugging. Normally temporary files are removed after export.

## Example

```json
{
  "width": 640,
  "height": 360,
  "fps": 24,
  "scenes": [
    {
      "duration": 2,
      "background": "#101825",
      "elements": [
        {
          "kind": "text", "value": "Input becomes output",
          "x": 320, "y": 95, "font_size": 32,
          "anchor": "middle", "fill": "white"
        },
        {
          "kind": "circle", "cx": 450, "cy": 210, "r": 28,
          "fill": "#00d4ee", "ease": "smooth",
          "from": {"cx": 80}
        }
      ]
    }
  ]
}
```

Coordinates are pixels from the top-left. Text `y` is its baseline. Scene
length is either integer `duration` in seconds or integer `frames`;
`frames` takes precedence. Scenes are concatenated with hard cuts.

Element kinds:

| Kind | Required | Optional |
|---|---|---|
| `rect` | `x, y, width, height` | `fill` |
| `circle` | `cx, cy, r` | `fill` |
| `line` | `x1, y1, x2, y2` | `stroke, stroke_width` |
| `text` | `x, y, value` | `fill, font_size, anchor` |

All shape kinds can set `from` and/or `to` as geometry overrides. The
Ain renderer interpolates each numeric property over the entire scene:

- `from` supplies the starting coordinates/sizes, defaulting to base fields.
- `to` supplies the ending coordinates/sizes, defaulting to base fields.
- `ease` may be `linear` (default) or `smooth` (integer smoothstep).

Interpolation is frame-accurate and deterministic, with the first frame at
`from` and last frame at `to`. Values are integers. Geometry is animated
without tweening text or fill colors. The SVG backend safely escapes text.

## Limits

- 2..8192 px resolution per axis, even dimensions; 1..60 FPS.
- Maximum 18,000 frames, 1,000 scenes, and 999 objects per scene.
- Uses the **Rust-native Ain backend**, not the LLVM or C bootstrap.
- No camera moves, LaTeX typesetting, path morphing, gradients, images, masks,
  text layout or real-time preview yet.
- PNG rasterization for every frame is simple but not especially fast.
- Ain compilation is performed on each invocation; suitable for CLI jobs,
  not interactive editing.
- Input is local and trusted. Ain's capability checks are not a sandbox.

## Test

```sh
python3 -m unittest discover -s apps/video/tests -v
cargo test --test video_generator
```

The Rust test compiles the Ain renderer to catch syntax and type issues.
The Python tests check input validation and FFmpeg command setup. An actual
video export additionally needs librsvg and FFmpeg installed locally.

## Where to extend

For a more Manim-like editor, next add a richer scene graph (paths, transforms,
camera, reusable groups), object-level timing, math text using LaTeX/SVG,
asset/image support and GPU or streamed rasterization. Keep scene timing in Ain;
keep hardware video encoding in a specialized native process.
