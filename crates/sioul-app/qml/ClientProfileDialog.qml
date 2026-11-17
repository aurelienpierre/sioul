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
// Saving writes only what the form changed, key by key: what the file
// holds besides (comments, keys a newer Sioul wrote, an identifier left
// unprinted) stays as written (the review of phase 3, F6).

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
    // Its identifiers as typed: [{kind, value, printed}].
    property var ids: []
    // The form as it opened: what Save compares to send only what changed.
    property var initial: ({})
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
        dialog.ids = (found.ids || []).map(i => ({ kind: i.kind, value: i.value, printed: i.printed !== false }))
        // A kind the form does not offer shows as itself, after its own, and is kept unless another is chosen (R15).
        kind.currentIndex = dialog.kinds.indexOf(found.kind) >= 0 ? dialog.kinds.indexOf(found.kind) : dialog.kinds.length
        name.text = found.name || ""
        lines.text = found.lines || ""
        postcode.text = found.postcode || ""
        city.text = found.city || ""
        country.text = found.country_name || found.country || ""
        deliveryLines.text = found.delivery_lines || ""
        deliveryPostcode.text = found.delivery_postcode || ""
        deliveryCity.text = found.delivery_city || ""
        deliveryCountry.text = found.delivery_country || ""
        language.currentIndex = Math.max(0, dialog.languages.indexOf(found.language || ""))
        currency.text = found.currency || ""
        terms.text = found.terms_days === null || found.terms_days === undefined ? "" : String(found.terms_days)
        endOfMonth.checked = found.end_of_month === true
        email.text = found.email || ""
        order.text = found.order || ""
        dialog.initial = dialog.formNow()
        dialog.open()
    }

    // An address's street lines as typed, one a line.
    function linesOf(text) {
        return text.split("\n").map(l => l.trim()).filter(l => l !== "")
    }

    // The form as it is now, as `clients::ProfileChange` names its fields.
    function formNow() {
        return {
            kind: kind.currentIndex < dialog.kinds.length ? dialog.kinds[kind.currentIndex] : (dialog.profile ? dialog.profile.kind : "business"),
            name: name.text,
            lines: dialog.linesOf(lines.text),
            postcode: postcode.text,
            city: city.text,
            country: country.text,
            delivery_lines: dialog.linesOf(deliveryLines.text),
            delivery_postcode: deliveryPostcode.text,
            delivery_city: deliveryCity.text,
            delivery_country: deliveryCountry.text,
            ids: dialog.ids.filter(i => i.value.trim() !== "").map(i => ({ kind: i.kind, value: i.value.trim(), printed: i.printed !== false })),
            language: dialog.languages[language.currentIndex],
            currency: currency.text,
            terms_days: terms.text.trim(),
            end_of_month: endOfMonth.checked,
            email: email.text,
            order: order.text
        }
    }

    // Only what changed (a new profile: all of it), with the profile's id.
    function save() {
        const now = dialog.formNow()
        const made = dialog.profile.exists === true
        const form = { id: dialog.profile.id || "" }
        for (const key in now)
            if (!made || JSON.stringify(now[key]) !== JSON.stringify(dialog.initial[key]))
                form[key] = now[key]
        if (!made && dialog.profile.contact)
            form.contact = dialog.profile.contact
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
                model: dialog.kinds.map(k => dialog.sioul.text("invoices-client-kind-" + k)).concat(dialog.profile && dialog.profile.kind && dialog.kinds.indexOf(dialog.profile.kind) < 0 ? [dialog.theme.plain(dialog.profile.kind)] : [])
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
            // A delivery address, when it differs: its street, postcode, city and country, each kept as written.
            TextArea {
                id: deliveryLines

                objectName: "clientProfileDelivery"
                Layout.fillWidth: true
                placeholderText: dialog.sioul.text("clients-delivery")
                wrapMode: TextEdit.Wrap
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: 6

                TextField {
                    id: deliveryPostcode

                    Layout.preferredWidth: 90
                    placeholderText: dialog.sioul.text("clients-postcode")
                }
                TextField {
                    id: deliveryCity

                    Layout.fillWidth: true
                    placeholderText: dialog.sioul.text("clients-city")
                }
                TextField {
                    id: deliveryCountry

                    Layout.preferredWidth: 110
                    placeholderText: dialog.sioul.text("invoices-country")
                }
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
                    CheckBox {
                        objectName: "clientProfileIdPrinted"
                        text: dialog.sioul.text("clients-id-printed")
                        checked: dialog.ids[idRow.index].printed !== false
                        onToggled: dialog.ids[idRow.index].printed = checked
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
                onClicked: dialog.ids = dialog.ids.concat([{ kind: dialog.profile && dialog.profile.id_kinds.length > 0 ? dialog.profile.id_kinds[0].kind : "other", value: "", printed: true }])
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
