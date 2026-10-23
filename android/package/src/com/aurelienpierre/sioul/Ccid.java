// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import java.io.IOException;
import java.util.Arrays;

/**
 * A smart card's messages over USB (CCID, the USB Integrated Circuit(s) Cards
 * Interface Device class, revision 1.1, §6), for your security key plugged
 * into the phone (SecurityKeyLink.java). Pure Java over a {@link Pipe}, the
 * key's two bulk endpoints, so that android/jvm-checks runs it against a
 * stand-in reader.
 *
 * <ul>
 * <li><b>Messages</b>: PC_to_RDR_IccPowerOn (0x62) answered by the card's ATR,
 * PC_to_RDR_XfrBlock (0x6F) carrying one APDU, PC_to_RDR_IccPowerOff (0x63);
 * each with a sequence number its answer must repeat, slot 0.</li>
 * <li><b>Sent</b> in packets of the endpoint's size, with a zero-length packet
 * when a message fills its last packet exactly, so that the key knows it
 * ended.</li>
 * <li><b>A key waiting for a touch</b> says so with a time extension (status
 * 0x80), or stays silent: Sioul reads again, as long as {@link #WAIT_MS} at
 * most (a YubiKey waits 15 seconds for a touch), while the band asks for the
 * touch, so that no touch setting breaks the operation; a key unplugged
 * meanwhile fails at once. Any other failure is said as a code, never
 * bytes.</li>
 * <li><b>Nothing kept</b>: each message's buffer is wiped once sent or
 * read.</li>
 * </ul>
 */
final class Ccid {
    /** The key's two bulk endpoints. */
    interface Pipe {
        /** Sends {@code length} bytes from {@code offset}: how many went, or a negative number. */
        int send(byte[] data, int offset, int length, int timeoutMs);

        /** Receives one transfer into {@code into}: how many came, or a negative number (nothing within the time, or a failure). */
        int receive(byte[] into, int timeoutMs);

        /** Whether the key is still plugged in. */
        boolean present();
    }

    /** The key's answer that said nothing sensible, or a failure of its own: a code, never its bytes. */
    static final class Failure extends IOException {
        private static final long serialVersionUID = 1L;

        Failure(String code) {
            super(code);
        }
    }

    /** One bulk transfer at most. */
    static final int TRANSFER_MS = 1000;
    /** How long a key may keep Sioul waiting for an answer, a touch included: a YubiKey gives up waiting for one after 15 seconds; ten more as a margin. */
    static final int WAIT_MS = 25000;
    /** A message's header. */
    static final int HEADER = 10;
    /** The longest answer read: an extended APDU's, with room. */
    static final int LONGEST = 65538 + HEADER;

    static final byte POWER_ON = 0x62;
    static final byte POWER_OFF = 0x63;
    static final byte XFR_BLOCK = 0x6F;
    static final byte DATA_BLOCK = (byte) 0x80;
    static final byte SLOT_STATUS = (byte) 0x81;

    private final Pipe pipe;
    private final int packet;
    private byte sequence;

    /** Messages over {@code pipe}, whose endpoints take {@code packet} bytes a packet. */
    Ccid(Pipe pipe, int packet) {
        this.pipe = pipe;
        this.packet = Math.max(8, packet);
    }

    /** The key powered on: its ATR. */
    byte[] powerOn() throws IOException {
        return exchange(POWER_ON, new byte[0], DATA_BLOCK);
    }

    /** The key powered off, its state with it. */
    void powerOff() throws IOException {
        exchange(POWER_OFF, new byte[0], SLOT_STATUS);
    }

    /** One APDU to the key: its answer, status words included. The APDU is the caller's to wipe. */
    byte[] transmit(byte[] apdu) throws IOException {
        return exchange(XFR_BLOCK, apdu, DATA_BLOCK);
    }

