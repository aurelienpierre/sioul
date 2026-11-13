# Holding other apps' notifications without doubling the noise (Android)

Researched on 13 November 2026, before phase 4 of the redesign of notifications: AOSP's notification service read at each release branch, no phone. Options 1 and 2 of section 5 were chosen the same day; what was built is in [android.md](../android.md#notifications-from-other-apps).

**Sources.** AOSP `frameworks/base`, read at the heads of the release branches on android.googlesource.com (`android10-release` … `android16-release`, plus `android12L-release`, `android13-s1-release`, `android13-qpr1/2/3-release`, `android14-qpr1/3-release`, `android15-qpr2-release`), and LineageOS `android_frameworks_base` on GitHub (`lineage-17.1` … `lineage-23.0`). Line numbers are those of the branch head and drift between branches; function names are stable. The AOSP paths are:

- `services/core/java/com/android/server/notification/NotificationManagerService.java` (NMS below)
- `services/core/java/com/android/server/notification/SnoozeHelper.java`
- `services/core/java/com/android/server/notification/NotificationAttentionHelper.java` (Android 15+)
- `services/core/java/com/android/server/notification/GroupHelper.java`
- `services/core/java/com/android/server/notification/ZenModeHelper.java`, `PreferencesHelper.java`
- `core/java/android/service/notification/NotificationListenerService.java`

URL pattern: `https://android.googlesource.com/platform/frameworks/base/+/refs/heads/<branch>/<path>`; on cs.android.com, `https://cs.android.com/android/platform/superproject/+/<branch>:frameworks/base/<path>`. Gitiles hides the commit history behind a sign-in, so each version boundary below was found by reading each branch head, not from the commit that made the change.

---

## 1. The unsnooze: does a held notice alert again when it comes back?

**It depends on the Android version, and the docs are right only for some versions.** A timed snooze ends when `SnoozeHelper`'s exact alarm fires. `SnoozeHelper.mBroadcastReceiver.onReceive` then calls `repost(key, userId, false)`, which hands the stored record to NMS's callback, set up in `NotificationManagerService.onStart` (`new SnoozeHelper(getContext(), (userId, r, muteOnReturn) -> enqueueNotificationInternal(...))`). What that callback passes decides the sound:

| Android | LineageOS | Callback (in `onStart`) | Return of a timed snooze |
|---|---|---|---|
| 10 | 17.1 | `enqueueNotificationInternal(..., userId)`: no silence parameter exists (`android10-release` NMS l.1730–1744) | **alerts** as its channel says |
| 11, 12, 12L, 13 first release | 18.1, 19.1 | `enqueueNotificationInternal(..., userId, true)`: `postSilently = true` for every return (`android12-release` NMS l.2438–2449; the same in `android11-release`, `android12L-release`, `android13-release`, `android13-s1-release`, `lineage-18.1`, `lineage-19.1`) | **silent** |
| 13 QPR1 (Dec 2022) and later: 13 QPR2/3, 14, 15, 16 | 20.0, 21.0, 22.1, 22.2, 23.0 | `enqueueNotificationInternal(..., userId, muteOnReturn, ...)` (`android13-qpr1-release` NMS l.2496; `android15-release` NMS l.2701–2712) with `muteOnReturn = false` from the alarm | **alerts** as its channel says |

**Why the return alerts on 10 and on 13 QPR1+.** Snoozing removes the record from the live lists: `SnoozeNotificationRunnable.snoozeNotificationLocked` calls `removeFromNotificationListsLocked(r)` and `cancelNotificationLocked(r, false, REASON_SNOOZED, ...)` (`android15-release` NMS l.8214 ff.). The return therefore finds no record under its key: `PostNotificationRunnable` takes the `index < 0` branch, leaves `r.isUpdate` false and sets `isInterruptive` from `isVisuallyInterruptive(null, r)`, which is true (`android15-release` NMS l.8698–8708). It is a fresh post. The app's own `FLAG_ONLY_ALERT_ONCE` does not help, because `shouldMuteNotificationLocked` honours it only for updates (`record.isUpdate && FLAG_ONLY_ALERT_ONCE`; `android15-release` `NotificationAttentionHelper.shouldMuteNotificationLocked` l.576 ff.; `android10-release` NMS l.5823 ff.). `buzzBeepBlinkLocked` then plays the channel's sound and vibration. On 15 and later that code lives in `NotificationAttentionHelper.buzzBeepBlinkLocked` (l.364); on 10–14 it is `NotificationManagerService.buzzBeepBlinkLocked`; the logic is the same.

