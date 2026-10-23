// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import java.io.IOException;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

/**
 * Ccid.java, on the JVM (android/jvm-checks/run.sh), against a stand-in
 * reader: the messages' bytes (type, length, slot, sequence), packets of the
 * endpoint's size with a zero-length packet after a message that fills its
 * last one, an answer in several transfers, a key waiting for a touch (time
 * extensions, then its answer), a key that keeps waiting too long, an answer
 * to another message, a failure of the reader's, a key unplugged (nothing
 * comes), and the messages wiped once sent.
 */
public final class CcidCheck
{
    static int failed = 0;
    static int checked = 0;

    static void expect(String what, boolean holds)
    {
        checked++;
        if (!holds) {
            failed++;
            System.out.println("FAIL " + what);
        }
    }

    /** A reader as its bulk endpoints see it: what was sent, the answers queued, the buffers it was handed. */
    static final class Reader implements Ccid.Pipe
    {
        final List<byte[]> sent = new ArrayList<>();
        final List<byte[]> answers = new ArrayList<>();
        final List<byte[]> handed = new ArrayList<>();
        boolean gone;
        int receives;

        @Override
        public int send(byte[] data, int offset, int length, int timeoutMs)
        {
            if (gone)
                return -1;
            sent.add(Arrays.copyOfRange(data, offset, offset + length));
            if (!handed.contains(data))
                handed.add(data);
            return length;
        }

        @Override
        public int receive(byte[] into, int timeoutMs)
        {
            receives++;
            if (gone || answers.isEmpty())
                return -1;
            byte[] next = answers.remove(0);
            // A transfer's time without a word: the key waits, for a touch.
            if (next == null)
                return -1;
            int length = Math.min(into.length, next.length);
            System.arraycopy(next, 0, into, 0, length);
            return length;
        }

        @Override
        public boolean present()
        {
            return !gone;
        }
    }

    /** A reader's answer of {@code type} to message {@code sequence}, with its status and data. */
    static byte[] answer(int type, int sequence, int status, int error, byte[] data)
    {
        byte[] message = new byte[10 + data.length];
        message[0] = (byte) type;
        message[1] = (byte) data.length;
        message[2] = (byte) (data.length >> 8);
        message[6] = (byte) sequence;
        message[7] = (byte) status;
        message[8] = (byte) error;
        System.arraycopy(data, 0, message, 10, data.length);
        return message;
    }

    static byte[] bytes(int length, int value)
    {
        byte[] out = new byte[length];
        Arrays.fill(out, (byte) value);
        return out;
    }

    static boolean zeros(byte[] data)
    {
        for (byte b : data)
            if (b != 0)
                return false;
        return true;
    }

