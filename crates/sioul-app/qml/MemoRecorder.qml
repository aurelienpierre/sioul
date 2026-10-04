// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// An audio memo, recorded into the notes folder as Opus: it starts when
// loaded and ends with `stop()`, then opens as a note, to tie to anything.
// Loaded only while recording, so the microphone is never open otherwise.
// The system is asked for the microphone first (macOS asks you once; Linux
// has nothing to ask): refused, nothing is recorded and `refused` says so.

import QtQuick
import QtCore
import QtMultimedia

Item {
    id: memo

    required property url location
    property int seconds: 0
    property bool started: false

    signal finished(url location)
    signal failed(string problem)
    signal refused

    function stop() {
        recorder.stop()
    }

    // Without a place in the notes folder, nothing is recorded ("" said as the
    // problem): Qt would pick a folder of its own, where the memo is never found.
    function begin() {
        if (memo.location.toString() === "")
            Qt.callLater(() => memo.failed(""))
        else if (microphone.status === Qt.PermissionStatus.Granted) {
            if (recorder.recorderState === MediaRecorder.StoppedState && !memo.started)
                recorder.record()
        } else if (microphone.status === Qt.PermissionStatus.Denied)
            Qt.callLater(() => memo.refused())
        else
            microphone.request()
    }

    // Asked once; the answer starts the memo, or says it cannot be.
    MicrophonePermission {
        id: microphone

        onStatusChanged: memo.begin()
    }

    CaptureSession {
        audioInput: AudioInput {}
        recorder: MediaRecorder {
            id: recorder

            outputLocation: memo.location
            mediaFormat {
                fileFormat: MediaFormat.Ogg
                audioCodec: MediaFormat.AudioCodec.Opus
            }
            onRecorderStateChanged: {
                if (recorder.recorderState === MediaRecorder.RecordingState)
                    memo.started = true
                else if (recorder.recorderState === MediaRecorder.StoppedState && memo.started)
                    memo.finished(recorder.actualLocation)
            }
            onErrorOccurred: (error, problem) => memo.failed(problem !== "" ? problem : String(error))
        }
    }

    Timer {
        interval: 1000
        running: recorder.recorderState === MediaRecorder.RecordingState
        repeat: true
        onTriggered: memo.seconds += 1
    }

    Component.onCompleted: memo.begin()
}