**Why it is silent on 11 to 13.0.** `NotificationRecord.setPostSilently(true)` is set in `enqueueNotificationInternal`, and `shouldMuteNotificationLocked` returns true with the comment "Suppressed because a user manually unsnoozed something (or similar)" (`android12-release` NMS l.7576–7586). The notice still enters the shade and the status bar. Whether SystemUI still shows it as a heads-up banner (for a high-importance channel) I could not confirm: the silent flag is not part of the `Ranking` SystemUI reads.

**Who can bring a notice back silently on 13 QPR1+.** Only `unsnoozeNotificationFromSystemListener`, which passes `muteOnReturn = true` but throws `SecurityException("Not allowed to unsnooze before deadline")` unless `ManagedServiceInfo.isSystem` (`android15-release` NMS l.5214–5229). SystemUI uses it for its own "Undo". A third-party listener cannot be `isSystem`. The assistant's `unsnoozeNotificationFromAssistant` passes `false` (l.5195–5206), so it alerts as well.

**What else mutes a return on every version** (all in `shouldMuteNotificationLocked`):
- a do-not-disturb mode that intercepts it at the moment it comes back (`record.isIntercepted()`);
- a listener hint `HINT_HOST_DISABLE_EFFECTS` or `HINT_HOST_DISABLE_NOTIFICATION_EFFECTS` in force at that moment (`disableNotificationEffects`, `android15-release` `NotificationAttentionHelper` l.936 ff.);
- the per-app rate limit: an app alerts at most once a second (`AlertRateLimiter.ALLOWED_ALERT_INTERVAL = 1000`, checked through `mUsageStats.isAlertRateLimited(pkg)`). A gathered burst of ten notices from one app therefore rings once; ten notices from five apps ring up to five times within a second or two;
- a child whose group alert behaviour gives the alert to the summary (`suppressAlertingDueToGrouping`);
- on 15+, "notification cooldown" (`PolitenessStrategy`), which softens and then mutes repeated alerts from one app. It is behind the `politeNotifications` flag; I could not confirm whether release builds of 15 turn it on.

**The first alert (problem 1), confirmed in code.** `PostNotificationRunnable.postNotification` calls `buzzBeepBlinkLocked` before it tells the listeners (`notifyListenersPostedAndLogLocked`, `android15-release` NMS l.8764 then l.8775; `mListeners.notifyPostedLocked` after `buzzBeepBlinkLocked` on `android10-release` l.5403/5408 and `android12-release` l.7098/7103). The listener's `onNotificationPosted` is dispatched after the sound has started, on every version.

**Side findings that matter to Sioul's holding code** (`AppNotes.java`):
- **Re-snoozing a held notice is a no-op on Android 10.** There, `SnoozeNotificationRunnable.run` looks the key up with `findNotificationByKeyLocked`, which sees only posted notices. From 11 on it uses `findInCurrentAndSnoozedNotificationByKeyLocked`, so `review`'s new times and its "release by a one-second snooze" work only from Android 11. On 10, a held notice keeps its first time. (Android 9, the declared minimum, was not read; it is presumably like 10.)
- **App updates while held stay held and silent.** `checkDisqualifyingFeatures` sees `mSnoozeHelper.isSnoozed(...)`, stores the update with `mSnoozeHelper.update(...)` and returns before any sound (`android15-release` NMS l.8081–8094). An app that posts a *new* notification per message (a new id) is a new post, and that one rings first.
- **Each hold counts as a user interaction with the app** (`reportUserInteraction(r)` in `snoozeNotificationLocked`), which feeds app standby. This is harmless, but it is not nothing.
- **On 15+, every hold notes an app op against Sioul.** `snoozeNotificationInt` notes `OP_RAPID_CLEAR_NOTIFICATIONS_BY_LISTENER` when the notice is less than 5 s old (`NOTIFICATION_RAPID_CLEAR_THRESHOLD_MS = 5000`, behind the flag `rapidClearNotificationsByListenerAppOp`; `android15-release` NMS l.10094–10148). Sioul holds within milliseconds, so it notes this op at every hold. AOSP only records the op; I could not find what reads it (presumably Google Play Protect, absent from de-Googled builds; not confirmed).
- **Limits.** From 13 on, `SnoozeHelper.canSnooze` refuses beyond `CONCURRENT_SNOOZE_LIMIT = 500` held notices ("Cannot snooze … too many snoozed notifications"); the notice then stays in the shade. Snoozing the only child of an app group also snoozes its summary, and snoozing a summary snoozes all its children (`snoozeLocked`). Autogroup summaries are never snoozed (`isSnoozable`).

