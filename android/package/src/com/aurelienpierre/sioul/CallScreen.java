// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.telecom.Call;
import android.telecom.CallScreeningService;
import android.util.Log;

/**
 * Android's question about each call, to Sioul as the "Caller ID &amp; spam app"
 * (docs/android.md, "Calls"): answered from the table Rust wrote ahead
 * (Calls.decide), in a process of its own (":calls") that never loads
 * Sioul's library or Qt, so that Telecom, which holds the ringing until it
 * hears back (five seconds at most), waits milliseconds. A call is let ring,
 * or refused plainly: declined as by hand, so the network sends it to your
 * voicemail; no missed-call notification for it; still in the phone's call
 * history, with Sioul's name; listed for Sioul. Sioul never answers a call,
 * never records one, never listens.
 *
 * Outgoing calls come here too (to name who is called, which Sioul does
 * not): one to an emergency number lets every call ring for a day.
 */
public final class CallScreen extends CallScreeningService
{
    @Override
    public void onScreenCall(Call.Details details)
    {
        Calls.Decision decision;
        try {
            decision = Calls.decide(this, details, System.currentTimeMillis());
        } catch (Throwable e) {
            // Whatever fails, the call rings, as Android would without Sioul.
            Log.e(Calls.TAG, "Calls: no decision, the call rings.", e);
            decision = Calls.Decision.ring("error");
        }
        CallResponse.Builder response = new CallResponse.Builder();
        if (decision.refuse) {
            response.setDisallowCall(true)
                .setRejectCall(true)
                .setSkipNotification(true);
        }
        try {
            respondToCall(details, response.build());
        } catch (RuntimeException e) {
            Log.e(Calls.TAG, "Calls: the answer did not reach Android; the call rings.", e);
            return;
        }
        if (!"outgoing".equals(decision.why))
            Log.i(Calls.TAG, "Calls: " + (decision.refuse ? "refused" : "rings") + " (" + decision.why + (decision.who.isEmpty() ? "" : ", " + decision.who) + (decision.column.isEmpty() ? "" : ", " + decision.column) + ").");
        if (decision.refuse) {
            try {
                Calls.held(this, decision);
            } catch (RuntimeException e) {
                Log.w(Calls.TAG, "Calls: a call held not listed: " + e);
            }
        }
    }
}
