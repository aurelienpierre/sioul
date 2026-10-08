// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The devices of calls in sites (docs/sites.md, "The devices of calls"). Run
// in every page and frame of a site's view, in the page's own world, before
// the page's scripts (SitesPage.qml, `webFixesFor`), as
//
//     (chosen, token) => { … }
//
// chosen: {camera, microphone, speaker}, by the names the system gives them,
// "" or absent for the system's own; given only to sites allowed a microphone
// or a camera. token: a secret of this run of Sioul, which marks what this
// script tells Sioul through the console; the page never sees it.
//
// A call keeps the tracks it was given from start to end, and Sioul changes
// what feeds them: for each camera or microphone track, the page gets a relay
// (Chromium's MediaStreamTrackGenerator) fed from the real device's track
// through a MediaStreamTrackProcessor. Changing the device asks for a new real
// track with the call's own settings, feeds the relay from it, and stops the
// old one. The page's tracks, its streams, its senders, its preview, its
// effects, its mute button and its voice meter never see a change. Without
// the relay, a call's tracks are swapped in its senders and its streams
// instead (`swap`), which a voice meter or an effect may not follow.
//
// At a call's start the device chosen in Sioul is asked for; before a site
// was first allowed a device its names are unknown, so the call is moved to
// the chosen device right after that first grant, before the page gets its
// tracks. During a call, a device the page asks for by its id (the site's own
// menu) is given: the latest choice wins. The speaker is given to every
// element and audio context the page plays through, those playing by
// themselves too.
//
// Sioul hears, through the console function kept before the page ran:
// "call" when a call starts in this page, "end" when every device is closed
// or the page goes, "changed" and "kept" (a device that could not be opened,
// with only its kind and the error's name).
(chosen, token) => {
    if (!navigator.mediaDevices || MediaDevices.prototype.__sioulCalls)
        return;
    Object.defineProperty(MediaDevices.prototype, "__sioulCalls", { value: true });
    const Gen = window.MediaStreamTrackGenerator, Proc = window.MediaStreamTrackProcessor;
    const relayable = typeof Gen === "function" && typeof Proc === "function";
    const originalAsk = MediaDevices.prototype.getUserMedia;
    const originalList = MediaDevices.prototype.enumerateDevices;
    const tell = console.debug.bind(console);
    const stringify = JSON.stringify.bind(JSON), assign = Object.assign;
    const report = (what, detail) => tell("sioul:" + token + ":" + stringify(assign({ what: what }, detail || {})));
    let names = Object.assign({}, chosen || {});
    // The calls of this page: relays (or swapped tracks) by their real track.
    const relays = new Set();
    const fold = s => String(s || "").toLowerCase();
    const listed = async kind => (await originalList.call(navigator.mediaDevices)).filter(d => d.kind === kind && d.label);
    // A device by its name: the same, then one starting with the other (Chromium
    // adds " (vid:pid)" to a camera's name), then one containing it.
    const find = async (kind, name) => {
        if (!name)
            return null;
        try {
            const all = await listed(kind), want = fold(name);
            const hit = all.find(d => fold(d.label) === want) || all.find(d => fold(d.label).startsWith(want))
                || all.find(d => want.startsWith(fold(d.label))) || all.find(d => fold(d.label).includes(want));
            return hit ? hit.deviceId : null;
        } catch (e) {
            return null;
        }
    };
    const inputOf = kind => kind === "video" ? "videoinput" : "audioinput";
    const nameOf = kind => kind === "video" ? names.camera : names.microphone;
    const named = (track, name) => !name || fold(track.label).startsWith(fold(name)) || fold(name).startsWith(fold(track.label));
    const enabledOf = Object.getOwnPropertyDescriptor(MediaStreamTrack.prototype, "enabled");
    const mutedOf = Object.getOwnPropertyDescriptor(MediaStreamTrack.prototype, "muted");
    const stopTrack = MediaStreamTrack.prototype.stop, cloneTrack = MediaStreamTrack.prototype.clone;
    const live = kind => Array.from(relays).some(r => r.kind === kind && !r.ended);
    let calling = false;
    const sayCalling = () => {
        const now = Array.from(relays).some(r => !r.ended);
        if (now !== calling) {
            calling = now;
            report(now ? "call" : "end", { relay: relayable });
        }
    };
    // The page goes (a reload, a frame taken out): its calls end with it.
    window.addEventListener("pagehide", () => {
        if (calling)
            report("end", { relay: relayable });
        calling = false;
    });

    // The relay's pump: one writer for its life; the reader changes with the device.
    function pump(relay) {
        const writer = relay.gen.writable.getWriter();
        (async () => {
            while (!relay.ended) {
                const reader = relay.reader;
                let chunk;
                try {
                    chunk = await reader.read();
                } catch (e) {
                    chunk = { done: true };
                }
                if (relay.ended) {
                    if (chunk.value)
                        chunk.value.close();
                    break;
                }
                if (chunk.done) {
                    if (relay.reader !== reader)
                        continue;
                    // Unplugged, or taken by the system: the call's track ends, as it would.
                    relay.end();
                    break;
                }
                try {
                    await writer.write(chunk.value);
                } catch (e) {
                    try { chunk.value.close(); } catch (x) {}
                }
            }
            try { await writer.close(); } catch (e) {}
        })();
    }
    function feed(relay, real) {
        relay.real = real;
        relay.reader = new Proc({ track: real }).readable.getReader();
        enabledOf.set.call(real, true);
        real.addEventListener("mute", () => { if (relay.real === real) relay.gen.dispatchEvent(new Event("mute")); });
        real.addEventListener("unmute", () => { if (relay.real === real) relay.gen.dispatchEvent(new Event("unmute")); });
    }
    // What the page reads of its track is the real device's.
    function dress(track, relay) {
        relay.copies.add(track);
        Object.defineProperty(track, "label", { get: () => relay.real.label, configurable: true });
        Object.defineProperty(track, "muted", { get: () => mutedOf.get.call(relay.real) || mutedOf.get.call(track), configurable: true });
        track.getSettings = () => relay.real.getSettings();
        track.getCapabilities = () => relay.real.getCapabilities ? relay.real.getCapabilities() : {};
        track.getConstraints = () => relay.real.getConstraints();
        track.applyConstraints = c => relay.real.applyConstraints(c);
        track.stop = () => {
            stopTrack.call(track);
            relay.copies.delete(track);
            if (relay.copies.size === 0)
                relay.end();
        };
        track.clone = () => {
            const copy = cloneTrack.call(track);
            dress(copy, relay);
            return copy;
        };
    }
    function makeRelay(real, asked) {
        const gen = new Gen({ kind: real.kind });
        const relay = { kind: real.kind, gen: gen, asked: asked, real: null, reader: null, ended: false, copies: new Set() };
        relay.end = () => {
            if (relay.ended)
                return;
            relay.ended = true;
            try { relay.reader.cancel(); } catch (e) {}
            stopTrack.call(relay.real);
            relays.delete(relay);
            sayCalling();
        };
        feed(relay, real);
        dress(gen, relay);
        relays.add(relay);
        pump(relay);
        return gen;
    }
    const relayOf = track => Array.from(relays).find(r => r.copies && r.copies.has(track)) || null;
    // A stream's copy (MediaStream.clone) is dressed too.
    const cloneStream = MediaStream.prototype.clone;
    MediaStream.prototype.clone = function () {
        const copy = cloneStream.call(this);
        const mine = this.getTracks(), theirs = copy.getTracks();
        mine.forEach((track, i) => {
            const relay = relayOf(track);
            if (relay)
                dress(theirs[i], relay);
        });
        return copy;
    };
    // A still photo (ImageCapture) is taken from the real camera: a relay has no photo settings.
    if (relayable && typeof window.ImageCapture === "function") {
        const Capture = window.ImageCapture;
        window.ImageCapture = class extends Capture {
            constructor(track) {
                const relay = relayOf(track);
                super(relay ? relay.real : track);
            }
        };
    }

    // Without the relay: the peers and streams a call uses, to swap its tracks in.
    const peers = [], streams = [], current = new Map();
    if (!relayable && typeof window.RTCPeerConnection === "function") {
        const Peer = window.RTCPeerConnection;
        window.RTCPeerConnection = class extends Peer {
            constructor(...args) {
                super(...args);
                peers.push(new WeakRef(this));
            }
        };
        // What the page does to its old track reaches the one now sent.
        const now = track => {
            let t = track;
            while (current.has(t))
                t = current.get(t);
            return t;
        };
        MediaStreamTrack.prototype.stop = function () {
            stopTrack.call(this);
            const sent = now(this);
            if (sent !== this)
                stopTrack.call(sent);
            for (const r of relays) {
                if (r.real === sent || r.first === this)
                    r.end();
            }
        };
        Object.defineProperty(MediaStreamTrack.prototype, "enabled", {
            configurable: true, enumerable: enabledOf.enumerable, get: enabledOf.get,
            set(value) {
                enabledOf.set.call(this, value);
                const sent = now(this);
                if (sent !== this)
                    enabledOf.set.call(sent, value);
            }
        });
    }
    function keepPlain(stream, asked) {
        streams.push(new WeakRef(stream));
        for (const real of stream.getTracks()) {
            const relay = { kind: real.kind, asked: asked[real.kind], real: real, first: real, ended: false };
            relay.end = () => {
                if (relay.ended)
                    return;
                relay.ended = true;
                relays.delete(relay);
                sayCalling();
            };
            relays.add(relay);
        }
    }
    async function swap(relay, fresh) {
        const old = relay.real;
        for (const ref of peers) {
            const peer = ref.deref();
            if (!peer)
                continue;
            for (const sender of peer.getSenders()) {
                if (sender.track === old)
                    await sender.replaceTrack(fresh);
            }
        }
        for (const ref of streams) {
            const stream = ref.deref();
            if (stream && stream.getTracks().includes(old)) {
                stream.removeTrack(old);
                stream.addTrack(fresh);
            }
        }
        enabledOf.set.call(fresh, enabledOf.get.call(old));
        current.set(old, fresh);
        relay.real = fresh;
    }

    // A real track on the device named, with the call's settings; looser if the device cannot meet them.
    async function open(kind, asked, name) {
        const id = await find(inputOf(kind), name);
        if (name && !id)
            throw Object.assign(new Error("not found"), { name: "NotFoundError" });
        const wanted = asked && asked !== true ? Object.assign({}, asked) : {};
        delete wanted.groupId;
        if (id)
            wanted.deviceId = { exact: id };
        else
            delete wanted.deviceId;
        try {
            return (await originalAsk.call(navigator.mediaDevices, { [kind]: wanted })).getTracks()[0];
        } catch (e) {
            if (e.name !== "OverconstrainedError")
                throw e;
            const loose = { deviceId: wanted.deviceId };
            for (const key of ["echoCancellation", "noiseSuppression", "autoGainControl", "channelCount"]) {
                if (key in wanted)
                    loose[key] = wanted[key];
            }
            return (await originalAsk.call(navigator.mediaDevices, { [kind]: loose })).getTracks()[0];
        }
    }
    const ownChoice = c => c && c !== true && c.deviceId !== undefined;

    MediaDevices.prototype.getUserMedia = async function (constraints) {
        const asked = constraints ? Object.assign({}, constraints) : {};
        for (const kind of ["video", "audio"]) {
            const name = nameOf(kind);
            // At a call's start Sioul's choice wins; during a call, the page's own.
            if (!asked[kind] || !name || (live(kind) && ownChoice(asked[kind])))
                continue;
            const id = await find(inputOf(kind), name);
            // Ideal: the call starts even when this device cannot open (said below).
            if (id)
                asked[kind] = Object.assign(asked[kind] === true ? {} : Object.assign({}, asked[kind]), { deviceId: { ideal: id } });
        }
        const stream = await originalAsk.call(this, asked);
        // Names are known once a device is allowed: the call moves to the chosen
        // one now, before the page sees its tracks (a site's first call).
        for (const kind of ["video", "audio"]) {
            const name = nameOf(kind), track = stream.getTracks().find(t => t.kind === kind);
            if (!track || named(track, name) || (live(kind) && ownChoice(constraints && constraints[kind])))
                continue;
            try {
                const better = await open(kind, constraints && constraints[kind], name);
                stream.removeTrack(track);
                stopTrack.call(track);
                stream.addTrack(better);
            } catch (e) {
                report("kept", { kind: kind, why: e.name });
            }
        }
        if (!relayable) {
            keepPlain(stream, constraints || {});
            sayCalling();
            return stream;
        }
        const given = new MediaStream();
        for (const track of stream.getTracks())
            given.addTrack(makeRelay(track, constraints && constraints[track.kind]));
        sayCalling();
        return given;
    };

    // The old callback forms (still in Chromium) take the same way.
    for (const key of ["getUserMedia", "webkitGetUserMedia"]) {
        if (typeof Navigator.prototype[key] === "function") {
            Navigator.prototype[key] = function (constraints, done, failed) {
                navigator.mediaDevices.getUserMedia(constraints).then(done, failed);
            };
        }
    }

    async function change(kind, name) {
        for (const relay of Array.from(relays)) {
            if (relay.kind !== kind || relay.ended)
                continue;
            const old = relay.real;
            const asked = Object.assign({}, relay.asked && relay.asked !== true ? relay.asked : {}, old.getConstraints());
            delete asked.deviceId;
            delete asked.groupId;
            let fresh;
            try {
                fresh = await open(kind, asked, name);
            } catch (e) {
                report("kept", { kind: kind, why: e.name });
                continue;
            }
            if (relay.ended) {
                stopTrack.call(fresh);
                continue;
            }
            if (relayable) {
                const oldReader = relay.reader;
                feed(relay, fresh);
                try { oldReader.cancel(); } catch (e) {}
            } else {
                await swap(relay, fresh);
            }
            stopTrack.call(old);
            report("changed", { kind: kind });
        }
    }

    // The speaker: every element and audio context the page plays through. One
    // the page sent elsewhere itself (its own menu) keeps that, until the next
    // change in Sioul; with the system's own, the page's choices stand.
    const players = new Set(), contexts = new Set(), heard = new WeakSet(), ownSink = new WeakSet();
    const setSink = { element: HTMLMediaElement.prototype.setSinkId, context: window.AudioContext && AudioContext.prototype.setSinkId };
    const sinkTo = async (target, changed) => {
        const set = target instanceof HTMLMediaElement ? setSink.element : setSink.context;
        if (!set || (!changed && (ownSink.has(target) || !names.speaker)))
            return;
        if (changed)
            ownSink.delete(target);
        const name = names.speaker;
        const id = name ? await find("audiooutput", name) : "";
        if (id === null || target.sinkId === id)
            return;
        try {
            await set.call(target, id);
        } catch (e) {
            report("kept", { kind: "speaker", why: e.name });
        }
    };
    for (const [owner, key] of [[HTMLMediaElement.prototype, "element"], [window.AudioContext && AudioContext.prototype, "context"]]) {
        if (owner && setSink[key]) {
            owner.setSinkId = function (id) {
                ownSink.add(this);
                return setSink[key].call(this, id);
            };
        }
    }
    const watch = element => {
        if (!heard.has(element)) {
            heard.add(element);
            players.add(new WeakRef(element));
        }
        sinkTo(element, false);
    };
    const play = HTMLMediaElement.prototype.play;
    HTMLMediaElement.prototype.play = function () {
        watch(this);
        return play.apply(this, arguments);
    };
    // Elements playing by themselves (autoplay) are given their stream here, never call play().
    const srcObject = Object.getOwnPropertyDescriptor(HTMLMediaElement.prototype, "srcObject");
    if (srcObject && srcObject.set) {
        Object.defineProperty(HTMLMediaElement.prototype, "srcObject", {
            configurable: true, enumerable: srcObject.enumerable, get: srcObject.get,
            set(value) {
                srcObject.set.call(this, value);
                if (value)
                    watch(this);
            }
        });
    }
    if (window.AudioContext && AudioContext.prototype.setSinkId) {
        const Original = window.AudioContext;
        window.AudioContext = class extends Original {
            constructor(...args) {
                super(...args);
                contexts.add(new WeakRef(this));
                sinkTo(this, false);
            }
        };
    }

    // A change from Sioul (SitesPage.qml, `setCallDevice`).
    window.addEventListener("sioul-devices", e => {
        const before = names;
        names = Object.assign({}, (e && e.detail) || {});
        if (names.camera !== before.camera)
            change("video", names.camera);
        if (names.microphone !== before.microphone)
            change("audio", names.microphone);
        if (names.speaker !== before.speaker) {
            document.querySelectorAll("audio, video").forEach(element => {
                if (!heard.has(element)) {
                    heard.add(element);
                    players.add(new WeakRef(element));
                }
            });
            for (const set of [players, contexts]) {
                for (const ref of Array.from(set)) {
                    const target = ref.deref();
                    if (target)
                        sinkTo(target, true);
                    else
                        set.delete(ref);
                }
            }
        }
    });
}