    public static void main(String[] args) throws IOException
    {
        // Power on, then an APDU, with their sequence numbers.
        Reader reader = new Reader();
        Ccid ccid = new Ccid(reader, 64);
        reader.answers.add(answer(0x80, 0, 0, 0, new byte[] { 0x3B, (byte) 0xFD }));
        byte[] atr = ccid.powerOn();
        expect("the ATR read", Arrays.equals(atr, new byte[] { 0x3B, (byte) 0xFD }));
        expect("IccPowerOn sent: type 0x62, no data, slot 0, sequence 0", Arrays.equals(reader.sent.get(0), new byte[] { 0x62, 0, 0, 0, 0, 0, 0, 0, 0, 0 }));
        byte[] select = { 0x00, (byte) 0xA4, 0x04, 0x00, 0x06, (byte) 0xD2, 0x76, 0x00, 0x01, 0x24, 0x01 };
        reader.answers.add(answer(0x80, 1, 0, 0, new byte[] { (byte) 0x90, 0x00 }));
        byte[] said = ccid.transmit(select);
        expect("the APDU's answer", Arrays.equals(said, new byte[] { (byte) 0x90, 0x00 }));
        byte[] block = reader.sent.get(1);
        expect("XfrBlock: type 0x6F, its length, slot 0, sequence 1", block.length == 10 + select.length && block[0] == 0x6F && block[1] == select.length && block[5] == 0 && block[6] == 1);
        expect("XfrBlock carries the APDU", Arrays.equals(Arrays.copyOfRange(block, 10, block.length), select));
        boolean wiped = true;
        for (byte[] handed : reader.handed)
            wiped &= zeros(handed);
        expect("each message wiped once sent", wiped);

        // A message that fills its packets exactly ends with a zero-length packet; a longer one is split.
        reader = new Reader();
        ccid = new Ccid(reader, 64);
        reader.answers.add(answer(0x80, 0, 0, 0, new byte[] { (byte) 0x90, 0x00 }));
        ccid.transmit(bytes(54, 7));
        expect("64 bytes: one packet, then a zero-length one", reader.sent.size() == 2 && reader.sent.get(0).length == 64 && reader.sent.get(1).length == 0);
        reader.answers.add(answer(0x80, 1, 0, 0, new byte[] { (byte) 0x90, 0x00 }));
        ccid.transmit(bytes(150, 7));
        expect("160 bytes: 64, 64, 32, no zero-length packet", reader.sent.size() == 5 && reader.sent.get(2).length == 64 && reader.sent.get(3).length == 64 && reader.sent.get(4).length == 32);

        // An answer in several transfers, and a zero-length packet left from the one before.
        reader = new Reader();
        ccid = new Ccid(reader, 64);
        byte[] long_answer = answer(0x80, 0, 0, 0, bytes(300, 5));
        reader.answers.add(new byte[0]);
        reader.answers.add(Arrays.copyOfRange(long_answer, 0, 64));
        reader.answers.add(Arrays.copyOfRange(long_answer, 64, 200));
        reader.answers.add(Arrays.copyOfRange(long_answer, 200, long_answer.length));
        said = ccid.transmit(new byte[] { 0x00, (byte) 0xCA, 0x00, 0x6E, 0x00 });
        expect("an answer read across transfers, a leftover empty packet passed over", said.length == 300 && said[0] == 5 && said[299] == 5);

        // A key waiting for a touch: time extensions, then its answer.
        reader = new Reader();
        ccid = new Ccid(reader, 64);
        for (int i = 0; i < 5; i++)
            reader.answers.add(answer(0x80, 0, 0x80, 1, new byte[0]));
        reader.answers.add(answer(0x80, 0, 0, 0, new byte[] { 1, 2, (byte) 0x90, 0x00 }));
        said = ccid.transmit(new byte[] { 0x00, 0x2A, (byte) 0x9E, (byte) 0x9A, 0x01, 0x07 });
        expect("time extensions waited through, the answer read", Arrays.equals(said, new byte[] { 1, 2, (byte) 0x90, 0x00 }));

        // A key silent while it waits for a touch, a second per transfer, then its answer (review of 23 October 2026, F9).
        reader = new Reader();
        ccid = new Ccid(reader, 64);
        for (int i = 0; i < 4; i++)
            reader.answers.add(null);
        reader.answers.add(answer(0x80, 0, 0x80, 1, new byte[0]));
        reader.answers.add(null);
        reader.answers.add(answer(0x80, 0, 0, 0, new byte[] { 3, (byte) 0x90, 0x00 }));
        said = ccid.transmit(new byte[] { 0x00, 0x2A, (byte) 0x80, (byte) 0x86, 0x01, 0x00 });
        expect("silent transfers waited through within the touch's time", Arrays.equals(said, new byte[] { 3, (byte) 0x90, 0x00 }));
        expect("the touch's wait covers a YubiKey's 15 seconds, with a margin", Ccid.WAIT_MS >= 20000);

        // What goes wrong: an answer to another message, a reader's failure, a key unplugged.
        String[] cases = { "another message", "a failure", "unplugged", "another type" };
        for (String which : cases) {
            reader = new Reader();
            ccid = new Ccid(reader, 64);
            switch (which) {
                case "another message":
                    reader.answers.add(answer(0x80, 9, 0, 0, new byte[] { (byte) 0x90, 0x00 }));
                    break;
                case "a failure":
                    reader.answers.add(answer(0x80, 0, 0x42, 0xFE, new byte[0]));
                    break;
                case "unplugged":
                    reader.gone = true;
                    break;
                default:
                    reader.answers.add(answer(0x81, 0, 0, 0, new byte[0]));
            }
            String failure = null;
            try {
                ccid.transmit(new byte[] { 0x00, 0x20, 0x00, (byte) 0x82, 0x06, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36 });
            } catch (Ccid.Failure e) {
                failure = e.getMessage();
            }
            expect(which + ": said as a failure", failure != null);
            if (which.equals("unplugged"))
                expect("unplugged: said at once, not after the touch's wait", reader.receives <= 1);
            expect(which + ": the failure says no byte of the command", failure == null || !failure.contains("123456") && !failure.contains("49"));
        }

        // Power off answered by a slot status.
        reader = new Reader();
        ccid = new Ccid(reader, 64);
        reader.answers.add(answer(0x81, 0, 0, 0, new byte[0]));
        ccid.powerOff();
        expect("IccPowerOff sent: type 0x63", reader.sent.get(0)[0] == 0x63);

        System.out.println("CcidCheck: " + (checked - failed) + " of " + checked + " held");
        if (failed > 0)
            System.exit(1);
    }
}
