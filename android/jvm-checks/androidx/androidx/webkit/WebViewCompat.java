// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package androidx.webkit;

/**
 * A stand-in for androidx.webkit's WebViewCompat, for android/jvm-checks
 * alone (as {@link WebViewFeature}); never part of Sioul's program. Its
 * signature is the library's (webkit-1.18.0, read with javap, 19 November
 * 2026).
 */
public final class WebViewCompat
{
    private WebViewCompat()
    {
    }

    /**
     * Whether the web view draws pages in a process of its own, in the
     * library; any thread.
     *
     * @return whether it does
     */
    public static boolean isMultiProcessEnabled()
    {
        throw new UnsupportedOperationException("no web view on a computer");
    }
}
