// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.content.Intent;
import android.service.quicksettings.Tile;
import android.service.quicksettings.TileService;

/**
 * "Do not disturb (Sioul)" among the quick settings: Sioul's switch, on every
 * device (docs/do-not-disturb.md), pressed without opening Sioul. Lit while
 * Sioul's do-not-disturb holds on this phone, as Rust said at its last apply
 * (PauseMode.sioulOn); a tap turns it off when it holds, else on until
 * turned off, through DndReceiver in Sioul's own process. Beside Android's
 * own tile, which Sioul hears too: both say the same once Sioul applied.
 */
public final class DndTile extends TileService
{
    @Override
    public void onStartListening()
    {
        show(PauseMode.sioulOn(this));
    }

    @Override
    public void onTileAdded()
    {
        show(PauseMode.sioulOn(this));
    }

    @Override
    public void onClick()
    {
        // Shown at once as asked; Rust's apply says the rest (PauseMode.flags).
        show(!PauseMode.sioulOn(this));
        sendBroadcast(new Intent(this, DndReceiver.class).setAction(DndReceiver.TOGGLE));
    }

    private void show(boolean on)
    {
        Tile tile = getQsTile();
        if (tile == null)
            return;
        tile.setState(on ? Tile.STATE_ACTIVE : Tile.STATE_INACTIVE);
        tile.updateTile();
    }
}
