// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package androidx.webkit;

import android.webkit.WebStorage;

/**
 * A stand-in for androidx.webkit's WebStorageCompat, for android/jvm-checks
 * alone (as {@link WebViewFeature}): Sioul's Java compiles against it on a
 * computer; never part of Sioul's program. Its signature is the library's
 * (webkit-1.18.0, read with javap, 19 November 2026); on a computer it
 * deletes nothing, as no web view answers.
 */
public final class WebStorageCompat
{
    private WebStorageCompat()
    {
    }

    /**
     * Everything stored for a site, in the library: cookies, the cache and
     * the pages' own storage, for the site of {@code site}.
     *
     * @param instance the web view's storage
     * @param site a domain or an address
     * @param doneCallback called once deleted
     * @return the site deleted for
     */
    public static String deleteBrowsingDataForSite(WebStorage instance, String site, Runnable doneCallback)
    {
        throw new UnsupportedOperationException("no web view on a computer");
    }
}
