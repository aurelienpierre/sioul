#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre
"""A demo profile for Sioul: an invented, calm and lived-in life, in English,
for trying Sioul and for the documentation's screenshots.

    make-demo.py --into DIR [--now 2026-10-05T10:30+02:00] [--no-hours] [--notes-at PATH]

Writes DIR/config, DIR/data, DIR/state and DIR/cache (the four XDG folders:
run Sioul with XDG_CONFIG_HOME=DIR/config, XDG_DATA_HOME=DIR/data,
XDG_STATE_HOME=DIR/state, XDG_CACHE_HOME=DIR/cache and SIOUL_DEMO=1, which
keeps it off the network), and DIR/notes, the folder of notes (the case store).

Everything is invented: the people, the companies, the messages. Addresses are
on example.org, example.com and .invalid hosts (RFC 2606), phone numbers in the
ranges the French regulator keeps for fiction. Nothing is random: the same
--now gives the same files. Dates are laid around --now (by default, now):
mail of the last two weeks, a one-time code from two minutes ago, this week's
events, two months of bank movements.

--no-hours leaves the working hours out, so the Porch asks for them.
--notes-at says where Sioul will find DIR/notes when it runs in a sandbox that
mounts it elsewhere (tools/demo/screenshots.sh mounts it at /home/demo/Notes).

DIR must be new, empty, or a demo profile made by this script: it is then
emptied first."""

import argparse
import base64
import email.header
import email.utils
import json
import os
import shutil
import struct
import sys
import zlib
from datetime import date, datetime, time, timedelta, timezone
from pathlib import Path

MARK = ".sioul-demo-profile"

ME = "Noa Ferrand"
WORK = "noa@example.com"
HOME = "noa.ferrand@example.org"
# The authserv-id of the mail provider, and Sioul's own for this profile: Sioul
# writes its checks at arrival under its own name; the messages carry them
# already, so nothing is checked again (nor asked of a DNS server) when it starts.
PROVIDER = "mx.example.invalid"
SIOUL = "sioul-demo.invalid"
# Each folder's UIDVALIDITY: the inbox's, then Sent's and Archive's after it.
VALIDITY = {"work": 1712040001, "personal": 1698003302}
DAV = "cloud"
DAV_URL = "https://dav.example.invalid/"
KIND = "tag:aurelienpierre.com,2026:sioul/task-type/"
OFFICE = "tag:aurelienpierre.com,2026:sioul/needs/office-hours"


# --- Time -------------------------------------------------------------------

class Clock:
    """The moment the profile is made for, and the dates around it."""

    def __init__(self, now: datetime):
        self.now = now.replace(microsecond=0)
        self.zone = self.now.tzinfo
        self.today = self.now.date()
        self.monday = self.today - timedelta(days=self.today.weekday())

    def day(self, offset: int) -> date:
        return self.today + timedelta(days=offset)

    def at(self, day: date, hhmm: str) -> datetime:
        hour, minute = (int(x) for x in hhmm.split(":"))
        return datetime.combine(day, time(hour, minute), tzinfo=self.zone)

    def ago(self, hours: float = 0, minutes: float = 0) -> datetime:
        return self.now - timedelta(hours=hours, minutes=minutes)

    def weekday_before(self, offset: int) -> date:
        """`offset` days from today, moved back to the Friday when it falls on a weekend."""
        d = self.day(offset)
        while d.weekday() >= 5:
            d -= timedelta(days=1)
        return d

    def weekday_after(self, offset: int) -> date:
        d = self.day(offset)
        while d.weekday() >= 5:
            d += timedelta(days=1)
        return d

    def this_week(self, weekday: int) -> date:
        """This week's day: 0 Monday … 6 Sunday."""
        return self.monday + timedelta(days=weekday)


def unix(moment: datetime) -> int:
    return int(moment.timestamp())


def utc(moment: datetime) -> str:
    return moment.astimezone(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def mail_date(moment: datetime) -> str:
    # Senders write their own offset; the instant is what counts.
    return email.utils.format_datetime(moment.astimezone(timezone(timedelta(hours=2))))


def ics_date(day: date) -> str:
    return day.strftime("%Y%m%d")


# --- Small files: PNG and PDF, drawn here --------------------------------------

def png(width: int, height: int, pixel) -> bytes:
    """An RGB PNG; `pixel(x, y)` gives each pixel's (r, g, b)."""
    rows = bytearray()
    for y in range(height):
        rows.append(0)
        for x in range(width):
            rows.extend(pixel(x, y))

    def chunk(kind: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)

    header = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(bytes(rows), 9)) + chunk(b"IEND", b"")