    /** The header and the data of one message to the key. */
    static byte[] message(byte type, byte[] data, byte sequence) {
        byte[] message = new byte[HEADER + data.length];
        message[0] = type;
        message[1] = (byte) data.length;
        message[2] = (byte) (data.length >> 8);
        message[3] = (byte) (data.length >> 16);
        message[4] = (byte) (data.length >> 24);
        message[5] = 0; // slot
        message[6] = sequence;
        // bPowerSelect (automatic), bBWI, wLevelParameter: all 0.
        System.arraycopy(data, 0, message, HEADER, data.length);
        return message;
    }

    private byte[] exchange(byte type, byte[] data, byte expected) throws IOException {
        byte number = sequence++;
        byte[] message = message(type, data, number);
        try {
            send(message);
        } finally {
            Arrays.fill(message, (byte) 0);
        }
        long until = System.nanoTime() + WAIT_MS * 1_000_000L;
        while (true) {
            byte[] answer = receive(until);
            try {
                if (answer[0] != expected)
                    throw new Failure("CCID answer of type " + (answer[0] & 0xFF));
                if (answer[6] != number)
                    throw new Failure("CCID answer to another message");
                int status = answer[7] & 0xFF;
                switch (status >> 6) {
                    case 0:
                        return Arrays.copyOfRange(answer, HEADER, answer.length);
                    case 2:
                        // Time extension: the key waits, for a touch most often.
                        if (System.nanoTime() > until)
                            throw new Failure("CCID: the key kept waiting");
                        continue;
                    default:
                        throw new Failure("CCID failure " + (answer[8] & 0xFF) + ", slot status " + (status & 0x03));
                }
            } finally {
                Arrays.fill(answer, (byte) 0);
            }
        }
    }

    private void send(byte[] message) throws IOException {
        int at = 0;
        while (at < message.length) {
            int length = Math.min(packet, message.length - at);
            int sent = pipe.send(message, at, length, TRANSFER_MS);
            if (sent <= 0)
                throw new Failure("USB: nothing sent");
            at += sent;
        }
        if (message.length % packet == 0 && pipe.send(message, 0, 0, TRANSFER_MS) < 0)
            throw new Failure("USB: no end sent");
    }

    /** One whole message from the key, its header first. */
    private byte[] receive(long until) throws IOException {
        byte[] buffer = new byte[Math.max(packet, 4096)];
        byte[] whole = new byte[0];
        try {
            int total = -1;
            int empty = 0;
            while (total < 0 || whole.length < total) {
                int got = pipe.receive(buffer, TRANSFER_MS);
                // Nothing within a transfer's time: a key waiting for a touch may
                // stay silent; read again until the deadline, unless it is gone.
                if (got < 0) {
                    if (!pipe.present())
                        throw new Failure("USB: the key is gone");
                    if (System.nanoTime() > until)
                        throw new Failure("USB: nothing came");
                    continue;
                }
                // A zero-length packet ends an answer that filled its last packet: the
                // one before this, read whole already. Within an answer, it cuts it short.
                if (got == 0) {
                    if (whole.length > 0 || ++empty > 2)
                        throw new Failure("USB: an answer cut short");
                    continue;
                }
                byte[] grown = Arrays.copyOf(whole, whole.length + got);
                System.arraycopy(buffer, 0, grown, whole.length, got);
                Arrays.fill(whole, (byte) 0);
                whole = grown;
                if (total < 0 && whole.length >= HEADER) {
                    long length = (whole[1] & 0xFFL) | (whole[2] & 0xFFL) << 8 | (whole[3] & 0xFFL) << 16 | (whole[4] & 0xFFL) << 24;
                    if (length > LONGEST - HEADER)
                        throw new Failure("CCID answer too long");
                    total = HEADER + (int) length;
                }
            }
            return Arrays.copyOf(whole, total);
        } finally {
            Arrays.fill(buffer, (byte) 0);
            Arrays.fill(whole, (byte) 0);
        }
    }
}
