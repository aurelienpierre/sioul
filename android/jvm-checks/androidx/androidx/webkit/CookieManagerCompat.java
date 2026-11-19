// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package androidx.webkit;

import android.webkit.CookieManager;

import java.util.List;

/**
 * A stand-in for androidx.webkit's CookieManagerCompat, for android/jvm-checks
 * alone (as {@link WebViewFeature}); never part of Sioul's program. Its
 * signature is the library's (webkit-1.18.0, read with javap, 19 November
 * 2026).
 */
public final class CookieManagerCompat
{
    private CookieManagerCompat()
    {
    }

    /**
     * The cookies for an address with their attributes, in the library:
     * each as a Set-Cookie header ("name=value; domain=.example.org; path=/").
     *
     * @param cookieManager the web view's cookies
     * @param url the address
     * @return the cookies
     */
    public static List<String> getCookieInfo(CookieManager cookieManager, String url)
    {
        throw new UnsupportedOperationException("no web view on a computer");
    }
}