**Consequence for the docs.** `docs/android.md` and the header of `AppNotes.java` said, before 13 November 2026, a held notice "comes back whole … alerting as its channel says". That is true on Android 10 and on 13 QPR1 and later, which covers LineageOS 20–23 and current /e/OS. It is false on 11, 12, 12L and the first release of 13, where the return is silent. The 2019 test phone runs Android 12 ([android.md](../android.md)), so the second alert **cannot be heard on that phone**. Any test of a phase 4 fix needs a phone on 13 QPR1 or later.

**Two more side findings, found while answering item 2.**
- **A reboot loses what is held.** `SnoozeHelper` keeps the held records in memory only (`mSnoozedNotifications`); its policy file stores just key → time (`writeXml`, `mPersistedSnoozedNotifications`). After a reboot, a held notice comes back only if its app posts the same key again: NMS then snoozes it again before any sound, until the old time (`getSnoozeTimeForUnpostedNotification`, `android15-release` NMS l.8527–8535). Most apps do not post again at boot, so their held notices are gone. The docs' "Android brings it back then … even while Sioul sleeps" holds only without a reboot in between.
- **LineageOS 17.1 (Android 10)** has the same callback as AOSP 10 (`lineage-17.1` NMS l.1911) and the same re-snooze no-op (`findNotificationByKeyLocked`, l.5552).

---

## 2. Bringing a held notice back without the second alert

| Mechanism | Silent return? | What it needs | Verdict |
|---|---|---|---|
| `snoozeNotification(key, ms)`, what Sioul does now | silent on 11–13.0 only | notification access | this is the problem |
| Snooze criterion `snoozeNotification(key, criterionId)` | no | a hidden system API | no |
| `unsnoozeNotification` | yes | a system listener (SystemUI) | out of reach |
| Cancel, and repost under Sioul's own silent channel | yes | notification access | works, at a cost |
| Listener hint `HINT_HOST_DISABLE_NOTIFICATION_EFFECTS` held for a few seconds around the release | yes | notification access | **works, cheap** |
| Sioul's own do-not-disturb rule turned on for the release | yes, for what its policy intercepts | policy access (Sioul has it) | works, visible |
| The assistant lowers the importance of the returning notice | yes | becoming the notification assistant | works, needs adb on 12+ |
| A companion-device association, then `updateNotificationChannel` | yes, for the channels Sioul silences | one association dialog | works, per channel |

**The snooze criterion gains nothing.** `NotificationListenerService.snoozeNotification(String key, String snoozeCriterionId)` is `@hide @removed @SystemApi` (`android15-release` l.794–818), so the SDK does not offer it to an app. A criterion snooze sets no alarm (`SnoozeHelper.snooze(record, contextId)` only stores the record). It ends when the assistant calls `unsnoozeNotificationFromAssistant`, which passes `muteOnReturn = false`, or when someone snoozes the notice again with a time. Both returns alert. I did not test whether reflection could reach the hidden method; it would not help anyway.

**Unsnoozing early is out of reach.** `unsnoozeNotificationFromSystemListener` requires `info.isSystem` (item 1). The public assistant method `NotificationAssistantService.unsnoozeNotification` reaches `unsnoozeNotificationInt(key, info, false)` and alerts.

**Cancel, then repost in a digest: it works, but the app finds out.**
- A listener's `cancelNotification(key)` goes through `cancelNotificationFromListenerLocked`, which cancels with `sendDelete = true` and refuses ongoing notices (`mustNotHaveFlags = FLAG_ONGOING_EVENT`, plus `FLAG_LIFETIME_EXTENDED_BY_DIRECT_REPLY` on 15; `android15-release` NMS l.5141–5152). `cancelNotificationLocked` then fires the app's `deleteIntent` (l.9705–9713): the app learns that its notice was dismissed. A snooze never fires it.
- The app's next post under that key is a fresh post. It rings on arrival, and Sioul must hold or cancel it again.
- The original's taps and buttons survive. The listener receives the whole `Notification`, its `PendingIntent`s included. When Sioul posts a notice carrying them, NMS allow-lists every `PendingIntent` in it for background activity starts and foreground-service starts (the `allPendingIntents` loop, l.7524–7543), exactly as for the original. They fail silently once the app cancels or replaces them (`FLAG_CANCEL_CURRENT`, `FLAG_ONE_SHOT`), and after a reboot.
- What is lost: the conversation itself (its shortcut, bubble, people ranking, and the priority-conversation exception of do-not-disturb), the app's small icon (Sioul can show it as the large icon), the app's own grouping, and what a watch or another listener shows. A Reply copied into Sioul's notice would make SystemUI wait for *Sioul* to update its notice, while the app answers by posting its own (I know this from SystemUI's RemoteInput behaviour; I did not trace it in source).
- It fits automated notices, which have one tap target and rarely a reply. It breaks the rule written in `docs/android.md`: "Holding is snoozing, never cancelling".

