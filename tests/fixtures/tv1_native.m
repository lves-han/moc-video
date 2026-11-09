#import <Cocoa/Cocoa.h>
#import <ApplicationServices/ApplicationServices.h>
#include <unistd.h>
#include <math.h>

// Test-only native input and window observation, always bound to the owned PID.
static NSDictionary *windowFor(pid_t pid) {
    NSArray *windows = CFBridgingRelease(CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly, kCGNullWindowID));
    for (NSDictionary *window in windows) {
        if ([window[(id)kCGWindowOwnerPID] intValue] == pid && [window[(id)kCGWindowLayer] intValue] == 0) return window;
    }
    return nil;
}
static void mouse(CGEventType type, CGPoint point) {
    CGEventRef event = CGEventCreateMouseEvent(NULL, type, point, kCGMouseButtonLeft);
    CGEventPost(kCGHIDEventTap, event);
    CFRelease(event);
    usleep(100000);
}
int main(int argc, const char **argv) {
    @autoreleasepool {
        if (argc < 3) return 2;
        NSString *action = @(argv[1]);
        pid_t pid = atoi(argv[2]);
        NSMutableDictionary *result = [@{@"accessibility": @(AXIsProcessTrusted()), @"screen_recording": @(CGPreflightScreenCaptureAccess())} mutableCopy];
        if ([action isEqual:@"permissions"]) {
            // A diagnostic only; input is validated by the receiving WebView.
        } else {
            if ([action isEqual:@"click"]) {
                NSRunningApplication *owned = [NSRunningApplication runningApplicationWithProcessIdentifier:pid];
                if (!owned || ![owned activateWithOptions:NSApplicationActivateAllWindows]) {
                    fprintf(stderr, "Owned application activation rejected\n"); return 4;
                }
                usleep(200000);
            }
            NSDictionary *window = windowFor(pid);
            if (!window) { fprintf(stderr, "Owned visible window missing\n"); return 3; }
            result[@"id"] = window[(id)kCGWindowNumber];
            result[@"bounds"] = window[(id)kCGWindowBounds];
            CGRect frame;
            CGRectMakeWithDictionaryRepresentation((__bridge CFDictionaryRef)window[(id)kCGWindowBounds], &frame);
            if ([action isEqual:@"hover"] || [action isEqual:@"click"] || [action isEqual:@"drag"]) {
                if (argc != 5 && argc != 7) return 2;
                CGPoint start = CGPointMake(frame.origin.x + atof(argv[3]), frame.origin.y + atof(argv[4]));
                mouse(kCGEventMouseMoved, start);
                if (![action isEqual:@"hover"]) {
                    mouse(kCGEventLeftMouseDown, start);
                    if ([action isEqual:@"drag"]) {
                        if (argc != 7) return 2;
                        for (int i=1; i<=10; i++) mouse(kCGEventLeftMouseDragged, CGPointMake(start.x + atof(argv[5])*i/10, start.y + atof(argv[6])*i/10));
                        start.x += atof(argv[5]); start.y += atof(argv[6]);
                    }
                    mouse(kCGEventLeftMouseUp, start);
                }
            } else if ([action isEqual:@"key"]) {
                if (argc != 4) return 2;
                if (NSWorkspace.sharedWorkspace.frontmostApplication.processIdentifier != pid) {
                    fprintf(stderr, "Owned application must be frontmost for keyboard input\n"); return 5;
                }
                int code = atoi(argv[3]);
                if (code != 123 && code != 124 && code != 125 && code != 126 && code != 53 && code != 48 && code != 36 && code != 49 && code != 3 && code != 46) return 2;
                for (int down=1; down>=0; down--) {
                    CGEventRef event = CGEventCreateKeyboardEvent(NULL, code, down);
                    CGEventPost(kCGHIDEventTap, event); CFRelease(event); usleep(100000);
                }
            } else if ([action isEqual:@"pixel"]) {
                if (argc != 4) return 2;
                NSBitmapImageRep *capture = [NSBitmapImageRep imageRepWithContentsOfFile:@(argv[3])];
                NSColor *pixel = [[capture colorAtX:capture.pixelsWide / 2 y:capture.pixelsHigh / 2]
                    colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
                if (!capture || !pixel) { fprintf(stderr, "Owned window PNG read failed\n"); return 6; }
                result[@"pixel"] = @[@(lround(pixel.redComponent * 255)), @(lround(pixel.greenComponent * 255)),
                    @(lround(pixel.blueComponent * 255)), @(lround(pixel.alphaComponent * 255))];
                result[@"color_space"] = @"sRGB";
            } else if (![action isEqual:@"window"]) return 2;
        }
        NSData *data = [NSJSONSerialization dataWithJSONObject:result options:0 error:nil];
        puts([[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding].UTF8String);
    }
    return 0;
}
