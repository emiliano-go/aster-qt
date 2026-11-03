import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window
import QtWebEngine
import AsterQt 1.0

ApplicationWindow {
    id: root
    visible: true
    width: 1440
    height: 900
    minimumWidth: 980
    minimumHeight: 620
    title: "Aster"
    color: c("bg", "#121417")

    AppController { id: app }

    readonly property var th: {
        try { return JSON.parse(app.tokensJson || "{}") } catch (e) { return {} }
    }
    function c(key, fallback) { return th[key] !== undefined ? th[key] : fallback }
    function n(key, fallback) { return th[key] !== undefined ? th[key] : fallback }

    function normId(value) {
        return (value || "").trim().replace(/^</, "").replace(/>$/, "");
    }

    function subjectKey(value) {
        return (value || "").replace(/^((re|fwd|fw)\s*:\s*)+/i, "").trim().toLowerCase();
    }

    function threadKey(message) {
        if (!message) return "";
        var byId = {};
        for (var i = 0; i < root.messages.length; i++) {
            var candidate = root.messages[i];
            if (candidate.message_id) byId[normId(candidate.message_id)] = candidate;
        }
        var current = message;
        var depth = 0;
        while (current && depth < 24) {
            var parentId = normId(current.in_reply_to);
            if (!parentId && current.references) {
                var refs = current.references.split(/\s+/).filter(function(v) { return v.length > 0; });
                if (refs.length > 0) parentId = normId(refs[refs.length - 1]);
            }
            var parent = parentId ? byId[parentId] : null;
            if (!parent || parent === current) break;
            current = parent;
            depth += 1;
        }
        return (current && current.message_id) ? normId(current.message_id) : (subjectKey(message.subject) || message.id);
    }

    readonly property var threadSiblings: {
        if (!root.detail || !root.detail.id) return [];
        var key = threadKey(root.detail);
        var out = [];
        for (var i = 0; i < root.messages.length; i++) {
            if (threadKey(root.messages[i]) === key) out.push(root.messages[i]);
        }
        return out;
    }

    function parse(text, fallback) {
        try { return JSON.parse(text || "") } catch (e) { return fallback }
    }

    readonly property var mailboxes: parse(app.mailboxesJson, [])
    readonly property var messages: parse(app.messagesJson, [])
    readonly property var detail: parse(app.selectedJson, ({}))

    readonly property var systemOrder: ["inbox", "sent", "drafts", "archive", "spam", "trash"]
    readonly property var systemFolders: {
        var out = [];
        for (var i = 0; i < root.systemOrder.length; i++) {
            for (var j = 0; j < root.mailboxes.length; j++) {
                if (root.mailboxes[j].folder_label === root.systemOrder[i]) {
                    out.push(root.mailboxes[j]);
                    break;
                }
            }
        }
        return out;
    }
    readonly property var customFolders: {
        var out = [];
        for (var i = 0; i < root.mailboxes.length; i++) {
            if (root.systemOrder.indexOf(root.mailboxes[i].folder_label) === -1)
                out.push(root.mailboxes[i]);
        }
        return out;
    }
    readonly property int unreadTotal: {
        var total = 0;
        for (var i = 0; i < root.mailboxes.length; i++) {
            var f = root.mailboxes[i].folder_label;
            if (f !== "sent" && f !== "drafts") total += root.mailboxes[i].unread;
        }
        return total;
    }

    property string query: ""
    property bool unreadOnly: false
    property bool composeOpen: false
    property bool settingsOpen: false
    property string settingsTab: "appearance"
    property bool readerOpen: true
    property bool toastVisible: false
    property bool deleteConfirmOpen: false
    property string pendingDeleteToken: ""
    property string pendingDeleteIds: "[]"
    property string deleteConfirmWord: ""
    property bool narrow: width < 1160
    property bool sidebarVisible: width >= 860
    readonly property bool systemDark: Application.styleHints.colorScheme === Qt.Dark
    readonly property bool dark: !app.themeLight
    readonly property var settings: parse(app.settingsJson, {})
    readonly property var rulesData: parse(app.rulesJson, ({}))
    readonly property var rules: rulesData.rules || []
    readonly property var identities: parse(app.identitiesJson, [])
    readonly property var pgp: parse(app.pgpJson, ({}))
    readonly property var contacts: parse(app.contactsJson, [])
    readonly property var threadMessages: parse(app.threadJson, [])
    readonly property var mailingLists: parse(app.listsJson, [])
    property string listFilter: ""
    readonly property var themes: parse(app.themesJson, [])
    readonly property var lightThemes: themes.filter(function(t) { return t.light === true; })
    readonly property var darkThemes: themes.filter(function(t) { return t.light !== true; })
    property int currentIndex: 0
    property bool ruleMatchAll: true

    ListModel {
        id: conditionModel
        ListElement { field: "from"; op: "contains"; value: "" }
    }
    ListModel {
        id: actionModel
        ListElement { kind: "move_to"; target: "" }
    }

    function createVisualRule() {
        var conditions = [];
        for (var i = 0; i < conditionModel.count; i++) {
            var c = conditionModel.get(i);
            if (!c.field) continue;
            conditions.push({ field: c.field, op: c.op || "contains", value: c.value || "" });
        }
        var actions = [];
        for (var j = 0; j < actionModel.count; j++) {
            var a = actionModel.get(j);
            var kind = (a.kind || "").trim();
            var target = (a.target || "").trim();
            if (kind === "move_to") actions.push({ type: "move_to", folder_token: target });
            else if (kind === "apply_labels") actions.push({ type: "apply_labels", label_tokens: target.split(",").map(function(s) { return s.trim(); }).filter(function(s) { return s.length > 0; }) });
            else if (kind === "forward") actions.push({ type: "forward", to: target });
            else if (kind === "mark_as") actions.push({ type: "mark_as", value: target || "read" });
            else if (kind === "star") actions.push({ type: "star" });
            else if (kind === "skip_inbox") actions.push({ type: "skip_inbox" });
            else if (kind === "delete") actions.push({ type: "delete" });
            else if (kind === "auto_reply") actions.push({ type: "auto_reply", template_id: target });
            else if (kind === "pin") actions.push({ type: "pin" });
            else if (kind === "snooze") actions.push({ type: "snooze", until_iso8601: target });
            else if (kind === "categorize") actions.push({ type: "categorize", category: target });
            else if (kind === "notify") actions.push({ type: "notify", enabled: target !== "false" });
        }
        app.createRuleJson(JSON.stringify({
            name: ruleName.text || "Rule",
            color: "#d98e5c",
            enabled: true,
            match_mode: root.ruleMatchAll ? "all" : "any",
            conditions: conditions,
            actions: actions
        }));
    }

    onSystemDarkChanged: app.applySystemDark(systemDark)

    readonly property var visibleMessages: {
        var out = [];
        var q = root.query.trim().toLowerCase();
        for (var i = 0; i < root.messages.length; i++) {
            var m = root.messages[i];
            if (root.listFilter.length > 0 && m.list_id !== root.listFilter) continue;
            if (root.unreadOnly && m.is_read) continue;
            if (q.length > 0) {
                var hay = (m.sender + " " + m.subject + " " + m.preview).toLowerCase();
                if (hay.indexOf(q) === -1) continue;
            }
            out.push(m);
        }
        return out;
    }

    readonly property var groupedMessages: {
        var out = [];
        var seen = {};
        for (var i = 0; i < root.visibleMessages.length; i++) {
            var message = root.visibleMessages[i];
            var key = root.threadKey(message);
            if (seen[key] !== undefined) {
                out[seen[key]].thread_count += 1;
                continue;
            }
            var copy = Object.assign({}, message);
            copy.thread_count = 1;
            seen[key] = out.length;
            out.push(copy);
        }
        return out;
    }

    function folderLabel(label) {
        var names = { inbox: "Inbox", sent: "Sent", drafts: "Drafts", archive: "Archive", spam: "Junk", trash: "Trash" };
        return names[label] || label;
    }

    function folderIcon(label) {
        var icons = { inbox: "inbox", sent: "send", drafts: "drafts", archive: "archive", spam: "junk", trash: "trash" };
        return icons[label] || "folder";
    }

    function formatWhen(iso) {
        if (!iso) return "";
        var d = new Date(iso);
        if (isNaN(d.getTime())) return "";
        var now = new Date();
        if (d.toDateString() === now.toDateString()) return Qt.formatDateTime(d, "HH:mm");
        if (d.getFullYear() === now.getFullYear()) return Qt.formatDateTime(d, "MMM d");
        return Qt.formatDateTime(d, "MMM d, yyyy");
    }

    function formatBytes(bytes) {
        if (!bytes) return "";
        if (bytes < 1024) return bytes + " B";
        if (bytes < 1048576) return (bytes / 1024).toFixed(0) + " KB";
        return (bytes / 1048576).toFixed(1) + " MB";
    }

    function selectAt(index) {
        if (index < 0 || index >= root.groupedMessages.length) return;
        root.currentIndex = index;
        app.selectMessage(root.groupedMessages[index].id);
    }

    function openSettings(tab) {
        root.settingsTab = tab || "appearance";
        root.settingsOpen = true;
        app.loadSettingsData();
        app.refreshThemes();
    }

    function move(delta) {
        var next = root.currentIndex + delta;
        if (next < 0) next = 0;
        if (next >= root.groupedMessages.length) next = root.groupedMessages.length - 1;
        root.selectAt(next);
    }

    Shortcut { sequence: "j"; onActivated: root.move(1) }
    Shortcut { sequence: "k"; onActivated: root.move(-1) }
    Shortcut { sequences: ["Ctrl+N", "c"]; onActivated: root.composeOpen = true }
    Shortcut {
        sequence: "Escape"
        onActivated: {
            if (root.deleteConfirmOpen) {
                app.resolveDelete(root.pendingDeleteToken, false);
                root.deleteConfirmOpen = false;
            } else {
                root.composeOpen = false;
            }
        }
    }
    Shortcut { sequence: "/"; onActivated: searchInput.forceActiveFocus() }
    Shortcut { sequence: "Ctrl+,"; onActivated: root.openSettings("appearance") }

    Component.onCompleted: {
        app.applySystemDark(root.systemDark);
        app.boot();
    }


    Timer {
        interval: 8000
        running: app.ready
        repeat: true
        onTriggered: app.refreshMail()
    }


    component ModeChip: Rectangle {
        property string label
        property bool active: false
        signal clicked()
        implicitWidth: chipText.implicitWidth + n("space-3", 12) * 2
        implicitHeight: 28
        radius: n("radius-control", 8)
        color: active ? c("accent-button", "#d98e5c") : (chipHover.hovered ? c("hover", "#2e2e323a") : c("input", "#221b1e23"))
        border.width: active ? 0 : n("border-width", 1)
        border.color: c("border", "#171b1e23")
        HoverHandler { id: chipHover }
        Text {
            id: chipText
            anchors.centerIn: parent
            text: parent.label
            color: parent.active ? c("accent-ink", "#171717") : c("text-2", "#dbebe7de")
            font.family: c("font-family", "Inter")
            font.pixelSize: n("font-size-small", 12)
            font.weight: parent.active ? n("font-weight-semibold", 600) : n("font-weight-medium", 500)
        }
        MouseArea {
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onClicked: parent.clicked()
        }
    }

    component ThemeRow: Rectangle {
        id: themeRow
        property var entry
        property bool active: false
        implicitHeight: 30
        radius: n("radius-control", 8)
        color: active ? c("selected", "#3d2e323a") : (themeHover.hovered ? c("hover", "#2e2e323a") : "transparent")
        HoverHandler { id: themeHover }
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: n("space-2", 8)
            anchors.rightMargin: n("space-2", 8)
            spacing: n("space-2", 8)
            Icon {
                name: "check"
                size: n("icon-small", 14)
                color: c("accent-text", "#e7a97f")
                visible: themeRow.active
            }
            Text {
                text: themeRow.entry && themeRow.entry.name ? themeRow.entry.name : ""
                color: c("text", "#ece7de")
                font.family: c("font-family", "Inter")
                font.pixelSize: n("font-size-small", 12)
                font.weight: themeRow.active ? n("font-weight-semibold", 600) : n("font-weight-regular", 400)
                Layout.fillWidth: true
            }
            Text {
                text: themeRow.entry && themeRow.entry.builtin ? "built in" : "css"
                color: c("muted", "#a8ece7de")
                font.family: c("font-family", "Inter")
                font.pixelSize: n("font-size-micro", 9)
            }
        }
        MouseArea {
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onClicked: {
                if (!themeRow.entry) return;
                app.setThemeSlot(themeRow.entry.id, themeRow.entry.light ? "light" : "dark");
                themePopup.close();
            }
        }
    }


    component Toggle: Rectangle {
        property bool checked: false
        signal toggled()
        implicitWidth: 40
        implicitHeight: 22
        radius: 11
        color: checked ? c("accent-button", "#d98e5c") : c("surface-strong", "#2e323a")
        border.width: checked ? 0 : n("border-width", 1)
        border.color: c("border-strong", "#292e323a")
        Rectangle {
            width: 18
            height: 18
            radius: 9
            y: 2
            x: parent.checked ? parent.width - 20 : 2
            color: parent.checked ? c("accent-ink", "#171717") : c("muted", "#a8ece7de")
            Behavior on x { NumberAnimation { duration: n("motion-fast", 110); easing.type: Easing.OutCubic } }
        }
        MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: parent.toggled() }
    }

    component SettingsField: Rectangle {
        id: fieldRoot
        property alias text: field.text
        property string placeholder: ""
        signal edited(string value)
        implicitHeight: n("control-height", 36)
        radius: n("radius-control", 8)
        color: c("input", "#221b1e23")
        border.width: field.activeFocus ? n("border-width-focus", 1.5) : n("border-width", 1)
        border.color: field.activeFocus ? c("accent", "#d98e5c") : c("border", "#171b1e23")
        TextInput {
            id: field
            anchors.fill: parent
            anchors.leftMargin: n("space-3", 12)
            anchors.rightMargin: n("space-3", 12)
            verticalAlignment: TextInput.AlignVCenter
            color: c("text", "#ece7de")
            selectionColor: c("accent", "#d98e5c")
            selectedTextColor: c("accent-ink", "#171717")
            font.family: c("font-family", "Inter")
            font.pixelSize: n("font-size-small", 12)
            clip: true
            onTextChanged: fieldRoot.edited(text)
            Text {
                visible: field.text.length === 0
                text: fieldRoot.placeholder
                color: c("muted", "#a8ece7de")
                font.family: c("font-family", "Inter")
                font.pixelSize: n("font-size-small", 12)
            }
        }
    }

    component NavSettingsRow: Rectangle {
        property string label
        property string tab
        property string icon
        property bool active: root.settingsTab === tab
        implicitHeight: 34
        radius: n("radius-control", 8)
        color: active ? c("selected", "#3d2e323a") : (navHover.hovered ? c("hover", "#2e2e323a") : "transparent")
        HoverHandler { id: navHover }
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: n("space-2", 8)
            anchors.rightMargin: n("space-2", 8)
            spacing: n("space-2", 8)
            Icon { name: parent.parent.icon; size: n("icon-body", 16); color: parent.parent.active ? c("accent-text", "#e7a97f") : c("muted", "#a8ece7de") }
            Text {
                text: parent.parent.label
                color: c("text", "#ece7de")
                font.family: c("font-family", "Inter")
                font.pixelSize: n("font-size-small", 12)
                font.weight: parent.parent.active ? n("font-weight-semibold", 600) : n("font-weight-regular", 400)
                Layout.fillWidth: true
            }
        }
        MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: root.settingsTab = tab }
    }

    component SettingsTitle: Text {
        color: c("text", "#ece7de")
        font.family: c("font-family", "Inter")
        font.pixelSize: n("font-size-page", 18)
        font.weight: n("font-weight-semibold", 600)
        Layout.fillWidth: true
    }

    component SettingsSubtitle: Text {
        color: c("muted", "#a8ece7de")
        font.family: c("font-family", "Inter")
        font.pixelSize: n("font-size-small", 12)
        wrapMode: Text.WordWrap
        Layout.fillWidth: true
    }

    component LinkText: Text {
        property string action: ""
        color: c("accent-text", "#e7a97f")
        font.family: c("font-family", "Inter")
        font.pixelSize: n("font-size-small", 12)
        MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: action() }
    }

    component FolderRow: Rectangle {
        id: folderRow
        property var folder
        property bool active: app.folderId === folder.folder_label
        implicitHeight: 32
        radius: n("radius-control", 8)
        color: active ? c("selected", "#3d2e323a") : (folderHover.hovered ? c("hover", "#2e2e323a") : "transparent")
        Behavior on color { ColorAnimation { duration: n("motion-fast", 110) } }
        HoverHandler { id: folderHover }
        Rectangle {
            width: n("unread-bar-width", 3)
            height: 16
            radius: 2
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            color: c("accent", "#d98e5c")
            visible: folderRow.active
        }
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: n("space-2", 8) + 8
            anchors.rightMargin: n("space-3", 12)
            spacing: n("space-2", 8)
            Icon {
                name: root.folderIcon(folderRow.folder.folder_label)
                size: n("icon-body", 16)
                color: folderRow.active ? c("accent-text", "#e7a97f") : c("muted", "#a8ece7de")
            }
            Text {
                text: root.folderLabel(folderRow.folder.name || folderRow.folder.folder_label)
                color: c("text", "#ece7de")
                font.family: c("font-family", "Inter")
                font.pixelSize: n("font-size-body", 14)
                font.weight: folderRow.active ? n("font-weight-semibold", 600) : n("font-weight-regular", 400)
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
            Text {
                text: folderRow.folder.unread > 0 ? folderRow.folder.unread : ""
                color: folderRow.active ? c("accent-text", "#e7a97f") : c("muted", "#a8ece7de")
                font.family: c("font-family", "Inter")
                font.pixelSize: n("font-size-small", 12)
            }
        }
        MouseArea {
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onClicked: {
                root.currentIndex = 0;
                root.listFilter = "";
                app.openMailbox(folderRow.folder.folder_label)
            }
        }
    }

    component IconButton: Rectangle {
        id: iconButton
        property string icon
        property string label
        property bool accent: false
        property bool danger: false
        signal clicked()
        implicitWidth: n("control-height", 36)
        implicitHeight: n("control-height", 36)
        radius: n("radius-control", 8)
        color: accent ? (iconHover.hovered ? Qt.lighter(c("accent-button", "#d98e5c"), 1.06) : c("accent-button", "#d98e5c"))
                      : (iconHover.hovered ? c("hover", "#2e2e323a") : "transparent")
        Behavior on color { ColorAnimation { duration: n("motion-fast", 110) } }
        activeFocusOnTab: true
        Accessible.role: Accessible.Button
        Accessible.name: label
        HoverHandler { id: iconHover }
        Icon {
            anchors.centerIn: parent
            name: iconButton.icon
            size: n("icon-body", 16)
            color: iconButton.accent ? c("accent-ink", "#171717") : (iconButton.danger ? c("danger", "#d97a72") : c("text-2", "#dbebe7de"))
        }
        border.width: activeFocus ? n("border-width-focus", 1.5) : 0
        border.color: c("accent", "#d98e5c")
        ToolTip.visible: iconHover.hovered
        ToolTip.text: label
        ToolTip.delay: 500
        Keys.onSpacePressed: iconButton.clicked()
        Keys.onReturnPressed: iconButton.clicked()
        MouseArea {
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onClicked: iconButton.clicked()
        }
    }

    component MessageRow: Rectangle {
        id: row
        property var item
        property bool selected: item && root.detail.id === item.id
        implicitHeight: 88
        color: selected ? c("surface-strong", "#2e323a") : (rowHover.hovered ? c("hover", "#2e2e323a") : "transparent")
        Behavior on color { ColorAnimation { duration: n("motion-fast", 110) } }
        HoverHandler { id: rowHover }
        Rectangle {
            width: n("unread-bar-width", 3)
            height: parent.height - 24
            radius: 2
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            color: row.selected ? c("accent", "#d98e5c") : c("unread-dot", "#d98e5c")
            visible: row.selected || (row.item && !row.item.is_read)
        }
        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: 1
            color: c("message-divider", "#343a44")
        }
        ColumnLayout {
            anchors.fill: parent
            anchors.leftMargin: n("space-4", 16)
            anchors.rightMargin: n("space-4", 16)
            anchors.topMargin: n("space-3", 12)
            anchors.bottomMargin: n("space-3", 12)
            spacing: 3
            RowLayout {
                Layout.fillWidth: true
                spacing: n("space-2", 8)
                Text {
                    text: row.item && row.item.sender ? row.item.sender : "Unknown sender"
                    color: c("text", "#ece7de")
                    font.family: c("font-family", "Inter")
                    font.pixelSize: n("font-size-body", 14)
                    font.weight: row.item && !row.item.is_read ? n("font-weight-unread", 600) : n("font-weight-regular", 400)
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
                Text {
                    visible: row.item && row.item.thread_count > 1
                    text: row.item ? row.item.thread_count : ""
                    color: c("accent-text", "#e7a97f")
                    font.family: c("font-family", "Inter")
                    font.pixelSize: n("font-size-micro", 9)
                    font.weight: n("font-weight-semibold", 600)
                }
                Text {
                    text: row.item ? root.formatWhen(row.item.date) : ""
                    color: c("muted", "#a8ece7de")
                    font.family: c("font-family", "Inter")
                    font.pixelSize: n("font-size-caption", 10)
                }
            }
            Text {
                text: row.item && row.item.subject ? row.item.subject : "(no subject)"
                color: c("text", "#ece7de")
                font.family: c("font-family", "Inter")
                font.pixelSize: n("font-size-small", 12)
                font.weight: row.item && !row.item.is_read ? n("font-weight-semibold", 600) : n("font-weight-medium", 500)
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: n("space-2", 8)
                Text {
                    text: row.item ? row.item.preview : ""
                    color: c("muted", "#a8ece7de")
                    font.family: c("font-family", "Inter")
                    font.pixelSize: n("font-size-small", 12)
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
                Text {
                    visible: row.item && (row.item.list_name || row.item.list_id)
                    text: row.item ? (row.item.list_name || row.item.list_id) : ""
                    color: c("muted", "#a8ece7de")
                    font.family: c("font-family", "Inter")
                    font.pixelSize: n("font-size-micro", 9)
                    elide: Text.ElideRight
                    Layout.maximumWidth: 120
                }
                Icon {
                    name: "attach"
                    size: n("icon-small", 14)
                    color: c("muted", "#a8ece7de")
                    visible: row.item && row.item.has_attachments
                }
            }
        }
        MouseArea {
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onClicked: {
                root.currentIndex = index;
                root.readerOpen = true;
                app.selectMessage(row.item.id)
            }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: n("topbar-height", 56)
            color: c("sidebar", "#1a1d22")
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: n("space-4", 16)
                anchors.rightMargin: n("space-4", 16)
                spacing: n("space-3", 12)

                Rectangle {
                    width: 28; height: 28
                    radius: n("radius-control", 8)
                    color: c("accent-button", "#d98e5c")
                    Icon {
                        anchors.centerIn: parent
                        name: "send"
                        size: n("icon-body", 16)
                        color: c("accent-ink", "#171717")
                    }
                }
                Column {
                    spacing: -2
                    Text {
                        text: "Aster"
                        color: c("text", "#ece7de")
                        font.family: c("font-brand", "TeX Gyre Adventor")
                        font.pixelSize: n("font-size-page", 18)
                        font.weight: n("font-weight-semibold", 600)
                    }
                    Text {
                        text: app.accountEmail || "Aster Mail"
                        color: c("muted", "#a8ece7de")
                        font.family: c("font-family", "Inter")
                        font.pixelSize: n("font-size-micro", 9)
                    }
                }

                Rectangle {
                    Layout.preferredWidth: Math.min(360, root.width * 0.3)
                    Layout.preferredHeight: n("control-height", 36)
                    radius: n("radius-control", 8)
                    color: c("input", "#221b1e23")
                    border.width: searchInput.activeFocus ? n("border-width-focus", 1.5) : n("border-width", 1)
                    border.color: searchInput.activeFocus ? c("accent", "#d98e5c") : c("border", "#171b1e23")
                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: n("space-3", 12)
                        anchors.rightMargin: n("space-3", 12)
                        spacing: n("space-2", 8)
                        Icon { name: "search"; size: n("icon-small", 14); color: c("muted", "#a8ece7de") }
                        TextInput {
                            id: searchInput
                            Layout.fillWidth: true
                            color: c("text", "#ece7de")
                            selectionColor: c("accent", "#d98e5c")
                            selectedTextColor: c("accent-ink", "#171717")
                            font.family: c("font-family", "Inter")
                            font.pixelSize: n("font-size-small", 12)
                            verticalAlignment: TextInput.AlignVCenter
                            onTextChanged: root.query = text
                            Text {
                                visible: searchInput.text.length === 0
                                text: "Search mail"
                                color: c("muted", "#a8ece7de")
                                font.family: c("font-family", "Inter")
                                font.pixelSize: n("font-size-small", 12)
                            }
                        }
                        Text {
                            text: "Ctrl K"
                            color: c("muted", "#a8ece7de")
                            font.family: c("font-mono", "JetBrains Mono")
                            font.pixelSize: n("font-size-micro", 9)
                            visible: searchInput.text.length === 0
                        }
                    }
                }

                Item { Layout.fillWidth: true }

                RowLayout {
                    spacing: n("space-2", 8)
                    Rectangle {
                        width: 7; height: 7
                        radius: 4
                        color: app.ready ? c("success", "#7fae83") : c("warning", "#d9b36a")
                    }
                    Text {
                        text: app.ready ? "Synced" : "Syncing"
                        color: c("muted", "#a8ece7de")
                        font.family: c("font-family", "Inter")
                        font.pixelSize: n("font-size-small", 12)
                    }
                    IconButton {
                        icon: "settings"
                        label: "Settings"
                        onClicked: root.openSettings("appearance")
                    }
                    Rectangle {
                        implicitWidth: composeRow.implicitWidth + n("space-4", 16) * 2
                        implicitHeight: n("control-height", 36)
                        radius: n("radius-control", 8)
                        color: composeHover.hovered ? Qt.lighter(c("accent-button", "#d98e5c"), 1.06) : c("accent-button", "#d98e5c")
                        HoverHandler { id: composeHover }
                        Row {
                            id: composeRow
                            anchors.centerIn: parent
                            spacing: n("space-2", 8)
                            Icon { name: "plus"; size: n("icon-small", 14); color: c("accent-ink", "#171717") }
                            Text {
                                text: "Compose"
                                color: c("accent-ink", "#171717")
                                font.family: c("font-family", "Inter")
                                font.pixelSize: n("font-size-small", 12)
                                font.weight: n("font-weight-semibold", 600)
                            }
                        }
                        MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: root.composeOpen = true }
                    }
                }
            }
        }

        SplitView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            orientation: Qt.Horizontal
            handle: Rectangle { implicitWidth: 1; implicitHeight: 1; color: c("border", "#171b1e23") }

            Rectangle {
                SplitView.preferredWidth: 236
                SplitView.minimumWidth: 210
                visible: root.sidebarVisible
                color: c("sidebar", "#1a1d22")
                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: n("space-3", 12)
                    spacing: n("space-1", 4)

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 56
                        radius: n("radius-card", 14)
                        color: c("card", "#23262c")
                        RowLayout {
                            anchors.fill: parent
                            anchors.margins: n("space-3", 12)
                            spacing: n("space-2", 8)
                            Rectangle {
                                width: 30; height: 30
                                radius: 15
                                color: c("accent-button", "#d98e5c")
                                Text {
                                    anchors.centerIn: parent
                                    text: (app.accountEmail || "A").substring(0, 1).toUpperCase()
                                    color: c("accent-ink", "#171717")
                                    font.family: c("font-family", "Inter")
                                    font.pixelSize: n("font-size-body", 14)
                                    font.weight: n("font-weight-semibold", 600)
                                }
                            }
                            ColumnLayout {
                                spacing: 1
                                Layout.fillWidth: true
                                Text {
                                    text: app.accountEmail || "Aster Mail"
                                    color: c("text", "#ece7de")
                                    font.family: c("font-family", "Inter")
                                    font.pixelSize: n("font-size-small", 12)
                                    font.weight: n("font-weight-semibold", 600)
                                    elide: Text.ElideRight
                                    Layout.fillWidth: true
                                }
                                Text {
                                    text: root.unreadTotal + " unread"
                                    color: c("muted", "#a8ece7de")
                                    font.family: c("font-family", "Inter")
                                    font.pixelSize: n("font-size-micro", 9)
                                }
                            }
                        }
                    }

                    Text {
                        text: "MAILBOXES"
                        color: c("muted", "#a8ece7de")
                        font.family: c("font-family", "Inter")
                        font.pixelSize: n("font-size-micro", 9)
                        font.letterSpacing: 1
                        Layout.topMargin: n("space-3", 12)
                        Layout.leftMargin: n("space-2", 8)
                    }

                    Repeater {
                        model: root.systemFolders
                        delegate: FolderRow {
                            Layout.fillWidth: true
                            folder: modelData
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 1
                        Layout.topMargin: n("space-2", 8)
                        Layout.bottomMargin: n("space-2", 8)
                        color: c("border", "#171b1e23")
                    }

                    RowLayout {
                        Layout.fillWidth: true
                        Text {
                            text: "CUSTOM"
                            color: c("muted", "#a8ece7de")
                            font.family: c("font-family", "Inter")
                            font.pixelSize: n("font-size-micro", 9)
                            font.letterSpacing: 1
                            Layout.leftMargin: n("space-2", 8)
                            Layout.fillWidth: true
                        }
                        Icon { name: "plus"; size: n("icon-small", 14); color: c("accent-text", "#e7a97f") }
                    }

                    Repeater {
                        model: root.customFolders
                        delegate: FolderRow {
                            Layout.fillWidth: true
                            folder: modelData
                        }
                    }

                    Text {
                        text: "MAILING LISTS"
                        color: c("muted", "#a8ece7de")
                        font.family: c("font-family", "Inter")
                        font.pixelSize: n("font-size-micro", 9)
                        font.letterSpacing: 1
                        Layout.topMargin: n("space-3", 12)
                        Layout.leftMargin: n("space-2", 8)
                    }
                    Rectangle {
                        Layout.fillWidth: true
                        implicitHeight: 28
                        radius: n("radius-control", 8)
                        color: root.listFilter === "" ? c("hover", "#2e2e323a") : "transparent"
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            anchors.left: parent.left
                            anchors.leftMargin: n("space-3", 12)
                            text: "All mail"
                            color: c("text-2", "#dbebe7de")
                            font.family: c("font-family", "Inter")
                            font.pixelSize: n("font-size-small", 12)
                        }
                        MouseArea {
                            anchors.fill: parent
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.listFilter = ""
                        }
                    }
                    Repeater {
                        model: root.mailingLists
                        delegate: Rectangle {
                            Layout.fillWidth: true
                            implicitHeight: 28
                            radius: n("radius-control", 8)
                            color: root.listFilter === modelData.list_id ? c("selected", "#3d2e323a") : (listHover.hovered ? c("hover", "#2e2e323a") : "transparent")
                            HoverHandler { id: listHover }
                            RowLayout {
                                anchors.fill: parent
                                anchors.leftMargin: n("space-2", 8) + 4
                                anchors.rightMargin: n("space-2", 8)
                                spacing: n("space-2", 8)
                                Icon { name: "tag"; size: n("icon-small", 14); color: c("muted", "#a8ece7de") }
                                Text {
                                    text: modelData.name
                                    color: c("text-2", "#dbebe7de")
                                    font.family: c("font-family", "Inter")
                                    font.pixelSize: n("font-size-small", 12)
                                    elide: Text.ElideRight
                                    Layout.fillWidth: true
                                }
                                Text {
                                    text: modelData.unread > 0 ? modelData.unread : ""
                                    color: c("muted", "#a8ece7de")
                                    font.family: c("font-family", "Inter")
                                    font.pixelSize: n("font-size-micro", 9)
                                }
                            }
                            MouseArea {
                                anchors.fill: parent
                                cursorShape: Qt.PointingHandCursor
                                onClicked: {
                                    root.listFilter = modelData.list_id;
                                    app.openMailbox("inbox");
                                }
                            }
                        }
                    }
                    Text {
                        visible: root.mailingLists.length === 0
                        text: "Mailing lists appear when List-Id headers are present."
                        color: c("muted", "#a8ece7de")
                        font.family: c("font-family", "Inter")
                        font.pixelSize: n("font-size-micro", 9)
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                        Layout.leftMargin: n("space-2", 8)
                    }

                    Text {
                        visible: root.customFolders.length === 0
                        text: "Create a mailbox to route mail here."
                        color: c("muted", "#a8ece7de")
                        font.family: c("font-family", "Inter")
                        font.pixelSize: n("font-size-micro", 9)
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                        Layout.leftMargin: n("space-2", 8)
                    }

                    Item { Layout.fillHeight: true }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 1
                        color: c("border", "#171b1e23")
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 30
                        spacing: n("space-2", 8)
                        Icon { name: "rule"; size: n("icon-small", 14); color: c("muted", "#a8ece7de") }
                        Text {
                            text: "Mail rules"
                            color: c("text-2", "#dbebe7de")
                            font.family: c("font-family", "Inter")
                            font.pixelSize: n("font-size-small", 12)
                            Layout.fillWidth: true
                        }
                        Text {
                            text: "3"
                            color: c("muted", "#a8ece7de")
                            font.family: c("font-family", "Inter")
                            font.pixelSize: n("font-size-micro", 9)
                        }
                    }
                }
            }

            Rectangle {
                SplitView.preferredWidth: root.readerOpen ? 360 : 520
                SplitView.minimumWidth: 300
                SplitView.fillWidth: !root.readerOpen
                color: c("bg", "#121417")
                ColumnLayout {
                    anchors.fill: parent
                    spacing: 0
                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 58
                        color: "transparent"
                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: n("space-4", 16)
                            anchors.rightMargin: n("space-4", 16)
                            spacing: n("space-2", 8)
                            ColumnLayout {
                                spacing: 1
                                Text {
                                    text: root.folderLabel(app.folderId)
                                    color: c("text", "#ece7de")
                                    font.family: c("font-family", "Inter")
                                    font.pixelSize: n("font-size-title", 16)
                                    font.weight: n("font-weight-semibold", 600)
                                }
                                Text {
                                    text: root.visibleMessages.length + " messages"
                                    color: c("muted", "#a8ece7de")
                                    font.family: c("font-family", "Inter")
                                    font.pixelSize: n("font-size-micro", 9)
                                }
                            }
                            Item { Layout.fillWidth: true }
                            Rectangle {
                                implicitWidth: 28; implicitHeight: 28
                                radius: n("radius-control", 8)
                                color: root.unreadOnly ? c("selected", "#3d2e323a") : "transparent"
                                Icon { anchors.centerIn: parent; name: "inbox"; size: n("icon-small", 14); color: root.unreadOnly ? c("accent-text", "#e7a97f") : c("muted", "#a8ece7de") }
                                MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: root.unreadOnly = !root.unreadOnly }
                            }
                            IconButton { icon: "refresh"; label: "Refresh"; onClicked: app.refreshMail() }
                        }
                    }

                    Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: c("border", "#171b1e23") }

                    Item {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        visible: app.loading
                        ColumnLayout {
                            anchors.centerIn: parent
                            spacing: n("space-3", 12)
                            BusyIndicator { running: app.loading; Layout.alignment: Qt.AlignHCenter }
                            Text {
                                text: app.statusText
                                color: c("muted", "#a8ece7de")
                                font.family: c("font-family", "Inter")
                                font.pixelSize: n("font-size-small", 12)
                            }
                        }
                    }

                    ListView {
                        id: messageList
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        visible: !app.loading
                        clip: true
                        model: root.groupedMessages
                        delegate: MessageRow {
                            width: ListView.view.width
                            item: modelData
                        }
                        ScrollBar.vertical: ScrollBar {}
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        visible: !app.loading && root.groupedMessages.length === 0
                        spacing: n("space-3", 12)
                        Item { Layout.fillHeight: true }
                        Icon { name: "inbox"; size: 34; color: c("muted", "#a8ece7de"); Layout.alignment: Qt.AlignHCenter }
                        Text {
                            text: root.query.length > 0 || root.unreadOnly ? "No matching mail" : "This mailbox is empty"
                            color: c("text-2", "#dbebe7de")
                            font.family: c("font-family", "Inter")
                            font.pixelSize: n("font-size-body", 14)
                            Layout.alignment: Qt.AlignHCenter
                        }
                        Text {
                            text: root.query.length > 0 ? "Try a different search." : "New mail will appear here."
                            color: c("muted", "#a8ece7de")
                            font.family: c("font-family", "Inter")
                            font.pixelSize: n("font-size-small", 12)
                            Layout.alignment: Qt.AlignHCenter
                        }
                        Item { Layout.fillHeight: true }
                    }
                }
            }

            Rectangle {
                SplitView.fillWidth: root.readerOpen
                SplitView.minimumWidth: 320
                visible: root.readerOpen
                color: c("surface", "#1b1e23")
                ColumnLayout {
                    anchors.fill: parent
                    spacing: 0
                    RowLayout {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 58
                        Layout.leftMargin: n("space-4", 16)
                        Layout.rightMargin: n("space-4", 16)
                        spacing: n("space-1", 4)
                        IconButton { icon: "reply"; label: "Reply"; onClicked: root.composeOpen = true }
                        IconButton { icon: "forward"; label: "Forward"; onClicked: root.composeOpen = true }
                        Item { Layout.fillWidth: true }
                        IconButton { icon: "archive"; label: "Archive" }
                        IconButton { icon: "trash"; label: "Delete"; danger: true }
                        IconButton { icon: "more"; label: "More" }
                        IconButton { icon: "close"; label: "Close reader"; onClicked: root.readerOpen = false }
                    }
                    Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: c("border", "#171b1e23") }

                    Item {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        visible: !root.detail.id
                        ColumnLayout {
                            anchors.centerIn: parent
                            spacing: n("space-3", 12)
                            Icon { name: "inbox"; size: 38; color: c("muted", "#a8ece7de"); Layout.alignment: Qt.AlignHCenter }
                            Text {
                                text: "Select a message"
                                color: c("text-2", "#dbebe7de")
                                font.family: c("font-family", "Inter")
                                font.pixelSize: n("font-size-body", 14)
                            }
                        }
                    }

                    Flickable {
                        id: readerFlick
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        visible: root.detail.id !== undefined && root.detail.id !== ""
                        clip: true
                        contentWidth: width
                        contentHeight: readerColumn.implicitHeight + n("space-6", 32)
                        ScrollBar.vertical: ScrollBar {}
                        ColumnLayout {
                            id: readerColumn
                            width: Math.min(readerFlick.width - n("space-6", 32), n("reader-width", 720))
                            anchors.horizontalCenter: parent.horizontalCenter
                            anchors.top: parent.top
                            anchors.topMargin: n("space-5", 24)
                            spacing: n("space-3", 12)

                            Text {
                                text: root.detail.subject || "(no subject)"
                                color: c("text", "#ece7de")
                                font.family: c("font-family", "Inter")
                                font.pixelSize: n("font-size-large", 22)
                                font.weight: n("font-weight-semibold", 600)
                                wrapMode: Text.WordWrap
                                Layout.fillWidth: true
                            }
                            RowLayout {
                                Layout.fillWidth: true
                                spacing: n("space-3", 12)
                                Rectangle {
                                    width: 34; height: 34
                                    radius: 17
                                    color: c("accent-button", "#d98e5c")
                                    Text {
                                        anchors.centerIn: parent
                                        text: (root.detail.sender || "?").substring(0, 1).toUpperCase()
                                        color: c("accent-ink", "#171717")
                                        font.family: c("font-family", "Inter")
                                        font.pixelSize: n("font-size-body", 14)
                                        font.weight: n("font-weight-semibold", 600)
                                    }
                                }
                                ColumnLayout {
                                    spacing: 2
                                    Layout.fillWidth: true
                                    Text {
                                        text: root.detail.sender || "Unknown sender"
                                        color: c("text", "#ece7de")
                                        font.family: c("font-family", "Inter")
                                        font.pixelSize: n("font-size-small", 12)
                                        font.weight: n("font-weight-semibold", 600)
                                        elide: Text.ElideRight
                                        Layout.fillWidth: true
                                    }
                                    Text {
                                        text: "to " + (root.detail.recipients || "me") + " · " + root.formatWhen(root.detail.date)
                                        color: c("muted", "#a8ece7de")
                                        font.family: c("font-family", "Inter")
                                        font.pixelSize: n("font-size-micro", 9)
                                        elide: Text.ElideRight
                                        Layout.fillWidth: true
                                    }
                                }
                                Text {
                                    visible: root.detail.list_name || root.detail.list_id
                                    text: root.detail.list_name || root.detail.list_id || ""
                                    color: c("muted", "#a8ece7de")
                                    font.family: c("font-family", "Inter")
                                    font.pixelSize: n("font-size-micro", 9)
                                    elide: Text.ElideRight
                                    Layout.maximumWidth: 180
                                }
                                Rectangle {
                                    implicitWidth: trustRow.implicitWidth + n("space-3", 12) * 2
                                    implicitHeight: 26
                                    radius: n("radius-pill", 999)
                                    color: root.detail.sender_unverified || root.detail.not_end_to_end_encrypted
                                           ? Qt.rgba(0.85, 0.71, 0.42, 0.16)
                                           : Qt.rgba(0.5, 0.68, 0.51, 0.16)
                                    Row {
                                        id: trustRow
                                        anchors.centerIn: parent
                                        spacing: 6
                                        Icon {
                                            name: "shield"
                                            size: n("icon-small", 14)
                                            color: root.detail.sender_unverified || root.detail.not_end_to_end_encrypted
                                                   ? c("trust-warning", "#d9b36a")
                                                   : c("trust-verified", "#7fae83")
                                        }
                                        Text {
                                            text: root.detail.sender_unverified ? "Unverified sender"
                                                  : (root.detail.not_end_to_end_encrypted ? "Not end-to-end" : "Verified")
                                            color: root.detail.sender_unverified || root.detail.not_end_to_end_encrypted
                                                   ? c("trust-warning", "#d9b36a")
                                                   : c("trust-verified", "#7fae83")
                                            font.family: c("font-family", "Inter")
                                            font.pixelSize: n("font-size-micro", 9)
                                            font.weight: n("font-weight-medium", 500)
                                        }
                                    }
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                visible: root.threadSiblings.length > 1
                                spacing: n("space-2", 8)
                                Repeater {
                                    model: root.threadSiblings
                                    delegate: Rectangle {
                                        implicitWidth: threadPill.implicitWidth + n("space-3", 12) * 2
                                        implicitHeight: 26
                                        radius: n("radius-pill", 999)
                                        color: modelData.id === root.detail.id ? c("selected", "#3d2e323a") : c("card", "#23262c")
                                        border.width: n("border-width", 1)
                                        border.color: c("border", "#171b1e23")
                                        Row {
                                            id: threadPill
                                            anchors.centerIn: parent
                                            spacing: 6
                                            Text { text: (modelData.sender || "?").substring(0, 18); color: c("text-2", "#dbebe7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 9) }
                                            Text { text: root.formatWhen(modelData.date); color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 9) }
                                        }
                                        MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: app.selectMessage(modelData.id) }
                                    }
                                }
                                Item { Layout.fillWidth: true }
                            }

                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: 1
                                color: c("border", "#171b1e23")
                            }

                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: Math.max(340, Math.min(720, readerFlick.height - 220))
                                radius: n("radius-card", 14)
                                color: root.detail.is_html ? c("html-mail-bg", "#ffffff") : c("surface", "#1b1e23")
                                clip: true
                                WebEngineView {
                                    id: mailView
                                    anchors.fill: parent
                                    visible: root.detail.body_html && root.detail.body_html.length > 0
                                    settings.javascriptEnabled: false
                                    settings.pluginsEnabled: false
                                    settings.localStorageEnabled: false
                                    settings.localContentCanAccessRemoteUrls: false
                                    settings.localContentCanAccessFileUrls: false
                                    settings.autoLoadImages: false
                                    backgroundColor: root.detail.is_html ? c("html-mail-bg", "#ffffff") : c("surface", "#1b1e23")
                                    onNavigationRequested: function(request) {
                                        if (request.url.toString().indexOf("http") === 0) {
                                            Qt.openUrlExternally(request.url);
                                            request.action = WebEngineView.IgnoreRequest;
                                        }
                                    }
                                }
                                Text {
                                    anchors.centerIn: parent
                                    visible: !root.detail.body_html || root.detail.body_html.length === 0
                                    text: "This message has no readable body."
                                    color: c("muted", "#a8ece7de")
                                    font.family: c("font-family", "Inter")
                                    font.pixelSize: n("font-size-small", 12)
                                }
                                Connections {
                                    target: root
                                    function onDetailChanged() {
                                        mailReload.restart();
                                    }
                                }
                                Timer {
                                    id: mailReload
                                    interval: 60
                                    onTriggered: {
                                        if (root.detail.body_html)
                                            mailView.loadHtml(root.detail.body_html, "about:blank");
                                    }
                                }
                                Component.onCompleted: mailReload.restart()
                                onVisibleChanged: if (visible) mailReload.restart()
                            }

                            Text {
                                visible: root.detail.attachments && root.detail.attachments.length > 0
                                text: "ATTACHMENTS"
                                color: c("muted", "#a8ece7de")
                                font.family: c("font-family", "Inter")
                                font.pixelSize: n("font-size-micro", 9)
                                font.letterSpacing: 1
                            }
                            Flow {
                                Layout.fillWidth: true
                                spacing: n("space-2", 8)
                                Repeater {
                                    model: root.detail.attachments || []
                                    delegate: Rectangle {
                                        implicitWidth: attachRow.implicitWidth + n("space-3", 12) * 2
                                        implicitHeight: n("attachment-chip-height", 30)
                                        radius: n("radius-control", 8)
                                        color: c("card", "#23262c")
                                        border.width: n("border-width", 1)
                                        border.color: c("border", "#171b1e23")
                                        Row {
                                            id: attachRow
                                            anchors.centerIn: parent
                                            spacing: 6
                                            Icon { name: "attach"; size: n("icon-small", 14); color: c("muted", "#a8ece7de") }
                                            Text {
                                                text: modelData.name + " · " + root.formatBytes(modelData.size)
                                                color: c("text-2", "#dbebe7de")
                                                font.family: c("font-family", "Inter")
                                                font.pixelSize: n("font-size-micro", 9)
                                            }
                                        }
                                        MouseArea {
                                            anchors.fill: parent
                                            cursorShape: Qt.PointingHandCursor
                                            onClicked: app.saveAttachment(root.detail.id, modelData.seq)
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: n("statusbar-height", 30)
            color: c("sidebar", "#1a1d22")
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: n("space-3", 12)
                anchors.rightMargin: n("space-3", 12)
                spacing: n("space-3", 12)
                Text {
                    text: app.statusText
                    color: c("muted", "#a8ece7de")
                    font.family: c("font-family", "Inter")
                    font.pixelSize: n("font-size-micro", 9)
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
                Text {
                    text: "MCP idle"
                    color: c("muted", "#a8ece7de")
                    font.family: c("font-family", "Inter")
                    font.pixelSize: n("font-size-micro", 9)
                }
                Text {
                    text: "Theme " + app.themeId
                    color: c("muted", "#a8ece7de")
                    font.family: c("font-mono", "JetBrains Mono")
                    font.pixelSize: n("font-size-micro", 9)
                }
            }
        }
    }



    Connections {
        target: app
        function onToastTextChanged() {
            root.toastVisible = app.toastText.length > 0;
            toastTimer.restart();
        }
        function onConfirmDelete(token, ids) {
            root.pendingDeleteToken = token;
            root.pendingDeleteIds = ids;
            root.deleteConfirmWord = "";
            root.deleteConfirmOpen = true;
        }
    }

    Timer {
        id: toastTimer
        interval: 4000
        onTriggered: root.toastVisible = false
    }

    Rectangle {
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: n("statusbar-height", 30) + n("space-4", 16)
        width: toastContent.implicitWidth + n("space-4", 16) * 2
        height: 34
        radius: n("radius-control", 8)
        color: c("surface-strong", "#2e323a")
        border.width: n("border-width", 1)
        border.color: c("border-strong", "#292e323a")
        visible: root.toastVisible && app.toastText.length > 0
        Text {
            id: toastContent
            anchors.centerIn: parent
            text: app.toastText
            color: c("text", "#ece7de")
            font.family: c("font-family", "Inter")
            font.pixelSize: n("font-size-small", 12)
        }
    }

    Rectangle {
        anchors.fill: parent
        color: c("overlay", "#8c000000")
        visible: root.settingsOpen
        MouseArea { anchors.fill: parent; onClicked: root.settingsOpen = false }
        Rectangle {
            id: settingsPanel
            width: Math.min(root.width - 64, 1040)
            height: Math.min(root.height - 56, 760)
            anchors.centerIn: parent
            radius: n("radius-panel", 16)
            color: c("surface", "#1b1e23")
            border.width: n("border-width", 1)
            border.color: c("border-strong", "#292e323a")
            MouseArea { anchors.fill: parent }
            RowLayout {
                anchors.fill: parent
                spacing: 0
                Rectangle {
                    Layout.preferredWidth: 208
                    Layout.fillHeight: true
                    color: c("sidebar", "#1a1d22")
                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: n("space-3", 12)
                        spacing: n("space-1", 4)
                        RowLayout {
                            Layout.fillWidth: true
                            Layout.bottomMargin: n("space-3", 12)
                            spacing: n("space-2", 8)
                            Icon { name: "settings"; size: n("icon-body", 16); color: c("accent-text", "#e7a97f") }
                            Text {
                                text: "Settings"
                                color: c("text", "#ece7de")
                                font.family: c("font-family", "Inter")
                                font.pixelSize: n("font-size-title", 16)
                                font.weight: n("font-weight-semibold", 600)
                                Layout.fillWidth: true
                            }
                        }
                        NavSettingsRow { label: "Appearance"; tab: "appearance"; icon: "sun"; Layout.fillWidth: true }
                        NavSettingsRow { label: "Mail"; tab: "mail"; icon: "inbox"; Layout.fillWidth: true }
                        NavSettingsRow { label: "Rules & filters"; tab: "rules"; icon: "rule"; Layout.fillWidth: true }
                        NavSettingsRow { label: "Aliases & mailboxes"; tab: "aliases"; icon: "folder"; Layout.fillWidth: true }
                        NavSettingsRow { label: "Contacts"; tab: "contacts"; icon: "inbox"; Layout.fillWidth: true }
                        NavSettingsRow { label: "Security & PGP"; tab: "security"; icon: "shield"; Layout.fillWidth: true }
                        NavSettingsRow { label: "MCP"; tab: "mcp"; icon: "send"; Layout.fillWidth: true }
                        NavSettingsRow { label: "Diagnostics"; tab: "diagnostics"; icon: "settings"; Layout.fillWidth: true }
                        Item { Layout.fillHeight: true }
                        Text {
                            text: "Aster Qt"
                            color: c("muted", "#a8ece7de")
                            font.family: c("font-broken", "Inter")
                            font.pixelSize: n("font-size-micro", 9)
                        }
                    }
                }
                Rectangle {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    color: c("surface", "#1b1e23")
                    ColumnLayout {
                        anchors.fill: parent
                        spacing: 0
                        RowLayout {
                            Layout.fillWidth: true
                            Layout.preferredHeight: 54
                            Layout.leftMargin: n("space-4", 16)
                            Layout.rightMargin: n("space-4", 16)
                            Text {
                                text: ({ appearance: "Appearance", mail: "Mail", rules: "Rules & filters", aliases: "Aliases & mailboxes", contacts: "Contacts", security: "Security & PGP", mcp: "MCP", diagnostics: "Diagnostics" })[root.settingsTab] || "Settings"
                                color: c("text", "#ece7de")
                                font.family: c("font-family", "Inter")
                                font.pixelSize: n("font-size-title", 16)
                                font.weight: n("font-weight-semibold", 600)
                                Layout.fillWidth: true
                            }
                            IconButton { icon: "close"; label: "Close settings"; onClicked: root.settingsOpen = false }
                        }
                        Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: c("border", "#171b1e23") }
                        Flickable {
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            clip: true
                            contentWidth: width
                            contentHeight: settingsContent.implicitHeight + n("space-5", 24) * 2
                            ScrollBar.vertical: ScrollBar {}
                            ColumnLayout {
                                id: settingsContent
                                width: Math.min(parent.width - n("space-5", 24) * 2, 720)
                                anchors.horizontalCenter: parent.horizontalCenter
                                anchors.top: parent.top
                                anchors.topMargin: n("space-5", 24)
                                spacing: n("space-3", 12)

                                // Appearance
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    visible: root.settingsTab === "appearance"
                                    spacing: n("space-3", 12)
                                    SettingsTitle { text: "Appearance" }
                                    SettingsSubtitle { text: "Theme selection lives only here. Every visual value is a CSS token; drop themes into the folder below." }
                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: n("space-2", 8)
                                        ModeChip { label: "System"; active: app.themeMode === "system"; onClicked: app.applyThemeMode("system") }
                                        ModeChip { label: "Light"; active: app.themeMode === "light"; onClicked: app.applyThemeMode("light") }
                                        ModeChip { label: "Dark"; active: app.themeMode === "dark"; onClicked: app.applyThemeMode("dark") }
                                    }
                                    Text { text: "LIGHT THEME"; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 9); font.letterSpacing: 1; Layout.topMargin: n("space-2", 8) }
                                    Repeater {
                                        model: root.lightThemes
                                        delegate: ThemeRow { Layout.fillWidth: true; entry: modelData; active: app.themeId === modelData.id }
                                    }
                                    Text { text: "DARK THEME"; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 9); font.letterSpacing: 1; Layout.topMargin: n("space-2", 8) }
                                    Repeater {
                                        model: root.darkThemes
                                        delegate: ThemeRow { Layout.fillWidth: true; entry: modelData; active: app.themeId === modelData.id }
                                    }
                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: n("space-3", 12)
                                        LinkText { text: "Open themes folder"; action: function() { app.openThemesFolder() } }
                                        LinkText { text: "New from template"; action: function() { app.createThemeFromTemplate() } }
                                        Item { Layout.fillWidth: true }
                                        LinkText { text: "Reload"; action: function() { app.refreshThemes() } }
                                    }
                                    Text {
                                        text: app.dataDir + "/themes"
                                        color: c("muted", "#a8ece7de")
                                        font.family: c("font-mono", "JetBrains Mono")
                                        font.pixelSize: n("font-size-micro", 9)
                                        elide: Text.ElideMiddle
                                        Layout.fillWidth: true
                                    }
                                }

                                // Mail
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    visible: root.settingsTab === "mail"
                                    spacing: n("space-3", 12)
                                    SettingsTitle { text: "Mail" }
                                    SettingsSubtitle { text: "Sync and reading preferences." }
                                    RowLayout {
                                        Layout.fillWidth: true
                                        ColumnLayout { spacing: 2; Layout.fillWidth: true
                                            Text { text: "Full mail history"; color: c("text", "#ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-small", 12); font.weight: n("font-weight-medium", 500) }
                                            Text { text: "Index every message, not only the newest 2,000 per folder."; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 9); wrapMode: Text.WordWrap; Layout.fillWidth: true }
                                        }
                                        Toggle {
                                            checked: root.settings.mail ? root.settings.mail.full_history !== false : true
                                            onToggled: app.applySettings(JSON.stringify({ mail: { full_history: !checked } }))
                                        }
                                    }
                                    RowLayout {
                                        Layout.fillWidth: true
                                        ColumnLayout { spacing: 2; Layout.fillWidth: true
                                            Text { text: "Poll interval"; color: c("text", "#ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-small", 12); font.weight: n("font-weight-medium", 500) }
                                            Text { text: "Seconds between sync passes. Applies on next launch."; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 9); wrapMode: Text.WordWrap; Layout.fillWidth: true }
                                        }
                                        SettingsField {
                                            id: pollField
                                            Layout.preferredWidth: 96
                                            placeholder: "30"
                                            text: root.settings.mail ? String(root.settings.mail.poll_interval_secs || 30) : "30"
                                        }
                                        LinkText {
                                            text: "Save"
                                            action: function() {
                                                app.applySettings(JSON.stringify({ mail: { poll_interval_secs: parseInt(pollField.text) || 30 } }))
                                            }
                                        }
                                    }
                                }

                                // Rules
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    visible: root.settingsTab === "rules"
                                    spacing: n("space-3", 12)
                                    SettingsTitle { text: "Rules & filters" }
                                    SettingsSubtitle { text: "Server-side rules run even when Aster Qt is closed. Conditions support is, contains, starts_with, ends_with, matches_domain, and matches_regex." }
                                    Repeater {
                                        model: root.rules
                                        delegate: Rectangle {
                                            Layout.fillWidth: true
                                            implicitHeight: ruleRow.implicitHeight + n("space-3", 12) * 2
                                            radius: n("radius-control", 8)
                                            color: c("card", "#23262c")
                                            border.width: n("border-width", 1)
                                            border.color: c("border", "#171b1e23")
                                            RowLayout {
                                                id: ruleRow
                                                anchors.fill: parent
                                                anchors.margins: n("space-3", 12)
                                                spacing: n("space-2", 8)
                                                ColumnLayout {
                                                    spacing: 2
                                                    Layout.fillWidth: true
                                                    Text { text: modelData.name || "Untitled rule"; color: c("text", "#ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-small", 12); font.weight: n("font-weight-semibold", 600) }
                                                    Text { text: (modelData.conditions ? modelData.conditions.length : 0) + " conditions · " + (modelData.actions ? modelData.actions.length : 0) + " actions · " + (modelData.applied_count || 0) + " applied"; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 9) }
                                                }
                                                LinkText { text: "Run"; action: function() { app.runRule(modelData.id) } }
                                                LinkText { text: "Delete"; action: function() { app.deleteRule(modelData.id) } }
                                            }
                                        }
                                    }
                                    Text { visible: root.rules.length === 0; text: "No rules yet. Create one below."; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-small", 12) }
                                    Text { text: "NEW RULE"; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 9); font.letterSpacing: 1; Layout.topMargin: n("space-2", 8) }
                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: n("space-2", 8)
                                        SettingsField { id: ruleName; Layout.fillWidth: true; placeholder: "Rule name" }
                                        ModeChip { label: "All"; active: root.ruleMatchAll; onClicked: root.ruleMatchAll = true }
                                        ModeChip { label: "Any"; active: !root.ruleMatchAll; onClicked: root.ruleMatchAll = false }
                                    }
                                    Text { text: "Conditions"; color: c("text-2", "#dbebe7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-small", 12); font.weight: n("font-weight-medium", 500) }
                                    Repeater {
                                        model: conditionModel
                                        delegate: RowLayout {
                                            Layout.fillWidth: true
                                            spacing: n("space-2", 8)
                                            SettingsField {
                                                Layout.preferredWidth: 120
                                                placeholder: "field"
                                                Component.onCompleted: text = model.field
                                                onEdited: function(value) { conditionModel.setProperty(index, "field", value) }
                                            }
                                            SettingsField {
                                                Layout.preferredWidth: 140
                                                placeholder: "operator"
                                                Component.onCompleted: text = model.op
                                                onEdited: function(value) { conditionModel.setProperty(index, "op", value) }
                                            }
                                            SettingsField {
                                                Layout.fillWidth: true
                                                placeholder: "value"
                                                Component.onCompleted: text = model.value
                                                onEdited: function(value) { conditionModel.setProperty(index, "value", value) }
                                            }
                                            LinkText { text: "Remove"; action: function() { conditionModel.remove(index) } }
                                        }
                                    }
                                    RowLayout {
                                        Layout.fillWidth: true
                                        LinkText { text: "Add condition"; action: function() { conditionModel.append({ field: "subject", op: "contains", value: "" }) } }
                                        Item { Layout.fillWidth: true }
                                    }
                                    Text { text: "ACTIONS"; color: c("text-2", "#dbebe7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-small", 12); font.weight: n("font-weight-medium", 500) }
                                    Repeater {
                                        model: actionModel
                                        delegate: RowLayout {
                                            Layout.fillWidth: true
                                            spacing: n("space-2", 8)
                                            SettingsField {
                                                Layout.preferredWidth: 130
                                                placeholder: "action"
                                                Component.onCompleted: text = model.kind
                                                onEdited: function(value) { actionModel.setProperty(index, "kind", value) }
                                            }
                                            SettingsField {
                                                Layout.fillWidth: true
                                                placeholder: "target (mailbox token, address, label, template, date)"
                                                Component.onCompleted: text = model.target
                                                onEdited: function(value) { actionModel.setProperty(index, "target", value) }
                                            }
                                            LinkText { text: "Remove"; action: function() { actionModel.remove(index) } }
                                        }
                                    }
                                    RowLayout {
                                        Layout.fillWidth: true
                                        LinkText { text: "Add action"; action: function() { actionModel.append({ kind: "move_to", target: "" }) } }
                                        Item { Layout.fillWidth: true }
                                        LinkText { text: "Create rule"; action: function() { root.createVisualRule() } }
                                    }
                                    Text {
                                        text: "Fields: from, to, cc, bcc, subject, body, list_id, header, attachment_name, has_attachment, is_reply, is_forward, attachment_size, total_size, recipient_count, spam_score, date_received, dkim_result, spf_result, dmarc_result. Operators: is, is_not, contains, does_not_contain, starts_with, ends_with, matches_domain, does_not_match_domain, is_empty, matches_regex. Actions: move_to, apply_labels, mark_as, star, skip_inbox, delete, forward, auto_reply, pin, snooze, categorize, notify."
                                        color: c("muted", "#a8ece7de")
                                        font.family: c("font-family", "Inter")
                                        font.pixelSize: n("font-size-micro", 9)
                                        wrapMode: Text.WordWrap
                                        Layout.fillWidth: true
                                    }
                                    Text { text: "NEW RULE (FULL JSON)"; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 9); font.letterSpacing: 1; Layout.topMargin: n("space-2", 8) }
                                    Rectangle {
                                        Layout.fillWidth: true
                                        Layout.preferredHeight: 150
                                        radius: n("radius-control", 8)
                                        color: c("input", "#221b1e23")
                                        border.width: n("border-width", 1)
                                        border.color: c("border", "#171b1e23")
                                        TextArea {
                                            id: ruleJson
                                            anchors.fill: parent
                                            anchors.margins: n("space-2", 8)
                                            color: c("text", "#ece7de")
                                            placeholderText: '{"name":"Invoices","color":"#d98e5c","enabled":true,"match_mode":"all","conditions":[{"field":"subject","op":"matches_regex","value":"(?i)invoice|receipt"}],"actions":[{"type":"move_to","folder_token":"<mailbox-token>"}]}' 
                                            placeholderTextColor: c("muted", "#a8ece7de")
                                            selectionColor: c("accent", "#d98e5c")
                                            selectedTextColor: c("accent-ink", "#171717")
                                            wrapMode: TextArea.Wrap
                                            background: null
                                            font.family: c("font-mono", "JetBrains Mono")
                                            font.pixelSize: n("font-size-micro", 9)
                                        }
                                    }
                                    RowLayout {
                                        Layout.fillWidth: true
                                        LinkText { text: "Create rule"; action: function() { app.createRuleJson(ruleJson.text) } }
                                        Item { Layout.fillWidth: true }
                                        Text { text: "Mailbox tokens are shown in Aliases & mailboxes."; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 9) }
                                    }
                                }

                                // Aliases and mailboxes
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    visible: root.settingsTab === "aliases"
                                    spacing: n("space-3", 12)
                                    SettingsTitle { text: "Aliases & mailboxes" }
                                    SettingsSubtitle { text: "Route mail for an alias into a dedicated mailbox. This creates the mailbox and a server-side rule automatically." }
                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: n("space-2", 8)
                                        SettingsField { id: newMailboxName; Layout.fillWidth: true; placeholder: "Mailbox name" }
                                        LinkText { text: "Create mailbox"; action: function() { app.createMailbox(newMailboxName.text) } }
                                    }
                                    Repeater {
                                        model: root.identities
                                        delegate: Rectangle {
                                            Layout.fillWidth: true
                                            implicitHeight: aliasRow.implicitHeight + n("space-3", 12) * 2
                                            radius: n("radius-control", 8)
                                            color: c("card", "#23262c")
                                            border.width: n("border-width", 1)
                                            border.color: c("border", "#171b1e23")
                                            RowLayout {
                                                id: aliasRow
                                                anchors.fill: parent
                                                anchors.margins: n("space-3", 12)
                                                spacing: n("space-2", 8)
                                                ColumnLayout {
                                                    spacing: 2
                                                    Layout.fillWidth: true
                                                    Text { text: modelData.address; color: c("text", "#ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-small", 12); font.weight: n("font-weight-medium", 500) }
                                                    Text { text: modelData.kind + (modelData.is_default ? " · default" : ""); color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 9) }
                                                }
                                                LinkText { text: "Route to mailbox"; action: function() { app.routeAlias(modelData.address, newMailboxName.text) } }
                                            }
                                        }
                                    }
                                    Text { text: "MAILBOX TOKENS"; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 9); font.letterSpacing: 1; Layout.topMargin: n("space-2", 8) }
                                    Repeater {
                                        model: root.customFolders
                                        delegate: Text {
                                            text: modelData.name + "  " + (modelData.token || "")
                                            color: c("muted", "#a8ece7de")
                                            font.family: c("font-mono", "JetBrains Mono")
                                            font.pixelSize: n("font-size-micro", 9)
                                        }
                                    }
                                    Text { visible: root.customFolders.length === 0; text: "No custom mailboxes yet."; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 9) }
                                }

                                // Contacts
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    visible: root.settingsTab === "contacts"
                                    spacing: n("space-3", 12)
                                    SettingsTitle { text: "Contacts" }
                                    SettingsSubtitle { text: "Create or update a contact from a vCard, then sync happens through the Aster encrypted contact store." }
                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: n("space-2", 8)
                                        SettingsField { id: contactUid; Layout.preferredWidth: 180; placeholder: "uid" }
                                        LinkText { text: "Save vCard"; action: function() { app.saveContact(contactUid.text, contactVcard.text) } }
                                        Item { Layout.fillWidth: true }
                                    }
                                    Rectangle {
                                        Layout.fillWidth: true
                                        Layout.preferredHeight: 120
                                        radius: n("radius-control", 8)
                                        color: c("input", "#221b1e23")
                                        border.width: n("border-width", 1)
                                        border.color: c("border", "#171b1e23")
                                        TextArea {
                                            id: contactVcard
                                            anchors.fill: parent
                                            anchors.margins: n("space-2", 8)
                                            color: c("text", "#ece7de")
                                            placeholderText: "BEGIN:VCARD\nVERSION:3.0\nFN:Name\nEMAIL:name@example.com\nEND:VCARD"
                                            placeholderTextColor: c("muted", "#a8ece7de")
                                            wrapMode: TextArea.Wrap
                                            background: null
                                            font.family: c("font-mono", "JetBrains Mono")
                                            font.pixelSize: n("font-size-micro", 9)
                                        }
                                    }
                                    Repeater {
                                        model: root.contacts
                                        delegate: Rectangle {
                                            Layout.fillWidth: true
                                            implicitHeight: contactRow.implicitHeight + n("space-3", 12) * 2
                                            radius: n("radius-control", 8)
                                            color: c("card", "#23262c")
                                            border.width: n("border-width", 1)
                                            border.color: c("border", "#171b1e23")
                                            RowLayout {
                                                id: contactRow
                                                anchors.fill: parent
                                                anchors.margins: n("space-3", 12)
                                                spacing: n("space-2", 8)
                                                ColumnLayout {
                                                    spacing: 2
                                                    Layout.fillWidth: true
                                                    Text { text: modelData.uid; color: c("text", "#ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-small", 12); font.weight: n("font-weight-medium", 500) }
                                                    Text { text: (modelData.vcard || "").substring(0, 120); color: c("muted", "#a8ece7de"); font.family: c("font-mono", "JetBrains Mono"); font.pixelSize: n("font-size-micro", 9); elide: Text.ElideRight; Layout.fillWidth: true }
                                                }
                                                LinkText { text: "Delete"; action: function() { app.deleteContact(modelData.uid) } }
                                            }
                                        }
                                    }
                                    Text { visible: root.contacts.length === 0; text: "No contacts loaded."; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 9) }
                                }

                                // Security
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    visible: root.settingsTab === "security"
                                    spacing: n("space-3", 12)
                                    SettingsTitle { text: "Security & PGP" }
                                    SettingsSubtitle { text: "Account key, keyserver publication, and encryption preferences." }
                                    Rectangle {
                                        Layout.fillWidth: true
                                        implicitHeight: pgpCol.implicitHeight + n("space-3", 12) * 2
                                        radius: n("radius-control", 8)
                                        color: c("card", "#23262c")
                                        border.width: n("border-width", 1)
                                        border.color: c("border", "#171b1e23")
                                        ColumnLayout {
                                            id: pgpCol
                                            anchors.fill: parent
                                            anchors.margins: n("space-3", 12)
                                            spacing: 3
                                            Text { text: root.pgp.fingerprint ? "PGP key" : "PGP key unavailable"; color: c("text", "#ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-small", 12); font.weight: n("font-weight-semibold", 600) }
                                            Text { text: root.pgp.fingerprint || ""; color: c("muted", "#a8ece7de"); font.family: c("font-mono", "JetBrains Mono"); font.pixelSize: n("font-size-micro", 9); wrapMode: Text.WrapAnywhere; Layout.fillWidth: true }
                                            Text { text: (root.pgp.algorithm || "") + " " + (root.pgp.key_size || "") + " · created " + (root.pgp.created_at || ""); color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 9); wrapMode: Text.WordWrap; Layout.fillWidth: true }
                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: n("space-3", 12)
                                                LinkText { text: "Publish WKD"; action: function() { app.publishPgp("wkd") } }
                                                LinkText { text: "Unpublish WKD"; action: function() { app.publishPgp("wkd-off") } }
                                                LinkText { text: "Publish keyserver"; action: function() { app.publishPgp("keyserver") } }
                                                Item { Layout.fillWidth: true }
                                            }
                                        }
                                    }
                                    RowLayout {
                                        Layout.fillWidth: true
                                        ColumnLayout { spacing: 2; Layout.fillWidth: true
                                            Text { text: "Require post-quantum protection"; color: c("text", "#ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-small", 12); font.weight: n("font-weight-medium", 500) }
                                            Text { text: "Block sends to Aster recipients whose keys do not support PQ."; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 9); wrapMode: Text.WordWrap; Layout.fillWidth: true }
                                        }
                                        Toggle {
                                            checked: root.settings.security ? root.settings.security.require_post_quantum === true : false
                                            onToggled: app.applySettings(JSON.stringify({ security: { require_post_quantum: !checked } }))
                                        }
                                    }
                                    RowLayout {
                                        Layout.fillWidth: true
                                        ColumnLayout { spacing: 2; Layout.fillWidth: true
                                            Text { text: "Encrypt messages by default"; color: c("text", "#ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-small", 12); font.weight: n("font-weight-medium", 500) }
                                            Text { text: "Use Aster end-to-end encryption whenever recipient keys are available."; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 9); wrapMode: Text.WordWrap; Layout.fillWidth: true }
                                        }
                                        Toggle {
                                            checked: root.settings.security ? root.settings.security.encrypt_emails !== false : true
                                            onToggled: app.applySettings(JSON.stringify({ security: { encrypt_emails: !checked } }))
                                        }
                                    }
                                }

                                // MCP
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    visible: root.settingsTab === "mcp"
                                    spacing: n("space-3", 12)
                                    SettingsTitle { text: "MCP" }
                                    SettingsSubtitle { text: "Controls the dedicated MCP server. Mutating tools stay off until you enable them here." }
                                    RowLayout {
                                        Layout.fillWidth: true
                                        Text { text: "Enable MCP server"; color: c("text", "#ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-small", 12); Layout.fillWidth: true }
                                        Toggle { checked: root.settings.mcp ? root.settings.mcp.enabled === true : false; onToggled: app.applySettings(JSON.stringify({ mcp: { enabled: !checked } })) }
                                    }
                                    RowLayout {
                                        Layout.fillWidth: true
                                        Text { text: "Allow sending mail"; color: c("text", "#ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-small", 12); Layout.fillWidth: true }
                                        Toggle { checked: root.settings.mcp ? root.settings.mcp.allow_send === true : false; onToggled: app.applySettings(JSON.stringify({ mcp: { allow_send: !checked } })) }
                                    }
                                    RowLayout {
                                        Layout.fillWidth: true
                                        Text { text: "Allow destructive actions"; color: c("text", "#ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-small", 12); Layout.fillWidth: true }
                                        Toggle { checked: root.settings.mcp ? root.settings.mcp.allow_destructive === true : false; onToggled: app.applySettings(JSON.stringify({ mcp: { allow_destructive: !checked } })) }
                                    }
                                }

                                // Diagnostics
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    visible: root.settingsTab === "diagnostics"
                                    spacing: n("space-3", 12)
                                    SettingsTitle { text: "Diagnostics" }
                                    SettingsSubtitle { text: "Local state used by Aster Qt." }
                                    Text { text: "Data directory"; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 9); font.letterSpacing: 1 }
                                    Text { text: app.dataDir; color: c("text-2", "#dbebe7de"); font.family: c("font-mono", "JetBrains Mono"); font.pixelSize: n("font-size-micro", 9); wrapMode: Text.WrapAnywhere; Layout.fillWidth: true }
                                    RowLayout {
                                        Layout.fillWidth: true
                                        LinkText { text: "Open themes folder"; action: function() { app.openThemesFolder() } }
                                        Item { Layout.fillWidth: true }
                                        LinkText { text: "Reload themes"; action: function() { app.refreshThemes() } }
                                        LinkText { text: "Refresh mail"; action: function() { app.refreshMail() } }
                                    }
                                    Text { text: app.toastText; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 9); wrapMode: Text.WordWrap; Layout.fillWidth: true }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Rectangle {
        anchors.fill: parent
        color: c("overlay", "#8c000000")
        visible: root.deleteConfirmOpen
        MouseArea { anchors.fill: parent }
        Rectangle {
            width: 420
            height: 210
            anchors.centerIn: parent
            radius: n("radius-panel", 16)
            color: c("surface", "#1b1e23")
            border.width: n("border-width", 1)
            border.color: c("border-strong", "#292e323a")
            ColumnLayout {
                anchors.fill: parent
                anchors.margins: n("space-5", 24)
                spacing: n("space-3", 12)
                Text {
                    text: "Permanently delete mail?"
                    color: c("text", "#ece7de")
                    font.family: c("font-family", "Inter")
                    font.pixelSize: n("font-size-title", 16)
                    font.weight: n("font-weight-semibold", 600)
                }
                Text {
                    text: "An MCP agent requested permanent deletion of " + JSON.parse(root.pendingDeleteIds || "[]").length + " message(s). This cannot be undone."
                    color: c("text-2", "#dbebe7de")
                    font.family: c("font-family", "Inter")
                    font.pixelSize: n("font-size-small", 12)
                    wrapMode: Text.WordWrap
                    Layout.fillWidth: true
                }
                SettingsField {
                    id: deleteWord
                    Layout.fillWidth: true
                    placeholder: 'Type delete to confirm'
                    onEdited: function(value) { root.deleteConfirmWord = value }
                }
                Item { Layout.fillHeight: true }
                RowLayout {
                    Layout.fillWidth: true
                    spacing: n("space-2", 8)
                    Item { Layout.fillWidth: true }
                    Rectangle {
                        implicitWidth: 80
                        implicitHeight: n("control-height", 36)
                        radius: n("radius-control", 8)
                        color: cancelHover.hovered ? c("hover", "#2e2e323a") : "transparent"
                        HoverHandler { id: cancelHover }
                        Text { anchors.centerIn: parent; text: "Cancel"; color: c("text-2", "#dbebe7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-small", 12) }
                        MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: { app.resolveDelete(root.pendingDeleteToken, false); root.deleteConfirmOpen = false; } }
                    }
                    Rectangle {
                        implicitWidth: 96
                        implicitHeight: n("control-height", 36)
                        radius: n("radius-control", 8)
                        color: root.deleteConfirmWord.toLowerCase() === "delete"
                               ? (deleteHover.hovered ? Qt.lighter(c("danger", "#d97a72"), 1.08) : c("danger", "#d97a72"))
                               : c("surface-strong", "#2e323a")
                        opacity: root.deleteConfirmWord.toLowerCase() === "delete" ? 1.0 : 0.5
                        HoverHandler { id: deleteHover }
                        Text { anchors.centerIn: parent; text: "Delete"; color: root.deleteConfirmWord.toLowerCase() === "delete" ? c("accent-ink", "#171717") : c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-small", 12); font.weight: n("font-weight-semibold", 600) }
                        MouseArea {
                            anchors.fill: parent
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                if (root.deleteConfirmWord.toLowerCase() !== "delete") return;
                                app.resolveDelete(root.pendingDeleteToken, true);
                                root.deleteConfirmOpen = false;
                            }
                        }
                    }
                }
            }
        }
    }

    Rectangle {
        anchors.fill: parent
        color: c("overlay", "#8c000000")
        visible: root.composeOpen
        MouseArea { anchors.fill: parent; onClicked: root.composeOpen = false }
        Rectangle {
            width: Math.min(root.width - 80, 760)
            height: Math.min(root.height - 80, 640)
            anchors.centerIn: parent
            radius: n("radius-panel", 16)
            color: c("surface", "#1b1e23")
            border.width: n("border-width", 1)
            border.color: c("border-strong", "#292e323a")
            MouseArea { anchors.fill: parent }
            ColumnLayout {
                anchors.fill: parent
                anchors.margins: n("space-5", 24)
                spacing: n("space-3", 12)
                RowLayout {
                    Layout.fillWidth: true
                    Text {
                        text: "New message"
                        color: c("text", "#ece7de")
                        font.family: c("font-family", "Inter")
                        font.pixelSize: n("font-size-title", 16)
                        font.weight: n("font-weight-semibold", 600)
                        Layout.fillWidth: true
                    }
                    Text {
                        text: "Markdown"
                        color: c("accent-text", "#e7a97f")
                        font.family: c("font-mono", "JetBrains Mono")
                        font.pixelSize: n("font-size-micro", 9)
                    }
                    IconButton { icon: "close"; label: "Close composer"; onClicked: root.composeOpen = false }
                }
                SettingsField { id: composeTo; Layout.fillWidth: true; placeholder: "To, comma separated" }
                RowLayout {
                    Layout.fillWidth: true
                    spacing: n("space-2", 8)
                    SettingsField { id: composeCc; Layout.fillWidth: true; placeholder: "Cc" }
                    SettingsField { id: composeBcc; Layout.fillWidth: true; placeholder: "Bcc" }
                }
                SettingsField { id: composeSubject; Layout.fillWidth: true; placeholder: "Subject" }
                Text {
                    text: "BODY · MARKDOWN"
                    color: c("muted", "#a8ece7de")
                    font.family: c("font-family", "Inter")
                    font.pixelSize: n("font-size-micro", 9)
                    font.letterSpacing: 1
                }
                Rectangle {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    radius: n("radius-control", 8)
                    color: c("input", "#221b1e23")
                    border.width: composeBody.activeFocus ? n("border-width-focus", 1.5) : n("border-width", 1)
                    border.color: composeBody.activeFocus ? c("accent", "#d98e5c") : c("border", "#171b1e23")
                    TextArea {
                        id: composeBody
                        anchors.fill: parent
                        anchors.margins: n("space-3", 12)
                        color: c("text", "#ece7de")
                        placeholderText: "# Hello\n\nWrite in Markdown. Bold, lists, links, quotes and code blocks are converted to email HTML and plain text."
                        placeholderTextColor: c("muted", "#a8ece7de")
                        selectionColor: c("accent", "#d98e5c")
                        selectedTextColor: c("accent-ink", "#171717")
                        wrapMode: TextArea.Wrap
                        background: null
                        font.family: c("font-mono", "JetBrains Mono")
                        font.pixelSize: n("font-size-small", 12)
                    }
                }
                RowLayout {
                    Layout.fillWidth: true
                    spacing: n("space-2", 8)
                    IconButton { icon: "attach"; label: "Attach" }
                    Item { Layout.fillWidth: true }
                    Text {
                        text: "Markdown is converted to HTML and plain text before sending."
                        color: c("muted", "#a8ece7de")
                        font.family: c("font-family", "Inter")
                        font.pixelSize: n("font-size-micro", 9)
                    }
                    Rectangle {
                        implicitWidth: 96
                        implicitHeight: n("control-height", 36)
                        radius: n("radius-control", 8)
                        color: sendHover.hovered ? Qt.lighter(c("accent-button", "#d98e5c"), 1.06) : c("accent-button", "#d98e5c")
                        HoverHandler { id: sendHover }
                        Text {
                            anchors.centerIn: parent
                            text: "Send"
                            color: c("accent-ink", "#171717")
                            font.family: c("font-family", "Inter")
                            font.pixelSize: n("font-size-small", 12)
                            font.weight: n("font-weight-semibold", 600)
                        }
                        MouseArea {
                            anchors.fill: parent
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                app.sendMessage(composeTo.text, composeCc.text, composeBcc.text, composeSubject.text, composeBody.text);
                                root.composeOpen = false;
                            }
                        }
                    }
                }
            }
        }
    }
}
