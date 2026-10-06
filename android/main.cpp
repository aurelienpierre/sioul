// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Sioul on Android: Qt for Android loads this library and calls main(). The
// window is Rust's (crates/sioul-app/src/lib.rs). Here, what a desktop gives
// a program and Android does not: the folders, named the XDG way; the
// language; the system's certificates; a log for what Rust writes to stderr;
// the Java side Rust needs for the KeyStore and the DNS servers; the doses'
// alarms (package/src/com/aurelienpierre/sioul/DoseAlarms.java); the time
// running's notification (TimeNote.java); the alarm at waking
// (WakeAlarms.java); the events' reminders and new mail's notification
// (EventAlarms.java, MailNotes.java); the pauses' do-not-disturb (PauseMode.java); do-not-disturb
// on every device and the phone kept in step in the background (StepService.java,
// DndReceiver.java, DndContacts.java); other apps' notifications held until
// their time (AppNotes.java, in a process of its own); and your
// addresses offered in the share sheet (MailShortcuts.java), with what other
// apps share to Sioul (ShareActivity.java).
//
// Android may start Sioul for a dose's alarm, a button of the time running,
// or the alarm at waking, alone: DoseAlarms.java then loads this library
// without Qt's Java side, which only Qt's loader starts, when the window
// opens. So all they reach here (the start, the sync apps' broadcasts, the
// alarms, the tapped dose, the time running, the wakings) speaks to Java
// directly (JNI), never through Qt's (QJniObject).

#include <QByteArray>
#include <QDir>
#include <QGuiApplication>
#include <QJniObject>
#include <QQuickWindow>
#include <QString>
#include <QtCore/qcoreapplication_platform.h>
#include <QtCore/private/qandroidextras_p.h>

#include <android/api-level.h>
#include <android/log.h>
#include <jni.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include <atomic>
#include <mutex>

extern "C" int sioul_app_run();
extern "C" bool sioul_android_init(void *vm, void *context);
extern "C" void sioul_android_account_chosen(const char *name, const char *kind);
// At a dose's alarm (crates/sioul-app/src/alarms.rs): JSON texts, given back
// to sioul_string_free.
extern "C" char *sioul_alarm_decide(const char *key);
extern "C" char *sioul_alarm_taken(const char *key);
// A button of the time running (crates/sioul-app/src/timenote.rs): the note
// shown next, as JSON, also given back to sioul_string_free.
extern "C" char *sioul_time_action(const char *action);
// The alarm at waking's coming rings (crates/sioul-app/src/wake.rs), in the
// phone's zone, as JSON; also given back to sioul_string_free.
extern "C" char *sioul_wake_next(const char *zone, bool fetch);
extern "C" void sioul_string_free(char *text);
// Something shared to Sioul, or the files copied for it (crates/sioul-app/src/outside.rs).
extern "C" void sioul_handed();
// At an event's reminder, and the coming ones in the phone's zone
// (crates/sioul-app/src/eventalarms.rs): JSON, given back to sioul_string_free.
extern "C" char *sioul_event_decide(const char *key);
extern "C" char *sioul_event_coming(const char *zone);
// Do-not-disturb on every device and the phone in the background
// (crates/sioul-app/src/steps.rs, everywhere.rs): a step of the background
// service, its process said, do-not-disturb applied in Sioul's own process;
// JSON answers given back to sioul_string_free.
extern "C" char *sioul_steps_step(const char *reason);
extern "C" void sioul_steps_in_service();
extern "C" char *sioul_dnd_apply();
// Other apps' notifications (crates/sioul-app/src/appnotes.rs): one decided,
// and what is held worked out again; JSON, given back to sioul_string_free.
extern "C" char *sioul_appnotes_decide(const char *json);
extern "C" char *sioul_appnotes_review(const char *json);

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

// The application's Context, as Qt has it: for the window's own questions.
QJniObject applicationContext()
{
    const QJniObject context = QNativeInterface::QAndroidApplication::context();
    if (!context.isValid())
        return {};
    return context.callObjectMethod("getApplicationContext", "()Landroid/content/Context;");
}

// Java, without Qt: the process's JavaVM, given when Java loads this library;
// the application's Context, given at the start; DoseAlarms.java and
// TimeNote.java, found when the library is loaded, by the app's class
// loader, which Rust's threads lack.
JavaVM *javaVm = nullptr;
std::atomic<jobject> appContext = nullptr;
jclass doseAlarms = nullptr;
jclass timeNote = nullptr;
jclass wakeAlarms = nullptr;
jclass pauseMode = nullptr;
jclass mailShortcuts = nullptr;
jclass eventAlarms = nullptr;
jclass mailNotes = nullptr;
jclass stepService = nullptr;
jclass appNotes = nullptr;

