#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre
#
# Qt for CXX-Qt in the Flatpak build. The runtime's Qt is in /usr, Qt
# WebEngine (from its BaseApp) in /app, and CXX-Qt reads one folder of
# libraries and one of headers from qmake. This gathers both into ./qt-both,
# as links, and writes ./qmake-both, which answers for them and asks the real
# qmake everything else. The build then sets QMAKE to it.
set -eu
here=$(pwd)
both="$here/qt-both"
qmake=$(command -v qmake6 || command -v qmake)
mkdir -p "$both/lib" "$both/include"
for dir in "$("$qmake" -query QT_INSTALL_LIBS)" /app/lib /app/lib/x86_64-linux-gnu /app/lib/aarch64-linux-gnu /app/lib64; do
    [ -d "$dir" ] || continue
    for file in "$dir"/libQt6*; do
        [ -e "$file" ] && ln -sf "$file" "$both/lib/"
    done
done
for dir in "$("$qmake" -query QT_INSTALL_HEADERS)" /app/include/qt6 /app/include; do
    [ -d "$dir" ] || continue
    for module in "$dir"/Qt*; do
        [ -d "$module" ] && [ ! -e "$both/include/${module##*/}" ] && ln -s "$module" "$both/include/"
    done
done
cat > "$here/qmake-both" <<WRAPPER
#!/bin/sh
case "\$2" in
    QT_INSTALL_LIBS) echo "$both/lib" ;;
    QT_INSTALL_HEADERS) echo "$both/include" ;;
    *) exec "$qmake" "\$@" ;;
esac
WRAPPER
chmod +x "$here/qmake-both"
echo "qt-both: $(ls "$both/lib" | grep -c '\.so') libraries, $(ls "$both/include" | wc -l) header folders; Qt WebEngine: $(ls "$both/lib" | grep -c 'WebEngine')"
