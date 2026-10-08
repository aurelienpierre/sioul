// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import java.util.HashSet;
import java.util.Set;

/** Calls.key against what phones::key gives (expected values from the Rust tests). */
public final class KeyCheck
{
    public static void main(String[] args)
    {
        Set<String> zero = new HashSet<>(java.util.Arrays.asList("31", "32", "33", "353", "358", "41", "43", "44", "46", "49", "61", "64", "81", "91", "212", "213", "262", "590", "594", "596"));
        Calls.Region fr = new Calls.Region();
        fr.calling = "33"; fr.trunk = "0"; fr.international = "00"; fr.fewest = 10; fr.most = 10; fr.french = true;
        String[][] overseas = { {"262","262"},{"263","262"},{"269","262"},{"639","262"},{"692","262"},{"693","262"},{"590","590"},{"690","590"},{"691","590"},{"594","594"},{"694","594"},{"596","596"},{"696","596"},{"697","596"} };
        for (String[] o : overseas) fr.overseas.put(o[0], o[1]);
        Calls.Region us = new Calls.Region();
        us.calling = "1"; us.trunk = "1"; us.international = "011"; us.fewest = 10; us.most = 11;
        Calls.Region it = new Calls.Region();
        it.calling = "999"; it.trunk = ""; it.international = "00"; it.fewest = 9; it.most = 10;
        Calls.Region gb = new Calls.Region();
        gb.calling = "44"; gb.trunk = "0"; gb.international = "00"; gb.fewest = 10; gb.most = 11;
        Object[][] cases = {
            { fr, "0199001234", "+33199001234" }, { fr, "01 99 00 12 34", "+33199001234" }, { fr, "+33199001234", "+33199001234" },
            { fr, "0033199001234", "+33199001234" }, { fr, "+330199001234", "+33199001234" }, { fr, "0692123456", "+262692123456" },
            { fr, "0590123456", "+590590123456" }, { fr, "112", "112" }, { fr, "15", "15" }, { fr, "0800112112", "+33800112112" },
            { fr, "08 00 11 21 12", "+33800112112" }, { fr, "+44 20 7946 0018", "+442079460018" }, { fr, "tel:+262639981234", "+262639981234" },
            { fr, "*#06#", "*#06#" }, { fr, "639981234", "639981234" }, { fr, "1-555-SIOUL", "1-555-sioul" }, { fr, "‪+262 6 39 98 12 34‬", "+262639981234" },
            { fr, "06 39 98 12 34", "+262639981234" }, { fr, "01 99 00 12 34", "+33199001234" }, { fr, "‪+33 1 99 00 12 34‬", "+33199001234" },
            { us, "(212) 555-0100", "+12125550100" }, { us, "1 212 555 0100", "+12125550100" }, { us, "011 262 6 39 98 12 34", "+262639981234" }, { us, "911", "911" },
            { it, "639 98 12 34", "+999639981234" }, { it, "0639 98 12 34", "+9990639981234" }, { it, "+999 0639 98 12 34", "+9990639981234" }, { it, "112", "112" },
            { gb, "07700 900123", "+447700900123" }, { gb, "020 7946 0018", "+442079460018" }, { gb, "0044 7700 900123", "+447700900123" }, { gb, "999", "999" },
            { null, "06 39 98 12 34", "0639981234" }, { fr, "06 39 98 12 34", "+262639981234" }, { null, "+33 05 36 49 12 34", "+33536491234" },
        };
        int failed = 0;
        for (Object[] c : cases) {
            String got = Calls.key((String) c[1], (Calls.Region) c[0], zero);
            if (!got.equals(c[2])) {
                failed++;
                System.out.println("FAIL " + c[1] + " -> " + got + " (expected " + c[2] + ")");
            }
        }
        System.out.println(failed == 0 ? "all " + cases.length + " keys agree" : failed + " failed");
        System.exit(failed == 0 ? 0 : 1);
    }
}