// This thread's JNIEnv. A thread of Rust's is attached to Java the first time,
// and let go when it ends.
JNIEnv *jni()
{
    if (!javaVm)
        return nullptr;
    JNIEnv *env = nullptr;
    if (javaVm->GetEnv(reinterpret_cast<void **>(&env), JNI_VERSION_1_6) == JNI_OK)
        return env;
    static pthread_key_t attached;
    static const bool keyed = pthread_key_create(&attached, [](void *) { javaVm->DetachCurrentThread(); }) == 0;
    if (!keyed || javaVm->AttachCurrentThread(&env, nullptr) != JNI_OK)
        return nullptr;
    pthread_setspecific(attached, env);
    return env;
}

// Java's local references made meanwhile, freed with it: on a thread of
// Rust's, nothing else frees them.
struct LocalFrame
{
    JNIEnv *env;
    explicit LocalFrame(JNIEnv *env)
        : env(env)
    {
        env->PushLocalFrame(16);
    }
    ~LocalFrame() { env->PopLocalFrame(nullptr); }
};

// Whether Java threw: the exception then logged and cleared.
bool threw(JNIEnv *env)
{
    if (!env->ExceptionCheck())
        return false;
    env->ExceptionDescribe();
    env->ExceptionClear();
    return true;
}

// Texts to and from Java as UTF-16: what JNI calls UTF-8 is not quite it.
jstring javaText(JNIEnv *env, const char *text)
{
    const QString utf16 = QString::fromUtf8(text);
    return env->NewString(reinterpret_cast<const jchar *>(utf16.utf16()), jsize(utf16.size()));
}

QByteArray utf8(JNIEnv *env, jstring text)
{
    if (!text)
        return {};
    const jchar *chars = env->GetStringChars(text, nullptr);
    if (!chars)
        return {};
    const QByteArray bytes = QString(reinterpret_cast<const QChar *>(chars), env->GetStringLength(text)).toUtf8();
    env->ReleaseStringChars(text, chars);
    return bytes;
}

// What a method of `object` without arguments answers, a text; "" when it fails.
QByteArray textOf(JNIEnv *env, jobject object, const char *method)
{
    if (!object)
        return {};
    const jmethodID id = env->GetMethodID(env->GetObjectClass(object), method, "()Ljava/lang/String;");
    if (threw(env) || !id)
        return {};
    const auto text = static_cast<jstring>(env->CallObjectMethod(object, id));
    return threw(env) ? QByteArray() : utf8(env, text);
}

// A static method of DoseAlarms.java; null when it is not there.
jmethodID doseMethod(JNIEnv *env, const char *name, const char *signature)
{
    if (!doseAlarms)
        return nullptr;
    const jmethodID id = env->GetStaticMethodID(doseAlarms, name, signature);
    return threw(env) ? nullptr : id;
}

// A static method of TimeNote.java; null when it is not there.
jmethodID timeMethod(JNIEnv *env, const char *name, const char *signature)
{
    if (!timeNote)
        return nullptr;
    const jmethodID id = env->GetStaticMethodID(timeNote, name, signature);
    return threw(env) ? nullptr : id;
}

// A static method of WakeAlarms.java; null when it is not there.
jmethodID wakeMethod(JNIEnv *env, const char *name, const char *signature)
{
    if (!wakeAlarms)
        return nullptr;
    const jmethodID id = env->GetStaticMethodID(wakeAlarms, name, signature);
    return threw(env) ? nullptr : id;
}

// One of the app's classes, kept for Rust's threads; null when it is not there.
jclass appClass(JNIEnv *env, const char *name)
{
    const jclass found = env->FindClass(name);
    if (threw(env) || !found)
        return nullptr;
    const auto kept = static_cast<jclass>(env->NewGlobalRef(found));
    env->DeleteLocalRef(found);
    return kept;
}

// Sioul's folders, the XDG way, in the app's private storage, which no other
// app reads: configuration, data and state among its files, the cache where
// Android may empty it.
void setFolders(JNIEnv *env, jobject context)
{
    const auto folder = [env, context](const char *method) {
        const jmethodID id = env->GetMethodID(env->GetObjectClass(context), method, "()Ljava/io/File;");
        if (threw(env) || !id)
            return QByteArray();
        const jobject file = env->CallObjectMethod(context, id);
        return threw(env) ? QByteArray() : textOf(env, file, "getAbsolutePath");
    };
    const QByteArray files = folder("getFilesDir");
    const QByteArray cache = folder("getCacheDir");
    if (files.isEmpty())
        return;
    setDefault("HOME", files);
    setDefault("XDG_CONFIG_HOME", files + "/config");
    setDefault("XDG_DATA_HOME", files + "/data");
    setDefault("XDG_STATE_HOME", files + "/state");
    if (!cache.isEmpty())
        setDefault("XDG_CACHE_HOME", cache);
}