**The listener hint: a quiet window around the release.** It is the cheapest fix, and it needs no new permission.
- `requestListenerHints(int)` has been public API since Android 5. NMS's `requestHintsFromListener` checks only that the caller is a bound listener (`android15-release` NMS l.5371–5391).
- While `HINT_HOST_DISABLE_NOTIFICATION_EFFECTS` is set, `disableNotificationEffects` returns `"listenerNoti"` for every notice whose audio usage is not `USAGE_NOTIFICATION_RINGTONE`, and `shouldMuteNotificationLocked` mutes it: no sound, no vibration, no light (`android15-release` `NotificationAttentionHelper` l.936–950; `android13-release` NMS l.5946–5966). Phone calls still ring.
- **What else it does.** `updateEffectsSuppressorLocked` passes the hint to `ZenModeHelper.setSuppressedEffects`, and `applyRestrictions` then denies `OP_PLAY_AUDIO` and `OP_VIBRATE` to *every* app for the notification audio usages (`android15-release` `ZenModeHelper` l.2184–2235). That includes Sioul's own reminders and an app that plays its own sound with a notification usage. Calls and alarms are not touched. NMS sends `ACTION_EFFECTS_SUPPRESSOR_CHANGED` to registered receivers, and every listener gets `onListenerHintsChanged`. AOSP SystemUI shows nothing: `VolumeDialogControllerImpl` stores the suppressor's name, and `VolumeDialogImpl` does not display it on 10, 12, 13 or 15.
- **It fails safe.** Hints belong to the listener that asked. NMS drops them when that listener unbinds or dies (`onServiceRemovedLocked` → `removeDisabledHints`, `android15-release` NMS l.11553–11557). A crash inside the window therefore cannot leave the phone mute.
- **How Sioul would use it.** At a release, Sioul sets the hint, snoozes each released key again for one second, and clears the hint once `onNotificationPosted` has come for every released key (the listener hears of a post only after the sound decision, item 1), or after about five seconds at most.
- **What it costs.** Something unrelated that arrives inside those seconds (a person's message, one of Sioul's own reminders) comes silent. Sioul sees it, since it is not among the released keys, and can snooze it for one second after the window so that it rings on its return (on 13 QPR1+; on 11–13.0 returns are silent anyway). Sioul avoids putting a window on its own reminder times.
- **What it needs.** The listener's process does not run during Doze, so the release must come from Sioul's own exact alarm (`setExactAndAllowWhileIdle`, under "Alarms & reminders", which Sioul already asks for for events) instead of `SnoozeHelper`'s. `SnoozeHelper`'s timer stays as a safety net: Sioul snoozes until T plus a few minutes and releases at T inside its window. If Sioul's alarm never comes, the notice returns at the safety-net time with its sound, and nothing is lost. On Android 10, where a held notice cannot be moved (item 1), Sioul opens the window around the notice's first time instead.

**Sioul's own do-not-disturb rule, on for the release.** This also works: a notice intercepted at its return is muted (`record.isIntercepted()`). A policy that allows calls, messages, conversations, alarms and media intercepts exactly the automated notices (`ZenModeFiltering.shouldIntercept`, `android15-release` l.165 ff.). The costs are a do-not-disturb icon that flickers three times a day, an `onInterruptionFilterChanged` storm into Sioul's own do-not-disturb state machine, a clash with the person's own modes, and an automaton's SMS (category message) that still rings. The hint does the same job invisibly. **Not recommended.**

