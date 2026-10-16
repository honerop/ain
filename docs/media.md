# Multimedia exporters in Ain

Ain's native backend can export several kinds of media directly from structured
`Data` values without third-party language packages.

## PCM WAV synthesis

`wavwrite PATH SPEC` writes mono 16-bit PCM WAV audio. A SPEC record contains
`rate` (sample rate, default 44100) and a `notes` list. Each note is a
record with `hz` (frequency, default 440), `ms` (duration, default 250) and
`volume` (percentage, default 65).

```text
song = json "{\"rate\":8000,\"notes\":[{\"hz\":440,\"ms\":200},{\"hz\":660,\"ms\":200}]}"
saved = wavwrite "melody.wav" song
print asbool field saved "ok"
```

Validation includes 8–96 kHz sample rates, maximum 15 seconds of output,
maximum 500 notes and bounded note frequency/volume/duration. It synthesizes
sine waves with small fades, not SoundFont rendering or real-time audio.

## 2D SVG graphics

`svgwrite PATH SCENE` exports SVG drawings. Scene has integer `width`
and `height` (1..8192), and up to 1000 `shapes`. Currently supported
shape types are `rect` (x/y/width/height/fill), `circle` (cx/cy/r/fill),
and `text` (x/y/value/fill). Colors are validated, and text is XML-escaped.

```text
picture = json "{\"width\":100,\"height\":80,\"shapes\":[{\"kind\":\"rect\",\"x\":0,\"y\":0,\"width\":100,\"height\":80,\"fill\":\"#00ccff\"}]}"
print asbool field svgwrite "scene.svg" picture "ok"
```

## glTF 2.0 mesh export

`gltfwrite PATH MESH` exports a 3D triangle mesh to glTF 2.0 JSON with
embedded binary position/index buffers. The MESH record requires `positions`
as triples of numeric coordinates and `indices` as triangle vertex indices.

```text
mesh = json "{\"positions\":[[0,0,0],[1,0,0],[0,1,0]],\"indices\":[0,1,2]}"
print asbool field gltfwrite "triangle.gltf" mesh "ok"
```

Coordinates must be finite and within 1 million units, indices must address
defined vertices, and the mesh is limited to 20,000 positions and 100,000
indices. This is asset export, not a rendering engine or complete scene graph.

All three functions return a recoverable Data result with `ok` boolean,
`value` (written sample/shape/vertex count) and `error` text. They require
the `write` capability under optional Ain builtin restrictions and use the
native Rust code-generation backend. None of these features automatically
brings GTK/Chromium into unrelated programs.

Tests compile and execute generated programs, inspect the actual WAV RIFF
headers, verify XML escaping and glTF buffers/accessors, and reject malformed
input without creating output files.

## JSON-driven animated videos

See [Ain Video](../apps/video/README.md) for an end-to-end renderer written
in Ain. Its timeline engine uses `svgwrite` frame-by-frame; librsvg and
FFmpeg provide rasterization and MP4 encoding. The SVG exporter additionally
supports diagram lines (`x1,y1,x2,y2,stroke,stroke_width`) and text properties
(`font_size` and `anchor` set to `start`, `middle` or `end`).
