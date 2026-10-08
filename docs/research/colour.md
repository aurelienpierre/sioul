# Colours on the screen: profiles, Chromium, Qt, and calmer colours

**The question**: how can Sioul show websites, in its Sites view (Qt WebEngine 6.11), converted from sRGB to the screen's ICC profile on X11 and Wayland without converting twice; and how can it soften loud colours further, optionally, before that conversion?

Researched on 8 October 2026, from source code (Chromium 140.0.7339.207 at chromium.googlesource.com; Qt 6.11.2 at code.qt.io; xiccd and colord-kde), specifications, merge requests and published crates, each named at its finding; a few facts come from news reports, marked as such. The measurements were made with a Python model of the transform, checked against Little CMS 2.16, and with Sioul's own tests against Little CMS 2.19. "Built" and "not built" say where Sioul stood on 8 October 2026. The feature: [colour.md](../colour.md).

## 1. What a wide screen does to sRGB numbers
A screen close to Adobe RGB (1998), as many photo-grade laptop panels are, reads sRGB numbers as its own, wider primaries. Modelled with Adobe RGB (1998)'s published primaries and its 563/256 gamma, and measured in OKLab (ΔE ×100; about 2 is just noticeable):

| Colour | Chroma | Hue | ΔE ×100 |
|---|---|---|---|
| WhatsApp's green `#25d366` | +40 % | 150° → 159° | 9.9 |
| Pure red `#ff0000` | +12 %, lighter | | 8.0 |
| Orange `#ff8c00` | +16 % | 58° → 53° | 5.3 |
| Google's blue `#4285f4` | +12 % | 260° → 251° | 4.0 |
| Sioul's accent `#4c6b5c` | +44 % | 163° → 171° | 2.3 |
| Sioul's background `#f5f2ec` | +7 % | | 0.2 |

