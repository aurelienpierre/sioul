#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre

"""Reads Sioul's word packs for words that fit one person (docs/words.md,
"Three rules keep shipped words from being one person's").

The packs are crates/sioul-core/data/words/: a file per language, a file per
country (countries/), and international.toml. Each entry is looked at, and
flagged when it looks like one person's or one organisation's:

  address  a whole address (name@domain): one person's, or one sender's;
  domain   a domain in a list that is not one of domains (brands.brands,
           brands.shared): one organisation's;
  number   six digits or more in a row, an IBAN, a phone number (the codes'
           documented example "123456" aside): one account, one line;
  person   a common first name written with its capital, or followed by
           another capitalised word: one person's name;
  brand    in a language's pack, a word naming an organisation the packs
           name elsewhere (a brand, a body, a processor, an operator):
           one organisation's wording in everyone's list; a language's pack
           names no organisation;
  alone    in a country's or the international pack, a list of a kind
           (contracts.kinds.*, approvals.phrases) holding one name alone:
           a name ships only inside a list complete for its country and
           category.

What a reader other than a pack's author read and kept is listed in
tools/word-packs-kept.toml, each entry with why: it is not flagged again,
and an entry kept there that no pack holds any more is said, to be taken
out. A reader still reads every pack before a release; this catches what
looks tailored between two readings. Exits with 1 when a flag is left, so
that CI runs it.

    tools/check-word-packs.py                 # the packs of this tree
    tools/check-word-packs.py --all           # the kept ones too, with why
    tools/check-word-packs.py --json
    tools/check-word-packs.py --self-test     # each flag, on packs made up for it
"""

import argparse
import json
import pathlib
import re
import sys
import tempfile
import tomllib
import unicodedata

ROOT = pathlib.Path(__file__).resolve().parent.parent
PACKS = ROOT / "crates" / "sioul-core" / "data" / "words"
KEPT = ROOT / "tools" / "word-packs-kept.toml"

# Lists whose entries are domains.
DOMAIN_LISTS = ("brands.brands.", "brands.shared")
# Lists of a kind, in a country's or the international pack, that must not name one organisation alone.
KIND_LISTS = ("contracts.kinds.", "approvals.phrases")
# Example codes the code phrases are written around ("123456 is your code").
EXAMPLES = {"123456"}
DOMAIN = re.compile(r"(?<![\w@.-])(?:[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\.)+[a-z]{2,}(?![\w.-])")
ADDRESS = re.compile(r"[^\s@]+@[a-z0-9-]+(?:\.[a-z0-9-]+)+", re.I)
DIGITS = re.compile(r"\d{6,}")
IBAN = re.compile(r"\b[A-Z]{2}\d{2}(?:\s?[A-Z0-9]{4}){3,}", re.I)
PHONE = re.compile(r"(?:\+\d{1,3}[\s.]?)?(?:\(?\d{1,4}\)?[\s.-]?){3,}\d{2,4}")
# Common first names of the languages Sioul reads, written with their capital
# in a pack: a person's. Lowercase they are often words ("rose", "bill", "pierre").
FIRST_NAMES = set("""
Adam Adrien Agnès Alain Albert Alexandre Alexandra Alice Aline Amélie Anaïs André Andrea Andreas Angela Anna Anne Annie
Antoine Antonio Arnaud Arthur Aurélie Aurélien Baptiste Barbara Benjamin Benoît Bernard Bertrand Brigitte Bruno Camille
Carla Carlos Carmen Caroline Catherine Cécile Céline Charles Charlotte Chloé Christian Christine Christophe Claire
Claude Claudia Clément Corinne Cyril Damien Daniel Daniela David Denis Didier Dominique Dorothée Edouard Élise Elena
Elisabeth Elizabeth Émilie Emma Emmanuel Eric Éric Étienne Fabien Fabrice Fanny Florence Florian Francesca Francesco
Francis Franck François Françoise Frank Frédéric Gabriel Gaëlle Georges Gérard Giovanni Giulia Giuseppe Grégoire
Guillaume Gustave Hans Hélène Henri Hugo Isabelle Jacques James Jan Javier Jean Jeanne Jennifer Jérémy Jérôme Joël
Johann John Jonathan José Joseph Juan Julia Julie Julien Juliette Jürgen Karine Karl Klaus Laura Laurent Laurence Léa
Léon Louis Louise Luc Luca Lucas Lucie Luis Manon Manuel Marc Marco Margaux Maria Marie Marion Mark Martin Martine
Mathieu Mathilde Matthieu Maurice Max Maxime Michael Michel Michèle Monique Nathalie Nicolas Noémie Olivier Pascal
Pascale Patrick Paul Pauline Peter Philippe Pierre Raphaël Rémi Richard Robert Romain Sabine Sandrine Sarah Sébastien
Serge Simon Sophie Stéphane Stéphanie Sylvain Sylvie Thierry Thomas Thomas Valérie Véronique Vincent Virginie Xavier
Yann Yves Zoé
""".split())


