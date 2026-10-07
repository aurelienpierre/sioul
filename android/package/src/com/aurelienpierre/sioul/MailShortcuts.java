// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.content.ComponentName;
import android.content.Context;
import android.content.Intent;
import android.content.SharedPreferences;
import android.content.pm.ShortcutInfo;
import android.content.pm.ShortcutManager;
import android.graphics.drawable.Icon;
import android.net.Uri;
import android.os.Build;
import android.util.Log;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

import java.util.ArrayList;
import java.util.Collections;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

/**
 * Sioul's addresses that can send, published to Android (docs/android.md,
 * "Sharing"): one shortcut each, which the share sheet offers as a direct
 * target from Android 10 (its {@code <share-target>} in res/xml/sioul_shortcuts.xml
 * matches their category), and a long press on Sioul's icon as "Write from
 * you@example.org". Chosen, ShareActivity makes the draft from that address.
 *
 * Rust gives the list whenever it changes (crates/sioul-app/src/outside.rs,
 * android/main.cpp): JSON {accounts: [{id, short, long}], gone}, the most
 * used first. Only Sioul's own "mail:" shortcuts are touched; the "Pause"
 * one (the manifest's) stays. Only the address shows in them: nothing else
 * of the account, and nothing is sent until you press Send.
 */
final class MailShortcuts
{
    /** The share sheet's category for them, as res/xml/sioul_shortcuts.xml names it. */
    static final String CATEGORY = "com.aurelienpierre.sioul.category.MAIL";
    private static final String KEPT = "sioul-mail-shortcuts";
    private static final String GIVEN = "given";

    private MailShortcuts() {}

    /** Rust's list, published when it differs from the last one given. Any thread. */
    static synchronized void set(Context context, String json)
    {
        ShortcutManager manager = context.getSystemService(ShortcutManager.class);
        if (manager == null)
            return;
        SharedPreferences kept = context.getSharedPreferences(KEPT, Context.MODE_PRIVATE);
        if (json.equals(kept.getString(GIVEN, "")) && !missing(manager, json))
            return;
        final JSONArray accounts;
        final String gone;
        try {
            JSONObject list = new JSONObject(json);
            accounts = list.optJSONArray("accounts") == null ? new JSONArray() : list.getJSONArray("accounts");
            gone = list.optString("gone", "");
        } catch (JSONException e) {
            Log.e(DoseAlarms.TAG, "Sharing: the list of addresses does not read; the shortcuts stay as they were. " + e);
            return;
        }
        // Never more than Android takes: its limit per activity, less the manifest's ("Pause").
        int room = Math.max(0, manager.getMaxShortcutCountPerActivity() - manager.getManifestShortcuts().size());
        List<ShortcutInfo> made = new ArrayList<>();
        Set<String> ids = new HashSet<>();
        for (int i = 0; i < accounts.length() && made.size() < room; i++) {
            JSONObject account = accounts.optJSONObject(i);
            String id = account == null ? "" : account.optString("id", "");
            if (id.isEmpty() || !ids.add(ShareActivity.PREFIX + id))
                continue;
            made.add(shortcut(context, id, account.optString("short", id), account.optString("long", ""), made.size()));
        }
        try {
            if (manager.isRateLimitingActive()) {
                Log.i(DoseAlarms.TAG, "Sharing: Android asks to wait before the addresses change; next time.");
                return;
            }
            // Those of addresses gone: off the share sheet and the long press;
            // one pinned to the home screen says the address is no longer in Sioul.
            List<String> stale = new ArrayList<>();
            for (ShortcutInfo old : manager.getDynamicShortcuts())
                if (old.getId().startsWith(ShareActivity.PREFIX) && !ids.contains(old.getId()))
                    stale.add(old.getId());
            if (!stale.isEmpty()) {
                manager.removeDynamicShortcuts(stale);
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R)
                    manager.removeLongLivedShortcuts(stale);
            }
            List<String> pinnedGone = new ArrayList<>();
            List<String> pinnedBack = new ArrayList<>();
            for (ShortcutInfo pinned : manager.getPinnedShortcuts()) {
                if (!pinned.getId().startsWith(ShareActivity.PREFIX))
                    continue;
                if (!ids.contains(pinned.getId()))
                    pinnedGone.add(pinned.getId());
                else if (!pinned.isEnabled())
                    pinnedBack.add(pinned.getId());
            }
            if (!pinnedGone.isEmpty())
                manager.disableShortcuts(pinnedGone, gone);
            if (!pinnedBack.isEmpty())
                manager.enableShortcuts(pinnedBack);
            // Those kept, renamed or new: made again with their rank.
            List<ShortcutInfo> known = new ArrayList<>();
            List<ShortcutInfo> fresh = new ArrayList<>();
            Set<String> dynamic = new HashSet<>();
            for (ShortcutInfo old : manager.getDynamicShortcuts())
                dynamic.add(old.getId());
            for (ShortcutInfo one : made)
                (dynamic.contains(one.getId()) ? known : fresh).add(one);
            if (!known.isEmpty())
                manager.updateShortcuts(known);
            if (!fresh.isEmpty())
                manager.addDynamicShortcuts(fresh);
            kept.edit().putString(GIVEN, json).apply();
            Log.i(DoseAlarms.TAG, "Sharing: " + made.size() + " addresses offered in the share sheet.");
        } catch (IllegalArgumentException | IllegalStateException e) {
            Log.e(DoseAlarms.TAG, "Sharing: the addresses could not be offered: " + e);
        }
    }

    /** Whether shortcuts given before are gone (Sioul's data cleared, a restore): then given again. */
    private static boolean missing(ShortcutManager manager, String json)
    {
        try {
            JSONArray accounts = new JSONObject(json).optJSONArray("accounts");
            int wanted = accounts == null ? 0 : Math.min(accounts.length(), Math.max(0, manager.getMaxShortcutCountPerActivity() - manager.getManifestShortcuts().size()));
            int have = 0;
            for (ShortcutInfo one : manager.getDynamicShortcuts())
                if (one.getId().startsWith(ShareActivity.PREFIX))
                    have++;
            return have != wanted;
        } catch (JSONException e) {
            return true;
        }
    }

    /**
     * One address's shortcut: under its icon in the share sheet, the address;
     * at a long press on Sioul's icon, "Write from …", which opens a new
     * message from it (ShareActivity, as a mailto: link would).
     */
    private static ShortcutInfo shortcut(Context context, String id, String label, String longLabel, int rank)
    {
        Intent write = new Intent(Intent.ACTION_SENDTO, Uri.parse("mailto:"))
            .setComponent(new ComponentName(context, ShareActivity.class))
            .putExtra(ShareActivity.ACCOUNT, id);
        ShortcutInfo.Builder builder = new ShortcutInfo.Builder(context, ShareActivity.PREFIX + id)
            .setShortLabel(label.isEmpty() ? id : label)
            .setIcon(Icon.createWithResource(context, R.mipmap.sioul_mail_shortcut))
            .setIntent(write)
            .setCategories(Collections.singleton(CATEGORY))
            .setRank(rank);
        if (!longLabel.isEmpty())
            builder.setLongLabel(longLabel);
        // Kept by the share sheet between its showings (Android 11 and later).
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R)
            builder.setLongLived(true);
        return builder.build();
    }
}
