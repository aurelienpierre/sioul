// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// colour: the sites in the screen's own colours, and calmer colours
// (docs/colour.md). What this file reads, it only reads: no profile, no
// display setting, no calibration curve is ever changed.

#include "screencolours.h"

#include <QAbstractNativeEventFilter>
#include <QCryptographicHash>
#include <QGuiApplication>
#include <QMutexLocker>
#include <QPointer>
#include <QQmlEngine>
#include <QQuickImageProvider>
#include <QQuickWindow>
#include <QSGRendererInterface>
#include <QScreen>
#include <QtCore/qfloat16.h>

#include <cstdint>
#include <cstdlib>
#include <optional>
#include <type_traits>
#include <vector>

#if defined(Q_OS_LINUX)
#if QT_CONFIG(xcb)
#include <QtGui/qguiapplication_platform.h>
#include <dlfcn.h>
#define SIOUL_X11 1
#endif
#endif

#if defined(Q_OS_LINUX)
#include <QDBusArgument>
#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusObjectPath>
#include <QDBusVariant>
#define SIOUL_COLORD 1
#endif

// colour.rs: the tables, a profile's name, and what the settings say.
extern "C" int sioul_colour_tables(const unsigned char *profile, size_t length, int calmer, size_t cube_size, float *cube, size_t curve_size, float *curve);
extern "C" size_t sioul_colour_profile_name(const unsigned char *profile, size_t length, unsigned char *out, size_t capacity);
extern "C" void sioul_colour_said(int state, const char *name);

namespace {

// colour.rs's CUBE and CURVE (checked there: other sizes are refused).
constexpr int Cube = 33;
constexpr int Curve = 1024;

#ifdef SIOUL_X11
// libxcb, which Qt's X11 platform has loaded already, reached by its stable
// C interface: Sioul needs no X11 headers to build (they are not installed
// everywhere Sioul builds), and loads nothing new. These structures are
// libxcb's own (xproto.h), unchanged since 2008.
struct AtomCookie {
    unsigned int sequence;
};
struct AtomReply {
    uint8_t responseType;
    uint8_t pad0;
    uint16_t sequence;
    uint32_t length;
    uint32_t atom;
};
struct PropertyCookie {
    unsigned int sequence;
};
struct PropertyReply {
    uint8_t responseType;
    uint8_t format;
    uint16_t sequence;
    uint32_t length;
    uint32_t type;
    uint32_t bytesAfter;
    uint32_t valueLength;
    uint8_t pad0[12];
};
struct ScreenData {
    uint32_t root;
};
struct ScreenIterator {
    ScreenData *data;
    int rem;
    int index;
};
struct PropertyNotify {
    uint8_t responseType;
    uint8_t pad0;
    uint16_t sequence;
    uint32_t window;
    uint32_t atom;
    uint32_t time;
    uint8_t state;
    uint8_t pad1[3];
};
constexpr uint8_t PropertyNotifyEvent = 28;

struct Xcb {
    AtomCookie (*intern)(xcb_connection_t *, uint8_t, uint16_t, const char *) = nullptr;
    AtomReply *(*internReply)(xcb_connection_t *, AtomCookie, void **) = nullptr;
    PropertyCookie (*getProperty)(xcb_connection_t *, uint8_t, uint32_t, uint32_t, uint32_t, uint32_t, uint32_t) = nullptr;
    PropertyReply *(*getPropertyReply)(xcb_connection_t *, PropertyCookie, void **) = nullptr;
    void *(*value)(const PropertyReply *) = nullptr;
    int (*valueLength)(const PropertyReply *) = nullptr;
    const void *(*setup)(xcb_connection_t *) = nullptr;
    ScreenIterator (*roots)(const void *) = nullptr;
    xcb_connection_t *connection = nullptr;