// The phone's language, from Java's Locale ("fr-FR"), where Sioul reads it on Linux.
void setLanguage(JNIEnv *env)
{
    const jclass locales = env->FindClass("java/util/Locale");
    const jmethodID current = locales ? env->GetStaticMethodID(locales, "getDefault", "()Ljava/util/Locale;") : nullptr;
    if (threw(env) || !current)
        return;
    const jobject locale = env->CallStaticObjectMethod(locales, current);
    if (threw(env))
        return;
    QByteArray tag = textOf(env, locale, "toLanguageTag");
    if (!tag.isEmpty())
        setDefault("LANG", tag.replace('-', '_') + ".UTF-8");
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

// The start, once per process: by main() when Sioul opens, or first by
// DoseAlarms.java when Android starts Sioul for a dose's alarm alone.
void initialize(JNIEnv *env, jobject context)
{
    static std::once_flag once;
    std::call_once(once, [env, context] {
        sendOutputToLogcat();
        if (env) {
            const LocalFrame frame(env);
            if (context) {
                setFolders(env, context);
                // Lent to Rust for good: a global reference, never released.
                const jobject kept = env->NewGlobalRef(context);
                appContext.store(kept);
                sioul_android_init(javaVm, kept);
            }
            setLanguage(env);
        }
        if (!env || !context)
            __android_log_write(ANDROID_LOG_ERROR, "sioul", "No Android Context: no keyring, no DNS servers for sender checks, no doses' alarms.");
        setCertificates();
    });
}

// From Android 13, reminders need your yes: asked once a run, the first time
// doses are given while Sioul is on the screen, never as it is put away.
// Qt's question, which needs Qt's Java side: there only with the window.
void askNotifications()
{
    static std::atomic<bool> asked = false;
    if (android_get_device_api_level() < 33 || !QtAndroidPrivate::javaVM() || !qGuiApp
        || QGuiApplication::applicationState() != Qt::ApplicationActive)
        return;
    const QString permission = QStringLiteral("android.permission.POST_NOTIFICATIONS");
    if (QtAndroidPrivate::checkPermission(permission).result() == QtAndroidPrivate::Authorized || asked.exchange(true))
        return;
    QtAndroidPrivate::requestPermission(permission);
}

// What Rust answers at a dose's alarm, as a Java text.
jstring answered(JNIEnv *env, char *answer)
{
    const jstring text = answer ? javaText(env, answer) : nullptr;
    sioul_string_free(answer);
    return text;
}

} // namespace

// Android calls a library's JNI_OnLoad when Java loads it; for one without,
// the first it finds among the libraries it needs: Qt Multimedia's, which
// needs Qt's Java side started. This one keeps what the doses' alarms need,
// and starts nothing of Qt's: Qt's loader does, if the window opens.
extern "C" JNIEXPORT jint JNI_OnLoad(JavaVM *vm, void *)
{
    javaVm = vm;
    JNIEnv *env = nullptr;
    if (vm->GetEnv(reinterpret_cast<void **>(&env), JNI_VERSION_1_6) != JNI_OK)
        return JNI_ERR;
    if (!doseAlarms)
        doseAlarms = appClass(env, "com/aurelienpierre/sioul/DoseAlarms");
    if (!timeNote)
        timeNote = appClass(env, "com/aurelienpierre/sioul/TimeNote");
    if (!wakeAlarms)
        wakeAlarms = appClass(env, "com/aurelienpierre/sioul/WakeAlarms");
    if (!pauseMode)
        pauseMode = appClass(env, "com/aurelienpierre/sioul/PauseMode");
    if (!mailShortcuts)
        mailShortcuts = appClass(env, "com/aurelienpierre/sioul/MailShortcuts");
    if (!eventAlarms)
        eventAlarms = appClass(env, "com/aurelienpierre/sioul/EventAlarms");
    if (!mailNotes)
        mailNotes = appClass(env, "com/aurelienpierre/sioul/MailNotes");
    if (!stepService)
        stepService = appClass(env, "com/aurelienpierre/sioul/StepService");
    if (!appNotes)
        appNotes = appClass(env, "com/aurelienpierre/sioul/AppNotes");
    return JNI_VERSION_1_6;
}

// Android's own chooser of the phone's accounts (Murena, Google, another
// address): the one you pick, by its name (an address, usually) and its kind,
// goes back to Rust (crates/sioul-app/src/backend.rs). Android lends no
// password: Sioul finds the servers and asks it once, as on a computer.
extern "C" void sioul_android_choose_account()
{
    const QJniObject intent = QJniObject::callStaticObjectMethod(
        "android/accounts/AccountManager", "newChooseAccountIntent",
        "(Landroid/accounts/Account;Ljava/util/List;[Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;[Ljava/lang/String;Landroid/os/Bundle;)Landroid/content/Intent;",
        nullptr, nullptr, nullptr, nullptr, nullptr, nullptr, nullptr);
    if (!intent.isValid()) {
        sioul_android_account_chosen("", "");
        return;
    }
    QtAndroidPrivate::startActivity(intent, 4242, [](int, int result, const QJniObject &data) {
        // Activity.RESULT_OK is -1; AccountManager.KEY_ACCOUNT_NAME, KEY_ACCOUNT_TYPE.
        if (result != -1 || !data.isValid()) {
            sioul_android_account_chosen("", "");
            return;
        }
        const auto extra = [&data](const char *key) {
            return data.callObjectMethod("getStringExtra", "(Ljava/lang/String;)Ljava/lang/String;", QJniObject::fromString(QString::fromLatin1(key)).object<jstring>()).toString().toUtf8();
        };
        const QByteArray name = extra("authAccount");
        const QByteArray kind = extra("accountType");
        sioul_android_account_chosen(name.constData(), kind.constData());
    });
}

