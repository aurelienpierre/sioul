// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.Manifest;
import android.content.ActivityNotFoundException;
import android.content.ContentValues;
import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.database.Cursor;
import android.net.Uri;
import android.provider.ContactsContract;
import android.util.Log;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

import java.util.ArrayList;

/**
 * The people who may reach you during do-not-disturb, as this phone's
 * contacts have them (crates/sioul-app/src/everywhere.rs): Android lets only
 * starred contacts through a mode, so Sioul says who on the list is not
 * starred here, and opens their contact for you to star it. Reading only
 * (READ_CONTACTS): Sioul never writes a star nor a contact; "Add to
 * contacts" opens the Contacts app's own form, filled, which you save.
 */
final class DndContacts
{
    private DndContacts() {}

    static boolean allowed(Context context)
    {
        return context.checkSelfPermission(Manifest.permission.READ_CONTACTS) == PackageManager.PERMISSION_GRANTED;
    }

    /**
     * {people: [{id, phones}]} → {permission, people: [{id, state, contact}]}:
     * "starred" when a contact with one of their numbers is starred,
     * "not-starred" when one has a number but none is starred (its address
     * to open), "unknown" when no contact has their numbers.
     */
    static JSONObject stars(Context context, JSONObject asked) throws JSONException
    {
        JSONObject answer = new JSONObject();
        if (!allowed(context)) {
            answer.put("permission", false);
            return answer;
        }
        answer.put("permission", true);
        JSONArray found = new JSONArray();
        JSONArray people = asked.optJSONArray("people");
        for (int i = 0; people != null && i < people.length(); i++) {
            JSONObject person = people.getJSONObject(i);
            JSONArray phones = person.optJSONArray("phones");
            String state = "unknown";
            String contact = "";
            for (int j = 0; phones != null && j < phones.length() && !"starred".equals(state); j++) {
                String number = phones.optString(j, "").trim();
                if (number.isEmpty())
                    continue;
                Uri lookup = Uri.withAppendedPath(ContactsContract.PhoneLookup.CONTENT_FILTER_URI, Uri.encode(number));
                String[] columns = { ContactsContract.PhoneLookup._ID, ContactsContract.PhoneLookup.LOOKUP_KEY, ContactsContract.PhoneLookup.STARRED };
                try (Cursor rows = context.getContentResolver().query(lookup, columns, null, null, null)) {
                    while (rows != null && rows.moveToNext()) {
                        long id = rows.getLong(0);
                        String key = rows.getString(1);
                        boolean starred = rows.getInt(2) != 0;
                        if (starred) {
                            state = "starred";
                            contact = "";
                            break;
                        }
                        state = "not-starred";
                        if (contact.isEmpty() && key != null)
                            contact = ContactsContract.Contacts.getLookupUri(id, key).toString();
                    }
                } catch (RuntimeException e) {
                    Log.w(DoseAlarms.TAG, "Do not disturb: a number not looked up: " + e);
                }
            }
            JSONObject seen = new JSONObject();
            seen.put("id", person.optString("id"));
            seen.put("state", state);
            seen.put("contact", contact);
            found.put(seen);
        }
        answer.put("people", found);
        return answer;
    }

    /** A contact opened in the Contacts app, where its star is. */
    static boolean open(Context context, String uri)
    {
        if (uri.isEmpty() || !uri.startsWith(ContactsContract.Contacts.CONTENT_LOOKUP_URI.toString()))
            return false;
        try {
            context.startActivity(new Intent(Intent.ACTION_VIEW, Uri.parse(uri)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
            return true;
        } catch (ActivityNotFoundException | SecurityException e) {
            Log.w(DoseAlarms.TAG, "Do not disturb: no app to open a contact: " + e);
            return false;
        }
    }

    /** The Contacts app's form for a new contact, filled: {name, phones, emails}. Saved by you, or not. */
    static boolean add(Context context, JSONObject person)
    {
        Intent form = new Intent(ContactsContract.Intents.Insert.ACTION).setType(ContactsContract.RawContacts.CONTENT_TYPE);
        String name = person.optString("name", "");
        if (!name.isEmpty())
            form.putExtra(ContactsContract.Intents.Insert.NAME, name);
        ArrayList<ContentValues> data = new ArrayList<>();
        JSONArray phones = person.optJSONArray("phones");
        for (int i = 0; phones != null && i < phones.length(); i++) {
            ContentValues row = new ContentValues();
            row.put(ContactsContract.Data.MIMETYPE, ContactsContract.CommonDataKinds.Phone.CONTENT_ITEM_TYPE);
            row.put(ContactsContract.CommonDataKinds.Phone.NUMBER, phones.optString(i));
            row.put(ContactsContract.CommonDataKinds.Phone.TYPE, ContactsContract.CommonDataKinds.Phone.TYPE_MOBILE);
            data.add(row);
        }
        JSONArray emails = person.optJSONArray("emails");
        for (int i = 0; emails != null && i < emails.length(); i++) {
            ContentValues row = new ContentValues();
            row.put(ContactsContract.Data.MIMETYPE, ContactsContract.CommonDataKinds.Email.CONTENT_ITEM_TYPE);
            row.put(ContactsContract.CommonDataKinds.Email.ADDRESS, emails.optString(i));
            row.put(ContactsContract.CommonDataKinds.Email.TYPE, ContactsContract.CommonDataKinds.Email.TYPE_OTHER);
            data.add(row);
        }
        form.putParcelableArrayListExtra(ContactsContract.Intents.Insert.DATA, data);
        try {
            context.startActivity(form.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
            return true;
        } catch (ActivityNotFoundException | SecurityException e) {
            Log.w(DoseAlarms.TAG, "Do not disturb: no Contacts app to add a contact: " + e);
            return false;
        }
    }
}
