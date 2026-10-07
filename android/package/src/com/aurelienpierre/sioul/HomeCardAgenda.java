// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

/**
 * Sioul's agenda card on the home screen: the coming events alone, day by
 * day, as the full card lists them (HomeCardRows). HomeCard draws it, with
 * the others, from the same file.
 */
public final class HomeCardAgenda extends HomeCard
{
    @Override
    String kind()
    {
        return AGENDA;
    }
}