// Whether Sioul may reach your files by their path, as the sharing folder
// your sync app carries is: "All files access" (Android 11 and later), else
// the storage permission of older versions.
extern "C" bool sioul_android_files_access()
{
    if (QNativeInterface::QAndroidApplication::sdkVersion() >= 30)
        return QJniObject::callStaticMethod<jboolean>("android/os/Environment", "isExternalStorageManager");
    return QtAndroidPrivate::checkPermission(QStringLiteral("android.permission.WRITE_EXTERNAL_STORAGE")).result() == QtAndroidPrivate::Authorized;
}

// Android's own switch for it: its settings page for Sioul, or the question.
extern "C" void sioul_android_ask_files_access()
{
    if (QNativeInterface::QAndroidApplication::sdkVersion() < 30) {
        QtAndroidPrivate::requestPermission(QStringLiteral("android.permission.WRITE_EXTERNAL_STORAGE"));
        return;
    }
    const QJniObject context = applicationContext();
    const QString package = context.callObjectMethod("getPackageName", "()Ljava/lang/String;").toString();
    const QJniObject uri = QJniObject::callStaticObjectMethod("android/net/Uri", "parse", "(Ljava/lang/String;)Landroid/net/Uri;",
                                                              QJniObject::fromString(QStringLiteral("package:") + package).object<jstring>());
    const QJniObject intent("android/content/Intent", "(Ljava/lang/String;Landroid/net/Uri;)V",
                            QJniObject::fromString(QStringLiteral("android.settings.MANAGE_APP_ALL_FILES_ACCESS_PERMISSION")).object<jstring>(),
                            uri.object());
    QtAndroidPrivate::startActivity(intent, 4243, static_cast<QAndroidActivityResultReceiver *>(nullptr));
}

// A sync app asked to look for changes now rather than at its next round
// (Murena's eDrive looks every half hour): a broadcast to its receiver, by
// its package, its receiver's class and the action it waits for (the known
// sync apps are Rust's, crates/sioul-app/src/share.rs). What Sioul wrote goes
// up and what the other devices wrote comes down in seconds. Without that
// app, nothing happens. Also at a dose's alarm, without Qt.
extern "C" void sioul_android_broadcast(const char *package, const char *receiver, const char *action)
{
    JNIEnv *env = jni();
    const jobject context = appContext.load();
    if (!env || !context || !package || !receiver || !action)
        return;
    const LocalFrame frame(env);
    const jclass intents = env->FindClass("android/content/Intent");
    const jmethodID make = intents ? env->GetMethodID(intents, "<init>", "(Ljava/lang/String;)V") : nullptr;
    const jmethodID name = intents ? env->GetMethodID(intents, "setClassName", "(Ljava/lang/String;Ljava/lang/String;)Landroid/content/Intent;") : nullptr;
    const jmethodID send = env->GetMethodID(env->GetObjectClass(context), "sendBroadcast", "(Landroid/content/Intent;)V");
    if (threw(env) || !make || !name || !send)
        return;
    const jobject intent = env->NewObject(intents, make, javaText(env, action));
    if (threw(env) || !intent)
        return;
    env->CallObjectMethod(intent, name, javaText(env, package), javaText(env, receiver));
    if (threw(env))
        return;
    env->CallVoidMethod(context, send, intent);
    threw(env);
}

// The doses of the next two days, given to Android's alarm clock in place of
// those given before (crates/sioul-app/src/alarms.rs): JSON [{key, at (Unix
// milliseconds), title}]. DoseAlarms.java keeps the list for Android's
// restarts. Any thread.
extern "C" void sioul_android_set_alarms(const char *json)
{
    JNIEnv *env = jni();
    const jobject context = appContext.load();
    if (!env || !context || !json)
        return;
    {
        const LocalFrame frame(env);
        const jmethodID set = doseMethod(env, "set", "(Landroid/content/Context;Ljava/lang/String;)V");
        if (!set)
            return;
        env->CallStaticVoidMethod(doseAlarms, set, context, javaText(env, json));
        threw(env);
    }
    if (strstr(json, "\"key\""))
        askNotifications();
}

// Whether the doses' alarms ring on time: Android's "Alarms & reminders"
// allowed (yours to take back on Android 12, given from 13). Without it they
// may come up to an hour late. Not known: false.
extern "C" bool sioul_android_exact_alarms()
{
    JNIEnv *env = jni();
    const jobject context = appContext.load();
    if (!env || !context)
        return false;
    const LocalFrame frame(env);
    const jmethodID exact = doseMethod(env, "exact", "(Landroid/content/Context;)Z");
    if (!exact)
        return false;
    const jboolean on = env->CallStaticBooleanMethod(doseAlarms, exact, context);
    return !threw(env) && on;
}

