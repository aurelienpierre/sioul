// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package android.compat.annotation;

import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;

/** AOSP's mark on hidden API that apps once used, as AOSP's MMS classes carry it: nothing reads it here. */
@Retention(RetentionPolicy.CLASS)
public @interface UnsupportedAppUsage
{
    int maxTargetSdk() default Integer.MAX_VALUE;

    long trackingBug() default 0;

    String implicitMember() default "";

    String publicAlternatives() default "";

    String expectedSignature() default "";

    String overrideSourcePosition() default "";
}