def folded(text):
    """Trimmed, lowercase, accents aside: as Sioul compares words (words::folded)."""
    text = unicodedata.normalize("NFKD", text.strip().lower())
    return "".join(c for c in text if not unicodedata.combining(c))


def lists(table, path=""):
    """Every list of a pack, by its dotted path: ("codes.code", [...])."""
    for key, value in table.items():
        here = f"{path}.{key}" if path else key
        if isinstance(value, dict):
            yield from lists(value, here)
        elif isinstance(value, list):
            yield here, [v for v in value if isinstance(v, str)]


def packs(folder=PACKS):
    """(name, kind, table) for each pack: kind "language", "country" or "international"."""
    for path in sorted(folder.glob("*.toml")):
        table = tomllib.loads(path.read_text(encoding="utf-8"))
        yield path.stem, "international" if path.stem == "international" else "language", table
    for path in sorted((folder / "countries").glob("*.toml")):
        yield path.stem, "country", tomllib.loads(path.read_text(encoding="utf-8"))


def organisations(all_packs):
    """The organisations the country and international packs name, folded,
    with the everyday words among them left out (a brand that is also a
    word: "free", "orange", "apple"), and two-letter names (too short to tell)."""
    names, everyday = set(), set()
    for _, kind, table in all_packs:
        if kind == "language":
            continue
        brands = table.get("brands", {})
        everyday.update(folded(w) for w in brands.get("everyday", []))
        # By their names only: a domain's own name is often a word ("office" of office.com).
        names.update(folded(name) for name in brands.get("brands", {}))
        names.update(folded(name) for name in table.get("letters", {}).get("senders", {}))
        for words in table.get("contracts", {}).get("kinds", {}).values():
            names.update(folded(w) for w in words)
        names.update(folded(w) for w in table.get("bank", {}).get("processors", []))
    # Everyday words among the names, and the words of a brand's own name ("free" of "Free Mobile").
    everyday |= {part for word in everyday for part in word.split()}
    return {n for n in names if len(n) > 2 and n not in everyday and not any(n in folded(e).split() for e in everyday)}


def flags(all_packs):
    """Every flag: (pack, list, entry, kind, why)."""
    found = []
    names = organisations(all_packs)
    name_pattern = re.compile(r"(?<![\w-])(" + "|".join(sorted((re.escape(n) for n in names), key=len, reverse=True)) + r")(?![\w-])") if names else None
    for pack, kind, table in all_packs:
        for path, entries in lists(table):
            for entry in entries:
                low = folded(entry)
                if ADDRESS.search(entry):
                    found.append((pack, path, entry, "address", "a whole address"))
                elif DOMAIN.search(low) and not path.startswith(DOMAIN_LISTS):
                    found.append((pack, path, entry, "domain", "a domain in a list of words"))
                elif any(m not in EXAMPLES for m in DIGITS.findall(entry)) or IBAN.search(entry) or (PHONE.search(entry) and sum(c.isdigit() for c in entry) >= 8):
                    found.append((pack, path, entry, "number", "digits that may be one account's or one line's"))
                words = re.findall(r"[^\W\d_]+", entry)
                for i, word in enumerate(words):
                    if word in FIRST_NAMES and (len(words) == 1 or (i + 1 < len(words) and words[i + 1][:1].isupper()) or entry[:1].isupper()):
                        found.append((pack, path, entry, "person", f"{word}: a first name"))
                        break
                if kind == "language" and name_pattern and not path.startswith(DOMAIN_LISTS):
                    hit = name_pattern.search(low)
                    if hit:
                        found.append((pack, path, entry, "brand", f"{hit.group(1)}: an organisation the packs name"))
            if kind != "language" and path.startswith(KIND_LISTS) and len(entries) == 1:
                found.append((pack, path, entries[0], "alone", "one name alone in a list of its kind"))
    return found