// Whether a reminder can show at all: Sioul's notifications on (from Android
// 13, your yes) and its "Doses" channel not turned off. Not known: false.
extern "C" bool sioul_android_notifications_allowed()
{
    JNIEnv *env = jni();
    const jobject context = appContext.load();
    if (!env || !context)
        return false;
    const LocalFrame frame(env);
    const jmethodID allowed = doseMethod(env, "notificationsAllowed", "(Landroid/content/Context;)Z");
    if (!allowed)
        return false;
    const jboolean on = env->CallStaticBooleanMethod(doseAlarms, allowed, context);
    return !threw(env) && on;
}

// A dose marked, here or on another device: its reminder, if one shows, goes,
// so that none says due what was taken. Any thread.
extern "C" void sioul_android_remove_reminder(const char *key)
{
    JNIEnv *env = jni();
    const jobject context = appContext.load();
    if (!env || !context || !key)
        return;
    const LocalFrame frame(env);
    if (const jmethodID dismiss = doseMethod(env, "dismiss", "(Landroid/content/Context;Ljava/lang/String;)V")) {
        env->CallStaticVoidMethod(doseAlarms, dismiss, context, javaText(env, key));
        threw(env);
    }
}

// The dose of a tapped reminder, once (kept by DoseOpener.java): copied into
// `key`, `size` bytes with the closing zero. False, and "", when there is none.
extern "C" bool sioul_android_take_opened(char *key, int size)
{
    if (!key || size <= 0)
        return false;
    key[0] = '\0';
    JNIEnv *env = jni();
    const jobject context = appContext.load();
    if (!env || !context)
        return false;
    const LocalFrame frame(env);
    const jmethodID take = doseMethod(env, "takeOpened", "(Landroid/content/Context;)Ljava/lang/String;");
    if (!take)
        return false;
    const auto text = static_cast<jstring>(env->CallStaticObjectMethod(doseAlarms, take, context));
    if (threw(env))
        return false;
    const QByteArray opened = utf8(env, text);
    if (opened.isEmpty())
        return false;
    if (opened.size() >= size) {
        __android_log_print(ANDROID_LOG_ERROR, "sioul", "A tapped dose's key is longer than %d bytes: %s", size - 1, opened.constData());
        return false;
    }
    memcpy(key, opened.constData(), size_t(opened.size()) + 1);
    return true;
}

// The time running, in Android's notifications while a focus session runs
// (crates/sioul-app/src/timenote.rs): JSON as TimeNote.java reads it, put up
// or changed; "" takes it away. Its first showing asks for notifications
// from Android 13, as the doses do. Any thread.
extern "C" void sioul_android_time_note(const char *json)
{
    JNIEnv *env = jni();
    const jobject context = appContext.load();
    if (!env || !context || !json)
        return;
    {
        const LocalFrame frame(env);
        const jmethodID show = timeMethod(env, "show", "(Landroid/content/Context;Ljava/lang/String;)V");
        if (!show)
            return;
        env->CallStaticVoidMethod(timeNote, show, context, javaText(env, json));
        threw(env);
    }
    if (*json)
        askNotifications();
}

// The alarm at waking's coming rings (crates/sioul-app/src/wake.rs): JSON
// {rings, looks, week, zone, words}, kept by WakeAlarms.java, which gives the
// next to Android's alarm clock. The first list that rings asks for
// notifications from Android 13, as the doses do: the ring shows its Stop in
// one. Any thread.
extern "C" void sioul_android_set_wake(const char *json)
{
    JNIEnv *env = jni();
    const jobject context = appContext.load();
    if (!env || !context || !json)
        return;
    {
        const LocalFrame frame(env);
        const jmethodID set = wakeMethod(env, "set", "(Landroid/content/Context;Ljava/lang/String;)V");
        if (!set)
            return;
        env->CallStaticVoidMethod(wakeAlarms, set, context, javaText(env, json));
        threw(env);
    }
    const char *rings = strstr(json, "\"rings\":[");
    if (rings && rings[9] >= '0' && rings[9] <= '9')
        askNotifications();
}

// What Android allows the alarm at waking: 1 exact alarms ("Alarms &
// reminders"), 2 the screen lit over the lock screen (refusable from Android
// 14), 4 notifications. Not known: 0.
extern "C" int sioul_android_wake_state()
{
    JNIEnv *env = jni();
    const jobject context = appContext.load();
    if (!env || !context)
        return 0;
    const LocalFrame frame(env);
    const jmethodID state = wakeMethod(env, "state", "(Landroid/content/Context;)I");
    if (!state)
        return 0;
    const jint bits = env->CallStaticIntMethod(wakeAlarms, state, context);
    return threw(env) ? 0 : int(bits);
}

