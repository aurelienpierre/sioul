// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.PendingIntent;
import android.content.Intent;
import android.os.Build;
import android.service.quicksettings.Tile;
import android.service.quicksettings.TileService;

/**
 * "Pause" among the quick settings: a tap opens Sioul, which starts the pause,
 * or shows it when it is on already; never an end (ending is the pause
 * screen's one button: no exit by accident). Shown active while the pause is
 * on. On a locked phone the tap asks to unlock first: no pause from a pocket.
 */
public final class PauseTile extends TileService
{
    @Override
    public void onStartListening()
    {
        show();
    }

    @Override
    public void onTileAdded()
    {
        show();
    }

    @Override
    public void onClick()
    {
        if (isLocked())
            unlockAndRun(this::open);
        else
            open();
    }

    private void show()
    {
        Tile tile = getQsTile();
        if (tile == null)
            return;
        tile.setState(PauseMode.paused(this) ? Tile.STATE_ACTIVE : Tile.STATE_INACTIVE);
        tile.updateTile();
    }

    /** Sioul brought up through PauseOpener, which keeps the press; the shade closed. */
    @SuppressWarnings("deprecation")
    private void open()
    {
        Intent opener = new Intent(this, PauseOpener.class).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE)
            startActivityAndCollapse(PendingIntent.getActivity(this, 0, opener, PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT));
        else
            startActivityAndCollapse(opener);
    }
}