**Becoming the notification assistant: the only hook that acts per notice before its sound.** It fixes problems 1 and 2 at once.
- At every enqueue, the return of a snoozed notice included, NMS gives the assistant `onNotificationEnqueuedWithChannel` and delays the post by `DELAY_FOR_ASSISTANT_TIME`: 100 ms on 10, 200 ms on 15 (`android15-release` NMS l.503, l.8595–8600). An `Adjustment` with `KEY_IMPORTANCE = IMPORTANCE_LOW`, applied within that delay, lowers the notice below the threshold `buzzBeepBlinkLocked` needs (`importance >= IMPORTANCE_DEFAULT`): no sound and no heads-up.
- `KEY_IMPORTANCE` is in `ALLOWED_ADJUSTMENTS` (l.451–462). `NotificationRecord.calculateImportance` ignores it when the person set the channel's importance by hand (`hasUserSetImportance`) or the importance is fixed (`android15-release` `NotificationRecord` l.888–903).
- **Getting the role.** On 10 and 11, Settings lists the installed assistants to choose from (`NotificationAssistantPreferenceController` opens `NotificationAssistantPicker`). From 12 on, Settings only toggles the default one ("Enhanced notifications", a `TogglePreferenceController`). The person must then run `adb shell cmd notification allow_assistant com.aurelienpierre.sioul/<service>` once from a computer (`NotificationShellCmd`; `setNotificationAssistantAccessGrantedForUser` accepts only system, SystemUI or shell). The grant survives reboots; turning "Enhanced notifications" back on restores the default assistant.
- **What it costs.** There is one assistant per user, and Sioul would replace the default, which is `android.ext.services/android.ext.services.notification.Assistant` on AOSP and LineageOS 22.2 (`config_defaultAssistantAccessComponent`). That service gives smart replies and actions and, on 15, the `KEY_SENSITIVE_CONTENT` mark. Android 15 uses that mark to hide one-time codes from untrusted listeners (NMS l.5347, l.10624). Sioul would have to set it itself, or codes would reach every other listener in clear. A decision later than 100–200 ms lets the notice post and ring, as today.

**A companion-device association: the listener may then change other apps' channels** (details in item 3). Sioul would make a held app's channels silent once, and both alerts would be gone for those channels.

---

## 3. Preventing the first alert for the apps Sioul holds

The sound starts before any listener hears of the notice (item 1), so a listener can only prevent it ahead of time, through state that Android reads at post time. Four such states exist: the channel's settings, a do-not-disturb policy, the assistant's adjustment (item 2), and a listener hint. All of them exist unchanged in LineageOS 20.0, 21.0 and 22.2, which I spot-checked: `hasCompanionDevice`, `DELAY_FOR_ASSISTANT_TIME = 200`, the token-only check in `requestHintsFromListener`, the `isSystem` check on early unsnooze, and `"listenerNoti"` in `NotificationAttentionHelper` on 22.2. /e/OS builds on LineageOS; I did not read its trees.

**a. A silent channel, set by the person (the current choice).** This works on every version and needs nothing. It is per channel: an SMS app with one channel for friends and bank codes cannot be split. From 11 on, a conversation the person marks "Priority" or "Default" gets its own channel at importance `max(original, DEFAULT)`, so it rings even under a silenced app channel (SystemUI `NotificationConversationInfo.UpdateChannelRunnable`, `android15-release` l.631–651). That conversation also rings when Sioul holds it, so this only moves the problem from the app to the person.

**b. A silent channel, set by Sioul after one companion-device association.** No root and no adb.
- **What it unlocks.** `NotificationListenerService.updateNotificationChannel(pkg, user, channel)` reaches `updateNotificationChannelFromPrivilegedListener`. That method calls `verifyPrivilegedListener(token, user, false)`, which passes when `hasCompanionDevice(info)` finds *any* association for the listener's package, of any profile (`withDeviceProfiles = null`; `android10-release` NMS l.7279–7304, `android15-release` l.10672–10705, `android16-release` l.11928–11961). It then calls `updateNotificationChannelInt(pkg, uid, channel, true)`, and `PreferencesHelper.updateNotificationChannel(..., fromUser = true, ...)` records the change **as the person's own** and locks those fields against the app (`lockFieldsForUpdateLocked`; `android15-release` `PreferencesHelper` l.1131–1201). `getNotificationChannels(pkg, user)` opens with the same association. On 15, changing a channel's sound to a URI Sioul cannot read is refused (`verifyPrivilegedListenerUriPermission`); setting no sound, or importance LOW, needs no URI.
- **Getting the association.** Sioul declares `<uses-feature android:name="android.software.companion_device_setup" android:required="false"/>` and calls `CompanionDeviceManager.associate(...)` with a Bluetooth, BLE or Wi-Fi device filter and **no device profile**. A profile-less request needs no permission. By contrast, `REQUEST_COMPANION_SELF_MANAGED` and the COMPUTER profile are `signature|privileged`, and the WATCH profile is `normal` (`android15-release` `core/res/AndroidManifest.xml` l.4335–4386). Android shows its own chooser, which lists already-paired Bluetooth devices next to discovered ones (`CompanionDeviceDiscoveryService`: `getBondedDevices()`, `android12-release` l.183, `android15-release` l.284) and Wi-Fi scan results. The person picks their computer, which is honest, since Sioul relays notifications to it.
- **It lasts.** The association stays until Sioul is uninstalled or drops it. Only *self-managed* associations are pruned after 90 idle days (`DisassociationProcessor.removeIdleSelfManagedAssociations`, `ASSOCIATION_REMOVAL_TIME_WINDOW_DEFAULT = 90 days`). The computer need not come near again.
- **It is available on de-Googled builds.** AOSP ships the chooser app in `build/target/product/media_system.mk` (`CompanionDeviceManager`), and `frameworks/native/data/etc/handheld_core_hardware.xml` declares the feature. LineageOS and /e/OS phones use these product files. One device tree could drop them; that is unverified per device.
- **What Sioul would do with it.** For each app it holds, Sioul sets the channels the person names, or that rang while held (`app-notes-seen.toml`), to importance LOW: in the shade, no sound, no heads-up. Neither the arrival nor the return rings. Sioul keeps the original values and writes them back when the person stops holding that app. The changes appear in Android's settings as the person's own.
- **Limits.** The fix is per channel, as in (a). From Android 16 a companion listener may also create a conversation's own channel (`createConversationNotificationChannelForPackageFromPrivilegedListener`, `android16-release` NMS l.7068), which would allow per-person sound inside one chat app; 11–15 lack it.

