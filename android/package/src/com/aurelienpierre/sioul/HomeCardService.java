// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.content.Intent;
import android.widget.RemoteViewsService;

/**
 * The home-screen cards' lists (HomeCard): Android's widget service binds here
 * for the rows of a card's list, each card its own factory (HomeCardRows),
 * the card's kind in its address. Bound by the system alone (its permission,
 * BIND_REMOTEVIEWS, in the manifest); not exported. In Sioul's own process,
 * Java alone: no Rust, no Qt.
 */
public final class HomeCardService extends RemoteViewsService
{
    /** The card's kind (HomeCard.ALL, MAIL, AGENDA). */
    static final String KIND = "com.aurelienpierre.sioul.extra.CARD_KIND";

    @Override
    public RemoteViewsFactory onGetViewFactory(Intent intent)
    {
        String kind = intent == null ? null : intent.getStringExtra(KIND);
        return new HomeCardRows(getApplicationContext(), kind == null ? HomeCard.ALL : kind);
    }
}
