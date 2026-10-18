// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package android.os;

/** The one constant of Android's Build that AOSP's MMS classes name, in their annotations. */
public final class Build
{
    private Build() {}

    /** Android's version codes. */
    public static final class VERSION_CODES
    {
        private VERSION_CODES() {}

        /** Android 11. */
        public static final int R = 30;
    }
}