**c. Sioul's do-not-disturb rule kept on all day.** This needs nothing new (Sioul owns a rule for its pauses), and it fails where Sioul's matrix differs from Android's.
- In priority mode, `ZenModeFiltering.shouldIntercept` (`android15-release` l.165 ff.) lets through critical notices, channels that bypass do-not-disturb (from 15 only if the policy allows priority channels: `ZenPolicy.Builder.allowPriorityChannels`, a public modes API in 15; `allowChannels(int)` is hidden), alarms, events, reminders, media and system as the policy says, conversations (all, or priority only), calls, and messages from anyone, contacts or starred contacts. It intercepts everything else. An intercepted notice is muted at arrival and again at return while the rule is on. With `ZenPolicy` visual effects shown in the shade but peeking off, it stays visible without flashing. Android 15's `ZenDeviceEffects` (grayscale, dimmed wallpaper, …) concern the screen, not sound.
- **What it costs.** A do-not-disturb icon shows all day and the Quick Settings tile reads "on". Every app sees priority mode, and the person's own modes merge with Sioul's (the most restrictive wins). The matrix maps only onto starred, contacts and anyone, plus priority conversations and, on 15, priority channels. **A person Sioul lets through but the policy intercepts arrives silent**, and no listener can make that notice ring afterwards: its return would be intercepted too. For pauses, which is today's use, this trade is right. All day, it is not.

**d. The hint held permanently, with Sioul playing the sounds itself: rejected.** While the hint is set, `ZenModeHelper.applyRestrictions` denies `OP_PLAY_AUDIO` and `OP_VIBRATE` to every app for the notification audio usages, Sioul included (item 2). Sioul would have to play its sounds under a call, alarm or system usage and rewrite Android's rules for silent and vibrate modes, do-not-disturb, calls in progress and exclusive audio (`NotificationAttentionHelper.playSound`, `playVibration`). That is a fragile copy of Android's alert logic.

**e. The assistant (item 2).** This is exact and per notice, on every version. It needs the settings picker on 10–11 and one adb command from 12 on, and it takes over the default assistant's duties, including the one-time-code mark on 15.

**What needs privileges, and what does not.** (a), (c) and the hint window need nothing beyond what Sioul holds today. (b) needs one Android dialog, with a paired Bluetooth device or a Wi-Fi network in range. (e) needs adb once on 12+. Nothing here needs root or a system signature. Early unsnooze, criterion snoozes, self-managed companion associations and the COMPUTER profile are out of reach.

---

## 4. Auto-grouping (for phase 3)

**Threshold** (`config_autoGroupAtCount`, `core/res/res/values/config.xml`, passed to `new GroupHelper(...)` in NMS):

| Android | LineageOS | Ungrouped notices of one app before the system groups them |
|---|---|---|
| 10, 11, 12, 12L | 17.1, 18.1, 19.1 | **4** |
| 13 (first release onward), 14, 15, 16 | 20.0, 21.0, 22.2 | **2** |

