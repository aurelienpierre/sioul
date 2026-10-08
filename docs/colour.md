# Colours on the screen, and calmer colours

A screen wider than sRGB, an Adobe RGB laptop panel or a P3 monitor, shows sRGB numbers as its own, wider primaries: every colour comes out more saturated than its page meant. Nothing between a site and such a screen converts today on X11: Chromium inside Qt WebEngine paints sRGB numbers, believing every screen is sRGB, and Qt Quick hands them to the screen unchanged. Sioul converts each site's picture to the screen's colour profile once, on the graphics card, and can soften loud colours further: "Calmer colours". The research behind this note, with its sources and measurements, is [research/colour.md](research/colour.md).

## What the person sees
Settings ▸ Display holds two lines, under the colours of the theme:
- **Colours for this screen**, a switch, on unless turned off (`screen_colours = false` in the configuration). Under it, one sentence says what this screen gets: its profile's own name ("This screen's profile is used: “…”."), or that the desktop adapts colours itself (Wayland, macOS), or that the screen has no profile and colours are shown as they are. Before the Sites page has been made in this session, the sentence is not there yet.
- **Calmer colours on sites**: Off, A little, More (`calmer_colours = "little"` or `"more"`). Loud colours on websites are softened; greys and soft tints stay as they are.

Both apply at once to every site and every site pop-up, without reloading a page. A phone shows neither line: its sites open in the browser.

## Who converts, where

| Where | The screen's colours | Calmer colours |
|---|---|---|
| Linux, X11 session, a profile published for the screen | Sioul, through the shader | yes |
| Linux, X11, no profile, or an sRGB one | nothing to do | yes |
| Linux, Wayland session (KWin, Mutter), Sioul on Wayland or on Xwayland | the compositor | yes |
| X11 with a colour-managing compositor (`_ICC_COLOR_DESKTOP` set on the root window) | the compositor | yes |
| macOS | macOS, Sioul's windows being tagged sRGB | yes |
| Windows | Windows when its Auto Color Management is on; otherwise nothing | yes |
| Android | — (sites open in the browser) | — |

"Never twice" is the rule: Sioul converts only where nothing else does. A Wayland session is recognised by the platform's name, by `XDG_SESSION_TYPE=wayland`, or by `WAYLAND_DISPLAY`, so that Sioul started on Xwayland inside a Wayland session does not convert either: KWin treats Xwayland's windows as sRGB and converts them, while a daemon such as `xiccd` may still publish `_ICC_PROFILE` on Xwayland's root.

