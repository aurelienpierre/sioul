#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre
"""A demo profile for Sioul: an invented, calm and lived-in life, in English
or in French, for trying Sioul and for the documentation's screenshots.

    make-demo.py --into DIR [--now 2026-10-05T14:00+02:00] [--language en|fr] [--no-hours] [--notes-at PATH] [--spam] [--calls]

Writes DIR/config, DIR/data, DIR/state and DIR/cache (the four XDG folders:
run Sioul with XDG_CONFIG_HOME=DIR/config, XDG_DATA_HOME=DIR/data,
XDG_STATE_HOME=DIR/state, XDG_CACHE_HOME=DIR/cache and SIOUL_DEMO=1, which
keeps it off the network), and DIR/notes, the notes folder.

Everything is invented: the people, the companies, the messages. Addresses are
on example.org, example.com, .example, .test and .invalid hosts (RFC 2606), phone numbers in the
ranges the French regulator keeps for fiction (04 65 71, and 06 39 98,
which is Mayotte's: +262 6 39 98). Nothing is
random: the same --now gives the same files. Dates are laid around --now (by
default, now): mail of the last two weeks, a one-time code from two minutes
ago, this week's events, two months of bank movements.

--language fr writes the same life in French, thing for thing (the same
people, under French names where theirs were English; the same messages,
tasks, notes and money), with Sioul in French.
--no-hours leaves the working hours out, so the Porch asks for them.
--notes-at says where Sioul will find DIR/notes when it runs in a sandbox that
mounts it elsewhere (tools/demo/screenshots.sh mounts it at /home/demo/Notes).
--spam (or SIOUL_DEMO_SPAM=1 in the environment) adds Sioul's own spam filter:
a table made by hand, two strangers' messages it flags and one it moved into
the Junk folder, waiting in the Porch's review queue (docs/spam-filter.md).
--calls (or SIOUL_DEMO_CALLS=1) adds the calls a phone of yours screened, as
the sharing brings its log to a computer: the Porch lists those it declined,
and the doctor's sheet her calls of the month (docs/porch.md, "Calls declined").
--phone-messages (or SIOUL_DEMO_PHONE_MESSAGES=1) adds the messages a phone's
notifications brought, as the sharing brings its log to a computer, with the
part "Messages from your phone" on here: the Porch's "From your phone" says
them, and the doctor's sheet her messages of the week (docs/porch.md, "From
your phone").
--texts (or SIOUL_DEMO_TEXTS=1) adds the texts a phone read and those written
here for it to send, as the sharing's part "Texts" brings them, written plain
and marked as the demo's stand-ins (crates/sioul-app/src/texts.rs, `Seal`):
the Texts page lists their conversations and says each text's state
(docs/texts.md).
--compose (or SIOUL_DEMO_COMPOSE=1) adds a message to answer on a phone's
width: a subject of more than sixty characters, replies asked to two
addresses, a copy to a third (SIOUL_GRAB_STEPS=compose, docs/building.md).
--doses-asked (or SIOUL_DEMO_DOSES_ASKED=1) leaves today's two first doses
unanswered: the morning's as due while Sioul was closed, the noon's as due
while Sioul ran but its reminder could not be shown; the Porch and the Health
page ask about each under its own question (docs/health.md).

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

# The language of the life written: "en" or "fr" (set by --language).
LANG = "en"


def typo(text: str) -> str:
    """French typography, as Sioul's French strings write it: ’ for the
    apostrophe, a narrow no-break space before : ; ? ! and inside « », a
    no-break space before € and after n°."""
    text = text.replace("'", "\u2019")
    for mark in ":;?!":
        text = text.replace(" " + mark, "\u202f" + mark)
    text = text.replace("« ", "«\u202f").replace(" »", "\u202f»")
    return text.replace(" €", "\u00a0€").replace("n° ", "n°\u00a0")


def t(en: str, fr: str) -> str:
    """The text in the language of the profile."""
    return typo(fr) if LANG == "fr" else en


def street(address: str) -> str:
    """A street as each language writes it: "2 Rue de l'Exemple", "2 rue de l’Exemple"."""
    if LANG != "fr":
        return address
    number, kind, rest = address.split(" ", 2)
    return typo(f"{number} {kind.lower()} {rest}")


MONTHS_FR = ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"]


def long_date(day: date, year: bool = False) -> str:
    """"25 September" or "25 septembre", "1er" for the first in French."""
    if LANG == "fr":
        text = f"{'1er' if day.day == 1 else day.day} {MONTHS_FR[day.month - 1]}"
    else:
        text = f"{day.day} {day.strftime('%B')}"
    return f"{text} {day.year}" if year else text


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
    """A one-page A4 PDF with a title and a few lines, in Helvetica (Windows-1252 text)."""
    def escape(text: str) -> str:
        return text.replace("\\", "\\\\").replace("(", "\\(").replace(")", "\\)")

    parts = ["BT", "/F1 20 Tf", "60 770 Td", f"({escape(lines[0])}) Tj", "/F1 12 Tf"]
    for text in lines[1:]:
        parts += ["0 -22 Td", f"({escape(text)}) Tj"]
    parts += ["ET", "0.75 G", "50 700 m 545 700 l S"]
    stream = "\n".join(parts).replace("\u202f", "\u00a0").encode("cp1252")
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

class World:
    """Who and what, in the language of the profile: names, domains, places."""

    def __init__(self):
        fr = LANG == "fr"
        pick = lambda en, f: typo(f) if fr else en
        # Domains of the people and companies.
        self.library = pick("fernhill.example.com", "fougeres.example.com")
        self.inkwell = pick("inkwell.example.com", "encrier.example.com")
        self.coop = pick("greenfield-coop.example.com", "champs-verts.example.com")
        self.foundry = pick("foundrynorth.example.com", "fonderienord.example.com")
        self.letter = pick("typeandpixels.example.com", "lettresetpixels.example.com")
        self.prizes = pick("lucky-prizes.example.com", "gros-lots.example.com")
        self.paper = pick("paperandink.example.org", "papierencre.example.org")
        self.parcels = pick("parcels.example.org", "colis.example.org")
        self.choir = pick("choir.example.org", "choeur.example.org")
        self.bank = pick("riversidebank.example.org", "banquedesberges.example.org")
        self.energy = pick("brightwatt.example.org", "clairwatt.example.org")
        self.club = pick("climbing-club.example.org", "club-escalade.example.org")
        self.phone = pick("wavecell.example.org", "ondea.example.org")
        self.health = pick("healthcover.example.org", "caisse-sante.example.org")
        self.tax = pick("tax-office.example.org", "impots.example.org")
        self.accounts = pick("duval-accounts.example.org", "cabinet-duval.example.org")
        self.deals = pick("deals-today.example.com", "bonnes-affaires.example.com")
        self.gadgets = pick("shiny-gadgets.example.com", "gadgets-brillants.example.com")
        # Names.
        self.library_name = pick("Fernhill Library", "Médiathèque des Fougères")
        self.project_title = pick("Fernhill Library — website", "Médiathèque des Fougères — site web")
        self.repository = pick("fernhill-site", "site-fougeres")
        self.iris = (pick("Iris Calloway", "Iris Calvet"), f"iris@{self.library}")
        self.oskar = (pick("Oskar Lind", "Oscar Lindet"), f"{pick('oskar', 'oscar')}@{self.library}")
        self.tom = (pick("Tom Achebe", "Tom Achard"), f"tom@{self.inkwell}")
        self.jonas = (pick("Jonas Bell", "Jonas Belin"), pick("jonas.bell@example.org", "jonas.belin@example.org"))
        self.marc = ("Marc Duval", f"marc@{self.accounts}")
        self.nimbus = pick("Nimbus Hosting", "Nimbus Hébergement")
        self.bank_name = pick("Riverside Bank", "Banque des Berges")
        self.energy_name = pick("Brightwatt", "Clairwatt")
        self.phone_name = pick("Wavecell", "Ondéa Mobile")
        self.health_name = pick("Health cover office", "Caisse santé")
        self.tax_name = pick("Tax office", "Centre des impôts")
        self.choir_name = pick("Two Rivers Voices", "Chœur des Deux Rivières")
        self.lets = pick("Hillside Lets", "Agence des Coteaux")
        self.insurer = pick("Keystone Insurance", "Assurances Clé de Voûte")
        self.gym = pick("Bloc & Boulder", "Bloc et Prise")
        self.silk = pick("Silkworks Studio", "Studio La Soierie")
        self.paper_name = pick("Paper & Ink", "Papier & Encre")
        # The notes of the library's project, by their place in the folder of notes.
        self.project_note = pick("Projects/Fernhill Library.md", "Projets/Médiathèque des Fougères.md")
        self.meeting_note = pick("Projects/Fernhill meeting 2026-09-29.md", "Projets/Réunion Fougères 2026-09-29.md")
        self.sketch = pick("Projects/homepage-sketch.png", "Projets/croquis-accueil.png")
        self.tax_note = pick("Admin/Income tax 2026.md", "Administratif/Impôt sur le revenu 2026.md")
        self.choir_note = pick("Choir/Winter concert.md", "Chorale/Concert d'hiver.md")


W: World = World()


# People and offices, by key: name, addresses, phones, organisation, title,
# postal address (street, code, town), birthday, notes, categories, place (lat, lon).
def contacts() -> dict:
    return {
        "iris": dict(name=W.iris[0], first="Iris", last=W.iris[0].split()[1], email=[("work", W.iris[1])], phone=[("work", "+33 4 65 71 20 14")],
                     org=W.library_name, title=t("Head librarian", "Directrice de la médiathèque"), adr=(street("2 Rue de l'Exemple"), "69006", "Lyon"), place=(45.7765, 4.8540),
                     note=t("The library's new website: Iris decides; Oskar does the technical side.",
                            "Le nouveau site de la médiathèque : Iris décide ; Oscar s'occupe de la technique."),
                     url=f"https://{W.library}", cats=["clients"]),
        "oskar": dict(name=W.oskar[0], first=W.oskar[0].split()[0], last=W.oskar[0].split()[1], email=[("work", W.oskar[1])], phone=[("work", "+33 4 65 71 20 15")],
                      org=W.library_name, title=t("IT and digital services", "Informatique et services numériques"), adr=(street("2 Rue de l'Exemple"), "69006", "Lyon"),
                      place=(45.7765, 4.8540), cats=["clients"]),
        "tom": dict(name=W.tom[0], first="Tom", last=W.tom[0].split()[1], email=[("work", W.tom[1])], phone=[("cell", "+262 6 39 98 41 07")],
                    org=t("Inkwell Studio", "Atelier L'Encrier"), title=t("Illustrator", "Illustrateur"), adr=(street("9 Quai des Exemples"), "69001", "Lyon"), place=(45.7675, 4.8337),
                    cats=[t("colleagues", "collègues")], note=t("Shares the stand at the December craft fair.", "Partage le stand du salon des créateurs en décembre.")),
        "maud": dict(name="Maud Ferrand", first="Maud", last="Ferrand", email=[("home", "maud.ferrand@example.org")], phone=[("cell", "+262 6 39 98 12 30")],
                     adr=(street("14 Allée Fictive"), "69100", "Villeurbanne"), place=(45.7676, 4.8800), bday="--03-21", cats=[t("family", "famille")], note=t("Mum.", "Maman.")),
        "camille": dict(name="Camille Ferrand", first="Camille", last="Ferrand", email=[("home", "camille.ferrand@example.org")], phone=[("cell", "+262 6 39 98 55 02")],
                        adr=(street("31 Rue Imaginaire"), "38000", "Grenoble"), place=(45.1916, 5.7281), bday="--07-02", cats=[t("family", "famille")]),
        "hugo": dict(name="Hugo Ferrand", first="Hugo", last="Ferrand", email=[("home", "hugo.ferrand@example.org")], phone=[("cell", "+262 6 39 98 70 19")],
                     adr=(street("6 Passage Inventé"), "42000", "Saint-Étienne"), place=(45.4397, 4.3872), bday="--10-14", cats=[t("family", "famille")]),
        "jonas": dict(name=W.jonas[0], first="Jonas", last=W.jonas[0].split()[1], email=[("home", W.jonas[1])], phone=[("cell", "+262 6 39 98 33 84")],
                      adr=(street("18 Rue de l'Exemple"), "69007", "Lyon"), place=(45.7457, 4.8423), cats=[t("friends", "amis")],
                      note=t("Climbing on Saturdays.", "Escalade le samedi.")),
        "varga": dict(name="Dr Elena Varga", first="Elena", last="Varga", email=[], phone=[("work", "+33 4 65 71 47 70")],
                      org=t("Riverside Health Centre", "Centre de santé des Berges"), title=t("General practitioner", "Médecin généraliste"),
                      adr=(street("5 Rue Imaginaire"), "69003", "Lyon"), place=(45.7606, 4.8590),
                      note=t("Appointments by phone, mornings.", "Rendez-vous par téléphone, le matin.")),
        "marc": dict(name="Marc Duval", first="Marc", last="Duval", email=[("work", W.marc[1])], phone=[("work", "+33 4 65 71 63 25")],
                     org=t("Duval Accounts", "Cabinet Duval"), title=t("Accountant", "Expert-comptable"), adr=(street("11 Place des Exemples"), "69400", "Villefranche-sur-Saône"),
                     place=(45.9900, 4.7180)),
        "priya": dict(name="Priya Nair", first="Priya", last="Nair", email=[("home", f"priya@{W.choir}")], phone=[("cell", "+262 6 39 98 90 46")],
                      org=t("Two Rivers Voices choir", "Chœur des Deux Rivières"), title=t("Choir director", "Cheffe de chœur"), adr=(street("3 Impasse Fictive"), "69004", "Lyon"),
                      place=(45.7745, 4.8320), cats=[t("friends", "amis")]),
        "healthcover": dict(name=W.health_name, email=[("work", f"members@{W.health}")], phone=[("work", "+33 4 65 71 80 00")],
                            org=W.health_name, adr=(street("20 Boulevard de l'Exemple"), "69003", "Lyon"), place=(45.7608, 4.8458),
                            note=t("Open Monday to Friday, 9:00 to 12:00 and 14:00 to 16:30.", "Ouvert du lundi au vendredi, de 9 h à 12 h et de 14 h à 16 h 30.")),
        "sophie": dict(name="Sophie Marchand", first="Sophie", last="Marchand", email=[("work", f"sophie.marchand@{W.bank}")], phone=[("work", "+33 4 65 71 30 52")],
                       org=W.bank_name, title=t("Account adviser", "Conseillère clientèle"), adr=(street("1 Cours Imaginaire"), "69002", "Lyon"), place=(45.7578, 4.8320)),
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
        lines.append("NOTE:" + c["note"].replace(",", "\\,").replace(";", "\\;"))
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
    iris, oskar, tom, jonas, marc = W.iris, W.oskar, W.tom, W.jonas, W.marc
    me_work = (ME, WORK)
    me_home = (ME, HOME)
    after_review = c.ago(minutes=55) >= c.at(c.today, "11:15")
    mails = [
        # Work: noa@example.com.
        Mail("work", c.ago(minutes=2), (W.nimbus, "no-reply@nimbus.example.com"), t("Your sign-in code", "Votre code de connexion"), t(
            """Hello Noa,

