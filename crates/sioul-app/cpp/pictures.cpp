// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Pictures made small for a multimedia message, on a computer
// (crates/sioul-app/src/attach.rs; docs/texts.md, "Multimedia messages sent
// from a computer"). Rust tells a picture by its first bytes (`attach::sniff`)
// and names its format; only that format's reader then decodes it, no other
// tried, so that none of the documents Qt's plugins can also render (PDF,
// SVG, PostScript, a camera's raw file) is ever opened here. A picture is
// read with its orientation applied (its EXIF tag), decoded at about the size
// wanted (a JPEG in eighths of its own), turned into sRGB; then written as a
// baseline JPEG from a fresh copy of its pixels laid on white, which holds no
// metadata: no EXIF (no GPS position, no camera, no time), no XMP, no IPTC,
// no comment, no colour profile. Qt's readers keep a JPEG's comments and a
// PNG's texts as the image's text, and scaling copies that text
// (QImage::copyMetadata), so the writer would put them back; it would also
// write the image's colour space as a profile: the copy painted anew carries
// neither, and the writer then writes JFIF alone (tests/attach.rs proves it).
// Rust chooses each size and quality (`attach::fit`); this only reads and
// writes.

#include <QColorSpace>
#include <QFileInfo>
#include <QImage>
#include <QImageReader>
#include <QImageWriter>
#include <QPainter>
#include <QString>

struct SioulPicture
{
    QImage image;
};

// Whether Qt reads pictures of `format` here ("webp", "tiff", "heif"…: some
// systems' Qt has no reader of them).
extern "C" int sioul_picture_reads(const char *format)
{
    return QImageReader::supportedImageFormats().contains(QByteArray(format)) ? 1 : 0;
}

// The picture at `path` (UTF-8) decoded by the reader of `format` alone (no
// other is tried, whatever the file holds): its orientation applied, its
// long side at `largest` or more when it is longer (a smaller one whole), in
// sRGB. Its size then in `width` and `height`, and how many frames it holds
// in `frames` (an animated GIF more than one). Null when it does not read.
extern "C" SioulPicture *sioul_picture_open(const char *path, const char *format, int largest, int *width, int *height, int *frames)
{
    *width = 0;
    *height = 0;
    *frames = 0;
    QImageReader reader(QString::fromUtf8(path), QByteArray(format));
    reader.setAutoDetectImageFormat(false);
    reader.setAutoTransform(true);
    if (!reader.canRead())
        return nullptr;
    *frames = qMax(reader.imageCount(), 1);
    const QSize stored = reader.size();
    if (stored.isValid() && largest > 0 && qMax(stored.width(), stored.height()) > largest)
        reader.setScaledSize(stored.scaled(largest, largest, Qt::KeepAspectRatio));
    QImage image = reader.read();
    if (image.isNull())
        return nullptr;
    if (image.colorSpace().isValid() && image.colorSpace() != QColorSpace(QColorSpace::SRgb))
        image.convertToColorSpace(QColorSpace(QColorSpace::SRgb));
    *width = image.width();
    *height = image.height();
    return new SioulPicture { image };
}

// The picture written to `out` (UTF-8) as a baseline JPEG at `quality` (1 to
// 100), its long side made `longSide` when it is longer: laid on white (a
// picture's transparency too), from a fresh copy of its pixels, with no text
// and no colour space. The file's size in bytes, -1 when it was not written.
extern "C" long long sioul_picture_jpeg(const SioulPicture *picture, int longSide, int quality, const char *out)
{
    if (!picture)
        return -1;
    QImage source = picture->image;
    if (longSide > 0 && qMax(source.width(), source.height()) > longSide)
        source = source.scaled(longSide, longSide, Qt::KeepAspectRatio, Qt::SmoothTransformation);
    QImage clean(source.size(), QImage::Format_RGB32);
    clean.fill(Qt::white);
    {
        QPainter painter(&clean);
        painter.drawImage(0, 0, source);
    }
    const QString path = QString::fromUtf8(out);
    QImageWriter writer(path, "jpeg");
    writer.setQuality(quality);
    writer.setOptimizedWrite(true);
    writer.setProgressiveScanWrite(false);
    if (!writer.write(clean))
        return -1;
    return QFileInfo(path).size();
}

// The picture let go.
extern "C" void sioul_picture_close(SioulPicture *picture)
{
    delete picture;
}
