// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Sioul on Android: Qt for Android loads this library and calls main(). The
// window is Rust's (crates/sioul-app/src/lib.rs). Here, what a desktop gives
// a program and Android does not: the folders, named the XDG way; the
// language; the system's certificates; a log for what Rust writes to stderr;
// and the Java side Rust needs for the KeyStore and the DNS servers.

#include <QByteArray>
#include <QDir>
#include <QJniEnvironment>
#include <QJniObject>
#include <QString>
#include <QtCore/qcoreapplication_platform.h>

#include <android/log.h>
#include <pthread.h>
#include <stdio.h>
#include <unistd.h>

extern "C" int sioul_app_run();
extern "C" bool sioul_android_init(void *vm, void *context);

namespace {

// Android throws away what a program writes to stdout and stderr: Rust's
// messages and panics go to logcat instead, line by line, tagged "sioul".
int logPipe[2];

void *forwardLines(void *)
{
    FILE *lines = fdopen(logPipe[0], "r");
    if (!lines)
        return nullptr;
    char line[2048];
    while (fgets(line, sizeof line, lines))
        __android_log_write(ANDROID_LOG_INFO, "sioul", line);
    return nullptr;
}

void sendOutputToLogcat()
{
    setvbuf(stdout, nullptr, _IOLBF, 0);
    setvbuf(stderr, nullptr, _IONBF, 0);
    if (pipe(logPipe) != 0)
        return;
    dup2(logPipe[1], STDOUT_FILENO);
    dup2(logPipe[1], STDERR_FILENO);
    pthread_t thread;
    if (pthread_create(&thread, nullptr, forwardLines, nullptr) == 0)
        pthread_detach(thread);
}

// A variable of the environment, set unless it is already.
void setDefault(const char *name, const QByteArray &value)
{
    if (qEnvironmentVariableIsEmpty(name))
        qputenv(name, value);
}

// The application's Context: lives as long as the process.
QJniObject applicationContext()
{
    const QJniObject context = QNativeInterface::QAndroidApplication::context();
    if (!context.isValid())
        return {};
    return context.callObjectMethod("getApplicationContext", "()Landroid/content/Context;");
}

// Sioul's folders, the XDG way, in the app's private storage, which no other
// app reads: configuration, data and state among its files, the cache where
// Android may empty it.
void setFolders(const QJniObject &context)
{
    const auto folder = [&context](const char *method) {
        return context.callObjectMethod(method, "()Ljava/io/File;").callObjectMethod("getAbsolutePath", "()Ljava/lang/String;").toString();
    };
    const QString files = folder("getFilesDir");
    const QString cache = folder("getCacheDir");
    if (files.isEmpty())
        return;
    setDefault("HOME", files.toUtf8());
    setDefault("XDG_CONFIG_HOME", (files + QStringLiteral("/config")).toUtf8());
    setDefault("XDG_DATA_HOME", (files + QStringLiteral("/data")).toUtf8());
    setDefault("XDG_STATE_HOME", (files + QStringLiteral("/state")).toUtf8());
    if (!cache.isEmpty())
        setDefault("XDG_CACHE_HOME", cache.toUtf8());
}

// The phone's language, from Java's Locale ("fr-FR"), where Sioul reads it on Linux.
void setLanguage()
{
    const QJniObject locale = QJniObject::callStaticObjectMethod("java/util/Locale", "getDefault", "()Ljava/util/Locale;");
    QString tag = locale.callObjectMethod("toLanguageTag", "()Ljava/lang/String;").toString();
    if (!tag.isEmpty())
        setDefault("LANG", tag.replace(QLatin1Char('-'), QLatin1Char('_')).toUtf8() + ".UTF-8");
}

// The system's certificates for TLS, read through SSL_CERT_DIR: those Android
// keeps up to date (its Conscrypt module, Android 14 and later), else the
// system image's.
void setCertificates()
{
    for (const char *dir : { "/apex/com.android.conscrypt/cacerts", "/system/etc/security/cacerts" }) {
        if (QDir(QString::fromLatin1(dir)).exists()) {
            setDefault("SSL_CERT_DIR", dir);
            return;
        }
    }
}

} // namespace

int main(int, char *[])
{
    sendOutputToLogcat();
    const QJniObject context = applicationContext();
    if (context.isValid()) {
        setFolders(context);
        // Lent to Rust for good: a global reference, never released.
        QJniEnvironment env;
        jobject kept = env->NewGlobalRef(context.object());
        sioul_android_init(QJniEnvironment::javaVM(), kept);
    } else {
        __android_log_write(ANDROID_LOG_ERROR, "sioul", "No Android Context: no keyring, no DNS servers for sender checks.");
    }
    setLanguage();
    setCertificates();
    return sioul_app_run();
}
