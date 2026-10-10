# Android, as Sioul uses it

This page is for developers who have not written an Android app. It explains the parts of Android that Sioul's code relies on, with excerpts from the code: what the app is made of, which parts Android starts and why, how Java and Rust call each other, where the files are, and how the APK is built and signed. [Android](android.md) then says, feature by feature, what Sioul does on a phone and what was tried there; it uses the words explained here. Android's own documentation goes further: [application fundamentals](https://developer.android.com/guide/components/fundamentals), and the pages named in each section below. [Start here](start-here.md) has the map of the whole code.

## The shape of it
- **One window, the computer's.** The phone runs the same Rust and the same QML as a computer: Qt for Android builds Sioul's window into an Android app. Two pages differ (`crates/sioul-app/qml/android/`), because Qt WebEngine and Qt PDF do not exist on Android, and the tray is left out ([Qt Quick and QML](qt-quick.md#a-computer-and-a-phone)).
- **Java for what runs without the window.** Android freezes an app it does not show, and stops it when it needs the memory. What must happen then (a dose's reminder, the alarm at waking, a call to screen, another app's notification to hold, the phone kept in step with your other devices) is started by Android itself, from what the manifest declares, and written in Java: `android/package/src/com/aurelienpierre/sioul/`, 34 files.
- **Rust decides; Java does what only Android can do.** The Java classes ask Rust what to do (what a reminder says, when to look again) and do what needs Android's own interfaces (alarms, notifications, do-not-disturb modes). They talk through C++, in `android/main.cpp` ([Java and Rust: JNI](#java-and-rust-jni)).

## The app: an APK
An Android app is installed from an *APK* (Android package), a ZIP file that holds:
- **the manifest**, `AndroidManifest.xml`: the app's name, its *package name* (`com.aurelienpierre.sioul`, its identity on the phone), what it may do (its *permissions*), and the parts Android may start ([The manifest](#the-manifest));
- **the Java classes**, Sioul's and Qt's, compiled for Android's runtime;
- **native libraries**, one folder per processor type (*ABI*): `lib/arm64-v8a/` holds Qt's libraries and Sioul's, `libsioul_arm64-v8a.so`, which holds all of Sioul's Rust and the window's QML (built in as Qt resources);
- **resources**: the icon, the home-screen card's layout and colours, a few words (`android/package/res/`);
- **its signature** ([Signing](#signing)).

Android versions are numbered by *API level*: 28 is Android 9, 33 is Android 13, 36 is Android 16. Sioul runs on 28 and later (`QT_ANDROID_MIN_SDK_VERSION 28` in `android/CMakeLists.txt`), and is built for 36 (`QT_ANDROID_TARGET_SDK_VERSION 36`: the version whose rules the app follows). The Java code tests the version wherever Android changed: `Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q` (Android 10) before asking for a role that exists only from then. Android's documentation: [ABIs](https://developer.android.com/ndk/guides/abis).

## Qt for Android: the window's activity
An *activity* is one screen of an app: a Java class Android makes and shows. Sioul's window is Qt's activity, `org.qtproject.qt.android.bindings.QtActivity`, named in the manifest with the launcher's *intent filter*, which makes Android show it among the apps and start it at a tap on the icon:

```xml
<activity
    android:name="org.qtproject.qt.android.bindings.QtActivity"
    …
    android:launchMode="singleTop"
    android:exported="true">
    <intent-filter>
        <action android:name="android.intent.action.MAIN" />
        <category android:name="android.intent.category.LAUNCHER" />
    </intent-filter>

    <meta-data
        android:name="android.app.lib_name"
        android:value="-- %%INSERT_APP_LIB_NAME%% --" />
```

What happens at a tap on Sioul's icon:
1. **Android starts the app's process** and makes the activity. Qt's Java side, built into the APK, loads Qt's libraries, then the library the manifest names (`android.app.lib_name`, filled in at build time), `libsioul_arm64-v8a.so`.
2. **Qt calls the library's `main()`**, on a thread of Qt's own, not on Android's main thread. That `main()` is in `android/main.cpp`: it sets up what a computer gives a program and Android does not (`initialize`: the folders, the language, the certificates, the log, Java lent to Rust), then calls Rust's `sioul_app_run()`.
3. **Rust runs the window**: `sioul_app_run` calls `run()` (`crates/sioul-app/src/lib.rs`), the function the computer's program calls from `main.rs`. The QML engine loads `main.qml`, and Qt draws it in the activity.

Two facts shape the rest of this page:
- **Put away, the window is frozen**, and its process may be killed. `main.qml` hears Qt's application state change (`Qt.application.state`) and calls `sioul.goingAway()`; from then on no page is computed while nobody can see it, until `sioul.backHere()` ([The bridge](cxx-qt.md#which-thread-does-what), `backend::AWAY`). The window's minute does not turn while it is frozen.
- **Closing the activity ends its process.** Qt's activity calls `System.exit(0)` as it is destroyed (Back on the last page, a swipe from the recent apps): anything that must outlive the window lives in a process of its own ([Processes](#processes)).

Qt provides the rest of the Android project too: Qt's Java classes, and a template of the project that Gradle, Android's build tool, turns into the APK. Sioul adds its manifest, its resources and its Java to that template ([Building the APK](#building-the-apk)).

Qt's documentation: [Qt for Android](https://doc.qt.io/qt-6/android.html), [getting started](https://doc.qt.io/qt-6/android-getting-started.html).

## The manifest
`android/package/AndroidManifest.xml` is Qt 6.11's template with Sioul's own parts added. It declares *components*: the parts of an app that Android starts by itself, each a Java class that Android makes when needed, starting the app's process first if it is not running. There are four kinds:
- **Activities**, screens. Besides the window: `ShareActivity` (what other apps share to Sioul, and `mailto:` links), `WakeScreen` (the alarm at waking, over the lock screen), and five that are never seen: `DoseOpener`, `ReminderOpener`, `PauseOpener` and `HomeCardOpener` keep what was tapped (a reminder, the tile, a line of the home-screen card) for Rust, then bring the window up; `CallsSetup` asks Android for the call-screening role.
- **Services**, work without a screen. `StepService` keeps the phone in step with your other devices while Sioul is closed; `WakeRinger` plays the alarm at waking; `PauseTile` is "Pause" among the quick settings; `CallScreen` answers Android's question about each call; `AppNotes` hears other apps' notifications; `HomeCardService` gives the launcher the rows of the home-screen cards' lists (a `RemoteViewsService`, bound by the system alone).
- **Broadcast receivers**: Android calls a receiver's `onReceive` when a *broadcast* it listens for is sent, by Android or by an app. An alarm Sioul set rings (`DoseReceiver`, `WakeReceiver`, `EventReceiver`), a notification's button is pressed (`TimeReceiver`), the phone has started or Sioul was updated (`DoseBootReceiver`, `StepReceiver`, `DndReceiver`, and others). `HomeCard`, the card on the home screen, is a receiver too: Android's widgets are drawn by one (and so are its mail and agenda cards, `HomeCardMail` and `HomeCardAgenda`).
- **Content providers**: `StartChoices`, made as the process starts, before Qt (it keeps Qt's accessibility for screen readers only), and Qt's `FileProvider`, which hands files to other apps ("Open with…").

The other classes (`DoseAlarms`, `WakeAlarms`, `EventAlarms`, `TimeNote`, `MailNotes`, `MailShortcuts`, `PauseMode`, `DndContacts`, `Calls`, `HomeCardRows`) are not components: they hold the code that the components and Rust share. Each class says what it is for in its first comment; [the API reference](https://aurelienpierre.github.io/sioul/dev/api.html) shows them all.

What matters in each declaration:
- **`android:exported`**: whether other apps may start the component. Sioul's are `false`, except the window, `ShareActivity` (other apps share to it), and the services the system itself binds (`PauseTile`, `CallScreen`, `AppNotes`); a permission only the system holds keeps those for it (`android:permission="android.permission.BIND_SCREENING_SERVICE"` on `CallScreen`).
- **`<intent-filter>`**: what reaches the component. An *intent* is Android's message: an action, and data or a class. `android.intent.action.BOOT_COMPLETED` comes when the phone has started, `android.intent.action.MY_PACKAGE_REPLACED` when Sioul was updated, `android.intent.action.SEND` when another app shares something.
- **`android:process`**: the process the component runs in ([Processes](#processes)).
- **`android:directBootAware`**: the component may run after a restart, before the phone is first unlocked, when the app's usual files cannot be read yet (the alarm at waking).
- **`android:foregroundServiceType`**: what a foreground service is for ([The foreground service](#the-foreground-service-and-its-notification)).
- **`<queries>`**: the other apps Sioul talks to (Murena's eDrive, asked to sync now). From Android 11, an app sees only the apps it names there.

*Permissions* say what the app may do. Android has several kinds, and Sioul uses each:
- **Granted at install**, by the line alone: the network (`INTERNET`, `ACCESS_NETWORK_STATE`), starting after a restart (`RECEIVE_BOOT_COMPLETED`), keeping the phone awake (`WAKE_LOCK`), foreground services (`FOREGROUND_SERVICE` and one per type), exact alarms from Android 13 (`USE_EXACT_ALARM`).
- **Asked at the time**, with Android's question: notifications (`POST_NOTIFICATIONS`, from Android 13) and the contacts (`READ_CONTACTS`). `android/main.cpp` asks through Qt (`QtAndroidPrivate::requestPermission`), which needs the window: never from the background.
- **Special access**, turned on by the person in Android's settings: "Alarms & reminders" on Android 12 (`SCHEDULE_EXACT_ALARM`), "All files access" (`MANAGE_EXTERNAL_STORAGE`, for the sharing folder a sync app carries), "Do Not Disturb access" (`ACCESS_NOTIFICATION_POLICY`), "Notification access" (for `AppNotes`), the full screen over the lock screen (`USE_FULL_SCREEN_INTENT`, which Android 14 may refuse), and leaving battery optimisation. The manifest's line alone gives nothing: Sioul's settings say whether each is given, with a button to Android's page.
- **A role**: "Caller ID & spam app" (`RoleManager.ROLE_CALL_SCREENING`, Android 10 and later), which one app holds at a time; `CallsSetup` asks for it.

Android's documentation: [the app manifest](https://developer.android.com/guide/topics/manifest/manifest-intro), [permissions](https://developer.android.com/guide/topics/permissions/overview).

## Processes
A *process* is a running program, with memory of its own. Android runs all of an app's components in one process, named after its package, unless the manifest gives a component another one: `android:process=":steps"` runs it in a process of its own, `com.aurelienpierre.sioul:steps`. Sioul has four:

| Process | What runs there | Why apart |
|---|---|---|
| `com.aurelienpierre.sioul`, the window's | the window, the other activities, the receivers of alarms and buttons, `WakeRinger`, `PauseTile` | The usual one. A receiver started while the window is closed loads Sioul's library there without Qt's window ([Running without the window](#running-without-the-window)). |
| `com.aurelienpierre.sioul:steps` | `StepService` and its `StepReceiver` | The window's process ends when the window closes; the service must not. |
| `com.aurelienpierre.sioul:calls` | `CallScreen` | Android holds a call's ringing until the app answers, five seconds at most. This process never loads Sioul's library nor Qt: it reads a table Rust wrote ahead (`files/state/sioul/calls/table.json`) and answers in milliseconds. |
| `com.aurelienpierre.sioul:listener` | `AppNotes`, the notification listener | Not the window's: closing the window would unbind the listener. Not `:steps`, which runs only while its setting is on, and does long work a notification must not wait for. |

What follows from it:
- **Nothing in memory is shared.** Each process loads its own copy of Sioul's library: its statics, its caches and its `OnceLock`s are its own. The processes meet through files, locked when two may write the same one (`sioul_core::filelock::with_lock`, [rust.md](rust.md#threads-and-async)), and through broadcasts: the background service asks the window's process to apply do-not-disturb with a broadcast to `DndReceiver`, since Sioul's modes are kept there (`PauseMode`).
- **Rust knows where it runs** when it matters: `StepService` tells it (`nativeService`, then `steps::in_service()` is true), so that the service's process never touches Android's modes.
- **Each process costs memory**: Sioul's library and Qt's, without a window, are estimated at 30 to 50 MB ([android.md](android.md#in-the-background)).

Android's documentation: [processes and app lifecycle](https://developer.android.com/guide/components/activities/process-lifecycle).

## Running without the window
Since Android freezes an app it does not show, Sioul gives Android ahead of time what must happen later, and Android starts Sioul's Java when the time comes:
- **Alarms**: each coming dose, the coming wakings, the reminders before events, the background service's next step and do-not-disturb's next end are given to Android's alarm service (`AlarmManager`), each with the receiver it must reach ([Alarms](#alarms)).
- **Notifications** posted ahead stay while Sioul is frozen, and their buttons reach a receiver: the time running needs nothing running ([android.md](android.md#the-time-running)).
- **The home-screen card** is written in frames, one per change of time until the end of the next day, which Java shows in turn ([android.md](android.md#the-card-on-the-home-screen)).

When a receiver is called, `DoseReceiver` for instance:
1. **Android starts the window's process** if it is not running, without the window, makes the receiver and calls `onReceive` on Android's main thread, which must never wait.
2. **The receiver keeps the broadcast open** with `goAsync()` and starts a thread: Rust may take seconds (twenty, while it gives the sync app time to bring your other devices' news). Android gives a broadcast ten seconds when it comes from the foreground (a notification's button) and a minute from the background (an alarm); each receiver lets go before its deadline and says something sensible meanwhile (`DoseReceiver`: after 45 seconds, a reminder to check in Sioul).
3. **On that thread, Sioul's library is loaded alone**, once per process: `DoseAlarms.load(context)` calls `System.loadLibrary` with the library's name and the processor's, then `nativeInit`, which runs `initialize` in `main.cpp`, what `main()` does before the window. Every receiver and service that needs Rust calls the same `DoseAlarms.load`. The library has its own `JNI_OnLoad`, which Android calls as Java loads it: it keeps the Java VM and finds Sioul's Java classes, and starts nothing of Qt's.
4. **The Java method declared `native` calls Rust** ([Java and Rust: JNI](#java-and-rust-jni)), and the receiver shows Rust's answer.

**Why Sioul's library has its own `JNI_OnLoad`**, from Qt 6.11's sources: Android looks `JNI_OnLoad` up from the library Java loads, through that library's dependencies too (the dynamic linker searches them breadth first), so a library without one of its own would run the first of Qt's, Qt Multimedia's, which needs Qt6Core's Java side ready. And Qt6Core's own `JNI_OnLoad` reads Qt's class loader once per process: run in a receiver's process before Qt's loader has set it, it would leave the window, opened later in that process, without a class loader. So a receiver's process runs nothing of Qt's Java side, and Qt's loader starts it, the first time, when the window opens. A second trap of the same family: `QtActivityBase.onNewIntent` does not call `setIntent`, so a new intent reaches only Qt's own listeners (`QtAndroidPrivate::registerNewIntentListener`). A tapped reminder therefore opens Sioul through an invisible activity of its own (`DoseOpener`, `ReminderOpener`…), which keeps what was tapped for Rust (`DoseAlarms.opened`) and brings the window up by its launcher intent: no dose rides in the window's intent, where a window Android restores would find it again.

From `DoseReceiver.java`:

```java
final Asked asked = new Asked(goAsync());
…
new Thread(() -> {
    String answer = null;
    try {
        DoseAlarms.load(app);
        answer = alarm ? DoseAlarms.nativeDecide(key) : DoseAlarms.nativeTaken(key);
    } catch (Throwable e) {
        // Sioul's library that does not load, among others: said all the same.
        Log.e(DoseAlarms.TAG, "Doses: no answer about " + key, e);
    }
    main.removeCallbacks(deadline);
    asked.answer(app, intent, key, alarm, answer);
}, "sioul-dose").start();
```

If the window opens later in the same process, Qt starts there as usual. Android's documentation: [broadcasts](https://developer.android.com/develop/background-work/background-tasks/broadcasts).

## Alarms
An alarm is a moment given to `AlarmManager` with a *PendingIntent*: an intent handed to Android, which sends it at that moment on the app's behalf, here a broadcast to one of Sioul's receivers. From `DoseAlarms.java`:

```java
AlarmManager alarms = context.getSystemService(AlarmManager.class);
PendingIntent ring = PendingIntent.getBroadcast(context, key.hashCode(), alarmIntent(context, key).putExtra(NAME, name),
                                                PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
try {
    if (exact(context)) {
        alarms.setExactAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, at, ring);
        return;
    }
} catch (SecurityException e) {
    // Taken back between the question and the alarm.
}
alarms.setAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, at, ring);
```

- **Exact or not**: an exact alarm rings at its minute, with the permission for it; an inexact one may come up to an hour late. *Doze*, a phone's deep sleep, holds ordinary alarms until its next window of maintenance; the `…AllowWhileIdle` kinds ring through it. The alarm at waking uses `setAlarmClock`, which Android shows in the status bar and on the lock screen.
- **Each alarm is told apart** by its request code (`key.hashCode()`) and its intent's data, so that it can be replaced or cancelled.
- **Android forgets alarms when the phone restarts.** Each list is kept by Java, and given again by a receiver of `BOOT_COMPLETED` (or `LOCKED_BOOT_COMPLETED`, before the first unlock, for the waking), of `MY_PACKAGE_REPLACED` after an update, and of `SCHEDULE_EXACT_ALARM_PERMISSION_STATE_CHANGED` when exact alarms are allowed again.

| Alarms | Given by Rust in | Kept and set by | Received by |
|---|---|---|---|
| Doses of the next two days | `crates/sioul-app/src/alarms.rs` (`sioul_android_set_alarms`) | `DoseAlarms` | `DoseReceiver` |
| The alarm at waking | `wake.rs` (`sioul_android_set_wake`) | `WakeAlarms` | `WakeReceiver` |
| Reminders before events | `eventalarms.rs` (`sioul_android_set_event_alarms`) | `EventAlarms` | `EventReceiver` |
| The background service's next step | `steps.rs`, in each step's answer | `StepService` | `StepReceiver` |
| Do-not-disturb's next end | `steps.rs` (`next`) | `DndReceiver` | `DndReceiver` |
| The home-screen card's next frame | `homecard.rs`, in the frames | `HomeCard` | `HomeCard` |

Android's documentation: [alarms](https://developer.android.com/develop/background-work/services/alarms), [Doze](https://developer.android.com/training/monitoring-device-state/doze-standby).

## The foreground service and its notification
A *service* runs without a screen; from Android 8, the services of an app in the background are stopped after a short while. A *foreground service* is one the person can see: it shows a notification for as long as it runs, and Android lets it run while the app is away. From Android 14, its *type* (what it is for) is declared in the manifest, each type with its permission and its rules.
- **`StepService`**, in `:steps`: "Sioul keeps your devices in step". Its type is `specialUse`, with its reason written in the manifest, since no other type fits (`dataSync` would be stopped after six hours a day from Android 15):

    ```xml
    <service
        android:name="com.aurelienpierre.sioul.StepService"
        android:process=":steps"
        android:exported="false"
        android:foregroundServiceType="specialUse">
        <property
            android:name="android.app.PROPERTY_SPECIAL_USE_FGS_SUBTYPE"
            android:value="Keeps the person's do-not-disturb and the people allowed through it in step with their other devices, through the folder their own sync app carries, while the app is closed." />
    </service>
    ```

    It is started with `startForegroundService`, and calls `startForeground` with its notification, as Android requires within seconds. `onStartCommand` returns `START_STICKY`: Android starts it again if it had to stop it. Its notification channel, "Devices in step", is at the lowest importance (`IMPORTANCE_MIN`): no sound, no icon in the status bar. Rust's side is `crates/sioul-app/src/steps.rs`.

- **`WakeRinger`**, in the window's process, in the foreground only while the alarm at waking rings: type `systemExempted`, which Android 14 allows to apps that may set exact alarms.

*Notification channels* (Android 8 and later): each notification belongs to a channel the app makes ("Doses", "Waking", "Events", "New mail", "Focus timer", "Devices in step"…), and the person sets each channel's sound and importance in Android's settings. Sioul's channels are named in the person's language, by Rust ([fluent.md](fluent.md#what-fluent-does-not-do-here) for the few words Java keeps itself).

Android's documentation: [foreground services](https://developer.android.com/develop/background-work/services/fgs), [notification channels](https://developer.android.com/develop/ui/compose/notifications/channels).

## Java and Rust: JNI
*JNI* (the Java Native Interface) is how Java calls functions of a native library, written in C, C++ or Rust, and how such a library calls Java. Sioul's Java and Rust never call each other directly: both talk to `android/main.cpp`, which speaks JNI on one side and plain C functions on the other.

### Java calls Rust
1. **Java declares the method `native`**, without a body (`DoseAlarms.java`):

    ```java
    static native void nativeInit(Context context);
    static native String nativeDecide(String key);
    static native String nativeTaken(String key);
    ```

2. **`main.cpp` defines it**, under the name JNI looks for: `Java_`, the package with `_` for `.`, the class, the method. It turns Java's text into UTF-8, calls Rust, and turns Rust's answer into a Java string:

    ```cpp
    extern "C" JNIEXPORT jstring JNICALL Java_com_aurelienpierre_sioul_DoseAlarms_nativeDecide(JNIEnv *env, jclass, jstring key)
    {
        return answered(env, sioul_alarm_decide(utf8(env, key).constData()));
    }
    ```

3. **Rust exports a C function**: `#[unsafe(no_mangle)]` keeps its name as written, `extern "C"` gives it C's way of calling ([rust.md](rust.md#calling-c-and-being-called-from-it)). From `crates/sioul-app/src/alarms.rs`:

    ```rust
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn sioul_alarm_decide(key: *const c_char) -> *mut c_char {
        let key = unsafe { key_of(key) };
        let answer = std::panic::catch_unwind(|| crate::health::alarm_decide(&key)).unwrap_or_else(|_| {
            …
        });
        …
        handed(answer)
    }
    ```

    `main.cpp` declares it (`extern "C" char *sioul_alarm_decide(const char *key);`), copies the answer, then gives it back to `sioul_string_free`: what Rust allocated, Rust frees.

### Rust calls Java
1. **Rust declares the C function**, for Android only (`crates/sioul-app/src/alarms.rs`):

    ```rust
    #[cfg(target_os = "android")]
    unsafe extern "C" {
        /// Android's alarm clock given the coming doses, replacing those given before (android/main.cpp).
        fn sioul_android_set_alarms(json: *const c_char);
        …
    }
    ```

    The Rust that calls it is under the same `cfg`; on a computer the function that would call it does nothing, or answers what a computer would.

2. **`main.cpp` defines it**: it takes the thread's JNI environment (`jni()`), looks up the static Java method by its name and its types, and calls it:

    ```cpp
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
        …
    }
    ```

    `(Landroid/content/Context;Ljava/lang/String;)V` is how JNI writes the method's types: a `Context` and a `String`, returning nothing (`V`, void).

3. **Java receives it** as an ordinary static method: `static synchronized void set(Context context, String json)` in `DoseAlarms.java`.

### What the glue keeps to
- **JSON text, both ways.** Anything with a shape crosses as one JSON string: Java reads it with Android's `org.json`, Rust with serde_json. A new field needs no change in the C++.
- **One owner per allocation.** Text Rust hands over is freed by Rust (`sioul_string_free`); text `main.cpp` copies for Rust with `strdup` is freed by C++ (`sioul_android_dnd_free`).
- **Nothing crashes across.** Rust's entry points run their work inside `std::panic::catch_unwind` and answer something sensible when it panics ([rust.md](rust.md#the-error-types)); `main.cpp` checks for a Java exception after each call (`threw`), logs it and clears it.
- **Threads.** A JNI environment belongs to one thread. A thread Rust started is attached to Java the first time it calls (`jni()`), and detached when it ends. Sioul's Java classes are looked up once, in `JNI_OnLoad`, as Java loads the library (`appClass`): a thread Rust started could not find them by name, since it lacks the app's class loader.
- **Text**: what JNI calls UTF-8 is not quite UTF-8, so `main.cpp` converts through UTF-16 (`javaText`, `utf8`). Local references made on a thread of Rust's are freed with a frame (`LocalFrame`).
- **Two ways to Java.** What only the window asks (Android's chooser of accounts, the permissions' questions, the "All files access" page) goes through Qt's helpers (`QJniObject`, `QtAndroidPrivate`), which work only once Qt's Java side runs. Everything a receiver or a service reaches uses plain JNI, since there Qt's Java side never started.
- **One door, many questions.** Some classes take a verb and a JSON text, and answer in JSON: `StepService.call`, `PauseMode.call`, `AppNotes.call` (`sioul_android_steps`, `sioul_android_dnd`, `sioul_android_appnotes` in `main.cpp`; `steps::java` in Rust). A new question to them is a new verb, not a new function.

### Adding a call
1. **Java**: a `static native` method in the class Android starts, or a `static` method Rust will call, taking a `Context` and a JSON `String`.
2. **`main.cpp`**: the `Java_com_aurelienpierre_sioul_<Class>_<method>` function, or a `sioul_android_<what>` function, written as its neighbours are (`jni()`, `LocalFrame`, `threw`, `javaText`, `utf8`). A class Rust calls is found in `JNI_OnLoad`.
3. **Rust**: a `#[unsafe(no_mangle)] pub unsafe extern "C" fn sioul_<what>` whose comment has a `# Safety` section and whose work runs in `catch_unwind`; or the declaration, in an `unsafe extern "C"` block under `#[cfg(target_os = "android")]`, and what a computer does instead.
4. **Build the APK**: the Java and `main.cpp` are compiled only there, in CI ([Building the APK](#building-the-apk)).

Android's documentation: [JNI tips](https://developer.android.com/ndk/guides/jni-tips).

## Where files live on the phone
An app's *private storage* is a folder only that app can read, removed with it: its `files` folder (`Context.getFilesDir()`) and its cache. `main.cpp` (`setFolders`) points the XDG variables there before Rust starts, so that the core finds its folders as on Linux (`sioul_core::config::config_dir`, `data_dir`, `state_dir`, `cache_dir`):

| What | Variable | On the phone |
|---|---|---|
| Home | `HOME` | `files/` |
| Configuration | `XDG_CONFIG_HOME` | `files/config/`, so Sioul's is `files/config/sioul/config.toml` |
| Mail, drafts, contacts, calendars, what other apps shared | `XDG_DATA_HOME` | `files/data/`, so `files/data/sioul/…` |
| State: where sync stopped, the home-screen card, the calls' table | `XDG_STATE_HOME` | `files/state/`: `files/state/sioul/home-card.json`, `files/state/sioul/calls/table.json` |
| Cache | `XDG_CACHE_HOME` | the app's cache folder, which Android may empty |

- **Java reads the same files**, by the same paths under `getFilesDir()`: `HomeCard` reads `state/sioul/home-card.json`, `Calls` reads `state/sioul/calls/`, `ShareActivity` writes into `data/sioul/handed/`.
- **Java's own memory** (the alarms given, to give them again after a restart; the shortcuts last given) is in Android's `SharedPreferences`, small key-value files of the app. The alarm at waking keeps its list in *device-protected storage* (`createDeviceProtectedStorageContext`), which can be read before the phone's first unlock; the other files cannot.
- **The notes folder** is `~/Notes`, so `files/Notes`, unless another is chosen (`case_store` in the configuration).
- **The sharing folder** is wherever your sync app keeps it, in the phone's shared storage, read by its path with "All files access"; or a folder Sioul keeps in step itself, in its own storage ([android.md](android.md#what-differs-on-android)).
- **Passwords** go to Android's KeyStore: a key the KeyStore keeps, and never lets out, encrypts each password into the app's private storage (the `android-keyring` crate, set up by `sioul_sync::android::init` with what `main.cpp` lends: the Java VM and the application's `Context`).
- **No cloud backup**: `android:allowBackup="false"`, and `res/xml/sioul_data_extraction_rules.xml` for moving to a new phone ([android.md](android.md#how-it-is-made)).
- **The language and the certificates**: `main.cpp` sets `LANG` from the phone's language, and `SSL_CERT_DIR` to Android's certificates.

Android's documentation: [data and file storage](https://developer.android.com/training/data-storage).

## Building the APK
The APK is built by `.github/workflows/android.yml`; [android.md](android.md#building-it) gives the same steps by hand. It needs:
- **Qt for Android 6.11** (`android_arm64_v8a`) with the modules the pages import (Qt Multimedia, Qt Positioning, Qt Location, Qt Image Formats, and Qt SerialPort, which Qt Positioning's NMEA plugin wants), and the desktop Qt of the same version, whose tools run on the computer that builds;
- **the Android SDK** with the NDK Qt 6.11 is built with (r27c, `27.2.12479018`), the platform `android-36` and its build tools; the *NDK* is Android's kit of C and C++ compilers;
- **Java**, for Gradle (17 or newer; the workflow uses 21);
- **Rust** with the phone's target: `rustup target add aarch64-linux-android`.

The two commands:

```
<Qt for Android>/bin/qt-cmake -S android -B build-android -G Ninja \
    -DCMAKE_BUILD_TYPE=Release -DQT_HOST_PATH=<desktop Qt> \
    -DANDROID_SDK_ROOT=<SDK> -DANDROID_NDK_ROOT=<NDK> \
    -DANDROID_SUPPORT_FLEXIBLE_PAGE_SIZES=ON
cmake --build build-android --target apk
```

What `android/CMakeLists.txt` makes of them:
1. **`qt-cmake`** is CMake with Qt for Android's settings: the NDK's compilers, the processor. The file reads Sioul's version from the root `Cargo.toml`.
2. **Corrosion** (fetched by CMake, v0.6.1) builds the `sioul-app` crate with Cargo for `aarch64-linux-android`, as a static library (`OVERRIDE_CRATE_TYPE sioul_app=staticlib`): on a computer the crate makes the program, here a library linked into Qt's.
3. **CXX-Qt's build script** runs inside that Cargo build, as on a computer ([cxx-qt.md](cxx-qt.md#buildrs-what-the-module-holds)). CXX-Qt's own CMake function (`cxx_qt_import_crate`) names the crate's export folder after its library (`sioul_app`) and its build script after its package (`sioul-app`), which differ here; so the file sets the build script's variables itself, and links the two object files it leaves, which register the QML module.
4. **`qt_add_executable(sioul main.cpp …)`** links `main.cpp`, the Rust library and Qt into `libsioul_arm64-v8a.so`: on Android, Qt's "executable" is a library its Java side loads. It is linked for phones with 16 KB memory pages (Android 15 and later), and exports little, which makes it start faster ([android.md](android.md#starting)).
5. **`androiddeployqt`**, run by the `apk` target, copies Qt's Android project template into the build folder and lays `android/package/` over it (`QT_ANDROID_PACKAGE_SOURCE_DIR`: the manifest, `res/`, `src/`); fills the manifest's `-- %%INSERT_…%% --` parts (the version, the library's name) and `<!-- %%INSERT_PERMISSIONS -->` (what Qt's own modules need); gathers Qt's libraries, plugins and the QML modules the pages import (it reads them from `crates/sioul-app/qml`, `QT_QML_ROOT_PATH`); then runs **Gradle**, Android's build tool, which compiles the Java classes and packs the APK.
6. **The APK comes out unsigned**, under `build-android/…/outputs/apk/release/`.

The version: Android's `versionName` is `Cargo.toml`'s version (`0.0.5`); its `versionCode`, the number Android compares to allow an update, is worked out from it: major × 10,000 + minor × 100 + patch.

**In CI**, `.github/workflows/android.yml` runs on each push to `main` that changes `android/`, `crates/`, `Cargo.toml` or `Cargo.lock`, and by hand (Actions, "Android", "Run workflow"). It installs Rust with the phone's target, Java, the NDK, the platform and the build tools into the runner's SDK, and Qt for Android with its desktop Qt (`jurplel/install-qt-action`); configures and builds as above; signs (below); checks the alignment of every library of the APK for 16 KB memory pages (Sioul's own must pass; the run's summary lists all of them); and keeps the APK as the run's artifact, `sioul-android-arm64`.

Qt's documentation: [deploying an application on Android](https://doc.qt.io/qt-6/deployment-android.html), [androiddeployqt](https://doc.qt.io/qt-6/android-deploy-qt-tool.html), [QT_ANDROID_PACKAGE_SOURCE_DIR](https://doc.qt.io/qt-6/cmake-target-property-qt-android-package-source-dir.html). Corrosion: [its book](https://corrosion-rs.github.io/corrosion/).

## Signing
Android installs only a signed APK. It installs a new version over the old one, the app's data kept, only when both are signed with the same key and the new version code is not lower. So every version of Sioul is signed with one key, kept for good.
- **The key** is a keystore (a password-protected file holding the key) kept in the repository's secrets on GitHub: `ANDROID_KEYSTORE`, the keystore in base64, and `ANDROID_KEYSTORE_PASSWORD`.
- **The workflow's "Sign" step** writes the keystore into the runner's temporary folder, finds the key's alias, its name in the keystore (`keytool -list`), signs with the SDK's `apksigner`, deletes the keystore file, and prints the certificate it signed with (`apksigner verify --print-certs`). The APK is named `Sioul-<version>-android-arm64.apk`.
- **Without the secrets** (a fork's run, say), the step makes a key for that run alone: the APK installs, but a phone takes the next one for another app, to be uninstalled first.
- **A version**: a tag `v…` pushed starts `.github/workflows/release.yml`. Its `android` job calls the same workflow on the tag (`uses: ./.github/workflows/android.yml`, with `secrets: inherit`, so with the same key), and its `release` job puts the APK on the GitHub release beside the other packages.

Android's documentation: [signing an app](https://developer.android.com/studio/publish/app-signing).

## Trying it, and reading what it says
- **The log**: `main.cpp` sends what Rust writes to its standard output and error to Android's log, *logcat*, tagged `sioul`; most Java classes log under the same tag. `adb logcat -s sioul` shows them, with the phone plugged in and USB debugging on (`adb` is Android's command-line tool, in the SDK's platform tools). Qt's messages go there too, such as each page's making time (`sioul-perf`).
- **Android's view of Sioul**: `adb shell dumpsys alarm` lists the alarms given; `adb shell dumpsys meminfo com.aurelienpierre.sioul:steps` gives one process's memory; [android.md](android.md#in-the-background) lists more.
- **On a computer**: Rust's tests cover what is decided (`cargo test -p sioul-app homecard`, `appnotes`…), Android's side being left out by `cfg` there; `SIOUL_GRAB_PHONE`, with `SIOUL_GRAB`, lays the window out at a phone's size ([Qt Quick and QML](qt-quick.md#a-computer-and-a-phone)). The Java and `main.cpp` are compiled only by the APK's build, and the Java reference is made with the website ([the API reference](https://aurelienpierre.github.io/sioul/dev/api.html)).
- **What was tried on a phone, and what not yet**: each part of [android.md](android.md) ends with it.

Android's documentation: [adb](https://developer.android.com/tools/adb), [logcat](https://developer.android.com/tools/logcat).