// "Try the alarm" (crates/sioul-app/src/wake.rs): JSON {at (Unix ms), words},
// rung by WakeAlarms.java as a waking is, the next waking left as it is.
// Its answer: 0 it rings; 1 exact alarms refused, 2 notifications, 3 the full
// screen (nothing set); -1 not asked. Notifications refused: asked for (from
// Android 13), as for the doses. From the window.
extern "C" int sioul_android_wake_try(const char *json)
{
    JNIEnv *env = jni();
    const jobject context = appContext.load();
    if (!env || !context || !json)
        return -1;
    jint answer = -1;
    {
        const LocalFrame frame(env);
        const jmethodID tried = wakeMethod(env, "tryNow", "(Landroid/content/Context;Ljava/lang/String;)I");
        if (!tried)
            return -1;
        answer = env->CallStaticIntMethod(wakeAlarms, tried, context, javaText(env, json));
        if (threw(env))
            return -1;
    }
    if (answer == 2)
        askNotifications();
    return int(answer);
}

// Android's own page where the alarm at waking is allowed: "exact",
// "screen", "notifications". From the window.
extern "C" void sioul_android_wake_settings(const char *which)
{
    JNIEnv *env = jni();
    const jobject context = appContext.load();
    if (!env || !context || !which)
        return;
    const LocalFrame frame(env);
    if (const jmethodID settings = wakeMethod(env, "settings", "(Landroid/content/Context;Ljava/lang/String;)V")) {
        env->CallStaticVoidMethod(wakeAlarms, settings, context, javaText(env, which));
        threw(env);
    }
}

// The pauses' do-not-disturb (crates/sioul-app/src/dnd.rs, PauseMode.java):
// a verb ("can", "enter", "leave", "pressed", "open") and its JSON, Java's
// answer in JSON, given back to sioul_android_dnd_free; null when Java gave
// none. Any thread.
extern "C" char *sioul_android_dnd(const char *verb, const char *json)
{
    JNIEnv *env = jni();
    const jobject context = appContext.load();
    if (!env || !context || !verb || !json || !pauseMode)
        return nullptr;
    const LocalFrame frame(env);
    const jmethodID call = env->GetStaticMethodID(pauseMode, "call", "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;");
    if (threw(env) || !call)
        return nullptr;
    const auto answer = static_cast<jstring>(env->CallStaticObjectMethod(pauseMode, call, context, javaText(env, verb), javaText(env, json)));
    if (threw(env) || !answer)
        return nullptr;
    return strdup(utf8(env, answer).constData());
}

extern "C" void sioul_android_dnd_free(char *text)
{
    free(text);
}

// Your addresses that can send, offered in the share sheet and at a long
// press on Sioul's icon (crates/sioul-app/src/outside.rs, MailShortcuts.java):
// JSON {accounts: [{id, short, long}], gone}, the most used first; given when
// it changes, while the window is shown. Any thread.
extern "C" void sioul_android_mail_shortcuts(const char *json)
{
    JNIEnv *env = jni();
    const jobject context = appContext.load();
    if (!env || !context || !json || !mailShortcuts)
        return;
    const LocalFrame frame(env);
    const jmethodID set = env->GetStaticMethodID(mailShortcuts, "set", "(Landroid/content/Context;Ljava/lang/String;)V");
    if (threw(env) || !set)
        return;
    env->CallStaticVoidMethod(mailShortcuts, set, context, javaText(env, json));
    threw(env);
}

// ShareActivity.java's side: a request written in Sioul's drafts folder, or
// the files copied for one; Rust takes it now if the window runs, else as it
// comes up. From a thread of Java's, Qt's loader having loaded this library.
extern "C" JNIEXPORT void JNICALL Java_com_aurelienpierre_sioul_ShareActivity_nativeHanded(JNIEnv *, jclass)
{
    sioul_handed();
}

// Sioul away (in the back, the screen off): what the window draws with (its
// scene graph and graphics, about 90 MB on a phone) given back to Android,
// made again when Sioul comes back. Qt keeps it by default; here it lets go
// when the window is no longer shown and `releaseResources` is asked
// (main.qml, as Sioul goes away). Once the window is made, on its thread.
extern "C" void sioul_android_lean_window()
{
    for (QWindow *window : QGuiApplication::topLevelWindows()) {
        if (auto *quick = qobject_cast<QQuickWindow *>(window)) {
            quick->setPersistentGraphics(false);
            quick->setPersistentSceneGraph(false);
        }
    }
}

// DoseAlarms.java's side here: Sioul started without its window, then Rust's
// answers at a dose's alarm, on a thread of Java's (each may take half a
// minute: never Android's main thread).
extern "C" JNIEXPORT void JNICALL Java_com_aurelienpierre_sioul_DoseAlarms_nativeInit(JNIEnv *env, jclass, jobject context)
{
    initialize(env, context);
}

extern "C" JNIEXPORT jstring JNICALL Java_com_aurelienpierre_sioul_DoseAlarms_nativeDecide(JNIEnv *env, jclass, jstring key)
{
    return answered(env, sioul_alarm_decide(utf8(env, key).constData()));
}