## Where the profile comes from (X11)
- **The atoms.** The ICC Profiles in X specification (version 0.4) puts the first monitor's profile in the root window's `_ICC_PROFILE` property, the next ones in `_ICC_PROFILE_1`, `_ICC_PROFILE_2` and so on, as whole ICC files of 8-bit values. `xiccd` and colord-kde publish them from colord, and load each profile's calibration curves into the graphics card; Sioul reads the atoms and never writes anything. They are read whole: a profile with tables can weigh a megabyte, and Qt's own reading stops at 32 KB.
- **Which atom is which screen.** With one screen, `_ICC_PROFILE` is its own. With several, the daemons number atoms differently (`xiccd` writes only `_ICC_PROFILE`, for the primary screen; colord-kde numbers screens in the order of their CRTCs). colord knows each output, by the name X11 gives it (`eDP-1`), with the checksums of its profiles (`ProfileId` "icc-<MD5>", `FILE_checksum`): an atom goes to the screen whose profiles hold its MD5. Without colord, the specification's order: the primary screen first, then the others as Qt lists them. colord is read over the system bus, read only, with a 400 ms limit per question; the Flatpak may talk to it (`--system-talk-name=org.freedesktop.ColorManager`).
- **Never colord alone.** A profile that colord lists but that no daemon published may not have its calibration curves loaded, and would then describe another screen than the one shown.
- **Changes** are followed: a property change on the root window for any `_ICC_PROFILE*` atom (Qt already listens to the root window's properties), a screen plugged in or taken out, the primary screen changed. A window moving to another screen takes that screen's tables (`Screen.name` in QML). A window across two screens takes the one Qt says it is on, as every program does.
- **libxcb without its headers.** The atoms are read through Qt's own X11 connection and the libxcb that Qt's X11 platform has already loaded (`dlopen` with `RTLD_NOLOAD`, nine functions by name). Building Sioul needs no X11 development files: they are missing on some of the systems Sioul is built on.

## The two tables
`colour.rs` makes them once per screen and setting, in a few milliseconds:
- **The cube**: 33 × 33 × 33 entries over the sRGB numbers a page holds (steps of 1/32), packed as 33 tiles of 33 × 33 side by side, blue choosing the tile (an image of 1,089 × 33). Each entry holds the colour the screen must show as the screen's own **linear** values, **unclipped**: sRGB is decoded, softened when calmer colours are on, taken to XYZ (D50) with Little CMS's own sRGB colorants, then to the screen's linear RGB with the inverse of its colorant matrix. A colour just outside the screen's gamut keeps its small negative value, so that interpolating across the edge of the gamut stays smooth; the clamp comes after.
- **The curve**: 1,024 entries per channel turning linear values into the screen's numbers (Little CMS's inverse of each tone curve: gamma, parametric or a table), read at `u = y^(1/2.4)`, which spends the entries where the eye sees most.
- **Precision**: both are half floats (RGBA16F): a `QImage::Format_RGBX16FPx4`, which Qt Quick uploads as such. Eight bits per channel cannot hold negative values and erred by up to 15 codes.
- **Profiles of tables** (an `A2B`/`B2A` profile, with or without a matrix): Little CMS's own transform from sRGB fills the cube with the screen's numbers instead (made linear by u^2.4, so that the curve gives them back), with errors of a few codes near black and near clipped colours. Matrix profiles, by far the most common for screens, go the precise way above.
- **When nothing changes**: an sRGB screen (or one within a fifth of a code of sRGB) with calmer colours off gives no tables at all, and the page is drawn as before.

Measured against Little CMS's own float transform on 4,104 colours (`cargo test --release -p sioul-app colour`), as the graphics card computes it (half floats, bilinear reads): within one 8-bit code everywhere for a screen like Adobe RGB, and for one with 256-entry tone curves.

## Checked end to end, with no graphics card
`tools/check-colour.py` runs the whole path: Weston headless (pixman) with Xwayland (without glamor) gives an X screen nobody sees, inside bubblewrap with a network of its own and a `/dev` holding no graphics card at all (no `/dev/dri`, no `/dev/nvidia*`); Mesa's software OpenGL (llvmpipe) draws, its libraries forced through glvnd (`__GLX_VENDOR_LIBRARY_NAME=mesa`, Mesa's EGL vendor file), since another vendor's library made Xwayland crash with no device to open. The check writes a test profile on that screen's root window, Adobe RGB (1998) from its published primaries and gamma, never a real screen's; Sioul runs on a demo profile with a local page of twelve colour patches as its only site, and takes its grab steps "site-colour": a picture in the screen's colours, one with "A little" calmer colours too, one with both off. Each patch is read back and compared with what it should be, computed in floating point from the profile.

On 8 October 2026: the plain picture held the page's own numbers exactly; the screen's colours were within 0.56 of a code of what they should be, and calmer colours within 0.66. WhatsApp's green `#25d366` became 122, 210, 109 for that screen, and 148, 196, 141 with calmer colours; a grey and an off-white did not move.

## Calmer colours
Each colour's chroma in OKLCh (Björn Ottosson's OKLab, 2020) is limited softly, `C' = k · tanh(C / k)`, its lightness and hue kept: k = 0.12 for "A little", 0.08 for "More". A soft tint (chroma 0.03) keeps 99 % and 96 % of its chroma; a face (0.05 to 0.08) loses 8 % and 15 %; pure red (0.26) about half and two thirds; greys do not move. It happens before the screen's conversion, in the same cube, so it costs nothing more, and needs no colour profile: it is plain arithmetic, the same on every computer.