    // libxcb as Qt loaded it, and Qt's connection; false when Qt is not on X11.
    bool load()
    {
        auto *x11 = qGuiApp ? qGuiApp->nativeInterface<QNativeInterface::QX11Application>() : nullptr;
        connection = x11 ? x11->connection() : nullptr;
        void *library = connection ? dlopen("libxcb.so.1", RTLD_NOW | RTLD_NOLOAD) : nullptr;
        if (!library)
            return false;
        auto symbol = [library](auto &function, const char *name) {
            function = reinterpret_cast<std::remove_reference_t<decltype(function)>>(dlsym(library, name));
            return function != nullptr;
        };
        const bool found = symbol(intern, "xcb_intern_atom") && symbol(internReply, "xcb_intern_atom_reply")
            && symbol(getProperty, "xcb_get_property") && symbol(getPropertyReply, "xcb_get_property_reply")
            && symbol(value, "xcb_get_property_value") && symbol(valueLength, "xcb_get_property_value_length")
            && symbol(setup, "xcb_get_setup") && symbol(roots, "xcb_setup_roots_iterator");
        // The handle stays open as long as Qt keeps the library: dropping ours changes nothing.
        dlclose(library);
        return found;
    }

    uint32_t root() const
    {
        const ScreenIterator first = roots(setup(connection));
        return first.data ? first.data->root : 0;
    }

    uint32_t atom(const QByteArray &name) const
    {
        void *error = nullptr;
        AtomReply *reply = internReply(connection, intern(connection, 0, uint16_t(name.size()), name.constData()), &error);
        std::free(error);
        const uint32_t atom = reply ? reply->atom : 0;
        std::free(reply);
        return atom;
    }

    // A property of 8-bit values on the window, whole (a profile with tables
    // may weigh a megabyte; Qt's own reading stops at 32 KB); empty when unset.
    QByteArray property(uint32_t window, uint32_t atom) const
    {
        if (!atom)
            return {};
        void *error = nullptr;
        // AnyPropertyType (0); up to 64 MB, in 32-bit units.
        PropertyReply *reply = getPropertyReply(connection, getProperty(connection, 0, window, atom, 0, 0, 16 * 1024 * 1024), &error);
        std::free(error);
        QByteArray bytes;
        if (reply && reply->format == 8)
            bytes = QByteArray(static_cast<const char *>(value(reply)), valueLength(reply));
        std::free(reply);
        return bytes;
    }
};
#endif

#ifdef SIOUL_COLORD
// colord's checksums (MD5) of the profiles it keeps for an output, named as
// X11 names it ("eDP-1"); an empty list when colord does not know the output,
// nothing when colord does not answer. Read only, over the system bus.
std::optional<QStringList> colordChecksums(const QString &output)
{
    QDBusConnection bus = QDBusConnection::systemBus();
    if (!bus.isConnected())
        return std::nullopt;
    const QString service = QStringLiteral("org.freedesktop.ColorManager");
    QDBusMessage find = QDBusMessage::createMethodCall(service, QStringLiteral("/org/freedesktop/ColorManager"), service, QStringLiteral("FindDeviceByProperty"));
    find << QStringLiteral("XRANDR_name") << output;
    const QDBusMessage device = bus.call(find, QDBus::Block, 400);
    if (device.type() != QDBusMessage::ReplyMessage) {
        // colord answered that it does not know this output; anything else: no colord.
        if (device.errorName().startsWith(service))
            return QStringList();
        return std::nullopt;
    }
    auto get = [&](const QString &path, const QString &interface, const QString &name) -> QVariant {
        QDBusMessage ask = QDBusMessage::createMethodCall(service, path, QStringLiteral("org.freedesktop.DBus.Properties"), QStringLiteral("Get"));
        ask << interface << name;
        const QDBusMessage answer = bus.call(ask, QDBus::Block, 400);
        if (answer.type() != QDBusMessage::ReplyMessage || answer.arguments().isEmpty())
            return {};
        return answer.arguments().first().value<QDBusVariant>().variant();
    };
    const QString path = device.arguments().value(0).value<QDBusObjectPath>().path();
    const QVariant listed = get(path, QStringLiteral("org.freedesktop.ColorManager.Device"), QStringLiteral("Profiles"));
    QList<QDBusObjectPath> profiles;
    if (listed.metaType() == QMetaType::fromType<QDBusArgument>())
        listed.value<QDBusArgument>() >> profiles;
    else
        profiles = listed.value<QList<QDBusObjectPath>>();
    QStringList sums;
    for (const QDBusObjectPath &profile : std::as_const(profiles)) {
        // "icc-<MD5 of the file>", as colord names a profile it read from a file.
        const QString id = get(profile.path(), QStringLiteral("org.freedesktop.ColorManager.Profile"), QStringLiteral("ProfileId")).toString();
        if (id.startsWith(QLatin1String("icc-")))
            sums << id.mid(4).toLower();
        const QVariant metadata = get(profile.path(), QStringLiteral("org.freedesktop.ColorManager.Profile"), QStringLiteral("Metadata"));
        QMap<QString, QString> values;
        if (metadata.metaType() == QMetaType::fromType<QDBusArgument>())
            metadata.value<QDBusArgument>() >> values;
        if (const QString sum = values.value(QStringLiteral("FILE_checksum")); !sum.isEmpty())
            sums << sum.toLower();
    }
    return sums;
}
#endif

} // namespace

