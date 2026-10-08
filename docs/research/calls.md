# Calls in sites: choosing and changing the camera, the microphone and the speaker

**The question**: how can Sioul, whose Sites view is Qt WebEngine 6.11, give a site's call the camera, the microphone and the speaker you chose, and change them during the call, so that the site's own call switches at once, without breaking what the site does with its tracks?

Researched on 8 October 2026, from source code (Qt WebEngine 6.11 at code.qt.io; Chromium 140, branch-heads/7339, at chromium.googlesource.com; Jitsi Meet and BigBlueButton at github.com), specifications and MDN, the help pages of closed-source call sites, and measurements in a lab. The lab was a small Qt WebEngine 6.11.2 program, running offline in bubblewrap, with a fresh `/dev` (no camera, no sound card) and no sound server, on Chromium's fake devices. Its stand-in site makes a loopback call: two peers in one page, a preview of its own stream, its own references to its tracks, and optionally a canvas effect, a processor effect and a voice meter. A private PipeWire stood in for the sound system, with virtual devices only. "Built" and "not built" say where Sioul stood on 9 October 2026. The feature: [sites.md](../sites.md), "The devices of calls".

## 1. What Sioul did until 8 October 2026
- **The choice was read only when a page first asked for a device.** A script in every page wrapped `getUserMedia` and added `deviceId: { ideal: … }`, found by the device's name, when the page named no device. A change only replaced the script's list of names, so a running call kept its tracks. The lab reproduced it: after a change, the senders, the preview and the remote picture stayed on the old camera and microphone.
- **A site's first call ignored the choice.** Before a site is allowed a device, Chromium gives no labels, so the name could not be found and the call opened the default device.
- **The speaker missed elements that play by themselves.** It was set only in `play()` and in new audio contexts, and an `autoplay` element given a `srcObject` never calls `play()`. Many call sites play the other person's voice this way.
- **The change reached only main frames,** never calls in a frame of another site (`runJavaScript` runs in the main frame), and never pop-up windows.
- **A page reloaded after a change started with the old names.** The view's scripts were given once.
- **The names went to every site,** those allowed no device too, in an event any page could listen to.

