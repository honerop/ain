# Ain-only JSON video generator

An experimental video generator written entirely in Ain (apps/video/render.ain).
It reads JSON scenes and writes a real **YUV4MPEG2 (.y4m) raw video** with
Ain's existing JSON, math, iteration, string and file-writing builtins.

**No Python, FFmpeg, rasterizer, video library, subprocess, or changes to Ain's
compiler/runtime are needed.** Ain's existing native compiler is still used
to run Ain source.

Y4M is a standard raw-video format supported by video tools/players, but is
**not MP4**. Files are uncompressed and unsuitable for direct YouTube upload.
A high-quality compressed export cannot currently be produced through Ain's
existing text-only file API without a codec and binary-write support.

## Run

From the repository root:

~~~sh
nix develop
AIN_VIDEO_SPEC=apps/video/examples/explainer.json \
AIN_VIDEO_OUTPUT=explainer.y4m \
cargo run -- run apps/video/render.ain
~~~

The resulting explainer.y4m contains actual timed video frames. Many video
players support Y4M, though not all GUI players open it by default.

No external video encoder is called or required.

## JSON format

~~~json
{
  "width": 96,
  "height": 64,
  "fps": 8,
  "scenes": [
    {
      "frames": 8,
      "background": "black",
      "elements": [
        {
          "kind": "text",
          "value": "AIN VIDEO",
          "x": 8, "y": 7, "scale": 2, "fill": "white",
          "from": {"x": -20}, "ease": "smooth"
        },
        {
          "kind": "rect",
          "x": 9, "y": 42, "width": 76, "height": 3,
          "fill": "cyan", "from": {"width": 0}
        }
      ]
    }
  ]
}
~~~

Scenes run sequentially. Each scene has integer frames, or integer
duration in seconds if frames is absent. Object from/to keyframes animate
geometry over the full scene; ease is linear or smooth.

Supported types:

| kind | properties |
|---|---|
| rect | x, y, width, height, fill |
| circle | cx, cy, r, fill |
| text | x, y, value, scale (1..12), fill |

Text is rasterized using a small embedded 3x5 uppercase ASCII font, not a
system font. Supported characters are letters A-Z, numbers 0-9, spaces
and a handful of punctuation marks.

The default background is black. The fill and background keys map
common color names to different **luminance values**, not full RGB. This
first implementation is limited-color, largely grayscale video.

## Why Y4M?

Ain's current fswrite(path, text) writes UTF-8, not arbitrary bytes.
Y4M has an ASCII header and raw YUV pixels. By choosing only single-byte
ASCII values (16..126) as pixel samples, this program generates a legitimate
Y4M stream without changing Ain or invoking another program.

We deliberately do **not** pretend it produces H.264, H.265 or MP4.
Those require substantially more work and a general binary output API.

The encoder writes YUV 4:2:0 planes. Luminance is rasterized in Ain, while
chroma is held at ASCII byte value 126. This is a technical first
step, not a faithful full-color rendering pipeline.

## Current limits

- Even width and height, automatically clamped to 16..320 by 16..180 pixels.
- 1..24 FPS, at most 240 frames (silently shortened if JSON requests more).
- First version has no sound, clip import, native image assets, lines,
  LaTeX, antialiasing, or general scene transforms.
- Fully in-memory rendering through Ain strings, so long or large videos
  are slower and memory-intensive.
- Only valid, trusted JSON. The initial prototype has minimal input checks.

This makes the generator suitable for short raw-video proof-of-concept
animations, not yet finished 3Blue1Brown-length productions.

## Tests

~~~sh
cargo test --test video_generator
~~~

This test compiles Ain source and verifies the output Y4M header, frame sizes,
animated pixels, scene transitions and chroma planes. It does not require any
video tools.

## Later improvements

Add in-memory byte buffers and a binary file writer **only if necessary**,
keeping the video API in the Ain application. That would enable full-color Y4M,
uncompressed AVI or eventually a codec implemented in Ain without Python or
FFmpeg. Until then, keep this app independent of language changes.