Your sign-in code is 482 913.

It is valid for 15 minutes. If you did not try to sign in to your Nimbus Hosting panel, you can ignore this message: nobody can sign in without the code.

Nimbus Hosting""",
            """Bonjour Noa,

Votre code de connexion est 482 913.

Il est valable 15 minutes. Si vous n'avez pas essayé de vous connecter à votre espace Nimbus Hébergement, ignorez ce message : personne ne peut se connecter sans ce code.

Nimbus Hébergement"""), key="code"),
        Mail("work", c.ago(minutes=55), iris, t("Homepage: two small changes", "Page d'accueil : deux petites modifications"), (t(
            """Hello Noa,

Thank you for this morning's review: the whole team likes the calm layout, the one with the large photo of the reading room.

The two small changes we spoke about, so that you have them in writing:
""",
            """Bonjour Noa,

Merci pour la revue de ce matin : toute l'équipe aime la mise en page calme, celle avec la grande photo de la salle de lecture.

Les deux petites modifications dont nous avons parlé, pour que vous les ayez par écrit :
""") if after_review else t(
            """Hello Noa,

The whole team likes the calm layout, the one with the large photo of the reading room. Before our review this morning, two small changes:
""",
            """Bonjour Noa,

Toute l'équipe aime la mise en page calme, celle avec la grande photo de la salle de lecture. Avant notre revue de ce matin, deux petites modifications :
""")) + t(
            """
1. The opening hours right under the photo, rather than at the bottom: most people come to the site for them.
2. The children's corner in the menu, as its own entry.

No hurry at all.

Best,
Iris

--
Iris Calloway
Head librarian, Fernhill Library
2 Rue de l'Exemple, 69006 Lyon""",
            """
1. Les horaires juste sous la photo plutôt qu'en bas de page : la plupart des gens viennent sur le site pour eux.
2. Le coin des enfants dans le menu, comme une entrée à part.

Rien ne presse.

Bien à vous,
Iris

--
Iris Calvet
Directrice, Médiathèque des Fougères
2 rue de l'Exemple, 69006 Lyon"""), key="homepage"),
        Mail("work", c.ago(hours=2, minutes=10), ("Hana Okafor", f"hana@{W.coop}"), t("A small website for our cooperative?", "Un petit site pour notre coopérative ?"), t(
            """Dear Noa,

Iris Calloway at Fernhill Library gave me your address. We are a small cooperative of market gardeners near Lyon, and our website has not changed since 2014.

Would you have time this winter for a simple site: who we are, our market days, how to order a basket? Could you tell me roughly what it would cost?

With thanks,
Hana Okafor
Greenfield Cooperative""",
            """Bonjour Noa,

Iris Calvet, de la Médiathèque des Fougères, m'a donné votre adresse. Nous sommes une petite coopérative de maraîchers près de Lyon, et notre site n'a pas changé depuis 2014.

Auriez-vous du temps cet hiver pour un site simple : qui nous sommes, nos jours de marché, comment commander un panier ? Pourriez-vous me dire à peu près ce que cela coûterait ?

Merci d'avance,
Hana Okafor
Coopérative des Champs Verts"""), key="hana"),
        Mail("work", c.ago(hours=20), tom, t("Shared stand at the craft fair?", "Un stand partagé au salon des créateurs ?"), t(
            """Hi Noa,

The craft fair in December has a few double stands left. Shall we share one again? Your posters and my prints worked well side by side last year.

They want an answer by the end of the week. No pressure, just say yes or no when you can.

Tom""",
            """Salut Noa,

Il reste quelques stands doubles au salon des créateurs de décembre. On en partage un à nouveau ? Tes affiches et mes gravures allaient bien ensemble l'an dernier.

Ils veulent une réponse d'ici la fin de la semaine. Pas de pression, dis-moi oui ou non quand tu peux.

Tom"""), key="fair"),
        Mail("work", c.ago(hours=23, minutes=30), (t("Code forge", "Forge de code"), "notifications@forge.example.com"),
             t(f"[{W.repository}] Pull request merged: calmer footer", f"[{W.repository}] Demande de fusion acceptée : pied de page plus calme"), t(
            f"""Oskar Lind merged pull request #14 into main.

calmer footer: one line, the address and the opening hours

--
You receive this because you are watching {W.repository}.""",
            f"""Oscar Lindet a fusionné la demande n° 14 dans main.

pied de page plus calme : une ligne, l'adresse et les horaires

--
Vous recevez ce message parce que vous suivez {W.repository}."""), key="forge"),
        Mail("work", c.ago(hours=26), (t("Prize Department", "Service des gains"), f"{t('win', 'gagnant')}@{W.prizes}"),
             t("Congratulations! You have won a cruise", "Félicitations ! Vous avez gagné une croisière"), t(
            """CONGRATULATIONS!!!

You have been selected to receive a FREE cruise for two. Claim your prize within 24 hours by sending your bank details.""",
            """FÉLICITATIONS !!!

Vous avez été sélectionné(e) pour une croisière GRATUITE pour deux. Réclamez votre lot sous 24 heures en envoyant vos coordonnées bancaires."""),
             auth="spam", key="spam"),
        Mail("work", c.ago(hours=28), (t("Type & Pixels", "Lettres & Pixels"), f"{t('letter', 'lettre')}@{W.letter}"),
             t("Type & Pixels #112: quiet layouts", "Lettres & Pixels n° 112 : des mises en page calmes"), t(
            """This week: quiet layouts.

- Why white space is not empty space
- Three typefaces for public libraries
- A reader's question: how large should body text be on a phone?

You receive Type & Pixels because you subscribed on typeandpixels.example.com.""",
            """Cette semaine : des mises en page calmes.

- Pourquoi le blanc n'est pas du vide
- Trois polices pour les bibliothèques publiques
- La question d'une lectrice : quelle taille pour le texte sur un téléphone ?

Vous recevez Lettres & Pixels parce que vous vous êtes abonné(e) sur lettresetpixels.example.com."""), kind="list", key="news"),
        Mail("work", c.at(c.weekday_before(-2), "16:20"), oskar, t("Photos of the reading room", "Photos de la salle de lecture"), t(
            """Hi Noa,

Here are the photos of the reading room and of the children's corner, taken this morning while it was quiet. Use whichever you like; the library holds the rights.

Oskar""",
            """Bonjour Noa,

Voici les photos de la salle de lecture et du coin des enfants, prises ce matin au calme. Utilisez celles que vous voulez ; la médiathèque en détient les droits.

Oscar"""), attachments=[(t("reading-room-sketch.png", "salle-de-lecture-croquis.png"), "image/png", sketch_png())], flags="S", key="photos"),
        Mail("work", c.at(c.weekday_before(-3), "06:30"), (W.nimbus, "no-reply@nimbus.example.com"), t("Your invoice NH-2026-1009", "Votre facture NH-2026-1009"), t(
            """Hello Noa,

Your invoice NH-2026-1009 for October is available in your panel.

Plan: Studio S (web hosting)
Amount due: €12.00
It will be charged on 10 October to your card ending in 0042.

Nimbus Hosting""",
            """Bonjour Noa,

Votre facture NH-2026-1009 d'octobre est disponible dans votre espace.

Offre : Studio S (hébergement web)
Montant dû : 12,00 €
Il sera prélevé le 10 octobre sur votre carte se terminant par 0042.

Nimbus Hébergement"""), flags="S", key="nimbus-invoice"),
        Mail("work", c.at(c.weekday_before(-4), "11:12"), (t("Foundry North", "Fonderie du Nord"), f"noreply@{W.foundry}"),
             t("Your receipt for order #4471", "Reçu de votre paiement : commande n° 4471"), t(
            """Thank you for your order.

Order #4471
Lantern Serif, desktop and web licence (1 site)
Total: €48.00
Paid with PayPal.

Foundry North""",
            """Merci pour votre commande.

Commande n° 4471
Lanterne Serif, licence bureau et web (1 site)
Total : 48,00 €
Payé avec PayPal.

Fonderie du Nord"""), flags="S", key="fonts"),
        Mail("work", c.at(c.weekday_before(-6), "11:05"), iris, t("Re: Opening hours page", "Re : Page des horaires"), t(
            """Hello Noa,

Yes, the opening hours can come from our booking system: Oskar will send you the address of its calendar.

Iris""",
            """Bonjour Noa,

Oui, les horaires peuvent venir de notre système de réservation : Oscar vous enverra l'adresse de son calendrier.

Iris"""), flags="RS", key="hours"),
        Mail("work", c.at(c.weekday_before(-9), "10:40"), tom, t("Re: Poster series", "Re : Série d'affiches"), t(
            """Lovely. The blue one should go in your portfolio first.

Tom""",
            """Superbe. La bleue devrait entrer la première dans ton portfolio.

Tom"""), flags="S", key="posters"),
        Mail("work", c.at(c.weekday_before(-12), "09:30"), iris, t("The new website: our agreement", "Le nouveau site : notre accord"), t(
            """Hello Noa,

As agreed on the phone: the new website in three stages (mock-up, pages, launch), at €55 an hour, invoiced at the end of each stage. Our signed copy of the quote is attached.

We are glad to work with you again.

Iris""",
            """Bonjour Noa,

Comme convenu au téléphone : le nouveau site en trois étapes (maquette, pages, mise en ligne), à 55 € de l'heure, facturées à la fin de chaque étape. Vous trouverez ci-joint notre exemplaire signé du devis.

Nous sommes heureux de travailler à nouveau avec vous.

Iris"""), attachments=[(t("fernhill-quote-signed.pdf", "devis-fougeres-signe.pdf"), "application/pdf", pdf(
                [t("Fernhill Library - new website", "Médiathèque des Fougères - nouveau site"), t("Quote 2026-Q07, signed", "Devis 2026-Q07, signé"),
                 t("Three stages: mock-up, pages, launch", "Trois étapes : maquette, pages, mise en ligne"), t("Rate: 55 EUR an hour", "Tarif : 55 EUR de l'heure"),
                 t("Specimen: invented for Sioul's demo", "Spécimen : inventé pour la démo de Sioul")]))],
             folder=".Archive", flags="S", key="agreement"),
        # Sent from work.
        Mail("work", c.at(c.weekday_before(-6), "14:02"), me_work, t("Re: Opening hours page", "Re : Page des horaires"), t(
            """Thank you Iris, that is the best way: the page will never be out of date.

Noa""",
            """Merci Iris, c'est la meilleure façon : la page ne sera jamais périmée.

Noa"""), to=[iris], folder=".Sent", flags="S", key="sent-hours"),
        Mail("work", c.at(c.weekday_before(-3), "17:05"), me_work, t("Homepage mock-up, first draft", "Maquette de la page d'accueil, premier jet"), t(
            """Hello Iris,

Here is a first draft of the homepage, in two layouts. Tell me which feels closer to the library.

Noa""",
            """Bonjour Iris,

Voici un premier jet de la page d'accueil, en deux mises en page. Dites-moi laquelle vous semble la plus proche de la médiathèque.

Noa"""), to=[iris], cc=[oskar], folder=".Sent", flags="S", key="sent-draft"),
        Mail("work", c.at(c.weekday_before(-9), "09:15"), me_work, t("Poster series", "Série d'affiches"), t(
            """Tom, the poster series is finished: six bridges, one colour each. Which one would you show first?

Noa""",
            """Tom, la série d'affiches est finie : six ponts, une couleur chacun. Laquelle montrerais-tu en premier ?

Noa"""), to=[tom], folder=".Sent", flags="S", key="sent-posters"),

        # Personal: noa.ferrand@example.org.
        Mail("personal", c.ago(minutes=6), (W.paper_name, f"no-reply@{W.paper}"), t("Confirm your email address", "Confirmez votre adresse e-mail"), t(
            f"""Hello,

Please confirm your email address to finish creating your Paper & Ink account:

https://{W.paper}/confirm?token=7Q2-demo-41

This link is valid for 30 minutes. If you did not create an account, ignore this message.

Paper & Ink""",
            f"""Bonjour,

Confirmez votre adresse e-mail pour terminer la création de votre compte Papier & Encre :

https://{W.paper}/confirmer?jeton=7Q2-demo-41

Ce lien est valable 30 minutes. Si vous n'avez pas créé de compte, ignorez ce message.

Papier & Encre"""), key="confirm"),
        Mail("personal", c.ago(hours=3, minutes=5), (ME, WORK), t("Note to self: craft fair sizes", "Note pour moi : tailles pour le salon"), t(
            """Posters in A3 and A2; the frames are in the attic, the small table is at Mum's.""",
            """Affiches en A3 et A2 ; les cadres sont au grenier, la petite table est chez Maman."""), key="self"),
        Mail("personal", c.ago(hours=1, minutes=20), ("Maud Ferrand", "maud.ferrand@example.org"), t("Sunday lunch?", "Déjeuner dimanche ?"), t(
            """Hello my dear,

Would you come for lunch on Sunday? Hugo will be there, and I am making the apple cake.

Love,
Mum""",
            """Coucou,

Tu viens déjeuner dimanche ? Hugo sera là, et je fais le gâteau aux pommes.

Je t'embrasse,
Maman"""), key="lunch"),
        Mail("personal", c.ago(hours=18), jonas, t("Climbing on Saturday?", "Escalade samedi ?"), t(
            """Saturday morning at the gym, 10:00? The new routes on the left wall are lovely.

Jonas""",
            """Samedi matin à la salle, 10 h ? Les nouvelles voies du mur de gauche sont superbes.

Jonas"""), key="climbing"),
        Mail("personal", c.ago(hours=29), (t("Parcel service", "Service colis"), f"notification@{W.parcels}"),
             t("Your parcel arrives on Wednesday", "Votre colis arrive mercredi"), t(
            """Your parcel from Paper & Ink will arrive on Wednesday between 9:00 and 13:00. Nothing to do: if you are out, it waits at the Rue de l'Exemple relay.""",
            """Votre colis de Papier & Encre arrivera mercredi entre 9 h et 13 h. Rien à faire : si vous êtes absent(e), il vous attend au relais de la rue de l'Exemple."""),
             key="parcel"),
        Mail("personal", c.at(c.weekday_before(-2), "18:40"), ("Priya Nair", f"priya@{W.choir}"),
             t("Concert programme and an extra rehearsal", "Programme du concert et une répétition en plus"), t(
            """Dear all,

The winter concert programme is settled: Ave verum, two old carols and the Rutter. We add one rehearsal on the Thursday before the concert.

Altos: please learn the second page of the Ave verum by next week.

Priya""",
            """Bonjour à toutes et à tous,

Le programme du concert d'hiver est fixé : Ave verum, deux vieux noëls et le Rutter. Nous ajoutons une répétition le jeudi avant le concert.

Les altos : apprenez la deuxième page de l'Ave verum pour la semaine prochaine, s'il vous plaît.

Priya"""), flags="S", key="choir"),
        Mail("personal", c.at(c.day(-2), "11:30"), ("Agnès Morel", "agnes.morel@example.org"), t("Hello from the new neighbour", "Bonjour de la nouvelle voisine"), t(
            """Hello,

I have just moved into the flat on the third floor. If my boxes in the hall bother you, tell me: they will be gone by Friday.

Agnès""",
            """Bonjour,

Je viens d'emménager au troisième étage. Si mes cartons dans l'entrée vous gênent, dites-le-moi : ils seront partis vendredi.

Agnès"""), key="neighbour"),
        Mail("personal", c.at(c.weekday_before(-3), "08:10"), (W.bank_name, f"no-reply@{W.bank}"),
             t("A new document in your secure space", "Un nouveau document dans votre espace sécurisé"), t(
            """Hello,

A new document is waiting in your secure space: your September statement.

Riverside Bank""",
            """Bonjour,

Un nouveau document vous attend dans votre espace sécurisé : votre relevé de septembre.

Banque des Berges"""), flags="S", key="bank-doc"),
        Mail("personal", c.at(c.weekday_before(-5), "07:45"), (W.energy_name, f"no-reply@{W.energy}"),
             t("Your October bill: €64.20", "Votre facture d'octobre : 64,20 €"), t(
            """Hello,

Your October electricity bill is ready.

Amount: €64.20
It will be taken by direct debit on 15 October.

Your meter reading is due before the 20th.

Brightwatt""",
            """Bonjour,

Votre facture d'électricité d'octobre est disponible.

Montant : 64,20 €
Il sera prélevé le 15 octobre.

Votre relevé de compteur est à transmettre avant le 20.

Clairwatt"""), flags="S", key="energy"),
        Mail("personal", c.at(c.weekday_before(-6), "09:02"), (t("Lyon Climbing Club", "Club d'escalade de Lyon"), f"{t('news', 'actus')}@{W.club}"),
             t("October news: new routes on the left wall", "Nouvelles d'octobre : de nouvelles voies sur le mur de gauche"), t(
            """Ten new routes on the left wall, from beginner to hard. The gym closes early on 31 October.""",
            """Dix nouvelles voies sur le mur de gauche, du débutant au difficile. La salle ferme plus tôt le 31 octobre."""), kind="list", flags="S", key="club"),
        Mail("personal", c.at(c.weekday_before(-7), "10:20"), (W.phone_name, f"noreply@{W.phone}"), t("Your receipt: €19.99", "Merci pour votre paiement : 19,99 €"), t(
            """Thank you for your payment.

Mobile plan, October
Amount paid: €19.99

Wavecell""",
            """Merci pour votre paiement.

Forfait mobile, octobre
Montant payé : 19,99 €

Ondéa Mobile"""), flags="S", key="phone"),
        Mail("personal", c.at(c.weekday_before(-8), "14:15"), (W.health_name, f"no-reply@{W.health}"),
             t("Your refund of €18.50 has been paid", "Votre remboursement de 18,50 € a été versé"), t(
            """Hello,

Your refund of €18.50 for the consultation of 22 September has been paid into your account.

Health cover office""",
            """Bonjour,

Votre remboursement de 18,50 € pour la consultation du 22 septembre a été versé sur votre compte.

Caisse santé"""), flags="S", key="refund"),
        Mail("personal", c.at(c.weekday_before(-10), "07:00"), (W.tax_name, f"no-reply@{W.tax}"),
             t("Your 2026 tax notice is available", "Votre avis d'impôt 2026 est disponible"), t(
            f"""Hello,

Your 2026 tax notice (2025 income) is available in your secure space on {W.tax}.

Tax office""",
            f"""Bonjour,

Votre avis d'impôt 2026 (revenus 2025) est disponible dans votre espace sécurisé sur {W.tax}.

Centre des impôts"""), flags="S", key="tax"),
        Mail("personal", c.at(c.day(-4), "19:30"), ("Camille Ferrand", "camille.ferrand@example.org"), t("Photos from the weekend", "Les photos du week-end"), t(
            """Here they are, the ones from the beach. The dog is in all of them.

Camille""",
            """Les voilà, celles de la plage. Le chien est sur toutes.

Camille"""), flags="S", key="camille"),
        Mail("personal", c.at(c.weekday_before(-7), "16:00"), marc, t("Receipts for 2025", "Justificatifs 2025"), t(
            """Hello Noa,

When you have a moment before the end of October, could you send me your 2025 receipts? A folder of scans is perfect.

Marc Duval""",
            """Bonjour Noa,

Quand vous aurez un moment avant la fin octobre, pourriez-vous m'envoyer vos justificatifs 2025 ? Un dossier de scans, c'est parfait.

Marc Duval"""), flags="RS", key="marc"),
        Mail("personal", c.at(c.day(-1), "21:40"), (W.tax_name, f"{t('refund', 'remboursement')}@{W.tax}"),
             t("Your tax refund is waiting", "Votre remboursement d'impôt vous attend"), t(
            """You are owed a refund of €312.40. Confirm your card details within 48 hours to receive it.""",
            """Un remboursement de 312,40 € vous est dû. Confirmez les données de votre carte sous 48 heures pour le recevoir."""), auth="forged", key="phishing"),
        # Sent from home.
        Mail("personal", c.at(c.weekday_before(-6), "18:20"), me_home, t("Re: Receipts for 2025", "Re : Justificatifs 2025"), t(
            """Hello Marc, I will gather them this month and send them by the 30th.

Noa""",
            """Bonjour Marc, je les rassemble ce mois-ci et vous les envoie d'ici le 30.

Noa"""), to=[marc], folder=".Sent", flags="S", key="sent-marc"),
        Mail("personal", c.at(c.day(-3), "20:05"), me_home, t("Gym hours", "Horaires de la salle"), t(
            """Jonas, the gym opens at 9:30 on Saturdays now, not 10:00.""",
            """Jonas, la salle ouvre à 9 h 30 le samedi maintenant, pas à 10 h."""), to=[jonas], folder=".Sent", flags="S", key="sent-jonas"),
    ]
    # Each message its id; replies point at what they answer.
    by_key = {m.key: m for m in mails}
    for m in mails:
        m.message_id = f"<{m.when.strftime('%Y%m%d%H%M')}.{m.key}@{domain_of(m.sender[1])}>"
    for reply, original in [("sent-hours", "hours"), ("homepage", "sent-draft"), ("sent-marc", "marc"), ("posters", "sent-posters")]:
        by_key[reply].reply_to_id = by_key[original].message_id
    return mails