## 2. Qt WebEngine 6.11
- **Qt WebEngine 6.11 is Chromium 140** ("Qt WebEngine is now based on Chromium 140", What's new in Qt 6.11, doc.qt.io, read 8 October 2026).
- **No way to choose or change a site's devices.** `src/core/media_capture_devices_dispatcher.cpp` (branch 6.11) gives the device the page asked for, else the first of Chromium's list: `getDefaultDevices(requested_audio_device_ids.front(), requested_video_device_ids.front(), …)`, `findDeviceWithId`, else `audioDevices.front()` and `videoDevices.front()`. A permission request (`QWebEnginePermission`, since 6.8) is only granted or denied. The public 6.11.2 headers have no device, speaker or "capturing" API; `QWebEngineDesktopMediaRequest` covers screens and windows only. So the choice must be made inside the page.
- **Frames can be reached.** Since 6.8, `WebEngineView.mainFrame`, `WebEngineFrame.children` (6.10 in QML) and `frame.runJavaScript(script, worldId, callback)` reach every frame. In the lab, a call inside a frame from another host changed only when the event was sent this way.
- **Workers cannot carry the call.** In 6.11.2, a camera track cannot be sent to a worker (`DataCloneError`), and a dedicated worker has no `MediaStreamTrackProcessor`, `VideoTrackGenerator` or `MediaStreamTrackGenerator`. Pages have `MediaStreamTrackProcessor` and `MediaStreamTrackGenerator`, but not `VideoTrackGenerator`.
- **The old callback forms** `navigator.getUserMedia` and `navigator.webkitGetUserMedia` are still in Chromium 140; they went around a wrapper of `MediaDevices.getUserMedia`.

## 3. The system's default (Linux)
- **Chromium opens the "default" device with no name.** Chromium 140's `media/audio/pulse/pulse_util.cc` opens it with `pa_stream_connect_record(…, nullptr, …)`, and the same for playback, without `PA_STREAM_DONT_MOVE`. `audio_manager_pulse.cc` puts a generic "default" first in the list, "so PULSE_SOURCE redirection works". In Qt WebEngine the default entries have an empty label.
- **A call on the default follows the system's default at once.** With PipeWire 1.6.9 and WirePlumber 0.5.18 in the lab, changing the default source moved the live capture within 1.5 s, and the page's track stayed live. The speaker followed too. PulseAudio has done the same for sinks since 14.0: "Now streams are moved from the old default sink to the new one as well. This doesn't apply to streams that have been manually moved" (PulseAudio 14.0 release notes, wiki.freedesktop.org, read 8 October 2026).
- **A call on a named device stays on it**, and so does a stream moved by hand. Jitsi Meet passes the real id of the default microphone (below), so its calls do not follow.
- **The site is never told.** No `devicechange` event fired in the lab, neither for a new virtual source nor for a change of default: Chromium hears about devices on Linux from udev, that is from real plugs.
- **Moving a stream from outside works.** The capture stream is named "Chromium input", process `QtWebEngineProcess`, and playback streams carry the application's name. Sioul could move them itself, on Linux only; not built (§6).
- **Cameras have no system default** (V4L2), so they must be changed in the page.
- **Windows and macOS were not examined.**

## 4. Changing a device inside the page: two ways compared in the lab
- **A. Relay (built).** The page gets, for each camera or microphone track, a `MediaStreamTrackGenerator` fed from the real device's track through a `MediaStreamTrackProcessor`. A change feeds it from a new real track.
- **B. Plain `replaceTrack`.** The new track replaces the old one in the senders and in the streams `getUserMedia` returned, and the old one is stopped. MDN, `RTCRtpSender.replaceTrack()` (last modified 4 July 2024): "Most track replacements can be done without renegotiation".

| Case | A | B |
|---|---|---|
| Senders, preview and remote picture follow the camera; the microphone follows | yes | yes |
| The site's own mute (`enabled = false` on its reference) | works | no effect: it acts on the dead old track |
| The site's voice meter (Web Audio on its stream) after a microphone change | reads on | reads 0 |
| Canvas effect (hidden video, canvas, `captureStream`) | follows | follows |
| Processor effect bound to the site's track | follows | ends with the old track |
| Hanging up with the site's own references | every device closed | the new camera and microphone stay open |
| Echo cancellation, noise suppression and gain after a change | kept | — |

- **Why B fails the meter.** The Web Audio API (1.1 Editor's Draft, 9 September 2026, §1.24.1) binds a `MediaStreamAudioSourceNode` to the first audio track of the stream at construction: "After construction, any change to the MediaStream that was passed to the constructor do not affect the underlying output of this AudioNode." A site's voice gate on it would cut your voice.
- **Why B leaves the devices open.** MDN, `MediaStreamTrack.stop()` (last modified 25 December 2023): the `ended` event "will not be fired" when a script stops a track, so the site never learns that its old track died, and at hang-up it stops only the tracks it knows.
- **A preview follows a swap.** Chromium's `<video>` playing a stream reloads its renderer when the stream's tracks change (`WebMediaPlayerMS::TrackAdded`, `TrackRemoved`, `ReloadVideo`, Chromium 140), which is why both A and B update the preview.
- **What A costs**, measured with 640 × 480 at 20 fps, over 15 to 20 s loopback calls, on a machine loaded by other builds:
  - On an idle page, nothing measurable: 0 % concealed sound, 29 ms jitter-buffer delay, 20 fps.
  - When the page's main thread is busy 30 % of the time, sound delay rises from 29 to 52–61 ms, and the video stays at 20 fps.
  - When it is busy 60 % of the time, sound delay is 56–59 ms instead of 31; the video held 20 fps in four runs out of five, and fell to 10–11 fps in one.
  - The page's process used about 0.8 s more processor time per 20 s of call, some 4 % of one core.
- **How a site can see A.** `track.constructor.name` is "MediaStreamTrackGenerator". `ImageCapture.takePhoto()` fails on it ("setPhotoOptions failed") while `grabFrame()` works, so Sioul makes an `ImageCapture` on the real track.
- **`MediaStreamTrackGenerator` is Chromium's own API.** MDN (last modified 17 April 2025) marks it "Experimental" and "Non-standard": "Consider using VideoTrackGenerator instead". Mozilla's WebRTC blog (Jan-Ivar Bruaroey, 20 September 2024) notes that Chrome's versions are on the main thread only and also handle sound, while the standard ones are worker-only and video-only. No removal date was found.

## 5. How sites differ
- **Jitsi Meet** (`conference.js`, master, last changed 17 September 2026) has its own in-call device menu (`onAudioDeviceChanged`). It asks for a new track and calls its own `replaceTrack`. On `devicechange` it stops its streams and asks again: "Otherwise GUM will return a stream using the old default device". For the "default" microphone it passes the real device's id instead of "default" (crbug 997689). Its noise suppression is an effect on its track.
- **BigBlueButton** (`bigbluebutton-html5/imports/api/audio/client/bridge/base.js`, v3.0.x-release, last changed 17 January 2025) mutes by setting `enabled` on the tracks of `peer.getSenders()`. Its own change of device (`liveChangeInputDevice`) clones the input stream as a backup, stops its tracks and asks again with the device chosen.
- **Google Meet** has pickers next to its camera and microphone buttons (Google Meet Help, "Connect your video & audio", read 8 October 2026). **Teams on the web** has a Device settings panel for use before or during meetings (Microsoft 365 message MC1234663, 19 February 2026). **Discord** in a browser has a "Default" input. All three are closed source: how they hold their tracks is unknown, and a real call on each is still to be tried.
- **Whereby** rooms are often embedded in a frame of another site: the frame case above.
- **Signal** has no web client: Signal Desktop has been a standalone app since its Chrome app was retired (signal.org, "Standalone Signal Desktop", 31 October 2017).

## 6. What was decided
- **Built: A, the relay, for the camera and the microphone, on every system,** and B only as a fallback should Chromium drop `MediaStreamTrackGenerator` from pages. This is decision 1 of 8 October 2026.
  - *Not built*: moving Sioul's own sound streams at the system level on Linux (PipeWire or PulseAudio). It would cost no delay, but works only on Linux and not for the camera.
- **Built: at a call's start Sioul's choice wins; during a call, a device the site's own menu asks for is kept** (the latest choice wins). With "The system's own", the site's choice stands.
- **Built: the call button,** shown only while a site or a pop-up it opened has a call. A change applies to every open call and to the next ones.
- **Built: "The system's own" stays the default,** and the help text says that it follows the system.
- **Built: the faults of §1 fixed.**
  - The first call moves to the chosen device right after the first grant.
  - The speaker reaches elements given a `srcObject`.
  - A change reaches every frame and pop-up.
  - Scripts are given again after a change.
  - Only sites allowed a device receive the names.
  - The old callback forms are routed through the same code.
- **Built: what fails is said.** The script tells Sioul "call", "end", "changed" and "kept" through the console, marked with a secret of the run. A device that cannot be opened is said in the line above the site, with the names Sioul has.
- **Built: `tools/check-sites.py devices`,** on Chromium's fake devices in a sandbox where no real camera or microphone can be opened: 26 checks, all passing on 9 October 2026.
- **Not built:**
  - a per-site switch to let a site handle its devices itself, until a site needs it;
  - the PipeWire check, which stays in the lab and out of the regular checks.
- **Not done yet:** a real call (Jitsi, between a computer and a phone; Meet and Teams if possible), changing the camera and the microphone from the call button, then hanging up.

## Sources
- Qt WebEngine 6.11: `src/core/media_capture_devices_dispatcher.cpp`, <https://code.qt.io/cgit/qt/qtwebengine.git/tree/src/core/media_capture_devices_dispatcher.cpp?h=6.11>, read 8 October 2026; What's new in Qt 6.11, <https://doc.qt.io/qt-6/whatsnew611.html>, read 8 October 2026; the 6.11.2 headers (`qwebengineframe.h`, `qwebenginepermission.h`, `qquickwebengineframe_p.h`).
- Chromium 140 (branch-heads/7339), read 8 October 2026: `media/audio/pulse/pulse_util.cc`, `media/audio/pulse/audio_manager_pulse.cc`, `third_party/blink/renderer/modules/mediastream/web_media_player_ms.cc`, `media/capture/video/fake_video_capture_device_factory.cc`, at <https://chromium.googlesource.com/chromium/src/>.
- PulseAudio 14.0 release notes, <https://wiki.freedesktop.org/www/Software/PulseAudio/Notes/14.0>, read 8 October 2026.
- Web Audio API 1.1, Editor's Draft, 9 September 2026, §1.24.1, <https://webaudio.github.io/web-audio-api/>.
- MDN: `RTCRtpSender.replaceTrack()` (4 July 2024), `MediaStreamTrack.stop()` (25 December 2023), `MediaStreamTrackGenerator` (17 April 2025), <https://developer.mozilla.org/>.
- Jan-Ivar Bruaroey, "Unbundling MediaStreamTrackProcessor and VideoTrackGenerator", Mozilla WebRTC blog, 20 September 2024, <https://blog.mozilla.org/webrtc/unbundling-mediastreamtrackprocessor-and-videotrackgenerator/>.
- Jitsi Meet, `conference.js`, <https://github.com/jitsi/jitsi-meet/blob/master/conference.js>, read 8 October 2026.
- BigBlueButton, `bigbluebutton-html5/imports/api/audio/client/bridge/base.js`, <https://github.com/bigbluebutton/bigbluebutton/tree/v3.0.x-release>, read 8 October 2026.
- Google Meet Help, "Connect your video & audio", <https://support.google.com/meet/answer/10409699>, read 8 October 2026.
- Microsoft 365 message center, MC1234663, "New device settings experience in Teams on the web", 19 February 2026, <https://mc.merill.net/message/MC1234663>.
- Signal, "Standalone Signal Desktop", 31 October 2017, <https://signal.org/blog/standalone-signal-desktop/>.
