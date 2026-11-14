// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.content.ContentResolver;
import android.content.Context;
import android.content.Intent;
import android.content.SyncAdapterType;
import android.content.pm.ActivityInfo;
import android.content.pm.PackageManager;
import android.content.pm.ResolveInfo;

import org.json.JSONException;
import org.json.JSONObject;

import java.util.ArrayList;
import java.util.List;

/**
 * Whether a calendar app rings an event's own alarms on this device in
 * Sioul's place (the notifications' decision 3; crates/sioul-app/src/
 * eventalarms.rs, `calendar_app`). The rule, without reading the calendars
 * (Sioul asks for no calendar permission): an app answers Android's calendar
 * category (CATEGORY_APP_CALENDAR), and Android has an account type that
 * syncs agendas into it (a sync adapter for the calendars' authority, shown
 * in Android's account settings: DAVx⁵, a Google or a Murena account…).
 * Both, and that calendar app rings them: Sioul says so once, with a button
 * to its notifications in Android's settings. Either missing, Sioul rings
 * an event's own alarms itself, as before, and says that once. Neither can
 * tell whether the agendas synced there are those Sioul shows: the notice
 * says which app, so that a person can see.
 */
final class CalendarApps
{
    /** Android's calendars' authority, the one a sync adapter of agendas names. */
    static final String AUTHORITY = "com.android.calendar";

    private CalendarApps() {}

    /**
     * The calendar app that rings an event's own alarms by the rule above:
     * the phone's default one among `apps` (`preferred`), else the first;
     * "" when there is none, or when no account type syncs agendas
     * (`synced`). Pure, checked on a JVM (android/jvm-checks/NoticesCheck.java).
     *
     * @param apps the packages answering the calendar category
     * @param preferred the default one, or null
     * @param synced whether a sync adapter of agendas is there
     * @return the calendar app's package, or ""
     */
    static String ringing(List<String> apps, String preferred, boolean synced)
    {
        if (!synced || apps.isEmpty())
            return "";
        return preferred != null && apps.contains(preferred) ? preferred : apps.get(0);
    }

    /**
     * The answer for Rust (StepService.call "calendar-app"): {package,
     * label}, "" each when Sioul rings an event's own alarms itself.
     */
    static String find(Context context) throws JSONException
    {
        PackageManager packages = context.getPackageManager();
        Intent calendar = new Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_APP_CALENDAR);
        List<String> apps = new ArrayList<>();
        for (ResolveInfo app : packages.queryIntentActivities(calendar, 0))
            if (app.activityInfo != null && !app.activityInfo.packageName.equals(context.getPackageName()) && !apps.contains(app.activityInfo.packageName))
                apps.add(app.activityInfo.packageName);
        ResolveInfo chosen = packages.resolveActivity(calendar, PackageManager.MATCH_DEFAULT_ONLY);
        ActivityInfo preferred = chosen == null ? null : chosen.activityInfo;
        boolean synced = false;
        for (SyncAdapterType adapter : ContentResolver.getSyncAdapterTypes())
            synced |= AUTHORITY.equals(adapter.authority) && adapter.isUserVisible();
        String ringing = ringing(apps, preferred == null ? null : preferred.packageName, synced);
        String label = "";
        if (!ringing.isEmpty()) {
            try {
                label = packages.getApplicationLabel(packages.getApplicationInfo(ringing, 0)).toString();
            } catch (PackageManager.NameNotFoundException e) {
                label = ringing;
            }
        }
        JSONObject answer = new JSONObject();
        answer.put("package", ringing);
        answer.put("label", label);
        return answer.toString();
    }
}