// The root window's property changes, seen among X11's events (Qt already
// listens to them), for ScreenColours.
class ProfileWatcher : public QAbstractNativeEventFilter
{
public:
    explicit ProfileWatcher(ScreenColours *colours)
        : m_colours(colours)
    {
    }

    bool nativeEventFilter(const QByteArray &eventType, void *message, qintptr *) override
    {
        if (m_colours->profileChanged(eventType, message))
            m_colours->later();
        return false;
    }

private:
    ScreenColours *m_colours;
};

namespace {

// The tables as images Qt Quick uploads as half floats (RGBA16F), the
// precision the cube's negative values and fine steps need.
QImage halfFloats(const std::vector<float> &values, int width, int height)
{
    QImage image(width, height, QImage::Format_RGBX16FPx4);
    for (int y = 0; y < height; ++y) {
        auto *line = reinterpret_cast<qfloat16 *>(image.scanLine(y));
        for (int x = 0; x < width * 4; ++x)
            line[x] = qfloat16(values[size_t(y) * size_t(width) * 4 + size_t(x)]);
    }
    return image;
}

// The image provider: "image://sioul-colour/cube/0/12".
class ColourTables : public QQuickImageProvider
{
public:
    explicit ColourTables(ScreenColours *colours)
        : QQuickImageProvider(QQuickImageProvider::Image)
        , m_colours(colours)
    {
    }

    QImage requestImage(const QString &id, QSize *size, const QSize &) override
    {
        QImage image = m_colours ? m_colours->image(id) : QImage();
        if (image.isNull()) {
            image = QImage(1, 1, QImage::Format_RGBX16FPx4);
            image.fill(Qt::black);
        }
        if (size)
            *size = image.size();
        return image;
    }

private:
    QPointer<ScreenColours> m_colours;
};

} // namespace

ScreenColours *ScreenColours::create(QQmlEngine *engine, QJSEngine *)
{
    auto *colours = new ScreenColours();
    if (engine)
        engine->addImageProvider(QStringLiteral("sioul-colour"), new ColourTables(colours));
    return colours;
}

ScreenColours::ScreenColours(QObject *parent)
    : QObject(parent)
{
    m_later.setSingleShot(true);
    m_later.setInterval(150);
    connect(&m_later, &QTimer::timeout, this, &ScreenColours::refresh);
    // A screen plugged in, taken out, or made the primary one.
    connect(qGuiApp, &QGuiApplication::screenAdded, this, &ScreenColours::later);
    connect(qGuiApp, &QGuiApplication::screenRemoved, this, &ScreenColours::later);
    connect(qGuiApp, &QGuiApplication::primaryScreenChanged, this, &ScreenColours::later);
    refresh();
}

ScreenColours::~ScreenColours()
{
    if (qGuiApp && m_watcher)
        qGuiApp->removeNativeEventFilter(m_watcher.get());
}

bool ScreenColours::screenConversion() const
{
    return m_screenConversion;
}

void ScreenColours::setScreenConversion(bool on)
{
    if (m_screenConversion == on)
        return;
    m_screenConversion = on;
    later();
}

