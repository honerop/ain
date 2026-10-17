# Native desktop layouts

Ain exposes portable native widgets through `gui`, `guiupdate`, and
`guitick`. The renderer uses GTK4 rather than embedding a web browser.
GTK4 runs on Linux, Windows and macOS when the matching native GTK4 libraries
and Rust linker toolchain are installed. The existing Nix package remains
Linux-specific. GUI builds and packaging on macOS/Windows have not yet been
verified by this repository's CI.

## Layout format

The `layout` argument is a newline-separated string. Each directive uses
tabs between fields. AIN strings may encode tabs with `\\t` and newlines
with `\\n`.

| Directive | Purpose |
| --- | --- |
| `row<TAB>classes<TAB>gap<TAB>grow` | Horizontal flex-like GTK box |
| `column<TAB>classes<TAB>gap<TAB>grow` | Vertical flex-like GTK box |
| `panel<TAB>classes<TAB>gap<TAB>grow` | Padded vertical group |
| `grid<TAB>classes<TAB>max-columns<TAB>gap` | Responsive wrapping GTK FlowBox |
| `end` | Close the last row/column/panel/grid |
| `text<TAB>classes<TAB>content` | Wrapping text with CSS classes |
| `spacer` | Flexible empty space |
| `css<TAB>GTK-CSS` | Window-scoped app CSS, use `\\n` for newlines |
| `theme<TAB>cyan` | Apply the built-in cyan theme |
| `conversation` | Scrollable message list; follow with `message` entries |
| `entry<TAB>id<TAB>placeholder` | Single-line text input |
| `action<TAB>id<TAB>label` | Button placed in its current container |
| `button<TAB>id<TAB>label` | Button in its current container, or default bottom action strip |
| `select<TAB>id<TAB>label<TAB>value<TAB>options` | Native dropdown |

The original `toolbar`/`endtoolbar`, `header`, `hint`, `log`, `image`,
`progress`, `separator` and `vim` directives still work.

The maximum nested depth is 32 levels. Gaps are clamped to 0–64 pixels,
grid column counts to 1–12. CSS class tokens must be alphanumeric,
underscore or hyphen (max 64 bytes). App CSS is limited to 64 KiB.
Unbalanced `end` directives do not pop the root or the active toolbar.

```ain
guiupdate "Layout demo" "theme\tcyan\ncss\t.ain-panel-custom { background: #203040; padding: 20px; }\ncolumn\tain-panel-custom\t12\tgrow\ntext\tain-heading\tHello\nrow\tactions\t8\nentry\tquery\tAsk something\naction\tsend\tSend\nend\t\ngrid\tchoices\t3\t8\nbutton\t1\tOne\nbutton\t2\tTwo\nbutton\t3\tThree\nend\t\nend\t\n"
print guitick 50
```

GTK CSS resembles web CSS in selectors, colors, borders, fonts and spacing,
but is **not HTML/CSS**: there is no DOM, CSS Grid, browser flexbox algorithm,
media queries, or JavaScript. `row`, `column`, and `grid` implement the
layout directly in GTK widgets. This avoids bundling a browser but does not
promise pixel parity with web pages.

## Redraw and scroll behavior

Calling `guiupdate` still rebuilds the widget tree, so it is not a
fine-grained VDOM. Before rebuilding, the runtime captures the transcript's
vertical adjustment. When GTK finishes measuring the new content, it restores
the saved position; when the transcript was near its bottom, it follows new
messages. This prevents Enter-to-send from jumping to the top. Calling
`guitick` does not rebuild widgets and preserves the current entry text.

## Platform setup

**Linux**: `nix develop` installs `gtk4` and `pkg-config`, or install your
distribution's GTK4 development package and run `cargo run -- build
examples/gui_layout.ain -o layout-demo`.

**macOS**: install Rust and Xcode Command Line Tools, then `brew install
pkg-config gtk4`. Build using `cargo run -- build examples/gui_layout.ain
-o layout-demo`. Bundle GTK libraries for distribution.

**Windows**: use the matching 64-bit GNU Rust toolchain and the MSYS2 UCRT64
`mingw-w64-ucrt-x86_64-gtk4`, `mingw-w64-ucrt-x86_64-pkgconf` and
`mingw-w64-ucrt-x86_64-toolchain` packages. Make the UCRT64 `bin` directory
visible to the build and executable, and run `ainc build
examples/gui_layout.ain -o layout-demo.exe`. The default compiler path for
built or temporary native programs uses `.exe` on Windows. App distribution
needs its GTK runtime DLLs and resources.

For a non-Nix application, keep `GTK4` and `pkg-config` visible to the
compiler. Linux-only packaging in a separate application's flake does not
automatically make that application portable.