## The shader
- `shaders/colour.frag`, baked by `tools/make-shaders.sh` (Qt's `qsb`, with `--qt6`: SPIR-V, GLSL ES 1.00, 1.20 and 1.50, HLSL 5.0, MSL 1.2) into `colour.frag.qsb`, kept in the repository and in the resources (`shaders/shaders.qrc`, `qrc:/sioul/shaders/`): a build needs no shader tools. Run the script after changing the shader.
- Per pixel: the page's premultiplied colour is divided by its alpha; the cube is read twice, bilinearly within a tile and mixed between two tiles; the result is clamped; the curve is read three times; alpha is multiplied back.
- `qml/ColourEffect.qml` is a `ShaderEffect` with the tables as two hidden `Image`s from the image provider `image://sioul-colour/<cube|curve>/<screen>/<revision>`; `cpp/screencolours.cpp` (`ScreenColours`, a QML singleton) serves them.
- Each site's `WebEngineView` and each pop-up's sets `layer.enabled` while it is visible and `ScreenColours.active(Screen.name)` says its tables change something, and `layer.effect: ColourEffect {}`. Otherwise no layer: the page is drawn as it always was. The blur of covered chats stays on the view's holder, around it.
- **Cost, estimated, not measured**: one more pass over the view (the layer), and about ten texture reads per pixel. On an Intel HD Graphics 630 (Gen9, 26.4 Gtexel/s, 3.3 Gpixel/s, published figures), 3.5 to 6 ms per redraw for a 4K window, 1.5 to 2.7 ms for a Sites view of 2560 × 1440 device pixels; under 2 ms at 4K on a Gen12 Iris Xe. A page that does not change costs nothing: Qt Quick draws only on change. The layer holds one texture of the view's size (33 MB at 4K, only while visible); the tables take 295 KB. No figure was measured on a graphics card: see the research note for why, and for the short run that would measure it.

## Files
- `crates/sioul-app/src/colour.rs`: the tables, calmer colours, the profile's name, what Settings says; its tests against Little CMS.
- `crates/sioul-app/cpp/screencolours.{h,cpp}`: `ScreenColours`, the atoms, colord, the image provider.
- `crates/sioul-app/qml/ColourEffect.qml`, `crates/sioul-app/shaders/colour.frag` (`.qsb`, `shaders.qrc`), `tools/make-shaders.sh`.
- `tools/check-colour.py`, and the grab steps "site-colour" in `main.qml`.
- `SitesPage.qml` and `SitePopup.qml`: the layer on each web view, and the two settings given to `ScreenColours` (from `sioul.reading`).
- `cpp/webengine.cpp`: on macOS, Sioul's windows are tagged sRGB (`QSurfaceFormat::setDefaultFormat` with `QColorSpace::SRgb`), so that macOS converts them, sites included.
- `sioul-core`: `screen_colours` and `calmer_colours` in the configuration (`config.rs`), their two lines in Settings ▸ Display (`settings.rs`); the sentence under the first is added by `backend.rs`.
- Little CMS (`lcms2` 6.2, its sources built in with its `static` feature, on Linux only), so that no build needs the system's library.

## Limits
- A page's own pop-ups that Qt WebEngine shows in windows of their own (a `<select>` list) are not converted.
- Wide-gamut content (P3 photos, `color(display-p3 …)`) reaches Sioul already clipped to sRGB by Chromium.
- Sioul's own window around the sites keeps its sRGB numbers: its muted colours shift less than websites' (about half their chroma more on an Adobe RGB panel, just above what one notices). Converting the whole window through the same shader is the next step, once its cost has been measured on a real screen.
- macOS's tag is untested: no Mac was at hand.
- Windows: the screen's profile is not read (Windows' own colour management converts when it is on).
