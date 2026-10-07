// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

/**
 * Sioul's mail card on the home screen: the Porch's latest messages alone,
 * as the full card lists them (HomeCardRows). HomeCard draws it, with the
 * others, from the same file.
 */
public final class HomeCardMail extends HomeCard
{
    @Override
    String kind()
    {
        return MAIL;
    }
}