extern "C" JNIEXPORT jstring JNICALL Java_com_aurelienpierre_sioul_DoseAlarms_nativeTaken(JNIEnv *env, jclass, jstring key)
{
    return answered(env, sioul_alarm_taken(utf8(env, key).constData()));
}

// TimeReceiver.java's side: a button of the time running, after DoseAlarms
// loaded and started Sioul's library; on a thread of Java's (it waits for the
// sharing: never Android's main thread).
extern "C" JNIEXPORT jstring JNICALL Java_com_aurelienpierre_sioul_TimeNote_nativeAction(JNIEnv *env, jclass, jstring action)
{
    return answered(env, sioul_time_action(utf8(env, action).constData()));
}

// WakeAlarms.java's side: the alarm at waking's coming rings, asked at a
// ring, before one, after a restart or a change of time or zone, once
// DoseAlarms loaded and started Sioul's library; Rust reads what your other
// devices wrote first, on a thread of Java's (never Android's main thread).
extern "C" JNIEXPORT jstring JNICALL Java_com_aurelienpierre_sioul_WakeAlarms_nativeNext(JNIEnv *env, jclass, jstring zone, jboolean fetch)
{
    return answered(env, sioul_wake_next(utf8(env, zone).constData(), fetch == JNI_TRUE));
}

// Reminders before events (crates/sioul-app/src/eventalarms.rs,
// EventAlarms.java): the coming ones, as JSON {reminders, look, words}, given
// to Android's alarm clock in place of those given before; the first list
// that holds one asks for notifications from Android 13, as the doses do.
// Any thread.
extern "C" void sioul_android_set_event_alarms(const char *json)
{
    JNIEnv *env = jni();
    const jobject context = appContext.load();
    if (!env || !context || !json || !eventAlarms)
        return;
    {
        const LocalFrame frame(env);
        const jmethodID set = env->GetStaticMethodID(eventAlarms, "set", "(Landroid/content/Context;Ljava/lang/String;)V");
        if (threw(env) || !set)
            return;
        env->CallStaticVoidMethod(eventAlarms, set, context, javaText(env, json));
        threw(env);
    }
    if (strstr(json, "\"key\""))
        askNotifications();
}

// An event's reminder shown, "Events" (EventAlarms.show): from the window's
// minute, or from Rust's answer at its alarm. Any thread.
extern "C" void sioul_android_event_note(const char *json)
{
    JNIEnv *env = jni();
    const jobject context = appContext.load();
    if (!env || !context || !json || !eventAlarms)
        return;
    const LocalFrame frame(env);
    const jmethodID show = env->GetStaticMethodID(eventAlarms, "show", "(Landroid/content/Context;Ljava/lang/String;)V");
    if (threw(env) || !show)
        return;
    env->CallStaticVoidMethod(eventAlarms, show, context, javaText(env, json));
    threw(env);
}

// New mail at the times it may come, "New mail" (crates/sioul-app/src/
// mailnote.rs, MailNotes.java): from the window's process, or from the one
// that fetches mail while Sioul is away. The first, shown while Sioul is on
// the screen, asks for notifications from Android 13, as the doses do. Any thread.
extern "C" void sioul_android_mail_note(const char *json)
{
    JNIEnv *env = jni();
    const jobject context = appContext.load();
    if (!env || !context || !json || !mailNotes)
        return;
    {
        const LocalFrame frame(env);
        const jmethodID show = env->GetStaticMethodID(mailNotes, "show", "(Landroid/content/Context;Ljava/lang/String;)V");
        if (threw(env) || !show)
            return;
        env->CallStaticVoidMethod(mailNotes, show, context, javaText(env, json));
        threw(env);
    }
    askNotifications();
}

// What a tapped reminder or mail notification opens, once (kept by
// ReminderOpener.java), JSON {kind, key}: copied into `buffer`, `size` bytes
// with the closing zero. False, and "", when there is none.
extern "C" bool sioul_android_take_reminder_opened(char *buffer, int size)
{
    if (!buffer || size <= 0)
        return false;
    buffer[0] = '\0';
    JNIEnv *env = jni();
    const jobject context = appContext.load();
    if (!env || !context || !eventAlarms)
        return false;
    const LocalFrame frame(env);
    const jmethodID take = env->GetStaticMethodID(eventAlarms, "takeOpened", "(Landroid/content/Context;)Ljava/lang/String;");
    if (threw(env) || !take)
        return false;
    const auto text = static_cast<jstring>(env->CallStaticObjectMethod(eventAlarms, take, context));
    if (threw(env))
        return false;
    const QByteArray opened = utf8(env, text);
    if (opened.isEmpty() || opened.size() >= size)
        return false;
    memcpy(buffer, opened.constData(), size_t(opened.size()) + 1);
    return true;
}