A device overlay may change the value; the AOSP and LineageOS defaults are as shown. Android 16 adds a second rule: an app with `AUTOGROUP_SPARSE_GROUPS_AT_COUNT = 6` or more groups of fewer than `MIN_CHILD_COUNT_TO_AVOID_FORCE_GROUPING = 3` children is grouped by force (`android16-release` NMS l.641, `GroupHelper` l.89–91), behind the flags `notificationForceGrouping` and `notificationForceGroupSingletons`. I could not confirm whether 16's release builds turn them on.

**Does the autogroup summary inherit `FLAG_ONGOING_EVENT` / `FLAG_NO_CLEAR` from a foreground-service child?**

| Android | Foreground-service notice gets (NMS, `PostNotificationRunnable`) | Autogroup summary gets |
|---|---|---|
| 10 | `FLAG_ONGOING_EVENT \| FLAG_NO_CLEAR` (`android10-release` l.5395) | nothing: `createAutoGroupSummary` sets only the summary flags and `GroupHelper` tracks no ongoing state |
| 11, 12 | `FLAG_ONGOING_EVENT \| FLAG_NO_CLEAR` (`android12-release` l.7088) | `FLAG_ONGOING_EVENT` while any autogrouped child is ongoing (`GroupHelper.addToOngoingGroupCount` → `NMS.updateAutobundledSummaryFlags`) |
| 13 | `FLAG_NO_CLEAR`, plus `FLAG_ONGOING_EVENT` unless `mAllowFgsDismissal` (`android13-release` l.7473–7477) | `FLAG_ONGOING_EVENT` as on 12 (`createAutoGroupSummary(..., needsOngoingFlag)`) |
| 14, 15, 16 | `FLAG_NO_CLEAR` only; ongoing only if the app set it (`android14-release` l.7929) | **`FLAG_ONGOING_EVENT \| FLAG_NO_CLEAR` if any child has either** (`ANY_CHILDREN_FLAGS`, `GroupHelper.getAutogroupSummaryFlags`), and `FLAG_AUTO_CANCEL` only if all children have it |

So on 11–13, a foreground-service notice (always ongoing there) in the autogroup makes the summary ongoing: the group cannot be swiped away, and "Clear all" skips it. From 14 on, a foreground-service child gives the summary no-clear ("Clear all" skips the group), and a child the app made ongoing also makes the summary ongoing.

**An explicit `setGroup` avoids it, on 10–15.** `GroupHelper.onNotificationPosted` groups only notices with `!sbn.isAppGroup()`, and `StatusBarNotification.isAppGroup()` is true as soon as the app gives a group key **or a sort key**. An app-grouped notice is also taken out of an existing autogroup (`maybeUngroup`). The rule is the same on every branch from 10 to 15. **On 16 with force grouping, a group child without a summary counts as ungrouped** (`isGroupChildWithoutSummary` → `addToUngroupedAndMaybeAggregate`, `android16-release` `GroupHelper` l.827–834, 1606–1624). Sioul should therefore give each of its notices an explicit group **and** post a summary for every group that has children. Colorized foreground-service notices, call-style and media notices are never force-grouped on 16 (`isNotificationGroupable`, l.2010–2030). The simplest safe recipe is: the foreground-service notice (if any) in a group of its own; Sioul's other notices in named groups, each with a summary.

---

## 5. Options for Sioul, and a recommendation

**A distinction the research makes plain.** For a *person*, the alert at arrival is the unwanted one, since it comes when that person may not reach the person. The alert at the return is the right one, since the person may reach the person now. For *automated* notices gathered three times a day, both alerts are unwanted. A good fix therefore silences the return of a gathered batch, and of a release the person asked for (they are looking at the screen), but lets a person's return ring. Today's code does the opposite on 11–13.0, where every return is silent, and rings twice on 13 QPR1+.

