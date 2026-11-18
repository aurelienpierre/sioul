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
 * starred here, and opens their contact for you to star it.
 *
 * Starred people both ways (the owner's decision of 18 November 2026, which
 * takes back the rule of 6 October that Sioul never writes a star): once you
 * turn it on and Android allows it (WRITE_CONTACTS), Rust looks at what this
 * phone stars ({@link #look}) and decides (crates/sioul-core/src/stars.rs);
 * {@link #write} then sets or clears the star of the contacts it names, and
 * nothing else: one column, {@code Contacts.STARRED}, of a contact that
 * exists. Sioul never creates, edits nor deletes a contact; "Add to
 * contacts" opens the Contacts app's own form, filled, which you save.
 */
final class DndContacts
{
    private DndContacts() {}

    static boolean allowed(Context context)
    {
        return context.checkSelfPermission(Manifest.permission.READ_CONTACTS) == PackageManager.PERMISSION_GRANTED;
    }

    /** Whether Sioul may read contacts and write their stars. */
    static boolean writable(Context context)
    {
        return allowed(context) && context.checkSelfPermission(Manifest.permission.WRITE_CONTACTS) == PackageManager.PERMISSION_GRANTED;
    }

    /**
     * {people: [{id, phones}]} → {permission, write, contacts: [{id, name,
     * phones, emails, starred}]}: every contact this phone stars, and every
     * contact holding a number of someone on the list, each once, by its
     * lookup key (its id here, stable through Android's merging of contacts).
     */
    static JSONObject look(Context context, JSONObject asked) throws JSONException
    {
        JSONObject answer = new JSONObject();
        answer.put("permission", allowed(context));
        answer.put("write", writable(context));
        if (!allowed(context))
            return answer;
        android.content.ContentResolver resolver = context.getContentResolver();
        java.util.Map<String, JSONObject> found = new java.util.LinkedHashMap<>();
        String[] columns = { ContactsContract.Contacts._ID, ContactsContract.Contacts.LOOKUP_KEY, ContactsContract.Contacts.DISPLAY_NAME, ContactsContract.Contacts.STARRED };
        try (Cursor rows = resolver.query(ContactsContract.Contacts.CONTENT_URI, columns, ContactsContract.Contacts.STARRED + "=1", null, null)) {
            while (rows != null && rows.moveToNext())
                seen(resolver, found, rows.getLong(0), rows.getString(1), rows.getString(2), rows.getInt(3) != 0);
        } catch (RuntimeException e) {
            Log.w(DoseAlarms.TAG, "Stars: the starred contacts not read: " + e);
        }
        JSONArray people = asked.optJSONArray("people");
        for (int i = 0; people != null && i < people.length(); i++) {
            JSONArray phones = people.getJSONObject(i).optJSONArray("phones");
            for (int j = 0; phones != null && j < phones.length(); j++) {
                String number = phones.optString(j, "").trim();
                if (number.isEmpty())
                    continue;
                Uri lookup = Uri.withAppendedPath(ContactsContract.PhoneLookup.CONTENT_FILTER_URI, Uri.encode(number));
                String[] by = { ContactsContract.PhoneLookup._ID, ContactsContract.PhoneLookup.LOOKUP_KEY, ContactsContract.PhoneLookup.DISPLAY_NAME, ContactsContract.PhoneLookup.STARRED };
                try (Cursor rows = resolver.query(lookup, by, null, null, null)) {
                    while (rows != null && rows.moveToNext())
                        seen(resolver, found, rows.getLong(0), rows.getString(1), rows.getString(2), rows.getInt(3) != 0);
                } catch (RuntimeException e) {
                    Log.w(DoseAlarms.TAG, "Stars: a number not looked up: " + e);
                }
            }
        }
        JSONArray contacts = new JSONArray();
        for (JSONObject contact : found.values())
            contacts.put(contact);
        answer.put("contacts", contacts);
        return answer;
    }

    /** A contact found, with its numbers and addresses, once. */
    private static void seen(android.content.ContentResolver resolver, java.util.Map<String, JSONObject> found, long id, String key, String name, boolean starred) throws JSONException
    {
        if (key == null || key.isEmpty() || found.containsKey(key))
            return;
        JSONObject contact = new JSONObject();
        contact.put("id", key);
        contact.put("name", name == null ? "" : name);
        contact.put("starred", starred);
        contact.put("phones", column(resolver, ContactsContract.CommonDataKinds.Phone.CONTENT_URI, ContactsContract.CommonDataKinds.Phone.NUMBER, ContactsContract.CommonDataKinds.Phone.CONTACT_ID, id));
        contact.put("emails", column(resolver, ContactsContract.CommonDataKinds.Email.CONTENT_URI, ContactsContract.CommonDataKinds.Email.ADDRESS, ContactsContract.CommonDataKinds.Email.CONTACT_ID, id));
        found.put(key, contact);
    }

    /** One column of a contact's data rows (its numbers, its addresses). */
    private static JSONArray column(android.content.ContentResolver resolver, Uri table, String column, String contactId, long id)
    {
        JSONArray values = new JSONArray();
        try (Cursor rows = resolver.query(table, new String[] { column }, contactId + "=?", new String[] { Long.toString(id) }, null)) {
            while (rows != null && rows.moveToNext()) {
                String value = rows.getString(0);
                if (value != null && !value.trim().isEmpty())
                    values.put(value.trim());
            }
        } catch (RuntimeException e) {
            Log.w(DoseAlarms.TAG, "Stars: a contact's data not read: " + e);
        }
        return values;
    }

    /**
     * {star: [lookup keys], unstar: [lookup keys]} → {written: n, failed:
     * [keys]}: the star of each contact set or cleared, the one column
     * {@code Contacts.STARRED} of a contact that exists, found by its lookup
     * key; nothing else is ever written. Without the permission, every one
     * fails, nothing written.
     */
    static JSONObject write(Context context, JSONObject asked) throws JSONException
    {
        JSONObject answer = new JSONObject();
        JSONArray failed = new JSONArray();
        int written = 0;
        boolean may = writable(context);
        android.content.ContentResolver resolver = context.getContentResolver();
        for (String list : new String[] { "star", "unstar" }) {
            JSONArray keys = asked.optJSONArray(list);
            for (int i = 0; keys != null && i < keys.length(); i++) {
                String key = keys.optString(i, "");
                if (may && starred(resolver, key, "star".equals(list)))
                    written++;
                else
                    failed.put(key);
            }
        }
        answer.put("written", written);
        answer.put("failed", failed);
        return answer;
    }

    /** One contact's star set or cleared; false when it is not found or Android refuses. */
    private static boolean starred(android.content.ContentResolver resolver, String key, boolean on)
    {
        if (key.isEmpty())
            return false;
        try {
            Uri contact = ContactsContract.Contacts.lookupContact(resolver, Uri.withAppendedPath(ContactsContract.Contacts.CONTENT_LOOKUP_URI, Uri.encode(key)));
            if (contact == null)
                return false;
            ContentValues star = new ContentValues();
            star.put(ContactsContract.Contacts.STARRED, on ? 1 : 0);
            return resolver.update(contact, star, null, null) > 0;
        } catch (RuntimeException e) {
            Log.w(DoseAlarms.TAG, "Stars: a star not written: " + e);
            return false;
        }
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
