#include <fcitx/addonfactory.h>
#include <fcitx/addonmanager.h>
#include <fcitx/inputcontext.h>
#include <fcitx/inputcontextproperty.h>
#include <fcitx/inputmethodengine.h>
#include <fcitx/instance.h>
#include <fcitx/inputcontextmanager.h>
#include <fcitx-utils/key.h>
#include "rustype.h"

namespace fcitx {

namespace {

// Per-input-context composition state. Owns the rustype-core handle.
class TelexProperty : public InputContextProperty {
public:
    TelexProperty() : telex_(rustype_telex_new(false, true)) {}
    ~TelexProperty() override { rustype_telex_free(telex_); }

    TelexProperty(const TelexProperty &) = delete;
    TelexProperty &operator=(const TelexProperty &) = delete;

    RustypeTelex *telex() const { return telex_; }

private:
    RustypeTelex *telex_;
};

// Large enough for a restored long word or an expanded macro.
constexpr size_t kCommitBufSize = 4096;

} // namespace

class RustypeEngine : public InputMethodEngineV2 {
public:
    RustypeEngine(Instance *instance) : instance_(instance) {
        instance_->inputContextManager().registerProperty("rustypeTelex", &telexPropKey_);
    }
    ~RustypeEngine() override = default;

    void keyEvent(const InputMethodEntry &entry, KeyEvent &keyEvent) override {
        FCITX_UNUSED(entry);
        if (keyEvent.isRelease()) {
            return;
        }

        auto *ic = keyEvent.inputContext();
        if (!ic) {
            return;
        }

        auto sym = keyEvent.key().sym();
        auto states = keyEvent.key().states();

        // Pass shortcuts (Ctrl, Alt, Super)
        if (states.testAny(KeyStates{KeyState::Ctrl, KeyState::Alt, KeyState::Super})) {
            reset(entry, keyEvent);
            return;
        }

        auto *telex = getOrCreateTelex(ic);
        if (!telex) {
            return;
        }

        if (sym == FcitxKey_BackSpace) {
            bool handled = false;
            size_t backspaces = 0;
            char commit_buf[kCommitBufSize] = {0};
            if (rustype_telex_backspace(telex, &handled, &backspaces, commit_buf, sizeof(commit_buf))) {
                if (backspaces > 0) {
                    ic->deleteSurroundingText(-(int)backspaces, backspaces);
                }
                if (commit_buf[0] != '\0') {
                    ic->commitString(commit_buf);
                }
                keyEvent.filterAndAccept();
            }
            return;
        }

        if (sym >= 0x20 && sym <= 0x7e) {
            bool handled = false;
            size_t backspaces = 0;
            char commit_buf[kCommitBufSize] = {0};
            if (rustype_telex_key(telex, (uint32_t)sym, &handled, &backspaces, commit_buf, sizeof(commit_buf))) {
                if (backspaces > 0) {
                    ic->deleteSurroundingText(-(int)backspaces, backspaces);
                }
                if (commit_buf[0] != '\0') {
                    ic->commitString(commit_buf);
                }
                keyEvent.filterAndAccept();
                return;
            }
        }

        // Other keys end composition
        rustype_telex_reset(telex);
    }

    void reset(const InputMethodEntry &entry, InputContextEvent &event) override {
        FCITX_UNUSED(entry);
        if (auto *telex = getTelex(event.inputContext())) {
            rustype_telex_reset(telex);
        }
    }

private:
    RustypeTelex *getOrCreateTelex(InputContext *ic) {
        // The factory creates the property on first access.
        auto *prop = ic->propertyFor(&telexPropKey_);
        return prop ? prop->telex() : nullptr;
    }

    RustypeTelex *getTelex(InputContext *ic) {
        if (!ic) {
            return nullptr;
        }
        return getOrCreateTelex(ic);
    }

    Instance *instance_;
    SimpleInputContextPropertyFactory<TelexProperty> telexPropKey_;
};

class RustypeEngineFactory : public AddonFactory {
public:
    AddonInstance *create(AddonManager *manager) override {
        return new RustypeEngine(manager->instance());
    }
};

} // namespace fcitx

FCITX_ADDON_FACTORY(fcitx::RustypeEngineFactory);