def sketch_png() -> bytes:
    """A homepage wireframe, pencil on paper: header, hero, three columns, footer."""
    paper, line, fill = (247, 244, 236), (120, 116, 108), (226, 221, 209)
    boxes = [(20, 16, 460, 44, False), (20, 56, 460, 150, True), (20, 162, 160, 250, False), (170, 162, 310, 250, False), (320, 162, 460, 250, False), (20, 262, 460, 284, True)]

    def pixel(x, y):
        for (x0, y0, x1, y1, shaded) in boxes:
            if x0 <= x <= x1 and y0 <= y <= y1:
                edge = x in (x0, x1) or y in (y0, y1)
                if edge:
                    return line
                if shaded and (x + y) % 6 == 0:
                    return fill
                # Text lines in the columns.
                if not shaded and y0 + 14 < y < y1 - 10 and (y - y0) % 14 in (0, 1) and x0 + 8 < x < x1 - 8 - ((y // 14) % 3) * 12:
                    return fill
                return paper
        return paper

    return png(480, 300, pixel)


def pdf(lines: list[str]) -> bytes:
    """A one-page A4 PDF with a title and a few lines, in Helvetica (ASCII only)."""
    def escape(text: str) -> str:
        return text.replace("\\", "\\\\").replace("(", "\\(").replace(")", "\\)")

    parts = ["BT", "/F1 20 Tf", "60 770 Td", f"({escape(lines[0])}) Tj", "/F1 12 Tf"]
    for text in lines[1:]:
        parts += ["0 -22 Td", f"({escape(text)}) Tj"]
    parts += ["ET", "0.75 G", "50 700 m 545 700 l S"]
    stream = "\n".join(parts).encode("ascii")
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>",
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
        b"<< /Length " + str(len(stream)).encode() + b" >>\nstream\n" + stream + b"\nendstream",
    ]
    out = bytearray(b"%PDF-1.4\n")
    offsets = []
    for number, body in enumerate(objects, start=1):
        offsets.append(len(out))
        out += f"{number} 0 obj\n".encode() + body + b"\nendobj\n"
    xref = len(out)
    out += f"xref\n0 {len(objects) + 1}\n0000000000 65535 f \n".encode()
    for offset in offsets:
        out += f"{offset:010d} 00000 n \n".encode()
    out += f"trailer\n<< /Size {len(objects) + 1} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode()
    return bytes(out)


# --- Writing ------------------------------------------------------------------

class Profile:
    def __init__(self, root: Path, clock: Clock, notes_at: str, hours: bool):
        self.root = root
        self.clock = clock
        self.notes_at = notes_at
        self.hours = hours
        self.config = root / "config" / "sioul"
        self.data = root / "data" / "sioul"
        self.state = root / "state" / "sioul"
        self.cache = root / "cache" / "sioul"
        self.notes = root / "notes"

    def write(self, path: Path, content, moment: datetime | None = None):
        path.parent.mkdir(parents=True, exist_ok=True)
        if isinstance(content, str):
            path.write_text(content, encoding="utf-8")
        else:
            path.write_bytes(content)
        if moment is not None:
            os.utime(path, (unix(moment), unix(moment)))


def toml_string(text: str) -> str:
    return json.dumps(text, ensure_ascii=False)


def toml_list(items) -> str:
    return "[" + ", ".join(toml_string(i) for i in items) + "]"


# --- The world ----------------------------------------------------------------

# People and offices, by key: name, addresses, phones, organisation, title,
# postal address (street, code, town), birthday, notes, categories, place (lat, lon).
CONTACTS = {
    "iris": dict(name="Iris Calloway", first="Iris", last="Calloway", email=[("work", "iris@fernhill.example.com")], phone=[("work", "+33 4 65 71 20 14")],
                 org="Fernhill Library", title="Head librarian", adr=("2 Rue de l'Exemple", "69006", "Lyon"), place=(45.7765, 4.8540),
                 note="The library's new website: Iris decides; Oskar does the technical side.", url="https://fernhill.example.com", cats=["clients"]),
    "oskar": dict(name="Oskar Lind", first="Oskar", last="Lind", email=[("work", "oskar@fernhill.example.com")], phone=[("work", "+33 4 65 71 20 15")],
                  org="Fernhill Library", title="IT and digital services", adr=("2 Rue de l'Exemple", "69006", "Lyon"), place=(45.7765, 4.8540), cats=["clients"]),
    "tom": dict(name="Tom Achebe", first="Tom", last="Achebe", email=[("work", "tom@inkwell.example.com")], phone=[("cell", "+33 6 39 98 41 07")],
                org="Inkwell Studio", title="Illustrator", adr=("9 Quai des Exemples", "69001", "Lyon"), place=(45.7675, 4.8337), cats=["colleagues"],
                note="Shares the stand at the December craft fair."),
    "maud": dict(name="Maud Ferrand", first="Maud", last="Ferrand", email=[("home", "maud.ferrand@example.org")], phone=[("cell", "+33 6 39 98 12 30")],
                 adr=("14 Allée Fictive", "69100", "Villeurbanne"), place=(45.7676, 4.8800), bday="--03-21", cats=["family"], note="Mum."),
    "camille": dict(name="Camille Ferrand", first="Camille", last="Ferrand", email=[("home", "camille.ferrand@example.org")], phone=[("cell", "+33 6 39 98 55 02")],
                    adr=("31 Rue Imaginaire", "38000", "Grenoble"), place=(45.1916, 5.7281), bday="--07-02", cats=["family"]),
    "hugo": dict(name="Hugo Ferrand", first="Hugo", last="Ferrand", email=[("home", "hugo.ferrand@example.org")], phone=[("cell", "+33 6 39 98 70 19")],
                 adr=("6 Passage Inventé", "42000", "Saint-Étienne"), place=(45.4397, 4.3872), bday="--10-14", cats=["family"]),
    "jonas": dict(name="Jonas Bell", first="Jonas", last="Bell", email=[("home", "jonas.bell@example.org")], phone=[("cell", "+33 6 39 98 33 84")],
                  adr=("18 Rue de l'Exemple", "69007", "Lyon"), place=(45.7457, 4.8423), cats=["friends"], note="Climbing on Saturdays."),
    "varga": dict(name="Dr Elena Varga", first="Elena", last="Varga", email=[], phone=[("work", "+33 4 65 71 47 70")],
                  org="Riverside Health Centre", title="General practitioner", adr=("5 Rue Imaginaire", "69003", "Lyon"), place=(45.7606, 4.8590),
                  note="Appointments by phone, mornings."),
    "marc": dict(name="Marc Duval", first="Marc", last="Duval", email=[("work", "marc@duval-accounts.example.org")], phone=[("work", "+33 4 65 71 63 25")],
                 org="Duval Accounts", title="Accountant", adr=("11 Place des Exemples", "69400", "Villefranche-sur-Saône"), place=(45.9900, 4.7180)),
    "priya": dict(name="Priya Nair", first="Priya", last="Nair", email=[("home", "priya@choir.example.org")], phone=[("cell", "+33 6 39 98 90 46")],
                  org="Two Rivers Voices choir", title="Choir director", adr=("3 Impasse Fictive", "69004", "Lyon"), place=(45.7745, 4.8320), cats=["friends"]),
    "healthcover": dict(name="Health cover office", email=[("work", "members@healthcover.example.org")], phone=[("work", "+33 4 65 71 80 00")],
                        org="Health cover office", adr=("20 Boulevard de l'Exemple", "69003", "Lyon"), place=(45.7608, 4.8458),
                        note="Open Monday to Friday, 9:00 to 12:00 and 14:00 to 16:30."),
    "sophie": dict(name="Sophie Marchand", first="Sophie", last="Marchand", email=[("work", "sophie.marchand@riversidebank.example.org")], phone=[("work", "+33 4 65 71 30 52")],
                   org="Riverside Bank", title="Account adviser", adr=("1 Cours Imaginaire", "69002", "Lyon"), place=(45.7578, 4.8320)),
}


def vcard(key: str, c: dict) -> str:
    lines = ["BEGIN:VCARD", "VERSION:3.0", f"UID:demo-contact-{key}", f"FN:{c['name']}"]
    if c.get("last"):
        lines.append(f"N:{c['last']};{c['first']};;;")
    else:
        lines.append(f"N:{c['name']};;;;")
    if c.get("org"):
        lines.append(f"ORG:{c['org']}")
    if c.get("title"):
        lines.append(f"TITLE:{c['title']}")
    for label, value in c.get("email", []):
        lines.append(f"EMAIL;TYPE={label}:{value}")
    for label, value in c.get("phone", []):
        lines.append(f"TEL;TYPE={label}:{value}")
    if c.get("adr"):
        street, code, town = c["adr"]
        kind = "home" if not c.get("org") else "work"
        lines.append(f"ADR;TYPE={kind}:;;{street};{town};;{code};France")
    if c.get("bday"):
        lines.append(f"BDAY:{c['bday']}")
    if c.get("url"):
        lines.append(f"URL:{c['url']}")
    if c.get("note"):
        lines.append("NOTE:" + c["note"].replace(",", "\\,"))
    if c.get("cats"):
        lines.append("CATEGORIES:" + ",".join(c["cats"]))
    lines.append("END:VCARD")
    return "\r\n".join(lines) + "\r\n"


def address_key(c: dict) -> str:
    """The address as Sioul keeps it in its places: its lines joined."""
    street, code, town = c["adr"]
    return f"{street}, {code} {town}, France"


# --- Mail ---------------------------------------------------------------------

class Mail:
    def __init__(self, account, when, sender, subject, body, *, to=None, folder="", flags="", auth="pass", kind=None,
                 attachments=(), reply_to_id=None, extra=(), key=None, cc=()):
        self.account = account
        self.when = when
        self.sender = sender  # (name, address)
        self.to = to or [(ME, WORK if account == "work" else HOME)]
        self.cc = list(cc)
        self.subject = subject
        self.body = body
        self.folder = folder
        self.flags = flags
        self.auth = auth  # "pass", "dkim", "none", "spam", "forged"
        self.kind = kind  # "list" for a newsletter
        self.attachments = list(attachments)
        self.reply_to_id = reply_to_id
        self.extra = list(extra)
        self.key = key
        self.message_id = None


def header_text(text: str) -> str:
    try:
        text.encode("ascii")
        return text
    except UnicodeEncodeError:
        return email.header.Header(text, "utf-8").encode()


def address(name: str, addr: str) -> str:
    return email.utils.formataddr((name, addr), charset="utf-8") if name else addr


def domain_of(addr: str) -> str:
    return addr.rsplit("@", 1)[1]


def raw_message(m: Mail, number: int) -> bytes:
    domain = domain_of(m.sender[1])
    m.message_id = m.message_id or f"<{m.when.strftime('%Y%m%d%H%M')}.{number}.{m.key or 'm'}@{domain}>"
    head = []
    if m.folder in ("", ".Archive"):
        if m.auth == "pass":
            checks = f"dkim=pass header.d={domain}; spf=pass smtp.mailfrom={domain}; dmarc=pass header.from={domain}"
        elif m.auth == "forged":
            checks = f"dkim=fail header.d={domain}; spf=fail smtp.mailfrom={domain}; dmarc=fail policy.dmarc=reject header.from={domain}"
        else:
            checks = f"dkim=none; spf=softfail smtp.mailfrom={domain}; dmarc=none header.from={domain}"
        # Sioul's own checks at arrival, then the provider's, as both write them.
        head.append(f"Authentication-Results: {SIOUL}; {checks}")
        head.append(f"Authentication-Results: {PROVIDER}; {checks}")
        if m.auth == "spam":
            head.append("X-Spam-Flag: YES")
            head.append("X-Spam-Status: Yes, score=9.4 required=5.0 tests=FREEMAIL_FORGED_REPLYTO,LOTS_OF_MONEY,URIBL_BLOCKED")
        head.append(f"Return-Path: <{m.sender[1]}>")
        head.append(f"Received: from mail.{domain} (mail.{domain} [192.0.2.{20 + number % 200}]) by {PROVIDER} with ESMTPS; {mail_date(m.when + timedelta(seconds=4))}")
    head.append(f"From: {address(*m.sender)}")
    head.append("To: " + ", ".join(address(n, a) for n, a in m.to))
    if m.cc:
        head.append("Cc: " + ", ".join(address(n, a) for n, a in m.cc))
    head.append(f"Subject: {header_text(m.subject)}")
    head.append(f"Date: {mail_date(m.when)}")
    head.append(f"Message-ID: {m.message_id}")
    if m.reply_to_id:
        head.append(f"In-Reply-To: {m.reply_to_id}")
        head.append(f"References: {m.reply_to_id}")
    if m.kind == "list":
        list_domain = domain
        head.append(f"List-Id: <letter.{list_domain}>")
        head.append(f"List-Unsubscribe: <mailto:unsubscribe@{list_domain}>")
    head.extend(m.extra)
    head.append("MIME-Version: 1.0")
    text = m.body.strip("\n") + "\n"
    if not m.attachments:
        head.append("Content-Type: text/plain; charset=UTF-8")
        head.append("Content-Transfer-Encoding: 8bit")
        return ("\n".join(head) + "\n\n" + text).encode("utf-8")
    boundary = f"=_sioul_demo_{number:04d}"
    head.append(f'Content-Type: multipart/mixed; boundary="{boundary}"')
    parts = [f"--{boundary}", "Content-Type: text/plain; charset=UTF-8", "Content-Transfer-Encoding: 8bit", "", text]
    for name, mime, content in m.attachments:
        encoded = base64.encodebytes(content).decode("ascii")
        parts += [f"--{boundary}", f'Content-Type: {mime}; name="{name}"', f'Content-Disposition: attachment; filename="{name}"', "Content-Transfer-Encoding: base64", "", encoded]
    parts.append(f"--{boundary}--")
    return ("\n".join(head) + "\n\n" + "\n".join(parts) + "\n").encode("utf-8")


def the_mail(clock: Clock) -> list[Mail]:
    c = clock
    iris = ("Iris Calloway", "iris@fernhill.example.com")
    oskar = ("Oskar Lind", "oskar@fernhill.example.com")
    tom = ("Tom Achebe", "tom@inkwell.example.com")
    me_work = (ME, WORK)
    me_home = (ME, HOME)
    mails = [
        # Work: noa@example.com.
        Mail("work", c.ago(minutes=2), ("Nimbus Hosting", "no-reply@nimbus.example.com"), "Your sign-in code",
             """Hello Noa,

Your sign-in code is 482 913.

It is valid for 15 minutes. If you did not try to sign in to your Nimbus Hosting panel, you can ignore this message: nobody can sign in without the code.

Nimbus Hosting""", key="code"),
        Mail("work", c.ago(minutes=55), iris, "Homepage: two small changes",
             ("""Hello Noa,

Thank you for this morning's review: the whole team likes the calm layout, the one with the large photo of the reading room.

The two small changes we spoke about, so that you have them in writing:
""" if c.ago(minutes=55) >= c.at(c.today, "11:15") else """Hello Noa,

The whole team likes the calm layout, the one with the large photo of the reading room. Before our review this morning, two small changes:
""") + """
1. The opening hours right under the photo, rather than at the bottom: most people come to the site for them.
2. The children's corner in the menu, as its own entry.

No hurry at all.

Best,
Iris

--
Iris Calloway
Head librarian, Fernhill Library
2 Rue de l'Exemple, 69006 Lyon""", key="homepage"),
        Mail("work", c.ago(hours=2, minutes=10), ("Hana Okafor", "hana@greenfield-coop.example.com"), "A small website for our cooperative?",
             """Dear Noa,

Iris Calloway at Fernhill Library gave me your address. We are a small cooperative of market gardeners near Lyon, and our website has not changed since 2014.

Would you have time this winter for a simple site: who we are, our market days, how to order a basket? Could you tell me roughly what it would cost?

With thanks,
Hana Okafor
Greenfield Cooperative""", key="hana"),
        Mail("work", c.ago(hours=20), tom, "Shared stand at the craft fair?",
             """Hi Noa,

The craft fair in December has a few double stands left. Shall we share one again? Your posters and my prints worked well side by side last year.

They want an answer by the end of the week. No pressure, just say yes or no when you can.

Tom""", key="fair"),
        Mail("work", c.ago(hours=23, minutes=30), ("Code forge", "notifications@forge.example.com"), "[fernhill-site] Pull request merged: calmer footer",
             """Oskar Lind merged pull request #14 into main.

calmer footer: one line, the address and the opening hours

--
You receive this because you are watching fernhill-site.""", key="forge"),
        Mail("work", c.ago(hours=26), ("Prize Department", "win@lucky-prizes.example.com"), "Congratulations! You have won a cruise",
             """CONGRATULATIONS!!!

You have been selected to receive a FREE cruise for two. Claim your prize within 24 hours by sending your bank details.""", auth="spam", key="spam"),
        Mail("work", c.ago(hours=28), ("Type & Pixels", "letter@typeandpixels.example.com"), "Type & Pixels #112: quiet layouts",
             """This week: quiet layouts.

- Why white space is not empty space
- Three typefaces for public libraries
- A reader's question: how large should body text be on a phone?

You receive Type & Pixels because you subscribed on typeandpixels.example.com.""", kind="list", key="news"),
        Mail("work", c.at(c.weekday_before(-2), "16:20"), oskar, "Photos of the reading room",
             """Hi Noa,

Here are the photos of the reading room and of the children's corner, taken this morning while it was quiet. Use whichever you like; the library holds the rights.

Oskar""", attachments=[("reading-room-sketch.png", "image/png", sketch_png())], flags="S", key="photos"),
        Mail("work", c.at(c.weekday_before(-3), "06:30"), ("Nimbus Hosting", "no-reply@nimbus.example.com"), "Your invoice NH-2026-1009",
             """Hello Noa,

Your invoice NH-2026-1009 for October is available in your panel.

Plan: Studio S (web hosting)
Amount due: €12.00
It will be charged on 10 October to your card ending in 0042.

Nimbus Hosting""", flags="S", key="nimbus-invoice"),
        Mail("work", c.at(c.weekday_before(-4), "11:12"), ("Foundry North", "noreply@foundrynorth.example.com"), "Your receipt for order #4471",
             """Thank you for your order.

Order #4471
Lantern Serif, desktop and web licence (1 site)
Total: €48.00
Paid with PayPal.

Foundry North""", flags="S", key="fonts"),
        Mail("work", c.at(c.weekday_before(-6), "11:05"), iris, "Re: Opening hours page",
             """Hello Noa,

Yes, the opening hours can come from our booking system: Oskar will send you the address of its calendar.

Iris""", flags="RS", key="hours"),
        Mail("work", c.at(c.weekday_before(-9), "10:40"), tom, "Re: Poster series",
             """Lovely. The blue one should go in your portfolio first.

Tom""", flags="S", key="posters"),
        Mail("work", c.at(c.weekday_before(-12), "09:30"), iris, "The new website: our agreement",
             """Hello Noa,

As agreed on the phone: the new website in three stages (mock-up, pages, launch), at €55 an hour, invoiced at the end of each stage. Our signed copy of the quote is attached.

We are glad to work with you again.

Iris""", attachments=[("fernhill-quote-signed.pdf", "application/pdf", pdf(["Fernhill Library - new website", "Quote 2026-Q07, signed", "Three stages: mock-up, pages, launch", "Rate: 55 EUR an hour", "Specimen: invented for Sioul's demo"]))],
             folder=".Archive", flags="S", key="agreement"),
        # Sent from work.
        Mail("work", c.at(c.weekday_before(-6), "14:02"), me_work, "Re: Opening hours page",
             """Thank you Iris, that is the best way: the page will never be out of date.

Noa""", to=[iris], folder=".Sent", flags="S", key="sent-hours"),
        Mail("work", c.at(c.weekday_before(-3), "17:05"), me_work, "Homepage mock-up, first draft",
             """Hello Iris,

Here is a first draft of the homepage, in two layouts. Tell me which feels closer to the library.

Noa""", to=[iris], cc=[oskar], folder=".Sent", flags="S", key="sent-draft"),
        Mail("work", c.at(c.weekday_before(-9), "09:15"), me_work, "Poster series",
             """Tom, the poster series is finished: six bridges, one colour each. Which one would you show first?

Noa""", to=[tom], folder=".Sent", flags="S", key="sent-posters"),

        # Personal: noa.ferrand@example.org.
        Mail("personal", c.ago(minutes=6), ("Paper & Ink", "no-reply@paperandink.example.org"), "Confirm your email address",
             """Hello,

Please confirm your email address to finish creating your Paper & Ink account:

https://paperandink.example.org/confirm?token=7Q2-demo-41

This link is valid for 30 minutes. If you did not create an account, ignore this message.

Paper & Ink""", key="confirm"),
        Mail("personal", c.ago(hours=3, minutes=5), (ME, WORK), "Note to self: craft fair sizes",
             """Posters in A3 and A2; the frames are in the attic, the small table is at Mum's.""", key="self"),
        Mail("personal", c.ago(hours=1, minutes=20), ("Maud Ferrand", "maud.ferrand@example.org"), "Sunday lunch?",
             """Hello my dear,

Would you come for lunch on Sunday? Hugo will be there, and I am making the apple cake.

Love,
Mum""", key="lunch"),
        Mail("personal", c.ago(hours=18), ("Jonas Bell", "jonas.bell@example.org"), "Climbing on Saturday?",
             """Saturday morning at the gym, 10:00? The new routes on the left wall are lovely.

Jonas""", key="climbing"),
        Mail("personal", c.ago(hours=29), ("Parcel service", "notification@parcels.example.org"), "Your parcel arrives on Wednesday",
             """Your parcel from Paper & Ink will arrive on Wednesday between 9:00 and 13:00. Nothing to do: if you are out, it waits at the Rue de l'Exemple relay.""", key="parcel"),
        Mail("personal", c.at(c.weekday_before(-2), "18:40"), ("Priya Nair", "priya@choir.example.org"), "Concert programme and an extra rehearsal",
             """Dear all,

The winter concert programme is settled: Ave verum, two old carols and the Rutter. We add one rehearsal on the Thursday before the concert.

Altos: please learn the second page of the Ave verum by next week.

Priya""", flags="S", key="choir"),
        Mail("personal", c.at(c.day(-2), "11:30"), ("Agnès Morel", "agnes.morel@example.org"), "Hello from the new neighbour",
             """Hello,

I have just moved into the flat on the third floor. If my boxes in the hall bother you, tell me: they will be gone by Friday.

Agnès""", key="neighbour"),
        Mail("personal", c.at(c.weekday_before(-3), "08:10"), ("Riverside Bank", "no-reply@riversidebank.example.org"), "A new document in your secure space",
             """Hello,

A new document is waiting in your secure space: your September statement.

Riverside Bank""", flags="S", key="bank-doc"),
        Mail("personal", c.at(c.weekday_before(-5), "07:45"), ("Brightwatt", "no-reply@brightwatt.example.org"), "Your October bill: €64.20",
             """Hello,

Your October electricity bill is ready.

Amount: €64.20
It will be taken by direct debit on 15 October.

Your meter reading is due before the 20th.

Brightwatt""", flags="S", key="energy"),
        Mail("personal", c.at(c.weekday_before(-6), "09:02"), ("Lyon Climbing Club", "news@climbing-club.example.org"), "October news: new routes on the left wall",
             """Ten new routes on the left wall, from beginner to hard. The gym closes early on 31 October.""", kind="list", flags="S", key="club"),
        Mail("personal", c.at(c.weekday_before(-7), "10:20"), ("Wavecell", "noreply@wavecell.example.org"), "Your receipt: €19.99",
             """Thank you for your payment.

Mobile plan, October
Amount paid: €19.99

Wavecell""", flags="S", key="phone"),
        Mail("personal", c.at(c.weekday_before(-8), "14:15"), ("Health cover office", "no-reply@healthcover.example.org"), "Your refund of €18.50 has been paid",
             """Hello,

Your refund of €18.50 for the consultation of 22 September has been paid into your account.

Health cover office""", flags="S", key="refund"),
        Mail("personal", c.at(c.weekday_before(-10), "07:00"), ("Tax office", "no-reply@tax-office.example.org"), "Your 2026 tax notice is available",
             """Hello,

Your 2026 tax notice (2025 income) is available in your secure space on tax-office.example.org.

Tax office""", flags="S", key="tax"),
        Mail("personal", c.at(c.day(-4), "19:30"), ("Camille Ferrand", "camille.ferrand@example.org"), "Photos from the weekend",
             """Here they are, the ones from the beach. The dog is in all of them.

Camille""", flags="S", key="camille"),
        Mail("personal", c.at(c.weekday_before(-7), "16:00"), ("Marc Duval", "marc@duval-accounts.example.org"), "Receipts for 2025",
             """Hello Noa,

When you have a moment before the end of October, could you send me your 2025 receipts? A folder of scans is perfect.

Marc Duval""", flags="RS", key="marc"),
        Mail("personal", c.at(c.day(-1), "21:40"), ("Tax office", "refund@tax-office.example.org"), "Your tax refund is waiting",
             """You are owed a refund of €312.40. Confirm your card details within 48 hours to receive it.""", auth="forged", key="phishing"),
        # Sent from home.
        Mail("personal", c.at(c.weekday_before(-6), "18:20"), me_home, "Re: Receipts for 2025",
             """Hello Marc, I will gather them this month and send them by the 30th.

Noa""", to=[("Marc Duval", "marc@duval-accounts.example.org")], folder=".Sent", flags="S", key="sent-marc"),
        Mail("personal", c.at(c.day(-3), "20:05"), me_home, "Gym hours",
             """Jonas, the gym opens at 9:30 on Saturdays now, not 10:00.""", to=[("Jonas Bell", "jonas.bell@example.org")], folder=".Sent", flags="S", key="sent-jonas"),
    ]
    # Each message its id; replies point at what they answer.
    by_key = {m.key: m for m in mails}
    for m in mails:
        m.message_id = f"<{m.when.strftime('%Y%m%d%H%M')}.{m.key}@{domain_of(m.sender[1])}>"
    for reply, original in [("sent-hours", "hours"), ("homepage", "sent-draft"), ("sent-marc", "marc"), ("posters", "sent-posters")]:
        by_key[reply].reply_to_id = by_key[original].message_id
    return mails


def write_mail(p: Profile, mails: list[Mail]):
    counters: dict[tuple[str, str], int] = {}
    numbered = sorted(enumerate(mails), key=lambda pair: pair[1].when)
    marks: dict[str, int] = {}
    closed_at = p.clock.ago(hours=30)
    for number, m in numbered:
        folder_key = (m.account, m.folder)
        uid = counters.get(folder_key, 3100 if m.account == "work" else 5200) + 1
        counters[folder_key] = uid
        validity = VALIDITY[m.account] + {"": 0, ".Sent": 1, ".Archive": 2}[m.folder]
        raw = raw_message(m, number + 1)
        unique = f"{unix(m.when)}.U{validity}-{uid}.sioul"
        root = p.data / "mail" / m.account / m.folder if m.folder else p.data / "mail" / m.account
        for sub in ("tmp", "new", "cur"):
            (root / sub).mkdir(parents=True, exist_ok=True)
        path = root / "cur" / f"{unique}:2,{m.flags}" if m.flags else root / "new" / unique
        p.write(path, raw, m.when)
        # The Porch was last closed a day and a bit ago: what came before is done.
        if m.folder == "" and m.when < closed_at:
            marks[m.account] = max(marks.get(m.account, 0), uid)
    for account in ("work", "personal"):
        for sub in ("tmp", "new", "cur"):
            for folder in ("", ".Drafts", ".Archive", ".Junk", ".Trash"):
                (p.data / "mail" / account / folder / sub if folder else p.data / "mail" / account / sub).mkdir(parents=True, exist_ok=True)
        folders = [("INBOX", "INBOX", "inbox", "", False), ("Sent", "Sent", "sent", ".Sent", True), ("Drafts", "Drafts", "drafts", ".Drafts", True),
                   ("Archive", "Archive", "archive", ".Archive", True), ("Junk", "Junk", "junk", ".Junk", True), ("Trash", "Trash", "trash", ".Trash", True)]
        text = "".join(f"[[folder]]\nname = {toml_string(n)}\ndisplay = {toml_string(d)}\nrole = \"{r}\"\nlocal = {toml_string(l)}\nspecial = {'true' if s else 'false'}\n\n" for n, d, r, l, s in folders)
        p.write(p.state / "sync" / f"{account}.folders.toml", text)
    porch = "".join(f"[done.{account}]\nvalidity = {VALIDITY[account]}\nuid = {uid}\n\n" for account, uid in sorted(marks.items()))
    p.write(p.state / "porch.toml", porch)
    p.write(p.state / "authserv-id", SIOUL + "\n")


# --- Calendars and tasks ------------------------------------------------------

def vcalendar(component: list[str]) -> str:
    return "\r\n".join(["BEGIN:VCALENDAR", "VERSION:2.0", "PRODID:-//Sioul//Demo profile//EN", *component, "END:VCALENDAR"]) + "\r\n"


def escape(text: str) -> str:
    return text.replace("\\", "\\\\").replace(";", "\\;").replace(",", "\\,").replace("\n", "\\n")


def event(clock: Clock, uid: str, title: str, day: date, start: str, end: str, *, location="", notes="", weekly=False,
          organizer=None, attendees=(), case=None) -> str:
    lines = ["BEGIN:VEVENT", f"UID:demo-event-{uid}", f"DTSTAMP:{utc(clock.at(clock.day(-14), '09:00'))}",
             f"DTSTART:{utc(clock.at(day, start))}", f"DTEND:{utc(clock.at(day, end))}", f"SUMMARY:{escape(title)}"]
    if location:
        lines.append(f"LOCATION:{escape(location)}")
    if notes:
        lines.append(f"DESCRIPTION:{escape(notes)}")
    if weekly:
        lines.append("RRULE:FREQ=WEEKLY")
    if organizer:
        lines.append(f"ORGANIZER;CN={organizer[0]}:mailto:{organizer[1]}")
    for name, addr, answer in attendees:
        lines.append(f"ATTENDEE;CN={name};PARTSTAT={answer}:mailto:{addr}")
    if case:
        lines.append(f"REFID:{case}")
    lines.append("END:VEVENT")
    return vcalendar(lines)


def write_calendars(p: Profile, mails: list[Mail]):
    c = p.clock
    collections = [
        ("calendars", "personal", "Personal", "#7a6f9b", ["VEVENT"]),
        ("calendars", "work", "Work", "#4c6b5c", ["VEVENT"]),
        ("calendars", "home-tasks", "Home", "#9a7b4f", ["VTODO"]),
        ("calendars", "work-tasks", "Work tasks", "#4c6b5c", ["VTODO"]),
        ("contacts", "contacts", "Contacts", None, []),
    ]
    for kind, cid, name, colour, components in collections:
        folder = p.data / kind / DAV / cid
        p.write(folder / "displayname", name)
        if colour:
            p.write(folder / "color", colour)
        state = [f"url = {toml_string(DAV_URL + kind + '/noa/' + cid + '/')}", f"ctag = \"demo-{cid}-1\"", "read_only = false",
                 "components = " + toml_list(components)]
        p.write(p.state / "dav" / DAV / kind / f"{cid}.toml", "\n".join(state) + "\n")

    me_w = (ME, WORK, "ACCEPTED")
    iris = ("Iris Calloway", "iris@fernhill.example.com")
    events = [
        ("work", event(c, "fernhill-review", "Fernhill: homepage review", c.day(0), "10:15", "11:15", location="Video call, meet.example.com/fernhill",
                       notes="The two layouts, the opening hours under the photo, the children's corner in the menu.",
                       organizer=iris, attendees=[me_w, ("Oskar Lind", "oskar@fernhill.example.com", "ACCEPTED")], case="fernhill")),
        ("work", event(c, "planning", "Weekly planning", c.this_week(0), "09:00", "10:00", notes="The week's steps, in Sioul's Tasks.", weekly=True)),
        ("personal", event(c, "doctor", "Dr Varga: check-up", c.weekday_after(1), "09:30", "10:00", location="Riverside Health Centre, 5 Rue Imaginaire, Lyon",
                           notes="Bring the blood test results.")),
        ("work", event(c, "accountant", "Call with Marc (accountant)", c.weekday_after(2), "11:00", "11:30", location="Phone: +33 4 65 71 63 25",
                       notes="The 2025 receipts, and the quarterly declaration.", case="taxes-2026")),
        ("personal", event(c, "choir", "Choir rehearsal", c.this_week(3), "18:45", "20:30", location="Two Rivers Voices, community hall", weekly=True, case="choir")),
        ("work", event(c, "tom-lunch", "Lunch with Tom", c.this_week(4), "12:30", "13:45", location="Café du Quai")),
        ("personal", event(c, "climbing", "Climbing with Jonas", c.this_week(5), "10:00", "12:00", location="Bloc & Boulder gym")),
        ("personal", event(c, "lunch-mum", "Lunch at Mum's", c.this_week(6), "12:30", "15:00", location="Villeurbanne")),
        ("work", event(c, "fernhill-pages", "Fernhill: pages kick-off", c.weekday_after(8), "10:00", "11:00", location="Fernhill Library",
                       organizer=iris, attendees=[me_w], case="fernhill")),
        ("personal", event(c, "hugo-birthday", "Hugo's birthday dinner", c.day(9), "19:30", "22:00", location="Saint-Étienne")),
    ]
    for calendar, text in events:
        uid = text.split("UID:")[1].split("\r\n")[0]
        p.write(p.data / "calendars" / DAV / calendar / f"{uid}.ics", text)

    by_key = {m.key: m for m in mails}
    write_tasks(p, by_key)


def task(clock: Clock, uid: str, title: str, *, status="NEEDS-ACTION", start=None, due=None, estimate=0, kind="", categories=(),
         case=None, parent=None, waits=(), notes="", links=(), contacts=(), office=None, completed=None, priority=0, created=None, energy="") -> str:
    made = created or clock.at(clock.day(-12), "09:00")
    lines = ["BEGIN:VTODO", f"UID:demo-task-{uid}", f"DTSTAMP:{utc(made)}", f"CREATED:{utc(made)}", f"SUMMARY:{escape(title)}", f"STATUS:{status}"]
    if notes:
        lines.append(f"DESCRIPTION:{escape(notes)}")
    if start:
        lines.append(f"DTSTART;VALUE=DATE:{ics_date(start)}")
    if due:
        lines.append(f"DUE;VALUE=DATE:{ics_date(due)}")
    if estimate:
        hours, minutes = divmod(estimate, 60)
        lines.append("ESTIMATED-DURATION:PT" + (f"{hours}H" if hours else "") + (f"{minutes}M" if minutes or not hours else ""))
    if priority:
        lines.append(f"PRIORITY:{priority}")
    if categories:
        lines.append("CATEGORIES:" + ",".join(categories))
    if kind:
        lines.append(f"CONCEPT:{KIND}{kind}")
    if office is not None:
        lines.append(f"CONCEPT:{OFFICE}")
        if office:
            lines.append(f"X-SIOUL-OFFICE-HOURS:{escape(office)}")
    if energy:
        lines.append(f"X-SIOUL-ENERGY:{energy.upper()}")
    if case:
        lines.append(f"REFID:{case}")
    if parent:
        lines.append(f"RELATED-TO;RELTYPE=PARENT:demo-task-{parent}")
    for other in waits:
        lines.append(f"RELATED-TO;RELTYPE=DEPENDS-ON:demo-task-{other}")
    for rel, uri in links:
        lines.append(f"LINK;LINKREL={rel};VALUE=URI:{uri}")
    for key in contacts:
        lines.append(f'CONTACT;ALTREP="sioul:contact/demo-contact-{key}":{escape(CONTACTS[key]["name"])}')
    if completed:
        lines += [f"COMPLETED:{utc(completed)}", "PERCENT-COMPLETE:100"]
    lines.append("END:VTODO")
    return vcalendar(lines)


def note_uri(path: str) -> str:
    out = ""
    for ch in path:
        out += "%20" if ch == " " else ch
    return "sioul:note/" + out


def mid(m: "Mail") -> str:
    return "mid:" + m.message_id.strip("<>")


def write_tasks(p: Profile, mail: dict):
    c = p.clock
    fernhill_note = note_uri("Projects/Fernhill Library.md")
    tasks = [
        # Work.
        ("work-tasks", task(c, "mockup", "Fernhill: homepage mock-up", due=c.weekday_after(4), kind="make", categories=["work"], case="fernhill",
                            links=[("describedby", fernhill_note)], contacts=["iris"], notes="Two layouts; the calm one first. Opening hours under the photo.")),
        ("work-tasks", task(c, "photos", "Gather the library's photos", status="COMPLETED", estimate=30, kind="online", categories=["work"], case="fernhill",
                            parent="mockup", completed=c.at(c.weekday_before(-2), "17:10"), links=[("via", mid(mail["photos"]))])),
        ("work-tasks", task(c, "sketch", "Sketch two layouts for the homepage", status="IN-PROCESS", start=c.day(0), estimate=240, kind="make",
                            categories=["work"], case="fernhill", parent="mockup", links=[("describedby", fernhill_note)])),
        ("work-tasks", task(c, "send-mockup", "Send the mock-up to Iris", estimate=30, kind="write", categories=["work"], case="fernhill",
                            parent="mockup", waits=["sketch"], contacts=["iris"])),
        ("work-tasks", task(c, "hours-page", "Fernhill: opening hours page", estimate=90, kind="make", categories=["work"], case="fernhill",
                            waits=["mockup"], due=c.weekday_after(11), links=[("via", mid(mail["hours"]))])),
        ("work-tasks", task(c, "invoice", "Invoice Fernhill for the mock-up stage", estimate=15, kind="online", categories=["work"], case="fernhill",
                            waits=["send-mockup"], due=c.weekday_after(6))),
        ("work-tasks", task(c, "quote", "Write a quote for the Greenfield Cooperative", estimate=60, kind="write", categories=["work"],
                            due=c.weekday_after(3), links=[("via", mid(mail["hana"]))], created=c.ago(hours=2))),
        ("work-tasks", task(c, "fair", "Craft fair: answer Tom about the shared stand", estimate=15, kind="write", categories=["work"],
                            due=c.weekday_after(4), contacts=["tom"], links=[("via", mid(mail["fair"]))])),
        ("work-tasks", task(c, "portfolio", "Portfolio: add the poster series", estimate=90, kind="make", categories=["work", "someday"])),
        ("work-tasks", task(c, "oskar", "Thank Oskar for the photos", status="COMPLETED", estimate=5, kind="write", categories=["work"], case="fernhill",
                            completed=c.at(c.day(0), "09:20"), contacts=["oskar"])),
        # Admin.
        ("home-tasks", task(c, "health-call", "Call the health cover office about the refund", estimate=15, kind="call", categories=["admin", "health"],
                            office="mo-fr 09:00-12:00, 14:00-16:30", contacts=["healthcover"], notes="The September consultation: only €18.50 came back.")),
        ("home-tasks", task(c, "meter", "Send the meter reading to Brightwatt", estimate=10, kind="online", categories=["admin"],
                            due=c.day(12), links=[("via", mid(mail["energy"]))])),
        ("home-tasks", task(c, "passport", "Passport: book an appointment at the town hall", estimate=15, kind="online", categories=["admin"],
                            start=c.day(0), due=c.day(30), links=[("related", "sioul:paper/passport")])),
        ("home-tasks", task(c, "receipts", "Gather the 2025 receipts", estimate=45, kind="make", categories=["admin"], case="taxes-2026",
                            links=[("describedby", note_uri("Admin/Income tax 2026.md"))])),
        ("home-tasks", task(c, "send-receipts", "Send the receipts to Marc", estimate=10, kind="write", categories=["admin"], case="taxes-2026",
                            waits=["receipts"], due=c.day(24), contacts=["marc"], links=[("via", mid(mail["marc"]))])),
        ("home-tasks", task(c, "tax-check", "Read the 2026 tax notice", estimate=20, kind="read", categories=["admin"], case="taxes-2026",
                            due=c.day(20), links=[("via", mid(mail["tax"]))])),
        ("home-tasks", task(c, "bike", "Fix the bike's back light", estimate=20, kind="make", categories=["admin"])),
        ("home-tasks", task(c, "bus-pass", "Renew the bus pass", status="COMPLETED", estimate=10, kind="online", categories=["admin"],
                            completed=c.at(c.day(-3), "18:30"))),
        # Leisure.
        ("home-tasks", task(c, "alto", "Learn the alto part of the Ave verum", estimate=30, kind="make", categories=["leisure", "joy"], case="choir",
                            links=[("via", mid(mail["choir"]))])),
        ("home-tasks", task(c, "gift", "Find a birthday present for Hugo", estimate=30, kind="out", categories=["family"], due=c.day(8), contacts=["hugo"])),
        ("home-tasks", task(c, "gym", "Book the climbing gym for Saturday", estimate=5, kind="online", categories=["leisure"], due=c.this_week(4),
                            contacts=["jonas"])),
    ]
    for calendar, text in tasks:
        uid = text.split("UID:")[1].split("\r\n")[0]
        p.write(p.data / "calendars" / DAV / calendar / f"{uid}.ics", text)


def write_contacts(p: Profile):
    for key, c in CONTACTS.items():
        p.write(p.data / "contacts" / DAV / "contacts" / f"demo-contact-{key}.vcf", vcard(key, c))
    places = ["[found]"]
    for c in CONTACTS.values():
        if c.get("adr"):
            lat, lon = c["place"]
            places.append(f"{toml_string(address_key(c))} = {{ lat = {lat}, lon = {lon} }}")
    p.write(p.cache / "places.toml", "\n".join(dict.fromkeys(places)) + "\n")


# --- The folder of notes: projects, notes, budgets, the bank, papers ------------

def write_notes(p: Profile, mails: list[Mail]):
    c = p.clock
    by_key = {m.key: m for m in mails}
    n = p.notes
    stamp = lambda days, hhmm: c.at(c.day(days), hhmm)
    notes = {
        "Projects/Fernhill Library.md": (stamp(0, "09:40"), f"""---
tags: [client, website]
aliases: [Fernhill]
---
# Fernhill Library: the new website

For [[Iris Calloway]] and Oskar. Three stages, invoiced at the end of each: mock-up, pages, launch. €55 an hour.

## Where it stands
- Mock-up: two layouts sketched, the calm one with the reading room's photo preferred.
- Opening hours come from the library's booking calendar ([[Fernhill meeting 2026-09-29]]).

## To do
- [x] Gather the library's photos
- [ ] Opening hours right under the photo
- [ ] A menu entry for the children's corner
- [ ] Check the contrast of the footer

![[homepage-sketch.png]]

The agreement: [the signed quote]({mid(by_key['agreement'])}). #client
"""),
        "Projects/Fernhill meeting 2026-09-29.md": (stamp(-6, "11:30"), """# Fernhill: first meeting

With Iris and Oskar, at the library.

- They want a site that reads well on a phone: most visitors look for the opening hours.
- The children's corner is the library's pride.
- Oskar can give a calendar address for the hours.
- Next: the mock-up, by the end of next week.

Back to [[Fernhill Library]].
"""),
        "Projects/homepage-sketch.png": (stamp(-1, "16:00"), sketch_png()),
        "Admin/Income tax 2026.md": (stamp(-9, "18:00"), f"""# Income tax 2026 (2025 income)

- The notice came on {c.weekday_before(-10).strftime('%-d %B')}: [the message]({mid(by_key['tax'])}).
- [[Marc Duval]] wants the 2025 receipts before the end of October.
- Professional income declared by Marc; nothing else to add this year.

## Receipts
- [ ] Bank statements, January to December
- [ ] Software and fonts
- [x] Hosting invoices

#taxes
"""),
        "Admin/Health cover.md": (stamp(-8, "15:00"), """# Health cover

Refund for the consultation of 22 September: €18.50, where €25 was expected. To ask them, by phone (mornings are quieter).

Office hours: Monday to Friday, 9:00–12:00 and 14:00–16:30. #health
"""),
        "Admin/Flat lease.md": (stamp(-30, "10:00"), """# The flat's lease

Signed on 1 March 2024 with Hillside Lets, three years, renewed by itself.
Notice: one month (furnished flat).

The boiler is checked every year in November: the agency calls. #home
"""),
        "Home/Lentil soup.md": (stamp(-5, "19:10"), """# Lentil soup

- 250 g green lentils, rinsed
- 1 onion, 2 carrots, 1 celery stick, diced
- 1 tsp cumin, 1 bay leaf
- 1.2 l water or stock

Soften the vegetables, add the rest, 35 minutes. Lemon at the end. #recipes
"""),
        "Home/Reading list.md": (stamp(-11, "21:00"), """# Reading list

- [x] The Overstory
- [ ] A Psalm for the Wild-Built
- [ ] The Hidden Life of Trees

#reading
"""),
        "Choir/Winter concert.md": (stamp(-2, "19:00"), """# Winter concert

Saturday 12 December, 20:00, at the community hall.

Programme: Ave verum (Mozart), two old carols, Rutter's Gaelic blessing.
- [ ] Alto part of the Ave verum, second page
- [ ] Black clothes, a coloured scarf

[[Priya Nair]] directs. #choir
"""),
        "Ideas/Poster series.md": (stamp(-9, "10:00"), """# Poster series: six bridges

One colour each, the same horizon line. The blue one first, says Tom.

Next: a page in the portfolio, then prints for the craft fair. #posters
"""),
        "Ideas/Craft fair.md": (stamp(-1, "18:30"), """# December craft fair

A shared stand with [[Tom Achebe]] again? Last year: posters on the left, his prints on the right.

- Prints: 30 of each bridge
- A small table, two chairs
#posters
"""),
        "Journal/This week.md": (stamp(0, "08:50"), """# This week

- Monday: the mock-up, slowly. The review at 14:30.
- Tuesday: the doctor at 9:30.
- Thursday: choir.
- Saturday: climbing with Jonas, if the weather holds.

One thing at a time.
"""),
        "Iris Calloway.md": (stamp(-6, "12:00"), """# Iris Calloway

Head librarian at Fernhill. Prefers calls in the morning, and short messages.

See [[Fernhill Library]].
"""),
        "Marc Duval.md": (stamp(-20, "12:00"), """# Marc Duval

Accountant, Villefranche-sur-Saône. The quarterly declaration, and the yearly one in spring.
"""),
        "Priya Nair.md": (stamp(-40, "12:00"), """# Priya Nair

Directs Two Rivers Voices. Rehearsals on Thursdays at 18:45.
"""),
        "Tom Achebe.md": (stamp(-40, "12:00"), """# Tom Achebe

Illustrator, Inkwell Studio. Lunch most Fridays. See [[Craft fair]].
"""),
    }
    for path, (moment, content) in notes.items():
        p.write(n / path, content, moment)

    # Projects and cases.
    p.write(n / "sioul-cases.toml", """# Projects and cases (docs/case-store.md, docs/projects.md).

[[case]]
id = "fernhill"
title = "Fernhill Library — website"
kind = "project"
client = "Fernhill Library"
rate = 55.0
budget = "work"
status = "open"
files = ["Projects/Fernhill Library.md", "Projects/Fernhill meeting 2026-09-29.md"]
[[case.route]]
from_domains = ["fernhill.example.com"]
[[case.route]]
from_addresses = ["notifications@forge.example.com"]
subject_contains = ["fernhill-site"]

[[case]]
id = "taxes-2026"
title = "Income tax 2026"
status = "waiting"
files = ["Admin/Income tax 2026.md"]
[[case.route]]
from_domains = ["tax-office.example.org"]
[[case.route]]
from_domains = ["duval-accounts.example.org"]

[[case]]
id = "choir"
title = "Winter concert"
area = "personal"
status = "open"
files = ["Choir/Winter concert.md"]
[[case.route]]
from_domains = ["choir.example.org"]
""")

    write_budgets(p, by_key)
    write_bank(p)
    write_contracts(p)
    write_papers(p)


def write_budgets(p: Profile, mail: dict):
    c = p.clock
    first = c.today.replace(day=1)
    paid_sep = paid_day(c)
    p.write(p.notes / "sioul-budgets.toml", f"""# Budgets, reserves and bank accounts (docs/accounting.md). Amounts in euros:
# positive in, negative out.

# Work pays for itself and for the rest: what it should leave each month.
[[budget]]
id = "work"
title = "Work"
period = "month"
target = 1250
area = "work"

# What keeps the flat and its people going: about this much a month.
[[budget]]
id = "household"
title = "Household duties"
period = "month"
target = -1050
area = "admin"

[[budget]]
id = "leisure"
title = "Leisure"
period = "month"
target = -130
area = "leisure"

# What comes back each month or year, and what is spent without a trace.
[[preset]]
id = "hosting"
budget = "work"
label = "Nimbus Hosting"
amount = -12
every = "month"
day = 10

[[preset]]
id = "software"
budget = "work"
label = "Design software"
amount = -24
every = "month"
day = 18

[[preset]]
id = "rent"
budget = "household"
label = "Rent"
amount = -620
every = "month"
day = 5

[[preset]]
id = "insurance"
budget = "household"
label = "Home insurance"
amount = -16.40
every = "month"
day = 8

[[preset]]
id = "phone"
budget = "household"
label = "Mobile phone"
amount = -19.99
every = "month"
day = 12

[[preset]]
id = "energy"
budget = "household"
label = "Brightwatt electricity"
amount = -64
every = "month"
day = 15

[[preset]]
id = "food"
budget = "household"
label = "Food (estimate)"
amount = -300
every = "month"
day = 31
estimate = true

[[preset]]
id = "gym"
budget = "leisure"
label = "Bloc & Boulder climbing gym"
amount = -35
every = "month"
day = 3

[[preset]]
id = "outings"
budget = "leisure"
label = "Outings and books (estimate)"
amount = -95
every = "month"
day = 31
estimate = true

[[preset]]
id = "choir-fee"
budget = "leisure"
label = "Choir fee"
amount = -90
every = "year"
month = 9
day = 20

# By hand: an invoice paid, and the two expected this month.
[[line]]
budget = "work"
date = {paid_sep}
amount = 990
label = "Invoice 2026-014, Fernhill Library"
links = ["sioul:invoice/2026-014", "sioul:case/fernhill"]

[[line]]
budget = "work"
date = {first.replace(day=20)}
amount = 720
label = "Invoice 2026-016, Silkworks Studio (illustrations), expected"
planned = true

[[line]]
budget = "work"
date = {first.replace(day=22)}
amount = 660
label = "Invoice 2026-015, Fernhill Library (mock-up), expected"
planned = true
links = ["sioul:case/fernhill"]

# Mail that becomes a line: proposed on the Budgets page.
[[mail_rule]]
budget = "household"
direction = "debit"
preset = "energy"
from_domains = ["brightwatt.example.org"]

[[mail_rule]]
budget = "household"
direction = "debit"
preset = "phone"
from_domains = ["wavecell.example.org"]

[[mail_rule]]
budget = "household"
direction = "credit"
from_domains = ["healthcover.example.org"]
subject_contains = ["refund"]

[[mail_rule]]
budget = "work"
direction = "debit"
preset = "hosting"
from_domains = ["nimbus.example.com"]
subject_contains = ["invoice"]

[[mail_rule]]
budget = "work"
direction = "debit"
from_domains = ["foundrynorth.example.com"]

# Savings: an instant one (money at once) and a long-term one (two weeks to come).
[[reserve]]
id = "instant"
title = "Instant savings"
balance = 3850
as_of = {first}
floor = 1000
delay_days = 0

[[reserve]]
id = "longterm"
title = "Long-term savings"
balance = 14200
as_of = {first}
floor = 5000
delay_days = 14

# Work's surplus goes to the long-term savings, which cover it in a lean month.
[[cover]]
reserve = "longterm"
budget = "work"
sweep = true

# Where the money is.
[[bank_account]]
id = "bank"
title = "Riverside Bank"
fills = ["household", "leisure", "work"]
floor = 200
topped_up_by = ["instant", "longterm"]

[[bank_account]]
id = "paypal"
title = "PayPal"
kind = "paypal"
fills = ["work", "leisure"]

# Where each movement goes.
[[split]]
words = ["brightwatt"]
preset = "energy"

[[split]]
words = ["wavecell"]
preset = "phone"

[[split]]
words = ["hillside lets"]
preset = "rent"

[[split]]
words = ["keystone insurance"]
preset = "insurance"

[[split]]
words = ["nimbus"]
preset = "hosting"

[[split]]
words = ["design software"]
preset = "software"

[[split]]
words = ["fernhill", "silkworks", "print"]
direction = "credit"
budget = "work"

[[split]]
words = ["bloc & boulder"]
preset = "gym"

[[split]]
words = ["two rivers voices"]
preset = "choir-fee"

[[split]]
words = ["cinema", "bookshop", "rail", "tidal notes"]
budget = "leisure"

[[split]]
words = ["market", "greengrocer", "bakery", "supermarket", "pharmacy"]
preset = "food"

[[split]]
words = ["health cover"]
direction = "credit"
budget = "household"

[[split]]
account = "paypal"
direction = "debit"
budget = "work"
""")


def write_bank(p: Profile):
    c = p.clock
    first = c.today.replace(day=1)
    months = [(first - timedelta(days=1)).replace(day=1), first]
    months.insert(0, (months[0] - timedelta(days=1)).replace(day=1))
    end = c.day(-2)
    rows = []

    def add(account, day, amount, label):
        if day <= end and day >= c.day(-62):
            rows.append((day, account, amount, label))

    for m in months:
        d = lambda n: m.replace(day=min(n, 28))
        add("bank", d(5), -620.00, "PRLV HILLSIDE LETS LOYER")
        add("bank", d(8), -16.40, "PRLV KEYSTONE INSURANCE HOME")
        add("bank", d(10), -12.00, "PRLV NIMBUS HOSTING")
        add("bank", d(12), -19.99, "PRLV WAVECELL MOBILE")
        add("bank", d(15), -61.80 if m != months[-1] else -64.20, "PRLV BRIGHTWATT")
        add("bank", d(3), -35.00, "PRLV BLOC & BOULDER")
        add("bank", d(18), -24.00, "PRLV DESIGN SOFTWARE SUBSCRIPTION")
        for day, amount, label in [(4, -42.30, "CB SUPERMARKET STRAND"), (9, -18.20, "CB GREENGROCER QUAI"), (13, -27.60, "CB MARKET HALL"),
                                   (17, -9.40, "CB BAKERY KIOSK"), (21, -51.10, "CB SUPERMARKET STRAND"), (25, -16.80, "CB MARKET HALL")]:
            add("bank", d(day), amount, label)
    # What does not come back.
    for offset, amount, label in [(-50, -9.50, "CB CINEMA STUDIO 9"), (-44, 660.00, "VIR FERNHILL LIBRARY FACT 2026-013"), (-31, -23.00, "CB RAIL TICKETS"),
                                  (-26, 450.00, "VIR SILKWORKS STUDIO FACT 2026-012"), (-24, -8.60, "CB PHARMACY CENTRAL"), (-20, -14.50, "CB BOOKSHOP LA PAGE"),
                                  (-13, 18.50, "VIR HEALTH COVER REFUND"), (-15, -90.00, "VIR TWO RIVERS VOICES CHOIR FEE")]:
        add("bank", c.weekday_before(offset), amount, label)
    rows.append((paid_day(c), "bank", 990.00, "VIR FERNHILL LIBRARY FACT 2026-014"))
    for offset, amount, label in [(-40, -9.00, "Tidal Notes music"), (-20, 25.00, "Payment from Ewan Price (print)"), (-4, -48.00, "Foundry North"),
                                  (-11, -12.00, "Paper & Ink"), (-33, 40.00, "Payment from Lise Moreau (print)")]:
        add("paypal", c.weekday_before(offset), amount, label)
    rows.sort()
    bank_balance = 1846.27
    paypal_balance = sum(r[2] for r in rows if r[1] == "paypal") + 61.50
    out = ["# Your bank's movements, read from its exports (docs/accounting.md). Kept here only.", "",
           "[[account]]", 'id = "bank"', 'title = "Riverside Bank"', f"balance = {bank_balance:.2f}", f"as_of = {end}", "",
           "[[account]]", 'id = "paypal"', 'title = "PayPal"', f"balance = {paypal_balance:.2f}", f"as_of = {end}", ""]
    for n, (day, account, amount, label) in enumerate(rows):
        out += ["[[movement]]", f'account = "{account}"', f"date = {day}", f"amount = {amount:.2f}", f"label = {toml_string(label)}", f'id = "demo-{n:03d}"', ""]
    p.write(p.notes / "sioul-bank.toml", "\n".join(out))


def write_contracts(p: Profile):
    c = p.clock
    year = c.today.year
    p.write(p.notes / "sioul-contracts.toml", f"""# Contracts and subscriptions (docs/accounting.md).

[[contract]]
id = "lease"
kind = "rent"
title = "Flat lease"
party = "Hillside Lets"
reference = "HL-2291"
preset = "rent"
started = 2024-03-01
renews = {year + 1}-03-01
every = "year"
notice_days = 30
cancel = "Hillside Lets, 4 Rue de l'Exemple, 69002 Lyon"
notes = "Furnished flat: one month's notice."

[[contract]]
id = "electricity"
kind = "energy"
title = "Electricity"
party = "Brightwatt"
reference = "BW-118204"
preset = "energy"
started = 2025-01-15
cancel = "https://brightwatt.example.org/account/close"

[[contract]]
id = "mobile"
kind = "telecom"
title = "Mobile phone"
party = "Wavecell"
reference = "WC-77310"
preset = "phone"
started = 2023-06-12
renews = {c.today.replace(day=12)}
every = "month"
notice_days = 10
cancel = "https://wavecell.example.org/cancel"

[[contract]]
id = "home-insurance"
kind = "insurance"
title = "Home insurance"
party = "Keystone Insurance"
reference = "LI-55-0192"
preset = "insurance"
started = 2024-03-01
renews = {year + 1}-03-01
every = "year"
notice_days = 30
covers = "Tenant's liability, water damage, legal protection."
paper = "home-insurance"
cancel = "Keystone Insurance, 7 Rue Fictive, 69003 Lyon"

[[contract]]
id = "hosting"
kind = "hosting"
title = "Web hosting"
party = "Nimbus Hosting"
reference = "NH-40112"
preset = "hosting"
started = 2022-02-10
renews = {c.today.replace(day=10)}
every = "month"
cancel = "https://nimbus.example.com/panel/close"

[[contract]]
id = "gym"
kind = "subscription"
title = "Climbing gym"
party = "Bloc & Boulder"
reference = "Member 2214"
preset = "gym"
started = 2025-12-01
renews = {year}-12-01
every = "year"
notice_days = 30
cancel = "https://bloc-boulder.example.org/membership"
""")


def write_papers(p: Profile):
    c = p.clock
    passport_until = c.day(74)
    papers = [
        ("passport", "passport", "Passport", "Passport.pdf", date(2016, 12, 21), passport_until, ["Passport", "Holder: Noa Ferrand", "Issued: 21 December 2016",
                                                                                              f"Valid until: {passport_until.strftime('%-d %B %Y')}"]),
        ("id-card", "identity", "Identity card", "Identity card.pdf", date(2021, 5, 10), date(2031, 5, 9), ["Identity card", "Holder: Noa Ferrand", "Valid until: 9 May 2031"]),
        ("health-card", "health-card", "Health card", "Health card.pdf", date(2019, 9, 2), None, ["Health card", "Holder: Noa Ferrand"]),
        ("health-cover", "health-cover", "Health cover certificate", "Health cover.pdf", date(c.today.year, 4, 1), date(c.today.year + 1, 3, 31),
         ["Health cover certificate", "Health cover office", f"Valid until: 31 March {c.today.year + 1}"]),
        ("home-insurance", "insurance", "Home insurance certificate", "Home insurance.pdf", date(2026, 3, 1), date(2027, 2, 28),
         ["Home insurance certificate", "Keystone Insurance", "Tenant's liability"]),
        ("tax-notice", "tax-notice", "Tax notice 2026 (2025 income)", "Tax notice 2026.pdf", c.weekday_before(-10), None, ["Tax notice 2026", "2025 income"]),
        ("rent-receipt", "rent-receipt", "Rent receipt, last month", "Rent receipt.pdf", c.today.replace(day=1) - timedelta(days=1), None,
         ["Rent receipt", "Hillside Lets", "Rent paid in full"]),
        ("bank-details", "bank-details", "Bank details (Riverside Bank)", "Bank details.pdf", date(2024, 3, 1), None, ["Bank details", "Riverside Bank",
                                                                                                               "IBAN FR76 0000 0000 0000 0000 0000 000"]),
        ("laptop", "warranty", "Laptop: proof of purchase", "Laptop invoice.pdf", date(2025, 2, 14), date(2027, 2, 13), ["Invoice", "Laptop, 14 inches", "Two years' warranty"]),
    ]
    out = ["# The papers wallet (docs/papers.md).", ""]
    for pid, kind, title, file, issued, until, text in papers:
        p.write(p.notes / "papers" / file, pdf(text + ["", "Specimen: invented for Sioul's demo."]), c.at(issued if issued <= c.today else c.today, "12:00"))
        out += ["[[paper]]", f'id = "{pid}"', f'kind = "{kind}"', f"title = {toml_string(title)}", f"file = {toml_string('papers/' + file)}", f"issued = {issued}"]
        if until:
            out.append(f"until = {until}")
        out += [f"added = {min(issued + timedelta(days=3), c.day(-20))}", ""]
    p.write(p.notes / "sioul-papers.toml", "\n".join(out))


# --- What Sioul keeps itself: time, invoices, health, weather, site news -------

def write_time(p: Profile):
    c = p.clock
    sessions = []

    def session(day: date, start: str, minutes: int, task="", project="", note="", invoice=""):
        moment = c.at(day, start)
        if moment + timedelta(minutes=minutes) > c.now:
            return
        sessions.append(dict(task=("demo-task-" + task) if task else "", project=project, start=unix(moment), minutes=minutes, note=note, invoice=invoice))

    # The first stage, billed (invoice 2026-014): eighteen hours over two weeks.
    first_stage = ["Kick-off at the library", "Site map", "Content inventory", "Typefaces", "Colours", "Wireframes", "Navigation",
                   "Accessibility review", "Revisions"]
    for note, offset in zip(first_stage, (-28, -27, -26, -25, -24, -21, -20, -19, -18)):
        session(c.monday + timedelta(days=offset), "09:30", 120, project="fernhill", note=note, invoice="2026-014")
    # The mock-up, not billed yet; the portfolio, for no one. A task's sessions
    # carry no note but the last one, where you stopped.
    for offset, start, minutes, task, note in [
        (-14, "09:10", 90, "photos", ""), (-13, "10:00", 60, "", "Meeting at the library"), (-13, "14:20", 45, "", "Site map, second version"),
        (-12, "09:30", 80, "", "Content of the new pages"), (-11, "14:00", 45, "hours-page", ""), (-11, "15:00", 40, "portfolio", ""),
        (-10, "09:15", 75, "", "Typefaces and colours, again"),
        (-7, "09:05", 50, "", "Accessibility notes"), (-7, "14:10", 35, "portfolio", ""), (-6, "10:00", 30, "", "Call with Oskar"),
        (-6, "14:15", 60, "photos", ""), (-5, "09:20", 70, "", "Navigation"), (-4, "09:00", 80, "sketch", ""), (-4, "14:30", 30, "portfolio", ""),
        (-3, "10:10", 45, "", "Notes for the review"),
    ]:
        session(c.monday + timedelta(days=offset), start, minutes, task=task, project="" if task else "fernhill", note=note)
    for offset in range(0, (c.today - c.monday).days):
        session(c.monday + timedelta(days=offset), "14:15", 50, project="fernhill", note="Homepage, details")
    session(c.today, "11:20", 40, task="sketch", note="the left layout's header, with the serif title")

    by_month: dict[str, list] = {}
    for s in sessions:
        month = datetime.fromtimestamp(s["start"], c.zone).strftime("%Y-%m")
        by_month.setdefault(month, []).append(s)
    for month, items in by_month.items():
        out = []
        for s in sorted(items, key=lambda s: s["start"]):
            out.append("[[session]]")
            if s["task"]:
                out.append(f'task = "{s["task"]}"')
            if s["project"]:
                out.append(f'project = "{s["project"]}"')
            if s["invoice"]:
                out.append(f'invoice = "{s["invoice"]}"')
            out += [f"start = {s['start']}", f"minutes = {s['minutes']}", "done = false"]
            if s["note"]:
                out.append(f"note = {toml_string(s['note'])}")
            out.append("")
        p.write(p.data / "time" / f"{month}.toml", "\n".join(out))
    return sessions


def invoice_day(c: Clock) -> date:
    """The day invoice 2026-014 was made: the Friday after its last session."""
    return c.monday - timedelta(days=17)


def paid_day(c: Clock) -> date:
    """The day the client paid it, ten days later (a weekday)."""
    d = invoice_day(c) + timedelta(days=10)
    while d.weekday() >= 5:
        d += timedelta(days=1)
    return d


def write_invoices(p: Profile, sessions: list):
    c = p.clock
    billed = [s for s in sessions if s["invoice"] == "2026-014"]
    day = invoice_day(c)
    lines = []
    for s in billed:
        cents = round(s["minutes"] / 60 * 55 * 100)
        lines.append(f'[[lines]]\nlabel = {toml_string(s["note"])}\nminutes = {s["minutes"]}\nrate = 55.0\ncents = {cents}\n')
    total = sum(round(s["minutes"] / 60 * 55 * 100) for s in billed)
    keys = [f'{s["start"]}:{s["project"]}:{s["minutes"]}' for s in billed]
    p.write(p.data / "invoices" / "2026-014.toml", f"""number = "2026-014"
date = "{day}"
due = "{day + timedelta(days=30)}"
project = "fernhill"
project_title = "Fernhill Library — website"
client = "Fernhill Library"
client_address = "2 Rue de l'Exemple\\n69006 Lyon"
total_cents = {total}
currency = "EUR"
sessions = {toml_list(keys)}
paid = true

[issuer]
name = "Noa Ferrand"
address = "8 Rue de l'Exemple\\n69004 Lyon"
siret = "000 000 000 00000"
vat = "VAT not applicable, art. 293 B of the French tax code"
payment = "IBAN FR76 0000 0000 0000 0000 0000 000"

""" + "\n".join(lines))


def write_health(p: Profile):
    c = p.clock
    p.write(p.data / "health.toml", f"""errands_list = "{DAV}/home-tasks"

[[prescription]]
id = "levothyroxine"
title = "Levothyroxine 75 µg"
prescriber = "Dr Elena Varga"
until = {c.day(118)}
refill_days = 28
last_refill = {c.day(-9)}

[[medicine]]
id = "levothyroxine"
name = "Levothyroxine"
dose = "75 µg"
prescription = "levothyroxine"
schedule = {{ every = "day", times = ["07:30"] }}

[[medicine]]
id = "magnesium"
name = "Magnesium"
dose = "1 tablet"
schedule = {{ every = "day", times = ["12:30", "20:00"] }}
until = {c.day(20)}

[movement]
enabled = true
minutes = 45

[chats]
enabled = true
minutes = 45
locked_minutes = 60
""")
    taken = []
    reminded = []
    doses = [(-1, "levothyroxine", "07:30", 9), (-1, "magnesium", "12:30", 20), (-1, "magnesium", "20:00", 35),
             (0, "levothyroxine", "07:30", 11), (0, "magnesium", "12:30", 14), (0, "magnesium", "20:00", 10)]
    for offset, medicine, hhmm, late in doses:
        moment = c.at(c.day(offset), hhmm)
        if moment + timedelta(minutes=30) < c.now:
            taken.append(f'"{medicine}@{unix(moment)}" = {unix(moment + timedelta(minutes=late))}')
            reminded.append(f'"{medicine}@{unix(moment)}" = {unix(moment)}')
    p.write(p.state / "health-state.toml", "[taken]\n" + "\n".join(taken) + "\n\n[reminded]\n" + "\n".join(reminded) + "\n")


def write_weather(p: Profile):
    c = p.clock
    hour0 = c.now.replace(minute=0, second=0)
    hours = []
    for i in range(40):
        moment = hour0 + timedelta(hours=i)
        h = moment.hour
        day = 8 <= h < 19
        # A soft autumn day: clouds, a shower in the afternoon, a clear night.
        temperature = 11 + 4.5 * max(0.0, 1 - abs(h - 15) / 8) + (0.5 if i > 24 else 0)
        code, rain = (2, 10) if day else (1, 0)
        if 15 <= h <= 17 and i < 24:
            code, rain = 61, 40
        elif 10 <= h <= 12:
            code, rain = 3, 15
        hours.append({"at": unix(moment), "temperature": round(temperature, 1), "rain": rain, "code": code, "day": day})
    p.write(p.state / "weather.json", json.dumps({"fetched": unix(c.now), "hours": hours}))


def write_site_news(p: Profile):
    c = p.clock
    p.write(p.state / "site-notices.toml", f"""[[notice]]
site = "chat"
title = "Tom Achebe"
text = "Shall we print the posters at the same place as last year?"
at = {unix(c.ago(hours=1, minutes=5))}

[[notice]]
site = "chat"
title = "Studio friends"
text = "Lise: the scanner at the co-working space works again."
at = {unix(c.ago(minutes=40))}

[[notice]]
site = "bank"
title = "Riverside Bank"
text = "A new document in your secure space."
at = {unix(c.at(c.weekday_before(-3), "08:10"))}
""")


# --- The configuration --------------------------------------------------------

WEEKDAYS = ("monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday")


def write_config(p: Profile):
    # Admin on Tuesday and Thursday evenings, and on the day the profile is made
    # for, so that its day shows the three kinds of hours.
    admin_days = [d for d in WEEKDAYS[:5] if d in ("tuesday", "thursday") or (WEEKDAYS.index(d) == p.clock.today.weekday())]
    hours = """
# The week's hours (docs/areas.md): work, then your own admin, then free time.
# Hours set for none of them are personal time. With no hours at all, the Porch
# is always open and asks for them.
""" + "".join(f"""
[[window]]
day = "{day}"
start = "{start}"
end = "{end}"
""" for day in WEEKDAYS[:5] for start, end in (("09:00", "12:00"), ("14:00", "17:00"))) + "".join(f"""
[[window]]
day = "{day}"
start = "17:00"
end = "18:30"
kind = "admin"
""" for day in admin_days) + "".join(f"""
[[window]]
day = "{day}"
start = "{start}"
end = "{end}"
kind = "leisure"
""" for day, start, end in [(d, "18:30", "21:00") for d in WEEKDAYS[:5]] + [("saturday", "10:00", "18:00")])
    p.write(p.config / "config.toml", f"""# Sioul's demo profile: an invented life, made by tools/demo/make-demo.py.
# Nothing here is real: the people, the companies, the messages. Run Sioul on it
# with SIOUL_DEMO=1, which keeps it off the network.

language = "en"

# The folder of notes, beside them the projects, budgets, the bank, the papers.
case_store = {toml_string(p.notes_at)}

# Minutes for tasks on each weekday, Monday first.
task_minutes = [120, 120, 120, 120, 120, 0, 0]

[tasks]
list = "{DAV}/home-tasks"

[agenda]
day_start = 8

[map]
geocode = false

[weather]
place = "Lyon"
latitude = 45.76
longitude = 4.84

[reminders]
events = true
asked_days = 2
payment_days = 2
waits = true
gather = true
gathered = ["09:00", "13:00", "18:00"]

[invoice]
name = "Noa Ferrand"
address = "8 Rue de l'Exemple\\n69004 Lyon"
siret = "000 000 000 00000"
vat = "VAT not applicable, art. 293 B of the French tax code"
prefix = "2026-"
rate = 50.0
currency = "EUR"
payment = "IBAN FR76 0000 0000 0000 0000 0000 000, within 30 days."

# The camera, microphone and speaker of calls in sites: the system's own.
[calls]
{hours if p.hours else ""}
# Time off: quiet from the first day to the last.
[[time_off]]
from = {p.clock.today.year}-12-21
until = {p.clock.today.year + 1}-01-03
label = "Winter holidays"

# Work: a freelance web designer's own address.
[[account]]
id = "work"
kind = "imap"
address = "{WORK}"
name = "Noa Ferrand"
signature = "Noa Ferrand  \\nWeb design and illustration, Lyon"
area = "work"
host = "imap.example.invalid"
port = 993
security = "tls"
smtp_host = "smtp.example.invalid"
smtp_port = 465
trusted_authserv_ids = ["{PROVIDER}"]

# Everything personal: admin and leisure.
[[account]]
id = "personal"
kind = "imap"
address = "{HOME}"
name = "Noa Ferrand"
signature = "Noa"
area = "admin+leisure"
host = "imap.example.invalid"
port = 993
security = "tls"
smtp_host = "smtp.example.invalid"
smtp_port = 465
trusted_authserv_ids = ["{PROVIDER}"]

# Calendars, tasks and contacts of the personal address.
[[account]]
id = "{DAV}"
kind = "dav"
address = "{HOME}"
host = "dav.example.invalid"
url = "{DAV_URL}"

# Sites: web-only mailboxes, a chat, video calls (docs/sites.md).
[[site]]
id = "bank"
name = "Riverside Bank"
url = "https://secure.riversidebank.example.org/"
site = "mailbox"
categories = ["Bank"]
area = "admin"
announced_by = ["riversidebank.example.org"]

[[site]]
id = "taxes"
name = "Tax office"
url = "https://my.tax-office.example.org/"
site = "mailbox"
categories = ["Taxes"]
area = "admin"
announced_by = ["tax-office.example.org"]

[[site]]
id = "healthcover"
name = "Health cover"
url = "https://members.healthcover.example.org/"
site = "mailbox"
categories = ["Health"]
area = "admin"
announced_by = ["healthcover.example.org"]

[[site]]
id = "chat"
name = "Studio chat"
url = "https://chat.example.com/"
site = "chat"
categories = ["Friends", "Work"]
area = "work+leisure"
background = false
announced_by = ["chat.example.com"]

[[site]]
id = "meet"
name = "Video calls"
url = "https://meet.example.com/noa"
site = "video"
categories = ["Work"]
area = "work"
announced_by = ["meet.example.com"]
""")
    p.write(p.config / "known-senders.txt", """# Senders let in: they skip the screener.
@fernhill.example.com
tom@inkwell.example.com
jonas.bell@example.org
marc@duval-accounts.example.org
@choir.example.org
sophie.marchand@riversidebank.example.org
""")
    p.write(p.config / "safe-senders.txt", """# Safe: their mail reaches you at any hour.
maud.ferrand@example.org
camille.ferrand@example.org
hugo.ferrand@example.org
*@choir.example.org
""")
    p.write(p.config / "neutral-senders.txt", """# Neutral within a safe domain: the choir's automatic mail waits for its hours.
noreply@choir.example.org
""")
    p.write(p.config / "blocked-senders.txt", """# Blocked: set aside for good, never shown.
*@deals-today.example.com
promo@shiny-gadgets.example.com
""")


# --- All of it ----------------------------------------------------------------

def prepare(root: Path):
    if root.exists():
        entries = [e for e in root.iterdir()]
        if entries and not (root / MARK).exists():
            sys.exit(f"{root}: not empty, and not a demo profile made by this script; nothing written.")
        for entry in entries:
            if entry.is_dir() and not entry.is_symlink():
                shutil.rmtree(entry)
            else:
                entry.unlink()
    root.mkdir(parents=True, exist_ok=True)
    (root / MARK).write_text("A demo profile made by tools/demo/make-demo.py; it may be emptied and made again.\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--into", required=True, type=Path, help="the profile's folder")
    parser.add_argument("--now", help="the moment the profile is made for, ISO 8601 with its offset (default: now)")
    parser.add_argument("--no-hours", action="store_true", help="leave the week's hours out: the Porch asks for them")
    parser.add_argument("--notes-at", help="where Sioul finds DIR/notes when it runs (default: DIR/notes)")
    args = parser.parse_args()
    now = datetime.fromisoformat(args.now) if args.now else datetime.now().astimezone()
    if now.tzinfo is None:
        now = now.astimezone()
    root = args.into.resolve()
    prepare(root)
    clock = Clock(now)
    profile = Profile(root, clock, args.notes_at or str(root / "notes"), not args.no_hours)
    mails = the_mail(clock)
    write_config(profile)
    write_mail(profile, mails)
    write_contacts(profile)
    write_calendars(profile, mails)
    write_notes(profile, mails)
    sessions = write_time(profile)
    write_invoices(profile, sessions)
    write_health(profile)
    write_weather(profile)
    write_site_news(profile)
    print(f"{root}: a demo profile for {clock.now.isoformat()}" + ("" if profile.hours else ", without hours"))


if __name__ == "__main__":
    main()