def kept(path=KEPT):
    """The entries a reader kept: {(pack, list, entry): why}."""
    if not path.exists():
        return {}
    table = tomllib.loads(path.read_text(encoding="utf-8"))
    return {(k["pack"], k["list"], k["entry"]): k.get("why", "") for k in table.get("kept", [])}


def self_test():
    """Each flag found on packs made up for it, nothing on their plain words,
    and an entry kept no longer flagged."""
    with tempfile.TemporaryDirectory() as folder:
        folder = pathlib.Path(folder)
        (folder / "countries").mkdir()
        (folder / "en.toml").write_text('''language = "en"
[codes]
code = ["verification code", "123456 is your code", "write to jane.doe@example.org", "your code from dupont-family.example"]
[payments]
fee = ["fee", "Exemplebank fee", "account 0012345678", "call 06 12 34 56 78", "FR76 3000 6000 0112"]
[quotes]
attribution_starts = ["On", "Le"]
[tasks]
personal = ["Jean Dupont", "Claire", "garden"]
''', encoding="utf-8")
        (folder / "international.toml").write_text('''[brands]
everyday = ["Apple"]
shared = ["mail.example"]
[brands.brands]
Exemplebank = ["exemplebank.example"]
Apple = ["apple.example"]
''', encoding="utf-8")
        (folder / "countries" / "ZZ.toml").write_text('''country = "ZZ"
[contracts.kinds]
energy = ["exemple-energie"]
telecom = ["one", "two"]
''', encoding="utf-8")
        found = {(pack, entry, kind) for pack, _, entry, kind, _ in flags(list(packs(folder)))}
        wanted = {
            ("en", "write to jane.doe@example.org", "address"),
            ("en", "your code from dupont-family.example", "domain"),
            ("en", "Exemplebank fee", "brand"),
            ("en", "account 0012345678", "number"),
            ("en", "call 06 12 34 56 78", "number"),
            ("en", "FR76 3000 6000 0112", "number"),
            ("en", "Jean Dupont", "person"),
            ("en", "Claire", "person"),
            ("ZZ", "exemple-energie", "alone"),
        }
        missing, extra = wanted - found, found - wanted
        assert not missing and not extra, f"missing {sorted(missing)}, extra {sorted(extra)}"
        (folder / "kept.toml").write_text('''[[kept]]
pack = "en"
list = "tasks.personal"
entry = "Claire"
why = "a made-up example"
''', encoding="utf-8")
        assert kept(folder / "kept.toml") == {("en", "tasks.personal", "Claire"): "a made-up example"}
    print("self-test: each flag found, plain words left alone.")
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--all", action="store_true", help="the kept entries too, with why")
    parser.add_argument("--json", action="store_true", help="the report as JSON")
    parser.add_argument("--self-test", action="store_true", help="each flag, on packs made up for it")
    args = parser.parse_args()
    if args.self_test:
        return self_test()
    all_packs = list(packs())
    found = flags(all_packs)
    read = kept()
    report = []
    for pack, path, entry, kind, why in found:
        reason = read.get((pack, path, entry))
        if reason is None or args.all:
            report.append({"pack": pack, "list": path, "entry": entry, "flag": kind, "why": why, "kept": reason})
    held = {(p, l, e) for p, _, t in all_packs for l, entries in lists(t) for e in entries}
    gone = sorted(key for key in read if key not in held)
    left = [r for r in report if r["kept"] is None]
    if args.json:
        json.dump({"flags": report, "kept_but_gone": [list(g) for g in gone]}, sys.stdout, ensure_ascii=False, indent=1)
        print()
    else:
        for r in report:
            where = f"{r['pack']}.toml {r['list']}"
            said = f"kept: {r['kept']}" if r["kept"] is not None else r["why"]
            print(f"{r['flag']:8} {where:44} {r['entry']!r}  ({said})")
        for pack, path, entry in gone:
            print(f"gone     {pack}.toml {path:36} {entry!r}  (kept in tools/word-packs-kept.toml, in no pack any more: take it out)")
        entries = sum(len(e) for _, _, t in all_packs for _, e in lists(t))
        print(f"\n{entries} entries in {len(all_packs)} packs: {len(left)} flagged, {len(found) - len(left)} kept after a reading, {len(gone)} kept but gone.")
    return 1 if left or gone else 0


if __name__ == "__main__":
    sys.exit(main())