// EventAlarms.java's side: Rust's answer at a reminder's time, and the
// coming list asked again (a restart, a change of time or zone, once a day),
// once DoseAlarms loaded and started Sioul's library; on a thread of Java's
// (Rust reads the calendars: never Android's main thread).
extern "C" JNIEXPORT jstring JNICALL Java_com_aurelienpierre_sioul_EventAlarms_nativeDecide(JNIEnv *env, jclass, jstring key)
{
    return answered(env, sioul_event_decide(utf8(env, key).constData()));
}

extern "C" JNIEXPORT jstring JNICALL Java_com_aurelienpierre_sioul_EventAlarms_nativeComing(JNIEnv *env, jclass, jstring zone)
{
    return answered(env, sioul_event_coming(utf8(env, zone).constData()));
}

// Do-not-disturb on every device and the phone in the background
// (crates/sioul-app/src/steps.rs, everywhere.rs; StepService.java): Rust's
// questions to StepService.call (starting and stopping the service, the
// list's people starred or not on this phone, the battery's exemption, the
// alarm of do-not-disturb's next end), answered in JSON and given back to
// sioul_android_dnd_free; null when Java gave none. Any thread, window or not.
extern "C" char *sioul_android_steps(const char *verb, const char *json)
{
    JNIEnv *env = jni();
    const jobject context = appContext.load();
    if (!env || !context || !verb || !json || !stepService)
        return nullptr;
    const LocalFrame frame(env);
    const jmethodID call = env->GetStaticMethodID(stepService, "call", "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;");
    if (threw(env) || !call)
        return nullptr;
    const auto answer = static_cast<jstring>(env->CallStaticObjectMethod(stepService, call, context, javaText(env, verb), javaText(env, json)));
    if (threw(env) || !answer)
        return nullptr;
    return strdup(utf8(env, answer).constData());
}

// Reading the contacts, asked from Settings ▸ Do not disturb to say who on
// the list is starred on this phone (never written): Qt's question, which
// needs Qt's Java side, there only with the window.
extern "C" void sioul_android_ask_contacts()
{
    if (!QtAndroidPrivate::javaVM() || !qGuiApp)
        return;
    QtAndroidPrivate::requestPermission(QStringLiteral("android.permission.READ_CONTACTS"));
}

// The background service's side, in its own process, once DoseAlarms loaded
// Sioul's library there: this process said to Rust, and a step on the
// service's thread (the phone kept awake meanwhile).
extern "C" JNIEXPORT void JNICALL Java_com_aurelienpierre_sioul_StepService_nativeService(JNIEnv *, jclass)
{
    sioul_steps_in_service();
}

extern "C" JNIEXPORT jstring JNICALL Java_com_aurelienpierre_sioul_StepService_nativeStep(JNIEnv *env, jclass, jstring reason)
{
    return answered(env, sioul_steps_step(utf8(env, reason).constData()));
}

// Do-not-disturb applied in Sioul's own process (DndReceiver.java), window or
// not, on a thread of Java's.
extern "C" JNIEXPORT jstring JNICALL Java_com_aurelienpierre_sioul_DndReceiver_nativeApply(JNIEnv *env, jclass)
{
    return answered(env, sioul_dnd_apply());
}

// Other apps' notifications (crates/sioul-app/src/appnotes.rs, AppNotes.java):
// Rust's questions to AppNotes.call (the access and Android's pages for it, an
// app's or a channel's notification settings, the contacts' permission),
// answered in JSON and given back to sioul_android_dnd_free; null when Java
// gave none. From the window's process, any thread.
extern "C" char *sioul_android_appnotes(const char *verb, const char *json)
{
    JNIEnv *env = jni();
    const jobject context = appContext.load();
    if (!env || !context || !verb || !json || !appNotes)
        return nullptr;
    const LocalFrame frame(env);
    const jmethodID call = env->GetStaticMethodID(appNotes, "call", "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;");
    if (threw(env) || !call)
        return nullptr;
    const auto answer = static_cast<jstring>(env->CallStaticObjectMethod(appNotes, call, context, javaText(env, verb), javaText(env, json)));
    if (threw(env) || !answer)
        return nullptr;
    return strdup(utf8(env, answer).constData());
}

// AppNotes.java's side, in the listener's own process once DoseAlarms loaded
// Sioul's library there: a notification decided, and what is held worked out
// again, on the listener's thread (never Android's main thread). What a
// notification says passes here in memory, to Rust, and nowhere else.
extern "C" JNIEXPORT jstring JNICALL Java_com_aurelienpierre_sioul_AppNotes_nativeDecide(JNIEnv *env, jclass, jstring json)
{
    return answered(env, sioul_appnotes_decide(utf8(env, json).constData()));
}

extern "C" JNIEXPORT jstring JNICALL Java_com_aurelienpierre_sioul_AppNotes_nativeReview(JNIEnv *env, jclass, jstring json)
{
    return answered(env, sioul_appnotes_review(utf8(env, json).constData()));
}

int main(int, char *[])
{
    const QJniObject context = applicationContext();
    initialize(jni(), context.isValid() ? context.object() : nullptr);
    return sioul_app_run();
}
