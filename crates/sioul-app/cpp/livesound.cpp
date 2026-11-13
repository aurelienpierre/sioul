// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A sound made as it plays (sioul_core::sounds::Stream, src/livesound.rs),
// played through Qt's audio output: the output asks for samples as it needs
// them, and the stream gives the ones its thread made a second ahead. Where the
// system's output takes no 16-bit stereo at the sound's rate, nothing starts
// and the window plays the sound's file instead (NoisePlayer.qml).

#include <QAudioDevice>
#include <QAudioFormat>
#include <QAudioSink>
#include <QIODevice>
#include <QMediaDevices>

#include <cstddef>
#include <cstdint>

// The next `length` bytes of `stream`, 16-bit samples, the two ears in turn
// (src/livesound.rs); silence where none is made yet.
extern "C" void sioul_live_read(const void *stream, unsigned char *out, std::size_t length);

namespace {

// The stream, as Qt's audio output reads it: endless, never at its end.
class LiveDevice final : public QIODevice
{
public:
    explicit LiveDevice(const void *stream)
        : stream(stream)
    {
        open(QIODevice::ReadOnly);
    }

    bool isSequential() const override { return true; }
    // Always more: a sound made as it plays has no end.
    qint64 bytesAvailable() const override { return (qint64(1) << 20) + QIODevice::bytesAvailable(); }

protected:
    qint64 readData(char *data, qint64 maxSize) override
    {
        // Whole frames only: two ears of two bytes.
        const qint64 length = maxSize - maxSize % 4;
        if (length > 0)
            sioul_live_read(stream, reinterpret_cast<unsigned char *>(data), static_cast<std::size_t>(length));
        return length;
    }

    qint64 writeData(const char *, qint64) override { return -1; }

private:
    const void *stream;
};

// A sound playing: the stream as a device, and the output reading it, made
// after the device and gone before it.
struct LiveSound
{
    LiveSound(const void *stream, const QAudioDevice &output, const QAudioFormat &format)
        : device(stream)
        , sink(output, format)
    {
    }

    LiveDevice device;
    QAudioSink sink;
};

}

// `stream` played at `rate` frames a second, both ears, 16 bits, at `volume`
// (0 to 1): the sound playing, for `sioul_live_volume` and `sioul_live_stop`;
// null when the system's output takes no such sound or does not start.
extern "C" void *sioul_live_start(const void *stream, int rate, double volume)
{
    const QAudioDevice output = QMediaDevices::defaultAudioOutput();
    QAudioFormat format;
    format.setSampleRate(rate);
    format.setChannelCount(2);
    format.setSampleFormat(QAudioFormat::Int16);
    if (output.isNull() || !output.isFormatSupported(format))
        return nullptr;
    auto *sound = new LiveSound(stream, output, format);
    sound->sink.setVolume(volume);
    // A quarter of a second asked for at a time: the stream holds a second.
    sound->sink.setBufferSize(rate);
    sound->sink.start(&sound->device);
    if (sound->sink.error() != QAudio::NoError || sound->sink.state() == QAudio::StoppedState) {
        delete sound;
        return nullptr;
    }
    return sound;
}

extern "C" void sioul_live_volume(void *sound, double volume)
{
    if (sound != nullptr)
        static_cast<LiveSound *>(sound)->sink.setVolume(volume);
}

// The sound stopped: its output reads its stream no more once this returns.
extern "C" void sioul_live_stop(void *sound)
{
    if (sound == nullptr)
        return;
    auto *playing = static_cast<LiveSound *>(sound);
    playing->sink.stop();
    delete playing;
}
