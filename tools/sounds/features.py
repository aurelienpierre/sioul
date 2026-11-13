#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre
"""Measures sounds of nature the same way, recordings and Sioul's own, so
that a recipe can be tuned to what real recordings hold.

    tools/sounds/features.py KIND FILE... [--json OUT] [--png OUT]

The recordings: tools/sounds/fetch-references.sh DIR. Sioul's own:
SIOUL_SOUNDS_OUT=DIR cargo test --release -p sioul-core --lib sounds:: writes
each sound of nature in DIR. Then compare.py puts them side by side. Needs
Python 3 with numpy and scipy (matplotlib for --png), and ffmpeg.

Each file is decoded by ffmpeg to stereo at 22,050 Hz (Sioul's nature rate),
its first and last two seconds left out, and brought to one RMS, so that
level never counts, only shape. Measures:

- spectrum: third-octave bands, 63 Hz to 8 kHz, dB against the whole;
- movement: the level in three bands (under 400 Hz, 400 Hz to 2 kHz, over
  2 kHz), every 10 ms: its spread (dB, 10th to 90th percentile, 100 ms
  smoothed) and its modulation, octave by octave from 0.03 to 16 Hz;
- lag: how long the high band follows the low one (waves: foam after crash);
- clicks: kurtosis of the high band, median over seconds (3 for noise);
- stereo: the two ears' correlation per band;
- tone: spectral flatness 1 to 8 kHz (1 for noise, near 0 for a whistle);
- events: chirps a second and their pitch (birds), claps a minute (thunder).
"""

import json
import subprocess
import sys

import numpy as np
from scipy import signal

RATE = 22050
BANDS = [63 * 2 ** (k / 3) for k in range(22)]  # 63 Hz .. 8 kHz
MOD = [0.03 * 2 ** k for k in range(10)]  # 0.03 .. 15.4 Hz, octaves


def decode(path):
    raw = subprocess.run(["ffmpeg", "-v", "error", "-i", path, "-ac", "2", "-ar", str(RATE), "-f", "f32le", "-"], capture_output=True, check=True).stdout
    x = np.frombuffer(raw, dtype=np.float32).reshape(-1, 2).astype(np.float64)
    x = x[2 * RATE : -2 * RATE] if len(x) > 8 * RATE else x
    return x / max(np.sqrt(np.mean(x**2)), 1e-9)


def band(x, low, high):
    if low and high:
        sos = signal.butter(4, [low, high], "bandpass", fs=RATE, output="sos")
    elif high:
        sos = signal.butter(4, high, "lowpass", fs=RATE, output="sos")
    else:
        sos = signal.butter(4, low, "highpass", fs=RATE, output="sos")
    return signal.sosfiltfilt(sos, x, axis=0)


def envelope(x, hop=220):
    n = len(x) // hop
    return np.sqrt(np.mean(x[: n * hop].reshape(n, hop) ** 2, axis=1) + 1e-12)


def modulation(env, hop=220):
    fs = RATE / hop
    e = env / np.mean(env) - 1.0
    f, p = signal.welch(e, fs=fs, nperseg=min(len(e), int(fs * 64)))
    out = []
    for lo in MOD:
        sel = (f >= lo) & (f < 2 * lo)
        out.append(float(10 * np.log10(np.sum(p[sel]) * (f[1] - f[0]) + 1e-12)) if sel.any() else None)
    return out


