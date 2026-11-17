// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A client's profile (docs/projects.md, "Client profiles"): who you bill,
// as invoices need it, kept in the notes folder's sioul-clients.toml beside
// the projects, tied to its contact, never written into the card on your
// contacts' server. A business, a person or a public body; its legal name;
// its billing address with its country, and a delivery address when it
// differs; its identifiers (a SIREN, a VAT number); its invoices'
// language, currency and terms; the address they are mailed to; a purchase
// order to print. Opened from a contact's page, a project's, or the editor.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

SafeDialog {
    id: dialog

    required property var sioul
    required property var theme
    // The profile as `clientProfile` gives it (a new one filled from the card when there is none).
    property var profile: null
    // Its identifiers as typed: [{kind, value}].
    property var ids: []
    property string problem: ""
    readonly property var kinds: ["business", "consumer", "public"]
    readonly property var languages: ["", "en", "fr"]

    // Saved: the page reads the profile again.
    signal saved

    // The profile of `reference` (a contact's address, a name), to change or to make.
    function edit(reference) {
        const found = JSON.parse(dialog.sioul.clientProfile(reference) || "{}")
        dialog.profile = found
        dialog.problem = ""
        dialog.ids = (found.ids || []).map(i => ({ kind: i.kind, value: i.value }))
        kind.currentIndex = Math.max(0, dialog.kinds.indexOf(found.kind))
        name.text = found.name || ""
        lines.text = found.lines || ""
        postcode.text = found.postcode || ""
        city.text = found.city || ""
        country.text = found.country_name || found.country || ""
        delivery.text = found.delivery || ""
        language.currentIndex = Math.max(0, dialog.languages.indexOf(found.language || ""))
        currency.text = found.currency || ""
        terms.text = found.terms_days === null || found.terms_days === undefined ? "" : String(found.terms_days)
        endOfMonth.checked = found.end_of_month === true
        email.text = found.email || ""
        order.text = found.order || ""
        dialog.open()
    }

    function save() {
        const days = parseInt(terms.text, 10)
        const form = {
            id: dialog.profile.id || "",
            contact: dialog.profile.contact || "",
            kind: dialog.kinds[kind.currentIndex],
            name: name.text,
            lines: lines.text,
            postcode: postcode.text,
            city: city.text,
            country: country.text,
            delivery: delivery.text,
            ids: dialog.ids.filter(i => i.value.trim() !== ""),
            language: dialog.languages[language.currentIndex],
            currency: currency.text,
            terms_days: isNaN(days) ? null : days,
            end_of_month: endOfMonth.checked,
            email: email.text,
            order: order.text
        }
        const answer = JSON.parse(dialog.sioul.saveClientProfile(JSON.stringify(form)) || "{}")
        if (answer.error) {
            dialog.problem = answer.error
            return
        }
        dialog.close()
        dialog.saved()
    }

    objectName: "clientProfileDialog"
    parent: Overlay.overlay
    anchors.centerIn: parent
    width: Math.min(520, (parent ? dialog.roomWidth : 520) - 2 * dialog.theme.gap)
    height: Math.min(implicitHeight, (parent ? dialog.roomHeight : 800) - 2 * dialog.theme.gap)
    modal: true
    title: dialog.sioul.text("clients-profile")

    ScrollView {
        id: scroll

        anchors.fill: parent
        contentWidth: availableWidth

        ColumnLayout {
            width: scroll.availableWidth
            spacing: 6

            Label {
                Layout.fillWidth: true
                text: dialog.sioul.text("clients-profile-about")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 12
                color: dialog.theme.muted
            }
            PlainComboBox {
                id: kind

                objectName: "clientProfileKind"
                Layout.fillWidth: true
                model: dialog.kinds.map(k => dialog.sioul.text("invoices-client-kind-" + k))
            }
            TextField {
                id: name

                objectName: "clientProfileName"
                Layout.fillWidth: true
                placeholderText: dialog.sioul.text("clients-name")
            }
            TextArea {
                id: lines

                Layout.fillWidth: true
                placeholderText: dialog.sioul.text("clients-street")
                wrapMode: TextEdit.Wrap
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: 6

                TextField {
                    id: postcode

                    Layout.preferredWidth: 90
                    placeholderText: dialog.sioul.text("clients-postcode")
                }
                TextField {
                    id: city

                    Layout.fillWidth: true
                    placeholderText: dialog.sioul.text("clients-city")
                }
            }
            TextField {
                id: country

                objectName: "clientProfileCountry"
                Layout.fillWidth: true
                placeholderText: dialog.sioul.text("invoices-country")
            }
            TextArea {
                id: delivery

                Layout.fillWidth: true
                placeholderText: dialog.sioul.text("clients-delivery")
                wrapMode: TextEdit.Wrap
            }
            // Its identifiers: a SIREN, a VAT number, another.
            Label {
                Layout.topMargin: 4
                text: dialog.sioul.text("clients-ids")
                font.weight: Font.DemiBold
                color: dialog.theme.text
            }
            Repeater {
                model: dialog.ids.length

                delegate: RowLayout {
                    id: idRow

                    required property int index

                    Layout.fillWidth: true
                    spacing: 6

                    PlainComboBox {
                        model: dialog.profile ? dialog.profile.id_kinds.map(k => dialog.theme.plain(k.label)) : []
                        currentIndex: dialog.profile ? Math.max(0, dialog.profile.id_kinds.findIndex(k => k.kind === dialog.ids[idRow.index].kind)) : 0
                        onActivated: index => dialog.ids[idRow.index].kind = dialog.profile.id_kinds[index].kind
                    }
                    TextField {
                        Layout.fillWidth: true
                        text: dialog.ids[idRow.index].value
                        onTextEdited: dialog.ids[idRow.index].value = text
                    }
                    ToolButton {
                        text: "×"
                        Accessible.name: dialog.sioul.text("clients-id-remove")
                        onClicked: dialog.ids = dialog.ids.filter((_, i) => i !== idRow.index)
                    }
                }
            }
            Button {
                flat: true
                text: "+  " + dialog.sioul.text("clients-id-add")
                onClicked: dialog.ids = dialog.ids.concat([{ kind: dialog.profile && dialog.profile.id_kinds.length > 0 ? dialog.profile.id_kinds[0].kind : "other", value: "" }])
            }
            // Its invoices: language, currency, terms; where they are mailed; its order.
            Label {
                Layout.topMargin: 4
                text: dialog.sioul.text("clients-invoices")
                font.weight: Font.DemiBold
                color: dialog.theme.text
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: 6

                PlainComboBox {
                    id: language

                    Layout.fillWidth: true
                    model: dialog.languages.map(l => l === "" ? dialog.sioul.text("clients-language-usual") : dialog.sioul.text("invoices-language-" + l))
                }
                TextField {
                    id: currency

                    Layout.preferredWidth: 80
                    placeholderText: "EUR"
                    Accessible.name: dialog.sioul.text("invoices-currency")
                }
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: 6

                TextField {
                    id: terms

                    Layout.preferredWidth: 80
                    inputMethodHints: Qt.ImhDigitsOnly
                    placeholderText: dialog.sioul.text("clients-terms")
                    Accessible.name: dialog.sioul.text("invoices-terms-days")
                }
                CheckBox {
                    id: endOfMonth

                    text: dialog.sioul.text("invoices-end-of-month")
                }
            }
            TextField {
                id: email

                objectName: "clientProfileEmail"
                Layout.fillWidth: true
                inputMethodHints: Qt.ImhEmailCharactersOnly
                placeholderText: dialog.sioul.text("clients-email")
            }
            TextField {
                id: order

                Layout.fillWidth: true
                placeholderText: dialog.sioul.text("invoices-order")
            }
            Label {
                visible: dialog.problem !== ""
                Layout.fillWidth: true
                text: dialog.problem
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: dialog.theme.warm
            }
        }
    }

    footer: DialogButtonBox {
        Button {
            objectName: "clientProfileSave"
            text: dialog.sioul.text("ui-save")
            highlighted: true
            onClicked: dialog.save()
        }
        Button {
            text: dialog.sioul.text("ui-cancel")
            onClicked: dialog.close()
        }
    }
}