def spam_mail(clock: Clock) -> list[Mail]:
    """Strangers' messages Sioul's own spam filter has a word for, with the
    demo's table (`spam_table`): one it is sure of ("probably spam"), one it
    doubts ("maybe spam"), both flagged, waiting in the review queue; and one
    it moved into the Junk folder (`moved_log`), listed there too. Their
    headers do it: replies going elsewhere, links elsewhere, a name that names
    another domain."""
    c = clock
    return [
        Mail("work", c.ago(hours=1), (t("Prize draws at winners.example", "Tirages au sort winners.example"), "draw@prize-draws.test"),
             t("You are this month's winner", "Vous êtes le gagnant du mois"), t(
            """Congratulations: your address was drawn. Claim your 500 EUR voucher today: https://claim.prize-draws.example/v/2210""",
            """Félicitations : votre adresse a été tirée au sort. Réclamez votre bon de 500 EUR aujourd'hui : https://claim.prize-draws.example/v/2210"""),
             auth="none", extra=["Reply-To: claims@claims-office.example"], key="moved", folder=".Junk"),
        Mail("work", c.ago(hours=2), (t("Parcel desk at deliveries.example", "Service colis livraisons.example"), "desk@quick-parcels.test"),
             t("Your parcel is waiting", "Votre colis vous attend"), t(
            """Hello,

Your parcel could not be delivered: customs fees of 1.99 EUR are unpaid.

Pay them today to receive it: https://pay.parcel-fees.example/r/7731
Follow it: https://track.parcel-fees.example/7731

Parcel desk""",
            """Bonjour,

Votre colis n'a pas pu être livré : des frais de douane de 1,99 EUR restent à payer.

Payez-les aujourd'hui pour le recevoir : https://pay.parcel-fees.example/r/7731
Suivez-le : https://track.parcel-fees.example/7731

Service colis"""), auth="none", extra=["Reply-To: claims@claims-office.example"], key="parcel"),
        Mail("work", c.ago(hours=4), (t("Print deals", "Bonnes affaires impression"), "offers@print-deals.example"),
             t("Business cards, this week only", "Cartes de visite, cette semaine seulement"), t(
            """Business cards from 9.90 EUR, this week only.

Order yours: https://print-orders.test/cards""",
            """Des cartes de visite à partir de 9,90 EUR, cette semaine seulement.

Commandez les vôtres : https://print-orders.test/cartes"""), auth="none", extra=["Reply-To: orders@print-orders.test"], key="print"),
    ]


def compose_mail(clock: Clock) -> list[Mail]:
    """A message to answer on a phone's width (SIOUL_GRAB_STEPS=compose): a
    subject of more than sixty characters, replies asked to two addresses
    (Reply-To), a copy to a third. Answered to all, the writing window holds a
    To of two addresses, a Cc and a long "Re: …"."""
    c = clock
    desk = (t("Fernhill Library front desk", "Accueil de la Médiathèque des Fougères"), f"{t('desk', 'accueil')}@{W.library}")
    return [
        Mail("work", c.ago(hours=3), W.iris,
             t("Fernhill Library website: the opening hours, the children's corner and the launch date",
               "Site de la Médiathèque des Fougères : les horaires, le coin des enfants et la date de mise en ligne"), t(
            """Hello Noa,

Could you tell us when the new site goes live? The front desk would like to announce it in the newsletter, with the opening hours and the children's corner.

Please answer us both: the front desk writes the newsletter.

Iris""",
            """Bonjour Noa,

Pourriez-vous nous dire quand le nouveau site sera en ligne ? L'accueil voudrait l'annoncer dans la lettre d'information, avec les horaires et le coin des enfants.

Répondez-nous à tous les deux : c'est l'accueil qui écrit la lettre.

Iris"""), cc=[W.oskar], flags="S", key="compose-long", extra=[f"Reply-To: {address(*W.iris)}, {address(*desk)}"]),
    ]