def measure(path):
    x = decode(path)
    mid = x.mean(axis=1)
    f, p = signal.welch(mid, fs=RATE, nperseg=8192)
    total = np.sum(p)
    spectrum = []
    for c in BANDS:
        sel = (f >= c / 2 ** (1 / 6)) & (f < c * 2 ** (1 / 6))
        spectrum.append(float(10 * np.log10(np.sum(p[sel]) / total + 1e-12)))
    parts = {"low": (None, 400), "mid": (400, 2000), "high": (2000, None)}
    filtered = {k: band(x, lo, hi) for k, (lo, hi) in parts.items()}
    out = {"file": path, "seconds": round(len(x) / RATE, 1), "spectrum": spectrum, "centroid": float(np.sum(f * p) / total)}
    for k, y in filtered.items():
        env = envelope(y.mean(axis=1))
        smooth = np.convolve(env, np.ones(10) / 10, mode="valid")
        db = 20 * np.log10(smooth + 1e-9)
        out[f"spread_{k}"] = float(np.percentile(db, 90) - np.percentile(db, 10))
        out[f"mod_{k}"] = modulation(env)
        l, r = y[:, 0], y[:, 1]
        out[f"stereo_{k}"] = float(np.corrcoef(l, r)[0, 1]) if np.std(l) > 0 and np.std(r) > 0 else 1.0
    # The high band after the low: the lag of their envelopes' best match, within ±4 s.
    el, eh = envelope(filtered["low"].mean(axis=1)), envelope(filtered["high"].mean(axis=1))
    el, eh = (el - el.mean()) / (el.std() + 1e-9), (eh - eh.mean()) / (eh.std() + 1e-9)
    lags = np.arange(-400, 401)
    cc = [np.mean(el[max(0, -k) : len(el) - max(0, k)] * eh[max(0, k) : len(eh) - max(0, -k)]) for k in lags]
    out["lag_high_after_low"] = float(lags[int(np.argmax(cc))] * 0.01)
    out["lag_match"] = float(np.max(cc))
    # Clicks: the high band's kurtosis, second by second.
    h = filtered["high"].mean(axis=1)
    sec = len(h) // RATE
    kurt = [float(np.mean(s**4) / (np.mean(s**2) ** 2 + 1e-18)) for s in h[: sec * RATE].reshape(sec, RATE)]
    out["clicks"] = float(np.median(kurt))
    # Tone: spectral flatness of 1 to 8 kHz, 46 ms frames, median.
    ff, tt, S = signal.stft(mid, fs=RATE, nperseg=1024)
    sel = (ff >= 1000) & (ff <= 8000)
    P = np.abs(S[sel]) ** 2 + 1e-18
    flat = np.exp(np.mean(np.log(P), axis=0)) / np.mean(P, axis=0)
    out["flatness"] = float(np.median(flat))
    # Events: chirps (tonal frames rising 8 dB over their neighbourhood, 1 to 8 kHz).
    peak_bin = np.argmax(P, axis=0)
    peak = P[peak_bin, np.arange(P.shape[1])]
    tonal = (peak / np.mean(P, axis=0)) > 12
    loud = 10 * np.log10(np.sum(P, axis=0))
    base = signal.medfilt(loud, 61)
    on = tonal & (loud - base > 8)
    starts = np.flatnonzero(on[1:] & ~on[:-1])
    out["chirps_per_second"] = float(len(starts) / (len(mid) / RATE))
    pitches = ff[sel][peak_bin[on]] if on.any() else np.array([0.0])
    out["chirp_pitch"] = [float(np.percentile(pitches, q)) for q in (10, 50, 90)]
    # Claps: the low band (under 200 Hz) rising 10 dB over its minute's median, 5 s apart at least.
    lowish = envelope(band(mid, None, 200), hop=2205)
    ldb = 20 * np.log10(lowish + 1e-9)
    peaks, _ = signal.find_peaks(ldb, height=np.median(ldb) + 10, distance=50)
    out["claps_per_minute"] = float(len(peaks) / (len(mid) / RATE / 60))
    return out, x


def spectrogram(paths_and_signals, png):
    import matplotlib

    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    n = len(paths_and_signals)
    fig, axes = plt.subplots(n, 1, figsize=(12, 2.2 * n), squeeze=False)
    for ax, (path, x) in zip(axes[:, 0], paths_and_signals):
        seg = x[: 40 * RATE].mean(axis=1)
        f, t, S = signal.spectrogram(seg, fs=RATE, nperseg=1024, noverlap=768)
        ax.pcolormesh(t, f, 10 * np.log10(S + 1e-12), shading="auto", vmin=-90, vmax=-20, cmap="magma")
        ax.set_yscale("symlog", linthresh=500)
        ax.set_ylim(40, 11000)
        ax.set_title(path.rsplit("/", 1)[-1], fontsize=8)
        ax.tick_params(labelsize=6)
    fig.tight_layout()
    fig.savefig(png, dpi=80)


def main():
    args = sys.argv[1:]
    out_json = args[args.index("--json") + 1] if "--json" in args else None
    out_png = args[args.index("--png") + 1] if "--png" in args else None
    files = [a for a in args[1:] if not a.startswith("--") and a not in (out_json, out_png)]
    results, signals = [], []
    for path in files:
        r, x = measure(path)
        results.append(r)
        signals.append((path, x))
    if out_json:
        with open(out_json, "w") as fh:
            json.dump({"kind": args[0], "results": results}, fh, indent=1)
    if out_png:
        spectrogram(signals, out_png)
    for r in results:
        print(json.dumps({k: (round(v, 2) if isinstance(v, float) else v) for k, v in r.items() if k not in ("spectrum",) and not k.startswith("mod_")}))


if __name__ == "__main__":
    main()