### Option 1: a quiet window for gathered releases (listener hint)
- **What it does.** Sioul releases each gathered batch from its own exact alarm, inside a window of a few seconds under `HINT_HOST_DISABLE_NOTIFICATION_EFFECTS` (item 2). It does the same for "Let what your phone holds through". A person released at their time comes back outside any window and rings as their channel says.
- **What the person hears and sees.** Gathered notices appear in the shade three times a day without a sound. A held person still rings once on arrival (if the app's channel is audible), and rings again when they may reach the person, on 13 QPR1+. Nothing shows in Android's UI.
- **Battery and complexity.** Three exact alarms a day and two binder calls per batch: negligible. About a hundred lines of Java in `AppNotes` (the window, the release driver, the five-second fallback, re-ringing a stray notice that fell inside the window). Rust gives the batch times it already computes. `SnoozeHelper`'s timer stays as the safety net (item 2).
- **What it needs from the person.** Nothing new: "Alarms & reminders" is already asked for for events.
- **Limit.** It leaves the first alert as it is.

### Option 2: Sioul silences the held apps' channels itself (companion association), plus Option 1
- **What it does.** Once, Sioul asks Android to associate it with the person's computer: Android's own dialog, listing the paired Bluetooth devices (item 3b). Then, for each app whose notices Sioul gathers, Sioul sets that app's channels to "silent" (importance LOW) itself, keeps the original values, and restores them when the person stops holding the app. This replaces the guided list of "channels that rang while held" with one dialog, and keeps that list as the fallback (no companion feature on the phone, or the person says no).
- **What the person hears and sees.** Gathered apps never ring, neither on arrival nor on return, and they no longer flash as banners. Chat apps keep their channel and their sound, so that the people let through ring; a person held in such an app still rings once on arrival (from Android 16, per-conversation channels could remove that too). The changes appear in Android's settings as if the person had made them.
- **Battery and complexity.** No running cost. Medium code: the association flow (one activity and its result), reading and writing channels, storing the originals, and restoring them when the person stops holding an app. Android gives Sioul no hook at its own uninstall, so the channels it silenced stay silent then; the guide must say so.
- **What it needs from the person.** One dialog, with Bluetooth paired to the computer, or a Wi-Fi filter on the home network instead.

### Option 3: Sioul as the notification assistant
- **What it does.** Sioul's listener becomes a `NotificationAssistantService`. At each enqueue, the return of a held notice included, Sioul answers within 200 ms with `KEY_IMPORTANCE = LOW` for whatever must not sound now (item 2).
- **What the person hears and sees.** Exactly the matrix. A person rings when, and only when, they may reach the person. Everything held arrives silent and without a banner, and every gathered batch is silent. No per-channel settings at all.
- **Battery and complexity.** The same wake-ups as the listener today. High complexity: the decision must come within 100–200 ms, so the Rust library must stay loaded and the decision stay in memory; on 15, Sioul must mark one-time codes (`KEY_SENSITIVE_CONTENT`) or they reach other listeners in clear; and a late answer falls back to today's behaviour.
- **What it needs from the person.** On 12+, one `adb shell cmd notification allow_assistant …` from a computer. The person also gives up Android's smart replies. Turning "Enhanced notifications" on in Settings silently undoes the grant.

### Recommendation
**Build Option 1 now, then Option 2; keep Option 3 as a later, opt-in "exact mode".**
- Option 1 is small and fails safe (a hint dies with the listener). It needs no new permission and fixes the worse half of the problem: the three daily batches, which come back with sound on every phone running 13 QPR1 or later.
- Option 2 then removes the arrival alert for the gathered apps with one honest dialog instead of a list of settings pages. It also suits Sioul's rule that the person corrects rather than validates.
- Option 3 is the only exact answer, but it costs an adb step, takes over a system role, and carries the one-time-code duty. It is worth it only if, after 1 and 2, the person still hears held people ring on arrival often enough to mind. Sioul can count that already (`app-notes-seen.toml`, the channels that rang while held).

### Fixes these findings call for, whichever option is chosen
- **Correct the docs** (`docs/android.md`, the header of `AppNotes.java`). A held notice's return rings on Android 10 and on 13 QPR1+, and is silent on 11–13.0. Held notices are lost at a reboot unless their app posts them again. On Android 10, a held notice keeps its first time (re-snoozing does nothing).
- **Test on a phone with Android 13 QPR1 or later.** The Android 12 test phone cannot show the second alert at all.
- **Note the Android 15 app op.** Every hold within 5 s of posting records `OP_RAPID_CLEAR_NOTIFICATIONS_BY_LISTENER` against Sioul. This is harmless on de-Googled phones as far as I can tell; I have not confirmed what reads it on phones with Google services.

### What I could not confirm
- Whether a silent return still shows a heads-up banner on 11–13.0.
- Whether SystemUI ignores listener hints for banners. From reading, it does not consult them, but I did not trace it.
- Whether Android 15 release builds turn on "notification cooldown", or Android 16 release builds turn on force grouping.
- Whether every LineageOS or /e/OS device tree keeps the companion-device feature.
- Which component reads `OP_RAPID_CLEAR_NOTIFICATIONS_BY_LISTENER`.
- The exact commit that changed the return on 13 QPR1: the branch heads bound it, but Gitiles hides the history behind a sign-in.