int ScreenColours::calmer() const
{
    return m_calmer;
}

void ScreenColours::setCalmer(int calmer)
{
    calmer = qBound(0, calmer, 2);
    if (m_calmer == calmer)
        return;
    m_calmer = calmer;
    later();
}

int ScreenColours::revision() const
{
    return m_revision;
}

int ScreenColours::cubeSize()
{
    return Cube;
}

bool ScreenColours::active(const QString &screen) const
{
    // Qt Quick's software renderer draws no shader: the page as it is, rather than nothing.
    if (QQuickWindow::graphicsApi() == QSGRendererInterface::Software)
        return false;
    QMutexLocker lock(&m_mutex);
    const QString name = m_tables.contains(screen) || m_screens.isEmpty() ? screen : m_screens.first();
    return !m_tables.value(name).identity;
}

QString ScreenColours::source(const QString &which, const QString &screen) const
{
    QMutexLocker lock(&m_mutex);
    const int number = qMax(0, m_screens.indexOf(screen));
    return QStringLiteral("image://sioul-colour/%1/%2/%3").arg(which).arg(number).arg(m_revision);
}

QImage ScreenColours::image(const QString &id) const
{
    QMutexLocker lock(&m_mutex);
    const QStringList parts = id.split(QLatin1Char('/'));
    const QString name = m_screens.value(parts.value(1).toInt());
    const Tables tables = m_tables.value(name);
    return parts.value(0) == QLatin1String("curve") ? tables.curve : tables.cube;
}

void ScreenColours::later()
{
    m_later.start();
}

bool ScreenColours::profileChanged(const QByteArray &eventType, const void *message) const
{
#ifdef SIOUL_X11
    if (eventType == "xcb_generic_event_t" && m_root) {
        const auto *event = static_cast<const PropertyNotify *>(message);
        return (event->responseType & 0x7f) == PropertyNotifyEvent && event->window == m_root && m_atoms.contains(event->atom);
    }
#else
    Q_UNUSED(eventType);
    Q_UNUSED(message);
#endif
    return false;
}

QHash<QString, QByteArray> ScreenColours::profiles(int *who)
{
    QHash<QString, QByteArray> found;
    const QList<QScreen *> screens = QGuiApplication::screens();
#if defined(Q_OS_MACOS)
    // macOS converts every window, Sioul's tagged as sRGB (cpp/webengine.cpp).
    *who = 1;
    return found;
#endif
    // A Wayland session converts every window, Xwayland's too (KWin, Mutter).
    const bool wayland = QGuiApplication::platformName() == QLatin1String("wayland")
        || qEnvironmentVariable("XDG_SESSION_TYPE") == QLatin1String("wayland") || qEnvironmentVariableIsSet("WAYLAND_DISPLAY");
    if (wayland) {
        *who = 1;
        return found;
    }
    *who = 2;
#ifdef SIOUL_X11
    static Xcb xcb;
    static const bool loaded = QGuiApplication::platformName() == QLatin1String("xcb") && xcb.load();
    if (!loaded)
        return found;
    if (m_atoms.isEmpty()) {
        m_root = xcb.root();
        // The atoms of up to eight monitors (ICC Profiles in X 0.4: "_ICC_PROFILE",
        // then "_ICC_PROFILE_1"…), and the one a colour-managing compositor sets.
        for (int i = 0; i < 8; ++i)
            m_atoms << xcb.atom(i == 0 ? QByteArray("_ICC_PROFILE") : "_ICC_PROFILE_" + QByteArray::number(i));
        m_atoms << xcb.atom("_ICC_COLOR_DESKTOP");
        m_watcher = std::make_unique<ProfileWatcher>(this);
        qGuiApp->installNativeEventFilter(m_watcher.get());
    }
    if (!m_root || m_atoms.size() < 9)
        return found;
    if (!xcb.property(m_root, m_atoms.at(8)).isEmpty()) {
        *who = 1;
        return found;
    }
    QList<QByteArray> atoms;
    for (int i = 0; i < 8; ++i)
        atoms << xcb.property(m_root, m_atoms.at(i));
    *who = 0;
    if (screens.size() <= 1) {
        if (!screens.isEmpty())
            found.insert(screens.first()->name(), atoms.first());
        return found;
    }
    // Several screens: the daemons that publish the atoms number them
    // differently (docs/colour.md). colord tells which profile belongs to
    // which output: an atom goes to the screen whose profiles hold its MD5.
    // Without colord, the specification's order: the primary screen first,
    // then the others as Qt lists them, which is X11's order.
    std::optional<QHash<QString, QStringList>> sums = QHash<QString, QStringList>();
#ifdef SIOUL_COLORD
    for (QScreen *screen : screens) {
        const std::optional<QStringList> of = colordChecksums(screen->name());
        if (!of) {
            sums.reset();
            break;
        }
        sums->insert(screen->name(), *of);
    }
#else
    sums.reset();
#endif
    for (int i = 0; i < screens.size(); ++i) {
        const QString name = screens.at(i)->name();
        if (!sums) {
            found.insert(name, atoms.value(i));
            continue;
        }
        for (const QByteArray &atom : std::as_const(atoms)) {
            if (!atom.isEmpty() && sums->value(name).contains(QString::fromLatin1(QCryptographicHash::hash(atom, QCryptographicHash::Md5).toHex()))) {
                found.insert(name, atom);
                break;
            }
        }
    }
#endif
    return found;
}