def spam_table(clock: Clock) -> bytes:
    """Sioul's own spam filter's table for the demo, written by hand in its
    file's format (crates/sioul-core/src/spam/table.rs): no fastText, no word of
    anyone's mail. Three header features weigh (replies going elsewhere, a name
    naming another domain, links elsewhere) and two placeholders (a price, a
    link), so that only `spam_mail`'s two messages get a word. Made for the
    format, the tokenizer and the header features Sioul reads now (format 2,
    tokenizer 2, header features 4, 35 of them: the centroid's kind, its
    header weights beside its words): with others, Sioul refuses it, and says so."""
    def fnv64(data: bytes) -> int:
        h = 0xCBF29CE484222325
        for byte in data:
            h = ((h ^ byte) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
        return h

    features = 35  # spam::features::N
    weights = [0.0] * features
    # reply_to_elsewhere, name_names_domain, links_elsewhere: their places in features.rs's NAMES.
    for at, weight in ((10, 2.5), (13, 2.0), (31, 1.5)):
        weights[at] = weight
    words = sorted((fnv64(word.encode()), score) for word, score in (("_PRICE_", 2.0), ("_URL_", 1.0)))
    buckets = [0.0] * 8
    meta = json.dumps({
        "trained_at": unix(clock.ago(hours=26)), "ham": 4210, "spam": 655, "test_ham": 1052, "test_spam": 164,
        "metrics": {"threshold_spam": 0.95, "threshold_unsure": 0.5, "ham_called_spam": 0.0019, "spam_caught": 0.872,
                    "ham_called_unsure": 0.0124, "spam_caught_unsure": 0.951, "unsure": 0.031, "auc": 0.991},
        "device": t("noa-desk", "noa-bureau"),
    }, separators=(",", ":")).encode()
    # The format, the tokenizer's and the features' versions, the dimension, n-grams of 3 to 6, then the counts.
    out = b"SIOULSPM" + struct.pack("<12I", 2, 2, 4, 2, 3, 6, len(buckets), len(words), features, len(meta), 0, 0)
    # The bias, the mean message's text, Platt's A and B: p = 1 / (1 + exp(-f)).
    out += struct.pack("<4f", -2.5, 0.0, -1.0, 0.0)
    out += b"".join(struct.pack("<Q", h) for h, _ in words) + b"".join(struct.pack("<f", score) for _, score in words)
    out += b"".join(struct.pack("<f", x) for x in buckets + weights + [0.0] * features)
    out += meta
    return out + struct.pack("<Q", fnv64(out))


def moved_log(p: Profile, mails: list[Mail]):
    """What the demo's filter moved into the Junk folder, as a device's log of
    its moves keeps it (crates/sioul-core/src/spam/labels.rs, `Moved`): a
    line each, never a label; the review queue lists them while unreviewed."""
    lines = "".join(json.dumps({
        "at": unix(m.when) + 60, "account": account_id(m.account), "folder": "INBOX", "uidvalidity": VALIDITY[m.account], "uid": 1,
        "message_id": m.message_id.strip("<>"), "class": "spam",
    }, separators=(",", ":")) + "\n" for m in mails)
    p.write(p.state / "spam" / "moved" / "demo-device.jsonl", lines)


def write_calls(p: Profile):
    """The calls a phone of yours screened, as its log reaches a computer
    through the sharing (crates/sioul-core/src/calls.rs, `Held`; the phone's
    name in the sharing invented): what that computer's Porch lists of those it
    declined, at the times their callers may reach you, and the doctor's calls
    of the month on her sheet. Fiction numbers: ARCEP's 01 99 00 and 04 65 71."""
    c = p.clock

    def call(moment: datetime, key: str, who: str, column: str, why: str = "matrix", rang: bool = False) -> str:
        line = {"at": unix(moment) * 1000}
        if key:
            line["key"] = key
        else:
            line["hidden"] = True
        line.update({"who": who, "column": column, "why": why, "rang": rang})
        return json.dumps(line, separators=(",", ":")) + "\n"

    doctor = "+33465714770"
    stranger = "+33199001234"
    lines = [
        call(c.at(c.day(-12), "18:40"), doctor, "safe", "leisure", rang=True),
        call(c.at(c.weekday_before(-3), "10:05"), doctor, "safe", "work", rang=True),
        call(c.at(c.day(-1), "23:10"), doctor, "safe", "sleep"),
        call(c.at(c.today, "06:50"), stranger, "stranger", "sleep"),
        call(c.at(c.today, "07:25"), stranger, "stranger", "sleep"),
        call(c.at(c.today, "07:31"), stranger, "stranger", "sleep", why="repeat", rang=True),
        call(c.at(c.today, "12:40"), "", "hidden", "meals"),
    ]
    p.write(p.state / "calls" / "log" / "demo-phone.jsonl", "".join(lines))


def write_phone_messages(p: Profile):
    """The messages a phone's notifications brought, as its log reaches a
    computer through the sharing (crates/sioul-core/src/phonemsgs.rs, `Line`;
    the phone's name in the sharing invented), and the part switched on here
    (share/here.toml): a text from the doctor while you slept, a pharmacy's,
    a service's, a code said without it, a chat's whose words stayed on the
    phone. Fiction numbers: ARCEP's 01 99 00 and 04 65 71."""
    c = p.clock

    def line(moment: datetime, name: str, who: str, text: str = "", key: str = "", talk: str = "", kind: str = "text", **more) -> str:
        at = unix(moment) * 1000
        fields = {"at": at, "id": f"{at}-{talk or name}", "app": "foundation.e.message" if kind == "text" else "org.thoughtcrime.securesms", "label": "Message" if kind == "text" else "Signal", "kind": kind}
        fields.update({k: v for k, v in {"talk": talk, "name": name, "key": key, "who": who, "text": text}.items() if v})
        fields.update(more)
        fields["shows"] = at
        return json.dumps(fields, ensure_ascii=False, separators=(",", ":")) + "\n"

    doctor = "+33465714770"
    lines = [
        line(c.at(c.day(-1), "23:10"), "Dr Elena Varga", "safe", t("Could you call me back tomorrow morning? Nothing worrying.", "Pourriez-vous me rappeler demain matin ? Rien d’inquiétant."), doctor, "doctor"),
        line(c.at(c.today, "09:41"), t("Linden Pharmacy", "Pharmacie des Tilleuls"), "stranger", t("Your prescription is ready. We are open until 19:30.", "Votre ordonnance est prête. Nous sommes ouverts jusqu’à 19 h 30."), "+33199001234", "pharmacy"),
        line(c.at(c.today, "10:15"), "AMELI", "automaton", t("Your certificate is available in your account.", "Votre attestation est disponible dans votre compte.")),
        line(c.at(c.today, "11:05"), t("My Bank", "Ma Banque"), "", code=True),
        line(c.at(c.today, "13:02"), "Sam", "neutral", "", "", "sam", kind="chat", withheld=True),
    ]
    p.write(p.state / "phone-messages" / "log" / "demo-phone.jsonl", "".join(lines))
    p.write(p.state / "share" / "here.toml", '[parts]\n"phone-messages" = true\n')


def parcel_png() -> bytes:
    """A parcel on a relay point's counter, as a courier photographs it: a taped box on a grey counter."""
    def pixel(x: int, y: int):
        if 50 <= x < 190 and 40 <= y < 140:
            if 112 <= x < 128 or 84 <= y < 94:
                return (214, 196, 150)  # the tape
            shade = 12 if x > 170 or y > 128 else 0
            return (176 - shade, 128 - shade, 78 - shade)
        if y >= 140:
            return (150, 150, 146)
        return (232, 230, 224)
    return png(240, 180, pixel)


def write_texts(p: Profile):
    """texts: the texts a phone read for this computer, and those written here
    for it to send, with what became of them (crates/sioul-core/src/texts.rs,
    `Text`, `Request`, `Outcome`), as the demo's stand-ins ("demo:" and the
    plain line, which the demo profile alone reads; a media file kept plain):
    the doctor's, a pharmacy's answered and delivered, one deleted on the
    phone, a courier's multimedia message with its picture and a video left
    on the phone, a text that waited too long and one waiting for the phone,
    and a week's conversation with Priya from the choir.
    Fiction numbers: ARCEP's 01 99 00 and 04 65 71."""
    import hashlib
    c = p.clock

    def line(fields: dict) -> str:
        return "demo:" + json.dumps(fields, ensure_ascii=False, separators=(",", ":")) + "\n"

    def text(id: str, moment: datetime, box: str, number: str, body: str, thread: str) -> str:
        return line({"id": id, "at": unix(moment) * 1000, "thread": thread, "box": box, "with": [number], "body": body, "sub": 1})

    doctor, pharmacy, courier = "+33465714770", "+33199001234", "+33465710042"
    log = [
        text("sms-101", c.at(c.day(-1), "23:10"), "in", doctor, t("Could you call me back tomorrow morning? Nothing worrying.", "Pourriez-vous me rappeler demain matin ? Rien d’inquiétant."), "3"),
        text("sms-102", c.at(c.today, "08:05"), "out", doctor, t("Of course, I will call at nine.", "Bien sûr, j’appelle à neuf heures."), "3"),
        text("sms-103", c.at(c.today, "09:41"), "in", pharmacy, t("Your prescription is ready. We are open until 19:30.", "Votre ordonnance est prête. Nous sommes ouverts jusqu’à 19 h 30."), "5"),
        text("sms-104", c.at(c.today, "10:03"), "out", pharmacy, t("Thank you, I will come by tonight.", "Merci, je passe ce soir."), "5"),
        text("sms-105", c.at(c.today, "11:20"), "in", courier, t("Your parcel is at the relay point until Saturday.", "Votre colis est au point relais jusqu’à samedi."), "6"),
        text("sms-106", c.at(c.day(-1), "18:30"), "in", pharmacy, t("Reminder: the pharmacy closes early on Thursday.", "Rappel : la pharmacie ferme plus tôt jeudi."), "5"),
        line({"id": "sms-106", "at": unix(c.at(c.day(-1), "18:30")) * 1000, "box": "in", "with": [pharmacy], "deleted": unix(c.at(c.today, "07:00")) * 1000}),
    ]
    # The courier's multimedia message: its picture kept here (the demo's media are plain), its video left on the phone.
    picture = parcel_png()
    digest = hashlib.sha256(picture).hexdigest()
    p.write(p.data / "texts" / "media" / digest, picture)
    log.append(line({"id": "mms-21", "at": unix(c.at(c.today, "11:22")) * 1000, "thread": "6", "box": "in", "with": [courier],
                     "body": t("Here is your parcel at the counter.", "Voici votre colis au comptoir."), "sub": 1, "picture": True,
                     "parts": [{"seq": -1, "ct": "application/smil", "text": "<smil/>", "state": "text"},
                               {"seq": 0, "ct": "image/png", "name": t("parcel.png", "colis.png"), "size": len(picture), "hash": digest, "state": "here"},
                               {"seq": 1, "ct": "video/mp4", "name": t("counter.mp4", "comptoir.mp4"), "size": 31_457_280, "state": "too-big"},
                               {"seq": 2, "ct": "text/plain", "text": t("Here is your parcel at the counter.", "Voici votre colis au comptoir."), "state": "text"}]}))
    # A long conversation with Priya from the choir, over the last week: the thread's days and its length.
    priya = "+262639989046"
    week = [
        (-6, "18:02", "in", "Did you get the new score for the winter concert?", "Tu as reçu la nouvelle partition pour le concert d’hiver ?"),
        (-6, "18:20", "out", "Yes, this morning. The second page is hard for the altos.", "Oui, ce matin. La deuxième page est difficile pour les altos."),
        (-6, "18:21", "in", "It is for everyone. We can work on it on Thursday.", "Pour tout le monde. On peut la travailler jeudi."),
        (-5, "08:45", "in", "Rehearsal moved to the small hall, the big one is booked.", "La répétition passe dans la petite salle, la grande est réservée."),
        (-5, "09:10", "out", "Noted. Same time?", "Noté. Même heure ?"),
        (-5, "09:12", "in", "Same time, half past seven.", "Même heure, sept heures et demie."),
        (-4, "19:05", "in", "Can you bring the music stands from the cupboard?", "Tu peux apporter les pupitres du placard ?"),
        (-4, "19:30", "out", "I will take three. I have no room for more on the bike.", "J’en prends trois. Je n’ai pas plus de place sur le vélo."),
        (-4, "19:31", "in", "Three is fine, Hugo brings the rest.", "Trois, c’est bien, Hugo apporte le reste."),
        (-3, "12:15", "out", "I found a recording of the piece, slower than ours.", "J’ai trouvé un enregistrement du morceau, plus lent que le nôtre."),
        (-3, "12:40", "in", "Send me the link, I will listen tonight.", "Envoie-moi le lien, j’écoute ce soir."),
        (-3, "12:41", "out", "It is in the choir's mail, under Tuesday.", "Il est dans le courrier de la chorale, à mardi."),
        (-3, "21:50", "in", "Listened. Their tempo is better for the breathing.", "Écouté. Leur tempo est meilleur pour la respiration."),
        (-2, "07:55", "in", "Coffee before rehearsal? The café by the hall opens at seven.", "Un café avant la répétition ? Le café près de la salle ouvre à sept heures."),
        (-2, "08:30", "out", "Gladly, at a quarter past seven.", "Avec plaisir, à sept heures et quart."),
        (-2, "22:10", "in", "Good evening tonight. The altos sounded much surer.", "Bonne soirée ce soir. Les altos étaient bien plus sûres."),
        (-2, "22:14", "out", "Thank you for your patience with the second page.", "Merci pour ta patience avec la deuxième page."),
        (-1, "10:02", "in", "The concert programme is printed. I have yours.", "Le programme du concert est imprimé. J’ai le tien."),
        (-1, "10:20", "out", "Keep it until Thursday, please.", "Garde-le jusqu’à jeudi, s’il te plaît."),
        (-1, "17:45", "in", "Of course. Do not forget the black folder.", "Bien sûr. N’oublie pas la pochette noire."),
        (0, "08:12", "in", "Thursday we start with the warm-up at seven sharp.", "Jeudi on commence l’échauffement à sept heures pile."),
        (0, "08:30", "out", "I will be there. Thank you for organising everything.", "Je serai là. Merci de tout organiser."),
    ]
    for n, (day, hour, box, english, french) in enumerate(week):
        log.append(text(f"sms-{140 + n}", c.at(c.day(day), hour), box, priya, t(english, french), "9"))
    p.write(p.data / "texts" / "log" / "demo-phone.jsonl", "".join(log))
    answered, expired, waiting = "a" * 32, "b" * 32, "c" * 32
    requests = [
        line({"key": answered, "phone": "demo-phone", "to": pharmacy, "body": t("Thank you, I will come by tonight.", "Merci, je passe ce soir."), "sub": -1, "written": unix(c.at(c.today, "10:02")) * 1000}),
        line({"key": expired, "phone": "demo-phone", "to": courier, "body": t("Thank you, I will pick it up on Friday.", "Merci, je le récupère vendredi."), "sub": -1, "written": unix(c.ago(minutes=50)) * 1000}),
        line({"key": waiting, "phone": "demo-phone", "to": courier, "body": t("Is the relay point open on Saturday morning?", "Le point relais est-il ouvert samedi matin ?"), "sub": -1, "written": unix(c.ago(minutes=1)) * 1000}),
    ]
    p.write(p.data / "texts" / "send" / "demo-desk.jsonl", "".join(requests))
    outcomes = [
        line({"key": answered, "state": "handed", "at": unix(c.at(c.today, "10:03")) * 1000}),
        line({"key": answered, "state": "sent", "at": unix(c.at(c.today, "10:03")) * 1000, "row": "sms-104"}),
        line({"key": answered, "state": "delivered", "at": unix(c.at(c.today, "10:04")) * 1000}),
        line({"key": expired, "state": "expired", "at": unix(c.ago(minutes=30)) * 1000}),
    ]
    p.write(p.data / "texts" / "outcome" / "demo-phone.jsonl", "".join(outcomes))
    # An AI agent's draft for the doctor, waiting in the Texts page, never sent
    # (crates/sioul-core/src/textdraft.rs; docs/mcp.md, "Texts"): this device's alone.
    p.write(p.state / "texts" / "drafts.jsonl", line({"id": "d" * 32, "to": doctor, "conversation": doctor,
                                                     "body": t("I can call at nine tomorrow, if that suits you.", "Je peux appeler demain à neuf heures, si cela vous convient."),
                                                     "written": unix(c.ago(minutes=5)) * 1000}))


def account_id(key: str) -> str:
    """The id of the work or the personal address, as the configuration names it (and the Mail page shows it)."""
    return {"work": t("work", "travail"), "personal": t("personal", "perso")}[key]


def write_mail(p: Profile, mails: list[Mail]):
    counters: dict[tuple[str, str], int] = {}
    numbered = sorted(enumerate(mails), key=lambda pair: pair[1].when)
    marks: dict[str, int] = {}
    closed_at = p.clock.ago(hours=30)
    for number, m in numbered:
        folder_key = (m.account, m.folder)
        uid = counters.get(folder_key, 3100 if m.account == "work" else 5200) + 1
        counters[folder_key] = uid
        validity = VALIDITY[m.account] + {"": 0, ".Sent": 1, ".Archive": 2, ".Junk": 3}[m.folder]
        raw = raw_message(m, number + 1)
        unique = f"{unix(m.when)}.U{validity}-{uid}.sioul"
        root = p.data / "mail" / account_id(m.account) / m.folder if m.folder else p.data / "mail" / account_id(m.account)
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
                (p.data / "mail" / account_id(account) / folder / sub if folder else p.data / "mail" / account_id(account) / sub).mkdir(parents=True, exist_ok=True)
        folders = [("INBOX", "INBOX", "inbox", "", False), ("Sent", "Sent", "sent", ".Sent", True), ("Drafts", "Drafts", "drafts", ".Drafts", True),
                   ("Archive", "Archive", "archive", ".Archive", True), ("Junk", "Junk", "junk", ".Junk", True), ("Trash", "Trash", "trash", ".Trash", True)]
        text = "".join(f"[[folder]]\nname = {toml_string(n)}\ndisplay = {toml_string(d)}\nrole = \"{r}\"\nlocal = {toml_string(l)}\nspecial = {'true' if s else 'false'}\n\n" for n, d, r, l, s in folders)
        p.write(p.state / "sync" / f"{account_id(account)}.folders.toml", text)
    porch = "".join(f"[done.{account_id(account)}]\nvalidity = {VALIDITY[account]}\nuid = {uid}\n\n" for account, uid in sorted(marks.items()))
    p.write(p.state / "porch.toml", porch)
    p.write(p.state / "authserv-id", SIOUL + "\n")


# --- Calendars and tasks ------------------------------------------------------

def vcalendar(component: list[str]) -> str:
    return "\r\n".join(["BEGIN:VCALENDAR", "VERSION:2.0", "PRODID:-//Sioul//Demo profile//EN", *component, "END:VCALENDAR"]) + "\r\n"


def escape(text: str) -> str:
    return text.replace("\\", "\\\\").replace(";", "\\;").replace(",", "\\,").replace("\n", "\\n")


def event(clock: Clock, uid: str, title: str, day: date, start: str, end: str, *, location="", notes="", weekly=False,
          organizer=None, attendees=(), project=None) -> str:
    lines = ["BEGIN:VEVENT", f"UID:demo-event-{uid}", f"DTSTAMP:{utc(clock.at(clock.day(-14), '09:00'))}",
             f"DTSTART:{utc(clock.at(day, start))}", f"DTEND:{utc(clock.at(day, end))}", f"SUMMARY:{escape(title)}"]
    if location:
        lines.append(f"LOCATION:{escape(location)}")
    if notes:
        lines.append(f"DESCRIPTION:{escape(notes)}")
    if weekly:
        lines.append("RRULE:FREQ=WEEKLY")
    if organizer:
        lines.append(f'ORGANIZER;CN="{organizer[0]}":mailto:{organizer[1]}')
    for name, addr, answer in attendees:
        lines.append(f'ATTENDEE;CN="{name}";PARTSTAT={answer}:mailto:{addr}')
    if project:
        lines.append(f"REFID:{project}")
    lines.append("END:VEVENT")
    return vcalendar(lines)


def write_calendars(p: Profile, mails: list[Mail]):
    c = p.clock
    collections = [
        ("calendars", "personal", t("Personal", "Personnel"), "#7a6f9b", ["VEVENT"]),
        ("calendars", "work", t("Work", "Travail"), "#4c6b5c", ["VEVENT"]),
        ("calendars", "home-tasks", t("Home", "Maison"), "#9a7b4f", ["VTODO"]),
        ("calendars", "work-tasks", t("Work tasks", "Tâches du travail"), "#4c6b5c", ["VTODO"]),
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
    events = [
        ("work", event(c, "fernhill-review", t("Fernhill: homepage review", "Fougères : revue de la page d'accueil"), c.day(0), "10:15", "11:15",
                       location=t("Video call, meet.example.com/fernhill", "Visio, meet.example.com/fougeres"),
                       notes=t("The two layouts, the opening hours under the photo, the children's corner in the menu.",
                               "Les deux mises en page, les horaires sous la photo, le coin des enfants dans le menu."),
                       organizer=W.iris, attendees=[me_w, (W.oskar[0], W.oskar[1], "ACCEPTED")], project="fernhill")),
        ("work", event(c, "planning", t("Weekly planning", "Planning de la semaine"), c.this_week(0), "09:00", "10:00",
                       notes=t("The week's steps, in Sioul's Tasks.", "Les étapes de la semaine, dans les Tâches de Sioul."), weekly=True)),
        ("personal", event(c, "doctor", t("Dr Varga: check-up", "Dr Varga : bilan"), c.weekday_after(1), "09:30", "10:00",
                           location=t("Riverside Health Centre, 5 Rue Imaginaire, Lyon", "Centre de santé des Berges, 5 rue Imaginaire, Lyon"),
                           notes=t("Bring the blood test results.", "Apporter les résultats de la prise de sang."))),
        ("work", event(c, "accountant", t("Call with Marc (accountant)", "Appel avec Marc (comptable)"), c.weekday_after(2), "11:00", "11:30",
                       location=t("Phone: +33 4 65 71 63 25", "Téléphone : +33 4 65 71 63 25"),
                       notes=t("The 2025 receipts, and the quarterly declaration.", "Les justificatifs 2025 et la déclaration trimestrielle."), project="taxes-2026")),
        ("personal", event(c, "choir", t("Choir rehearsal", "Répétition de la chorale"), c.this_week(3), "18:45", "20:30",
                           location=t("Two Rivers Voices, community hall", "Chœur des Deux Rivières, salle des fêtes"), weekly=True, project="choir")),
        ("work", event(c, "tom-lunch", t("Lunch with Tom", "Déjeuner avec Tom"), c.this_week(4), "12:30", "13:45", location="Café du Quai")),
        ("personal", event(c, "climbing", t("Climbing with Jonas", "Escalade avec Jonas"), c.this_week(5), "10:00", "12:00",
                           location=t("Bloc & Boulder gym", "Salle Bloc et Prise"))),
        ("personal", event(c, "lunch-mum", t("Lunch at Mum's", "Déjeuner chez Maman"), c.this_week(6), "12:30", "15:00", location="Villeurbanne")),
        ("work", event(c, "fernhill-pages", t("Fernhill: pages kick-off", "Fougères : lancement des pages"), c.weekday_after(8), "10:00", "11:00",
                       location=W.library_name, organizer=W.iris, attendees=[me_w], project="fernhill")),
        ("personal", event(c, "hugo-birthday", t("Hugo's birthday dinner", "Dîner d'anniversaire de Hugo"), c.day(9), "19:30", "22:00", location="Saint-Étienne")),
    ]
    for calendar, text in events:
        uid = text.split("UID:")[1].split("\r\n")[0]
        p.write(p.data / "calendars" / DAV / calendar / f"{uid}.ics", text)

    by_key = {m.key: m for m in mails}
    write_tasks(p, by_key)


def task(clock: Clock, uid: str, title: str, *, status="NEEDS-ACTION", start=None, due=None, estimate=0, kind="", categories=(),
         project=None, parent=None, waits=(), notes="", links=(), contacts=(), office=None, completed=None, priority=0, created=None, energy="") -> str:
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
        lines.append("CATEGORIES:" + ",".join(escape(c) for c in categories))
    if kind:
        lines.append(f"CONCEPT:{KIND}{kind}")
    if office is not None:
        lines.append(f"CONCEPT:{OFFICE}")
        if office:
            lines.append(f"X-SIOUL-OFFICE-HOURS:{escape(office)}")
    if energy:
        lines.append(f"X-SIOUL-ENERGY:{energy.upper()}")
    if project:
        lines.append(f"REFID:{project}")
    if parent:
        lines.append(f"RELATED-TO;RELTYPE=PARENT:demo-task-{parent}")
    for other in waits:
        lines.append(f"RELATED-TO;RELTYPE=DEPENDS-ON:demo-task-{other}")
    for rel, uri in links:
        lines.append(f"LINK;LINKREL={rel};VALUE=URI:{uri}")
    names = contacts_by_key()
    for key in contacts:
        lines.append(f'CONTACT;ALTREP="sioul:contact/demo-contact-{key}":{escape(names[key])}')
    if completed:
        lines += [f"COMPLETED:{utc(completed)}", "PERCENT-COMPLETE:100"]
    lines.append("END:VTODO")
    return vcalendar(lines)


def contacts_by_key() -> dict:
    return {key: c["name"] for key, c in contacts().items()}


def note_uri(path: str) -> str:
    """A note's address, as Sioul writes it: spaces and a few signs percent-encoded, "/" kept."""
    out = ""
    for ch in path:
        out += "".join(f"%{b:02X}" for b in ch.encode()) if ch in ' "#%<>?[]\\^`{|}' else ch
    return "sioul:note/" + out


def mid(m: "Mail") -> str:
    return "mid:" + m.message_id.strip("<>")


def write_tasks(p: Profile, mail: dict):
    c = p.clock
    project_note = note_uri(W.project_note)
    work, admin = t("work", "travail"), t("admin", "démarches")
    tasks = [
        # Work.
        ("work-tasks", task(c, "mockup", t("Fernhill: homepage mock-up", "Fougères : maquette de la page d'accueil"), due=c.weekday_after(4), kind="make",
                            categories=[work], project="fernhill", links=[("describedby", project_note)], contacts=["iris"],
                            notes=t("Two layouts; the calm one first. Opening hours under the photo.",
                                    "Deux mises en page ; la calme d'abord. Les horaires sous la photo."))),
        ("work-tasks", task(c, "photos", t("Gather the library's photos", "Rassembler les photos de la médiathèque"), status="COMPLETED", estimate=30, kind="online",
                            categories=[work], project="fernhill", parent="mockup", completed=c.at(c.weekday_before(-2), "17:10"), links=[("via", mid(mail["photos"]))])),
        ("work-tasks", task(c, "sketch", t("Sketch two layouts for the homepage", "Esquisser deux mises en page pour l'accueil"), status="IN-PROCESS", start=c.day(0),
                            estimate=240, kind="make", categories=[work], project="fernhill", parent="mockup", links=[("describedby", project_note)])),
        ("work-tasks", task(c, "send-mockup", t("Send the mock-up to Iris", "Envoyer la maquette à Iris"), estimate=30, kind="write", categories=[work], project="fernhill",
                            parent="mockup", waits=["sketch"], contacts=["iris"])),
        ("work-tasks", task(c, "hours-page", t("Fernhill: opening hours page", "Fougères : page des horaires"), estimate=90, kind="make", categories=[work],
                            project="fernhill", waits=["mockup"], due=c.weekday_after(11), links=[("via", mid(mail["hours"]))])),
        ("work-tasks", task(c, "invoice", t("Invoice Fernhill for the mock-up stage", "Facturer la médiathèque pour l'étape maquette"), estimate=15, kind="online",
                            categories=[work], project="fernhill", waits=["send-mockup"], due=c.weekday_after(6))),
        ("work-tasks", task(c, "quote", t("Write a quote for the Greenfield Cooperative", "Rédiger un devis pour la Coopérative des Champs Verts"), estimate=60,
                            kind="write", categories=[work], due=c.weekday_after(3), links=[("via", mid(mail["hana"]))], created=c.ago(hours=2))),
        ("work-tasks", task(c, "fair", t("Craft fair: answer Tom about the shared stand", "Salon des créateurs : répondre à Tom pour le stand"), estimate=15,
                            kind="write", categories=[work], due=c.weekday_after(4), contacts=["tom"], links=[("via", mid(mail["fair"]))])),
        ("work-tasks", task(c, "portfolio", t("Portfolio: add the poster series", "Portfolio : ajouter la série d'affiches"), estimate=90, kind="make",
                            categories=[work, "someday"])),
        ("work-tasks", task(c, "oskar", t("Thank Oskar for the photos", "Remercier Oscar pour les photos"), status="COMPLETED", estimate=5, kind="write",
                            categories=[work], project="fernhill", completed=c.at(c.day(0), "09:20"), contacts=["oskar"])),
        # Admin.
        ("home-tasks", task(c, "health-call", t("Call the health cover office about the refund", "Appeler la caisse santé pour le remboursement"), estimate=15,
                            kind="call", categories=[admin, t("health", "santé")], office="mo-fr 09:00-12:00, 14:00-16:30", contacts=["healthcover"],
                            notes=t("The September consultation: only €18.50 came back.", "La consultation de septembre : seulement 18,50 € remboursés."))),
        ("home-tasks", task(c, "meter", t("Send the meter reading to Brightwatt", "Envoyer le relevé de compteur à Clairwatt"), estimate=10, kind="online",
                            categories=[admin], due=c.day(12), links=[("via", mid(mail["energy"]))])),
        ("home-tasks", task(c, "passport", t("Passport: book an appointment at the town hall", "Passeport : prendre rendez-vous à la mairie"), estimate=15,
                            kind="online", categories=[admin], start=c.day(0), due=c.day(30), links=[("related", "sioul:paper/passport")])),
        ("home-tasks", task(c, "receipts", t("Gather the 2025 receipts", "Rassembler les justificatifs 2025"), estimate=45, kind="make", categories=[admin],
                            project="taxes-2026", links=[("describedby", note_uri(W.tax_note))])),
        ("home-tasks", task(c, "send-receipts", t("Send the receipts to Marc", "Envoyer les justificatifs à Marc"), estimate=10, kind="write", categories=[admin],
                            project="taxes-2026", waits=["receipts"], due=c.day(24), contacts=["marc"], links=[("via", mid(mail["marc"]))])),
        ("home-tasks", task(c, "tax-check", t("Read the 2026 tax notice", "Lire l'avis d'impôt 2026"), estimate=20, kind="read", categories=[admin],
                            project="taxes-2026", due=c.day(20), links=[("via", mid(mail["tax"]))])),
        ("home-tasks", task(c, "bike", t("Fix the bike's back light", "Réparer le feu arrière du vélo"), estimate=20, kind="make", categories=[admin])),
        ("home-tasks", task(c, "bus-pass", t("Renew the bus pass", "Renouveler l'abonnement de bus"), status="COMPLETED", estimate=10, kind="online",
                            categories=[admin], completed=c.at(c.day(-3), "18:30"))),
        # Leisure.
        ("home-tasks", task(c, "alto", t("Learn the alto part of the Ave verum", "Apprendre la partie d'alto de l'Ave verum"), estimate=30, kind="make",
                            categories=[t("leisure", "loisirs"), "joy"], project="choir", links=[("via", mid(mail["choir"]))])),
        ("home-tasks", task(c, "gift", t("Find a birthday present for Hugo", "Trouver un cadeau d'anniversaire pour Hugo"), estimate=30, kind="out",
                            categories=[t("family", "famille")], due=c.day(8), contacts=["hugo"])),
        ("home-tasks", task(c, "gym", t("Book the climbing gym for Saturday", "Réserver la salle d'escalade pour samedi"), estimate=5, kind="online",
                            categories=[t("leisure", "loisirs")], due=c.this_week(4), contacts=["jonas"])),
    ]
    for calendar, text in tasks:
        uid = text.split("UID:")[1].split("\r\n")[0]
        p.write(p.data / "calendars" / DAV / calendar / f"{uid}.ics", text)


def write_contacts(p: Profile):
    people = contacts()
    for key, c in people.items():
        p.write(p.data / "contacts" / DAV / "contacts" / f"demo-contact-{key}.vcf", vcard(key, c))
    places = ["[found]"]
    for c in people.values():
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
    library_page = W.project_note.split("/")[-1][:-3]
    meeting_page = W.meeting_note.split("/")[-1][:-3]
    sketch_file = W.sketch.split("/")[-1]
    tax_day = long_date(c.weekday_before(-10))
    notes = {
        W.project_note: (stamp(0, "09:40"), t(f"""---
tags: [client, website]
aliases: [Fernhill]
---
# Fernhill Library: the new website

For [[Iris Calloway]] and Oskar. Three stages, invoiced at the end of each: mock-up, pages, launch. €55 an hour.

## Where it stands
- Mock-up: two layouts sketched, the calm one with the reading room's photo preferred.
- Opening hours come from the library's booking calendar ([[{meeting_page}]]).

## To do
- [x] Gather the library's photos
- [ ] Opening hours right under the photo
- [ ] A menu entry for the children's corner
- [ ] Check the contrast of the footer

![[{sketch_file}]]

The agreement: [the signed quote]({mid(by_key['agreement'])}). #client
""", f"""---
tags: [client, site-web]
aliases: [Fougères]
---
# Médiathèque des Fougères : le nouveau site

Pour [[Iris Calvet]] et Oscar. Trois étapes, facturées à la fin de chacune : maquette, pages, mise en ligne. 55 € de l'heure.

## Où on en est
- Maquette : deux mises en page esquissées ; la calme, avec la photo de la salle de lecture, a la préférence.
- Les horaires viennent du calendrier de réservation de la médiathèque ([[{meeting_page}]]).

## À faire
- [x] Rassembler les photos de la médiathèque
- [ ] Les horaires juste sous la photo
- [ ] Une entrée de menu pour le coin des enfants
- [ ] Vérifier le contraste du pied de page

![[{sketch_file}]]

L'accord : [le devis signé]({mid(by_key['agreement'])}). #client
""")),
        W.meeting_note: (stamp(-6, "11:30"), t(f"""# Fernhill: first meeting

With Iris and Oskar, at the library.

- They want a site that reads well on a phone: most visitors look for the opening hours.
- The children's corner is the library's pride.
- Oskar can give a calendar address for the hours.
- Next: the mock-up, by the end of next week.

Back to [[{library_page}]].
""", f"""# Fougères : première réunion

Avec Iris et Oscar, à la médiathèque.

- Ils veulent un site qui se lise bien sur un téléphone : la plupart des visiteurs cherchent les horaires.
- Le coin des enfants est la fierté de la médiathèque.
- Oscar peut fournir l'adresse d'un calendrier pour les horaires.
- Ensuite : la maquette, d'ici la fin de la semaine prochaine.

Retour à [[{library_page}]].
""")),
        W.sketch: (stamp(-1, "16:00"), sketch_png()),
        W.tax_note: (stamp(-9, "18:00"), t(f"""# Income tax 2026 (2025 income)

- The notice came on {tax_day}: [the message]({mid(by_key['tax'])}).
- [[Marc Duval]] wants the 2025 receipts before the end of October.
- Professional income declared by Marc; nothing else to add this year.

## Receipts
- [ ] Bank statements, January to December
- [ ] Software and fonts
- [x] Hosting invoices

#taxes
""", f"""# Impôt sur le revenu 2026 (revenus 2025)

- L'avis est arrivé le {tax_day} : [le message]({mid(by_key['tax'])}).
- [[Marc Duval]] veut les justificatifs 2025 avant la fin octobre.
- Revenus professionnels déclarés par Marc ; rien d'autre à ajouter cette année.

## Justificatifs
- [ ] Relevés bancaires, de janvier à décembre
- [ ] Logiciels et polices
- [x] Factures d'hébergement

#impôts
""")),
        t("Admin/Health cover.md", "Administratif/Complémentaire santé.md"): (stamp(-8, "15:00"), t("""# Health cover

Refund for the consultation of 22 September: €18.50, where €25 was expected. To ask them, by phone (mornings are quieter).

Office hours: Monday to Friday, 9:00–12:00 and 14:00–16:30. #health
""", """# Complémentaire santé

Remboursement de la consultation du 22 septembre : 18,50 €, au lieu des 25 € attendus. À leur demander par téléphone (le matin, c'est plus calme).

Horaires : du lundi au vendredi, 9 h – 12 h et 14 h – 16 h 30. #santé
""")),
        t("Admin/Flat lease.md", "Administratif/Bail de l'appartement.md"): (stamp(-30, "10:00"), t("""# The flat's lease

Signed on 1 March 2024 with Hillside Lets, three years, renewed by itself.
Notice: one month (furnished flat).

The boiler is checked every year in November: the agency calls. #home
""", """# Le bail de l'appartement

Signé le 1er mars 2024 avec l'Agence des Coteaux, pour trois ans, renouvelé de lui-même.
Préavis : un mois (meublé).

La chaudière est révisée chaque année en novembre : l'agence appelle. #maison
""")),
        t("Home/Lentil soup.md", "Maison/Soupe de lentilles.md"): (stamp(-5, "19:10"), t("""# Lentil soup

- 250 g green lentils, rinsed
- 1 onion, 2 carrots, 1 celery stick, diced
- 1 tsp cumin, 1 bay leaf
- 1.2 l water or stock

Soften the vegetables, add the rest, 35 minutes. Lemon at the end. #recipes
""", """# Soupe de lentilles

- 250 g de lentilles vertes, rincées
- 1 oignon, 2 carottes, 1 branche de céleri, en dés
- 1 c. à café de cumin, 1 feuille de laurier
- 1,2 l d'eau ou de bouillon

Faire revenir les légumes, ajouter le reste, 35 minutes. Un trait de citron à la fin. #recettes
""")),
        t("Home/Reading list.md", "Maison/Liste de lecture.md"): (stamp(-11, "21:00"), t("""# Reading list

- [x] The Overstory
- [ ] A Psalm for the Wild-Built
- [ ] The Hidden Life of Trees

#reading
""", """# Liste de lecture

- [x] L'Arbre-monde
- [ ] Un psaume pour les recyclés sauvages
- [ ] La Vie secrète des arbres

#lecture
""")),
        W.choir_note: (stamp(-2, "19:00"), t("""# Winter concert

Saturday 12 December, 20:00, at the community hall.

Programme: Ave verum (Mozart), two old carols, Rutter's Gaelic blessing.
- [ ] Alto part of the Ave verum, second page
- [ ] Black clothes, a coloured scarf

[[Priya Nair]] directs. #choir
""", """# Concert d'hiver

Samedi 12 décembre, 20 h, à la salle des fêtes.

Programme : Ave verum (Mozart), deux vieux noëls, la bénédiction gaélique de Rutter.
- [ ] Partie d'alto de l'Ave verum, deuxième page
- [ ] Vêtements noirs, une écharpe de couleur

[[Priya Nair]] dirige. #chorale
""")),
        t("Ideas/Poster series.md", "Idées/Série d'affiches.md"): (stamp(-9, "10:00"), t("""# Poster series: six bridges

One colour each, the same horizon line. The blue one first, says Tom.

Next: a page in the portfolio, then prints for the craft fair. #posters
""", """# Série d'affiches : six ponts

Une couleur chacune, la même ligne d'horizon. La bleue d'abord, dit Tom.

Ensuite : une page dans le portfolio, puis des tirages pour le salon des créateurs. #affiches
""")),
        t("Ideas/Craft fair.md", "Idées/Salon des créateurs.md"): (stamp(-1, "18:30"), t(f"""# December craft fair

A shared stand with [[{W.tom[0]}]] again? Last year: posters on the left, his prints on the right.

- Prints: 30 of each bridge
- A small table, two chairs
#posters
""", f"""# Salon des créateurs de décembre

Un stand partagé avec [[{W.tom[0]}]] à nouveau ? L'an dernier : les affiches à gauche, ses gravures à droite.

- Tirages : 30 de chaque pont
- Une petite table, deux chaises
#affiches
""")),
        t("Journal/This week.md", "Journal/Cette semaine.md"): (stamp(0, "08:50"), t("""# This week

- Monday: the mock-up, slowly. The review at 10:15.
- Tuesday: the doctor at 9:30.
- Thursday: choir.
- Saturday: climbing with Jonas, if the weather holds.

One thing at a time.
""", """# Cette semaine

- Lundi : la maquette, tranquillement. La revue à 10 h 15.
- Mardi : le médecin à 9 h 30.
- Jeudi : chorale.
- Samedi : escalade avec Jonas, si le temps le permet.

Une chose à la fois.
""")),
        f"{W.iris[0]}.md": (stamp(-6, "12:00"), t(f"""# Iris Calloway

Head librarian at Fernhill. Prefers calls in the morning, and short messages.

See [[{library_page}]].
""", f"""# Iris Calvet

Directrice de la Médiathèque des Fougères. Préfère les appels le matin, et les messages courts.

Voir [[{library_page}]].
""")),
        "Marc Duval.md": (stamp(-20, "12:00"), t("""# Marc Duval

Accountant, Villefranche-sur-Saône. The quarterly declaration, and the yearly one in spring.
""", """# Marc Duval

Expert-comptable, Villefranche-sur-Saône. La déclaration trimestrielle, et l'annuelle au printemps.
""")),
        "Priya Nair.md": (stamp(-40, "12:00"), t("""# Priya Nair

Directs Two Rivers Voices. Rehearsals on Thursdays at 18:45.
""", """# Priya Nair

Dirige le Chœur des Deux Rivières. Répétitions le jeudi à 18 h 45.
""")),
        f"{W.tom[0]}.md": (stamp(-40, "12:00"), t("""# Tom Achebe

Illustrator, Inkwell Studio. Lunch most Fridays. See [[Craft fair]].
""", """# Tom Achard

Illustrateur, Atelier L'Encrier. Déjeuner presque tous les vendredis. Voir [[Salon des créateurs]].
""")),
    }
    for path, (moment, content) in notes.items():
        p.write(n / path, content, moment)

    # Projects.
    p.write(n / "sioul-projects.toml", f"""# Projects (docs/notes-folder.md, docs/projects.md).

[[project]]
id = "fernhill"
title = {toml_string(W.project_title)}
kind = "project"
client = {toml_string(W.library_name)}
rate = 55.0
budget = "work"
status = "open"
files = {toml_list([W.project_note, W.meeting_note])}
[[project.route]]
from_domains = ["{W.library}"]
[[project.route]]
from_addresses = ["notifications@forge.example.com"]
subject_contains = ["{W.repository}"]

[[project]]
id = "taxes-2026"
title = {toml_string(t("Income tax 2026", "Impôt sur le revenu 2026"))}
status = "waiting"
files = {toml_list([W.tax_note])}
[[project.route]]
from_domains = ["{W.tax}"]
[[project.route]]
from_domains = ["{W.accounts}"]

[[project]]
id = "choir"
title = {toml_string(t("Winter concert", "Concert d'hiver"))}
area = "personal"
status = "open"
files = {toml_list([W.choir_note])}
[[project.route]]
from_domains = ["{W.choir}"]
""")

    write_budgets(p, by_key)
    write_bank(p)
    write_contracts(p)
    write_papers(p)


def write_budgets(p: Profile, mail: dict):
    c = p.clock
    first = c.today.replace(day=1)
    paid_sep = paid_day(c)
    presets = [
        ("hosting", "work", W.nimbus, -12, "month", 10, None, False),
        ("software", "work", t("Design software", "Logiciel de design"), -24, "month", 18, None, False),
        ("rent", "household", t("Rent", "Loyer"), -620, "month", 5, None, False),
        ("insurance", "household", t("Home insurance", "Assurance habitation"), -16.40, "month", 8, None, False),
        ("phone", "household", t("Mobile phone", "Forfait mobile"), -19.99, "month", 12, None, False),
        ("energy", "household", t("Brightwatt electricity", "Électricité Clairwatt"), -64, "month", 15, None, False),
        ("food", "household", t("Food (estimate)", "Courses (estimation)"), -300, "month", 31, None, True),
        ("gym", "leisure", t("Bloc & Boulder climbing gym", "Salle d'escalade Bloc et Prise"), -35, "month", 3, None, False),
        ("outings", "leisure", t("Outings and books (estimate)", "Sorties et livres (estimation)"), -95, "month", 31, None, True),
        ("choir-fee", "leisure", t("Choir fee", "Cotisation de la chorale"), -90, "year", 20, 9, False),
    ]
    preset_text = "".join(f"""
[[preset]]
id = "{pid}"
budget = "{budget}"
label = {toml_string(label)}
amount = {amount}
every = "{every}"
{f"month = {month}" + chr(10) if month else ""}day = {day}
{"estimate = true" + chr(10) if estimate else ""}""" for pid, budget, label, amount, every, day, month, estimate in presets)
    splits = [
        ([W.energy_name.split()[0].lower()], None, "energy", None),
        ([t("wavecell", "ondea")], None, "phone", None),
        ([t("hillside lets", "agence coteaux")], None, "rent", None),
        ([t("keystone insurance", "cle de voute")], None, "insurance", None),
        (["nimbus"], None, "hosting", None),
        ([t("design software", "logiciel design")], None, "software", None),
        (t("fernhill|silkworks|print", "fougeres|soierie|tirage").split("|"), "credit", None, "work"),
        ([t("bloc & boulder", "bloc et prise")], None, "gym", None),
        ([t("two rivers voices", "deux rivieres")], None, "choir-fee", None),
        (t("cinema|bookshop|rail|tidal notes", "cinema|librairie|billets|disquaire").split("|"), None, None, "leisure"),
        (t("market|greengrocer|bakery|supermarket|pharmacy", "supermarche|primeur|halles|boulangerie|pharmacie").split("|"), None, "food", None),
        ([t("health cover", "caisse sante")], "credit", None, "household"),
    ]
    split_text = "".join(f"""
[[split]]
words = {toml_list(words)}
{f'direction = "{direction}"' + chr(10) if direction else ""}{f'preset = "{preset}"' + chr(10) if preset else ""}{f'budget = "{budget}"' + chr(10) if budget else ""}""" for words, direction, preset, budget in splits)
    p.write(p.notes / "sioul-budgets.toml", f"""# Budgets, reserves and bank accounts (docs/accounting.md). Amounts in euros:
# positive in, negative out.

# Work pays for itself and for the rest: what it should leave each month.
[[budget]]
id = "work"
title = {toml_string(t("Work", "Travail"))}
period = "month"
target = 1250
area = "work"

# What keeps the flat and its people going: about this much a month.
[[budget]]
id = "household"
title = {toml_string(t("Household duties", "Charges du foyer"))}
period = "month"
target = -1050
area = "admin"

[[budget]]
id = "leisure"
title = {toml_string(t("Leisure", "Loisirs"))}
period = "month"
target = -130
area = "leisure"

# What comes back each month or year, and what is spent without a trace.
{preset_text}
# By hand: an invoice paid, and the two expected this month.
[[line]]
budget = "work"
date = {paid_sep}
amount = 990
label = {toml_string(t("Invoice 2026-014, Fernhill Library", "Facture 2026-014, Médiathèque des Fougères"))}
links = ["sioul:invoice/2026-014", "sioul:project/fernhill"]

[[line]]
budget = "work"
date = {first.replace(day=20)}
amount = 720
label = {toml_string(t("Invoice 2026-016, Silkworks Studio (illustrations), expected", "Facture 2026-016, Studio La Soierie (illustrations), attendue"))}
planned = true

[[line]]
budget = "work"
date = {first.replace(day=22)}
amount = 660
label = {toml_string(t("Invoice 2026-015, Fernhill Library (mock-up), expected", "Facture 2026-015, Médiathèque des Fougères (maquette), attendue"))}
planned = true
links = ["sioul:project/fernhill"]

# Mail that becomes a line: proposed on the Budgets page.
[[mail_rule]]
budget = "household"
direction = "debit"
preset = "energy"
from_domains = ["{W.energy}"]

[[mail_rule]]
budget = "household"
direction = "debit"
preset = "phone"
from_domains = ["{W.phone}"]

[[mail_rule]]
budget = "household"
direction = "credit"
from_domains = ["{W.health}"]
subject_contains = [{toml_string(t("refund", "remboursement"))}]

[[mail_rule]]
budget = "work"
direction = "debit"
preset = "hosting"
from_domains = ["nimbus.example.com"]
subject_contains = [{toml_string(t("invoice", "facture"))}]

[[mail_rule]]
budget = "work"
direction = "debit"
from_domains = ["{W.foundry}"]

# Savings: an instant one (money at once) and a long-term one (two weeks to come).
[[reserve]]
id = "instant"
title = {toml_string(t("Instant savings", "Livret d'épargne"))}
balance = 3850
as_of = {first}
floor = 1000
delay_days = 0

[[reserve]]
id = "longterm"
title = {toml_string(t("Long-term savings", "Épargne longue"))}
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
title = {toml_string(W.bank_name)}
fills = ["household", "leisure", "work"]
floor = 200
topped_up_by = ["instant", "longterm"]

[[bank_account]]
id = "paypal"
title = "PayPal"
kind = "paypal"
fills = ["work", "leisure"]

# Where each movement goes.
{split_text}
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
        add("bank", d(5), -620.00, t("PRLV HILLSIDE LETS LOYER", "PRLV AGENCE COTEAUX LOYER"))
        add("bank", d(8), -16.40, t("PRLV KEYSTONE INSURANCE HOME", "PRLV ASSURANCES CLE DE VOUTE HABITATION"))
        add("bank", d(10), -12.00, t("PRLV NIMBUS HOSTING", "PRLV NIMBUS HEBERGEMENT"))
        add("bank", d(12), -19.99, t("PRLV WAVECELL MOBILE", "PRLV ONDEA MOBILE"))
        add("bank", d(15), -61.80 if m != months[-1] else -64.20, t("PRLV BRIGHTWATT", "PRLV CLAIRWATT"))
        add("bank", d(3), -35.00, t("PRLV BLOC & BOULDER", "PRLV BLOC ET PRISE ESCALADE"))
        add("bank", d(18), -24.00, t("PRLV DESIGN SOFTWARE SUBSCRIPTION", "PRLV LOGICIEL DESIGN ABONNEMENT"))
        for day, amount, label in [(4, -42.30, t("CB SUPERMARKET STRAND", "CB SUPERMARCHE QUAI")), (9, -18.20, t("CB GREENGROCER QUAI", "CB PRIMEUR QUAI")),
                                   (13, -27.60, t("CB MARKET HALL", "CB HALLES MARCHE")), (17, -9.40, t("CB BAKERY KIOSK", "CB BOULANGERIE KIOSQUE")),
                                   (21, -51.10, t("CB SUPERMARKET STRAND", "CB SUPERMARCHE QUAI")), (25, -16.80, t("CB MARKET HALL", "CB HALLES MARCHE"))]:
            add("bank", d(day), amount, label)
    # What does not come back.
    for offset, amount, label in [(-50, -9.50, "CB CINEMA STUDIO 9"),
                                  (-44, 660.00, t("VIR FERNHILL LIBRARY FACT 2026-013", "VIR MEDIATHEQUE FOUGERES FACT 2026-013")),
                                  (-31, -23.00, t("CB RAIL TICKETS", "CB BILLETS TRAIN")),
                                  (-26, 450.00, t("VIR SILKWORKS STUDIO FACT 2026-012", "VIR STUDIO LA SOIERIE FACT 2026-012")),
                                  (-24, -8.60, t("CB PHARMACY CENTRAL", "CB PHARMACIE CENTRALE")), (-20, -14.50, t("CB BOOKSHOP LA PAGE", "CB LIBRAIRIE LA PAGE")),
                                  (-13, 18.50, t("VIR HEALTH COVER REFUND", "VIR CAISSE SANTE REMBOURSEMENT")),
                                  (-15, -90.00, t("VIR TWO RIVERS VOICES CHOIR FEE", "VIR CHOEUR DEUX RIVIERES COTISATION"))]:
        add("bank", c.weekday_before(offset), amount, label)
    rows.append((paid_day(c), "bank", 990.00, t("VIR FERNHILL LIBRARY FACT 2026-014", "VIR MEDIATHEQUE FOUGERES FACT 2026-014")))
    for offset, amount, label in [(-40, -9.00, t("Tidal Notes music", "Disquaire Notes")), (-20, 25.00, t("Payment from Ewan Price (print)", "Paiement de Lucie Perrin (tirage)")),
                                  (-4, -48.00, t("Foundry North", "Fonderie du Nord")), (-11, -12.00, W.paper_name),
                                  (-33, 40.00, t("Payment from Lise Moreau (print)", "Paiement de Lise Moreau (tirage)"))]:
        add("paypal", c.weekday_before(offset), amount, label)
    rows.sort()
    bank_balance = 1846.27
    paypal_balance = sum(r[2] for r in rows if r[1] == "paypal") + 61.50
    out = ["# Your bank's movements, read from its exports (docs/accounting.md). Kept here only.", "",
           "[[account]]", 'id = "bank"', f"title = {toml_string(W.bank_name)}", f"balance = {bank_balance:.2f}", f"as_of = {end}", "",
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
title = {toml_string(t("Flat lease", "Bail de l'appartement"))}
party = {toml_string(W.lets)}
reference = "HL-2291"
preset = "rent"
started = 2024-03-01
renews = {year + 1}-03-01
every = "year"
notice_days = 30
cancel = {toml_string(W.lets + ", " + street("4 Rue de l'Exemple") + ", 69002 Lyon")}
notes = {toml_string(t("Furnished flat: one month's notice.", "Meublé : un mois de préavis."))}

[[contract]]
id = "electricity"
kind = "energy"
title = {toml_string(t("Electricity", "Électricité"))}
party = {toml_string(W.energy_name)}
reference = "BW-118204"
preset = "energy"
started = 2025-01-15
cancel = "https://{W.energy}/{t('account/close', 'compte/resilier')}"

[[contract]]
id = "mobile"
kind = "telecom"
title = {toml_string(t("Mobile phone", "Forfait mobile"))}
party = {toml_string(W.phone_name)}
reference = "WC-77310"
preset = "phone"
started = 2023-06-12
renews = {c.today.replace(day=12)}
every = "month"
notice_days = 10
cancel = "https://{W.phone}/{t('cancel', 'resilier')}"

[[contract]]
id = "home-insurance"
kind = "insurance"
title = {toml_string(t("Home insurance", "Assurance habitation"))}
party = {toml_string(W.insurer)}
reference = "LI-55-0192"
preset = "insurance"
started = 2024-03-01
renews = {year + 1}-03-01
every = "year"
notice_days = 30
covers = {toml_string(t("Tenant's liability, water damage, legal protection.", "Responsabilité civile du locataire, dégâts des eaux, protection juridique."))}
paper = "home-insurance"
cancel = {toml_string(W.insurer + ", " + street("7 Rue Fictive") + ", 69003 Lyon")}

[[contract]]
id = "hosting"
kind = "hosting"
title = {toml_string(t("Web hosting", "Hébergement web"))}
party = {toml_string(W.nimbus)}
reference = "NH-40112"
preset = "hosting"
started = 2022-02-10
renews = {c.today.replace(day=10)}
every = "month"
cancel = "https://nimbus.example.com/panel/{t('close', 'resilier')}"

[[contract]]
id = "gym"
kind = "subscription"
title = {toml_string(t("Climbing gym", "Salle d'escalade"))}
party = {toml_string(W.gym)}
reference = {toml_string(t("Member 2214", "Adhérent 2214"))}
preset = "gym"
started = 2025-12-01
renews = {year}-12-01
every = "year"
notice_days = 30
cancel = "https://bloc-boulder.example.org/{t('membership', 'adhesion')}"
""")


def write_papers(p: Profile):
    c = p.clock
    passport_until = c.day(74)
    holder = t("Holder: Noa Ferrand", "Titulaire : Noa Ferrand")
    papers = [
        ("passport", "passport", t("Passport", "Passeport"), t("Passport.pdf", "Passeport.pdf"), date(2016, 12, 21), passport_until,
         [t("Passport", "Passeport"), holder, t("Issued: 21 December 2016", "Délivré le 21 décembre 2016"),
          t(f"Valid until: {long_date(passport_until, year=True)}", f"Valable jusqu'au {long_date(passport_until, year=True)}")]),
        ("id-card", "identity", t("Identity card", "Carte d'identité"), t("Identity card.pdf", "Carte d'identité.pdf"), date(2021, 5, 10), date(2031, 5, 9),
         [t("Identity card", "Carte d'identité"), holder, t("Valid until: 9 May 2031", "Valable jusqu'au 9 mai 2031")]),
        ("health-card", "health-card", t("Health card", "Carte Vitale"), t("Health card.pdf", "Carte Vitale.pdf"), date(2019, 9, 2), None,
         [t("Health card", "Carte Vitale"), holder]),
        ("health-cover", "health-cover", t("Health cover certificate", "Attestation de complémentaire santé"), t("Health cover.pdf", "Complémentaire santé.pdf"),
         date(c.today.year, 4, 1), date(c.today.year + 1, 3, 31),
         [t("Health cover certificate", "Attestation de complémentaire santé"), W.health_name,
          t(f"Valid until: 31 March {c.today.year + 1}", f"Valable jusqu'au 31 mars {c.today.year + 1}")]),
        ("home-insurance", "insurance", t("Home insurance certificate", "Attestation d'assurance habitation"), t("Home insurance.pdf", "Assurance habitation.pdf"),
         date(2026, 3, 1), date(2027, 2, 28),
         [t("Home insurance certificate", "Attestation d'assurance habitation"), W.insurer, t("Tenant's liability", "Responsabilité civile du locataire")]),
        ("tax-notice", "tax-notice", t("Tax notice 2026 (2025 income)", "Avis d'impôt 2026 (revenus 2025)"), t("Tax notice 2026.pdf", "Avis d'impôt 2026.pdf"),
         c.weekday_before(-10), None, [t("Tax notice 2026", "Avis d'impôt 2026"), t("2025 income", "Revenus 2025")]),
        ("rent-receipt", "rent-receipt", t("Rent receipt, last month", "Quittance de loyer, mois dernier"), t("Rent receipt.pdf", "Quittance de loyer.pdf"),
         c.today.replace(day=1) - timedelta(days=1), None, [t("Rent receipt", "Quittance de loyer"), W.lets, t("Rent paid in full", "Loyer payé en totalité")]),
        ("bank-details", "bank-details", t("Bank details (Riverside Bank)", "RIB (Banque des Berges)"), t("Bank details.pdf", "RIB.pdf"), date(2024, 3, 1), None,
         [t("Bank details", "Relevé d'identité bancaire"), W.bank_name, "IBAN FR76 0000 0000 0000 0000 0000 000"]),
        ("laptop", "warranty", t("Laptop: proof of purchase", "Ordinateur portable : preuve d'achat"), t("Laptop invoice.pdf", "Facture ordinateur.pdf"),
         date(2025, 2, 14), date(2027, 2, 13),
         [t("Invoice", "Facture"), t("Laptop, 14 inches", "Ordinateur portable, 14 pouces"), t("Two years' warranty", "Garantie de deux ans")]),
    ]
    out = ["# The papers wallet (docs/papers.md).", ""]
    for pid, kind, title, file, issued, until, text in papers:
        p.write(p.notes / "papers" / file, pdf(text + ["", t("Specimen: invented for Sioul's demo.", "Spécimen : inventé pour la démo de Sioul.")]),
                c.at(issued if issued <= c.today else c.today, "12:00"))
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
    first_stage = t("Kick-off at the library|Site map|Content inventory|Typefaces|Colours|Wireframes|Navigation|Accessibility review|Revisions",
                    "Lancement à la médiathèque|Plan du site|Inventaire des contenus|Polices|Couleurs|Maquettes fil de fer|Navigation|Revue d'accessibilité|Corrections").split("|")
    for note, offset in zip(first_stage, (-28, -27, -26, -25, -24, -21, -20, -19, -18)):
        session(c.monday + timedelta(days=offset), "09:30", 120, project="fernhill", note=note, invoice="2026-014")
    # The mock-up, not billed yet; the portfolio, for no one. A task's sessions
    # carry no note but the last one, where you stopped.
    for offset, start, minutes, task, note in [
        (-14, "09:10", 90, "photos", ""), (-13, "10:00", 60, "", t("Meeting at the library", "Réunion à la médiathèque")),
        (-13, "14:20", 45, "", t("Site map, second version", "Plan du site, deuxième version")),
        (-12, "09:30", 80, "", t("Content of the new pages", "Contenu des nouvelles pages")), (-11, "14:00", 45, "hours-page", ""), (-11, "15:00", 40, "portfolio", ""),
        (-10, "09:15", 75, "", t("Typefaces and colours, again", "Polices et couleurs, encore")),
        (-7, "09:05", 50, "", t("Accessibility notes", "Notes d'accessibilité")), (-7, "14:10", 35, "portfolio", ""),
        (-6, "10:00", 30, "", t("Call with Oskar", "Appel avec Oscar")),
        (-6, "14:15", 60, "photos", ""), (-5, "09:20", 70, "", "Navigation"), (-4, "09:00", 80, "sketch", ""), (-4, "14:30", 30, "portfolio", ""),
        (-3, "10:10", 45, "", t("Notes for the review", "Notes pour la revue")),
    ]:
        session(c.monday + timedelta(days=offset), start, minutes, task=task, project="" if task else "fernhill", note=note)
    for offset in range(0, (c.today - c.monday).days):
        session(c.monday + timedelta(days=offset), "14:15", 50, project="fernhill", note=t("Homepage, details", "Page d'accueil, détails"))
    session(c.today, "11:20", 40, task="sketch", note=t("the left layout's header, with the serif title", "l'en-tête de la mise en page de gauche, avec le titre à empattements"))

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
project_title = {toml_string(W.project_title)}
client = {toml_string(W.library_name)}
client_address = {toml_string(street("2 Rue de l'Exemple") + chr(10) + "69006 Lyon")}
total_cents = {total}
currency = "EUR"
sessions = {toml_list(keys)}
paid = true

[issuer]
name = "Noa Ferrand"
address = {toml_string(street("8 Rue de l'Exemple") + chr(10) + "69004 Lyon")}
siret = "000 000 000 00000"
vat = {toml_string(t("VAT not applicable, art. 293 B of the French tax code", "TVA non applicable, art. 293 B du CGI"))}
payment = "IBAN FR76 0000 0000 0000 0000 0000 000"

""" + "\n".join(lines))


def write_health(p: Profile, asked: bool = False):
    c = p.clock
    p.write(p.data / "health.toml", f"""errands_list = "{DAV}/home-tasks"

[[prescription]]
id = "levothyroxine"
title = {toml_string(t("Levothyroxine 75 µg", "Lévothyroxine 75 µg"))}
prescriber = "Dr Elena Varga"
until = {c.day(118)}
refill_days = 28
last_refill = {c.day(-9)}

[[medicine]]
id = "levothyroxine"
name = "Thyrolan"
generic = {toml_string(t("levothyroxine", "lévothyroxine"))}
strength = "75 µg"
since = {c.day(-430)}
dose = {toml_string(t("1 tablet", "1 comprimé"))}
prescription = "levothyroxine"
schedule = {{ every = "day", times = ["07:30"] }}

[[medicine]]
id = "magnesium"
name = {toml_string(t("Magnesium", "Magnésium"))}
generic = {toml_string(t("magnesium lactate", "lactate de magnésium"))}
strength = {toml_string(t("48 mg per tablet", "48 mg par comprimé"))}
since = {c.day(-12)}
dose = {toml_string(t("12:30 · 1 tablet, 20:00 · 2 tablets", "12:30 · 1 comprimé, 20:00 · 2 comprimés"))}
schedule = {{ every = "day", times = ["12:30", "20:00"], amounts = {{ "12:30" = {toml_string(t("1 tablet", "1 comprimé"))}, "20:00" = {toml_string(t("2 tablets", "2 comprimés"))} }} }}
until = {c.day(20)}

[movement]
enabled = true
minutes = 45

[chats]
enabled = true
minutes = 45
locked_minutes = 60

[needs]
meals_on = true
naps_on = true
sleep_on = true

[[needs.naps]]
at = "14:00"
minutes = 20
after = 15
days = [false, false, false, false, false, true, true]

[needs.sleep]
bed = "23:00"
wake = "07:00"
wind_down = 60
""")
    # Days that differ from the usual one (the Health page's week): lunch later
    # tomorrow, dinner without notices the day after, a snack added the next.
    p.write(p.data / "health-days.toml", f"""# Meals, naps and nights changed for one day only (Sioul's Health page).

[{c.day(1)}."meal:1"]
at = "13:15"

[{c.day(2)}."meal:2"]
quiet = true

[{c.day(3)}.added-1]
kind = "meal"
name = {toml_string(t("Snack", "En-cas"))}
at = "16:00"
minutes = 15
before = 0
after = 0
""")
    taken = []
    reminded = []
    unshown = []
    doses = [(-1, "levothyroxine", "07:30", 9), (-1, "magnesium", "12:30", 20), (-1, "magnesium", "20:00", 35),
             (0, "levothyroxine", "07:30", 11), (0, "magnesium", "12:30", 14), (0, "magnesium", "20:00", 10)]
    for offset, medicine, hhmm, late in doses:
        moment = c.at(c.day(offset), hhmm)
        if moment + timedelta(minutes=30) < c.now:
            key = f'"{medicine}@{unix(moment)}"'
            # --doses-asked: the morning's due while Sioul was closed; the noon's, its reminder not shown.
            if asked and offset == 0 and hhmm == "07:30":
                continue
            if asked and offset == 0 and hhmm == "12:30":
                unshown.append(f"{key} = {unix(moment)}")
                continue
            taken.append(f"{key} = {unix(moment + timedelta(minutes=late))}")
            reminded.append(f"{key} = {unix(moment)}")
    state = "[taken]\n" + "\n".join(taken) + "\n\n[reminded]\n" + "\n".join(reminded) + "\n"
    if unshown:
        state += "\n[unshown]\n" + "\n".join(unshown) + "\n"
    p.write(p.state / "health-state.toml", state)


def write_weather(p: Profile):
    c = p.clock
    hour0 = c.now.replace(minute=0, second=0)
    hours = []
    # Three days of hours and nine of days, as Sioul asks Open-Meteo (weather.rs, `query`).
    for i in range(72):
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
    # The days, from today: an autumn week, rain on the third and the sixth.
    days = []
    for i in range(9):
        code, rain = [(61, 40), (2, 10), (63, 80), (3, 20), (1, 0), (61, 60), (0, 0), (2, 10), (3, 15)][i]
        days.append({"date": (c.now + timedelta(days=i)).date().isoformat(), "code": code, "low": 7.0 + i % 3, "high": 15.0 + (i * 7) % 5, "rain": rain})
    p.write(p.state / "weather.json", json.dumps({"fetched": unix(c.now), "hours": hours, "days": days}))


def write_site_news(p: Profile):
    c = p.clock
    p.write(p.state / "site-notices.toml", f"""[[notice]]
site = "chat"
title = {toml_string(W.tom[0])}
text = {toml_string(t("Shall we print the posters at the same place as last year?", "On imprime les affiches au même endroit que l'an dernier ?"))}
at = {unix(c.ago(hours=1, minutes=5))}

[[notice]]
site = "chat"
title = {toml_string(t("Studio friends", "Les amis de l'atelier"))}
text = {toml_string(t("Lise: the scanner at the co-working space works again.", "Lise : le scanner de l'espace de coworking remarche."))}
at = {unix(c.ago(minutes=40))}

[[notice]]
site = "bank"
title = {toml_string(W.bank_name)}
text = {toml_string(t("A new document in your secure space.", "Un nouveau document dans votre espace sécurisé."))}
at = {unix(c.at(c.weekday_before(-3), "08:10"))}
""")


# --- The configuration --------------------------------------------------------

WEEKDAYS = ("monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday")


def write_config(p: Profile):
    # Admin on Tuesday and Thursday evenings, and on the day the profile is made
    # for, so that its day shows both kinds of hours.
    admin_days = [d for d in WEEKDAYS[:5] if d in ("tuesday", "thursday") or (WEEKDAYS.index(d) == p.clock.today.weekday())]
    hours = """
# The week's hours (docs/areas.md): work, then your own admin. Every other
# time is leisure; meals and sleep come from Health. With no hours at all, the
# Porch is always open and asks for them.
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
""" for day in admin_days)
    sites = [
        # Named as you would name them; short enough for the column of sites.
        ("bank", t(W.bank_name, "Ma banque"), f"https://secure.{W.bank}/", "mailbox", [t("Bank", "Banque")], "admin", W.bank, None),
        ("taxes", t(W.tax_name, "Impôts"), f"https://{t('my', 'mon')}.{W.tax}/", "mailbox", [t("Taxes", "Impôts")], "admin", W.tax, None),
        ("healthcover", t("Health cover", "Caisse santé"), f"https://{t('members', 'adherents')}.{W.health}/", "mailbox", [t("Health", "Santé")], "admin", W.health, None),
        ("chat", t("Studio chat", "Chat de l'atelier"), "https://chat.example.com/", "chat", [t("Friends", "Amis"), t("Work", "Travail")], "work+leisure",
         "chat.example.com", False),
        ("meet", t("Video calls", "Visios"), "https://meet.example.com/noa", "video", [t("Work", "Travail")], "work", "meet.example.com", None),
    ]
    site_text = "".join(f"""
[[site]]
id = "{sid}"
name = {toml_string(name)}
url = "{url}"
site = "{kind}"
categories = {toml_list(categories)}
area = "{area}"
{"background = false" + chr(10) if background is False else ""}announced_by = ["{announced}"]
""" for sid, name, url, kind, categories, area, announced, background in sites)
    p.write(p.config / "config.toml", f"""# Sioul's demo profile: an invented life, made by tools/demo/make-demo.py.
# Nothing here is real: the people, the companies, the messages. Run Sioul on it
# with SIOUL_DEMO=1, which keeps it off the network.

language = "{LANG}"

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
address = {toml_string(street("8 Rue de l'Exemple") + chr(10) + "69004 Lyon")}
siret = "000 000 000 00000"
vat = {toml_string(t("VAT not applicable, art. 293 B of the French tax code", "TVA non applicable, art. 293 B du CGI"))}
prefix = "2026-"
rate = 50.0
currency = "EUR"
payment = {toml_string(t("IBAN FR76 0000 0000 0000 0000 0000 000, within 30 days.", "IBAN FR76 0000 0000 0000 0000 0000 000, à 30 jours."))}

# Other apps by time, set on the phone and shared with the settings (docs/attention.md,
# §1.3): the chat app let through during work, held otherwise.
[attention]
"app.org.example.chat" = ["work", "admin:later", "leisure:later", "meals:later", "sleep:later", "pause:later", "free:later", "slot:later", "dnd:later", "name=Chat"]

# The camera, microphone and speaker of calls in sites: the system's own.
[calls]
{hours if p.hours else ""}
# Time off: quiet from the first day to the last.
[[time_off]]
from = {p.clock.today.year}-12-21
until = {p.clock.today.year + 1}-01-03
label = {toml_string(t("Winter holidays", "Vacances d'hiver"))}

# Work: a freelance web designer's own address.
[[account]]
id = "{account_id('work')}"
kind = "imap"
address = "{WORK}"
name = "Noa Ferrand"
signature = {toml_string("Noa Ferrand  " + chr(10) + t("Web design and illustration, Lyon", "Web design et illustration, Lyon"))}
area = "work"
host = "imap.example.invalid"
port = 993
security = "tls"
smtp_host = "smtp.example.invalid"
smtp_port = 465
trusted_authserv_ids = ["{PROVIDER}"]

# Everything personal: admin and leisure.
[[account]]
id = "{account_id('personal')}"
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
{site_text}""")
    p.write(p.config / "known-senders.txt", f"""# Senders let in: they skip the screener.
@{W.library}
{W.tom[1]}
{W.jonas[1]}
{W.marc[1]}
@{W.choir}
sophie.marchand@{W.bank}
""")
    p.write(p.config / "safe-senders.txt", f"""# Safe: by default, their mail reaches you at any time; their calls, at any waking time.
maud.ferrand@example.org
camille.ferrand@example.org
hugo.ferrand@example.org
*@{W.choir}
# The doctor's card: its number rings at any waking time.
contact:demo-contact-varga
""")
    p.write(p.config / "neutral-senders.txt", f"""# Neutral within a safe domain: the choir's automatic mail waits for its hours.
noreply@{W.choir}
""")
    p.write(p.config / "restricted-senders.txt", f"""# Restricted: by default, their mail comes in working hours only.
*@{W.gadgets}
""")
    p.write(p.config / "blocked-senders.txt", f"""# Blocked: never, on any channel.
*@{W.deals}
promo@{W.gadgets}
# Premium-rate numbers.
tel:+33899*
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


def as_written_before(p: Profile):
    """The projects' file as a Sioul from before projects had one name wrote it
    (docs/notes-folder.md, "Its first name"): sioul-cases.toml, its entries
    [[case]], and projects' addresses as sioul:case/ in the budgets' lines."""
    new, old = p.notes / "sioul-projects.toml", p.notes / "sioul-cases.toml"
    text = new.read_text(encoding="utf-8").replace("[[project.route]]", "[[case.route]]").replace("[[project]]", "[[case]]")
    moment = datetime.fromtimestamp(new.stat().st_mtime).astimezone()
    new.unlink()
    p.write(old, text, moment)
    ledger = p.notes / "sioul-budgets.toml"
    if ledger.exists():
        moment = datetime.fromtimestamp(ledger.stat().st_mtime).astimezone()
        p.write(ledger, ledger.read_text(encoding="utf-8").replace("sioul:project/", "sioul:case/"), moment)


def main():
    global LANG, W
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--into", required=True, type=Path, help="the profile's folder")
    parser.add_argument("--now", help="the moment the profile is made for, ISO 8601 with its offset (default: now)")
    parser.add_argument("--language", choices=("en", "fr"), default="en", help="the language of the life written, and of Sioul")
    parser.add_argument("--no-hours", action="store_true", help="leave the week's hours out: the Porch asks for them")
    parser.add_argument("--notes-at", help="where Sioul finds DIR/notes when it runs (default: DIR/notes)")
    parser.add_argument("--spam", action="store_true", default=bool(os.environ.get("SIOUL_DEMO_SPAM")),
                        help="Sioul's own spam filter: a table made by hand, strangers' messages in its review queue (SIOUL_DEMO_SPAM=1 too)")
    parser.add_argument("--calls", action="store_true", default=bool(os.environ.get("SIOUL_DEMO_CALLS")),
                        help="the calls a phone of yours screened, as the sharing brings them: the Porch lists those it declined (SIOUL_DEMO_CALLS=1 too)")
    parser.add_argument("--phone-messages", action="store_true", default=bool(os.environ.get("SIOUL_DEMO_PHONE_MESSAGES")),
                        help="the messages a phone's notifications brought, as the sharing brings them, and their part on here (SIOUL_DEMO_PHONE_MESSAGES=1 too)")
    parser.add_argument("--texts", action="store_true", default=bool(os.environ.get("SIOUL_DEMO_TEXTS")),
                        help="texts: the texts a phone read and those written here for it, as the part Texts brings them (SIOUL_DEMO_TEXTS=1 too)")
    parser.add_argument("--compose", action="store_true", default=bool(os.environ.get("SIOUL_DEMO_COMPOSE")),
                        help="a message to answer on a phone's width: a long subject, replies to two addresses, a copy to a third (SIOUL_DEMO_COMPOSE=1 too)")
    parser.add_argument("--old-projects", action="store_true", default=bool(os.environ.get("SIOUL_DEMO_OLD_PROJECTS")),
                        help="the projects' file under its first name, sioul-cases.toml with [[case]], and the budgets' ties as sioul:case/: Settings offers to rename it (SIOUL_DEMO_OLD_PROJECTS=1 too)")
    parser.add_argument("--doses-asked", action="store_true", default=bool(os.environ.get("SIOUL_DEMO_DOSES_ASKED")),
                        help="today's morning dose due while Sioul was closed, and its noon dose whose reminder could not be shown, both unanswered (SIOUL_DEMO_DOSES_ASKED=1 too)")
    args = parser.parse_args()
    LANG = args.language
    W = World()
    now = datetime.fromisoformat(args.now) if args.now else datetime.now().astimezone()
    if now.tzinfo is None:
        now = now.astimezone()
    root = args.into.resolve()
    prepare(root)
    clock = Clock(now)
    profile = Profile(root, clock, args.notes_at or str(root / "notes"), not args.no_hours)
    mails = the_mail(clock) + (spam_mail(clock) if args.spam else []) + (compose_mail(clock) if args.compose else [])
    write_config(profile)
    write_mail(profile, mails)
    if args.spam:
        profile.write(profile.data / "spam" / "table.bin", spam_table(clock), clock.ago(hours=26))
        moved_log(profile, [m for m in mails if m.key == "moved"])
    write_contacts(profile)
    if args.calls:
        write_calls(profile)
    if args.phone_messages:
        write_phone_messages(profile)
    if args.texts:
        write_texts(profile)
    write_calendars(profile, mails)
    write_notes(profile, mails)
    if args.old_projects:
        as_written_before(profile)
    sessions = write_time(profile)
    write_invoices(profile, sessions)
    write_health(profile, args.doses_asked)
    write_weather(profile)
    write_site_news(profile)
    print(f"{root}: a demo profile for {clock.now.isoformat()}, in {'French' if LANG == 'fr' else 'English'}" + ("" if profile.hours else ", without hours"))


if __name__ == "__main__":
    main()
