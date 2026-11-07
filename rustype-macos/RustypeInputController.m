/**
 * Rustype macOS Input Controller (IMKInputController)
 * Bridges macOS InputMethodKit with rustype-core C ABI.
 */

#import <Cocoa/Cocoa.h>
#import <InputMethodKit/InputMethodKit.h>
#include "rustype.h"

@interface RustypeInputController : IMKInputController {
    RustypeTelex* _telex;
}
@end

@implementation RustypeInputController

- (id)initWithServer:(IMKServer *)server delegate:(id)delegate client:(id)client {
    self = [super initWithServer:server delegate:delegate client:client];
    if (self) {
        _telex = rustype_telex_new(false, true);
    }
    return self;
}

- (void)dealloc {
    if (_telex) {
        rustype_telex_free(_telex);
        _telex = NULL;
    }
}

- (BOOL)handleEvent:(NSEvent *)event client:(id)sender {
    if (!event) return NO;

    NSEventType type = [event type];
    if (type != NSEventTypeKeyDown) {
        return NO;
    }

    NSUInteger modifiers = [event modifierFlags] & NSEventModifierFlagDeviceIndependentFlagsMask;
    // Bypass Command and Control shortcuts
    if ((modifiers & NSEventModifierFlagCommand) || (modifiers & NSEventModifierFlagControl)) {
        if (_telex) rustype_telex_reset(_telex);
        return NO;
    }

    // Handle Backspace (virtual key code 51 on macOS)
    if ([event keyCode] == 51) {
        if (!_telex) return NO;
        bool handled = false;
        size_t backspaces = 0;
        char commit[64] = {0};

        if (rustype_telex_backspace(_telex, &handled, &backspaces, commit, sizeof(commit))) {
            if (backspaces > 0 && [sender respondsToSelector:@selector(deleteBackward:)]) {
                for (size_t i = 0; i < backspaces; i++) {
                    [sender deleteBackward:self];
                }
            }
            if (commit[0] != '\0') {
                NSString* str = [NSString stringWithUTF8String:commit];
                if (str && [sender respondsToSelector:@selector(insertText:replacementRange:)]) {
                    [sender insertText:str replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
                }
            }
            return YES;
        }
        return NO;
    }

    return NO;
}

- (BOOL)inputText:(NSString *)string client:(id)sender {
    if (!string || [string length] == 0 || !_telex) {
        return NO;
    }

    UTF32Char codepoint = [string characterAtIndex:0];
    if ([string length] > 1) {
        codepoint = [string UTF8String][0]; // Basic single char
    }

    bool handled = false;
    size_t backspaces = 0;
    char commit[64] = {0};

    if (rustype_telex_key(_telex, codepoint, &handled, &backspaces, commit, sizeof(commit))) {
        // Delete previous characters
        if (backspaces > 0 && [sender respondsToSelector:@selector(deleteBackward:)]) {
            for (size_t i = 0; i < backspaces; i++) {
                [sender deleteBackward:self];
            }
        }
        // Commit new composed text
        if (commit[0] != '\0') {
            NSString* str = [NSString stringWithUTF8String:commit];
            if (str && [sender respondsToSelector:@selector(insertText:replacementRange:)]) {
                [sender insertText:str replacementRange:NSMakeRange(NSNotFound, NSNotFound)];
            }
        }
        return YES;
    }

    return NO;
}

@end
