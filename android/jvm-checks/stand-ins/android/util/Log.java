// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package android.util;

import java.util.ArrayList;
import java.util.List;

/**
 * Android's log, for AOSP's MMS parser on a computer's JVM (MmsCheck): what
 * it says at the error level is kept, for the check to see; the rest is
 * passed over. android.jar's own Log is a stub that throws.
 */
public final class Log
{
    /** What the parser said at the error level, since the check last cleared it. */
    public static final List<String> ERRORS = new ArrayList<>();

    private Log() {}

    public static int v(String tag, String message) { return 0; }

    public static int v(String tag, String message, Throwable thrown) { return 0; }

    public static int d(String tag, String message) { return 0; }

    public static int i(String tag, String message) { return 0; }

    public static int w(String tag, String message) { return 0; }

    public static int e(String tag, String message)
    {
        ERRORS.add(tag + ": " + message);
        return 0;
    }

    public static int e(String tag, String message, Throwable thrown)
    {
        ERRORS.add(tag + ": " + message + " (" + thrown + ")");
        return 0;
    }
}
