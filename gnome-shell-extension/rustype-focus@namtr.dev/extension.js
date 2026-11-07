import { Extension } from 'resource:///org/gnome/shell/extensions/extension.js';
import Shell from 'gi://Shell';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import IBus from 'gi://IBus';

const RUSTYPE_BUS_NAME = 'org.freedesktop.IBus.Rustype';
const RUSTYPE_PATH = '/org/freedesktop/IBus/Factory';
const RUSTYPE_IFACE = 'org.freedesktop.IBus.Rustype';

export default class RustypeFocusExtension extends Extension {
    enable() {
        this._tracker = Shell.WindowTracker.get_default();
        this._ibus = IBus.Bus.new();

        this._signalId = global.display.connect('notify::focus-window', () => {
            this._onFocusChanged();
        });

        // Report initial focus
        GLib.idle_add(GLib.PRIORITY_DEFAULT_IDLE, () => {
            this._onFocusChanged();
            return GLib.SOURCE_REMOVE;
        });
    }

    _onFocusChanged() {
        if (!this._tracker) return;
        const app = this._tracker.focus_app;
        const appId = app ? app.get_id() : '';
        if (appId) {
            this._notifyRustype(appId);
        }
    }

    _notifyRustype(appId) {
        try {
            if (!this._ibus || !this._ibus.is_connected()) {
                return;
            }
            const conn = this._ibus.get_connection();
            if (!conn) {
                return;
            }
            conn.call(
                RUSTYPE_BUS_NAME,
                RUSTYPE_PATH,
                RUSTYPE_IFACE,
                'SetFocusApp',
                new GLib.Variant('(s)', [appId]),
                null,
                Gio.DBusCallFlags.NO_AUTO_START,
                1000,
                null,
                () => {} // callback ignore
            );
        } catch (_err) {
            // Ignore transient errors
        }
    }

    disable() {
        if (this._signalId) {
            global.display.disconnect(this._signalId);
            this._signalId = null;
        }
        if (this._ibus) {
            this._ibus.destroy();
            this._ibus = null;
        }
        this._tracker = null;
    }
}
