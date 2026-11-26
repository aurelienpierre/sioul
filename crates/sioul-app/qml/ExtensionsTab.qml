// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Settings ▸ Extensions (docs/extensions.md): what the signed list offers,
// each with what it brings, the sites it talks to and whether it keeps a
// sign-in, said before anything is enabled; then Enable, Sign in…, its state
// and its last problem in one sentence with that problem's own detail under
// it, Details… (the last lines of its log), the version that runs here and
// the one offered (Update to version …, or Take the update when it asks for
// more), each agenda it brought (a pupil's
// timetable) with the calendar of yours its lessons go into, chosen here
// (`extensionCalendar`), each set of contacts it brought (a business's
// clients) with the address book of yours they go into (`extensionBook`),
// Disable, Remove (which asks whether to take its lessons out of your
// calendars, and says its contacts stay), and the yes an update asks when it
// wants more.
// At its foot, the list in use (its number, when it was read, its signature
// checked), where it is read from, and Read the list again; Update all when
// several updates wait, one question first when some ask for more.
// Opening it reads the list (`extensionsPage`); the view comes back as
// `sioul.extensions` (extensions.rs). A sign-in that needs a choice first (a
// platform) asks it here (`extensionChoices`).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: tab

    required property var sioul
    required property var theme

    readonly property var view: {
        const text = tab.sioul.extensions
        return text ? JSON.parse(text) : { offers: [], calendars: [], where: "", problem: "", reading: true }
    }

    // The questions and the credentials form, over the window (their tests reach them so).
    readonly property alias removeDialog: removing
    readonly property alias chooseDialog: choosing
    readonly property alias signInFormDialog: signingIn
    readonly property alias logDialog: logCard
    readonly property alias updateAllDialog: updatingAll

    implicitHeight: column.implicitHeight
    Component.onCompleted: tab.sioul.extensionsPage()

    // The agenda whose new calendar is being made ("<extension>/<agenda>"), and what
    // Agenda ⚙ knows to make one (`agenda::setup`: its accounts, the persons each role may name).
    property string making: ""
    // The account it is made on.
    property string makingOn: ""
    property var setup: ({ accounts: [], persons: {} })

    // A calendar chosen for an agenda: kept, unless it is "Choose…" or the one already chosen.
    // A new one is made first with the form of Agenda ⚙'s New agenda… (NewAgendaForm.qml),
    // which asks whose it is; once made, it is the one chosen.
    function choose(id, agenda, choice) {
        if (choice.indexOf("new:") === 0) {
            tab.setup = JSON.parse(tab.sioul.agendasSetup() || "{}")
            tab.makingOn = choice.slice(4)
            tab.making = id + "/" + agenda.id
            return
        }
        tab.making = ""
        if (choice !== "" && choice !== "place:" + agenda.place)
            tab.sioul.extensionCalendar(id, agenda.id, choice)
    }

    // An address book chosen for a set of contacts: the same.
    function chooseBook(id, contactSet, choice) {
        if (choice !== "" && choice !== "place:" + contactSet.place)
            tab.sioul.extensionBook(id, contactSet.id, choice)
    }

    ColumnLayout {
        id: column

        width: tab.width
        spacing: 6

        Label {
            Layout.fillWidth: true
            text: tab.sioul.text("extensions-intro")
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: tab.theme.muted
        }
        Label {
            Layout.fillWidth: true
            visible: text !== ""
            text: tab.view.reading ? tab.sioul.text("extensions-reading") : ""
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: tab.theme.muted
        }
        Label {
            objectName: "extensionsProblem"
            Layout.fillWidth: true
            visible: text !== ""
            text: tab.view.problem || ""
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: tab.theme.warm
        }
        Label {
            Layout.fillWidth: true
            visible: !tab.view.reading && tab.view.offers.length === 0
            text: tab.sioul.text("extensions-none")
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: tab.theme.text
        }
        // Several updates offered: all taken at once, those that ask for more after one question.
        Button {
            objectName: "extensionsUpdateAll"
            visible: (tab.view.updates || 0) >= 2
            text: tab.theme.plain(tab.view.updateAll || "")
            onClicked: {
                if ((tab.view.asks || []).length > 0)
                    updatingAll.open()
                else
                    tab.sioul.extensionsUpdateAll()
            }
        }

        Repeater {
            model: tab.view.offers

            delegate: Section {
                id: offer

                required property var modelData

                objectName: "extension:" + offer.modelData.id
                Layout.fillWidth: true
                Layout.topMargin: 14
                theme: tab.theme
                title: tab.theme.plain(offer.modelData.name)
                body.spacing: 8

                Label {
                    Layout.fillWidth: true
                    text: offer.modelData.about
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: tab.theme.text
                }
                // What it brings, the sites it talks to, whether it keeps a sign-in: said before anything is enabled.
                Repeater {
                    model: [offer.modelData.brings, offer.modelData.sites, offer.modelData.signIn]

                    delegate: Label {
                        required property string modelData

                        Layout.fillWidth: true
                        text: modelData
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: tab.theme.muted
                    }
                }
                Label {
                    Layout.fillWidth: true
                    visible: text !== ""
                    text: offer.modelData.running ? tab.sioul.text("extension-running") : offer.modelData.line
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: tab.theme.text
                }
                // The version that runs here, and the one offered when it differs.
                Label {
                    objectName: "extensionVersion:" + offer.modelData.id
                    Layout.fillWidth: true
                    visible: text !== ""
                    text: offer.modelData.versionLine || ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: tab.theme.muted
                }
                Repeater {
                    model: [offer.modelData.unusable, offer.modelData.problem, offer.modelData.waiting]

                    delegate: Label {
                        required property string modelData

                        Layout.fillWidth: true
                        visible: modelData !== ""
                        text: modelData
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: tab.theme.warm
                    }
                }
                // Its last problem's own detail, under its sentence: the extension's words, in
                // English as it writes them, short; the rest of its log behind Details….
                Label {
                    objectName: "extensionDetail:" + offer.modelData.id
                    Layout.fillWidth: true
                    visible: text !== ""
                    text: offer.modelData.detail || ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: tab.theme.muted
                }
                // Each agenda it brought: its name as the extension gave it (plain text, never
                // Sioul's voice), where its lessons go and how it went, and the calendar chosen.
                Repeater {
                    model: offer.modelData.agendas

                    delegate: ColumnLayout {
                        id: agenda

                        required property var modelData
                        // "Choose…", then a new calendar on each account, then the calendars of yours that may be written.
                        readonly property var choices: [{ choice: "", label: tab.sioul.text("extension-calendar-choose") }].concat(tab.view.calendars || [])

                        objectName: "extensionAgenda:" + agenda.modelData.id
                        Layout.fillWidth: true
                        Layout.topMargin: 4
                        spacing: 2

                        Label {
                            Layout.fillWidth: true
                            text: agenda.modelData.name
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            font.bold: true
                            color: tab.theme.text
                        }
                        Label {
                            objectName: "extensionAgendaLine:" + agenda.modelData.id
                            Layout.fillWidth: true
                            text: agenda.modelData.line
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: agenda.modelData.warn ? tab.theme.warm : tab.theme.muted
                        }
                        // Whose the calendar chosen is, in words: said in Agenda ⚙ ▸ Your agendas
                        // (or in the form that made it), never chosen here.
                        Label {
                            objectName: "extensionAgendaWhose:" + agenda.modelData.id
                            Layout.fillWidth: true
                            visible: text !== ""
                            text: agenda.modelData.whose || ""
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: tab.theme.muted
                        }
                        // Every calendar of yours that holds events, each saying whose it is (as the
                        // agendas' table says it); one the extension cannot write (its server's word,
                        // a subscription, Sioul's busy times, not on this device) listed last, its
                        // reason under it, not to be chosen (26 November 2026).
                        PlainComboBox {
                            id: calendarChoice

                            readonly property int chosenAt: Math.max(0, agenda.choices.findIndex(c => c.choice === "place:" + agenda.modelData.place))

                            objectName: "extensionCalendar:" + agenda.modelData.id
                            Layout.fillWidth: true
                            Layout.maximumWidth: 460
                            textRole: "label"
                            model: agenda.choices.map(c => Object.assign({}, c, { label: tab.theme.plain(c.label) }))
                            currentIndex: calendarChoice.chosenAt
                            Accessible.name: tab.sioul.textWith("extension-calendar-for", "name", agenda.modelData.name)
                            onActivated: index => {
                                // One that cannot be chosen chooses nothing: the box goes back to the one chosen.
                                if (agenda.choices[index].closed) {
                                    calendarChoice.currentIndex = Qt.binding(() => calendarChoice.chosenAt)
                                    return
                                }
                                tab.choose(offer.modelData.id, agenda.modelData, agenda.choices[index].choice)
                            }

                            delegate: ItemDelegate {
                                id: calendarLine

                                required property var modelData
                                required property int index

                                objectName: "extensionCalendarLine:" + agenda.modelData.id + ":" + calendarLine.index
                                width: ListView.view.width
                                enabled: !calendarLine.modelData.closed
                                highlighted: calendarChoice.highlightedIndex === calendarLine.index
                                Accessible.name: [calendarLine.modelData.label, calendarLine.modelData.whose, calendarLine.modelData.why].filter(t => t).join(", ")

                                contentItem: ColumnLayout {
                                    spacing: 0

                                    Label {
                                        Layout.fillWidth: true
                                        text: calendarLine.modelData.label
                                        textFormat: Text.PlainText
                                        elide: Text.ElideRight
                                        font.weight: calendarChoice.currentIndex === calendarLine.index ? Font.DemiBold : Font.Normal
                                        color: calendarLine.enabled ? tab.theme.text : tab.theme.muted
                                    }
                                    Label {
                                        objectName: "extensionCalendarWhose:" + calendarLine.index
                                        Layout.fillWidth: true
                                        visible: text !== ""
                                        text: calendarLine.modelData.whose || ""
                                        textFormat: Text.PlainText
                                        elide: Text.ElideRight
                                        font.pixelSize: 12
                                        color: tab.theme.muted
                                    }
                                    Label {
                                        objectName: "extensionCalendarWhy:" + calendarLine.index
                                        Layout.fillWidth: true
                                        visible: text !== ""
                                        text: calendarLine.modelData.why || ""
                                        textFormat: Text.PlainText
                                        wrapMode: Text.Wrap
                                        font.pixelSize: 12
                                        color: tab.theme.muted
                                    }
                                }
                            }
                        }
                        // Your own word on that agenda (read only, its availability only) does not
                        // keep the extension out: said here, with where it is changed.
                        Label {
                            objectName: "extensionCalendarKept:" + agenda.modelData.id
                            Layout.fillWidth: true
                            Layout.maximumWidth: 460
                            visible: text !== ""
                            text: agenda.modelData.kept || ""
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: tab.theme.muted
                        }
                        // A new calendar for it: Agenda ⚙'s form, its account and its name proposed, events only.
                        Loader {
                            Layout.fillWidth: true
                            Layout.maximumWidth: 460
                            active: tab.making === offer.modelData.id + "/" + agenda.modelData.id
                            visible: active

                            sourceComponent: NewAgendaForm {
                                id: newCalendar

                                objectName: "extensionNewCalendar:" + agenda.modelData.id
                                sioul: tab.sioul
                                theme: tab.theme
                                accounts: tab.setup.accounts || []
                                persons: tab.setup.persons || ({})
                                eventsOnly: true
                                Component.onCompleted: newCalendar.begin(tab.makingOn, agenda.modelData.name)
                                onMade: place => {
                                    tab.making = ""
                                    tab.sioul.extensionCalendar(offer.modelData.id, agenda.modelData.id, "place:" + place)
                                }
                                onKept: {
                                    tab.making = ""
                                    calendarChoice.currentIndex = Qt.binding(() => Math.max(0, agenda.choices.findIndex(c => c.choice === "place:" + agenda.modelData.place)))
                                }
                            }
                        }
                    }
                }
                // Each set of contacts it brought: its name as the extension gave it (plain text,
                // never Sioul's voice), where its contacts go and how it went, and the address book chosen.
                Repeater {
                    model: offer.modelData.sets || []

                    delegate: ColumnLayout {
                        id: contactSet

                        required property var modelData
                        // "Choose…", then a new address book on each account, then the address books of yours that may be written.
                        readonly property var choices: [{ choice: "", label: tab.sioul.text("extension-book-choose") }].concat(tab.view.books || [])

                        objectName: "extensionSet:" + contactSet.modelData.id
                        Layout.fillWidth: true
                        Layout.topMargin: 4
                        spacing: 2

                        Label {
                            Layout.fillWidth: true
                            text: contactSet.modelData.name
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            font.bold: true
                            color: tab.theme.text
                        }
                        Label {
                            objectName: "extensionSetLine:" + contactSet.modelData.id
                            Layout.fillWidth: true
                            text: contactSet.modelData.line
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: contactSet.modelData.warn ? tab.theme.warm : tab.theme.muted
                        }
                        ComboBox {
                            objectName: "extensionBook:" + contactSet.modelData.id
                            Layout.fillWidth: true
                            Layout.maximumWidth: 460
                            model: contactSet.choices.map(c => tab.theme.plain(c.label))
                            currentIndex: Math.max(0, contactSet.choices.findIndex(c => c.choice === "place:" + contactSet.modelData.place))
                            Accessible.name: tab.sioul.textWith("extension-book-for", "name", contactSet.modelData.name)
                            onActivated: index => tab.chooseBook(offer.modelData.id, contactSet.modelData, contactSet.choices[index].choice)
                        }
                    }
                }

                Flow {
                    Layout.fillWidth: true
                    Layout.topMargin: 4
                    spacing: 8

                    Button {
                        objectName: "extensionEnable:" + offer.modelData.id
                        visible: offer.modelData.usable && !offer.modelData.enabled
                        text: tab.sioul.text("extension-enable")
                        onClicked: tab.sioul.extensionEnable(offer.modelData.id)
                    }
                    Button {
                        objectName: "extensionSignIn:" + offer.modelData.id
                        visible: offer.modelData.enabled && offer.modelData.needsSignIn
                        enabled: !offer.modelData.running
                        highlighted: !offer.modelData.signedIn
                        text: tab.sioul.text(offer.modelData.signedIn ? "extension-sign-in-again" : "extension-sign-in")
                        onClicked: tab.sioul.extensionSignIn(offer.modelData.id, "")
                    }
                    Button {
                        objectName: "extensionAccept:" + offer.modelData.id
                        visible: offer.modelData.waiting !== ""
                        text: tab.sioul.text("extension-accept")
                        onClicked: tab.sioul.extensionAccept(offer.modelData.id)
                    }
                    // A newer version that asks for no more than the one in use.
                    Button {
                        objectName: "extensionUpdate:" + offer.modelData.id
                        visible: (offer.modelData.update || "") !== ""
                        highlighted: true
                        text: tab.theme.plain(offer.modelData.updateLabel || "")
                        onClicked: tab.sioul.extensionUpdate(offer.modelData.id)
                    }
                    Button {
                        objectName: "extensionDisable:" + offer.modelData.id
                        visible: offer.modelData.enabled
                        text: tab.sioul.text("extension-disable")
                        onClicked: tab.sioul.extensionDisable(offer.modelData.id)
                    }
                    Button {
                        objectName: "extensionRemove:" + offer.modelData.id
                        visible: offer.modelData.installed
                        text: tab.sioul.text("extension-remove")
                        onClicked: removing.ask(offer.modelData)
                    }
                    // The last lines of its log on this device, to read and to copy: on every device.
                    Button {
                        objectName: "extensionDetails:" + offer.modelData.id
                        visible: offer.modelData.installed
                        text: tab.sioul.text("extension-details")
                        onClicked: logCard.show(offer.modelData.id)
                    }
                }
            }
        }

        // The list in use: its number, when it was read, its signature checked; where it is read
        // from; and a reading now, whatever the daily rhythm.
        Label {
            objectName: "extensionsList"
            Layout.fillWidth: true
            Layout.topMargin: 14
            visible: text !== ""
            text: tab.view.list || ""
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: 12
            color: tab.theme.muted
        }
        Label {
            Layout.fillWidth: true
            visible: text !== ""
            text: tab.view.where || ""
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: 12
            color: tab.theme.muted
        }
        Button {
            objectName: "extensionsReadList"
            enabled: !tab.view.reading
            text: tab.sioul.text("extensions-read-list")
            onClicked: tab.sioul.extensionsReadList()
        }

    }

    Connections {
        target: tab.sioul

        function onExtensionChoices(id, choices) {
            choosing.ask(id, JSON.parse(choices))
        }

        // A person made from the form ("New person…"): the persons each role may name read again.
        function onPersonsChanged() {
            if (tab.making !== "")
                tab.setup = JSON.parse(tab.sioul.agendasSetup() || "{}")
        }
    }

    // Remove: its sign-in forgotten, its file taken out; its lessons left in your calendars, or taken out;
    // its contacts always left in your address books.
    SafeDialog {
        id: removing

        property string id: ""
        property bool contacts: false
        // Whether it brings agendas: only then is there a question of its lessons.
        property bool lessons: true

        function ask(offer) {
            removing.id = offer.id
            removing.title = tab.theme.plain(tab.sioul.textWith("extension-remove-title", "name", offer.name))
            removing.contacts = (offer.sets || []).length > 0
            removing.lessons = offer.bringsAgenda !== false
            keep.checked = true
            removing.open()
        }

        objectName: "extensionRemoving"
        parent: Overlay.overlay
        anchors.centerIn: parent
        modal: true
        width: Math.min(460, removing.roomWidth - 2 * tab.theme.gap)

        contentItem: ColumnLayout {
            spacing: 10

            Label {
                Layout.fillWidth: true
                text: tab.sioul.text(removing.lessons ? "extension-remove-says" : "extension-remove-says-alone")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: tab.theme.text
            }
            CheckBox {
                id: keep

                objectName: "extensionKeepAgendas"
                Layout.fillWidth: true
                visible: removing.lessons
                checked: true
                text: tab.sioul.text("extension-leave-lessons")
            }
            Label {
                objectName: "extensionContactsStay"
                Layout.fillWidth: true
                visible: removing.contacts
                text: tab.sioul.text("extension-remove-contacts-stay")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: tab.theme.muted
            }
        }

        footer: DialogButtonBox {
            Button {
                text: tab.sioul.text("extension-remove-now")
                DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
            }
            Button {
                text: tab.sioul.text("ui-cancel")
                highlighted: true
                DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
            }
        }

        onAccepted: tab.sioul.extensionRemove(removing.id, keep.checked)
    }

    // A choice the extension asks before its sign-in (a platform), the names searchable.
    SafeDialog {
        id: choosing

        property string id: ""
        property var choices: []
        readonly property var shown: {
            const typed = filter.text.trim().toLowerCase()
            return typed === "" ? choosing.choices : choosing.choices.filter(c => c.name.toLowerCase().indexOf(typed) >= 0)
        }

        function ask(id, choices) {
            choosing.id = id
            choosing.choices = choices
            filter.text = ""
            choosing.open()
        }

        objectName: "extensionChoosing"
        title: tab.sioul.text("extension-choose-title")
        parent: Overlay.overlay
        anchors.centerIn: parent
        modal: true
        width: Math.min(460, choosing.roomWidth - 2 * tab.theme.gap)
        height: Math.min(560, choosing.roomHeight - 2 * tab.theme.gap)

        contentItem: ColumnLayout {
            spacing: 8

            TextField {
                id: filter

                objectName: "extensionChooseFilter"
                Layout.fillWidth: true
                placeholderText: tab.sioul.text("extension-choose-filter")
            }
            ListView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                model: choosing.shown

                delegate: BareButton {
                    id: choice

                    required property var modelData

                    objectName: "extensionChoice:" + choice.modelData.id
                    width: ListView.view.width
                    text: tab.theme.plain(choice.modelData.name)
                    onClicked: {
                        choosing.close()
                        tab.sioul.extensionSignIn(choosing.id, choice.modelData.id)
                    }
                }
            }
        }

        footer: DialogButtonBox {
            Button {
                text: tab.sioul.text("ui-cancel")
                DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
            }
        }
    }

    // A credentials sign-in form an extension asked for, framed by Sioul and
    // shown on every device (ExtensionSignIn.qml): opened when the backend
    // asks for one (`extensionSignInForm`), cleared when it is answered, given
    // up, or the extension removed.
    ExtensionSignIn {
        id: signingIn

        sioul: tab.sioul
        theme: tab.theme
    }
    // Update all, when some of the updates ask for more than the versions in use: what each
    // asks, said again, and one yes for all.
    SafeDialog {
        id: updatingAll

        objectName: "extensionsUpdatingAll"
        title: tab.sioul.text("extensions-update-all-title")
        parent: Overlay.overlay
        anchors.centerIn: parent
        modal: true
        width: Math.min(460, updatingAll.roomWidth - 2 * tab.theme.gap)

        contentItem: ColumnLayout {
            spacing: 8

            Label {
                Layout.fillWidth: true
                text: tab.sioul.text("extensions-update-all-asks")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: tab.theme.text
            }
            Repeater {
                model: tab.view.asks || []

                delegate: Label {
                    required property string modelData

                    Layout.fillWidth: true
                    text: modelData
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: tab.theme.warm
                }
            }
        }

        footer: DialogButtonBox {
            Button {
                objectName: "extensionsUpdateAllNow"
                text: tab.sioul.text("extensions-update-all-now")
                DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
            }
            Button {
                text: tab.sioul.text("ui-cancel")
                highlighted: true
                DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
            }
        }

        onAccepted: tab.sioul.extensionsUpdateAll()
    }
    // An extension's Details…: the last lines of its log on this device (ExtensionLog.qml).
    ExtensionLog {
        id: logCard

        sioul: tab.sioul
        theme: tab.theme
    }
    Connections {
        target: tab.sioul

        function onExtensionSignInFormChanged() {
            if (tab.sioul.extensionSignInForm === "")
                signingIn.hide()
            else
                signingIn.show(JSON.parse(tab.sioul.extensionSignInForm))
        }
    }
}
