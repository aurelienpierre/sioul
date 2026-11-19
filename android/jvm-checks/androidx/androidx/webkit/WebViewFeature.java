// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package androidx.webkit;

/**
 * A stand-in for androidx.webkit's WebViewFeature, for android/jvm-checks
 * alone: Sioul's Java (SiteWeb.java) compiles against it on a computer,
 * where the library, an Android archive, is not at hand; it is never part of
 * Sioul's program, which takes the real one through Gradle
 * (android/package/build.gradle). The names below are the library's own,
 * and so are their values: each holds its name (read with javap from
 * webkit-1.18.0's classes, 19 November 2026). On a computer no web view
 * answers: every feature reads as absent.
 */
public final class WebViewFeature
{
    private WebViewFeature()
    {
    }

    /** A script before a page's own, in every frame. */
    public static final String DOCUMENT_START_SCRIPT = "DOCUMENT_START_SCRIPT";
    /** A channel from a page's script back to the application. */
    public static final String WEB_MESSAGE_LISTENER = "WEB_MESSAGE_LISTENER";
    /** A web view's sound silenced. */
    public static final String MUTE_AUDIO = "MUTE_AUDIO";
    /** Pages darkened that have no dark style of their own. */
    public static final String ALGORITHMIC_DARKENING = "ALGORITHMIC_DARKENING";
    /** Profiles, each with its own store. */
    public static final String MULTI_PROFILE = "MULTI_PROFILE";

    /**
     * Whether this device's web view has a feature: never, on a computer.
     *
     * @param feature its name
     * @return false
     */
    public static boolean isFeatureSupported(String feature)
    {
        return false;
    }
}
