#include <fcitx/inputmethodengine.h>
#include <fcitx/instance.h>
#include <fcitx/addonfactory.h>
#include <fcitx/inputcontext.h>
#include "rustype.h"

namespace fcitx {

class RustypeEngine : public InputMethodEngineV2 {
public:
    RustypeEngine(Instance *instance) : instance_(instance) {}
    ~RustypeEngine() override = default;

    void keyEvent(const InputMethodEntry &entry, KeyEvent &keyEvent) override {
        FCITX_UNUSED(entry);
        if (keyEvent.isRelease()) {
            return;
        }

        auto ic = keyEvent.inputContext();
        if (!ic) {
            return;
        }

        auto sym = keyEvent.key().sym();
        auto states = keyEvent.key().states();

        // Pass shortcuts (Ctrl, Alt, Super)
        if (states & (KeyState::Ctrl | KeyState::Alt | KeyState::Super)) {
            reset(entry, ic);
            return;
        }

        auto *telex = getOrCreateTelex(ic);
        if (!telex) {
            return;
        }

        if (sym == FcitxKey_BackSpace) {
            bool handled = false;
            size_t backspaces = 0;
            char commit_buf[64] = {0};
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
            char commit_buf[64] = {0};
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

    void reset(const InputMethodEntry &, InputContext *ic) override {
        if (auto *telex = getTelex(ic)) {
            rustype_telex_reset(telex);
        }
    }

private:
    RustypeTelex* getOrCreateTelex(InputContext *ic) {
        auto &prop = ic->propertyFor(&telexPropKey_);
        if (!prop) {
            prop = std::unique_ptr<RustypeTelex, decltype(&rustype_telex_free)>(
                rustype_telex_new(false, true), &rustype_telex_free
            );
        }
        return static_cast<RustypeTelex*>(prop.get());
    }

    RustypeTelex* getTelex(InputContext *ic) {
        auto &prop = ic->propertyFor(&telexPropKey_);
        return prop ? static_cast<RustypeTelex*>(prop.get()) : nullptr;
    }

    Instance *instance_;
    SimpleInputContextPropertyFactory telexPropKey_;
};

class RustypeEngineFactory : public AddonFactory {
public:
    AddonInstance *create(AddonManager *manager) override {
        return new RustypeEngine(manager->instance());
    }
};

} // namespace fcitx

FCITX_ADDON_FACTORY(fcitx::RustypeEngineFactory);