A real laptop panel profiled at 2.2 gamma gave close figures (+43 % for the green, +49 % for Sioul's accent). sRGB's pure blue lies slightly outside such a panel's blue: a few blues clip.

## 2. Chromium and Qt WebEngine on Linux
- **Qt WebEngine 6.11.2 bundles Chromium 140.0.7339.**
- **Chromium inside Qt WebEngine believes every screen is sRGB.** Qt WebEngine builds Chromium's screens itself (`src/core/desktop_screen_qt.cpp`, `toDisplayDisplay`): a `display::Display` from each `QScreen`'s geometry, depth and scale, never a colour space. Chromium's constructor then gives it the default colour space, sRGB unless `--force-color-profile` is passed (`ui/display/display.cc`, `GetDefaultDisplayColorSpacesRef`). `render_widget_host_view_qt.cpp` reuses the same function for what pages learn of their screen.
- **Chrome reads the X11 profile; Qt WebEngine does not use that code.** Chrome on X11 reads `_ICC_PROFILE` and `_ICC_PROFILE_n`, n being the RandR monitor index, and keeps only the profile's primaries with an sRGB tone curve (`ui/base/x/x11_display_util.cc`, `ICCProfile::GetPrimariesOnlyColorSpace` in `ui/gfx/icc_profile.cc`). That is Chrome's own X11 platform, which Qt WebEngine replaces.
- **`--force-color-profile` names no ICC profile and no Adobe RGB.** Chromium 140 accepts `srgb`, `display-p3-d65`, `rec2020`, `scrgb-linear`, `hdr10`, `extended-srgb`, `generic-rgb` and `color-spin-gamma24`, else logs an error and keeps sRGB (`ForcedColorProfileStringToColorSpace`, `ui/display/util/display_util.cc`).
- **What reaches Qt Quick** (a probe off the screen, software rendering, no network): a page's sRGB numbers come back unchanged (`#25d366` as 37, 211, 102), and `color(display-p3 0 1 0)` comes back as 0, 255, 0: wide-gamut colours are clipped to sRGB before Qt sees them. With `--force-color-profile=display-p3-d65` the software path gave the same numbers; the graphics card's path was not tried.
- **So**: exactly one conversion is missing on X11, after Chromium and before the screen.

## 3. Qt 6.11's own colour management
- **X11: read, unused.** Qt's xcb platform reads the root window's `_ICC_PROFILE` (or the EDID's chromaticities, or sRGB) into `QPlatformScreen::colorSpace()` (`qxcbscreen.cpp`, `updateColorSpaceAndEdid`; added by commit fdc7eb80 on 25 January 2021, "for macOS and X11 screens"). It reads the root atom for every screen, 32 KB at most, and nothing draws with it; `QScreen` has no public colour space in 6.11. QTBUG-90535, "Missing support for color management of widgets", reported on 22 January 2021, was still open on 21 November 2025.
- **X11's sRGB surface** only asks GLX for an sRGB-capable framebuffer (`qglxconvenience.cpp`): blending, not conversion.
- **Wayland**: a window whose requested `QSurfaceFormat` carries a `QColorSpace` is tagged with a `wp_color_management_v1` image description (`qwaylandcolormanagement.cpp`, `qwaylandwindow.cpp`); untagged, the compositor takes it as sRGB.
- **macOS**: without a requested colour space, the Metal layer gets the window's own colour space, the screen's (`qnsview_drawing.mm`): Qt Quick's numbers reach the screen unconverted. Asking for `QColorSpace::SRgb` lets macOS convert them.
- **The scene graph** has no colour management. It uploads `QImage::Format_RGBX16FPx4` images as RGBA16F textures (`qsgplaintexture.cpp`). `Image.colorSpace` (Qt 5.15) converts only images that carry a colour space: one without, as most PNG icons and every SVG one, is only labelled (`qquickpixmapcache.cpp`), so it cannot convert an application's icons.
- **`QColorSpace`** reads matrix profiles (preferring the matrix when a profile also has tables) and, for profiles of tables, their perceptual tables A2B0/B2A0 only (`qicc.cpp`); no rendering intent, no black point compensation.

## 4. Where the profile comes from
- **ICC Profiles in X Specification 0.4** (Kai-Uwe Behrmann, revisions of 2008 to 2010, freedesktop.org's wiki; read through the Wayback Machine): the first monitor's profile is the root window's `_ICC_PROFILE`, the next ones `_ICC_PROFILE_1`, `_ICC_PROFILE_2`…, counted as Xinerama counts; the whole ICC file in 8-bit CARDINALs; a monitor without one is uncharacterised and shown as sRGB; programs should watch the root window's property changes.
- **Who publishes them.** `xiccd` (commit 177b9fe1, 6 August 2025, `src/randr-conn-private.c`) writes `_ICC_PROFILE` only, for the "main" display (the primary, else the laptop's panel, else the first lit one), and loads its calibration curves. colord-kde (`colord-kded/ColorD.cpp`, 9 January 2024) writes `_ICC_PROFILE` and `_ICC_PROFILE_n` per CRTC; it means to number the primary screen first, but its loop renumbers copies (`for (auto monitor : …) monitor.atomId = atomId++`), so the numbers stay in CRTC order. Both may run in one session: with one screen they agree, with two they may not.
- **colord** knows each output as a device with the property `XRANDR_name`, its profiles listed with the default first; each profile's `ProfileId` is "icc-" and the MD5 of its file, and its metadata hold `FILE_checksum`. On the machine studied, the root atom's bytes were the file's bytes, MD5 included: matching an atom to a screen by checksum works.
- **KDE**: on X11, KWin does no colour management, and colord-kde says in its code that Wayland is not supported. On Wayland, KWin keeps a profile per output in its own settings, and converts every window.

## 5. Wayland, and converting twice
- **wayland-protocols 1.41** (17 February 2025) brought `color-management-v1` into staging, merged on 13 February 2025 after five years of review (Phoronix).
- **KWin** applies an ICC profile set in the display settings to every window since Plasma 6.0 (merge request !4555, merged 25 October 2023), exposed a colour-management protocol by default in Plasma 6.2 (!6255, 15 August 2024), and moved to the upstream `wp_color_management_v1` in Plasma 6.3 (!7139, 18 February 2025) (invent.kde.org).
- **GNOME**: Mutter 48 (March 2025) speaks `wp_color_management_v1`; Mutter 49 (September 2025) applies ICC profiles on Wayland (Phoronix, through search).
- **Consequence**: on Wayland the compositor converts, KWin 6.0 to 6.2 even without the upstream protocol, so the protocol's absence proves nothing. Sioul never converts to the screen there, nor on Xwayland inside a Wayland session, where `xiccd` may still publish an atom.
- **Windows**: Auto Color Management (Windows 11 22H2 and later, qualifying displays, WDDM 3.0; Microsoft's DirectX blog, 12 October 2022) converts programs that are not colour-aware from sRGB when it is on; it is off on most computers.

## 6. Colour libraries for Rust (crates.io, 8 October 2026)
- **lcms2 6.2.0** (26 August 2026, MIT) over **lcms2-sys 4.0.7** (25 June 2026, MIT): Little CMS, the reference: float transforms, every intent, black point compensation, every profile kind. lcms2-sys finds the system's library by pkg-config, or builds its bundled sources (Little CMS 2.19) with `cc`; its `static` feature always builds them. Little CMS is MIT, compatible with the GPL.
- **qcms 0.3.0** (9 January 2024, MIT, Firefox's): 8-bit pixels only, and `transform_create` ignores the intent (its parameter is `_intent`): it cannot build a float table.
- **moxcms 0.9.1** (15 September 2026, BSD-3-Clause or Apache-2.0, pure Rust, minimum Rust 1.89): the `image` crate's colour engine, with float transforms, intents and OKLab; no black point compensation; its interface changed in March and July 2026.
- **Chosen**: lcms2, built from its own sources on Linux only, where Sioul converts.

## 7. The lookup table
Measured on a million random colours and a 64-step cube, against an exact float model (itself within Little CMS's 8-bit rounding, 2.3 codes at most):
- A table holding the screen's **numbers** errs by up to 6 codes on saturated reds: the clip of a channel at the gamut's edge is interpolated, then magnified by the 1/2.2 curve. With calmer colours, up to 4 codes near black.
- A table holding the screen's **linear values, unclipped**, clamped and passed through a curve after interpolation, errs by 0.63 codes at most at 33³ in half floats: 84 % of colours exact after rounding, the others one code off. 17³: 3.5 codes. 65³: no visible gain, nearly eight times larger (2.2 MB, more than a Gen9 graphics slice's 768 KB of L3 cache).
- Eight-bit storage cannot hold negative values: up to 15 codes. Half floats are needed.
- Tetrahedral interpolation brings nothing over trilinear here.
- The curve, 1,024 half floats read at `u = y^(1/2.4)`, errs by 0.06 codes for gamma and sRGB curves, 0.27 for 256-entry table curves (256 entries: 1.1).
- In Sioul's tests (`colour.rs`), as the graphics card computes it, against Little CMS 2.19's float transform on 4,104 colours: within one code for a screen like Adobe RGB and for one with 256-entry curves.
- End to end (`tools/check-colour.py`: the shader drawn by Mesa's software OpenGL on an X screen nobody sees, with no graphics card in reach, a test profile compatible with Adobe RGB (1998) on its root window): within 0.56 of a code for the screen's colours, 0.66 with calmer colours; the plain picture exact.

**Built**: 33³ unclipped linear values in RGBA16F, packed in two dimensions (Qt Quick's `ShaderEffect` binds 2D textures only), trilinear, then the curve.

## 8. Calmer colours
The request was to soften websites, optionally, by "reducing the gamut", for example by changing the profile. Claiming wider primaries in a profile would make a CMM send less saturated numbers, but in linear light: saturated colours would change lightness, and blues would drift towards purple. Limiting chroma in OKLCh (Björn Ottosson, "A perceptual color space for image processing", 23 December 2020) keeps lightness and hue, writes no profile, and goes into the same table at no cost. A soft limit, `C' = k · tanh(C / k)`, softens loud colours most and leaves soft ones nearly alone: with k = 0.12 ("A little"), a tint of chroma 0.03 keeps 99 % of it and a face (0.05 to 0.08) loses about 8 %, while pure red (0.26) keeps about half; with k = 0.08 ("More"), 96 %, 15 % lost, and a third. On a wide screen, pure red's numbers go from (216, 27, 0) to (174, 108, 95) with "A little". **Built**, with two steps.

## 9. Cost on the graphics card (estimated, not measured)
- No figure was measured on a graphics card. A first benchmark sent batches of full-screen draws long enough for the Intel driver to reset the card (i915's preemption timeout, 640 ms); measuring stopped there, and the estimate below comes from published figures.
- Intel HD Graphics 630 (Gen9 GT2): 24 texture units, 3.3 Gpixel/s, 26.4 Gtexel/s, 422 GFLOPS; Iris Xe G7 with 96 EUs (Gen12): 26.4 Gpixel/s, 52.8 Gtexel/s (TechPowerUp's database, through search).
- The effect adds a pass to draw the view into a layer (one read, one write per pixel) and about ten texture reads per pixel (two bilinear half-float reads of the cube and three of the curve, half-float filtering counting double on Gen9).
- At 3840 × 2160: about 3.5 ms of texture work and 2.5 ms of pixel writes, partly overlapping: **3.5 to 6 ms per redraw on Gen9**, 10 to 18 % of a frame at 30 frames per second, as during a video call; 1.5 to 2.7 ms for a view of 2560 × 1440 device pixels; under 2 ms at 4K on Gen12. With a pure gamma tone curve, the three curve reads could become one `pow()`. Memory: one texture of the view's size (33 MB at 4K) while visible, and 295 KB of tables. A page that does not change costs nothing.
- **To measure it**, a short run on a real screen: Sioul with `QSG_RENDER_TIMING=1` on a local page animating a canvas at 30 frames per second, twenty seconds with the conversion and twenty without, comparing the render times Qt prints.

## 10. What stays open
- **Sioul's own window**: its colours are sRGB numbers too (Sioul's accent gains 44 % chroma on an Adobe RGB screen, just above what one notices), and Breeze's icons keep their own blues and reds. Converting theme colours alone would leave icons, photos and maps unconverted (section 3); the whole window through the same shader would cover everything, at the cost above for every redraw. Not built: it waits for a measurement on a real screen.
- **Wide-gamut content**: forcing Chromium's output to Display P3 and converting from P3 could keep it; untested.
- **Windows**: reading the profile (`GetICMProfileW`) when Windows' own colour management is off. Not built.
