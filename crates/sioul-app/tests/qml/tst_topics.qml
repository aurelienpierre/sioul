// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Mail's topics in the window (docs/topics.md), on a stand-in for Sioul:
// "Topic…" (TopicMenu.qml) lists the topics with the message's ticked, gives
// one, takes it off, makes a new one and gives it in one act, and for one
// message offers its sender's rules; the filter (TopicFilter.qml) says which
// topic shows and picks another; Mail ▸ ⚙ ▸ Topics (TopicsSettings.qml)
// lists them with their seeds and rules and names a group Sioul noticed; at
// a phone's 412 points nothing passes the window's edge. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 412
    height: 732

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        // What the window asked of Rust, in order.
        property var asked: []
        property string topicGroups: ""
        // Said in the status line with Undo; the settings read the topics again when it changes.
        property string undoLine: ""
        // Mail's topics changed (`changes`): the views read them again.
        signal topicsChanged()
        readonly property var words: ({ "topic-none": "Aucun thème", "topic-new": "Nouveau thème…", "topic-make": "Le créer", "topic-filter-all": "Tous les thèmes", "topics-name-it": "Le nommer", "topics-rename": "Renommer", "topics-remove": "Retirer", "topics-add": "Ajouter un thème" })

        function text(key) {
            return mock.words[key] || key
        }
        function textWith(key, name, value) {
            return key + ":" + value
        }
        function textCounted(key, n) {
            return key + ":" + n
        }
        function topics() {
            return JSON.stringify([
                { id: "choir", name: "Chorale de la vallée et ses répétitions du jeudi soir", seeds: "D’après le dossier Chorale.", senders: [["chef@chorale.example", "always"]] },
                { id: "tax", name: "Impôts", seeds: "", senders: [] }
            ])
        }
        function tagMessages(keys, topic) {
            mock.asked = mock.asked.concat([["tag", keys, topic]])
        }
        function newTopic(keys, name) {
            mock.asked = mock.asked.concat([["new", keys, name]])
            return ""
        }
        function senderTopic(key, topic, rule) {
            mock.asked = mock.asked.concat([["sender", key, topic, rule]])
        }
        function renameTopic(topic, name) {
            mock.asked = mock.asked.concat([["rename", topic, name]])
            return ""
        }
        function removeTopic(topic) {
            mock.asked = mock.asked.concat([["remove", topic]])
        }
        function forgetSenderTopic(address, topic) {
            mock.asked = mock.asked.concat([["forget", address, topic]])
        }
        function findTopicGroups() {
            mock.topicGroups = JSON.stringify({ state: "done",
                groups: [{ size: 42, senders: 3, names: ["Opérateur Exemple", "Service client"], subjects: ["Votre facture de novembre"], proposed: "Téléphone et internet", proposed_by: "sites" },
                         { size: 20, senders: 2, names: ["Jeanne Martin", "Paul Durand"], subjects: ["Dimanche"], proposed: "", proposed_by: "" }],
                senders: [{ size: 30, senders: 1, names: ["Jeanne Martin"], subjects: ["Programme"], proposed: "", proposed_by: "" }] })
        }
        function topicPresets() {
            return JSON.stringify([{ id: "bank", name: "Banque" }, { id: "phone-internet", name: "Téléphone et internet" }])
        }
        function startTopicPreset(id) {
            mock.asked = mock.asked.concat([["preset", id]])
            return ""
        }
        function nameTopicSender(index, name) {
            mock.asked = mock.asked.concat([["always", index, name]])
            return ""
        }
        function nameTopicGroup(index, name) {
            mock.asked = mock.asked.concat([["name", index, name]])
            return ""
        }
    }

    Button {
        id: anchor

        text: "Thème…"
        y: 20
    }

    Component {
        id: menuComponent

        TopicMenu {
            sioul: mock
            theme: testTheme
        }
    }

    Component {
        id: filterComponent

        TopicFilter {
            sioul: mock
            theme: testTheme
            y: 60
        }
    }

    Component {
        id: settingsComponent

        TopicsSettings {
            sioul: mock
            theme: testTheme
            width: 412
        }
    }

    Component {
        id: spyComponent

        SignalSpy {}
    }

    TestCase {
        name: "Topics"
        when: windowShown

        function init() {
            mock.asked = []
            mock.topicGroups = ""
        }

        function all(item, test, into) {
            const found = into || []
            if (!item)
                return found
            if (test(item))
                found.push(item)
            for (let i = 0; i < (item.children || []).length; ++i)
                all(item.children[i], test, found)
            return found
        }

        // Opened and laid out: a layout's or a Flow's children all sit at 0, 0 until polished.
        function shown(menu) {
            tryCompare(menu, "opened", true)
            waitForItemPolished(menu.contentItem)
        }

        function named(item, name) {
            return all(item, i => i.objectName === name)[0] || all(root.Overlay.overlay, i => i.objectName === name)[0] || null
        }

        // Within the window's 412 points, its edges included.
        function inside(item) {
            const at = item.mapToItem(root, 0, 0)
            verify(at.x >= 0 && at.x + item.width <= root.width + 0.5, item.objectName + " passes the edge: " + at.x + " + " + item.width)
        }

        // One message: its topic ticked; another given at a tap; its sender's rules for its topic.
        function test_a_message_is_given_a_topic() {
            const menu = createTemporaryObject(menuComponent, root)
            menu.openAt(anchor, "[[\"/m/1\"]]", "/m/1", "choir")
            shown(menu)
            const choir = named(menu.contentItem, "topicChoice:choir")
            verify(choir.checked)
            inside(menu.contentItem)
            verify(named(menu.contentItem, "topicAlways").visible && named(menu.contentItem, "topicNever").visible)
            mouseClick(named(menu.contentItem, "topicChoice:tax"))
            compare(mock.asked[0], ["tag", "[[\"/m/1\"]]", "tax"])
            tryCompare(menu, "opened", false)
            menu.openAt(anchor, "[[\"/m/1\"]]", "/m/1", "choir")
            shown(menu)
            mouseClick(named(menu.contentItem, "topicAlways"))
            compare(mock.asked[1], ["sender", "/m/1", "choir", "always"])
            tryCompare(menu, "opened", false)
            menu.openAt(anchor, "[[\"/m/1\"]]", "/m/1", "choir")
            shown(menu)
            mouseClick(named(menu.contentItem, "topicNone"))
            compare(mock.asked[2], ["tag", "[[\"/m/1\"]]", ""])
        }

        // Several messages: no rule for a sender; "New topic…" makes it and gives it in one act.
        function test_several_messages_and_a_new_topic() {
            const menu = createTemporaryObject(menuComponent, root)
            menu.openAt(null, "[\"/m/1\",\"/m/2\"]", "", "")
            shown(menu)
            verify(!named(menu.contentItem, "topicAlways").visible)
            mouseClick(named(menu.contentItem, "topicNew"))
            const field = named(menu.contentItem, "topicNewName")
            verify(field.visible)
            waitForItemPolished(menu.contentItem)
            field.text = "Club de lecture"
            mouseClick(named(menu.contentItem, "topicMake"))
            compare(mock.asked[0], ["new", "[\"/m/1\",\"/m/2\"]", "Club de lecture"])
            tryCompare(menu, "opened", false)
        }

        // The filter says which topic shows, unfolds them all, and picks one.
        function test_the_filter_picks_a_topic() {
            const filter = createTemporaryObject(filterComponent, root)
            const button = named(filter, "topicFilter")
            verify(button.text.indexOf("Tous les thèmes") === 0)
            const picked = createTemporaryObject(spyComponent, root, { target: filter, signalName: "picked" })
            filter.unfold()
            const tax = named(filter, "topicFilter:tax")
            tryVerify(() => tax.visible)
            mouseClick(tax)
            compare(picked.count, 1)
            compare(picked.signalArguments[0][0], "tax")
            filter.chosen = "tax"
            verify(button.text.indexOf("Impôts") === 0)
            inside(button)
        }

        // The settings list the topics, their seeds and rules; Remove and ×; a group named.
        function test_the_settings_and_a_group_named() {
            const page = createTemporaryObject(settingsComponent, root)
            waitForItemPolished(page)
            // The offered presets side by side once the Flow has placed them.
            tryVerify(() => named(page, "topicPreset:phone-internet").x > named(page, "topicPreset:bank").x || named(page, "topicPreset:phone-internet").y > named(page, "topicPreset:bank").y)
            verify(named(page, "topicRow:choir") !== null && named(page, "topicRow:tax") !== null)
            mouseClick(named(page, "topicRemove:tax"))
            compare(mock.asked[0], ["remove", "tax"])
            // A topic Sioul knows, started in one tap.
            mouseClick(named(page, "topicPreset:bank"))
            compare(mock.asked[1], ["preset", "bank"])
            mock.asked = mock.asked.slice(0, 1)
            mouseClick(named(page, "topicGroupsLook"))
            tryVerify(() => named(page, "topicGroupName:0") !== null && named(page, "topicGroupName:0").visible)
            waitForItemPolished(page)
            mouseClick(named(page, "topicGroupName:0"))
            compare(named(page, "topicGroupField:0").text, "Téléphone et internet", "the name proposed")
            named(page, "topicGroupField:0").text = "Téléphone"
            waitForItemPolished(page)
            mouseClick(named(page, "topicGroupMake:0"))
            compare(mock.asked[1], ["name", 0, "Téléphone"])
            // A group of persons: no name proposed, and the page says so.
            mouseClick(named(page, "topicGroupName:1"))
            compare(named(page, "topicGroupField:1").text, "")
            compare(named(page, "topicGroupHow:1").text, "topics-named-none")
            // One sender with much mail, a person: "Always a topic…", no name proposed, the one you write given in one act.
            waitForItemPolished(page)
            mouseClick(named(page, "topicSenderAlways:0"))
            compare(named(page, "topicSenderField:0").text, "")
            named(page, "topicSenderField:0").text = "Chorale"
            waitForItemPolished(page)
            mouseClick(named(page, "topicSenderMake:0"))
            compare(mock.asked[2], ["always", 0, "Chorale"])
            // A small mailbox: no group yet, said so.
            mock.topicGroups = JSON.stringify({ state: "done", waiting: true, groups: [], senders: [] })
            tryCompare(named(page, "topicGroupsNone"), "text", "topics-groups-waiting")
            for (const row of all(page, i => i.objectName.indexOf("topicRow:") === 0))
                inside(row)
        }
    }
}
