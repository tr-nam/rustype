/**
 * Rustype macOS Input Method Server (main.m)
 * Powered by pure Rust rustype-core via C ABI.
 */

#import <Cocoa/Cocoa.h>
#import <InputMethodKit/InputMethodKit.h>

const NSString* kConnectionName = @"Rustype_Connection";
IMKServer* server;

int main(int argc, const char * argv[]) {
    @autoreleasepool {
        NSString* identifier = [[NSBundle mainBundle] bundleIdentifier];
        server = [[IMKServer alloc] initWithName:(NSString*)kConnectionName
                                bundleIdentifier:identifier];
        [[NSApplication sharedApplication] run];
    }
    return 0;
}
