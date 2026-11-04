import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window
import QtQuick.Dialogs
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

    function unsubscribeUrl(value) {
        if (!value) return "";
        var match = value.match(/<(https?:[^>]+|mailto:[^>]+)>/i);
        if (match) return match[1];
        match = value.match(/(https?:\/\/[^\s,]+|mailto:[^\s,]+)/i);
        return match ? match[1] : "";
    }

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

    readonly property var threadOrdered: {
        var source = root.threadMessages.length > 1 ? root.threadMessages : root.threadSiblings;
        var out = [];
        var seen = {};
        for (var i = 0; i < source.length; i++) {
            var message = source[i];
            if (!message || !message.id || seen[message.id]) continue;
            seen[message.id] = true;
            out.push(message);
        }
        out.sort(function(a, b) {
            var ta = a.date ? new Date(a.date).getTime() : 0;
            var tb = b.date ? new Date(b.date).getTime() : 0;
            return ta - tb;
        });
        return out;
    }

    readonly property int threadIndex: {
        for (var i = 0; i < root.threadOrdered.length; i++)
            if (root.threadOrdered[i].id === root.detail.id) return i;
        return -1;
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
    property string filterFrom: ""
    property string filterSubject: ""
    property string filterBody: ""
    property string filterSince: ""
    property string filterUntil: ""
    property bool filterStarred: false
    property bool filterAttachmentsOnly: false
    property string categoryFilter: ""
    property bool composeOpen: false
    property bool settingsOpen: false
    property string settingsTab: "appearance"
    property bool readerOpen: true
    property bool threadListOpen: false
    property var aliasSelection: []
    property string composeFrom: ""
    property string editingDraftId: ""
    property bool sendUndoVisible: false
    property var pendingSend: null
    readonly property var scheduledSends: root.settings.compose && root.settings.compose.scheduled ? root.settings.compose.scheduled : []
    property string activeIdentity: ""
    property var composeAttachments: []
    property bool toastVisible: false
    property bool deleteConfirmOpen: false
    property bool emptyConfirmOpen: false
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
    readonly property var gpg: parse(app.gpgJson, ({}))
    readonly property var contacts: parse(app.contactsJson, [])
    readonly property var tags: parse(app.tagsJson, [])
    readonly property var serverTemplates: parse(app.templatesJson, [])
    readonly property var serverSignatures: parse(app.signaturesJson, [])
    readonly property var threadMessages: parse(app.threadJson, [])
    readonly property var mailingLists: parse(app.listsJson, [])
    readonly property var outboxItems: parse(app.outboxJson, [])
    readonly property var activeList: {
        for (var i = 0; i < mailingLists.length; i++)
            if (mailingLists[i].list_id === root.listFilter) return mailingLists[i];
        return null;
    }
    property string listFilter: ""
    readonly property var themes: parse(app.themesJson, [])
    readonly property var lightThemes: themes.filter(function(t) { return t.light === true; })
    readonly property var darkThemes: themes.filter(function(t) { return t.light !== true; })
    property int currentIndex: 0
    property bool selectionMode: false
    property var selectedIds: []
    property int lastUnread: -1
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
    onUnreadTotalChanged: {
        if (root.lastUnread >= 0 && root.unreadTotal > root.lastUnread)
            app.notify("New mail", (root.unreadTotal - root.lastUnread) + " new message(s) in Aster");
        root.lastUnread = root.unreadTotal;
    }

    function toggleSelected(id) {
        var next = root.selectedIds.slice();
        var index = next.indexOf(id);
        if (index >= 0) next.splice(index, 1);
        else next.push(id);
        root.selectedIds = next;
    }

    function clearSelection() {
        root.selectedIds = [];
        root.selectionMode = false;
    }

    function batch(action) {
        if (root.selectedIds.length === 0) return;
        app.batchAction(action, JSON.stringify(root.selectedIds));
        root.clearSelection();
    }
    onIdentitiesChanged: {
        if (root.activeIdentity.length === 0)
            root.activeIdentity = root.defaultIdentityAddress();
    }
    onComposeOpenChanged: {
        if (composeOpen && root.composeFrom.length === 0)
            root.composeFrom = root.defaultIdentityAddress();
    }

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
            if (root.filterFrom.length > 0 && (m.sender || "").toLowerCase().indexOf(root.filterFrom.toLowerCase()) === -1) continue;
            if (root.filterSubject.length > 0 && (m.subject || "").toLowerCase().indexOf(root.filterSubject.toLowerCase()) === -1) continue;
            if (root.filterBody.length > 0 && (m.preview || "").toLowerCase().indexOf(root.filterBody.toLowerCase()) === -1) continue;
            if (root.filterStarred && !m.is_starred) continue;
            if (root.filterAttachmentsOnly && !m.has_attachments) continue;
            if (root.categoryFilter.length > 0 && m.category !== root.categoryFilter) continue;
            if (root.filterSince.length > 0) {
                var since = new Date(root.filterSince);
                if (!isNaN(since.getTime()) && m.date && new Date(m.date) < since) continue;
            }
            if (root.filterUntil.length > 0) {
                var until = new Date(root.filterUntil);
                if (!isNaN(until.getTime())) {
                    until.setDate(until.getDate() + 1);
                    if (m.date && new Date(m.date) >= until) continue;
                }
            }
            out.push(m);
        }
        return out;
    }

    readonly property var messageCategories: {
        var out = [];
        for (var i = 0; i < root.messages.length; i++) {
            var category = root.messages[i].category;
            if (category && out.indexOf(category) === -1) out.push(category);
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
        root.threadListOpen = false;
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

    function selectThreadMessage(id) {
        root.threadListOpen = false;
        app.selectMessage(id);
    }

    function stepThread(delta) {
        var next = root.threadIndex + delta;
        if (next < 0 || next >= root.threadOrdered.length) return;
        root.selectThreadMessage(root.threadOrdered[next].id);
    }

    function defaultIdentityAddress() {
        if (root.activeIdentity.length > 0) return root.activeIdentity;
        for (var i = 0; i < root.identities.length; i++)
            if (root.identities[i].is_default) return root.identities[i].address;
        return root.identities.length > 0 ? root.identities[0].address : "";
    }

    function localPath(url) {
        var value = url.toString();
        if (value.indexOf("file://") === 0) value = value.substring(7);
        return decodeURIComponent(value);
    }

    function useIdentity(address) {
        root.activeIdentity = address;
        root.composeFrom = address;
        identityPopup.close();
    }

    function applySavedSearch(search) {
        root.query = search.query || "";
        root.filterFrom = search.from || "";
        root.filterSubject = search.subject || "";
        root.filterBody = search.body || "";
        root.filterSince = search.since || "";
        root.filterUntil = search.until || "";
        root.filterStarred = search.starred === true;
        root.filterAttachmentsOnly = search.attachments === true;
        if (search.list_id) root.listFilter = search.list_id;
    }

    function saveCurrentSearch(name) {
        if (!name || name.trim().length === 0) return;
        var existing = root.settings.mail && root.settings.mail.saved_searches
            ? root.settings.mail.saved_searches.slice() : [];
        existing.push({
            name: name.trim(),
            query: root.query,
            from: root.filterFrom,
            subject: root.filterSubject,
            body: root.filterBody,
            since: root.filterSince,
            until: root.filterUntil,
            starred: root.filterStarred,
            attachments: root.filterAttachmentsOnly,
            list_id: root.listFilter
        });
        app.applySettings(JSON.stringify({ mail: { saved_searches: existing } }));
    }

    function clearSearchFilters() {
        root.filterFrom = "";
        root.filterSubject = "";
        root.filterBody = "";
        root.filterSince = "";
        root.filterUntil = "";
        root.filterStarred = false;
        root.filterAttachmentsOnly = false;
        root.categoryFilter = "";
    }

    function resetCompose() {
        composeTo.text = "";
        composeCc.text = "";
        composeBcc.text = "";
        composeSubject.text = "";
        composeBody.text = "";
        root.composeAttachments = [];
        root.composeFrom = root.defaultIdentityAddress();
    }

    function openCompose() {
        root.resetCompose();
        root.composeOpen = true;
    }

    function replySubject(subject) {
        var value = subject || "";
        return /^re\s*:/i.test(value) ? value : "Re: " + value;
    }

    function openReply() {
        root.resetCompose();
        composeTo.text = root.detail.sender || "";
        composeSubject.text = root.replySubject(root.detail.subject);
        var quoted = (root.detail.body || "").trim();
        if (quoted.length > 0) {
            var lines = quoted.split("\n").map(function(line) { return "> " + line; }).join("\n");
            composeBody.text = "\n\nOn " + (root.formatWhen(root.detail.date) || "an earlier date")
                + ", " + (root.detail.sender || "the sender") + " wrote:\n" + lines;
        }
        root.composeOpen = true;
    }

    function buildSendPayload() {
        return {
            from: root.composeFrom,
            to: composeTo.text,
            cc: composeCc.text,
            bcc: composeBcc.text,
            subject: composeSubject.text,
            body: composeBody.text,
            attachments: root.composeAttachments.slice()
        };
    }

    function sendWithUndo() {
        root.pendingSend = root.buildSendPayload();
        root.sendUndoVisible = true;
        sendUndoTimer.restart();
    }

    function performPendingSend() {
        if (!root.pendingSend) return;
        var payload = root.pendingSend;
        root.pendingSend = null;
        root.sendUndoVisible = false;
        if (root.editingDraftId.length > 0) {
            app.deleteDraft(root.editingDraftId);
            root.editingDraftId = "";
        }
        app.sendMessage(payload.from, payload.to, payload.cc, payload.bcc, payload.subject, payload.body, JSON.stringify(payload.attachments));
        root.composeOpen = false;
    }

    function cancelPendingSend() {
        root.pendingSend = null;
        root.sendUndoVisible = false;
    }

    function scheduleCurrentSend() {
        var when = new Date(scheduleAtField.text);
        if (isNaN(when.getTime())) return;
        var payload = root.buildSendPayload();
        payload.at = when.toISOString();
        var list = root.scheduledSends.slice();
        list.push(payload);
        app.applySettings(JSON.stringify({ compose: { scheduled: list } }));
        root.composeOpen = false;
    }

    function checkScheduledSends() {
        if (!root.scheduledSends || root.scheduledSends.length === 0) return;
        var now = Date.now();
        var keep = [];
        var due = [];
        for (var i = 0; i < root.scheduledSends.length; i++) {
            var item = root.scheduledSends[i];
            var at = new Date(item.at).getTime();
            if (!isNaN(at) && at <= now) due.push(item);
            else keep.push(item);
        }
        if (due.length === 0) return;
        app.applySettings(JSON.stringify({ compose: { scheduled: keep } }));
        for (var j = 0; j < due.length; j++) {
            var payload = due[j];
            app.sendMessage(payload.from, payload.to, payload.cc, payload.bcc, payload.subject, payload.body, JSON.stringify(payload.attachments || []));
        }
        app.notify("Scheduled mail sent", due.length + " message(s) sent");
    }

    function openDraft() {
        if (!root.detail || !root.detail.id) return;
        root.editingDraftId = root.detail.id;
        root.composeFrom = root.detail.recipients && root.identities.length > 0 ? root.identities[0].address : root.composeFrom;
        composeTo.text = root.detail.recipients || "";
        composeCc.text = "";
        composeBcc.text = "";
        composeSubject.text = root.detail.subject || "";
        composeBody.text = root.detail.body || "";
        root.composeAttachments = [];
        root.composeOpen = true;
    }

    function openForward() {
        root.resetCompose();
        composeSubject.text = /^fwd\s*:/i.test(root.detail.subject || "")
            ? (root.detail.subject || "")
            : "Fwd: " + (root.detail.subject || "");
        var body = (root.detail.body || "").trim();
        composeBody.text = "\n\n---------- Forwarded message ----------\n"
            + "From: " + (root.detail.sender || "") + "\n"
            + "Date: " + (root.detail.date || "") + "\n"
            + "Subject: " + (root.detail.subject || "") + "\n\n"
            + body;
        root.composeOpen = true;
    }

    function aliasIsSelected(address) {
        return root.aliasSelection.indexOf(address) !== -1;
    }

    function toggleAlias(address) {
        var out = root.aliasSelection.slice();
        var at = out.indexOf(address);
        if (at >= 0) out.splice(at, 1);
        else out.push(address);
        root.aliasSelection = out;
    }

    function routeSelectedAliases() {
        var mailbox = (newMailboxName.text || "").trim();
        if (mailbox.length === 0 || root.aliasSelection.length === 0) return;
        for (var i = 0; i < root.aliasSelection.length; i++)
            app.routeAlias(root.aliasSelection[i], mailbox);
        root.aliasSelection = [];
    }

    Shortcut { sequence: "j"; onActivated: root.move(1) }
    Shortcut { sequence: "k"; onActivated: root.move(-1) }
    Shortcut {
        sequence: "["
        enabled: root.readerOpen && root.detail.id !== "" && !root.threadListOpen
                 && !root.composeOpen && !root.settingsOpen && !searchInput.activeFocus
        onActivated: root.stepThread(-1)
    }
    Shortcut {
        sequence: "]"
        enabled: root.readerOpen && root.detail.id !== "" && !root.threadListOpen
                 && !root.composeOpen && !root.settingsOpen && !searchInput.activeFocus
        onActivated: root.stepThread(1)
    }
    Shortcut { sequences: ["Ctrl+N", "c"]; onActivated: root.openCompose() }
    Shortcut {
        sequence: "Escape"
        onActivated: {
            if (root.deleteConfirmOpen) {
                app.resolveDelete(root.pendingDeleteToken, false);
                root.deleteConfirmOpen = false;
            } else if (root.composeOpen) {
                root.composeOpen = false;
            } else if (root.settingsOpen) {
                root.settingsOpen = false;
            } else if (identityPopup.opened) {
                identityPopup.close();
            } else if (searchFilterPopup.opened) {
                searchFilterPopup.close();
            } else if (outboxPopup.opened) {
                outboxPopup.close();
            } else if (root.threadListOpen) {
                root.threadListOpen = false;
            }
        }
    }
    Shortcut { sequence: "/"; onActivated: searchInput.forceActiveFocus() }
    Shortcut { sequence: "Ctrl+,"; onActivated: root.openSettings("appearance") }

    FileDialog {
        id: attachDialog
        title: "Attach files"
        fileMode: FileDialog.OpenFiles
        onAccepted: {
            var added = [];
            for (var i = 0; i < selectedFiles.length; i++)
                added.push(root.localPath(selectedFiles[i]));
            root.composeAttachments = root.composeAttachments.concat(added);
        }
    }

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
        radius: n("radius-control", 2)
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
        radius: n("radius-control", 2)
        color: active ? c("selected", "#3d2e323a") : (themeHover.hovered ? c("hover", "#2e2e323a") : "transparent")
        HoverHandler { id: themeHover }
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: n("space-2", 8)
            anchors.rightMargin: n("space-2", 8)
            spacing: n("space-2", 8)
            Icon {
                name: "check"
                size: n("icon-small", 16)
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
                font.pixelSize: n("font-size-micro", 11)
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
        radius: n("radius-control", 2)
        color: checked ? c("accent-button", "#d98e5c") : c("surface-strong", "#2e323a")
        border.width: checked ? 0 : n("border-width", 1)
        border.color: c("border-strong", "#292e323a")
        Rectangle {
            width: 18
            height: 18
            radius: n("radius-small", 1)
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
        radius: n("radius-control", 2)
        color: c("input", "#221b1e23")
        border.width: field.activeFocus ? n("border-width-focus", 1.5) : n("border-width", 1)
        border.color: field.activeFocus ? c("accent", "#d98e5c") : c("border", "#171b1e23")
        TextInput {
            id: field
            anchors.fill: parent
            anchors.leftMargin: n("space-3", 12)
            anchors.rightMargin: n("space-3", 12)
            topPadding: 0
            bottomPadding: 0
            verticalAlignment: TextInput.AlignVCenter
            color: c("text", "#ece7de")
            selectionColor: c("accent", "#d98e5c")
            selectedTextColor: c("accent-ink", "#171717")
            font.family: c("font-family", "Inter")
            font.pixelSize: n("font-size-small", 12)
            clip: true
            onTextChanged: fieldRoot.edited(text)
            Text {
                anchors.fill: parent
                verticalAlignment: Text.AlignVCenter
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
        radius: n("radius-control", 2)
        color: active ? c("selected", "#3d2e323a") : (navHover.hovered ? c("hover", "#2e2e323a") : "transparent")
        activeFocusOnTab: true
        Accessible.role: Accessible.Button
        Accessible.name: label
        Accessible.onPressAction: root.settingsTab = tab
        Keys.onSpacePressed: root.settingsTab = tab
        Keys.onReturnPressed: root.settingsTab = tab
        Keys.onEnterPressed: root.settingsTab = tab
        border.width: activeFocus ? n("border-width-focus", 1.5) : 0
        border.color: c("focus", "#d98e5c")
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
        property var action: null
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
        radius: n("radius-control", 2)
        color: active ? c("selected", "#3d2e323a") : (folderHover.hovered ? c("hover", "#2e2e323a") : "transparent")
        Behavior on color { ColorAnimation { duration: n("motion-fast", 110) } }
        HoverHandler { id: folderHover }
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: n("space-2", 8) + 8
            anchors.rightMargin: n("space-3", 12)
            spacing: n("space-2", 8)
            Icon {
                name: root.folderIcon(folderRow.folder.folder_label)
                size: n("icon-body", 16)
                color: folderRow.active ? c("accent-text", "#e7a97f") : c("muted", "#a8ece7de")
                Layout.alignment: Qt.AlignVCenter
            }
            Text {
                text: root.folderLabel(folderRow.folder.name || folderRow.folder.folder_label)
                color: c("text", "#ece7de")
                font.family: c("font-family", "Inter")
                font.pixelSize: n("font-size-body", 14)
                font.weight: folderRow.active ? n("font-weight-semibold", 600) : n("font-weight-regular", 400)
                elide: Text.ElideRight
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignVCenter
            }
            Text {
                text: folderRow.folder.unread > 0 ? folderRow.folder.unread : ""
                color: folderRow.active ? c("accent-text", "#e7a97f") : c("muted", "#a8ece7de")
                font.family: c("font-family", "Inter")
                font.pixelSize: n("font-size-small", 12)
                font.weight: n("font-weight-medium", 500)
                horizontalAlignment: Text.AlignRight
                Layout.preferredWidth: 22
                Layout.alignment: Qt.AlignVCenter
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
        opacity: enabled ? 1 : 0.35
        radius: n("radius-control", 2)
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

    component MailScrollBar: ScrollBar {
        id: scrollBar
        policy: ScrollBar.AsNeeded
        implicitWidth: 10
        padding: 3
        contentItem: Rectangle {
            implicitWidth: 4
            implicitHeight: 4
            radius: 2
            color: scrollBar.pressed ? c("accent", "#d98e5c")
                 : scrollBar.hovered ? c("text-2", "#dbebe7de")
                 : c("muted", "#a8ece7de")
            opacity: scrollBar.size < 1 ? 1 : 0
            Behavior on color { ColorAnimation { duration: n("motion-fast", 110) } }
            Behavior on opacity { NumberAnimation { duration: n("motion-fast", 110) } }
        }
        background: Item {}
    }

    component SelectionBox: Rectangle {
        id: box
        property bool checked: false
        property string label: "Select"
        signal toggled()
        implicitWidth: 18
        implicitHeight: 18
        radius: n("radius-small", 1)
        color: box.checked ? c("accent", "#d98e5c") : (box.activeFocus ? c("hover", "#2e2e323a") : "transparent")
        border.width: box.activeFocus ? n("border-width-focus", 1.5) : n("border-width", 1)
        border.color: box.activeFocus ? c("focus", "#d98e5c") : (box.checked ? c("accent", "#d98e5c") : c("border-strong", "#292e323a"))
        activeFocusOnTab: true
        Accessible.role: Accessible.CheckBox
        Accessible.name: box.label
        Accessible.checkable: true
        Accessible.checked: box.checked
        Accessible.onPressAction: box.toggled()
        Keys.onSpacePressed: box.toggled()
        Keys.onReturnPressed: box.toggled()
        Keys.onEnterPressed: box.toggled()
        Icon {
            anchors.centerIn: parent
            name: "check"
            size: 12
            color: c("accent-ink", "#171717")
            visible: box.checked
        }
        MouseArea {
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onClicked: box.toggled()
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
                Rectangle {
                    visible: root.selectionMode
                    width: 18; height: 18
                    radius: n("radius-micro", 4)
                    color: root.selectedIds.indexOf(row.item.id) >= 0 ? c("accent-button", "#d98e5c") : "transparent"
                    border.width: n("border-width", 1)
                    border.color: c("border-strong", "#292e323a")
                    Icon { anchors.centerIn: parent; name: "check"; size: 12; color: c("accent-ink", "#171717"); visible: root.selectedIds.indexOf(row.item.id) >= 0 }
                    MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: root.toggleSelected(row.item.id) }
                }
                Text {
                    text: row.item && row.item.sender ? row.item.sender : "Unknown sender"
                    color: c("text", "#ece7de")
                    font.family: c("font-family", "Inter")
                    font.pixelSize: n("font-size-body", 14)
                    font.weight: row.item && !row.item.is_read ? n("font-weight-unread", 600) : n("font-weight-regular", 400)
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
                Icon {
                    name: "pin"
                    size: 12
                    color: c("accent-text", "#e7a97f")
                    visible: row.item && row.item.is_pinned
                    Layout.alignment: Qt.AlignVCenter
                }
                Icon {
                    name: "star"
                    size: 12
                    color: c("accent-text", "#e7a97f")
                    visible: row.item && row.item.is_starred
                    Layout.alignment: Qt.AlignVCenter
                }
                Text {
                    visible: row.item && row.item.thread_count > 1
                    text: row.item ? row.item.thread_count : ""
                    color: c("accent-text", "#e7a97f")
                    font.family: c("font-family", "Inter")
                    font.pixelSize: n("font-size-micro", 11)
                    font.weight: n("font-weight-semibold", 600)
                }
                Text {
                    text: row.item ? root.formatWhen(row.item.date) : ""
                    color: c("muted", "#a8ece7de")
                    font.family: c("font-family", "Inter")
                    font.pixelSize: n("font-size-caption", 11)
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
                    font.pixelSize: n("font-size-micro", 11)
                    elide: Text.ElideRight
                    Layout.maximumWidth: 120
                }
                Icon {
                    name: "attach"
                    size: n("icon-small", 16)
                    color: c("muted", "#a8ece7de")
                    visible: row.item && row.item.has_attachments
                }
            }
        }
        MouseArea {
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onClicked: {
                if (root.selectionMode) {
                    root.toggleSelected(row.item.id);
                    return;
                }
                root.currentIndex = index;
                root.readerOpen = true;
                root.threadListOpen = false;
                app.selectMessage(row.item.id)
            }
        }
    }

    component ThreadMessageRow: Rectangle {
        id: threadRow
        property var item
        readonly property bool selected: item && root.detail.id === item.id
        readonly property bool focused: ListView.isCurrentItem && ListView.view && ListView.view.activeFocus
        implicitHeight: 46
        radius: n("radius-small", 1)
        color: selected ? c("selected", "#3d2e323a") : (threadHover.hovered ? c("hover", "#2e2e323a") : "transparent")
        border.width: focused ? 1 : 0
        border.color: c("accent", "#d98e5c")
        Behavior on color { ColorAnimation { duration: n("motion-fast", 110) } }
        Accessible.role: Accessible.ListItem
        Accessible.name: (item ? (item.sender || "Unknown sender") + ". " + (item.subject || item.preview || "") : "")
        Accessible.selected: selected
        HoverHandler { id: threadHover }
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: n("space-3", 12)
            anchors.rightMargin: n("space-3", 12)
            spacing: n("space-2", 8)
            Rectangle {
                width: 6; height: 6; radius: 3
                color: c("accent", "#d98e5c")
                visible: threadRow.item && !threadRow.item.is_read
                Layout.alignment: Qt.AlignVCenter
            }
            ColumnLayout {
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignVCenter
                spacing: 1
                Text {
                    text: threadRow.item ? (threadRow.item.sender || "Unknown sender") : ""
                    color: c("text", "#ece7de")
                    font.family: c("font-family", "Inter")
                    font.pixelSize: n("font-size-small", 12)
                    font.weight: threadRow.item && !threadRow.item.is_read ? n("font-weight-semibold", 600) : n("font-weight-medium", 500)
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
                Text {
                    text: threadRow.item ? (threadRow.item.subject || threadRow.item.preview || "(no subject)") : ""
                    color: c("muted", "#a8ece7de")
                    font.family: c("font-family", "Inter")
                    font.pixelSize: n("font-size-micro", 11)
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
            }
            Text {
                text: threadRow.item ? root.formatWhen(threadRow.item.date) : ""
                color: c("muted", "#a8ece7de")
                font.family: c("font-family", "Inter")
                font.pixelSize: n("font-size-caption", 11)
                Layout.alignment: Qt.AlignVCenter
            }
        }
        MouseArea {
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onClicked: root.selectThreadMessage(threadRow.item.id)
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
                    radius: n("radius-control", 2)
                    color: c("accent-button", "#d98e5c")
                    Icon {
                        anchors.centerIn: parent
                        name: "send"
                        size: n("icon-body", 16)
                        color: c("accent-ink", "#171717")
                    }
                }
                Column {
                    spacing: 0
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
                        font.pixelSize: n("font-size-micro", 11)
                    }
                }

                Rectangle {
                    Layout.preferredWidth: Math.min(360, root.width * 0.3)
                    Layout.preferredHeight: n("control-height", 36)
                    radius: n("radius-control", 2)
                    color: c("input", "#221b1e23")
                    border.width: searchInput.activeFocus ? n("border-width-focus", 1.5) : n("border-width", 1)
                    border.color: searchInput.activeFocus ? c("accent", "#d98e5c") : c("border", "#171b1e23")
                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: n("space-3", 12)
                        anchors.rightMargin: n("space-3", 12)
                        spacing: n("space-2", 8)
                        Icon { name: "search"; size: n("icon-small", 16); color: c("muted", "#a8ece7de") }
                        TextInput {
                            id: searchInput
                            Layout.fillWidth: true
                            color: c("text", "#ece7de")
                            selectionColor: c("accent", "#d98e5c")
                            selectedTextColor: c("accent-ink", "#171717")
                            font.family: c("font-family", "Inter")
                            font.pixelSize: n("font-size-small", 12)
                            topPadding: 0
                            bottomPadding: 0
                            verticalAlignment: TextInput.AlignVCenter
                            onTextChanged: root.query = text
                            Text {
                                anchors.fill: parent
                                verticalAlignment: Text.AlignVCenter
                                visible: searchInput.text.length === 0
                                text: "Search mail"
                                color: c("muted", "#a8ece7de")
                                font.family: c("font-family", "Inter")
                                font.pixelSize: n("font-size-small", 12)
                            }
                        }
                        Rectangle {
                            implicitWidth: searchHint.implicitWidth + n("space-2", 8)
                            implicitHeight: 20
                            radius: n("radius-keycap", 3)
                            color: "transparent"
                            border.width: n("border-width", 1)
                            border.color: c("border-strong", "#292e323a")
                            visible: searchInput.text.length === 0
                            Text {
                                id: searchHint
                                anchors.centerIn: parent
                                text: "Ctrl K"
                                color: c("muted", "#a8ece7de")
                                font.family: c("font-mono", "JetBrains Mono")
                                font.pixelSize: n("font-size-micro", 11)
                            }
                        }
                    }
                }

                IconButton {
                    id: searchFilterButton
                    icon: "filter"
                    label: "Search filters"
                    onClicked: searchFilterPopup.open()
                }
                Item { Layout.fillWidth: true }

                RowLayout {
                    spacing: n("space-2", 8)
                    Rectangle {
                        width: 8; height: 8
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
                        radius: n("radius-control", 2)
                        color: composeHover.hovered ? Qt.lighter(c("accent-button", "#d98e5c"), 1.06) : c("accent-button", "#d98e5c")
                        HoverHandler { id: composeHover }
                        RowLayout {
                            id: composeRow
                            anchors.centerIn: parent
                            spacing: n("space-2", 8)
                            Icon {
                                name: "plus"
                                size: n("icon-small", 16)
                                color: c("accent-ink", "#171717")
                                Layout.alignment: Qt.AlignVCenter
                            }
                            Text {
                                text: "Compose"
                                color: c("accent-ink", "#171717")
                                font.family: c("font-family", "Inter")
                                font.pixelSize: n("font-size-small", 12)
                                font.weight: n("font-weight-semibold", 600)
                                Layout.alignment: Qt.AlignVCenter
                            }
                        }
                        MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: root.openCompose() }
                    }
                }
            }
        }

        SplitView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            orientation: Qt.Horizontal
            handle: Item {
                id: splitHandle
                implicitWidth: 10
                implicitHeight: 10
                readonly property bool hot: SplitHandle.hovered || SplitHandle.pressed
                HoverHandler { cursorShape: Qt.SplitHCursor }
                Rectangle {
                    anchors.centerIn: parent
                    width: splitHandle.hot ? 2 : 1
                    height: parent.height
                    color: splitHandle.hot ? c("accent", "#d98e5c") : c("border", "#171b1e23")
                    Behavior on width { NumberAnimation { duration: n("motion-fast", 110) } }
                    Behavior on color { ColorAnimation { duration: n("motion-fast", 110) } }
                }
            }

            Rectangle {
                SplitView.preferredWidth: 240
                SplitView.minimumWidth: 208
                visible: root.sidebarVisible
                color: c("sidebar", "#1a1d22")
                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: n("space-3", 12)
                    spacing: n("space-1", 4)

                    Rectangle {
                        id: accountCard
                        Layout.fillWidth: true
                        Layout.preferredHeight: 56
                        radius: n("radius-card", 6)
                        color: accountHover.hovered ? c("surface-strong", "#2e323a") : c("card", "#23262c")
                        activeFocusOnTab: true
                        Accessible.role: Accessible.Button
                        Accessible.name: "Switch alias, currently " + (root.activeIdentity.length > 0 ? root.activeIdentity : (app.accountEmail || "Aster Mail"))
                        Accessible.onPressAction: identityPopup.open()
                        Keys.onSpacePressed: identityPopup.open()
                        Keys.onReturnPressed: identityPopup.open()
                        Keys.onEnterPressed: identityPopup.open()
                        border.width: accountCard.activeFocus ? n("border-width-focus", 1.5) : 0
                        border.color: c("focus", "#d98e5c")
                        HoverHandler { id: accountHover }
                        RowLayout {
                            anchors.fill: parent
                            anchors.margins: n("space-3", 12)
                            spacing: n("space-2", 8)
                            Rectangle {
                                width: 32; height: 32
                                radius: 15
                                color: c("accent-button", "#d98e5c")
                                Text {
                                    anchors.centerIn: parent
                                    text: ((root.activeIdentity.length > 0 ? root.activeIdentity : (app.accountEmail || "A"))).substring(0, 1).toUpperCase()
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
                                    text: root.activeIdentity.length > 0 ? root.activeIdentity : (app.accountEmail || "Aster Mail")
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
                                    font.pixelSize: n("font-size-micro", 11)
                                }
                            }
                            Icon {
                                name: "chevronDown"
                                size: n("icon-small", 16)
                                color: c("muted", "#a8ece7de")
                                Layout.alignment: Qt.AlignVCenter
                            }
                        }
                        MouseArea {
                            anchors.fill: parent
                            cursorShape: Qt.PointingHandCursor
                            onClicked: identityPopup.open()
                        }
                    }

                    Text {
                        text: "Mailboxes"
                        color: c("muted", "#a8ece7de")
                        font.family: c("font-family", "Inter")
                        font.pixelSize: n("font-size-micro", 11)
                        font.weight: n("font-weight-medium", 500)
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

                    Rectangle {
                        id: customMailboxButton
                        Layout.fillWidth: true
                        Layout.preferredHeight: 32
                        radius: n("radius-control", 2)
                        color: customAddHover.hovered ? c("hover", "#2e2e323a") : "transparent"
                        border.width: customMailboxButton.activeFocus ? n("border-width-focus", 1.5) : 0
                        border.color: c("focus", "#d98e5c")
                        activeFocusOnTab: true
                        Accessible.role: Accessible.Button
                        Accessible.name: "Create or manage custom mailboxes"
                        Accessible.onPressAction: root.openSettings("aliases")
                        Keys.onSpacePressed: root.openSettings("aliases")
                        Keys.onReturnPressed: root.openSettings("aliases")
                        Keys.onEnterPressed: root.openSettings("aliases")
                        HoverHandler { id: customAddHover }
                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: n("space-2", 8)
                            anchors.rightMargin: n("space-2", 8)
                            spacing: n("space-2", 8)
                            Text {
                                text: "Custom"
                                color: c("muted", "#a8ece7de")
                                font.family: c("font-family", "Inter")
                                font.pixelSize: n("font-size-micro", 11)
                                font.weight: n("font-weight-medium", 500)
                                Layout.fillWidth: true
                                Layout.alignment: Qt.AlignVCenter
                            }
                            Icon {
                                name: "plus"
                                size: n("icon-small", 16)
                                color: c("accent-text", "#e7a97f")
                                Layout.alignment: Qt.AlignVCenter
                            }
                        }
                        MouseArea {
                            anchors.fill: parent
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.openSettings("aliases")
                        }
                    }

                    Repeater {
                        model: root.customFolders
                        delegate: FolderRow {
                            Layout.fillWidth: true
                            folder: modelData
                        }
                    }

                    Text {
                        text: "Mailing lists"
                        color: c("muted", "#a8ece7de")
                        font.family: c("font-family", "Inter")
                        font.pixelSize: n("font-size-micro", 11)
                        font.weight: n("font-weight-medium", 500)
                        Layout.topMargin: n("space-3", 12)
                        Layout.leftMargin: n("space-2", 8)
                    }
                    Rectangle {
                        Layout.fillWidth: true
                        implicitHeight: 28
                        radius: n("radius-control", 2)
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
                            radius: n("radius-control", 2)
                            color: root.listFilter === modelData.list_id ? c("selected", "#3d2e323a") : (listHover.hovered ? c("hover", "#2e2e323a") : "transparent")
                            HoverHandler { id: listHover }
                            RowLayout {
                                anchors.fill: parent
                                anchors.leftMargin: n("space-2", 8) + 4
                                anchors.rightMargin: n("space-2", 8)
                                spacing: n("space-2", 8)
                                Icon { name: "tag"; size: n("icon-small", 16); color: c("muted", "#a8ece7de") }
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
                                    font.pixelSize: n("font-size-micro", 11)
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
                        font.pixelSize: n("font-size-micro", 11)
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                        Layout.leftMargin: n("space-2", 8)
                    }

                    Text {
                        visible: root.customFolders.length === 0
                        text: "Create a mailbox to route mail here."
                        color: c("muted", "#a8ece7de")
                        font.family: c("font-family", "Inter")
                        font.pixelSize: n("font-size-micro", 11)
                        wrapMode: Text.WordWrap
                        Layout.fillWidth: true
                        Layout.leftMargin: n("space-2", 8)
                    }

                    Text {
                        text: "SAVED SEARCHES"
                        color: c("muted", "#a8ece7de")
                        font.family: c("font-family", "Inter")
                        font.pixelSize: n("font-size-micro", 9)
                        font.letterSpacing: 1
                        Layout.topMargin: n("space-3", 12)
                        Layout.leftMargin: n("space-2", 8)
                    }
                    Repeater {
                        model: root.settings.mail && root.settings.mail.saved_searches ? root.settings.mail.saved_searches : []
                        delegate: Rectangle {
                            Layout.fillWidth: true
                            implicitHeight: 28
                            radius: n("radius-control", 8)
                            color: savedHover.hovered ? c("hover", "#2e2e323a") : "transparent"
                            HoverHandler { id: savedHover }
                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                anchors.left: parent.left
                                anchors.leftMargin: n("space-3", 12)
                                anchors.right: parent.right
                                anchors.rightMargin: n("space-3", 12)
                                text: modelData.name
                                color: c("text-2", "#dbebe7de")
                                font.family: c("font-family", "Inter")
                                font.pixelSize: n("font-size-small", 12)
                                elide: Text.ElideRight
                            }
                            MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: root.applySavedSearch(modelData) }
                        }
                    }
                    Text {
                        visible: !(root.settings.mail && root.settings.mail.saved_searches && root.settings.mail.saved_searches.length > 0)
                        text: "Save a search from the filter dialog."
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
                        Icon { name: "rule"; size: n("icon-small", 16); color: c("muted", "#a8ece7de") }
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
                            font.pixelSize: n("font-size-micro", 11)
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
                        Layout.preferredHeight: 56
                        color: "transparent"
                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: n("space-4", 16)
                            anchors.rightMargin: n("space-4", 16)
                            spacing: n("space-2", 8)
                            ColumnLayout {
                                spacing: 1
                                Text {
                                    text: root.listFilter && root.activeList ? root.activeList.name : root.folderLabel(app.folderId)
                                    color: c("text", "#ece7de")
                                    font.family: c("font-family", "Inter")
                                    font.pixelSize: n("font-size-title", 16)
                                    font.weight: n("font-weight-semibold", 600)
                                    elide: Text.ElideRight
                                }
                                Text {
                                    text: root.listFilter && root.activeList
                                          ? root.activeList.list_id + " · " + root.visibleMessages.length + " messages"
                                          : root.visibleMessages.length + " messages"
                                    color: c("muted", "#a8ece7de")
                                    font.family: c("font-family", "Inter")
                                    font.pixelSize: n("font-size-micro", 11)
                                    elide: Text.ElideRight
                                    Layout.fillWidth: true
                                }
                            }
                            Item { Layout.fillWidth: true }
                            LinkText {
                                visible: root.listFilter.length > 0
                                text: "Archive all"
                                action: function() { app.archiveMailingList(root.listFilter) }
                            }
                            LinkText {
                                visible: root.listFilter.length > 0
                                text: "All mail"
                                action: function() { root.listFilter = "" }
                            }
                            LinkText {
                                visible: app.folderId === "trash" || app.folderId === "spam"
                                text: "Empty"
                                action: function() { root.emptyConfirmOpen = true }
                            }
                            Rectangle {
                                implicitWidth: 28; implicitHeight: 28
                                radius: n("radius-control", 2)
                                color: root.unreadOnly ? c("selected", "#3d2e323a") : "transparent"
                                Icon { anchors.centerIn: parent; name: "inbox"; size: n("icon-small", 16); color: root.unreadOnly ? c("accent-text", "#e7a97f") : c("muted", "#a8ece7de") }
                                MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: root.unreadOnly = !root.unreadOnly }
                            }
                            IconButton {
                                icon: "check"
                                label: root.selectionMode ? "Cancel selection" : "Select messages"
                                onClicked: {
                                    root.selectionMode = !root.selectionMode;
                                    if (!root.selectionMode) root.selectedIds = [];
                                }
                            }
                            IconButton { icon: "refresh"; label: "Refresh"; onClicked: app.refreshMail() }
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: root.messageCategories.length > 0 ? 34 : 0
                        visible: root.messageCategories.length > 0
                        color: "transparent"
                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: n("space-4", 16)
                            anchors.rightMargin: n("space-4", 16)
                            spacing: n("space-2", 8)
                            ModeChip { label: "All"; active: root.categoryFilter === ""; onClicked: root.categoryFilter = "" }
                            Repeater {
                                model: root.messageCategories
                                delegate: ModeChip { label: modelData; active: root.categoryFilter === modelData; onClicked: root.categoryFilter = modelData }
                            }
                            Item { Layout.fillWidth: true }
                        }
                    }
                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: root.selectionMode ? 36 : 0
                        visible: root.selectionMode
                        color: c("surface", "#1b1e23")
                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: n("space-4", 16)
                            anchors.rightMargin: n("space-4", 16)
                            spacing: n("space-3", 12)
                            Text { text: root.selectedIds.length + " selected"; color: c("text", "#ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-small", 12); Layout.fillWidth: true }
                            LinkText { text: "Read"; action: function() { root.batch("mark_read") } }
                            LinkText { text: "Unread"; action: function() { root.batch("mark_unread") } }
                            LinkText { text: "Star"; action: function() { root.batch("star") } }
                            LinkText { text: "Archive"; action: function() { root.batch("archive") } }
                            LinkText { text: "Trash"; action: function() { root.batch("trash") } }
                            LinkText { text: "Clear"; action: function() { root.clearSelection() } }
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
                        visible: !app.loading && root.groupedMessages.length > 0
                        clip: true
                        model: root.groupedMessages
                        flickDeceleration: 4000
                        maximumFlickVelocity: 8000
                        delegate: MessageRow {
                            width: ListView.view.width
                            item: modelData
                        }
                        ScrollBar.vertical: MailScrollBar {}
                        WheelHandler {
                            onWheel: function(event) {
                                var step = event.pixelDelta.y !== 0
                                         ? event.pixelDelta.y
                                         : event.angleDelta.y;
                                if (step === 0) return;
                                messageList.cancelFlick();
                                messageList.contentY = Math.max(0, Math.min(messageList.contentHeight - messageList.height,
                                                                             messageList.contentY - step));
                                event.accepted = true;
                            }
                        }
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        visible: !app.loading && root.groupedMessages.length === 0
                        spacing: n("space-3", 12)
                        Item { Layout.fillHeight: true }
                        Icon { name: "inbox"; size: 32; color: c("muted", "#a8ece7de"); Layout.alignment: Qt.AlignHCenter }
                        Text {
                            text: root.query.length > 0 || root.unreadOnly ? "No matching mail" : "This mailbox is empty"
                            color: c("text-2", "#dbebe7de")
                            font.family: c("font-family", "Inter")
                            font.pixelSize: n("font-size-body", 14)
                            horizontalAlignment: Text.AlignHCenter
                            Layout.fillWidth: true
                            Layout.alignment: Qt.AlignHCenter
                        }
                        Text {
                            text: root.query.length > 0 ? "Try a different search." : "New mail will appear here."
                            color: c("muted", "#a8ece7de")
                            font.family: c("font-family", "Inter")
                            font.pixelSize: n("font-size-small", 12)
                            horizontalAlignment: Text.AlignHCenter
                            Layout.fillWidth: true
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
                        Layout.preferredHeight: 56
                        Layout.leftMargin: n("space-4", 16)
                        Layout.rightMargin: n("space-4", 16)
                        spacing: n("space-1", 4)
                        LinkText { visible: app.folderId === "drafts"; text: "Edit draft"; action: function() { root.openDraft() } }
                        LinkText {
                            visible: root.detail.list_unsubscribe && root.detail.list_unsubscribe.length > 0
                            text: "Unsubscribe"
                            action: function() {
                                var url = root.unsubscribeUrl(root.detail.list_unsubscribe);
                                if (url.length > 0) Qt.openUrlExternally(url);
                            }
                        }
                        IconButton { icon: "reply"; label: "Reply"; onClicked: root.openReply() }
                        IconButton { icon: "forward"; label: "Forward"; onClicked: root.openForward() }
                        Item { Layout.fillWidth: true }
                        IconButton {
                            icon: "pin"
                            label: root.detail.is_pinned ? "Unpin" : "Pin"
                            onClicked: app.setPinned(root.detail.id, !root.detail.is_pinned)
                        }
                        IconButton {
                            icon: "star"
                            label: root.detail.is_starred ? "Unstar" : "Star"
                            accent: root.detail.is_starred === true
                            onClicked: app.setStarred(root.detail.id, !root.detail.is_starred)
                        }
                        IconButton {
                            icon: "check"
                            label: root.detail.is_read ? "Mark unread" : "Mark read"
                            onClicked: app.setRead(root.detail.id, !root.detail.is_read)
                        }
                        IconButton {
                            icon: "archive"
                            label: "Archive"
                            onClicked: app.archiveMessage(root.detail.id)
                        }
                        IconButton {
                            icon: "trash"
                            label: "Delete"
                            danger: true
                            onClicked: app.trashMessage(root.detail.id)
                        }
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
                            Icon { name: "inbox"; size: 32; color: c("muted", "#a8ece7de"); Layout.alignment: Qt.AlignHCenter }
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
                        ScrollBar.vertical: MailScrollBar {}
                        ColumnLayout {
                            id: readerColumn
                            width: Math.min(readerFlick.width - n("space-6", 32), n("reader-width", 960))
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
                                    width: 36; height: 36
                                    radius: 18
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
                                        font.pixelSize: n("font-size-micro", 11)
                                        elide: Text.ElideRight
                                        Layout.fillWidth: true
                                    }
                                }
                                Text {
                                    visible: root.detail.list_name || root.detail.list_id
                                    text: root.detail.list_name || root.detail.list_id || ""
                                    color: c("muted", "#a8ece7de")
                                    font.family: c("font-family", "Inter")
                                    font.pixelSize: n("font-size-micro", 11)
                                    elide: Text.ElideRight
                                    Layout.maximumWidth: 180
                                }
                                RowLayout {
                                    id: trustRow
                                    spacing: n("space-2", 8)
                                    Icon {
                                        name: "shield"
                                        size: n("icon-small", 16)
                                        color: root.detail.sender_unverified || root.detail.not_end_to_end_encrypted
                                               ? c("trust-warning", "#d9b36a")
                                               : c("trust-verified", "#7fae83")
                                        Layout.alignment: Qt.AlignVCenter
                                    }
                                    Text {
                                        text: root.detail.sender_unverified ? "Unverified sender"
                                              : (root.detail.not_end_to_end_encrypted ? "Not end-to-end" : "Verified")
                                        color: root.detail.sender_unverified || root.detail.not_end_to_end_encrypted
                                               ? c("trust-warning", "#d9b36a")
                                               : c("trust-verified", "#7fae83")
                                        font.family: c("font-family", "Inter")
                                        font.pixelSize: n("font-size-micro", 11)
                                        font.weight: n("font-weight-medium", 500)
                                        Layout.alignment: Qt.AlignVCenter
                                    }
                                }
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                visible: root.threadOrdered.length > 1
                                spacing: n("space-1", 4)
                                Text {
                                    text: "Conversation · " + root.threadOrdered.length + " messages"
                                    color: c("muted", "#a8ece7de")
                                    font.family: c("font-family", "Inter")
                                    font.pixelSize: n("font-size-micro", 11)
                                    font.weight: n("font-weight-medium", 500)
                                    elide: Text.ElideRight
                                    Layout.fillWidth: true
                                }
                                IconButton {
                                    icon: "chevronLeft"
                                    label: "Previous message in conversation"
                                    enabled: root.threadIndex > 0
                                    onClicked: root.stepThread(-1)
                                }
                                Text {
                                    text: root.threadIndex >= 0 ? (root.threadIndex + 1) + " of " + root.threadOrdered.length : ""
                                    color: c("text-2", "#dbebe7de")
                                    font.family: c("font-family", "Inter")
                                    font.pixelSize: n("font-size-caption", 11)
                                    horizontalAlignment: Text.AlignHCenter
                                    Layout.minimumWidth: 42
                                }
                                IconButton {
                                    icon: "chevronRight"
                                    label: "Next message in conversation"
                                    enabled: root.threadIndex >= 0 && root.threadIndex < root.threadOrdered.length - 1
                                    onClicked: root.stepThread(1)
                                }
                                IconButton {
                                    icon: root.threadListOpen ? "chevronUp" : "chevronDown"
                                    label: root.threadListOpen ? "Hide conversation messages" : "Show conversation messages"
                                    onClicked: root.threadListOpen = !root.threadListOpen
                                }
                            }

                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: Math.min(240, root.threadOrdered.length * 48 + n("space-2", 8))
                                visible: root.threadListOpen && root.threadOrdered.length > 1
                                radius: n("radius-control", 2)
                                color: c("surface", "#1b1e23")
                                border.width: n("border-width", 1)
                                border.color: c("border", "#171b1e23")
                                clip: true
                                ListView {
                                    id: threadList
                                    anchors.fill: parent
                                    anchors.margins: n("space-1", 4)
                                    clip: true
                                    model: root.threadOrdered
                                    spacing: 2
                                    ScrollBar.vertical: MailScrollBar {}
                                    onVisibleChanged: {
                                        if (visible) {
                                            currentIndex = root.threadIndex >= 0 ? root.threadIndex : 0;
                                            forceActiveFocus();
                                        }
                                    }
                                    Keys.onUpPressed: if (currentIndex > 0) currentIndex -= 1
                                    Keys.onDownPressed: if (currentIndex < count - 1) currentIndex += 1
                                    Keys.onReturnPressed: if (currentIndex >= 0 && currentIndex < root.threadOrdered.length) root.selectThreadMessage(root.threadOrdered[currentIndex].id)
                                    Keys.onEnterPressed: if (currentIndex >= 0 && currentIndex < root.threadOrdered.length) root.selectThreadMessage(root.threadOrdered[currentIndex].id)
                                    Keys.onEscapePressed: root.threadListOpen = false
                                    delegate: ThreadMessageRow {
                                        width: ListView.view.width
                                        item: modelData
                                    }
                                }
                            }

                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: 1
                                color: c("border", "#171b1e23")
                            }

                            Rectangle {
                                id: mailCard
                                readonly property bool bodyIsHtml: root.detail.is_html && (root.detail.body_html || "").length > 0
                                readonly property string bodyText: bodyIsHtml ? (root.detail.body_html || "") : (root.detail.body || "")

                                Layout.fillWidth: true
                                Layout.preferredHeight: Math.max(340, Math.min(720, readerFlick.height - 220))
                                radius: n("radius-card", 6)
                                color: mailCard.bodyIsHtml ? c("html-mail-bg", "#ffffff") : c("surface", "#1b1e23")
                                clip: true

                                Flickable {
                                    id: mailFlick
                                    anchors.fill: parent
                                    clip: true
                                    visible: mailCard.bodyText.length > 0
                                    contentWidth: width
                                    contentHeight: mailBody.contentHeight + n("space-6", 32)
                                    boundsBehavior: Flickable.StopAtBounds
                                    ScrollBar.vertical: MailScrollBar {}

                                    TextEdit {
                                        id: mailBody
                                        x: n("space-4", 16)
                                        y: n("space-4", 16)
                                        width: mailFlick.width - n("space-6", 32)
                                        height: contentHeight
                                        text: mailCard.bodyText
                                        textFormat: mailCard.bodyIsHtml ? TextEdit.RichText : TextEdit.PlainText
                                        readOnly: true
                                        selectByMouse: true
                                        wrapMode: TextEdit.Wrap
                                        color: mailCard.bodyIsHtml ? "#1c1b19" : c("text", "#ece7de")
                                        selectionColor: c("selection", "#59d98e5c")
                                        selectedTextColor: c("text", "#ece7de")
                                        font.family: mailCard.bodyIsHtml ? c("font-mail", "Inter") : c("font-mono", "JetBrains Mono")
                                        font.pixelSize: mailCard.bodyIsHtml ? n("font-size-html-mail", 15) : n("font-size-small", 12)
                                        onLinkActivated: function(link) {
                                            if (link && (link.indexOf("http") === 0 || link.indexOf("mailto:") === 0))
                                                Qt.openUrlExternally(link);
                                        }
                                        Accessible.role: Accessible.StaticText
                                        Accessible.name: "Message body"
                                    }
                                }

                                Text {
                                    anchors.centerIn: parent
                                    visible: mailCard.bodyText.length === 0
                                    text: "This message has no readable body."
                                    color: c("muted", "#a8ece7de")
                                    font.family: c("font-family", "Inter")
                                    font.pixelSize: n("font-size-small", 12)
                                }
                            }

                            Text {
                                visible: root.detail.attachments && root.detail.attachments.length > 0
                                text: "Attachments"
                                color: c("muted", "#a8ece7de")
                                font.family: c("font-family", "Inter")
                                font.pixelSize: n("font-size-micro", 11)
                                font.weight: n("font-weight-medium", 500)
                            }
                            Flow {
                                Layout.fillWidth: true
                                spacing: n("space-2", 8)
                                Repeater {
                                    model: root.detail.attachments || []
                                    delegate: Rectangle {
                                        implicitWidth: attachRow.implicitWidth + n("space-3", 12) * 2
                                        implicitHeight: n("attachment-chip-height", 30)
                                        radius: n("radius-control", 2)
                                        color: c("card", "#23262c")
                                        border.width: n("border-width", 1)
                                        border.color: c("border", "#171b1e23")
                                        RowLayout {
                                            id: attachRow
                                            anchors.centerIn: parent
                                            spacing: n("space-2", 8)
                                            Icon {
                                                name: "attach"
                                                size: n("icon-small", 16)
                                                color: c("muted", "#a8ece7de")
                                                Layout.alignment: Qt.AlignVCenter
                                            }
                                            Text {
                                                text: modelData.name + " · " + root.formatBytes(modelData.size)
                                                color: c("text-2", "#dbebe7de")
                                                font.family: c("font-family", "Inter")
                                                font.pixelSize: n("font-size-micro", 11)
                                                Layout.alignment: Qt.AlignVCenter
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
                    font.pixelSize: n("font-size-micro", 11)
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
                LinkText {
                    visible: root.outboxItems.length > 0
                    text: "Outbox (" + root.outboxItems.length + ")"
                    action: function() { app.loadOutbox(); outboxPopup.open() }
                }
                Text {
                    text: "MCP idle"
                    color: c("muted", "#a8ece7de")
                    font.family: c("font-family", "Inter")
                    font.pixelSize: n("font-size-micro", 11)
                }
                Text {
                    text: "Theme " + app.themeId
                    color: c("muted", "#a8ece7de")
                    font.family: c("font-mono", "JetBrains Mono")
                    font.pixelSize: n("font-size-micro", 11)
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

    Timer {
        id: sendUndoTimer
        interval: 5000
        onTriggered: root.performPendingSend()
    }

    Timer {
        interval: 15000
        running: true
        repeat: true
        onTriggered: root.checkScheduledSends()
    }

    Rectangle {
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: n("statusbar-height", 30) + n("space-4", 16)
        width: toastContent.implicitWidth + n("space-4", 16) * 2
        height: 34
        radius: n("radius-control", 2)
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
            radius: n("radius-panel", 8)
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
                            font.pixelSize: n("font-size-micro", 11)
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
                            ScrollBar.vertical: MailScrollBar {}
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
                                    Text { text: "Light theme"; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 11); font.weight: n("font-weight-medium", 500); Layout.topMargin: n("space-2", 8) }
                                    Repeater {
                                        model: root.lightThemes
                                        delegate: ThemeRow { Layout.fillWidth: true; entry: modelData; active: app.themeId === modelData.id }
                                    }
                                    Text { text: "Dark theme"; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 11); font.weight: n("font-weight-medium", 500); Layout.topMargin: n("space-2", 8) }
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
                                        font.pixelSize: n("font-size-micro", 11)
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
                                            Text { text: "Index every message, not only the newest 2,000 per folder."; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 11); wrapMode: Text.WordWrap; Layout.fillWidth: true }
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
                                            Text { text: "Seconds between sync passes. Applies on next launch."; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 11); wrapMode: Text.WordWrap; Layout.fillWidth: true }
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

                                    Text { text: "SIGNATURE"; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 9); font.letterSpacing: 1; Layout.topMargin: n("space-2", 8) }
                                    Rectangle {
                                        Layout.fillWidth: true
                                        Layout.preferredHeight: 80
                                        radius: n("radius-control", 8)
                                        color: c("input", "#221b1e23")
                                        border.width: n("border-width", 1)
                                        border.color: c("border", "#171b1e23")
                                        TextArea {
                                            id: signatureField
                                            anchors.fill: parent
                                            anchors.margins: n("space-2", 8)
                                            text: root.settings.compose ? (root.settings.compose.signature || "") : ""
                                            color: c("text", "#ece7de")
                                            wrapMode: TextArea.Wrap
                                            background: null
                                            font.family: c("font-mail", "Inter")
                                            font.pixelSize: n("font-size-small", 12)
                                        }
                                    }
                                    LinkText { text: "Save signature"; action: function() { app.applySettings(JSON.stringify({ compose: { signature: signatureField.text } })) } }
                                    Text { text: "TEMPLATE"; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 9); font.letterSpacing: 1; Layout.topMargin: n("space-2", 8) }
                                    Rectangle {
                                        Layout.fillWidth: true
                                        Layout.preferredHeight: 80
                                        radius: n("radius-control", 8)
                                        color: c("input", "#221b1e23")
                                        border.width: n("border-width", 1)
                                        border.color: c("border", "#171b1e23")
                                        TextArea {
                                            id: templateField
                                            anchors.fill: parent
                                            anchors.margins: n("space-2", 8)
                                            text: root.settings.compose ? (root.settings.compose.template || "") : ""
                                            color: c("text", "#ece7de")
                                            wrapMode: TextArea.Wrap
                                            background: null
                                            font.family: c("font-mail", "Inter")
                                            font.pixelSize: n("font-size-small", 12)
                                        }
                                    }
                                    LinkText { text: "Save template"; action: function() { app.applySettings(JSON.stringify({ compose: { template: templateField.text } })) } }
                                    Text { text: "SERVER TEMPLATES"; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 9); font.letterSpacing: 1; Layout.topMargin: n("space-2", 8) }
                                    Repeater {
                                        model: root.serverTemplates
                                        delegate: Text { text: modelData.name + " · " + modelData.category; color: c("text-2", "#dbebe7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-small", 12) }
                                    }
                                    Text { text: "SERVER SIGNATURES"; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 9); font.letterSpacing: 1; Layout.topMargin: n("space-2", 8) }
                                    Repeater {
                                        model: root.serverSignatures
                                        delegate: Text { text: modelData.name + (modelData.is_default ? " · default" : ""); color: c("text-2", "#dbebe7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-small", 12) }
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
                                            radius: n("radius-control", 2)
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
                                                    Text { text: (modelData.conditions ? modelData.conditions.length : 0) + " conditions · " + (modelData.actions ? modelData.actions.length : 0) + " actions · " + (modelData.applied_count || 0) + " applied"; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 11) }
                                                }
                                                LinkText { text: "Run"; action: function() { app.runRule(modelData.id) } }
                                                LinkText { text: "Delete"; action: function() { app.deleteRule(modelData.id) } }
                                            }
                                        }
                                    }
                                    Text { visible: root.rules.length === 0; text: "No rules yet. Create one below."; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-small", 12) }
                                    Text { text: "New rule"; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 11); font.weight: n("font-weight-medium", 500); Layout.topMargin: n("space-2", 8) }
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
                                    Text { text: "Actions"; color: c("text-2", "#dbebe7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-small", 12); font.weight: n("font-weight-medium", 500) }
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
                                        font.pixelSize: n("font-size-micro", 11)
                                        wrapMode: Text.WordWrap
                                        Layout.fillWidth: true
                                    }
                                    Text { text: "New rule (full JSON)"; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 11); font.weight: n("font-weight-medium", 500); Layout.topMargin: n("space-2", 8) }
                                    Rectangle {
                                        Layout.fillWidth: true
                                        Layout.preferredHeight: 150
                                        radius: n("radius-control", 2)
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
                                            font.pixelSize: n("font-size-micro", 11)
                                        }
                                    }
                                    RowLayout {
                                        Layout.fillWidth: true
                                        LinkText { text: "Create rule"; action: function() { app.createRuleJson(ruleJson.text) } }
                                        Item { Layout.fillWidth: true }
                                        Text { text: "Mailbox tokens are shown in Aliases & mailboxes."; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 11) }
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
                                        LinkText {
                                            visible: root.aliasSelection.length > 0
                                            text: "Route selected (" + root.aliasSelection.length + ")"
                                            action: function() { root.routeSelectedAliases() }
                                        }
                                    }
                                    Repeater {
                                        model: root.identities
                                        delegate: Rectangle {
                                            Layout.fillWidth: true
                                            implicitHeight: aliasRow.implicitHeight + n("space-3", 12) * 2
                                            radius: n("radius-control", 2)
                                            color: c("card", "#23262c")
                                            border.width: n("border-width", 1)
                                            border.color: c("border", "#171b1e23")
                                            RowLayout {
                                                id: aliasRow
                                                anchors.fill: parent
                                                anchors.margins: n("space-3", 12)
                                                spacing: n("space-2", 8)
                                                SelectionBox {
                                                    checked: root.aliasIsSelected(modelData.address)
                                                    label: "Select " + modelData.address
                                                    onToggled: root.toggleAlias(modelData.address)
                                                    Layout.alignment: Qt.AlignVCenter
                                                }
                                                ColumnLayout {
                                                    spacing: 2
                                                    Layout.fillWidth: true
                                                    Text { text: modelData.address; color: c("text", "#ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-small", 12); font.weight: n("font-weight-medium", 500) }
                                                    Text { text: modelData.kind + (modelData.is_default ? " · default" : ""); color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 11) }
                                                }
                                                LinkText { text: "Route to mailbox"; action: function() { app.routeAlias(modelData.address, newMailboxName.text) } }
                                            }
                                        }
                                    }
                                    Text { text: "TAGS"; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 9); font.letterSpacing: 1; Layout.topMargin: n("space-2", 8) }
                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: n("space-2", 8)
                                        SettingsField { id: newTagName; Layout.fillWidth: true; placeholder: "New tag name" }
                                        LinkText { text: "Create tag"; action: function() { app.createTag(newTagName.text) } }
                                    }
                                    Repeater {
                                        model: root.tags
                                        delegate: RowLayout {
                                            Layout.fillWidth: true
                                            spacing: n("space-2", 8)
                                            Text { text: modelData.name; color: c("text-2", "#dbebe7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-small", 12); Layout.fillWidth: true }
                                            LinkText { text: "Delete"; action: function() { app.deleteTag(modelData.tag_token) } }
                                        }
                                    }
                                    Text { visible: root.tags.length === 0; text: "No tags yet."; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 11) }
                                    Text { text: "FOLDER MANAGEMENT"; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 9); font.letterSpacing: 1; Layout.topMargin: n("space-2", 8) }
                                    SettingsField { id: folderRenameName; Layout.fillWidth: true; placeholder: "New folder name" }
                                    Repeater {
                                        model: root.customFolders
                                        delegate: RowLayout {
                                            Layout.fillWidth: true
                                            spacing: n("space-2", 8)
                                            Text { text: modelData.name; color: c("text-2", "#dbebe7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-small", 12); Layout.fillWidth: true }
                                            LinkText { text: "Rename"; action: function() { app.renameFolder(modelData.token, folderRenameName.text) } }
                                            LinkText { text: "Delete"; action: function() { app.deleteFolder(modelData.token) } }
                                        }
                                    }
                                    Text { visible: root.customFolders.length === 0; text: "No custom folders yet."; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 11) }
                                    Text { text: "Mailbox tokens"; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 11); font.weight: n("font-weight-medium", 500); Layout.topMargin: n("space-2", 8) }
                                    Repeater {
                                        model: root.customFolders
                                        delegate: Text {
                                            text: modelData.name + "  " + (modelData.token || "")
                                            color: c("muted", "#a8ece7de")
                                            font.family: c("font-mono", "JetBrains Mono")
                                            font.pixelSize: n("font-size-micro", 11)
                                        }
                                    }
                                    Text { visible: root.customFolders.length === 0; text: "No custom mailboxes yet."; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 11) }
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
                                        radius: n("radius-control", 2)
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
                                            font.pixelSize: n("font-size-micro", 11)
                                        }
                                    }
                                    Repeater {
                                        model: root.contacts
                                        delegate: Rectangle {
                                            Layout.fillWidth: true
                                            implicitHeight: contactRow.implicitHeight + n("space-3", 12) * 2
                                            radius: n("radius-control", 2)
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
                                                    Text { text: (modelData.vcard || "").substring(0, 120); color: c("muted", "#a8ece7de"); font.family: c("font-mono", "JetBrains Mono"); font.pixelSize: n("font-size-micro", 11); elide: Text.ElideRight; Layout.fillWidth: true }
                                                }
                                                LinkText { text: "Delete"; action: function() { app.deleteContact(modelData.uid) } }
                                            }
                                        }
                                    }
                                    Text { visible: root.contacts.length === 0; text: "No contacts loaded."; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 11) }
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
                                        radius: n("radius-control", 2)
                                        color: c("card", "#23262c")
                                        border.width: n("border-width", 1)
                                        border.color: c("border", "#171b1e23")
                                        ColumnLayout {
                                            id: pgpCol
                                            anchors.fill: parent
                                            anchors.margins: n("space-3", 12)
                                            spacing: 3
                                            Text { text: root.pgp.fingerprint ? "PGP key" : "PGP key unavailable"; color: c("text", "#ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-small", 12); font.weight: n("font-weight-semibold", 600) }
                                            Text { text: root.pgp.fingerprint || ""; color: c("muted", "#a8ece7de"); font.family: c("font-mono", "JetBrains Mono"); font.pixelSize: n("font-size-micro", 11); wrapMode: Text.WrapAnywhere; Layout.fillWidth: true }
                                            Text { text: (root.pgp.algorithm || "") + " " + (root.pgp.key_size || "") + " · created " + (root.pgp.created_at || ""); color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 11); wrapMode: Text.WordWrap; Layout.fillWidth: true }
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
                                    Rectangle {
                                        Layout.fillWidth: true
                                        implicitHeight: gpgCol.implicitHeight + n("space-3", 12) * 2
                                        radius: n("radius-control", 8)
                                        color: c("card", "#23262c")
                                        border.width: n("border-width", 1)
                                        border.color: c("border", "#171b1e23")
                                        ColumnLayout {
                                            id: gpgCol
                                            anchors.fill: parent
                                            anchors.margins: n("space-3", 12)
                                            spacing: 3
                                            Text { text: "System GnuPG agent"; color: c("text", "#ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-small", 12); font.weight: n("font-weight-semibold", 600) }
                                            Text { text: root.gpg.installed ? (root.gpg.version || "gpg detected") : "gpg not detected"; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 9); wrapMode: Text.WordWrap; Layout.fillWidth: true }
                                            Text { visible: root.gpg.agent_socket; text: root.gpg.agent_socket || ""; color: c("muted", "#a8ece7de"); font.family: c("font-mono", "JetBrains Mono"); font.pixelSize: n("font-size-micro", 9); elide: Text.ElideMiddle; Layout.fillWidth: true }
                                            Text { text: (root.gpg.secret_keys ? root.gpg.secret_keys.length : 0) + " secret keys in gpg-agent"; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 9) }
                                            Text {
                                                text: root.settings.security && root.settings.security.gpg_key_fingerprint
                                                      ? "Selected key: " + root.settings.security.gpg_key_fingerprint
                                                      : "No key selected"
                                                color: c("muted", "#a8ece7de")
                                                font.family: c("font-mono", "JetBrains Mono")
                                                font.pixelSize: n("font-size-micro", 9)
                                                elide: Text.ElideMiddle
                                                Layout.fillWidth: true
                                            }
                                            Repeater {
                                                model: root.gpg.secret_keys || []
                                                delegate: RowLayout {
                                                    Layout.fillWidth: true
                                                    spacing: n("space-2", 8)
                                                    Text {
                                                        text: modelData.uid + "  " + modelData.fingerprint.substring(0, 16)
                                                        color: c("text-2", "#dbebe7de")
                                                        font.family: c("font-family", "Inter")
                                                        font.pixelSize: n("font-size-micro", 9)
                                                        elide: Text.ElideRight
                                                        Layout.fillWidth: true
                                                    }
                                                    LinkText {
                                                        text: root.settings.security && root.settings.security.gpg_key_fingerprint === modelData.fingerprint ? "Using" : "Use"
                                                        action: function() {
                                                            app.applySettings(JSON.stringify({ security: { gpg_key_fingerprint: modelData.fingerprint } }))
                                                        }
                                                    }
                                                }
                                            }
                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: n("space-3", 12)
                                                Text { text: "Sign outgoing mail with gpg-agent"; color: c("text-2", "#dbebe7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-small", 12); Layout.fillWidth: true }
                                                Toggle {
                                                    checked: root.settings.security ? root.settings.security.gpg_sign_outgoing === true : false
                                                    onToggled: app.applySettings(JSON.stringify({ security: { gpg_sign_outgoing: !checked } }))
                                                }
                                            }
                                            RowLayout {
                                                Layout.fillWidth: true
                                                LinkText { text: "Import Aster key"; action: function() { app.importGpgKey() } }
                                                LinkText { text: "Refresh"; action: function() { app.refreshGpgStatus() } }
                                                Item { Layout.fillWidth: true }
                                            }
                                        }
                                    }
                                    Text {
                                        text: "Private key export is pending. Using the system gpg-agent for private key operations is the preferred safe path."
                                        color: c("muted", "#a8ece7de")
                                        font.family: c("font-family", "Inter")
                                        font.pixelSize: n("font-size-micro", 11)
                                        wrapMode: Text.WordWrap
                                        Layout.fillWidth: true
                                    }
                                    RowLayout {
                                        Layout.fillWidth: true
                                        ColumnLayout { spacing: 2; Layout.fillWidth: true
                                            Text { text: "Require post-quantum protection"; color: c("text", "#ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-small", 12); font.weight: n("font-weight-medium", 500) }
                                            Text { text: "Block sends to Aster recipients whose keys do not support PQ."; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 11); wrapMode: Text.WordWrap; Layout.fillWidth: true }
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
                                            Text { text: "Use Aster end-to-end encryption whenever recipient keys are available."; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 11); wrapMode: Text.WordWrap; Layout.fillWidth: true }
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
                                    Text { text: "Data directory"; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 11); font.weight: n("font-weight-medium", 500) }
                                    Text { text: app.dataDir; color: c("text-2", "#dbebe7de"); font.family: c("font-mono", "JetBrains Mono"); font.pixelSize: n("font-size-micro", 11); wrapMode: Text.WrapAnywhere; Layout.fillWidth: true }
                                    RowLayout {
                                        Layout.fillWidth: true
                                        LinkText { text: "Open themes folder"; action: function() { app.openThemesFolder() } }
                                        Item { Layout.fillWidth: true }
                                        LinkText { text: "Reload themes"; action: function() { app.refreshThemes() } }
                                        LinkText { text: "Refresh mail"; action: function() { app.refreshMail() } }
                                    }
                                    Text { text: app.toastText; color: c("muted", "#a8ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-micro", 11); wrapMode: Text.WordWrap; Layout.fillWidth: true }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Rectangle {
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: n("statusbar-height", 30) + n("space-4", 16) + 40
        width: undoSendRow.implicitWidth + n("space-4", 16) * 2
        height: 34
        radius: n("radius-control", 8)
        color: c("surface-strong", "#2e323a")
        border.width: n("border-width", 1)
        border.color: c("border-strong", "#292e323a")
        visible: root.sendUndoVisible
        Row {
            id: undoSendRow
            anchors.centerIn: parent
            spacing: n("space-3", 12)
            Text { text: "Sending in 5 seconds"; color: c("text", "#ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-small", 12) }
            LinkText { text: "Undo"; action: function() { root.cancelPendingSend() } }
        }
    }

    Rectangle {
        anchors.fill: parent
        color: c("overlay", "#8c000000")
        visible: root.emptyConfirmOpen
        MouseArea { anchors.fill: parent }
        Rectangle {
            width: 420; height: 190
            anchors.centerIn: parent
            radius: n("radius-panel", 18)
            color: c("surface", "#1b1e23")
            border.width: n("border-width", 1)
            border.color: c("border-strong", "#292e323a")
            ColumnLayout {
                anchors.fill: parent
                anchors.margins: n("space-5", 24)
                spacing: n("space-3", 12)
                Text { text: "Empty " + root.folderLabel(app.folderId) + "?"; color: c("text", "#ece7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-title", 16); font.weight: n("font-weight-semibold", 600) }
                Text { text: "Every message in this folder will be permanently deleted. This cannot be undone."; color: c("text-2", "#dbebe7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-small", 12); wrapMode: Text.WordWrap; Layout.fillWidth: true }
                Item { Layout.fillHeight: true }
                RowLayout {
                    Layout.fillWidth: true
                    Item { Layout.fillWidth: true }
                    LinkText { text: "Cancel"; action: function() { root.emptyConfirmOpen = false } }
                    LinkText { text: "Empty folder"; action: function() { app.emptyFolder(app.folderId); root.emptyConfirmOpen = false } }
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
            radius: n("radius-panel", 8)
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
                        radius: n("radius-control", 2)
                        color: cancelHover.hovered ? c("hover", "#2e2e323a") : "transparent"
                        HoverHandler { id: cancelHover }
                        Text { anchors.centerIn: parent; text: "Cancel"; color: c("text-2", "#dbebe7de"); font.family: c("font-family", "Inter"); font.pixelSize: n("font-size-small", 12) }
                        MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: { app.resolveDelete(root.pendingDeleteToken, false); root.deleteConfirmOpen = false; } }
                    }
                    Rectangle {
                        implicitWidth: 96
                        implicitHeight: n("control-height", 36)
                        radius: n("radius-control", 2)
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
            radius: n("radius-panel", 8)
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
                        font.pixelSize: n("font-size-micro", 11)
                    }
                    IconButton { icon: "close"; label: "Close composer"; onClicked: root.composeOpen = false }
                }
                RowLayout {
                    Layout.fillWidth: true
                    spacing: n("space-2", 8)
                    Text {
                        text: "From"
                        color: c("muted", "#a8ece7de")
                        font.family: c("font-family", "Inter")
                        font.pixelSize: n("font-size-small", 12)
                        Layout.preferredWidth: 42
                        Layout.alignment: Qt.AlignVCenter
                    }
                    ComboBox {
                        id: composeFromBox
                        Layout.fillWidth: true
                        model: root.identities.map(function(item) { return item.address })
                        currentIndex: Math.max(0, root.identities.findIndex(function(item) { return item.address === root.composeFrom }))
                        onActivated: root.composeFrom = root.identities[index].address
                        onCountChanged: {
                            if (count > 0 && root.composeFrom.length === 0)
                                root.composeFrom = root.identities[currentIndex].address;
                        }
                        Accessible.name: "Send from"
                        background: Rectangle {
                            radius: n("radius-control", 2)
                            color: c("input", "#221b1e23")
                            border.width: n("border-width", 1)
                            border.color: composeFromBox.activeFocus ? c("accent", "#d98e5c") : c("border", "#171b1e23")
                        }
                        contentItem: Text {
                            leftPadding: n("space-3", 12)
                            text: composeFromBox.displayText
                            color: c("text", "#ece7de")
                            font.family: c("font-family", "Inter")
                            font.pixelSize: n("font-size-small", 12)
                            topPadding: 0
                            bottomPadding: 0
                            verticalAlignment: Text.AlignVCenter
                            elide: Text.ElideRight
                        }
                        indicator: Icon {
                            x: composeFromBox.width - width - n("space-3", 12)
                            anchors.verticalCenter: parent.verticalCenter
                            name: "chevronDown"
                            size: n("icon-small", 16)
                            color: c("muted", "#a8ece7de")
                        }
                    }
                }
                SettingsField { id: composeTo; Layout.fillWidth: true; placeholder: "To, comma separated" }
                RowLayout {
                    Layout.fillWidth: true
                    spacing: n("space-2", 8)
                    SettingsField { id: composeCc; Layout.fillWidth: true; placeholder: "Cc" }
                    SettingsField { id: composeBcc; Layout.fillWidth: true; placeholder: "Bcc" }
                }
                SettingsField { id: composeSubject; Layout.fillWidth: true; placeholder: "Subject" }
                RowLayout {
                    Layout.fillWidth: true
                    spacing: n("space-3", 12)
                    LinkText {
                        text: "Insert signature"
                        action: function() {
                            var signature = root.settings.compose ? (root.settings.compose.signature || "") : "";
                            if (signature.length > 0) composeBody.text += "\n\n" + signature;
                        }
                    }
                    LinkText {
                        text: "Insert template"
                        action: function() {
                            var template = root.settings.compose ? (root.settings.compose.template || "") : "";
                            if (template.length > 0) composeBody.text = template + "\n\n" + composeBody.text;
                        }
                    }
                    Item { Layout.fillWidth: true }
                    Text {
                        text: "Body · Markdown"
                        color: c("muted", "#a8ece7de")
                        font.family: c("font-family", "Inter")
                        font.pixelSize: n("font-size-micro", 11)
                        font.weight: n("font-weight-medium", 500)
                    }
                }
                Rectangle {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    radius: n("radius-control", 2)
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
                Flow {
                    Layout.fillWidth: true
                    spacing: n("space-2", 8)
                    visible: root.composeAttachments.length > 0
                    Repeater {
                        model: root.composeAttachments
                        delegate: Rectangle {
                            id: attachmentChip
                            implicitWidth: attachmentChipRow.implicitWidth + n("space-3", 12) * 2
                            implicitHeight: 26
                            radius: n("radius-control", 2)
                            color: c("card", "#23262c")
                            border.width: n("border-width", 1)
                            border.color: c("border", "#171b1e23")
                            RowLayout {
                                id: attachmentChipRow
                                anchors.centerIn: parent
                                spacing: n("space-1", 4)
                                Icon { name: "attach"; size: n("icon-small", 16); color: c("muted", "#a8ece7de") }
                                Text {
                                    text: String(modelData).split("/").pop()
                                    color: c("text-2", "#dbebe7de")
                                    font.family: c("font-family", "Inter")
                                    font.pixelSize: n("font-size-small", 12)
                                    elide: Text.ElideRight
                                    Layout.maximumWidth: 180
                                }
                                Icon { name: "close"; size: 12; color: c("muted", "#a8ece7de") }
                            }
                            MouseArea {
                                anchors.fill: parent
                                cursorShape: Qt.PointingHandCursor
                                onClicked: {
                                    var next = root.composeAttachments.slice();
                                    next.splice(index, 1);
                                    root.composeAttachments = next;
                                }
                            }
                        }
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: n("space-2", 8)
                    SettingsField { id: scheduleAtField; Layout.fillWidth: true; placeholder: "Send at YYYY-MM-DD HH:MM" }
                    LinkText { text: "Schedule"; action: function() { root.scheduleCurrentSend() } }
                }
                RowLayout {
                    Layout.fillWidth: true
                    spacing: n("space-2", 8)
                    IconButton { icon: "attach"; label: "Attach"; onClicked: attachDialog.open() }
                    Item { Layout.fillWidth: true }
                    Text {
                        text: "Markdown is converted to HTML and plain text. Reference an attached image by filename to embed it inline."
                        color: c("muted", "#a8ece7de")
                        font.family: c("font-family", "Inter")
                        font.pixelSize: n("font-size-micro", 11)
                    }
                    Rectangle {
                        implicitWidth: 96
                        implicitHeight: n("control-height", 36)
                        radius: n("radius-control", 2)
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
                            onClicked: root.sendWithUndo()
                        }
                    }
                }
            }
        }
    }

    Popup {
        id: identityPopup
        width: accountCard.width
        x: accountCard.mapToItem(null, 0, accountCard.height).x
        y: accountCard.mapToItem(null, 0, accountCard.height).y
        modal: false
        focus: true
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
        onOpened: app.loadSettingsData()
        background: Rectangle {
            color: c("menu", "#23262c")
            border.width: n("border-width", 1)
            border.color: c("border-strong", "#292e323a")
            radius: n("radius-panel", 8)
        }
        ColumnLayout {
            anchors.fill: parent
            anchors.margins: n("space-2", 8)
            spacing: n("space-1", 4)
            Text {
                text: "Send from"
                color: c("muted", "#a8ece7de")
                font.family: c("font-family", "Inter")
                font.pixelSize: n("font-size-micro", 11)
                font.weight: n("font-weight-medium", 500)
                Layout.leftMargin: n("space-2", 8)
            }
            Repeater {
                model: root.identities
                delegate: Rectangle {
                    id: identityRow
                    property bool active: modelData.address === root.activeIdentity
                    Layout.fillWidth: true
                    implicitHeight: 40
                    radius: n("radius-control", 2)
                    color: identityRow.active ? c("selected", "#3d2e323a")
                         : (identityHover.hovered ? c("hover", "#2e2e323a") : "transparent")
                    activeFocusOnTab: true
                    Accessible.role: Accessible.Button
                    Accessible.name: "Send from " + modelData.address
                    Accessible.onPressAction: root.useIdentity(modelData.address)
                    Keys.onSpacePressed: root.useIdentity(modelData.address)
                    Keys.onReturnPressed: root.useIdentity(modelData.address)
                    Keys.onEnterPressed: root.useIdentity(modelData.address)
                    border.width: identityRow.activeFocus ? n("border-width-focus", 1.5) : 0
                    border.color: c("focus", "#d98e5c")
                    HoverHandler { id: identityHover }
                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: n("space-2", 8)
                        anchors.rightMargin: n("space-2", 8)
                        spacing: n("space-2", 8)
                        Icon {
                            name: "check"
                            size: n("icon-small", 16)
                            color: c("accent-text", "#e7a97f")
                            visible: identityRow.active
                        }
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 1
                            Text {
                                text: modelData.address
                                color: c("text", "#ece7de")
                                font.family: c("font-family", "Inter")
                                font.pixelSize: n("font-size-small", 12)
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                            }
                            Text {
                                visible: modelData.is_default && !identityRow.active
                                text: "default"
                                color: c("muted", "#a8ece7de")
                                font.family: c("font-family", "Inter")
                                font.pixelSize: n("font-size-micro", 11)
                            }
                        }
                    }
                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.useIdentity(modelData.address)
                    }
                }
            }
            Text {
                visible: root.identities.length === 0
                text: "No aliases available."
                color: c("muted", "#a8ece7de")
                font.family: c("font-family", "Inter")
                font.pixelSize: n("font-size-small", 12)
            }
        }
    }

    Popup {
        id: searchFilterPopup
        width: 460
        height: 430
        modal: true
        focus: true
        anchors.centerIn: Overlay.overlay
        background: Rectangle {
            color: c("menu", "#23262c")
            border.width: n("border-width", 1)
            border.color: c("border-strong", "#292e323a")
            radius: n("radius-panel", 8)
        }
        ColumnLayout {
            anchors.fill: parent
            anchors.margins: n("space-4", 16)
            spacing: n("space-2", 8)
            RowLayout {
                Layout.fillWidth: true
                Text {
                    text: "Search filters"
                    color: c("text", "#ece7de")
                    font.family: c("font-family", "Inter")
                    font.pixelSize: n("font-size-title", 16)
                    font.weight: n("font-weight-semibold", 600)
                    Layout.fillWidth: true
                }
                LinkText { text: "Clear"; action: function() { root.clearSearchFilters() } }
                IconButton { icon: "close"; label: "Close filters"; onClicked: searchFilterPopup.close() }
            }
            SettingsField { placeholder: "From contains"; text: root.filterFrom; onEdited: root.filterFrom = value }
            SettingsField { placeholder: "Subject contains"; text: root.filterSubject; onEdited: root.filterSubject = value }
            SettingsField { placeholder: "Body or preview contains"; text: root.filterBody; onEdited: root.filterBody = value }
            RowLayout {
                Layout.fillWidth: true
                spacing: n("space-2", 8)
                SettingsField { Layout.fillWidth: true; placeholder: "Since YYYY-MM-DD"; text: root.filterSince; onEdited: root.filterSince = value }
                SettingsField { Layout.fillWidth: true; placeholder: "Until YYYY-MM-DD"; text: root.filterUntil; onEdited: root.filterUntil = value }
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: n("space-2", 8)
                Text {
                    text: "Starred only"
                    color: c("text-2", "#dbebe7de")
                    font.family: c("font-family", "Inter")
                    font.pixelSize: n("font-size-small", 12)
                    Layout.fillWidth: true
                }
                Toggle { checked: root.filterStarred; onToggled: root.filterStarred = !root.filterStarred }
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: n("space-2", 8)
                Text {
                    text: "With attachments"
                    color: c("text-2", "#dbebe7de")
                    font.family: c("font-family", "Inter")
                    font.pixelSize: n("font-size-small", 12)
                    Layout.fillWidth: true
                }
                Toggle { checked: root.filterAttachmentsOnly; onToggled: root.filterAttachmentsOnly = !root.filterAttachmentsOnly }
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: n("space-2", 8)
                SettingsField { id: savedSearchName; Layout.fillWidth: true; placeholder: "Saved search name" }
                LinkText { text: "Save search"; action: function() { root.saveCurrentSearch(savedSearchName.text) } }
            }
            Item { Layout.fillHeight: true }
            Text {
                text: "Filters apply to the current mailbox and search."
                color: c("muted", "#a8ece7de")
                font.family: c("font-family", "Inter")
                font.pixelSize: n("font-size-micro", 11)
                Layout.fillWidth: true
            }
        }
    }

    Popup {
        id: outboxPopup
        width: 560
        height: 420
        modal: true
        focus: true
        anchors.centerIn: Overlay.overlay
        onOpened: app.loadOutbox()
        background: Rectangle {
            color: c("menu", "#23262c")
            border.width: n("border-width", 1)
            border.color: c("border-strong", "#292e323a")
            radius: n("radius-panel", 8)
        }
        ColumnLayout {
            anchors.fill: parent
            anchors.margins: n("space-4", 16)
            spacing: n("space-2", 8)
            RowLayout {
                Layout.fillWidth: true
                Text {
                    text: "Outbox"
                    color: c("text", "#ece7de")
                    font.family: c("font-family", "Inter")
                    font.pixelSize: n("font-size-title", 16)
                    font.weight: n("font-weight-semibold", 600)
                    Layout.fillWidth: true
                }
                IconButton { icon: "close"; label: "Close outbox"; onClicked: outboxPopup.close() }
            }
            Text {
                visible: root.outboxItems.length === 0
                text: "Nothing is waiting to send."
                color: c("muted", "#a8ece7de")
                font.family: c("font-family", "Inter")
                font.pixelSize: n("font-size-small", 12)
            }
            ListView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                model: root.outboxItems
                clip: true
                spacing: n("space-1", 4)
                ScrollBar.vertical: MailScrollBar {}
                delegate: Rectangle {
                    width: ListView.view.width
                    implicitHeight: outboxRow.implicitHeight + n("space-3", 12) * 2
                    radius: n("radius-control", 2)
                    color: c("card", "#23262c")
                    border.width: n("border-width", 1)
                    border.color: c("border", "#171b1e23")
                    RowLayout {
                        id: outboxRow
                        anchors.fill: parent
                        anchors.margins: n("space-3", 12)
                        spacing: n("space-2", 8)
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 2
                            Text {
                                text: modelData.envelope_to
                                color: c("text", "#ece7de")
                                font.family: c("font-family", "Inter")
                                font.pixelSize: n("font-size-small", 12)
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                            }
                            Text {
                                text: modelData.status + " · " + modelData.attempts + " attempts"
                                    + (modelData.last_error ? " · " + modelData.last_error : "")
                                color: c("muted", "#a8ece7de")
                                font.family: c("font-family", "Inter")
                                font.pixelSize: n("font-size-micro", 11)
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                            }
                        }
                        LinkText {
                            text: "Retry"
                            action: function() { app.retryOutbox(modelData.id) }
                        }
                    }
                }
            }
        }
    }
}
