# Sounds to focus or to rest by

The status line has a sound button, never on by itself: noise **to focus** (white, pink, brown), **nature to rest by** (waves on a beach, rain, wind in the trees, crickets at night, a distant storm), both made by Sioul, and **your own recordings**. One at a time, looping without a seam, fading in and out over five seconds.

## What the research says
- **Noise helps attention a little, for some.** White or pink noise gave a small benefit on attention tasks for people with ADHD or many ADHD traits (g = 0.25) and a small cost for others (g = −0.21) across 13 studies (Nigg et al. 2024, *JAACAP*, doi:10.1016/j.jaac.2023.12.014). No study tested brown noise. Hence "to focus", and offered, not pushed.
- **Nature sounds calm better.** Recovery after stress tended to be faster with water and birdsong than with noise (Alvarsson, Wiens & Nilsson 2010, doi:10.3390/ijerph7031036); across studies, natural sounds lowered stress and annoyance and improved mood, water most for mood (Buxton et al. 2021, *PNAS*, doi:10.1073/pnas.2013097118). Hence "to rest", with recordings.

## How it works
- **Noise is made here** (`sioul_core::sounds`): a minute of white noise, of pink noise (Paul Kellett's filter, −3 dB per octave) and of brown noise (a leaky integration, −6 dB per octave), quiet (about −20 dBFS), its last two seconds faded into its first with equal power, so the loop has no seam. Written once as WAV in `$XDG_CACHE_HOME/sioul/sounds/`. Nothing downloaded, nothing to license.
- **Nature is made here too** (`sioul_core::sounds::nature`), stereo at 22 kHz (what these sounds hold lies below 10 kHz; a minute is 5 MB), seeded the same every time, after Andy Farnell's *Designing Sound* (2010): noise shaped by slow envelopes and filters, and events at random.
  - **Waves**: each wave swells for two to four seconds (low-passed noise growing brighter as it rises), breaks, and withdraws in a band of hiss (the foam, 2.5 to 6 kHz); a wave every 6.5 to 11.5 seconds, a far surf under them; the right ear a quarter second behind, as from one sea. Two minutes.
  - **Rain**: a hiss (pink noise between 450 Hz and 6.5 kHz, its level wandering), about 160 small drops a second (short decaying tones, 2.4 to 7 kHz) and five larger ones on leaves and stones. One minute.
  - **Wind in the trees**: gusts that rise and fall over seconds (a band of noise moving from 220 Hz to 1.1 kHz), leaves rustling with them, and their crackle. A minute and a half.
  - **Crickets at night**: six field crickets (4.1 to 4.9 kHz, three or four pulses a chirp, each its rhythm and place between the ears, near or far), a tree cricket's soft trill at 2.8 kHz, the night's air under them. One minute.
  - **A distant storm**: lighter rain, and thunder far away (a low rumble, under 110 Hz, rolling for seconds) every 18 to 42 seconds. Three minutes.
  - Quiet (about −22 dBFS), never clipped, the end faded into the start over four seconds. Checked by tests: the seam, the level, where each sound's energy lies (the crickets near 4.5 kHz, the waves and the thunder low), and by looking at their spectrograms; listening is yours.
- **Recordings are yours**: any sound in a `sounds` folder of your notes (Ogg, Opus, FLAC or WAV loop exactly; MP3 adds a short gap at the loop, as its encoder pads the ends).
- Played with Qt Multimedia's `MediaPlayer` (`loops: MediaPlayer.Infinite`, seamless since Qt 6.5.1 with FFmpeg).

## Free recordings
Usable in Sioul: **CC0** (Freesound's CC0 sounds; BigSoundBank's "Free and Royalty Free" sounds) and CC BY with credit. Not usable: CC BY-NC, BBC Sound Effects (RemArc licence: personal use only), Sonniss's bundles and Pixabay (no redistribution as sounds). Long CC0 recordings found on Freesound on 3 October 2026, to listen to before choosing (some have traffic, planes or voices):
- Rain: "long_easy_rain_08" by joedeshon (24 min, freesound.org/s/430765), "Rain Los Angeles" by shelbyshark (16 min, /s/501238), "drops rain close-up on yurt" by bruno.auzet (13 min, /s/736845).
- Sea: "2016-10-11-DR-05-Ocean Waves-EDITED" by kingsrow (23 min, /s/362300), "Gentle Ocean Waves Mix (2018)" by esh9419 (12 min, /s/417797), "West Haven Waves" by hotemogf (45 min, /s/635403).
- Wind: "Winter Wind" by a23spyro (18 min, /s/329002), "Wind (From Inside)" by Fabrizio84 (16 min, /s/457668).
- Crickets: "Crickets, Constant and Rhythmic" by CHallSmith (11 min, /s/870522), "Crickets" by hopperphil (12 min, /s/685411).
- Distant storm: "DistantThunderAndWind001b" by andysm (15 min, /s/530416), "Thunderstorm Baltic Sea" by Borgory (27 min, /s/870414).
- Forest: "Spring forest (nature)" by gadesound (10 min, /s/653519), "Sunny Forest 2" by unfa (12 min, /s/156670).

Ten minutes of stereo Ogg is about 10 MB: they are not shipped with Sioul. A separate repository of a few chosen loops, with a CREDITS file, is the next step if wanted.

## ASMR and ambience channels, without advertising
Not inside Sioul, and not by any trick: YouTube's terms forbid blocking its advertising, playing it in a background or audio-only player, and caching it (Terms of Service, 15 December 2023; Developer Policies, September 2026); Invidious and Piped stream it outside those terms and take creators' income away. What is lawful:
1. **YouTube Premium** on your own account, in a site pinned in Sioul's Sites page, or in YouTube's own app.
2. **What creators sell or give**: Bandcamp purchases (yours to play on your devices), a Patreon member's private audio feed, their podcasts. The files go in the `sounds` folder.
3. **Internet radio** through Radio Browser (free and open; ambient and nature stations exist; their streams may carry advertising).

## Later
Several layers at once, each with its volume (rain and wind, waves and crickets).