void ScreenColours::refresh()
{
    int who = 2;
    const QHash<QString, QByteArray> found = profiles(&who);
    QStringList names;
    for (QScreen *screen : QGuiApplication::screens())
        names << screen->name();
    // The tables of each screen: one making per distinct profile.
    QHash<QString, Tables> made;
    QHash<QByteArray, Tables> byProfile;
    QHash<QString, QString> profileNames;
    for (const QString &name : std::as_const(names)) {
        QByteArray profile = m_screenConversion && who == 0 ? found.value(name) : QByteArray();
        if (!byProfile.contains(profile)) {
            std::vector<float> cube(size_t(Cube) * Cube * Cube * 4), curve(size_t(Curve) * 4);
            auto bytes = reinterpret_cast<const unsigned char *>(profile.constData());
            int result = sioul_colour_tables(profile.isEmpty() ? nullptr : bytes, size_t(profile.size()), m_calmer, Cube, cube.data(), Curve, curve.data());
            // A profile that cannot be used is as no profile: calmer colours still apply.
            if (result == -1) {
                profile.clear();
                result = sioul_colour_tables(nullptr, 0, m_calmer, Cube, cube.data(), Curve, curve.data());
            }
            Tables tables;
            if (result == 1) {
                tables.identity = false;
                tables.cube = halfFloats(cube, Cube * Cube, Cube);
                tables.curve = halfFloats(curve, Curve, 1);
            }
            byProfile.insert(profile, tables);
        }
        made.insert(name, byProfile.value(profile));
        if (!profile.isEmpty() && !made.value(name).identity) {
            unsigned char said[256];
            const size_t length = sioul_colour_profile_name(reinterpret_cast<const unsigned char *>(profile.constData()), size_t(profile.size()), said, sizeof said);
            profileNames.insert(name, QString::fromUtf8(reinterpret_cast<const char *>(said), qsizetype(length)));
        }
    }
    {
        QMutexLocker lock(&m_mutex);
        m_screens = names;
        m_tables = made;
        ++m_revision;
    }
    // What Settings ▸ Display says, of the primary screen: converted (to which
    // profile), by the desktop, nothing (no profile, or an sRGB one), turned off.
    const QString primary = QGuiApplication::primaryScreen() ? QGuiApplication::primaryScreen()->name() : QString();
    int state = 2;
    if (!m_screenConversion)
        state = 3;
    else if (who == 1)
        state = 1;
    else if (who == 0 && profileNames.contains(primary))
        state = 0;
    sioul_colour_said(state, state == 0 ? profileNames.value(primary).toUtf8().constData() : nullptr);
    Q_EMIT changed();
}
